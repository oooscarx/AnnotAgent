//! Receipts compose existing publication and Batch boundaries; no alternate Workflow.
use super::{SqliteStore, StorageError};
use chrono::Utc;
use rusqlite::{OptionalExtension, params};
use serde_json::Value;

impl SqliteStore {
    /// Reserve before an external request; failed/uncertain sends are never refunded.
    pub fn reserve_batch_model_call(
        &self,
        batch_id: annotagent_core::BatchId,
    ) -> Result<(), StorageError> {
        self.with_connection(|connection| {
            if connection.execute("UPDATE batch_model_call_allowances SET reserved=reserved+1 WHERE batch_id=?1 AND reserved<maximum", [batch_id.to_string()])? != 1 {
                return Err(StorageError::InvalidEnum("Processing model-call allowance exhausted or unavailable; no request was sent".into()));
            }
            Ok(())
        })
    }

    pub fn save_sample_scope_seal(&self, id: &str, scope: &Value) -> Result<(), StorageError> {
        self.with_connection(|connection| {
            connection.execute("INSERT INTO sample_scope_seals(sample_test_id,scope_json) VALUES(?1,?2) ON CONFLICT(sample_test_id) DO NOTHING", params![id, serde_json::to_string(scope)?])?;
            Ok(())
        })
    }

    pub fn sample_scope_seal(&self, id: &str) -> Result<Option<Value>, StorageError> {
        self.with_connection(|connection| {
            let value: Option<String> = connection
                .query_row(
                    "SELECT scope_json FROM sample_scope_seals WHERE sample_test_id=?1",
                    [id],
                    |row| row.get(0),
                )
                .optional()?;
            value
                .map(|value| serde_json::from_str(&value).map_err(Into::into))
                .transpose()
        })
    }

    pub fn processing_operation(&self, id: &str) -> Result<Option<Value>, StorageError> {
        self.with_connection(|connection| {
            let value: Option<String> = connection
                .query_row(
                    "SELECT state_json FROM processing_operations WHERE id=?1",
                    [id],
                    |row| row.get(0),
                )
                .optional()?;
            value
                .map(|value| serde_json::from_str(&value).map_err(Into::into))
                .transpose()
        })
    }

    pub fn reserve_processing_operation(
        &self,
        id: &str,
        project: &str,
        request: &Value,
        state: &Value,
    ) -> Result<(), StorageError> {
        self.with_connection(|connection| {
            let transaction = connection.unchecked_transaction()?;
            let existing: Option<(String, String)> = transaction.query_row("SELECT project_id,request_json FROM processing_operations WHERE id=?1", [id], |row| Ok((row.get(0)?, row.get(1)?))).optional()?;
            let serialized = serde_json::to_string(request)?;
            if let Some((owner, stored)) = existing {
                if owner != project || stored != serialized {
                    return Err(StorageError::InvalidEnum("Processing request key belongs to different authorization".into()));
                }
            } else {
                transaction.execute("INSERT INTO processing_operations(id,project_id,request_json,state_json,created_at,updated_at) VALUES(?1,?2,?3,?4,?5,?5)", params![id,project,serialized,serde_json::to_string(state)?,Utc::now().to_rfc3339()])?;
            }
            transaction.commit()?;
            Ok(())
        })
    }

    pub fn update_processing_operation(&self, id: &str, state: &Value) -> Result<(), StorageError> {
        self.with_connection(|connection| {
            if connection.execute(
                "UPDATE processing_operations SET state_json=?2,updated_at=?3 WHERE id=?1",
                params![id, serde_json::to_string(state)?, Utc::now().to_rfc3339()],
            )? != 1
            {
                return Err(StorageError::InvalidEnum(
                    "Processing receipt was not found".into(),
                ));
            }
            Ok(())
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn confirmation_key_is_scoped_and_survives_partial_publication() {
        let store = SqliteStore::open_in_memory().unwrap();
        let request = serde_json::json!({"draft":"draft-a","test":"test-a","budget":12});
        let state = serde_json::json!({"id":"one","project_id":"p","phase":"confirmed"});
        store
            .reserve_processing_operation("one", "p", &request, &state)
            .unwrap();
        let published = serde_json::json!({"id":"one","project_id":"p","phase":"published_start_failed","workflow_id":"draft-a","version":1});
        store
            .update_processing_operation("one", &published)
            .unwrap();
        store
            .reserve_processing_operation("one", "p", &request, &state)
            .unwrap();
        assert_eq!(store.processing_operation("one").unwrap(), Some(published));
        assert!(
            store
                .reserve_processing_operation("one", "other-project", &request, &state)
                .is_err()
        );
        assert!(
            store
                .reserve_processing_operation("one", "p", &serde_json::json!({"budget":24}), &state)
                .is_err()
        );
    }

    #[test]
    fn external_call_allowance_is_atomic_persistent_and_not_refunded() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("budget.db");
        let id = annotagent_core::BatchId::new();
        {
            let store = std::sync::Arc::new(SqliteStore::open(&path).unwrap());
            store.with_connection(|connection| { connection.execute("INSERT INTO batch_model_call_allowances(batch_id,maximum,reserved) VALUES(?1,2,0)", [id.to_string()])?; Ok(()) }).unwrap();
            let workers = (0..5)
                .map(|_| {
                    let store = store.clone();
                    std::thread::spawn(move || store.reserve_batch_model_call(id).is_ok())
                })
                .collect::<Vec<_>>();
            assert_eq!(
                workers
                    .into_iter()
                    .map(|worker| usize::from(worker.join().unwrap()))
                    .sum::<usize>(),
                2
            );
        }
        let store = SqliteStore::open(&path).unwrap();
        assert!(store.reserve_batch_model_call(id).is_err());
        assert!(
            store
                .reserve_batch_model_call(annotagent_core::BatchId::new())
                .is_err()
        );
    }
}
