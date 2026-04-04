//! Sync source management routes.
//!
//! Provides CRUD for sync sources (ChatGPT, Claude, Markdown directories,
//! remote Atomic instances, and log files) and a manual "run now" trigger.

use crate::error::{blocking_ok, ApiErrorResponse};
use crate::state::AppState;
use actix_web::{web, HttpResponse};
use atomic_core::SyncSourceInfo;
use serde::{Deserialize, Serialize};
use std::time::Duration;
use utoipa::ToSchema;

/// Cooldown between manual "run now" triggers for the same source.
const MANUAL_TRIGGER_COOLDOWN: Duration = Duration::from_secs(30);

// ==================== Request / Response types ====================

#[derive(Debug, Deserialize, Serialize, ToSchema)]
pub struct CreateSyncSourceBody {
    /// Human-readable name for this source.
    pub name: String,
    /// One of: "chatgpt", "claude", "markdown_dir", "remote_atomic", "log_file"
    pub source_type: String,
    /// URL of the remote Atomic instance (for remote_atomic sources).
    pub source_url: Option<String>,
    /// File system path (for file-based sources).
    pub source_path: Option<String>,
    /// Bearer token for authenticating with a remote Atomic instance.
    /// Write-only — never returned in API responses.
    pub source_token: Option<String>,
    /// Target database ID; null means the active database.
    pub target_db_id: Option<String>,
    /// Polling interval in seconds.  0 = manual only.  Negative values are rejected.
    pub interval_secs: Option<i64>,
}

#[derive(Debug, Deserialize, Serialize, ToSchema)]
pub struct UpdateSyncSourceBody {
    pub name: Option<String>,
    /// Pass `null` to clear the URL.
    pub source_url: Option<serde_json::Value>,
    /// Pass `null` to clear the path.
    pub source_path: Option<serde_json::Value>,
    /// Pass `null` to clear the token, a string to update it.
    pub source_token: Option<serde_json::Value>,
    pub target_db_id: Option<serde_json::Value>,
    /// Polling interval in seconds.  0 = manual only.  Negative values are rejected.
    pub interval_secs: Option<i64>,
    pub enabled: Option<bool>,
}

// ==================== Helpers ====================

/// Returns a 400 if `interval_secs` is negative.
fn validate_interval(secs: Option<i64>) -> Option<HttpResponse> {
    if let Some(s) = secs {
        if s < 0 {
            return Some(HttpResponse::BadRequest().json(ApiErrorResponse {
                error: format!("interval_secs must be >= 0, got {s}"),
            }));
        }
    }
    None
}

// ==================== Handlers ====================

#[utoipa::path(get, path = "/api/sync/sources",
    responses((status = 200, description = "List of sync sources", body = Vec<SyncSourceInfo>)),
    tag = "sync")]
pub async fn list_sync_sources(state: web::Data<AppState>) -> HttpResponse {
    let registry = state.manager.registry().clone();
    blocking_ok(move || registry.list_sync_sources()).await
}

#[utoipa::path(post, path = "/api/sync/sources",
    request_body = CreateSyncSourceBody,
    responses(
        (status = 200, description = "Sync source created", body = SyncSourceInfo),
        (status = 400, description = "Validation error", body = ApiErrorResponse),
    ),
    tag = "sync")]
pub async fn create_sync_source(
    state: web::Data<AppState>,
    body: web::Json<CreateSyncSourceBody>,
) -> HttpResponse {
    let valid_types = ["chatgpt", "claude", "markdown_dir", "remote_atomic", "log_file"];
    if !valid_types.contains(&body.source_type.as_str()) {
        return HttpResponse::BadRequest().json(ApiErrorResponse {
            error: format!(
                "Invalid source_type '{}'. Must be one of: {}",
                body.source_type,
                valid_types.join(", ")
            ),
        });
    }

    // Reject negative intervals
    if let Some(resp) = validate_interval(body.interval_secs) {
        return resp;
    }

    let registry = state.manager.registry().clone();
    let body = body.into_inner();
    blocking_ok(move || {
        registry.create_sync_source(
            &body.name,
            &body.source_type,
            body.source_url.as_deref(),
            body.source_path.as_deref(),
            body.source_token.as_deref(),
            body.target_db_id.as_deref(),
            body.interval_secs.unwrap_or(0),
        )
    })
    .await
}

#[utoipa::path(put, path = "/api/sync/sources/{id}",
    params(("id" = String, Path, description = "Sync source ID")),
    request_body = UpdateSyncSourceBody,
    responses(
        (status = 200, description = "Sync source updated", body = SyncSourceInfo),
        (status = 404, description = "Not found", body = ApiErrorResponse),
    ),
    tag = "sync")]
pub async fn update_sync_source(
    state: web::Data<AppState>,
    path: web::Path<String>,
    body: web::Json<UpdateSyncSourceBody>,
) -> HttpResponse {
    // Reject negative intervals before hitting the DB
    if let Some(resp) = validate_interval(body.interval_secs) {
        return resp;
    }

    let id = path.into_inner();
    let registry = state.manager.registry().clone();
    let body = body.into_inner();

    blocking_ok(move || {
        // Parse nullable JSON fields into Option<Option<String>>
        fn parse_nullable(v: serde_json::Value) -> Option<String> {
            if v.is_null() { None } else { v.as_str().map(String::from) }
        }

        let source_url: Option<Option<String>> = body.source_url.map(parse_nullable);
        let source_path: Option<Option<String>> = body.source_path.map(parse_nullable);
        let source_token: Option<Option<String>> = body.source_token.map(parse_nullable);
        let target_db_id: Option<Option<String>> = body.target_db_id.map(parse_nullable);

        registry.update_sync_source(
            &id,
            body.name.as_deref(),
            source_url.as_ref().map(|v| v.as_deref()),
            source_path.as_ref().map(|v| v.as_deref()),
            source_token.as_ref().map(|v| v.as_deref()),
            target_db_id.as_ref().map(|v| v.as_deref()),
            body.interval_secs,
            body.enabled,
        )
    })
    .await
}

#[utoipa::path(delete, path = "/api/sync/sources/{id}",
    params(("id" = String, Path, description = "Sync source ID")),
    responses(
        (status = 200, description = "Deleted"),
        (status = 404, description = "Not found", body = ApiErrorResponse),
    ),
    tag = "sync")]
pub async fn delete_sync_source(
    state: web::Data<AppState>,
    path: web::Path<String>,
) -> HttpResponse {
    let id = path.into_inner();
    let registry = state.manager.registry().clone();
    blocking_ok(move || registry.delete_sync_source(&id)).await
}

#[utoipa::path(post, path = "/api/sync/sources/{id}/run",
    params(("id" = String, Path, description = "Sync source ID")),
    responses(
        (status = 202, description = "Sync triggered"),
        (status = 404, description = "Not found", body = ApiErrorResponse),
        (status = 409, description = "Sync already running", body = ApiErrorResponse),
    ),
    tag = "sync")]
pub async fn run_sync_source(
    state: web::Data<AppState>,
    path: web::Path<String>,
) -> HttpResponse {
    let id = path.into_inner();
    let registry = state.manager.registry().clone();

    // Verify the source exists before touching the lock
    let source = match registry.get_sync_source_internal(&id) {
        Ok(s) => s,
        Err(e) => return crate::error::error_response(e),
    };

    // Enforce per-source cooldown between manual triggers.
    {
        let cooldowns = state.sync_cooldowns.lock().await;
        if let Some(last_triggered) = cooldowns.get(&id) {
            let elapsed = last_triggered.elapsed();
            if elapsed < MANUAL_TRIGGER_COOLDOWN {
                let remaining = (MANUAL_TRIGGER_COOLDOWN - elapsed).as_secs() + 1;
                return HttpResponse::TooManyRequests().json(ApiErrorResponse {
                    error: format!(
                        "Sync source '{id}' was triggered {}s ago; retry in {remaining}s",
                        elapsed.as_secs()
                    ),
                });
            }
        }
    }

    // Atomically check-and-insert under the lock to avoid TOCTOU races with
    // the background scheduler.  If the ID is already present the source is
    // running; return 409.  If not, insert it here — execute_sync_source will
    // only release the slot, not acquire it.
    {
        let mut running = state.sync_running.lock().await;
        if running.contains(&id) {
            return HttpResponse::Conflict().json(ApiErrorResponse {
                error: format!("Sync source '{id}' is already running"),
            });
        }
        running.insert(id.clone());
    }

    // Record the trigger time now that we know the run will proceed.
    {
        let mut cooldowns = state.sync_cooldowns.lock().await;
        cooldowns.insert(id.clone(), std::time::Instant::now());
    }

    let manager = state.manager.clone();
    let tx = state.event_tx.clone();
    let sync_running = state.sync_running.clone();

    tokio::spawn(async move {
        crate::sync::execute_sync_source(&source, &manager, tx, sync_running).await;
    });

    HttpResponse::Accepted().json(serde_json::json!({
        "message": "Sync started",
        "source_id": id
    }))
}

#[utoipa::path(get, path = "/api/sync/status",
    responses((status = 200, description = "Sync status")),
    tag = "sync")]
pub async fn sync_status(state: web::Data<AppState>) -> HttpResponse {
    let registry = state.manager.registry().clone();
    blocking_ok(move || {
        registry.list_sync_sources().map(|sources| {
            serde_json::json!({
                "sources": sources,
                "total": sources.len(),
                "enabled": sources.iter().filter(|s| s.enabled).count(),
            })
        })
    })
    .await
}

/// Response body for the test-connection endpoint.
#[derive(Serialize, ToSchema)]
pub struct TestConnectionResponse {
    /// Whether the connection test passed.
    pub ok: bool,
    /// Human-readable message describing the outcome.
    pub message: String,
}

#[utoipa::path(post, path = "/api/sync/sources/{id}/test-connection",
    params(("id" = String, Path, description = "Sync source ID")),
    responses(
        (status = 200, description = "Test result", body = TestConnectionResponse),
        (status = 400, description = "Missing configuration", body = ApiErrorResponse),
        (status = 404, description = "Not found", body = ApiErrorResponse),
    ),
    tag = "sync")]
pub async fn test_sync_connection(
    state: web::Data<AppState>,
    path: web::Path<String>,
) -> HttpResponse {
    let id = path.into_inner();
    let registry = state.manager.registry().clone();

    let source = match registry.get_sync_source_internal(&id) {
        Ok(s) => s,
        Err(e) => return crate::error::error_response(e),
    };

    match source.source_type.as_str() {
        "remote_atomic" => {
            let url = match source.source_url.as_deref() {
                Some(u) => u.trim_end_matches('/').to_string(),
                None => {
                    return HttpResponse::BadRequest().json(ApiErrorResponse {
                        error: "source_url is required for remote_atomic".to_string(),
                    });
                }
            };
            let token = source.source_token.clone();

            let client = match reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(10))
                .build()
            {
                Ok(c) => c,
                Err(e) => {
                    return HttpResponse::InternalServerError().json(ApiErrorResponse {
                        error: e.to_string(),
                    });
                }
            };

            let mut req = client.get(format!("{url}/health"));
            if let Some(t) = token.as_deref() {
                req = req.bearer_auth(t);
            }

            match req.send().await {
                Ok(resp) if resp.status().is_success() => {
                    HttpResponse::Ok().json(TestConnectionResponse {
                        ok: true,
                        message: format!(
                            "Connected to {} (HTTP {})",
                            url,
                            resp.status()
                        ),
                    })
                }
                Ok(resp) => HttpResponse::Ok().json(TestConnectionResponse {
                    ok: false,
                    message: format!("Remote returned HTTP {}", resp.status()),
                }),
                Err(e) => HttpResponse::Ok().json(TestConnectionResponse {
                    ok: false,
                    message: format!("Connection failed: {e}"),
                }),
            }
        }

        st @ ("chatgpt" | "claude" | "log_file" | "markdown_dir") => {
            let p = match source.source_path.as_deref() {
                Some(p) => p,
                None => {
                    return HttpResponse::BadRequest().json(ApiErrorResponse {
                        error: format!("source_path is required for {st}"),
                    });
                }
            };

            match std::path::Path::new(p).canonicalize() {
                Ok(canonical) => {
                    let expects_dir = st == "markdown_dir";
                    if expects_dir && !canonical.is_dir() {
                        return HttpResponse::Ok().json(TestConnectionResponse {
                            ok: false,
                            message: format!("'{}' exists but is not a directory", canonical.display()),
                        });
                    }
                    if !expects_dir && !canonical.is_file() {
                        return HttpResponse::Ok().json(TestConnectionResponse {
                            ok: false,
                            message: format!("'{}' exists but is not a regular file", canonical.display()),
                        });
                    }
                    HttpResponse::Ok().json(TestConnectionResponse {
                        ok: true,
                        message: format!("'{}' is accessible", canonical.display()),
                    })
                }
                Err(e) => HttpResponse::Ok().json(TestConnectionResponse {
                    ok: false,
                    message: format!("Path not accessible: {e}"),
                }),
            }
        }

        unknown => HttpResponse::BadRequest().json(ApiErrorResponse {
            error: format!("Unknown source_type '{unknown}'"),
        }),
    }
}
