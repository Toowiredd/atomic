//! Insights routes — novel discovery features for the Atomic knowledge base.
//!
//! Exposes three endpoints that complement traditional search and wiki generation
//! by helping users discover what they *don't know* they need:
//!
//! - `GET /insights/gaps` — knowledge gap analysis
//! - `GET /insights/serendipity` — serendipity walk through the semantic graph
//! - `GET /insights/time-capsule` — resurface forgotten-but-relevant old atoms

use crate::db_extractor::Db;
use crate::error::blocking_ok;
use actix_web::{web, HttpResponse};
use serde::Deserialize;
use utoipa::IntoParams;

// ---------------------------------------------------------------------------
// Knowledge Gaps
// ---------------------------------------------------------------------------

#[derive(Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct KnowledgeGapsQuery {
    /// Atoms with fewer connections than this are considered isolated (default: 2)
    pub isolation_threshold: Option<i32>,
    /// Tags with fewer atoms than this are considered sparse (default: 3)
    pub sparse_tag_threshold: Option<i32>,
    /// Maximum number of results per category (default: 20)
    pub max_results: Option<usize>,
}

/// Analyse the knowledge graph for gaps and underexplored areas.
///
/// Returns three categories:
/// - **isolated_atoms**: atoms with very few semantic connections
/// - **sparse_tags**: tags with very few atoms (unexplored topics)
/// - **bridge_atoms**: atoms that connect otherwise-disconnected knowledge clusters
#[utoipa::path(
    get,
    path = "/api/insights/gaps",
    params(KnowledgeGapsQuery),
    responses(
        (status = 200, description = "Knowledge gap analysis", body = atomic_core::models::KnowledgeGapsResult),
    ),
    tag = "insights"
)]
pub async fn knowledge_gaps(db: Db, query: web::Query<KnowledgeGapsQuery>) -> HttpResponse {
    let isolation_threshold = query.isolation_threshold.unwrap_or(2);
    let sparse_tag_threshold = query.sparse_tag_threshold.unwrap_or(3);
    let max_results = query.max_results.unwrap_or(20);
    let core = db.0;
    blocking_ok(move || core.knowledge_gaps(isolation_threshold, sparse_tag_threshold, max_results))
        .await
}

// ---------------------------------------------------------------------------
// Serendipity Walk
// ---------------------------------------------------------------------------

#[derive(Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct SerendipityQuery {
    /// ID of the atom to start from (required)
    pub start_atom_id: String,
    /// Number of hops to take (default: 5, max: 10)
    pub steps: Option<usize>,
    /// Exploration randomness 0.0–1.0 (default: 0.4)
    ///
    /// 0.0 always picks the highest-similarity neighbour; 1.0 picks uniformly at random.
    pub randomness: Option<f32>,
    /// Random seed for reproducibility (default: 0 = auto)
    pub seed: Option<u64>,
}

/// Walk the semantic graph from a starting atom with controlled randomness.
///
/// Returns a path of atoms that follows semantic connections but with enough
/// randomness to surface unexpected-but-related knowledge — great for creative
/// ideation and serendipitous discovery.
#[utoipa::path(
    get,
    path = "/api/insights/serendipity",
    params(SerendipityQuery),
    responses(
        (status = 200, description = "Serendipity walk result", body = atomic_core::models::SerendipityWalkResult),
        (status = 404, description = "Start atom not found"),
    ),
    tag = "insights"
)]
pub async fn serendipity_walk(
    db: Db,
    query: web::Query<SerendipityQuery>,
) -> HttpResponse {
    let start_atom_id = query.start_atom_id.clone();
    let steps = query.steps.unwrap_or(5).min(10);
    let randomness = query.randomness.unwrap_or(0.4).clamp(0.0, 1.0);
    // Default seed: use current time in nanoseconds for a different walk each call
    let seed = query.seed.unwrap_or_else(|| {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.subsec_nanos() as u64)
            .unwrap_or(42)
    });
    let core = db.0;
    blocking_ok(move || core.serendipity_walk(&start_atom_id, steps, randomness, seed)).await
}

// ---------------------------------------------------------------------------
// Time Capsule
// ---------------------------------------------------------------------------

#[derive(Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct TimeCapsuleQuery {
    /// Atoms created more than this many days ago are "old" (default: 30)
    pub lookback_days: Option<i32>,
    /// Atoms created within the last N days are "new" (default: 7)
    pub recent_days: Option<i32>,
    /// Minimum similarity score for a pair to appear (default: 0.5)
    pub similarity_threshold: Option<f32>,
    /// Maximum number of pairs to return (default: 20)
    pub limit: Option<usize>,
}

/// Surface old atoms that are semantically similar to recently added ones.
///
/// Finds pairs of (old atom, new atom) that share semantic similarity,
/// resurfacing knowledge you captured long ago that is suddenly relevant again
/// given what you've been adding recently.
#[utoipa::path(
    get,
    path = "/api/insights/time-capsule",
    params(TimeCapsuleQuery),
    responses(
        (status = 200, description = "Time capsule pairs", body = atomic_core::models::TimeCapsuleResult),
    ),
    tag = "insights"
)]
pub async fn time_capsule(
    db: Db,
    query: web::Query<TimeCapsuleQuery>,
) -> HttpResponse {
    let lookback_days = query.lookback_days.unwrap_or(30);
    let recent_days = query.recent_days.unwrap_or(7);
    let similarity_threshold = query.similarity_threshold.unwrap_or(0.5);
    let limit = query.limit.unwrap_or(20);
    let core = db.0;
    blocking_ok(move || {
        core.time_capsule(lookback_days, recent_days, similarity_threshold, limit)
    })
    .await
}
