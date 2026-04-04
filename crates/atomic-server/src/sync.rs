//! Background sync scheduler and per-source sync execution.
//!
//! The scheduler runs every 60 seconds, checks all enabled sync sources,
//! and executes any that are due.  The same `execute_sync_source` function
//! is used by the manual "run now" route trigger.

use crate::event_bridge::embedding_event_callback;
use crate::state::ServerEvent;
use atomic_core::registry::SyncSource;
use atomic_core::DatabaseManager;
use std::sync::Arc;
use tokio::sync::broadcast;

// ==================== Scheduler ====================

/// Spawn the background sync scheduler task.
///
/// Ticks every 60 seconds and runs any enabled sources whose interval has
/// elapsed since `last_synced_at`.
pub fn spawn_sync_scheduler(
    manager: Arc<DatabaseManager>,
    tx: broadcast::Sender<ServerEvent>,
) {
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(60));
        interval.tick().await; // skip the immediate first tick
        loop {
            interval.tick().await;
            tick_sync_sources(&manager, tx.clone()).await;
        }
    });
}

async fn tick_sync_sources(
    manager: &Arc<DatabaseManager>,
    tx: broadcast::Sender<ServerEvent>,
) {
    let sources = match manager.registry().list_sync_sources_internal() {
        Ok(s) => s,
        Err(e) => {
            tracing::warn!(error = %e, "sync scheduler: failed to list sources");
            return;
        }
    };

    for source in sources {
        if source.interval_secs <= 0 {
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

        if due {
            let mgr = Arc::clone(manager);
            let tx2 = tx.clone();
            tokio::spawn(async move {
                execute_sync_source(&source, &mgr, tx2).await;
            });
        }
    }
}

// ==================== Per-source execution ====================

/// Run a single sync source and emit progress events.
///
/// This is the shared implementation used by both the scheduler and the
/// manual trigger route.
pub async fn execute_sync_source(
    source: &SyncSource,
    manager: &Arc<DatabaseManager>,
    tx: broadcast::Sender<ServerEvent>,
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
            let path = source.source_path.as_deref().ok_or("source_path is required for chatgpt")?;
            let content = tokio::fs::read_to_string(path)
                .await
                .map_err(|e| format!("cannot read {path}: {e}"))?;
            let conversations =
                atomic_core::import::conversations::parse_chatgpt_export(&content)
                    .map_err(|e| format!("parse error: {e}"))?;
            let (c, m) = core
                .import_conversations(&conversations, on_progress)
                .map_err(|e| e.to_string())?;
            Ok((c, m, 0))
        }

        "claude" => {
            let path = source.source_path.as_deref().ok_or("source_path is required for claude")?;
            let content = tokio::fs::read_to_string(path)
                .await
                .map_err(|e| format!("cannot read {path}: {e}"))?;
            let conversations =
                atomic_core::import::conversations::parse_claude_export(&content)
                    .map_err(|e| format!("parse error: {e}"))?;
            let (c, m) = core
                .import_conversations(&conversations, on_progress)
                .map_err(|e| e.to_string())?;
            Ok((c, m, 0))
        }

        "markdown_dir" => {
            let dir = source.source_path.as_deref().ok_or("source_path is required for markdown_dir")?;
            let conversations = collect_markdown_conversations(dir).await?;
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
            let path = source.source_path.as_deref().ok_or("source_path is required for log_file")?;
            let content = tokio::fs::read_to_string(path)
                .await
                .map_err(|e| format!("cannot read {path}: {e}"))?;
            let source_name = source
                .source_url
                .as_deref()
                .unwrap_or_else(|| std::path::Path::new(path).file_name().and_then(|n| n.to_str()).unwrap_or(path));
            let request = atomic_core::import::IngestLogRequest {
                content,
                format: atomic_core::import::LogFormat::Auto,
                source_name: source_name.to_string(),
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
    dir: &str,
) -> Result<Vec<atomic_core::ImportedConversation>, String> {
    let dir = dir.to_string();
    tokio::task::spawn_blocking(move || {
        let mut conversations = Vec::new();
        let entries = std::fs::read_dir(&dir)
            .map_err(|e| format!("cannot read directory {dir}: {e}"))?;

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

/// Fetch conversations from a remote Atomic server via its REST API.
async fn fetch_remote_conversations(
    base_url: &str,
    token: Option<&str>,
) -> Result<Vec<atomic_core::ImportedConversation>, String> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .map_err(|e| e.to_string())?;

    let base_url = base_url.trim_end_matches('/');

    // 1. Fetch conversation list
    let mut list_req = client.get(format!("{base_url}/api/conversations?limit=200"));
    if let Some(t) = token {
        list_req = list_req.bearer_auth(t);
    }
    let list_resp = list_req.send().await.map_err(|e| e.to_string())?;
    if !list_resp.status().is_success() {
        return Err(format!(
            "remote returned {} for /api/conversations",
            list_resp.status()
        ));
    }
    let list_json: serde_json::Value = list_resp.json().await.map_err(|e| e.to_string())?;

    let conv_ids: Vec<String> = list_json
        .as_array()
        .map(|arr| {
            arr.iter()
                .filter_map(|c| c["id"].as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default();

    // 2. Fetch each conversation's full message history
    let mut results = Vec::new();
    for id in conv_ids {
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
