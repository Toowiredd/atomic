//! Daily briefing scheduled task.
//!
//! Wraps [`super::run_briefing`] in the [`ScheduledTask`] trait so the
//! scheduler loop can drive it on a timer. State (`last_run`, `enabled`,
//! `interval_hours`) lives in the per-database settings table keyed under
//! `task.daily_briefing.*`.

use crate::scheduler::{state as task_state, ScheduledTask, TaskContext, TaskError, TaskEvent};
use crate::AtomicCore;
use async_trait::async_trait;
use chrono::Utc;
use std::time::Duration;

/// The daily briefing task.
pub struct DailyBriefingTask;

const TASK_ID: &str = "daily_briefing";
const DEFAULT_INTERVAL: Duration = Duration::from_secs(24 * 60 * 60);
const DEFAULT_ENABLED: bool = true;

/// When the task has never run, seed `since` with this lookback so the
/// first briefing has real material to summarize.
const FIRST_RUN_LOOKBACK_DAYS: i64 = 7;

#[async_trait]
impl ScheduledTask for DailyBriefingTask {
    fn id(&self) -> &'static str {
        TASK_ID
    }

    fn display_name(&self) -> &'static str {
        "Daily briefing"
    }

    fn default_interval(&self) -> Duration {
        DEFAULT_INTERVAL
    }

    async fn run(&self, core: &AtomicCore, ctx: &TaskContext) -> Result<(), TaskError> {
        if !task_state::is_enabled(core, TASK_ID, DEFAULT_ENABLED) {
            return Err(TaskError::Disabled);
        }
        if !task_state::is_due(core, TASK_ID, DEFAULT_INTERVAL, DEFAULT_ENABLED) {
            return Err(TaskError::NotDue);
        }

        // Resolve the db_id for event reporting. For SQLite we use the file
        // stem; Postgres is not supported for briefings but the task ID is
        // still meaningful.
        let db_id = core
            .db_path()
            .file_stem()
            .and_then(|s| s.to_str())
            .map(|s| s.to_string())
            .unwrap_or_else(|| "default".to_string());

        let since = task_state::get_last_run(core, TASK_ID)?
            .unwrap_or_else(|| Utc::now() - chrono::Duration::days(FIRST_RUN_LOOKBACK_DAYS));

        (ctx.event_cb)(TaskEvent::Started {
            task_id: TASK_ID.to_string(),
            db_id: db_id.clone(),
        });

        match super::run_briefing(core, since).await {
            Ok(result) => {
                // Persist last_run so subsequent ticks correctly skip until
                // the next interval elapses.
                if let Err(e) = task_state::set_last_run(core, TASK_ID, Utc::now()) {
                    tracing::warn!(
                        task_id = TASK_ID,
                        error = %e,
                        "[scheduler] Failed to persist task last_run"
                    );
                }
                (ctx.event_cb)(TaskEvent::Completed {
                    task_id: TASK_ID.to_string(),
                    db_id,
                    result_id: Some(result.briefing.id.clone()),
                });
                Ok(())
            }
            Err(e) => {
                let msg = e.to_string();
                (ctx.event_cb)(TaskEvent::Failed {
                    task_id: TASK_ID.to_string(),
                    db_id,
                    error: msg.clone(),
                });
                Err(TaskError::Other(msg))
            }
        }
    }
}
