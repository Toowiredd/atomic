//! Centroid-based wiki generation strategy
//!
//! Uses the tag's centroid embedding to rank chunks by semantic relevance,
//! then generates/updates articles via a single-shot LLM call.

use crate::chunking::count_tokens;
use crate::db::Database;
use crate::embedding::distance_to_similarity;
use crate::models::{ChunkWithContext, WikiArticle, WikiArticleWithCitations, WikiCitation};
use crate::providers::ProviderConfig;

use chrono::Utc;
use rusqlite::Connection;

use super::{
    call_llm_for_wiki, extract_citations, batch_fetch_chunk_details, count_atoms_with_tags,
    get_tag_hierarchy, synthesize_article, WikiStrategyContext, MAX_WIKI_SOURCE_TOKENS,
    WIKI_UPDATE_SYSTEM_PROMPT,
};

/// Data needed for wiki article generation (extracted before async call)
pub struct WikiGenerationInput {
    pub chunks: Vec<ChunkWithContext>,
    pub atom_count: i32,
    pub tag_id: String,
    pub tag_name: String,
}

/// Data needed for wiki article update (extracted before async call)
pub struct WikiUpdateInput {
    pub new_chunks: Vec<ChunkWithContext>,
    pub existing_article: WikiArticle,
    pub existing_citations: Vec<WikiCitation>,
    pub atom_count: i32,
    pub tag_id: String,
}

/// Generate a wiki article using centroid-based chunk selection + single-shot LLM.
pub(crate) async fn generate(
    ctx: &WikiStrategyContext,
) -> Result<WikiArticleWithCitations, String> {
    eprintln!("[wiki/centroid] Preparing sources (centroid similarity)...");
    let input = prepare_wiki_generation(&ctx.db, &ctx.provider_config, &ctx.tag_id, &ctx.tag_name).await?;
    eprintln!("[wiki/centroid] Found {} chunks from {} atoms", input.chunks.len(), input.atom_count);

    eprintln!("[wiki/centroid] Calling LLM...");
    let result = generate_wiki_content(
        &ctx.provider_config,
        &input,
        &ctx.wiki_model,
        &ctx.linkable_article_names,
    ).await?;

    Ok(result)
}

/// Update an existing wiki article with new content using centroid strategy.
/// Returns None if no new content is available.
pub(crate) async fn update(
    ctx: &WikiStrategyContext,
    existing: &WikiArticleWithCitations,
) -> Result<Option<WikiArticleWithCitations>, String> {
    let update_input = {
        let conn = ctx.db.conn.lock().map_err(|e| e.to_string())?;
        prepare_wiki_update(
            &conn,
            &ctx.tag_id,
            &ctx.tag_name,
            &existing.article,
            &existing.citations,
        )?
    };

    let input = match update_input {
        Some(input) => input,
        None => return Ok(None),
    };

    let result = update_wiki_content(
        &ctx.provider_config,
        &input,
        &ctx.wiki_model,
        &ctx.linkable_article_names,
    ).await?;

    Ok(Some(result))
}

/// Prepare data for wiki article generation.
///
/// Uses the tag's centroid embedding to rank all chunks under the tag hierarchy
/// by semantic relevance, then selects the top chunks that fit within the token budget.
/// Falls back to a simple SQL fetch (ordered by atom/chunk index) if no centroid exists.
async fn prepare_wiki_generation(
    db: &Database,
    _provider_config: &ProviderConfig,
    tag_id: &str,
    tag_name: &str,
) -> Result<WikiGenerationInput, String> {
    let conn = db.conn.lock().map_err(|e| e.to_string())?;

    // Get all descendant tag IDs (including the tag itself)
    let all_tag_ids = get_tag_hierarchy(&conn, tag_id)?;

    if all_tag_ids.is_empty() {
        return Err("No content found for this tag".to_string());
    }

    // Build the set of atom IDs under this tag hierarchy (for filtering vec_chunks results)
    let placeholders = all_tag_ids.iter().map(|_| "?").collect::<Vec<_>>().join(",");
    let atom_ids_query = format!(
        "SELECT DISTINCT atom_id FROM atom_tags WHERE tag_id IN ({})",
        placeholders
    );
    let mut stmt = conn.prepare(&atom_ids_query)
        .map_err(|e| format!("Failed to prepare atom_ids query: {}", e))?;
    let scoped_atom_ids: std::collections::HashSet<String> = stmt
        .query_map(rusqlite::params_from_iter(all_tag_ids.iter()), |row| row.get(0))
        .map_err(|e| format!("Failed to query atom_ids: {}", e))?
        .collect::<Result<std::collections::HashSet<_>, _>>()
        .map_err(|e| format!("Failed to collect atom_ids: {}", e))?;

    if scoped_atom_ids.is_empty() {
        return Err("No content found for this tag".to_string());
    }

    // Try to load the tag's centroid embedding for ranked retrieval
    let centroid_blob: Option<Vec<u8>> = conn
        .query_row(
            "SELECT embedding FROM tag_embeddings WHERE tag_id = ?1",
            [tag_id],
            |row| row.get(0),
        )
        .ok();

    let chunks = if let Some(ref centroid) = centroid_blob {
        // Ranked path: query vec_chunks with centroid, filter to scoped atoms, fill token budget
        select_chunks_by_centroid(&conn, centroid, &scoped_atom_ids)?
    } else {
        // Fallback: no centroid yet (e.g. embeddings haven't run), fetch by insertion order
        eprintln!("[wiki/centroid] No centroid for tag {}, falling back to unranked chunk selection", tag_id);
        select_chunks_unranked(&conn, &placeholders, &all_tag_ids)?
    };

    if chunks.is_empty() {
        return Err("No content found for this tag".to_string());
    }

    let atom_count = count_atoms_with_tags(&conn, &all_tag_ids)?;

    Ok(WikiGenerationInput {
        chunks,
        atom_count,
        tag_id: tag_id.to_string(),
        tag_name: tag_name.to_string(),
    })
}

/// Select chunks ranked by similarity to the tag centroid, up to the token budget.
fn select_chunks_by_centroid(
    conn: &Connection,
    centroid_blob: &[u8],
    scoped_atom_ids: &std::collections::HashSet<String>,
) -> Result<Vec<ChunkWithContext>, String> {
    // Fetch more than we need from vec_chunks since we'll filter by scope.
    // Over-fetch by 3x to account for chunks outside the tag hierarchy.
    let fetch_limit = 3000_i32;

    let mut vec_stmt = conn.prepare(
        "SELECT chunk_id, distance
         FROM vec_chunks
         WHERE embedding MATCH ?1
         ORDER BY distance
         LIMIT ?2",
    ).map_err(|e| format!("Failed to prepare vec_chunks query: {}", e))?;

    let candidates: Vec<(String, f32)> = vec_stmt
        .query_map(rusqlite::params![centroid_blob, fetch_limit], |row| {
            Ok((row.get(0)?, row.get(1)?))
        })
        .map_err(|e| format!("Failed to query vec_chunks: {}", e))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| format!("Failed to collect vec_chunks: {}", e))?;

    // Batch-load chunk details for all candidates
    let chunk_ids: Vec<&str> = candidates.iter().map(|(id, _)| id.as_str()).collect();
    let chunk_details = batch_fetch_chunk_details(conn, &chunk_ids)?;

    // Filter to scoped atoms and fill token budget
    let mut chunks = Vec::new();
    let mut total_tokens = 0;

    for (chunk_id, distance) in &candidates {
        if let Some((atom_id, chunk_index, content)) = chunk_details.get(chunk_id.as_str()) {
            if !scoped_atom_ids.contains(atom_id) {
                continue;
            }
            let tokens = count_tokens(content);
            if total_tokens + tokens > MAX_WIKI_SOURCE_TOKENS && !chunks.is_empty() {
                break;
            }
            total_tokens += tokens;
            chunks.push(ChunkWithContext {
                atom_id: atom_id.clone(),
                chunk_index: *chunk_index,
                content: content.clone(),
                similarity_score: distance_to_similarity(*distance),
            });
        }
    }

    eprintln!(
        "[wiki/centroid] Selected {} chunks ({} tokens) by centroid similarity",
        chunks.len(), total_tokens
    );

    Ok(chunks)
}

/// Fallback: select chunks by insertion order up to the token budget.
fn select_chunks_unranked(
    conn: &Connection,
    placeholders: &str,
    all_tag_ids: &[String],
) -> Result<Vec<ChunkWithContext>, String> {
    let query = format!(
        "SELECT DISTINCT ac.atom_id, ac.chunk_index, ac.content
         FROM atom_chunks ac
         INNER JOIN atom_tags at ON ac.atom_id = at.atom_id
         WHERE at.tag_id IN ({})
         ORDER BY ac.atom_id, ac.chunk_index",
        placeholders
    );

    let mut stmt = conn.prepare(&query)
        .map_err(|e| format!("Failed to prepare chunks query: {}", e))?;

    let mut rows = stmt.query(rusqlite::params_from_iter(all_tag_ids.iter()))
        .map_err(|e| format!("Failed to query chunks: {}", e))?;

    let mut chunks = Vec::new();
    let mut total_tokens = 0;

    while let Some(row) = rows.next().map_err(|e| format!("Failed to read row: {}", e))? {
        let content: String = row.get(2).map_err(|e| format!("Failed to get content: {}", e))?;
        let tokens = count_tokens(&content);
        if total_tokens + tokens > MAX_WIKI_SOURCE_TOKENS && !chunks.is_empty() {
            break;
        }
        total_tokens += tokens;
        chunks.push(ChunkWithContext {
            atom_id: row.get(0).map_err(|e| format!("Failed to get atom_id: {}", e))?,
            chunk_index: row.get(1).map_err(|e| format!("Failed to get chunk_index: {}", e))?,
            content,
            similarity_score: 1.0,
        });
    }

    eprintln!(
        "[wiki/centroid] Selected {} chunks ({} tokens) by insertion order (no centroid)",
        chunks.len(), total_tokens
    );

    Ok(chunks)
}

/// Generate wiki article content via shared synthesis (async, no db needed)
async fn generate_wiki_content(
    provider_config: &ProviderConfig,
    input: &WikiGenerationInput,
    model: &str,
    existing_article_names: &[(String, String)],
) -> Result<WikiArticleWithCitations, String> {
    synthesize_article(
        provider_config,
        &input.tag_id,
        &input.tag_name,
        &input.chunks,
        input.atom_count,
        model,
        existing_article_names,
    )
    .await
}

/// Prepare data for wiki article update (sync, needs db connection).
///
/// Finds atoms added since the last update, then ranks their chunks by centroid
/// similarity (same as generation) to stay within the token budget.
fn prepare_wiki_update(
    conn: &Connection,
    tag_id: &str,
    _tag_name: &str,
    existing_article: &WikiArticle,
    existing_citations: &[WikiCitation],
) -> Result<Option<WikiUpdateInput>, String> {
    let last_update = &existing_article.updated_at;

    // Get atoms added after the last update
    let mut new_atom_stmt = conn
        .prepare(
            "SELECT DISTINCT a.id FROM atoms a
             INNER JOIN atom_tags at ON a.id = at.atom_id
             WHERE at.tag_id = ?1 AND a.created_at > ?2",
        )
        .map_err(|e| format!("Failed to prepare new atoms query: {}", e))?;

    let new_atom_ids: Vec<String> = new_atom_stmt
        .query_map(rusqlite::params![tag_id, last_update], |row| row.get(0))
        .map_err(|e| format!("Failed to query new atoms: {}", e))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| format!("Failed to collect new atom IDs: {}", e))?;

    if new_atom_ids.is_empty() {
        return Ok(None); // No new atoms
    }

    let new_atom_id_set: std::collections::HashSet<String> = new_atom_ids.into_iter().collect();

    // Try centroid-ranked selection scoped to new atoms only
    let centroid_blob: Option<Vec<u8>> = conn
        .query_row(
            "SELECT embedding FROM tag_embeddings WHERE tag_id = ?1",
            [tag_id],
            |row| row.get(0),
        )
        .ok();

    let new_chunks = if let Some(ref centroid) = centroid_blob {
        select_chunks_by_centroid(conn, centroid, &new_atom_id_set)?
    } else {
        // Fallback: fetch by insertion order with token budget
        eprintln!("[wiki/centroid] No centroid for tag {}, falling back to unranked update chunk selection", tag_id);
        select_new_chunks_unranked(conn, &new_atom_id_set)?
    };

    if new_chunks.is_empty() {
        return Ok(None);
    }

    eprintln!(
        "[wiki/centroid] Update: {} new chunks from {} new atoms",
        new_chunks.len(), new_atom_id_set.len()
    );

    let atom_count: i32 = conn
        .query_row(
            "SELECT COUNT(*) FROM atom_tags WHERE tag_id = ?1",
            [tag_id],
            |row| row.get(0),
        )
        .map_err(|e| format!("Failed to count atoms: {}", e))?;

    Ok(Some(WikiUpdateInput {
        new_chunks,
        existing_article: existing_article.clone(),
        existing_citations: existing_citations.to_vec(),
        atom_count,
        tag_id: tag_id.to_string(),
    }))
}

/// Fallback for update: select new chunks by insertion order up to the token budget.
fn select_new_chunks_unranked(
    conn: &Connection,
    new_atom_ids: &std::collections::HashSet<String>,
) -> Result<Vec<ChunkWithContext>, String> {
    if new_atom_ids.is_empty() {
        return Ok(Vec::new());
    }

    let placeholders = new_atom_ids.iter().map(|_| "?").collect::<Vec<_>>().join(",");
    let query = format!(
        "SELECT atom_id, chunk_index, content FROM atom_chunks WHERE atom_id IN ({}) ORDER BY atom_id, chunk_index",
        placeholders
    );

    let mut stmt = conn.prepare(&query)
        .map_err(|e| format!("Failed to prepare new chunks query: {}", e))?;

    let params: Vec<&str> = new_atom_ids.iter().map(|s| s.as_str()).collect();
    let mut rows = stmt.query(rusqlite::params_from_iter(params.iter()))
        .map_err(|e| format!("Failed to query new chunks: {}", e))?;

    let mut chunks = Vec::new();
    let mut total_tokens = 0;

    while let Some(row) = rows.next().map_err(|e| format!("Failed to read row: {}", e))? {
        let content: String = row.get(2).map_err(|e| format!("Failed to get content: {}", e))?;
        let tokens = count_tokens(&content);
        if total_tokens + tokens > MAX_WIKI_SOURCE_TOKENS && !chunks.is_empty() {
            break;
        }
        total_tokens += tokens;
        chunks.push(ChunkWithContext {
            atom_id: row.get(0).map_err(|e| format!("Failed to get atom_id: {}", e))?,
            chunk_index: row.get(1).map_err(|e| format!("Failed to get chunk_index: {}", e))?,
            content,
            similarity_score: 1.0,
        });
    }

    eprintln!(
        "[wiki/centroid] Selected {} new chunks ({} tokens) by insertion order (no centroid)",
        chunks.len(), total_tokens
    );

    Ok(chunks)
}

/// Update wiki article content via API (async, no db needed)
async fn update_wiki_content(
    provider_config: &ProviderConfig,
    input: &WikiUpdateInput,
    model: &str,
    existing_article_names: &[(String, String)],
) -> Result<WikiArticleWithCitations, String> {
    // Build existing sources section
    let mut existing_sources = String::new();
    for citation in &input.existing_citations {
        existing_sources.push_str(&format!(
            "[{}] {}\n\n",
            citation.citation_index, citation.excerpt
        ));
    }

    // Build new sources section (continuing numbering)
    let start_index = input.existing_citations.len() as i32 + 1;
    let mut new_sources = String::new();
    for (i, chunk) in input.new_chunks.iter().enumerate() {
        new_sources.push_str(&format!(
            "[{}] {}\n\n",
            start_index + i as i32,
            chunk.content
        ));
    }

    // Build existing articles list for cross-linking
    let articles_section = if existing_article_names.is_empty() {
        String::new()
    } else {
        let names: Vec<&str> = existing_article_names
            .iter()
            .filter(|(tid, _)| tid != &input.tag_id)
            .map(|(_, name)| name.as_str())
            .collect();
        if names.is_empty() {
            String::new()
        } else {
            format!(
                "\nEXISTING WIKI ARTICLES IN THIS KNOWLEDGE BASE:\n{}\n",
                names.join(", ")
            )
        }
    };

    let user_content = format!(
        "CURRENT ARTICLE:\n{}\n\nEXISTING SOURCES (already cited as [1] through [{}]):\n{}\nNEW SOURCES TO INCORPORATE (cite as [{}] onwards):\n{}{}\nUpdate the article to incorporate the new information.{}",
        input.existing_article.content,
        input.existing_citations.len(),
        existing_sources,
        start_index,
        new_sources,
        articles_section,
        if articles_section.is_empty() {
            ""
        } else {
            " Use [[Article Name]] to link to other articles listed above where relevant."
        }
    );

    // Call LLM API
    let result =
        call_llm_for_wiki(provider_config, WIKI_UPDATE_SYSTEM_PROMPT, &user_content, model).await?;

    // Create updated article
    let now = Utc::now().to_rfc3339();
    let article = WikiArticle {
        id: input.existing_article.id.clone(),
        tag_id: input.tag_id.clone(),
        content: result.article_content.clone(),
        created_at: input.existing_article.created_at.clone(),
        updated_at: now,
        atom_count: input.atom_count,
    };

    // Extract all citations from the updated content
    // Combine existing chunks with new chunks for citation mapping
    let mut all_chunks: Vec<ChunkWithContext> = input
        .existing_citations
        .iter()
        .map(|c| ChunkWithContext {
            atom_id: c.atom_id.clone(),
            chunk_index: c.chunk_index.unwrap_or(0),
            content: c.excerpt.clone(),
            similarity_score: 1.0,
        })
        .collect();
    all_chunks.extend(input.new_chunks.clone());

    let citations = extract_citations(&article.id, &result.article_content, &all_chunks)?;

    Ok(WikiArticleWithCitations { article, citations })
}
