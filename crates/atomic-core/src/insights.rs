//! Novel discovery features for the Atomic knowledge base.
//!
//! This module surfaces knowledge in ways that complement traditional search and
//! wiki generation. Where search helps you find what you *know* you want, these
//! features help you discover what you *don't know* you need.
//!
//! # Features
//!
//! * **Knowledge Gap Analysis** — identifies isolated atoms (no connections),
//!   sparse tags (underexplored topics), and bridge atoms (integrative hubs that
//!   link otherwise disconnected knowledge clusters).
//!
//! * **Serendipity Walk** — traverses the semantic graph with controlled
//!   randomness, surfacing an unexpected-but-connected path of atoms that
//!   mirrors free-association for creative ideation.
//!
//! * **Time Capsule** — finds old atoms that are semantically similar to recently
//!   added ones, resurfacing forgotten-but-relevant knowledge.

use std::collections::{HashMap, HashSet};

use rusqlite::Connection;

use crate::error::AtomicCoreError;
use crate::models::{
    BridgeAtom, IsolatedAtom, KnowledgeGapsResult, SerendipityStep, SerendipityWalkResult,
    SparseTag, TimeCapsulePair, TimeCapsuleResult,
};

// ---------------------------------------------------------------------------
// Knowledge Gap Analysis
// ---------------------------------------------------------------------------

/// Analyse the knowledge graph for areas that need more attention.
///
/// Returns:
/// - **isolated atoms** — atoms with fewer than `isolation_threshold` semantic
///   edges, meaning they sit disconnected from the rest of the graph.
/// - **sparse tags** — non-root tags with fewer than `sparse_tag_threshold`
///   atoms, indicating underexplored topic areas.
/// - **bridge atoms** — atoms that span multiple clusters, acting as the only
///   link between otherwise disconnected knowledge regions.
pub fn knowledge_gaps(
    conn: &Connection,
    isolation_threshold: i32,
    sparse_tag_threshold: i32,
    max_results: usize,
) -> Result<KnowledgeGapsResult, AtomicCoreError> {
    let isolated = find_isolated_atoms(conn, isolation_threshold, max_results)?;
    let sparse = find_sparse_tags(conn, sparse_tag_threshold, max_results)?;
    let bridges = find_bridge_atoms(conn, max_results)?;

    Ok(KnowledgeGapsResult {
        isolated_atoms: isolated,
        sparse_tags: sparse,
        bridge_atoms: bridges,
    })
}

fn find_isolated_atoms(
    conn: &Connection,
    threshold: i32,
    limit: usize,
) -> Result<Vec<IsolatedAtom>, AtomicCoreError> {
    // Count how many semantic edges each embedded atom has.
    // Atoms with fewer than `threshold` edges are "isolated".
    let mut stmt = conn.prepare(
        "SELECT a.id, a.title, a.snippet, a.created_at,
                CAST(COALESCE(e.edge_count, 0) AS INTEGER) AS cnt
         FROM atoms a
         LEFT JOIN (
             SELECT source_atom_id AS atom_id, COUNT(*) AS edge_count
             FROM semantic_edges
             GROUP BY source_atom_id
             UNION ALL
             SELECT target_atom_id AS atom_id, COUNT(*) AS edge_count
             FROM semantic_edges
             GROUP BY target_atom_id
         ) e ON a.id = e.atom_id
         WHERE a.embedding_status = 'complete'
         GROUP BY a.id
         HAVING CAST(COALESCE(SUM(e.edge_count), 0) AS INTEGER) < ?1
         ORDER BY cnt ASC, a.created_at DESC
         LIMIT ?2",
    )?;

    let rows = stmt
        .query_map([threshold, limit as i32], |row| {
            Ok(IsolatedAtom {
                atom_id: row.get(0)?,
                title: row.get(1)?,
                snippet: row.get(2)?,
                created_at: row.get(3)?,
                connection_count: row.get(4)?,
            })
        })?
        .filter_map(|r| r.ok())
        .collect();

    Ok(rows)
}

fn find_sparse_tags(
    conn: &Connection,
    threshold: i32,
    limit: usize,
) -> Result<Vec<SparseTag>, AtomicCoreError> {
    // Non-root tags with fewer than `threshold` atoms directly assigned.
    let mut stmt = conn.prepare(
        "SELECT t.id, t.name, t.parent_id,
                CAST(COALESCE(t.atom_count, 0) AS INTEGER) AS cnt,
                p.name AS parent_name
         FROM tags t
         LEFT JOIN tags p ON t.parent_id = p.id
         WHERE t.parent_id IS NOT NULL
           AND CAST(COALESCE(t.atom_count, 0) AS INTEGER) > 0
           AND CAST(COALESCE(t.atom_count, 0) AS INTEGER) < ?1
         ORDER BY cnt ASC
         LIMIT ?2",
    )?;

    let rows = stmt
        .query_map([threshold, limit as i32], |row| {
            Ok(SparseTag {
                tag_id: row.get(0)?,
                tag_name: row.get(1)?,
                atom_count: row.get(3)?,
                parent_name: row.get(4)?,
            })
        })?
        .filter_map(|r| r.ok())
        .collect();

    Ok(rows)
}

fn find_bridge_atoms(
    conn: &Connection,
    limit: usize,
) -> Result<Vec<BridgeAtom>, AtomicCoreError> {
    // Load cluster assignments
    let mut cluster_stmt =
        conn.prepare("SELECT atom_id, cluster_id FROM atom_clusters")?;

    let assignments: HashMap<String, i32> = cluster_stmt
        .query_map([], |row| Ok((row.get::<_, String>(0)?, row.get::<_, i32>(1)?)))?
        .filter_map(|r| r.ok())
        .collect();

    if assignments.is_empty() {
        return Ok(vec![]);
    }

    // Load semantic edges
    let mut edge_stmt = conn.prepare(
        "SELECT source_atom_id, target_atom_id FROM semantic_edges WHERE similarity_score >= 0.4",
    )?;

    let edges: Vec<(String, String)> = edge_stmt
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?
        .filter_map(|r| r.ok())
        .collect();

    // For each atom, collect the set of distinct clusters it bridges.
    // An atom "bridges" when its immediate neighbours span multiple clusters.
    let mut span: HashMap<String, HashSet<i32>> = HashMap::new();

    for (src, tgt) in &edges {
        if let (Some(&src_cluster), Some(&tgt_cluster)) =
            (assignments.get(src), assignments.get(tgt))
        {
            if src_cluster != tgt_cluster {
                span.entry(src.clone()).or_default().insert(tgt_cluster);
                span.entry(src.clone()).or_default().insert(src_cluster);
                span.entry(tgt.clone()).or_default().insert(src_cluster);
                span.entry(tgt.clone()).or_default().insert(tgt_cluster);
            }
        }
    }

    // Sort by number of bridged clusters descending
    let mut bridgers: Vec<(String, Vec<i32>)> = span
        .into_iter()
        .filter(|(_, clusters)| clusters.len() >= 2)
        .map(|(id, clusters)| {
            let mut v: Vec<i32> = clusters.into_iter().collect();
            v.sort_unstable();
            (id, v)
        })
        .collect();
    bridgers.sort_by(|a, b| b.1.len().cmp(&a.1.len()));
    bridgers.truncate(limit);

    if bridgers.is_empty() {
        return Ok(vec![]);
    }

    // Fetch metadata for bridge atoms
    let ids: Vec<String> = bridgers.iter().map(|(id, _)| id.clone()).collect();
    let placeholders = ids
        .iter()
        .enumerate()
        .map(|(i, _)| format!("?{}", i + 1))
        .collect::<Vec<_>>()
        .join(", ");

    let sql = format!(
        "SELECT id, title, snippet FROM atoms WHERE id IN ({})",
        placeholders
    );
    let mut meta_stmt = conn.prepare(&sql)?;

    let params: Vec<&dyn rusqlite::ToSql> = ids
        .iter()
        .map(|s| s as &dyn rusqlite::ToSql)
        .collect();

    let meta: HashMap<String, (String, String)> = meta_stmt
        .query_map(params.as_slice(), |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
            ))
        })?
        .filter_map(|r| r.ok())
        .map(|(id, title, snippet)| (id, (title, snippet)))
        .collect();

    let result = bridgers
        .into_iter()
        .filter_map(|(id, clusters)| {
            let (title, snippet) = meta.get(&id)?.clone();
            Some(BridgeAtom {
                atom_id: id,
                title,
                snippet,
                cluster_span: clusters.len() as i32,
                bridged_cluster_ids: clusters,
            })
        })
        .collect();

    Ok(result)
}

// ---------------------------------------------------------------------------
// Serendipity Walk
// ---------------------------------------------------------------------------

/// Walk the semantic graph from `start_atom_id` for `steps` hops.
///
/// At each step the walk follows a semantic edge chosen by a weighted-random
/// selection that favours high-similarity neighbours but occasionally explores
/// lower-similarity ones.  The `randomness` parameter (0.0–1.0) controls the
/// temperature: 0.0 always picks the best neighbour, 1.0 picks uniformly at
/// random.
///
/// Already-visited atoms are never revisited.
pub fn serendipity_walk(
    conn: &Connection,
    start_atom_id: &str,
    steps: usize,
    randomness: f32,
    seed: u64,
) -> Result<SerendipityWalkResult, AtomicCoreError> {
    // Verify start atom exists
    let start_meta: Option<(String, String)> = conn
        .query_row(
            "SELECT title, snippet FROM atoms WHERE id = ?1",
            [start_atom_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .ok();

    let (start_title, start_snippet) = start_meta.ok_or_else(|| {
        AtomicCoreError::NotFound(format!("Atom {} not found", start_atom_id))
    })?;

    let first_step = SerendipityStep {
        atom_id: start_atom_id.to_string(),
        title: start_title,
        snippet: start_snippet,
        connection_reason: "Starting point".to_string(),
        similarity_to_prev: None,
        depth: 0,
    };

    let mut walk = vec![first_step];
    let mut visited: HashSet<String> = HashSet::new();
    visited.insert(start_atom_id.to_string());

    // Deterministic LCG (Linear Congruential Generator) for reproducible walks
    // without the `rand` crate dependency.
    //   multiplier = 6364136223846793005  (Knuth's 64-bit LCG multiplier)
    //   addend     = 1442695040888963407  (standard Knuth LCG addend)
    // We XOR the seed into the initial state so that different seeds give
    // uncorrelated sequences from the very first step.
    let mut rng_state = seed.wrapping_add(6364136223846793005);

    let mut current_id = start_atom_id.to_string();

    for depth in 1..=steps {
        // Fetch all neighbours of the current atom from semantic_edges
        let mut nbr_stmt = conn.prepare(
            "SELECT target_atom_id, similarity_score
             FROM semantic_edges
             WHERE source_atom_id = ?1
             UNION
             SELECT source_atom_id, similarity_score
             FROM semantic_edges
             WHERE target_atom_id = ?1
             ORDER BY similarity_score DESC
             LIMIT 20",
        )?;

        let neighbours: Vec<(String, f32)> = nbr_stmt
            .query_map([&current_id], |row| Ok((row.get(0)?, row.get(1)?)))?
            .filter_map(|r| r.ok())
            .filter(|(id, _)| !visited.contains(id))
            .collect();

        if neighbours.is_empty() {
            break;
        }

        // Weighted selection with temperature.
        // weight_i = sim_i^(1 / temperature) where temperature = randomness.clamp(0.01, 1.0)
        let temperature = randomness.clamp(0.01, 1.0) as f64;
        let weights: Vec<f64> = neighbours
            .iter()
            .map(|(_, sim)| (*sim as f64).max(0.01).powf(1.0 / temperature))
            .collect();

        let total: f64 = weights.iter().sum();

        // LCG step
        rng_state = rng_state
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        let rand_val = (rng_state >> 33) as f64 / u32::MAX as f64;

        let mut cumulative = 0.0f64;
        let threshold = rand_val * total;
        let mut chosen_idx = 0usize;
        for (i, w) in weights.iter().enumerate() {
            cumulative += w;
            if cumulative >= threshold {
                chosen_idx = i;
                break;
            }
        }

        let (next_id, similarity) = &neighbours[chosen_idx];

        // Fetch metadata for the chosen atom
        let meta: Option<(String, String)> = conn
            .query_row(
                "SELECT title, snippet FROM atoms WHERE id = ?1",
                [next_id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .ok();

        let (title, snippet) = match meta {
            Some(m) => m,
            None => continue,
        };

        let connection_reason = build_connection_reason(*similarity, depth);

        walk.push(SerendipityStep {
            atom_id: next_id.clone(),
            title,
            snippet,
            connection_reason,
            similarity_to_prev: Some(*similarity),
            depth: depth as i32,
        });

        visited.insert(next_id.clone());
        current_id = next_id.clone();
    }

    Ok(SerendipityWalkResult {
        start_atom_id: start_atom_id.to_string(),
        steps: walk,
    })
}

fn build_connection_reason(similarity: f32, depth: usize) -> String {
    let strength = if similarity >= 0.8 {
        "strongly"
    } else if similarity >= 0.6 {
        "moderately"
    } else if similarity >= 0.4 {
        "loosely"
    } else {
        "tangentially"
    };

    let hop = if depth == 1 {
        "Direct connection"
    } else if depth == 2 {
        "Second-degree connection"
    } else {
        "Indirect connection"
    };

    format!("{} — {} related (similarity: {:.2})", hop, strength, similarity)
}

// ---------------------------------------------------------------------------
// Time Capsule
// ---------------------------------------------------------------------------

/// Surface old atoms that are semantically similar to recently added ones.
///
/// `lookback_days` controls how far back "old" atoms are drawn from (atoms
/// created more than `lookback_days` days ago).  "New" atoms are those created
/// within the last `recent_days` days.  Returns up to `limit` pairs ordered by
/// similarity score descending.
pub fn time_capsule(
    conn: &Connection,
    lookback_days: i32,
    recent_days: i32,
    similarity_threshold: f32,
    limit: usize,
) -> Result<TimeCapsuleResult, AtomicCoreError> {
    // Find semantic edges that bridge an "old" atom and a "new" atom.
    // We define:
    //   old  = created_at < (now - lookback_days)
    //   new  = created_at > (now - recent_days)
    let mut stmt = conn.prepare(
        "SELECT
             se.source_atom_id,
             a_src.title,
             a_src.snippet,
             a_src.created_at,
             se.target_atom_id,
             a_tgt.title,
             a_tgt.snippet,
             a_tgt.created_at,
             se.similarity_score
         FROM semantic_edges se
         JOIN atoms a_src ON se.source_atom_id = a_src.id
         JOIN atoms a_tgt ON se.target_atom_id = a_tgt.id
         WHERE se.similarity_score >= ?1
           AND (
               (datetime(a_src.created_at) < datetime('now', '-' || ?2 || ' days')
                AND datetime(a_tgt.created_at) > datetime('now', '-' || ?3 || ' days'))
               OR
               (datetime(a_tgt.created_at) < datetime('now', '-' || ?2 || ' days')
                AND datetime(a_src.created_at) > datetime('now', '-' || ?3 || ' days'))
           )
         ORDER BY se.similarity_score DESC
         LIMIT ?4",
    )?;

    let pairs: Vec<TimeCapsulePair> = stmt
        .query_map(
            rusqlite::params![similarity_threshold, lookback_days, recent_days, limit as i32],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,  // source id
                    row.get::<_, String>(1)?,  // source title
                    row.get::<_, String>(2)?,  // source snippet
                    row.get::<_, String>(3)?,  // source created_at
                    row.get::<_, String>(4)?,  // target id
                    row.get::<_, String>(5)?,  // target title
                    row.get::<_, String>(6)?,  // target snippet
                    row.get::<_, String>(7)?,  // target created_at
                    row.get::<_, f32>(8)?,     // similarity_score
                ))
            },
        )?
        .filter_map(|r| r.ok())
        .map(|(sid, st, ss, sca, tid, tt, ts, tca, score)| {
            // The WHERE clause guarantees one atom is "old" and one is "new",
            // but either the source or the target could be the older atom.
            // Compare timestamps lexicographically (ISO-8601 sorts correctly)
            // to assign the old/new labels correctly regardless of edge direction.
            if sca <= tca {
                TimeCapsulePair {
                    old_atom_id: sid,
                    old_atom_title: st,
                    old_atom_snippet: ss,
                    old_atom_created_at: sca,
                    new_atom_id: tid,
                    new_atom_title: tt,
                    new_atom_snippet: ts,
                    new_atom_created_at: tca,
                    similarity_score: score,
                }
            } else {
                TimeCapsulePair {
                    old_atom_id: tid,
                    old_atom_title: tt,
                    old_atom_snippet: ts,
                    old_atom_created_at: tca,
                    new_atom_id: sid,
                    new_atom_title: st,
                    new_atom_snippet: ss,
                    new_atom_created_at: sca,
                    similarity_score: score,
                }
            }
        })
        .collect();

    Ok(TimeCapsuleResult {
        pairs,
        lookback_days,
    })
}

// ---------------------------------------------------------------------------
// Unit tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn make_conn() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        unsafe {
            rusqlite::ffi::sqlite3_auto_extension(Some(std::mem::transmute(
                sqlite_vec::sqlite3_vec_init as *const (),
            )));
        }
        conn.execute_batch(
            "CREATE TABLE atoms (
                id TEXT PRIMARY KEY,
                content TEXT NOT NULL DEFAULT '',
                title TEXT NOT NULL DEFAULT '',
                snippet TEXT NOT NULL DEFAULT '',
                source_url TEXT,
                source TEXT,
                published_at TEXT,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL,
                embedding_status TEXT NOT NULL DEFAULT 'pending',
                tagging_status TEXT NOT NULL DEFAULT 'pending'
            );
            CREATE TABLE tags (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL,
                parent_id TEXT,
                created_at TEXT NOT NULL,
                atom_count INTEGER NOT NULL DEFAULT 0
            );
            CREATE TABLE semantic_edges (
                id TEXT PRIMARY KEY,
                source_atom_id TEXT NOT NULL,
                target_atom_id TEXT NOT NULL,
                similarity_score REAL NOT NULL,
                source_chunk_index INTEGER,
                target_chunk_index INTEGER,
                created_at TEXT NOT NULL
            );
            CREATE TABLE atom_clusters (
                atom_id TEXT NOT NULL,
                cluster_id INTEGER NOT NULL
            );",
        )
        .unwrap();
        conn
    }

    #[test]
    fn knowledge_gaps_empty_db() {
        let conn = make_conn();
        let result = knowledge_gaps(&conn, 1, 3, 20).unwrap();
        assert!(result.isolated_atoms.is_empty());
        assert!(result.sparse_tags.is_empty());
        assert!(result.bridge_atoms.is_empty());
    }

    #[test]
    fn isolated_atoms_detected() {
        let conn = make_conn();

        conn.execute_batch(
            "INSERT INTO atoms VALUES ('a1','','Title A1','Snippet A1',NULL,NULL,NULL,
             '2024-01-01T00:00:00Z','2024-01-01T00:00:00Z','complete','complete');
             INSERT INTO atoms VALUES ('a2','','Title A2','Snippet A2',NULL,NULL,NULL,
             '2024-01-02T00:00:00Z','2024-01-02T00:00:00Z','complete','complete');
             INSERT INTO atoms VALUES ('a3','','Title A3','Snippet A3',NULL,NULL,NULL,
             '2024-01-03T00:00:00Z','2024-01-03T00:00:00Z','complete','complete');
             -- a1 <-> a2 are connected; a3 is isolated
             INSERT INTO semantic_edges VALUES ('e1','a1','a2',0.8,NULL,NULL,'2024-01-01T00:00:00Z');",
        )
        .unwrap();

        let result = knowledge_gaps(&conn, 1, 3, 20).unwrap();
        let isolated_ids: Vec<&str> =
            result.isolated_atoms.iter().map(|a| a.atom_id.as_str()).collect();
        assert!(isolated_ids.contains(&"a3"), "a3 should be isolated");
        assert!(
            !isolated_ids.contains(&"a1"),
            "a1 has an edge and should not be isolated"
        );
    }

    #[test]
    fn sparse_tags_detected() {
        let conn = make_conn();

        conn.execute_batch(
            "INSERT INTO tags VALUES ('root','Topics',NULL,'2024-01-01T00:00:00Z',0);
             INSERT INTO tags VALUES ('t1','Machine Learning','root','2024-01-01T00:00:00Z',1);
             INSERT INTO tags VALUES ('t2','Deep Learning','root','2024-01-01T00:00:00Z',10);",
        )
        .unwrap();

        let result = knowledge_gaps(&conn, 1, 3, 20).unwrap();
        let sparse_ids: Vec<&str> =
            result.sparse_tags.iter().map(|t| t.tag_id.as_str()).collect();
        assert!(sparse_ids.contains(&"t1"), "t1 has 1 atom and should be sparse");
        assert!(!sparse_ids.contains(&"t2"), "t2 has 10 atoms and should not be sparse");
    }

    #[test]
    fn serendipity_walk_missing_start_returns_error() {
        let conn = make_conn();
        let result = serendipity_walk(&conn, "nonexistent", 5, 0.3, 42);
        assert!(result.is_err());
    }

    #[test]
    fn serendipity_walk_no_edges_returns_single_step() {
        let conn = make_conn();
        conn.execute(
            "INSERT INTO atoms VALUES ('a1','','Title A1','Snippet A1',NULL,NULL,NULL,
             '2024-01-01T00:00:00Z','2024-01-01T00:00:00Z','complete','complete')",
            [],
        )
        .unwrap();

        let result = serendipity_walk(&conn, "a1", 5, 0.3, 42).unwrap();
        assert_eq!(result.start_atom_id, "a1");
        assert_eq!(result.steps.len(), 1);
        assert_eq!(result.steps[0].depth, 0);
    }

    #[test]
    fn serendipity_walk_follows_edges() {
        let conn = make_conn();
        conn.execute_batch(
            "INSERT INTO atoms VALUES ('a1','','A1','s1',NULL,NULL,NULL,
             '2024-01-01T00:00:00Z','2024-01-01T00:00:00Z','complete','complete');
             INSERT INTO atoms VALUES ('a2','','A2','s2',NULL,NULL,NULL,
             '2024-01-02T00:00:00Z','2024-01-02T00:00:00Z','complete','complete');
             INSERT INTO atoms VALUES ('a3','','A3','s3',NULL,NULL,NULL,
             '2024-01-03T00:00:00Z','2024-01-03T00:00:00Z','complete','complete');
             INSERT INTO semantic_edges VALUES ('e1','a1','a2',0.9,NULL,NULL,'2024-01-01T00:00:00Z');
             INSERT INTO semantic_edges VALUES ('e2','a2','a3',0.8,NULL,NULL,'2024-01-01T00:00:00Z');",
        )
        .unwrap();

        let result = serendipity_walk(&conn, "a1", 3, 0.0, 42).unwrap();
        assert!(result.steps.len() >= 2, "Should have visited at least 2 atoms");
        assert_eq!(result.steps[0].atom_id, "a1");
    }

    #[test]
    fn time_capsule_empty_db() {
        let conn = make_conn();
        let result = time_capsule(&conn, 30, 7, 0.5, 10).unwrap();
        assert!(result.pairs.is_empty());
        assert_eq!(result.lookback_days, 30);
    }

    #[test]
    fn time_capsule_old_new_labels_correct_regardless_of_edge_direction() {
        // This test verifies that old/new labels are assigned by timestamp
        // comparison, not by which atom is the edge source vs target.
        let conn = make_conn();

        // old_atom was created 60 days ago (simulate with a far-past date)
        // new_atom was created yesterday (simulate with a recent date)
        conn.execute_batch(
            "INSERT INTO atoms VALUES ('old','','Old Atom','old snippet',NULL,NULL,NULL,
             '2000-01-01T00:00:00Z','2000-01-01T00:00:00Z','complete','complete');
             INSERT INTO atoms VALUES ('new','','New Atom','new snippet',NULL,NULL,NULL,
             '2099-12-31T00:00:00Z','2099-12-31T00:00:00Z','complete','complete');
             -- Edge where new atom is source, old atom is target (reversed direction)
             INSERT INTO semantic_edges VALUES ('e1','new','old',0.9,NULL,NULL,'2000-01-01T00:00:00Z');",
        )
        .unwrap();

        // Use very large lookback/recent so our synthetic dates qualify
        let result = time_capsule(&conn, 365 * 20, 365 * 20, 0.5, 10).unwrap();
        assert_eq!(result.pairs.len(), 1, "Should find the pair");

        let pair = &result.pairs[0];
        assert_eq!(pair.old_atom_id, "old", "old_atom_id should be the older atom regardless of edge direction");
        assert_eq!(pair.new_atom_id, "new", "new_atom_id should be the newer atom");
        assert!(pair.old_atom_created_at < pair.new_atom_created_at);
    }
}
