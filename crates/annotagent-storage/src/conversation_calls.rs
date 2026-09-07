//! Durable per-task call allowance shared by coordinator and existing executors.
//! This is a call-count budget, not a claim that unknown provider fees are zero.
use crate::{SqliteStore, StorageError};
use chrono::{DateTime, Utc};
use rusqlite::{OptionalExtension, params};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ConversationCallGrant {
    pub id: Uuid,
    pub task_id: Uuid,
    /// Server-computed binding/data/schema/permission snapshot digest.
    pub scope_hash: String,
    pub maximum_calls: u32,
    pub expires_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConversationCallStatus {
    Reserved,
    Completed,
    Failed,
    InDoubt,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ConversationCallReceipt {
    pub id: Uuid,
    pub task_id: Uuid,
    pub request_hash: String,
    pub status: ConversationCallStatus,
    pub evidence: Option<serde_json::Value>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ConversationCallAdmission {
    /// Only this result permits a caller to send a model request.
    Admitted,
    /// Never re-execute. Reserved means running or indeterminate after interruption.
    Existing(ConversationCallReceipt),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConversationCallCancellation {
    pub call_id: Uuid,
    pub task_id: Uuid,
    pub requested_at: String,
}

fn invalid(message: &str) -> StorageError {
    StorageError::InvalidConversation(message.into())
}
fn digest(value: &str) -> bool {
    value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}
fn owner(db: &rusqlite::Connection, project: &str, task: Uuid) -> Result<(), StorageError> {
    let owned: bool = db.query_row("SELECT EXISTS(SELECT 1 FROM conversation_tasks t JOIN project_conversations c ON c.id=t.conversation_id WHERE t.id=?1 AND c.project_id=?2)", params![task.to_string(),project], |row| row.get(0))?;
    if !owned {
        return Err(invalid("task does not belong to this Project"));
    }
    Ok(())
}
fn receipt(
    db: &rusqlite::Connection,
    id: Uuid,
) -> Result<Option<ConversationCallReceipt>, StorageError> {
    let row: Option<(String,String,String,Option<String>)> = db.query_row("SELECT task_id,request_hash,status,evidence_json FROM conversation_model_calls WHERE id=?1", [id.to_string()], |row| Ok((row.get(0)?,row.get(1)?,row.get(2)?,row.get(3)?))).optional()?;
    row.map(|(task, request_hash, status, evidence)| {
        Ok(ConversationCallReceipt {
            id,
            task_id: Uuid::parse_str(&task).map_err(|_| invalid("invalid saved task ID"))?,
            request_hash,
            status: match status.as_str() {
                "reserved" => ConversationCallStatus::Reserved,
                "completed" => ConversationCallStatus::Completed,
                "failed" => ConversationCallStatus::Failed,
                "in_doubt" => ConversationCallStatus::InDoubt,
                _ => return Err(invalid("invalid call status")),
            },
            evidence: evidence
                .map(|value| serde_json::from_str(&value))
                .transpose()?,
        })
    })
    .transpose()
}

impl SqliteStore {
    /// Durable cancellation may precede HTTP admission; it is not a fake call receipt.
    pub fn request_conversation_call_cancel(
        &self,
        project: &str,
        task: Uuid,
        call: Uuid,
    ) -> Result<ConversationCallCancellation, StorageError> {
        self.with_connection(|db| {
            let tx = db.unchecked_transaction()?;
            owner(&tx,project,task)?;
            if receipt(&tx,call)?.is_some_and(|saved| saved.task_id != task) { return Err(invalid("call belongs to another task")); }
            let existing: Option<(String,String)> = tx.query_row("SELECT task_id,requested_at FROM conversation_call_cancellations WHERE call_id=?1", [call.to_string()], |row| Ok((row.get(0)?,row.get(1)?))).optional()?;
            if let Some((saved_task,requested_at)) = existing {
                if saved_task != task.to_string() { return Err(invalid("cancellation belongs to another task")); }
                return Ok(ConversationCallCancellation { call_id:call,task_id:task,requested_at });
            }
            let requested_at = Utc::now().to_rfc3339();
            tx.execute("INSERT INTO conversation_call_cancellations(call_id,task_id,requested_at) VALUES(?1,?2,?3)", params![call.to_string(),task.to_string(),requested_at])?;
            tx.commit()?; Ok(ConversationCallCancellation { call_id:call,task_id:task,requested_at })
        })
    }

    pub fn conversation_call_cancellations(
        &self,
        project: &str,
        task: Uuid,
    ) -> Result<Vec<ConversationCallCancellation>, StorageError> {
        self.with_connection(|db| {
            owner(db,project,task)?;
            let mut query = db.prepare("SELECT call_id,requested_at FROM conversation_call_cancellations WHERE task_id=?1 ORDER BY requested_at,call_id")?;
            query.query_map([task.to_string()],|row| Ok((row.get::<_,String>(0)?,row.get::<_,String>(1)?)))?.map(|row| {
                let (call,date) = row?; Ok(ConversationCallCancellation { call_id:Uuid::parse_str(&call).map_err(|_| invalid("invalid cancellation ID"))?,task_id:task,requested_at:date })
            }).collect()
        })
    }

    pub fn abandon_conversation_call(
        &self,
        project: &str,
        task: Uuid,
        call: Uuid,
    ) -> Result<(), StorageError> {
        self.with_connection(|db| {
            owner(db,project,task)?;
            db.execute("UPDATE conversation_model_calls SET status='in_doubt',evidence_json=?3 WHERE id=?1 AND task_id=?2 AND status='reserved'",params![call.to_string(),task.to_string(),serde_json::json!({"error":"The call handler ended before saving its response. Remote outcome and cost are unknown; no automatic retry was started."}).to_string()])?;
            Ok(())
        })
    }
    /// Startup recovery never resends an indeterminate Provider request.
    pub fn recover_conversation_calls(&self) -> Result<(), StorageError> {
        self.with_connection(|db| {
            db.execute("UPDATE conversation_model_calls SET status='in_doubt',evidence_json=?1 WHERE status='reserved'", [serde_json::json!({"error":"The server stopped before this call was settled. Remote outcome and cost are unknown; no automatic retry was started."}).to_string()])?;
            Ok(())
        })
    }
    pub fn conversation_calls_active(
        &self,
        project: &str,
        task: Uuid,
    ) -> Result<bool, StorageError> {
        self.with_connection(|db| {
            owner(db, project, task)?;
            let grant: Option<(bool, String)> = db
                .query_row(
                    "SELECT revoked,expires_at FROM conversation_call_grants WHERE task_id=?1",
                    [task.to_string()],
                    |row| Ok((row.get(0)?, row.get(1)?)),
                )
                .optional()?;
            Ok(grant.is_some_and(|(revoked, expires)| {
                !revoked
                    && DateTime::parse_from_rfc3339(&expires)
                        .is_ok_and(|expiry| expiry > Utc::now())
            }))
        })
    }

    pub fn conversation_call_history(
        &self,
        project: &str,
        task: Uuid,
    ) -> Result<Vec<ConversationCallReceipt>, StorageError> {
        self.with_connection(|db| {
            owner(db, project, task)?;
            let mut query = db.prepare(
                "SELECT id FROM conversation_model_calls WHERE task_id=?1 ORDER BY created_at,id",
            )?;
            let ids = query
                .query_map([task.to_string()], |row| row.get::<_, String>(0))?
                .collect::<Result<Vec<_>, _>>()?;
            ids.into_iter()
                .map(|id| {
                    receipt(
                        db,
                        Uuid::parse_str(&id).map_err(|_| invalid("invalid call identity"))?,
                    )?
                    .ok_or_else(|| invalid("call receipt missing"))
                })
                .collect()
        })
    }
    pub fn conversation_call(
        &self,
        project: &str,
        task: Uuid,
        id: Uuid,
    ) -> Result<Option<ConversationCallReceipt>, StorageError> {
        self.with_connection(|db| {
            owner(db, project, task)?;
            let saved = receipt(db, id)?;
            if saved.as_ref().is_some_and(|value| value.task_id != task) {
                return Err(invalid("call belongs to another task"));
            }
            Ok(saved)
        })
    }
    /// Called only after explicit server-side authorization validation. No renewal or reset.
    pub fn authorize_conversation_calls(
        &self,
        project: &str,
        grant: &ConversationCallGrant,
    ) -> Result<(), StorageError> {
        if !digest(&grant.scope_hash) || !(1..=128).contains(&grant.maximum_calls) {
            return Err(invalid("invalid bounded call authorization"));
        }
        self.with_connection(|db| {
            let tx = db.unchecked_transaction()?;
            owner(&tx, project, grant.task_id)?;
            let existing: Option<(String,String,u32,String)> = tx.query_row("SELECT id,scope_hash,maximum_calls,expires_at FROM conversation_call_grants WHERE task_id=?1", [grant.task_id.to_string()], |row| Ok((row.get(0)?,row.get(1)?,row.get(2)?,row.get(3)?))).optional()?;
            if let Some((id,scope,maximum,expires)) = existing {
                if id != grant.id.to_string() || scope != grant.scope_hash || maximum != grant.maximum_calls || expires != grant.expires_at.to_rfc3339() { return Err(invalid("task authorization already exists; retries cannot change its scope or reset its budget")); }
                return Ok(());
            }
            if grant.expires_at <= Utc::now() { return Err(invalid("task authorization expired")); }
            tx.execute("INSERT INTO conversation_call_grants(task_id,id,scope_hash,maximum_calls,expires_at) VALUES(?1,?2,?3,?4,?5)", params![grant.task_id.to_string(),grant.id.to_string(),grant.scope_hash,grant.maximum_calls,grant.expires_at.to_rfc3339()])?;
            tx.commit()?; Ok(())
        })
    }

    pub fn reserve_conversation_call(
        &self,
        project: &str,
        task: Uuid,
        id: Uuid,
        scope_hash: &str,
        request_hash: &str,
    ) -> Result<ConversationCallAdmission, StorageError> {
        if !digest(scope_hash) || !digest(request_hash) {
            return Err(invalid("invalid call snapshot digest"));
        }
        self.with_connection(|db| {
            let tx = db.unchecked_transaction()?;
            owner(&tx, project, task)?;
            if let Some(saved) = receipt(&tx, id)? {
                if saved.task_id != task || saved.request_hash != request_hash { return Err(invalid("call idempotency key belongs to a different request")); }
                let original_scope: String = tx.query_row("SELECT scope_hash FROM conversation_call_grants WHERE task_id=?1", [task.to_string()], |row| row.get(0))?;
                if original_scope != scope_hash { return Err(invalid("call scope changed")); }
                return Ok(ConversationCallAdmission::Existing(saved));
            }
            let cancelled: bool = tx.query_row("SELECT EXISTS(SELECT 1 FROM conversation_call_cancellations WHERE call_id=?1)", [id.to_string()], |row| row.get(0))?;
            if cancelled { return Err(invalid("call cancelled before admission; no request sent")); }
            let grant: Option<(String,u32,String,bool)> = tx.query_row("SELECT scope_hash,maximum_calls,expires_at,revoked FROM conversation_call_grants WHERE task_id=?1", [task.to_string()], |row| Ok((row.get(0)?,row.get(1)?,row.get(2)?,row.get(3)?))).optional()?;
            let Some((scope,maximum,expires,revoked)) = grant else { return Err(invalid("explicit task authorization required")); };
            if scope != scope_hash || revoked || DateTime::parse_from_rfc3339(&expires).map_err(|_| invalid("invalid authorization expiry"))? <= Utc::now() { return Err(invalid("task authorization changed, expired or was revoked")); }
            let used: u32 = tx.query_row("SELECT COUNT(*) FROM conversation_model_calls WHERE task_id=?1", [task.to_string()], |row| row.get(0))?;
            if used >= maximum { return Err(invalid("task model-call allowance exhausted; no request sent")); }
            tx.execute("INSERT INTO conversation_model_calls(id,task_id,request_hash,status,created_at) VALUES(?1,?2,?3,'reserved',?4)", params![id.to_string(),task.to_string(),request_hash,Utc::now().to_rfc3339()])?;
            tx.commit()?; Ok(ConversationCallAdmission::Admitted)
        })
    }

    pub fn finish_conversation_call(
        &self,
        project: &str,
        task: Uuid,
        id: Uuid,
        status: ConversationCallStatus,
        evidence: serde_json::Value,
    ) -> Result<ConversationCallReceipt, StorageError> {
        if status == ConversationCallStatus::Reserved {
            return Err(invalid("cannot reset a model call reservation"));
        }
        self.with_connection(|db| {
            let tx = db.unchecked_transaction()?;
            owner(&tx, project, task)?;
            let mut saved =
                receipt(&tx, id)?.ok_or_else(|| invalid("model call reservation not found"))?;
            if saved.task_id != task {
                return Err(invalid("model call belongs to another task"));
            }
            if saved.status != ConversationCallStatus::Reserved {
                if saved.status != status || saved.evidence.as_ref() != Some(&evidence) {
                    return Err(invalid("model call evidence is immutable"));
                }
                return Ok(saved);
            }
            let state = match status {
                ConversationCallStatus::Completed => "completed",
                ConversationCallStatus::Failed => "failed",
                ConversationCallStatus::InDoubt => "in_doubt",
                ConversationCallStatus::Reserved => unreachable!(),
            };
            tx.execute(
                "UPDATE conversation_model_calls SET status=?2,evidence_json=?3 WHERE id=?1",
                params![id.to_string(), state, serde_json::to_string(&evidence)?],
            )?;
            tx.commit()?;
            saved.status = status;
            saved.evidence = Some(evidence);
            Ok(saved)
        })
    }

    pub fn revoke_conversation_calls(&self, project: &str, task: Uuid) -> Result<(), StorageError> {
        self.with_connection(|db| {
            owner(db, project, task)?;
            db.execute(
                "UPDATE conversation_call_grants SET revoked=1 WHERE task_id=?1",
                [task.to_string()],
            )?;
            Ok(())
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{BeginConversationTask, ConversationMessageInput};
    use chrono::Duration;

    #[test]
    fn allowance_and_unknown_outcomes_survive_retries_revocation_and_restart() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("history.db");
        let store = SqliteStore::open(&path).unwrap();
        let project = Uuid::new_v4().to_string();
        let conversation = store.create_conversation(&project).unwrap();
        let message = ConversationMessageInput {
            id: Uuid::new_v4(),
            text: "TEST cups".into(),
            image: None,
        };
        store
            .append_conversation_message(&project, conversation, &message)
            .unwrap();
        let task = Uuid::new_v4();
        store
            .begin_conversation_task(
                &project,
                conversation,
                &BeginConversationTask {
                    id: task,
                    source_message_id: message.id,
                    schema_revision: "a".repeat(64),
                },
            )
            .unwrap();
        let scope = "b".repeat(64);
        let request = "c".repeat(64);
        let call = Uuid::new_v4();
        assert!(
            store
                .reserve_conversation_call(&project, task, call, &scope, &request)
                .is_err()
        );
        let grant = ConversationCallGrant {
            id: Uuid::new_v4(),
            task_id: task,
            scope_hash: scope.clone(),
            maximum_calls: 2,
            expires_at: Utc::now() + Duration::minutes(10),
        };
        let preempted = Uuid::new_v4();
        let cancellation = store
            .request_conversation_call_cancel(&project, task, preempted)
            .unwrap();
        assert_eq!(
            store
                .request_conversation_call_cancel(&project, task, preempted)
                .unwrap(),
            cancellation
        );
        assert!(
            store
                .request_conversation_call_cancel("foreign", task, preempted)
                .is_err()
        );
        store
            .authorize_conversation_calls(&project, &grant)
            .unwrap();
        assert!(
            store
                .reserve_conversation_call(&project, task, preempted, &scope, &request)
                .is_err()
        );
        assert!(
            store
                .conversation_call(&project, task, preempted)
                .unwrap()
                .is_none()
        );
        assert!(
            store
                .authorize_conversation_calls("foreign", &grant)
                .is_err()
        );
        assert!(
            store
                .reserve_conversation_call(&project, task, call, &"d".repeat(64), &request)
                .is_err()
        );
        assert_eq!(
            store
                .reserve_conversation_call(&project, task, call, &scope, &request)
                .unwrap(),
            ConversationCallAdmission::Admitted
        );
        assert!(matches!(
            store
                .reserve_conversation_call(&project, task, call, &scope, &request)
                .unwrap(),
            ConversationCallAdmission::Existing(ConversationCallReceipt {
                status: ConversationCallStatus::Reserved,
                ..
            })
        ));
        assert!(
            store
                .reserve_conversation_call(&project, task, call, &scope, &"e".repeat(64))
                .is_err()
        );
        drop(store);
        let store = SqliteStore::open(&path).unwrap();
        assert_eq!(
            store
                .conversation_call_cancellations(&project, task)
                .unwrap(),
            vec![cancellation]
        );
        assert!(matches!(
            store
                .reserve_conversation_call(&project, task, call, &scope, &request)
                .unwrap(),
            ConversationCallAdmission::Existing(_)
        ));
        let evidence = serde_json::json!({"remote_outcome":"unknown"});
        store
            .finish_conversation_call(
                &project,
                task,
                call,
                ConversationCallStatus::InDoubt,
                evidence.clone(),
            )
            .unwrap();
        store
            .finish_conversation_call(
                &project,
                task,
                call,
                ConversationCallStatus::InDoubt,
                evidence,
            )
            .unwrap();
        assert!(
            store
                .finish_conversation_call(
                    &project,
                    task,
                    call,
                    ConversationCallStatus::Completed,
                    serde_json::json!({})
                )
                .is_err()
        );
        let second = Uuid::new_v4();
        assert_eq!(
            store
                .reserve_conversation_call(&project, task, second, &scope, &request)
                .unwrap(),
            ConversationCallAdmission::Admitted
        );
        store
            .authorize_conversation_calls(&project, &grant)
            .unwrap();
        assert!(
            store
                .reserve_conversation_call(&project, task, Uuid::new_v4(), &scope, &request)
                .is_err()
        );
        let mut expanded = grant.clone();
        expanded.maximum_calls = 128;
        assert!(
            store
                .authorize_conversation_calls(&project, &expanded)
                .is_err()
        );
        store.revoke_conversation_calls(&project, task).unwrap();
        store
            .authorize_conversation_calls(&project, &grant)
            .unwrap();
        assert!(
            store
                .reserve_conversation_call(&project, task, Uuid::new_v4(), &scope, &request)
                .is_err()
        );
        assert!(store.conversation_call("foreign", task, call).is_err());
        assert_eq!(
            store
                .conversation_call(&project, task, call)
                .unwrap()
                .unwrap()
                .status,
            ConversationCallStatus::InDoubt
        );
    }
}
