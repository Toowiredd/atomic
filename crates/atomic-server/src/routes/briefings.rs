//! Daily briefing routes.

use crate::db_extractor::Db;
use crate::error::{blocking_ok, error_response, ApiErrorResponse};
use crate::state::{AppState, ServerEvent};
use actix_web::{web, HttpRequest, HttpResponse};
use serde::Deserialize;
use utoipa::{IntoParams, ToSchema};

#[utoipa::path(
    get,
    path = "/api/briefings/latest",
    responses(
        (status = 200, description = "Most recent briefing with citations", body = atomic_core::BriefingWithCitations),
        (status = 404, description = "No briefings yet", body = ApiErrorResponse)
    ),
    tag = "briefings"
)]
pub async fn get_latest_briefing(db: Db) -> HttpResponse {
    let core = db.0;
    match web::block(move || core.get_latest_briefing()).await {
        Ok(Ok(Some(b))) => HttpResponse::Ok().json(b),
        Ok(Ok(None)) => HttpResponse::NotFound()
            .json(serde_json::json!({"error": "No briefings yet"})),
        Ok(Err(e)) => error_response(e),
        Err(e) => HttpResponse::InternalServerError()
            .json(serde_json::json!({"error": format!("Thread pool error: {}", e)})),
    }
}

#[derive(Debug, Clone, Deserialize, IntoParams, ToSchema)]
#[into_params(parameter_in = Query)]
pub struct ListBriefingsQuery {
    /// Max briefings to return (default 20)
    pub limit: Option<i32>,
}

#[utoipa::path(
    get,
    path = "/api/briefings",
    params(ListBriefingsQuery),
    responses(
        (status = 200, description = "Recent briefings (without citations)", body = Vec<atomic_core::Briefing>)
    ),
    tag = "briefings"
)]
pub async fn list_briefings(db: Db, query: web::Query<ListBriefingsQuery>) -> HttpResponse {
    let limit = query.limit.unwrap_or(20).clamp(1, 100);
    let core = db.0;
    blocking_ok(move || core.list_briefings(limit)).await
}

#[utoipa::path(
    get,
    path = "/api/briefings/{id}",
    responses(
        (status = 200, description = "Briefing with citations", body = atomic_core::BriefingWithCitations),
        (status = 404, description = "Briefing not found", body = ApiErrorResponse)
    ),
    tag = "briefings"
)]
pub async fn get_briefing(db: Db, path: web::Path<String>) -> HttpResponse {
    let core = db.0;
    let id = path.into_inner();
    match web::block(move || core.get_briefing(&id)).await {
        Ok(Ok(Some(b))) => HttpResponse::Ok().json(b),
        Ok(Ok(None)) => HttpResponse::NotFound()
            .json(serde_json::json!({"error": "Briefing not found"})),
        Ok(Err(e)) => error_response(e),
        Err(e) => HttpResponse::InternalServerError()
            .json(serde_json::json!({"error": format!("Thread pool error: {}", e)})),
    }
}

#[utoipa::path(
    post,
    path = "/api/briefings/run",
    responses(
        (status = 200, description = "Briefing generated", body = atomic_core::BriefingWithCitations),
        (status = 400, description = "Error", body = ApiErrorResponse)
    ),
    tag = "briefings"
)]
pub async fn run_briefing_now(
    state: web::Data<AppState>,
    req: HttpRequest,
) -> HttpResponse {
    // Resolve the target core before we start (same logic as Db extractor).
    let core = match state.resolve_core(&req) {
        Ok(c) => c,
        Err(e) => return error_response(e),
    };

    // Capture db_id for the broadcast event. We use the file stem of the
    // SQLite path; for Postgres this call returns a Configuration error
    // from run_daily_briefing anyway.
    let db_id = core
        .db_path()
        .file_stem()
        .and_then(|s| s.to_str())
        .map(|s| s.to_string())
        .unwrap_or_else(|| "default".to_string());

    match core.run_daily_briefing().await {
        Ok(result) => {
            let _ = state.event_tx.send(ServerEvent::BriefingReady {
                db_id,
                briefing_id: result.briefing.id.clone(),
            });
            HttpResponse::Ok().json(result)
        }
        Err(e) => error_response(e),
    }
}
