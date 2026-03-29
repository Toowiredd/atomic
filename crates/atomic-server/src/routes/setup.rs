//! Instance setup endpoint — allows claiming an unconfigured instance

use crate::state::AppState;
use actix_web::{web, HttpResponse};
use serde::Deserialize;

/// GET /api/setup/status — Check if the instance needs initial setup
pub async fn setup_status(state: web::Data<AppState>) -> HttpResponse {
    if !state.manager.is_initialized() {
        return HttpResponse::Ok().json(serde_json::json!({
            "needs_setup": true,
            "needs_passphrase": true,
        }));
    }

    let core = match state.manager.active_core() {
        Ok(c) => c,
        Err(e) => return crate::error::error_response(e),
    };
    match web::block(move || core.list_api_tokens()).await {
        Ok(Ok(tokens)) => {
            let active = tokens.iter().filter(|t| !t.is_revoked).count();
            HttpResponse::Ok().json(serde_json::json!({
                "needs_setup": active == 0,
                "needs_passphrase": false,
            }))
        }
        Ok(Err(e)) => crate::error::error_response(e),
        Err(e) => HttpResponse::InternalServerError().json(serde_json::json!({"error": e.to_string()})),
    }
}

#[derive(Deserialize)]
pub struct ClaimBody {
    pub name: Option<String>,
    /// Optional passphrase for SQLCipher database encryption.
    /// If set, all databases will be encrypted at rest.
    /// Cannot be changed after initial setup.
    pub passphrase: Option<String>,
}

/// POST /api/setup/claim — Create the first API token (only works when no tokens exist).
/// If the manager is not yet initialized (deferred mode), this also creates the databases
/// with the optional passphrase for encryption.
pub async fn claim_instance(
    state: web::Data<AppState>,
    body: web::Json<ClaimBody>,
) -> HttpResponse {
    let body = body.into_inner();
    let name = body.name.unwrap_or_else(|| "default".to_string());

    // Initialize databases if in deferred mode
    if !state.manager.is_initialized() {
        if let Err(e) = state.manager.initialize(body.passphrase) {
            return crate::error::error_response(e);
        }
    }

    let core = match state.manager.active_core() {
        Ok(c) => c,
        Err(e) => return crate::error::error_response(e),
    };

    match web::block(move || {
        // Check that no active tokens exist
        let tokens = core.list_api_tokens()?;
        let active = tokens.iter().filter(|t| !t.is_revoked).count();
        if active > 0 {
            return Ok(None);
        }
        let (info, raw) = core.create_api_token(&name)?;
        Ok(Some((info, raw)))
    })
    .await
    {
        Ok(Ok(Some((info, raw_token)))) => HttpResponse::Created().json(serde_json::json!({
            "id": info.id,
            "name": info.name,
            "token": raw_token,
            "prefix": info.token_prefix,
            "created_at": info.created_at,
        })),
        Ok(Ok(None)) => HttpResponse::Conflict().json(serde_json::json!({
            "error": "Instance already claimed"
        })),
        Ok(Err(e)) => crate::error::error_response(e),
        Err(e) => HttpResponse::InternalServerError().json(serde_json::json!({"error": e.to_string()})),
    }
}
