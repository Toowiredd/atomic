//! Bridge between atomic-core callback events and the server broadcast channel
//!
//! Provides helper functions that create callback closures which forward
//! EmbeddingEvent and ChatEvent instances into the tokio broadcast channel
//! as ServerEvent variants.

use crate::state::ServerEvent;
use std::sync::Arc;
use tokio::sync::broadcast;

/// Create an EmbeddingEvent callback that broadcasts to WebSocket clients
pub fn embedding_event_callback(
    tx: broadcast::Sender<ServerEvent>,
) -> impl Fn(atomic_core::EmbeddingEvent) + Send + Sync + Clone + 'static {
    move |event: atomic_core::EmbeddingEvent| {
        let _ = tx.send(ServerEvent::from(event));
    }
}

/// Create an IngestionEvent callback that broadcasts to WebSocket clients
pub fn ingestion_event_callback(
    tx: broadcast::Sender<ServerEvent>,
) -> impl Fn(atomic_core::IngestionEvent) + Send + Sync + Clone + 'static {
    move |event: atomic_core::IngestionEvent| {
        let _ = tx.send(ServerEvent::from(event));
    }
}

/// Create a ChatEvent callback that broadcasts to WebSocket clients
pub fn chat_event_callback(
    tx: broadcast::Sender<ServerEvent>,
) -> impl Fn(atomic_core::ChatEvent) + Send + Sync + 'static {
    move |event: atomic_core::ChatEvent| {
        let _ = tx.send(ServerEvent::from(event));
    }
}

/// Create a TaskEvent callback for the scheduler. Only the daily briefing
/// task produces a broadcast-visible event right now (BriefingReady). Other
/// task events are logged at debug level and ignored.
pub fn task_event_callback(
    tx: broadcast::Sender<ServerEvent>,
) -> Arc<dyn Fn(atomic_core::scheduler::TaskEvent) + Send + Sync> {
    Arc::new(move |event: atomic_core::scheduler::TaskEvent| {
        use atomic_core::scheduler::TaskEvent;
        match event {
            TaskEvent::Completed {
                task_id,
                db_id,
                result_id,
            } if task_id == "daily_briefing" => {
                if let Some(briefing_id) = result_id {
                    let _ = tx.send(ServerEvent::BriefingReady { db_id, briefing_id });
                }
            }
            TaskEvent::Started { task_id, db_id } => {
                tracing::debug!(task_id, db_id, "[scheduler] task started");
            }
            TaskEvent::Completed { task_id, db_id, .. } => {
                tracing::debug!(task_id, db_id, "[scheduler] task completed");
            }
            TaskEvent::Failed {
                task_id,
                db_id,
                error,
            } => {
                tracing::debug!(task_id, db_id, error = %error, "[scheduler] task failed");
            }
        }
    })
}
