//! Background sync scheduler and per-source sync execution.
//!
//! The scheduler runs every 60 seconds, checks all enabled sync sources,
//! and executes any that are due.  The same `execute_sync_source` function
//! is used by the manual "run now" route trigger.
//!
//! Concurrency is controlled by `sync_running` (stored in `AppState`).
//! **Both the scheduler and the route handler atomically check-and-insert
//! the source ID before spawning** — `execute_sync_source` only needs to
//! release the slot when it finishes.  This eliminates the TOCTOU race that
//! would exist if the check and insert happened inside the spawned task.

use crate::event_bridge::embedding_event_callback;
use crate::state::ServerEvent;
use atomic_core::registry::SyncSource;
use atomic_core::DatabaseManager;
use std::collections::HashSet;
use std::path::Path;
use std::sync::Arc;
use tokio::sync::{broadcast, Mutex};

// ==================== Scheduler ====================

/// Spawn the background sync scheduler task.
///
/// Ticks every 60 seconds and runs any enabled sources whose interval has
/// elapsed since `last_synced_at`.
pub fn spawn_sync_scheduler(
    manager: Arc<DatabaseManager>,
    tx: broadcast::Sender<ServerEvent>,
    sync_running: Arc<Mutex<HashSet<String>>>,
) {
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(60));
        interval.tick().await; // skip the immediate first tick
        loop {
            interval.tick().await;
            tick_sync_sources(&manager, tx.clone(), Arc::clone(&sync_running)).await;
        }
    });
}

async fn tick_sync_sources(
    manager: &Arc<DatabaseManager>,
    tx: broadcast::Sender<ServerEvent>,
    sync_running: Arc<Mutex<HashSet<String>>>,
) {
    let sources = match manager.registry().list_sync_sources_internal() {
        Ok(s) => s,
        Err(e) => {
            tracing::warn!(error = %e, "sync scheduler: failed to list sources");
            return;
        }
    };

    for source in sources {
        if source.interval_secs == 0 {
            continue; // manual-only
        }

        let due = source
            .last_synced_at
            .as_deref()
            .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
            .map(|dt| {
                chrono::Utc::now()
                    .signed_duration_since(dt)
                    .num_seconds()
                    >= source.interval_secs
            })
            .unwrap_or(true); // never run → due immediately

        if !due {
            continue;
        }

        // Atomically check-and-reserve the slot under the lock before spawning.
        // This prevents both the scheduler and a concurrent manual trigger from
        // running the same source simultaneously.
        {
            let mut running = sync_running.lock().await;
            if running.contains(&source.id) {
                tracing::debug!(source_id = %source.id, "sync already running, skipping scheduler tick");
                continue;
            }
            running.insert(source.id.clone());
        }

        let mgr = Arc::clone(manager);
        let tx2 = tx.clone();
        let running_ref = Arc::clone(&sync_running);
        tokio::spawn(async move {
            // Slot was pre-acquired above; execute_sync_source just releases it.
            execute_sync_source(&source, &mgr, tx2, running_ref).await;
        });
    }
}

// ==================== Per-source execution ====================

/// Run a single sync source and emit progress events.
///
/// **Callers must atomically check-and-insert `source.id` into `sync_running`
/// before calling this function.**  This function only removes the ID from the
/// set when it finishes (success or failure).
pub async fn execute_sync_source(
    source: &SyncSource,
    manager: &Arc<DatabaseManager>,
    tx: broadcast::Sender<ServerEvent>,
    sync_running: Arc<Mutex<HashSet<String>>>,
) {
    let _ = tx.send(ServerEvent::SyncStarted {
        source_id: source.id.clone(),
        source_name: source.name.clone(),
    });

    tracing::info!(
        source_id = %source.id,
        source_type = %source.source_type,
        name = %source.name,
        "starting sync"
    );

    let result = run_source(source, manager, tx.clone()).await;

    // Always release the concurrency slot.
    // (tokio::spawn tasks don't propagate panics so this always runs.)
    {
        let mut running = sync_running.lock().await;
        running.remove(&source.id);
    }

    let status = match &result {
        Ok(_) => "ok",
        Err(_) => "error",
    };

    // Record result in registry (best-effort)
    if let Err(e) = manager.registry().record_sync_result(&source.id, status) {
        tracing::warn!(error = %e, "failed to record sync result");
    }

    match result {
        Ok((convs, msgs, atoms)) => {
            tracing::info!(
                source_id = %source.id,
                conversations = convs,
                messages = msgs,
                atoms = atoms,
                "sync complete"
            );
            let _ = tx.send(ServerEvent::SyncComplete {
                source_id: source.id.clone(),
                source_name: source.name.clone(),
                conversations_imported: convs,
                messages_imported: msgs,
                atoms_imported: atoms,
            });
        }
        Err(e) => {
            tracing::error!(source_id = %source.id, error = %e, "sync failed");
            let _ = tx.send(ServerEvent::SyncFailed {
                source_id: source.id.clone(),
                source_name: source.name.clone(),
                error: e.clone(),
            });
        }
    }
}

// ==================== Path safety ====================

/// Canonicalize `raw_path` and verify it exists.
///
/// Returns an error string if the path cannot be canonicalized (e.g.
/// doesn't exist) to prevent path-traversal attacks via symlinks or `../`
/// sequences in user-supplied `source_path` values.
///
/// **Note:** canonicalization prevents traversal to non-existent paths and
/// resolves symlinks, but does not restrict access to a specific base
/// directory.  `source_path` values are set by authenticated administrators
/// only; the server process's filesystem permissions are the final authority
/// on what files may be read.
fn safe_canonicalize(raw_path: &str) -> Result<std::path::PathBuf, String> {
    Path::new(raw_path)
        .canonicalize()
        .map_err(|e| format!("invalid path '{}': {}", raw_path, e))
}

// ==================== Per-source execution (internal) ====================

/// Execute the source-specific logic.
/// Returns (conversations_imported, messages_imported, atoms_imported).
async fn run_source(
    source: &SyncSource,
    manager: &Arc<DatabaseManager>,
    tx: broadcast::Sender<ServerEvent>,
) -> Result<(i32, i32, i32), String> {
    // Resolve the target database core
    let core = if let Some(db_id) = source.target_db_id.as_deref() {
        manager.get_core(db_id).map_err(|e| e.to_string())?
    } else {
        manager.active_core().map_err(|e| e.to_string())?
    };

    let source_id = source.id.clone();
    let tx_progress = tx.clone();
    let on_progress = move |progress: atomic_core::ImportProgress| {
        let _ = tx_progress.send(ServerEvent::SyncProgress {
            source_id: source_id.clone(),
            current: progress.current,
            total: progress.total,
            message: progress.current_file,
        });
    };
    let on_event = embedding_event_callback(tx);

    match source.source_type.as_str() {
        "chatgpt" => {
            let raw = source.source_path.as_deref().ok_or("source_path is required for chatgpt")?;
            let path = safe_canonicalize(raw)?;
            let content = tokio::fs::read_to_string(&path)
                .await
                .map_err(|e| format!("cannot read {}: {}", path.display(), e))?;
            let conversations =
                atomic_core::import::conversations::parse_chatgpt_export(&content)
                    .map_err(|e| format!("parse error: {e}"))?;
            let (c, m) = core
                .import_conversations(&conversations, on_progress)
                .map_err(|e| e.to_string())?;
            Ok((c, m, 0))
        }

        "claude" => {
            let raw = source.source_path.as_deref().ok_or("source_path is required for claude")?;
            let path = safe_canonicalize(raw)?;
            let content = tokio::fs::read_to_string(&path)
                .await
                .map_err(|e| format!("cannot read {}: {}", path.display(), e))?;
            let conversations =
                atomic_core::import::conversations::parse_claude_export(&content)
                    .map_err(|e| format!("parse error: {e}"))?;
            let (c, m) = core
                .import_conversations(&conversations, on_progress)
                .map_err(|e| e.to_string())?;
            Ok((c, m, 0))
        }

        "markdown_dir" => {
            let raw = source.source_path.as_deref().ok_or("source_path is required for markdown_dir")?;
            let dir = safe_canonicalize(raw)?;
            let conversations = collect_markdown_conversations(&dir).await?;
            let (c, m) = core
                .import_conversations(&conversations, on_progress)
                .map_err(|e| e.to_string())?;
            Ok((c, m, 0))
        }

        "remote_atomic" => {
            let url = source.source_url.as_deref().ok_or("source_url is required for remote_atomic")?;
            let token = source.source_token.clone();
            let conversations = fetch_remote_conversations(url, token.as_deref()).await?;
            let (c, m) = core
                .import_conversations(&conversations, on_progress)
                .map_err(|e| e.to_string())?;
            Ok((c, m, 0))
        }

        "log_file" => {
            let raw = source.source_path.as_deref().ok_or("source_path is required for log_file")?;
            let path = safe_canonicalize(raw)?;
            let content = tokio::fs::read_to_string(&path)
                .await
                .map_err(|e| format!("cannot read {}: {}", path.display(), e))?;
            let source_name = path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or(raw)
                .to_string();
            let request = atomic_core::import::IngestLogRequest {
                content,
                format: atomic_core::import::LogFormat::Auto,
                source_name,
                tag_root: None,
                tag_category: None,
            };
            core.ingest_log_file(request, on_event)
                .map(|_| (0, 0, 1))
                .map_err(|e| e.to_string())
        }

        unknown => Err(format!("Unknown source_type '{unknown}'")),
    }
}

// ==================== Helpers ====================

/// Walk a directory and parse every *.md / *.markdown file as a conversation.
async fn collect_markdown_conversations(
    dir: &Path,
) -> Result<Vec<atomic_core::ImportedConversation>, String> {
    let dir = dir.to_path_buf();
    tokio::task::spawn_blocking(move || {
        let mut conversations = Vec::new();
        let entries = std::fs::read_dir(&dir)
            .map_err(|e| format!("cannot read directory {}: {}", dir.display(), e))?;

        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()).map(|e| e == "md" || e == "markdown").unwrap_or(false) {
                if let Ok(content) = std::fs::read_to_string(&path) {
                    let filename = path.file_name().and_then(|n| n.to_str()).unwrap_or("").to_string();
                    let conv =
                        atomic_core::import::conversations::parse_markdown_conversation(&content, &filename);
                    if !conv.messages.is_empty() {
                        conversations.push(conv);
                    }
                }
            }
        }
        Ok(conversations)
    })
    .await
    .map_err(|e| format!("thread error: {e}"))?
}

/// Fetch conversations from a remote Atomic server via its REST API.
/// Public so it can be called from the import route handler.
pub async fn fetch_remote_conversations_public(
    base_url: &str,
    token: Option<&str>,
) -> Result<Vec<atomic_core::ImportedConversation>, String> {
    fetch_remote_conversations(base_url, token).await
}

/// Fetch ALL conversations from a remote Atomic server, using offset-based
/// pagination to handle servers with more than one page of results.
async fn fetch_remote_conversations(
    base_url: &str,
    token: Option<&str>,
) -> Result<Vec<atomic_core::ImportedConversation>, String> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .map_err(|e| e.to_string())?;

    let base_url = base_url.trim_end_matches('/');
    const PAGE_SIZE: usize = 100;

    // 1. Paginate the conversation list until we get a short page
    let mut all_conv_ids: Vec<String> = Vec::new();
    let mut offset = 0usize;
    loop {
        let url = format!(
            "{base_url}/api/conversations?limit={PAGE_SIZE}&offset={offset}"
        );
        let mut req = client.get(&url);
        if let Some(t) = token {
            req = req.bearer_auth(t);
        }
        let resp = req.send().await.map_err(|e| e.to_string())?;
        if !resp.status().is_success() {
            return Err(format!(
                "remote returned {} for /api/conversations",
                resp.status()
            ));
        }
        let page: serde_json::Value = resp.json().await.map_err(|e| e.to_string())?;
        let ids: Vec<String> = page
            .as_array()
            .map(|arr| {
                arr.iter()
                    .filter_map(|c| c["id"].as_str().map(String::from))
                    .collect()
            })
            .unwrap_or_default();

        let page_len = ids.len();
        all_conv_ids.extend(ids);
        if page_len < PAGE_SIZE {
            break; // last page
        }
        offset += PAGE_SIZE;
    }

    // 2. Fetch each conversation's full message history
    let mut results = Vec::new();
    for id in all_conv_ids {
        let mut detail_req = client.get(format!("{base_url}/api/conversations/{id}"));
        if let Some(t) = token {
            detail_req = detail_req.bearer_auth(t);
        }
        let detail: serde_json::Value = match detail_req.send().await {
            Ok(r) if r.status().is_success() => match r.json().await {
                Ok(j) => j,
                Err(_) => continue,
            },
            _ => continue,
        };

        let title = detail["title"].as_str().map(String::from);
        let created_at = detail["created_at"].as_str().map(String::from);
        let messages: Vec<atomic_core::ImportedMessage> = detail["messages"]
            .as_array()
            .map(|arr| {
                arr.iter()
                    .filter_map(|m| {
                        let role = m["role"].as_str()?;
                        let content = m["content"].as_str()?;
                        if content.trim().is_empty() {
                            return None;
                        }
                        Some(atomic_core::ImportedMessage {
                            role: role.to_string(),
                            content: content.to_string(),
                            created_at: m["created_at"].as_str().map(String::from),
                        })
                    })
                    .collect()
            })
            .unwrap_or_default();

        if !messages.is_empty() {
            results.push(atomic_core::ImportedConversation {
                title,
                created_at,
                messages,
            });
        }
    }

    Ok(results)
}
