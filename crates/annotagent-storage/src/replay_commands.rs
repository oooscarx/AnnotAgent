use crate::{SqliteStore, StorageError};
use rusqlite::{OptionalExtension, params};
use serde_json::{Value, json};
use uuid::Uuid;
impl SqliteStore {
    pub fn replay_command(
        &self,
        id: Uuid,
        project: &str,
        run: &str,
        node: &str,
    ) -> Result<Option<Value>, StorageError> {
        self.with_connection(|db|{
            let value:Option<String>=db.query_row("SELECT receipt_json FROM replay_commands WHERE id=?1 AND project_id=?2 AND run_id=?3 AND node_id=?4",params![id.to_string(),project,run,node],|r|r.get(0)).optional()?;
            value.map(|s|serde_json::from_str(&s).map_err(Into::into)).transpose()
        })
    }
    pub fn reserve_replay_command(
        &self,
        id: Uuid,
        project: &str,
        run: &str,
        node: &str,
        request: &Value,
        epoch: &str,
    ) -> Result<(Value, bool), StorageError> {
        self.reserve_replay_command_with_scope(id, project, run, node, request, epoch, None)
    }
    #[allow(clippy::too_many_arguments)]
    pub fn reserve_replay_command_with_scope(
        &self,
        id: Uuid,
        project: &str,
        run: &str,
        node: &str,
        request: &Value,
        epoch: &str,
        scope: Option<&Value>,
    ) -> Result<(Value, bool), StorageError> {
        self.with_connection(|db|{
            let tx=rusqlite::Transaction::new_unchecked(db,rusqlite::TransactionBehavior::Immediate)?;
            let old:Option<String>=tx.query_row("SELECT receipt_json FROM replay_commands WHERE id=?1",[id.to_string()],|r|r.get(0)).optional()?;
            if let Some(old)=old {let old:Value=serde_json::from_str(&old)?;if old["request"]!=*request || old["project_id"]!=project || old["run_id"]!=run || old["node_id"]!=node {return Err(StorageError::Management{code:"replay_command_conflict".into(),message:"Replay command already has another scope".into()});}return Ok((old,false));}
            let now=chrono::Utc::now().to_rfc3339();
            let value=json!({"command_id":id,"project_id":project,"run_id":run,"node_id":node,"request":request,"authorized_scope":scope,"status":"running","started_at":now,"completed_at":null,"result":null,"failure":null,"epoch":epoch});
            tx.execute("INSERT INTO replay_commands(id,project_id,run_id,node_id,receipt_json) VALUES(?1,?2,?3,?4,?5)",params![id.to_string(),project,run,node,serde_json::to_string(&value)?])?;tx.commit()?;Ok((value,true))
        })
    }
    pub fn settle_replay_command(
        &self,
        id: Uuid,
        result: Option<Value>,
        failure: Option<&str>,
    ) -> Result<(), StorageError> {
        self.with_connection(|db| {
            let tx = db.unchecked_transaction()?;
            let raw: String = tx.query_row(
                "SELECT receipt_json FROM replay_commands WHERE id=?1",
                [id.to_string()],
                |r| r.get(0),
            )?;
            let mut value: Value = serde_json::from_str(&raw)?;
            if value["status"] != "running" {
                return Ok(());
            }
            value["status"] = json!(if result.is_some() {
                "completed"
            } else {
                "outcome_unknown"
            });
            value["result"] = result.unwrap_or(Value::Null);
            value["failure"] = json!(failure);
            value["completed_at"] = json!(chrono::Utc::now().to_rfc3339());
            tx.execute(
                "UPDATE replay_commands SET receipt_json=?2 WHERE id=?1",
                params![id.to_string(), serde_json::to_string(&value)?],
            )?;
            tx.commit()?;
            Ok(())
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn replay_command_reserves_once_and_survives_restart_without_redispatch() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("TEST-replay.sqlite");
        let a = SqliteStore::open(&path).unwrap();
        let b = SqliteStore::open(&path).unwrap();
        let id = Uuid::new_v4();
        let body = json!({"scope_hash":"TEST-exact"});
        let barrier = std::sync::Arc::new(std::sync::Barrier::new(2));
        let other = barrier.clone();
        let copy = body.clone();
        let thread = std::thread::spawn(move || {
            other.wait();
            b.reserve_replay_command(id, "TEST-P", "TEST-R", "TEST-N", &copy, "TEST-epoch")
                .unwrap()
        });
        barrier.wait();
        let first = a
            .reserve_replay_command(id, "TEST-P", "TEST-R", "TEST-N", &body, "TEST-epoch")
            .unwrap();
        let second = thread.join().unwrap();
        assert_ne!(first.1, second.1);
        assert_eq!(first.0, second.0);
        drop(a);
        let a = SqliteStore::open(&path).unwrap();
        assert!(
            !a.reserve_replay_command(id, "TEST-P", "TEST-R", "TEST-N", &body, "new-process")
                .unwrap()
                .1
        );
        assert!(
            a.replay_command(id, "TEST-foreign", "TEST-R", "TEST-N")
                .unwrap()
                .is_none()
        );
        assert!(
            a.reserve_replay_command(id, "TEST-P", "TEST-R", "changed", &body, "TEST-epoch")
                .is_err()
        );
        a.settle_replay_command(id, Some(json!({"sandbox":true})), None)
            .unwrap();
        let receipt = a
            .replay_command(id, "TEST-P", "TEST-R", "TEST-N")
            .unwrap()
            .unwrap();
        assert_eq!(receipt["status"], "completed");
        assert_eq!(
            a.reserve_replay_command(id, "TEST-P", "TEST-R", "TEST-N", &body, "new")
                .unwrap(),
            (receipt, false)
        );
    }
}
