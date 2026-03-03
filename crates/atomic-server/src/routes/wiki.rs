//! Wiki article routes

use crate::error::{blocking_ok, ok_or_error};
use crate::state::AppState;
use actix_web::{web, HttpResponse};
use serde::Deserialize;

pub async fn get_all_wiki_articles(state: web::Data<AppState>) -> HttpResponse {
    let db = state.core.database();
    let conn = match db.conn.lock() {
        Ok(c) => c,
        Err(e) => {
            return HttpResponse::InternalServerError()
                .json(serde_json::json!({"error": e.to_string()}));
        }
    };
    match atomic_core::wiki::load_all_wiki_articles(&conn) {
        Ok(articles) => HttpResponse::Ok().json(articles),
        Err(e) => HttpResponse::InternalServerError()
            .json(serde_json::json!({"error": e})),
    }
}

pub async fn get_wiki(state: web::Data<AppState>, path: web::Path<String>) -> HttpResponse {
    let tag_id = path.into_inner();
    let core = state.core.clone();
    blocking_ok(move || core.get_wiki(&tag_id)).await
}

pub async fn get_wiki_status(state: web::Data<AppState>, path: web::Path<String>) -> HttpResponse {
    let tag_id = path.into_inner();
    let core = state.core.clone();
    blocking_ok(move || core.get_wiki_status(&tag_id)).await
}

#[derive(Deserialize)]
pub struct GenerateWikiBody {
    pub tag_name: String,
}

pub async fn generate_wiki(
    state: web::Data<AppState>,
    path: web::Path<String>,
    body: web::Json<GenerateWikiBody>,
) -> HttpResponse {
    let tag_id = path.into_inner();
    match state.core.generate_wiki(&tag_id, &body.tag_name).await {
        Ok(article) => HttpResponse::Ok().json(article),
        Err(e) => crate::error::error_response(e),
    }
}

pub async fn update_wiki(
    state: web::Data<AppState>,
    path: web::Path<String>,
    body: web::Json<GenerateWikiBody>,
) -> HttpResponse {
    let tag_id = path.into_inner();
    match state.core.update_wiki(&tag_id, &body.tag_name).await {
        Ok(article) => HttpResponse::Ok().json(article),
        Err(e) => crate::error::error_response(e),
    }
}

pub async fn delete_wiki(state: web::Data<AppState>, path: web::Path<String>) -> HttpResponse {
    let tag_id = path.into_inner();
    let core = state.core.clone();
    blocking_ok(move || core.delete_wiki(&tag_id)).await
}

#[derive(Deserialize)]
pub struct RelatedTagsQuery {
    pub limit: Option<usize>,
}

pub async fn get_related_tags(
    state: web::Data<AppState>,
    path: web::Path<String>,
    query: web::Query<RelatedTagsQuery>,
) -> HttpResponse {
    let tag_id = path.into_inner();
    let limit = query.limit.unwrap_or(10);
    let core = state.core.clone();
    blocking_ok(move || core.get_related_tags(&tag_id, limit)).await
}

pub async fn get_wiki_links(state: web::Data<AppState>, path: web::Path<String>) -> HttpResponse {
    let tag_id = path.into_inner();
    let core = state.core.clone();
    blocking_ok(move || core.get_wiki_links(&tag_id)).await
}

#[derive(Deserialize)]
pub struct SuggestionsQuery {
    pub limit: Option<i32>,
}

pub async fn get_wiki_suggestions(
    state: web::Data<AppState>,
    query: web::Query<SuggestionsQuery>,
) -> HttpResponse {
    let limit = query.limit.unwrap_or(10);
    let core = state.core.clone();
    blocking_ok(move || core.get_suggested_wiki_articles(limit)).await
}

pub async fn list_wiki_versions(state: web::Data<AppState>, path: web::Path<String>) -> HttpResponse {
    let tag_id = path.into_inner();
    let core = state.core.clone();
    blocking_ok(move || core.list_wiki_versions(&tag_id)).await
}

pub async fn get_wiki_version(state: web::Data<AppState>, path: web::Path<String>) -> HttpResponse {
    let version_id = path.into_inner();
    let core = state.core.clone();
    blocking_ok(move || core.get_wiki_version(&version_id)).await
}

pub async fn recompute_all_tag_embeddings(state: web::Data<AppState>) -> HttpResponse {
    match state.core.recompute_all_tag_embeddings() {
        Ok(count) => HttpResponse::Ok().json(serde_json::json!({"count": count})),
        Err(e) => crate::error::error_response(e),
    }
}
