//! Import routes

use crate::db_extractor::Db;
use crate::event_bridge::embedding_event_callback;
use crate::state::{AppState, ServerEvent};
use actix_web::{web, HttpResponse};
use atomic_core::import::LogFormat;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

// ==================== Obsidian ====================

#[derive(Deserialize, Serialize, ToSchema)]
pub struct ImportObsidianRequest {
    /// Path to Obsidian vault directory
    pub vault_path: String,
    /// Max notes to import (all if not set)
    pub max_notes: Option<i32>,
}

#[utoipa::path(post, path = "/api/import/obsidian", request_body = ImportObsidianRequest, responses((status = 200, description = "Import result")), tag = "import")]
pub async fn import_obsidian_vault(
    state: web::Data<AppState>,
    db: Db,
    body: web::Json<ImportObsidianRequest>,
) -> HttpResponse {
    let on_event = embedding_event_callback(state.event_tx.clone());
    let tx = state.event_tx.clone();
    let on_progress = move |progress: atomic_core::ImportProgress| {
        let _ = tx.send(ServerEvent::ImportProgress {
            current: progress.current,
            total: progress.total,
            current_file: progress.current_file,
            status: progress.status,
        });
    };

    match db.0.import_obsidian_vault(
        &body.vault_path,
        body.max_notes,
        on_event,
        on_progress,
    ) {
        Ok(result) => HttpResponse::Ok().json(result),
        Err(e) => crate::error::error_response(e),
    }
}

// ==================== Conversation import ====================

#[derive(Deserialize, Serialize, ToSchema)]
pub struct ImportConversationsRequest {
    /// Source format: "chatgpt", "claude", or "markdown"
    pub source_type: String,
    /// Path to the export file or directory (for markdown: a directory path)
    pub path: String,
}

#[derive(Serialize, ToSchema)]
pub struct ImportConversationsResult {
    pub conversations_imported: i32,
    pub messages_imported: i32,
}

#[utoipa::path(post, path = "/api/import/conversations",
    request_body = ImportConversationsRequest,
    responses(
        (status = 200, description = "Import result", body = ImportConversationsResult),
        (status = 400, description = "Validation error"),
    ),
    tag = "import")]
pub async fn import_conversations(
    state: web::Data<AppState>,
    db: Db,
    body: web::Json<ImportConversationsRequest>,
) -> HttpResponse {
    let path = body.path.clone();
    let source_type = body.source_type.clone();
    let tx = state.event_tx.clone();

    let result = web::block(move || -> Result<(i32, i32), String> {
        let on_progress = {
            let tx2 = tx.clone();
            move |progress: atomic_core::ImportProgress| {
                let _ = tx2.send(ServerEvent::ImportProgress {
                    current: progress.current,
                    total: progress.total,
                    current_file: progress.current_file,
                    status: progress.status,
                });
            }
        };

        let conversations: Vec<atomic_core::ImportedConversation> = match source_type.as_str() {
            "chatgpt" => {
                let canonical = std::path::Path::new(&path)
                    .canonicalize()
                    .map_err(|e| format!("Invalid path '{}': {}", path, e))?;
                let content = std::fs::read_to_string(&canonical)
                    .map_err(|e| format!("Cannot read file: {e}"))?;
                atomic_core::import::conversations::parse_chatgpt_export(&content)
                    .map_err(|e| format!("Parse error: {e}"))?
            }
            "claude" => {
                let canonical = std::path::Path::new(&path)
                    .canonicalize()
                    .map_err(|e| format!("Invalid path '{}': {}", path, e))?;
                let content = std::fs::read_to_string(&canonical)
                    .map_err(|e| format!("Cannot read file: {e}"))?;
                atomic_core::import::conversations::parse_claude_export(&content)
                    .map_err(|e| format!("Parse error: {e}"))?
            }
            "markdown" => {
                let canonical = std::path::Path::new(&path)
                    .canonicalize()
                    .map_err(|e| format!("Invalid path '{}': {}", path, e))?;
                let dir = std::fs::read_dir(&canonical)
                    .map_err(|e| format!("Cannot read directory: {e}"))?;
                let mut convs = Vec::new();
                for entry in dir.flatten() {
                    let ep = entry.path();
                    let ext = ep.extension().and_then(|e| e.to_str()).unwrap_or("");
                    if ext == "md" || ext == "markdown" {
                        if let Ok(content) = std::fs::read_to_string(&ep) {
                            let filename = ep
                                .file_name()
                                .and_then(|n| n.to_str())
                                .unwrap_or("")
                                .to_string();
                            let c = atomic_core::import::conversations::parse_markdown_conversation(
                                &content, &filename,
                            );
                            if !c.messages.is_empty() {
                                convs.push(c);
                            }
                        }
                    }
                }
                convs
            }
            other => {
                return Err(format!(
                    "Unknown source_type '{other}'. Must be chatgpt, claude, or markdown"
                ))
            }
        };

        db.0.import_conversations(&conversations, on_progress)
            .map_err(|e| e.to_string())
    })
    .await;

    match result {
        Ok(Ok((c, m))) => HttpResponse::Ok().json(ImportConversationsResult {
            conversations_imported: c,
            messages_imported: m,
        }),
        Ok(Err(msg)) => {
            HttpResponse::BadRequest().json(serde_json::json!({"error": msg}))
        }
        Err(e) => HttpResponse::InternalServerError()
            .json(serde_json::json!({"error": e.to_string()})),
    }
}

// ==================== Log file import ====================

#[derive(Deserialize, Serialize, ToSchema)]
pub struct ImportLogsRequest {
    /// Path to a log file on the server's filesystem.
    pub path: Option<String>,
    /// Raw log content (alternative to path — used for direct upload).
    pub content: Option<String>,
    /// Format hint: "auto", "json_lines", "syslog", "plain_text"
    pub format: Option<String>,
    /// Human-readable source label (hostname, service, etc.)
    pub source_name: String,
    /// Override the tag root (default: "Logs")
    pub tag_root: Option<String>,
    /// Sub-category tag (e.g. "System" or "Application")
    pub tag_category: Option<String>,
}

#[utoipa::path(post, path = "/api/import/logs",
    request_body = ImportLogsRequest,
    responses(
        (status = 200, description = "Atom created"),
        (status = 400, description = "Validation error"),
    ),
    tag = "import")]
pub async fn import_logs(
    state: web::Data<AppState>,
    db: Db,
    body: web::Json<ImportLogsRequest>,
) -> HttpResponse {
    let body = body.into_inner();

    let content = match (body.content, body.path.as_deref()) {
        (Some(c), _) => c,
        (None, Some(p)) => {
            let canonical = match std::path::Path::new(p).canonicalize() {
                Ok(c) => c,
                Err(e) => return HttpResponse::BadRequest()
                    .json(serde_json::json!({"error": format!("Invalid path '{}': {}", p, e)})),
            };
            match std::fs::read_to_string(&canonical) {
                Ok(c) => c,
                Err(e) => {
                    return HttpResponse::BadRequest()
                        .json(serde_json::json!({"error": format!("Cannot read file: {e}")}))
                }
            }
        }
        (None, None) => {
            return HttpResponse::BadRequest()
                .json(serde_json::json!({"error": "Either 'path' or 'content' is required"}))
        }
    };

    let format = match body.format.as_deref().unwrap_or("auto") {
        "json_lines" => LogFormat::JsonLines,
        "syslog" => LogFormat::Syslog,
        "plain_text" => LogFormat::PlainText,
        _ => LogFormat::Auto,
    };

    let on_event = embedding_event_callback(state.event_tx.clone());
    let request = atomic_core::import::IngestLogRequest {
        content,
        format,
        source_name: body.source_name,
        tag_root: body.tag_root,
        tag_category: body.tag_category,
    };

    match db.0.ingest_log_file(request, on_event) {
        Ok(atom_id) => HttpResponse::Ok().json(serde_json::json!({"atom_id": atom_id})),
        Err(e) => crate::error::error_response(e),
    }
}

// ==================== Remote pull ====================

#[derive(Deserialize, Serialize, ToSchema)]
pub struct ImportRemoteRequest {
    /// Base URL of the remote Atomic server (e.g. "https://my.server.com")
    pub url: String,
    /// API token for the remote instance
    pub token: Option<String>,
    /// What to pull: "conversations" (only supported value currently)
    pub import_type: Option<String>,
    /// Max conversations to import (all if not set)
    pub max_items: Option<usize>,
}

#[utoipa::path(post, path = "/api/import/remote",
    request_body = ImportRemoteRequest,
    responses(
        (status = 200, description = "Import result"),
        (status = 400, description = "Validation error"),
    ),
    tag = "import")]
pub async fn import_remote(
    state: web::Data<AppState>,
    db: Db,
    body: web::Json<ImportRemoteRequest>,
) -> HttpResponse {
    let body = body.into_inner();
    let import_type = body.import_type.as_deref().unwrap_or("conversations");
    let tx = state.event_tx.clone();

    if import_type != "conversations" {
        return HttpResponse::BadRequest().json(serde_json::json!({
            "error": "import_type must be 'conversations'"
        }));
    }

    let url = body.url.trim_end_matches('/').to_string();
    let token = body.token.clone();
    let max_items = body.max_items;

    let result = async move {
        let conversations =
            crate::sync::fetch_remote_conversations_public(&url, token.as_deref())
                .await
                .map_err(|e| e.to_string())?;

        let conversations = if let Some(max) = max_items {
            conversations.into_iter().take(max).collect()
        } else {
            conversations
        };

        let on_progress = move |p: atomic_core::ImportProgress| {
            let _ = tx.send(ServerEvent::ImportProgress {
                current: p.current,
                total: p.total,
                current_file: p.current_file,
                status: p.status,
            });
        };

        db.0.import_conversations(&conversations, on_progress)
            .map_err(|e| e.to_string())
    }
    .await;

    match result {
        Ok((c, m)) => HttpResponse::Ok().json(serde_json::json!({
            "conversations_imported": c,
            "messages_imported": m,
        })),
        Err(e) => {
            HttpResponse::InternalServerError().json(serde_json::json!({"error": e}))
        }
    }
}

// ==================== Persist local logs as atom ====================

#[utoipa::path(post, path = "/api/import/persist-logs",
    responses((status = 200, description = "Logs persisted as atom")),
    tag = "import")]
pub async fn persist_logs(state: web::Data<AppState>, db: Db) -> HttpResponse {
    let log_text = state.log_buffer.dump();
    if log_text.trim().is_empty() {
        return HttpResponse::Ok()
            .json(serde_json::json!({"message": "No log lines to persist"}));
    }

    let on_event = embedding_event_callback(state.event_tx.clone());

    match db.0.persist_logs_as_atom(log_text, "atomic-server", on_event) {
        Ok(atom_id) => HttpResponse::Ok().json(serde_json::json!({"atom_id": atom_id})),
        Err(e) => crate::error::error_response(e),
    }
}
