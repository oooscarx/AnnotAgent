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
    /// Read-only index of the existing Sample Operations linked to one conversation task.
    /// Application/HTTP callers must first validate the stable conversation owner.
    pub fn conversation_sample_operations(
        &self,
        project: &str,
        conversation: uuid::Uuid,
        task: uuid::Uuid,
    ) -> Result<Vec<SampleOperation>, StorageError> {
        let ids = self.with_connection(|connection| {
            let mut statement = connection.prepare("SELECT id FROM sample_operations WHERE project_id=?1 AND json_extract(request_json,'$.conversation.conversation_id')=?2 AND json_extract(request_json,'$.conversation.task_id')=?3 ORDER BY rowid DESC LIMIT 100")?;
            Ok(statement.query_map(params![project,conversation.to_string(),task.to_string()], |row| row.get::<_,String>(0))?.collect::<Result<Vec<_>,_>>()?)
        })?;
        ids.iter()
            .map(|id| self.sample_operation(id))
            .collect::<Result<Vec<_>, _>>()
            .map(|items| items.into_iter().flatten().collect())
    }
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
            if value.request["conversation"]["human_review"] == true {
                transaction.execute("INSERT INTO conversation_sample_assistance(sample_id) VALUES(?1)",[&value.id])?;
            }
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

    /// Durable local work becomes deliverable only once the actual sample report is saved.
    pub fn pending_sample_assistance(&self) -> Result<Vec<SampleOperation>, StorageError> {
        let ids = self.with_connection(|db| {
            let mut query = db.prepare("SELECT a.sample_id FROM conversation_sample_assistance a JOIN sample_operations o ON o.id=a.sample_id JOIN workflow_sample_tests t ON t.id=a.sample_id WHERE a.status='waiting' AND o.status='succeeded' ORDER BY o.rowid")?;
            Ok(query.query_map([], |row| row.get::<_,String>(0))?.collect::<Result<Vec<_>,_>>()?)
        })?;
        ids.iter()
            .map(|id| self.sample_operation(id))
            .collect::<Result<Vec<_>, _>>()
            .map(|items| items.into_iter().flatten().collect())
    }

    pub fn settle_sample_assistance(
        &self,
        id: &str,
        error: Option<&str>,
    ) -> Result<(), StorageError> {
        self.with_connection(|db| {
            db.execute("UPDATE conversation_sample_assistance SET status=CASE WHEN ?2 IS NULL THEN 'completed' ELSE 'failed' END,error=?2 WHERE sample_id=?1 AND status='waiting'",params![id,error.map(|error|error.chars().take(1600).collect::<String>())])?;
            Ok(())
        })
    }

    pub fn sample_assistance_status(
        &self,
        id: &str,
    ) -> Result<Option<serde_json::Value>, StorageError> {
        self.with_connection(|db| Ok(db.query_row("SELECT status,error FROM conversation_sample_assistance WHERE sample_id=?1",[id],|row| Ok(serde_json::json!({"status":row.get::<_,String>(0)?,"error":row.get::<_,Option<String>>(1)?}))).optional()?))
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
    fn assistance_opt_in_is_atomic_and_failures_do_not_loop() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("TEST-assistance.sqlite");
        let store = SqliteStore::open(&path).unwrap();
        let legacy = operation("legacy");
        store.reserve_sample_operation(&legacy).unwrap();
        store.finish_sample_operation(&legacy.id, None).unwrap();
        assert!(
            store
                .sample_assistance_status(&legacy.id)
                .unwrap()
                .is_none()
        );
        let mut opted = operation("opted");
        opted.request = serde_json::json!({"conversation":{"human_review":true}});
        store.with_connection(|db| { db.execute_batch("CREATE TRIGGER fail_assistance BEFORE INSERT ON conversation_sample_assistance BEGIN SELECT RAISE(ABORT,'TEST rollback'); END;")?;Ok(()) }).unwrap();
        assert!(store.reserve_sample_operation(&opted).is_err());
        assert!(store.sample_operation(&opted.id).unwrap().is_none());
        store
            .with_connection(|db| {
                db.execute_batch("DROP TRIGGER fail_assistance;")?;
                Ok(())
            })
            .unwrap();
        store.reserve_sample_operation(&opted).unwrap();
        store.finish_sample_operation(&opted.id, None).unwrap();
        assert!(
            store.pending_sample_assistance().unwrap().is_empty(),
            "no report means no delivery"
        );
        store
            .settle_sample_assistance(&opted.id, Some("TEST unavailable source"))
            .unwrap();
        drop(store);
        let store = SqliteStore::open(&path).unwrap();
        assert!(store.pending_sample_assistance().unwrap().is_empty());
        assert_eq!(
            store.sample_assistance_status(&opted.id).unwrap().unwrap()["error"],
            "TEST unavailable source"
        );
        assert!(
            store
                .sample_assistance_status(&legacy.id)
                .unwrap()
                .is_none()
        );
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
    fn conversation_index_preserves_consent_and_rejects_changed_retry_context() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("TEST-sample-context.db");
        let store = SqliteStore::open(&path).unwrap();
        let conversation = uuid::Uuid::new_v4();
        let task = uuid::Uuid::new_v4();
        let mut value = operation("conversation-sample");
        value.request = serde_json::json!({"execution":{"image_indices":[0]},"conversation":{"conversation_id":conversation,"task_id":task,"scope_hash":"TEST-scope"}});
        assert!(store.reserve_sample_operation(&value).unwrap());
        store.finish_sample_operation(&value.id, None).unwrap();
        assert!(!store.reserve_sample_operation(&value).unwrap());
        let mut changed = value.clone();
        changed.request["conversation"]["task_id"] = serde_json::json!(uuid::Uuid::new_v4());
        assert!(store.reserve_sample_operation(&changed).is_err());
        drop(store);
        let store = SqliteStore::open(path).unwrap();
        let saved = store
            .conversation_sample_operations("project", conversation, task)
            .unwrap();
        assert_eq!(saved.len(), 1);
        assert_eq!(saved[0].request, value.request);
        assert_eq!(saved[0].status, "succeeded");
        assert!(
            store
                .conversation_sample_operations("foreign", conversation, task)
                .unwrap()
                .is_empty()
        );
        assert!(
            store
                .conversation_sample_operations("project", conversation, uuid::Uuid::new_v4())
                .unwrap()
                .is_empty()
        );
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
