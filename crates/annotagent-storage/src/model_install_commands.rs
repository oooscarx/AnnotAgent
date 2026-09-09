use crate::{SqliteStore, StorageError};
use rusqlite::{OptionalExtension, TransactionBehavior, params};
use serde_json::Value;
use uuid::Uuid;
fn conflict(code: &str) -> StorageError {
    StorageError::Management {
        code: code.into(),
        message: code.replace('_', " "),
    }
}
impl SqliteStore {
    pub fn model_install_command(
        &self,
        command: Uuid,
        scope: Option<&Value>,
    ) -> Result<Option<Value>, StorageError> {
        self.with_connection(|c| {
            let row:Option<(String,String)>=c.query_row("SELECT scope_json,operation_json FROM model_install_commands WHERE command_id=?1",[command.to_string()],|r|Ok((r.get(0)?,r.get(1)?))).optional()?;
            row.map(|(saved,operation)| {
                if let Some(scope)=scope && saved!=serde_json::to_string(scope)? {return Err(conflict("model_install_command_conflict"));}
                Ok(serde_json::from_str(&operation)?)
            }).transpose()
        })
    }
    /// Reserve before spawning any installer. The winner alone is allowed to dispatch.
    pub fn reserve_model_install_command(
        &self,
        command: Uuid,
        scope: &Value,
        operation: &Value,
    ) -> Result<(Value, bool), StorageError> {
        self.with_connection(|c| {
            let tx=rusqlite::Transaction::new_unchecked(c,TransactionBehavior::Immediate)?;
            let scope=serde_json::to_string(scope)?;
            let saved:Option<(String,String)>=tx.query_row("SELECT scope_json,operation_json FROM model_install_commands WHERE command_id=?1",[command.to_string()],|r|Ok((r.get(0)?,r.get(1)?))).optional()?;
            if let Some((existing,receipt))=saved {
                if existing!=scope {return Err(conflict("model_install_command_conflict"));}
                return Ok((serde_json::from_str(&receipt)?,false));
            }
            let active:bool=tx.query_row("SELECT EXISTS(SELECT 1 FROM model_install_commands WHERE (scope_json=?1 OR (json_extract(scope_json,'$.bundle_id')=json_extract(?1,'$.bundle_id') AND json_extract(scope_json,'$.bundle_version')=json_extract(?1,'$.bundle_version') AND json_extract(scope_json,'$.plugin_id')=json_extract(?1,'$.plugin_id') AND json_extract(scope_json,'$.plugin_version')=json_extract(?1,'$.plugin_version'))) AND json_extract(operation_json,'$.status') IN ('running','unknown'))",[&scope],|r|r.get(0))?;
            if active {return Err(conflict("model_installation_active"));}
            let id=operation["id"].as_str().ok_or_else(||conflict("invalid_install_operation"))?;
            tx.execute("INSERT INTO model_install_commands(command_id,operation_id,scope_json,operation_json) VALUES(?1,?2,?3,?4)",params![command.to_string(),id,scope,serde_json::to_string(operation)?])?;
            tx.commit()?;Ok((operation.clone(),true))
        })
    }
    pub fn update_model_install_receipt(
        &self,
        id: Uuid,
        operation: &Value,
    ) -> Result<(), StorageError> {
        self.with_connection(|c| {
            let changed = c.execute(
                "UPDATE model_install_commands SET operation_json=?2 WHERE operation_id=?1",
                params![id.to_string(), serde_json::to_string(operation)?],
            )?;
            if changed != 1 {
                return Err(conflict("model_install_receipt_missing"));
            }
            Ok(())
        })
    }
    pub fn model_install_receipt(&self, id: Uuid) -> Result<Option<Value>, StorageError> {
        self.with_connection(|c| {
            let receipt: Option<String> = c
                .query_row(
                    "SELECT operation_json FROM model_install_commands WHERE operation_id=?1",
                    [id.to_string()],
                    |r| r.get(0),
                )
                .optional()?;
            receipt
                .map(|s| serde_json::from_str(&s).map_err(Into::into))
                .transpose()
        })
    }
    pub fn model_install_receipts(&self) -> Result<Vec<Value>, StorageError> {
        self.with_connection(|c| {
            let receipts=c.prepare("SELECT operation_json FROM model_install_commands ORDER BY json_extract(operation_json,'$.updated_at') DESC,operation_id LIMIT 32")?.query_map([],|r|r.get::<_,String>(0))?.collect::<Result<Vec<_>,_>>()?;
            receipts.into_iter().map(|s|serde_json::from_str(&s).map_err(Into::into)).collect()
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn lost_response_restart_replay_and_scope_conflict() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("TEST.sqlite");
        let command = Uuid::new_v4();
        let id = Uuid::new_v4();
        let scope =
            json!({"plugin_id":"TEST","bundle_version":"1","installation_root":"TEST-root"});
        let operation = json!({"id":id,"command_id":command,"scope":scope,"status":"running"});
        let store = SqliteStore::open(&path).unwrap();
        assert!(
            store
                .model_install_command(command, None)
                .unwrap()
                .is_none()
        );
        assert!(store.model_install_receipts().unwrap().is_empty());
        assert!(
            store
                .reserve_model_install_command(command, &scope, &operation)
                .unwrap()
                .1
        );
        drop(store); // Lose response and process state after admission.
        let store = SqliteStore::open(&path).unwrap();
        assert_eq!(
            store
                .reserve_model_install_command(command, &scope, &json!({"id":Uuid::new_v4()}))
                .unwrap(),
            (operation.clone(), false)
        );
        assert_eq!(
            store.model_install_command(command, None).unwrap(),
            Some(operation)
        );
        for changed in [
            json!({"plugin_id":"OTHER"}),
            json!({"plugin_id":"TEST","bundle_version":"1","installation_root":"OTHER-root"}),
        ] {
            assert!(
                matches!(store.reserve_model_install_command(command,&changed,&json!({"id":Uuid::new_v4()})),Err(StorageError::Management{code,..}) if code=="model_install_command_conflict")
            );
        }
        assert!(
            store
                .reserve_model_install_command(
                    Uuid::new_v4(),
                    &scope,
                    &json!({"id":Uuid::new_v4()})
                )
                .is_err()
        );
        assert_eq!(store.model_install_receipts().unwrap().len(), 1);
    }
    #[test]
    fn concurrent_command_has_exactly_one_dispatch_winner() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("TEST.sqlite");
        let stores = [
            SqliteStore::open(&path).unwrap(),
            SqliteStore::open(&path).unwrap(),
        ];
        let command = Uuid::new_v4();
        let barrier = std::sync::Arc::new(std::sync::Barrier::new(2));
        let handles: Vec<_> = stores
            .into_iter()
            .map(|store| {
                let barrier = barrier.clone();
                std::thread::spawn(move || {
                    barrier.wait();
                    store
                        .reserve_model_install_command(
                            command,
                            &json!({"exact":"TEST"}),
                            &json!({"id":Uuid::new_v4(),"status":"running"}),
                        )
                        .unwrap()
                })
            })
            .collect();
        let results: Vec<_> = handles.into_iter().map(|h| h.join().unwrap()).collect();
        assert_eq!(results.iter().filter(|(_, winner)| *winner).count(), 1);
        assert_eq!(results[0].0, results[1].0);
    }
}
