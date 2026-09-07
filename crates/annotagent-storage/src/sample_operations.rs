//! Durable lifecycle receipts for the existing non-committing Sample Test executor.
use super::{SqliteStore, StorageError};
use chrono::Utc;
use rusqlite::{OptionalExtension, params};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SampleOperation {
    pub id: String,
    pub project_id: String,
    pub draft_id: String,
    pub authorization_fingerprint: String,
    pub request: serde_json::Value,
    pub status: String,
    pub error: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

impl SqliteStore {
    pub fn sample_operation(&self, id: &str) -> Result<Option<SampleOperation>, StorageError> {
        self.with_connection(|connection| {
            connection.query_row(
                "SELECT id,project_id,draft_id,authorization_fingerprint,request_json,status,error,created_at,updated_at FROM sample_operations WHERE id=?1",
                [id], |row| {
                    let request: String = row.get(4)?;
                    Ok(SampleOperation { id: row.get(0)?, project_id: row.get(1)?, draft_id: row.get(2)?, authorization_fingerprint: row.get(3)?, request: serde_json::from_str(&request).map_err(|error| rusqlite::Error::FromSqlConversionFailure(4, rusqlite::types::Type::Text, Box::new(error)))?, status: row.get(5)?, error: row.get(6)?, created_at: row.get(7)?, updated_at: row.get(8)? })
                },
            ).optional().map_err(Into::into)
        })
    }

    /// The idempotency key is the future Sample Test ID; no secret is persisted.
    pub fn reserve_sample_operation(&self, value: &SampleOperation) -> Result<bool, StorageError> {
        self.reserve_sample_operation_sealed(value, None)
    }

    pub fn reserve_sample_operation_sealed(
        &self,
        value: &SampleOperation,
        scope: Option<&serde_json::Value>,
    ) -> Result<bool, StorageError> {
        self.with_connection(|connection| {
            let transaction = connection.unchecked_transaction()?;
            let existing = transaction.query_row("SELECT project_id,draft_id,authorization_fingerprint,request_json FROM sample_operations WHERE id=?1", [&value.id], |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?, row.get::<_, String>(2)?, row.get::<_, String>(3)?))).optional()?;
            let request = serde_json::to_string(&value.request)?;
            if let Some((project, draft, fingerprint, saved_request)) = existing {
                if project != value.project_id || draft != value.draft_id || fingerprint != value.authorization_fingerprint || saved_request != request {
                    return Err(StorageError::InvalidSampleOperation("idempotency key already belongs to a different sample request".to_owned()));
                }
                return Ok(false);
            }
            let active: u32 = transaction.query_row("SELECT COUNT(*) FROM sample_operations WHERE status IN ('queued','running','cancelling')", [], |row| row.get(0))?;
            if active >= 2 {
                return Err(StorageError::InvalidSampleOperation("two sample tasks are already active; wait or stop one before starting another".to_owned()));
            }
            let project_active: bool = transaction.query_row("SELECT EXISTS(SELECT 1 FROM sample_operations WHERE project_id=?1 AND status IN ('queued','running','cancelling'))", [&value.project_id], |row| row.get(0))?;
            if project_active {
                return Err(StorageError::InvalidSampleOperation("this Project already has an active sample task".to_owned()));
            }
            transaction.execute("INSERT INTO sample_operations(id,project_id,draft_id,authorization_fingerprint,request_json,status,created_at,updated_at) VALUES(?1,?2,?3,?4,?5,'queued',?6,?6)", params![value.id,value.project_id,value.draft_id,value.authorization_fingerprint,request,value.created_at])?;
            if let Some(scope) = scope {
                transaction.execute("INSERT INTO sample_scope_seals(sample_test_id,scope_json) VALUES(?1,?2)", params![value.id,serde_json::to_string(scope)?])?;
            }
            transaction.commit()?;
            Ok(true)
        })
    }

    pub fn start_sample_operation(&self, id: &str) -> Result<bool, StorageError> {
        self.with_connection(|connection| Ok(connection.execute("UPDATE sample_operations SET status='running',updated_at=?2 WHERE id=?1 AND status='queued'", params![id, Utc::now().to_rfc3339()])? == 1))
    }

    pub fn cancel_sample_operation(&self, id: &str, project_id: &str) -> Result<(), StorageError> {
        self.with_connection(|connection| {
            connection.execute("UPDATE sample_operations SET status='cancelling',updated_at=?3 WHERE id=?1 AND project_id=?2 AND status IN ('queued','running')", params![id,project_id,Utc::now().to_rfc3339()])?;
            Ok(())
        })
    }

    pub fn finish_sample_operation(
        &self,
        id: &str,
        error: Option<&str>,
    ) -> Result<(), StorageError> {
        self.with_connection(|connection| {
            connection.execute("UPDATE sample_operations SET status=CASE WHEN status='cancelling' THEN 'cancelled' WHEN ?2 IS NULL THEN 'succeeded' ELSE 'failed' END,error=?2,updated_at=?3 WHERE id=?1 AND status IN ('queued','running','cancelling')", params![id,error,Utc::now().to_rfc3339()])?;
            Ok(())
        })
    }

    /// Called at application startup alongside existing Run/Agent recovery. Never re-executes.
    pub fn recover_sample_operations(&self) -> Result<(), StorageError> {
        self.with_connection(|connection| {
            let transaction = connection.unchecked_transaction()?;
            transaction.execute("DELETE FROM management_entity_leases WHERE lease_kind='sample_plan_revision'", [])?;
            transaction.execute("DELETE FROM management_entity_leases WHERE lease_kind='sample_test' AND owner IN (SELECT id FROM sample_operations WHERE status IN ('queued','running','cancelling'))", [])?;
            transaction.execute("UPDATE sample_operations SET status=CASE WHEN status='cancelling' THEN 'cancelled' WHEN EXISTS(SELECT 1 FROM workflow_sample_tests WHERE workflow_sample_tests.id=sample_operations.id) THEN 'succeeded' ELSE 'interrupted' END,error=CASE WHEN status='cancelling' OR EXISTS(SELECT 1 FROM workflow_sample_tests WHERE workflow_sample_tests.id=sample_operations.id) THEN NULL ELSE 'The server stopped before this sample task completed. Saved evidence remains; no automatic retry was started.' END,updated_at=?1 WHERE status IN ('queued','running','cancelling')", [Utc::now().to_rfc3339()])?;
            transaction.commit()?;
            Ok(())
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn operation(id: &str) -> SampleOperation {
        SampleOperation {
            id: id.to_owned(),
            project_id: "project".into(),
            draft_id: "draft".into(),
            authorization_fingerprint: "scope".into(),
            request: serde_json::json!({"image_indices":[0]}),
            status: "queued".into(),
            error: None,
            created_at: Utc::now().to_rfc3339(),
            updated_at: Utc::now().to_rfc3339(),
        }
    }
    #[test]
    fn admission_is_atomic_and_terminal_receipts_do_not_execute_twice() {
        let store = std::sync::Arc::new(SqliteStore::open_in_memory().unwrap());
        let barrier = std::sync::Arc::new(std::sync::Barrier::new(4));
        let workers = (0..4)
            .map(|_| {
                let store = store.clone();
                let barrier = barrier.clone();
                std::thread::spawn(move || {
                    barrier.wait();
                    store
                        .reserve_sample_operation(&operation("same-request"))
                        .unwrap()
                })
            })
            .collect::<Vec<_>>();
        assert_eq!(
            workers
                .into_iter()
                .map(|worker| usize::from(worker.join().unwrap()))
                .sum::<usize>(),
            1
        );
        store.start_sample_operation("same-request").unwrap();
        store.finish_sample_operation("same-request", None).unwrap();
        assert!(
            !store
                .reserve_sample_operation(&operation("same-request"))
                .unwrap()
        );
        assert!(!store.start_sample_operation("same-request").unwrap());
        assert_eq!(
            store
                .sample_operation("same-request")
                .unwrap()
                .unwrap()
                .status,
            "succeeded"
        );
        let one = operation("one");
        let mut two = operation("two");
        two.project_id = "second-project".into();
        let mut three = operation("three");
        three.project_id = "third-project".into();
        assert!(store.reserve_sample_operation(&one).unwrap());
        assert!(store.reserve_sample_operation(&two).unwrap());
        assert!(store.reserve_sample_operation(&three).is_err());
    }

    #[test]
    fn restart_preserves_receipts_without_reexecution() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("sample.sqlite");
        {
            let store = SqliteStore::open(&path).unwrap();
            store
                .reserve_sample_operation(&operation("pending"))
                .unwrap();
            store.start_sample_operation("pending").unwrap();
        }
        let store = SqliteStore::open(&path).unwrap();
        assert_eq!(
            store.sample_operation("pending").unwrap().unwrap().status,
            "running"
        );
        store.recover_sample_operations().unwrap();
        assert_eq!(
            store.sample_operation("pending").unwrap().unwrap().status,
            "interrupted"
        );
        assert!(
            !store
                .reserve_sample_operation(&operation("pending"))
                .unwrap()
        );
        assert!(!store.start_sample_operation("pending").unwrap());
    }

    #[test]
    fn receipts_deduplicate_scope_and_cancel_is_terminal() {
        let store = SqliteStore::open_in_memory().expect("store");
        let item = operation("one");
        assert!(store.reserve_sample_operation(&item).unwrap());
        assert!(!store.reserve_sample_operation(&item).unwrap());
        let mut other = item.clone();
        other.authorization_fingerprint = "changed".into();
        assert!(store.reserve_sample_operation(&other).is_err());
        assert!(store.reserve_sample_operation(&operation("two")).is_err());
        store.cancel_sample_operation("one", "wrong-owner").unwrap();
        assert!(store.start_sample_operation("one").unwrap());
        store.cancel_sample_operation("one", "project").unwrap();
        store.finish_sample_operation("one", None).unwrap();
        assert_eq!(
            store.sample_operation("one").unwrap().unwrap().status,
            "cancelled"
        );
        assert!(!store.reserve_sample_operation(&item).unwrap());
        assert!(store.reserve_sample_operation(&operation("two")).unwrap());
        store.recover_sample_operations().unwrap();
        assert_eq!(
            store.sample_operation("two").unwrap().unwrap().status,
            "interrupted"
        );
    }
}
