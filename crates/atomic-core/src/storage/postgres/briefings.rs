//! Postgres stub for briefings.
//!
//! Briefings are SQLite-only in v1. The Postgres backend returns a
//! Configuration error for every call so the scheduler loop can skip the
//! task cleanly without touching the runtime.

use super::PostgresStorage;
use crate::briefing::{Briefing, BriefingCitation, BriefingWithCitations};
use crate::error::AtomicCoreError;
use crate::models::AtomWithTags;
use crate::storage::traits::{BriefingStore, StorageResult};
use async_trait::async_trait;

fn unsupported<T>() -> StorageResult<T> {
    Err(AtomicCoreError::Configuration(
        "briefings not yet supported on Postgres".to_string(),
    ))
}

#[async_trait]
impl BriefingStore for PostgresStorage {
    async fn list_new_atoms_since(
        &self,
        _since: &str,
        _limit: i32,
    ) -> StorageResult<Vec<AtomWithTags>> {
        unsupported()
    }

    async fn count_new_atoms_since(&self, _since: &str) -> StorageResult<i32> {
        unsupported()
    }

    async fn insert_briefing(
        &self,
        _briefing: &Briefing,
        _citations: &[BriefingCitation],
    ) -> StorageResult<BriefingWithCitations> {
        unsupported()
    }

    async fn get_latest_briefing(&self) -> StorageResult<Option<BriefingWithCitations>> {
        unsupported()
    }

    async fn get_briefing(&self, _id: &str) -> StorageResult<Option<BriefingWithCitations>> {
        unsupported()
    }

    async fn list_briefings(&self, _limit: i32) -> StorageResult<Vec<Briefing>> {
        unsupported()
    }

    async fn delete_briefing(&self, _id: &str) -> StorageResult<()> {
        unsupported()
    }
}
