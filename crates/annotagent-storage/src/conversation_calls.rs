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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConversationCallBudget {
    pub current_grant: ConversationCallGrant,
    pub used_calls: u32,
    pub revoked: bool,
}

/// One task's ledger view over existing planning grants and explicitly confirmed Batch allocations.
/// The disjoint phase caps remain enforced by their original admission transactions.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConversationTaskBudget {
    pub planning_authorized_calls: u64,
    pub planning_reserved_calls: u64,
    pub processing_authorized_calls: u64,
    pub processing_reserved_calls: u64,
    pub total_authorized_calls: u64,
    pub total_reserved_calls: u64,
    /// Conversation-owned planning and Batch allocations across this stable Project.
    /// These are cumulative ledger totals, not an additional spending authorization.
    pub project_authorized_calls: u64,
    pub project_reserved_calls: u64,
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
    pub fn conversation_task_budget(
        &self,
        project: &str,
        task: Uuid,
    ) -> Result<ConversationTaskBudget, StorageError> {
        self.with_connection(|db| {
            owner(db,project,task)?;
            let planning_authorized_calls:i64=db.query_row("SELECT COALESCE(MAX(maximum_calls),0) FROM conversation_call_grants WHERE task_id=?1",[task.to_string()],|row|row.get(0))?;
            let planning_reserved_calls:i64=db.query_row("SELECT COUNT(*) FROM conversation_model_calls WHERE task_id=?1",[task.to_string()],|row|row.get(0))?;
            // Batch allocation and allowance are created together. Join by the durable operation
            // id, not a late `started` receipt, so a crash after Batch creation cannot hide spend.
            let (processing_authorized_calls,processing_reserved_calls):(i64,i64)=db.query_row("SELECT COALESCE(SUM(a.maximum),0),COALESCE(SUM(a.reserved),0) FROM batch_model_call_allowances a JOIN processing_operations p ON p.id=a.batch_id WHERE json_extract(p.state_json,'$.authorization.conversation.task_id')=?1",[task.to_string()],|row|Ok((row.get(0)?,row.get(1)?)))?;
            let count=|value:i64|u64::try_from(value).map_err(|_|invalid("invalid negative task budget"));
            let planning_authorized_calls=count(planning_authorized_calls)?;
            let planning_reserved_calls=count(planning_reserved_calls)?;
            let processing_authorized_calls=count(processing_authorized_calls)?;
            let processing_reserved_calls=count(processing_reserved_calls)?;
            // Aggregate each task/Batch once. Joining grants directly to calls would multiply
            // the allocation by the number of receipts and could hide cross-task overspend.
            let (project_authorized_calls,project_reserved_calls):(i64,i64)=db.query_row(
                "WITH owned_tasks AS (
                    SELECT t.id FROM conversation_tasks t JOIN project_conversations c ON c.id=t.conversation_id WHERE c.project_id=?1
                ), allocations AS (
                    SELECT COALESCE(SUM(g.maximum_calls),0) AS authorized,
                           (SELECT COUNT(*) FROM conversation_model_calls m JOIN owned_tasks t ON t.id=m.task_id) AS reserved
                    FROM conversation_call_grants g JOIN owned_tasks t ON t.id=g.task_id
                    UNION ALL
                    SELECT COALESCE(SUM(a.maximum),0),COALESCE(SUM(a.reserved),0)
                    FROM batch_model_call_allowances a JOIN processing_operations p ON p.id=a.batch_id
                    JOIN owned_tasks t ON t.id=json_extract(p.state_json,'$.authorization.conversation.task_id')
                ) SELECT SUM(authorized),SUM(reserved) FROM allocations",
                [project],|row|Ok((row.get(0)?,row.get(1)?)))?;
            Ok(ConversationTaskBudget {
                planning_authorized_calls,planning_reserved_calls,processing_authorized_calls,processing_reserved_calls,
                total_authorized_calls:planning_authorized_calls.checked_add(processing_authorized_calls).ok_or_else(||invalid("task budget overflow"))?,
                total_reserved_calls:planning_reserved_calls.checked_add(processing_reserved_calls).ok_or_else(||invalid("task usage overflow"))?,
                project_authorized_calls:count(project_authorized_calls)?,
                project_reserved_calls:count(project_reserved_calls)?,
            })
        })
    }
    pub fn conversation_authorization(
        &self,
        project: &str,
        task: Uuid,
        id: Uuid,
    ) -> Result<ConversationCallGrant, StorageError> {
        self.with_connection(|db| {
            owner(db,project,task)?;
            let (scope_hash,maximum_calls,expiry):(String,u32,String)=db.query_row("SELECT scope_hash,maximum_calls,expires_at FROM conversation_authorization_revisions WHERE id=?1 AND task_id=?2",params![id.to_string(),task.to_string()],|row|Ok((row.get(0)?,row.get(1)?,row.get(2)?)))?;
            Ok(ConversationCallGrant {id,task_id:task,scope_hash,maximum_calls,expires_at:DateTime::parse_from_rfc3339(&expiry).map_err(|_|invalid("Invalid grant expiry"))?.with_timezone(&Utc)})
        })
    }
    pub fn conversation_call_budget(
        &self,
        project: &str,
        task: Uuid,
    ) -> Result<Option<ConversationCallBudget>, StorageError> {
        self.with_connection(|db| {
            owner(db, project, task)?;
            let row: Option<(String,String,u32,String,bool)> = db.query_row("SELECT id,scope_hash,maximum_calls,expires_at,revoked FROM conversation_call_grants WHERE task_id=?1", [task.to_string()], |row| Ok((row.get(0)?,row.get(1)?,row.get(2)?,row.get(3)?,row.get(4)?))).optional()?;
            row.map(|(id,scope_hash,maximum_calls,expires,revoked)| {
                let used_calls = db.query_row("SELECT COUNT(*) FROM conversation_model_calls WHERE task_id=?1", [task.to_string()], |row| row.get(0))?;
                Ok(ConversationCallBudget { current_grant: ConversationCallGrant { id:Uuid::parse_str(&id).map_err(|_|invalid("invalid grant ID"))?,task_id:task,scope_hash,maximum_calls,expires_at:DateTime::parse_from_rfc3339(&expires).map_err(|_|invalid("invalid grant expiry"))?.with_timezone(&Utc) }, used_calls,revoked })
            }).transpose()
        })
    }

    /// Explicit next-phase consent increases a cumulative ceiling; spent/unknown calls remain charged.
    pub fn advance_conversation_authorization(
        &self,
        project: &str,
        previous: Uuid,
        grant: &ConversationCallGrant,
    ) -> Result<(), StorageError> {
        if !digest(&grant.scope_hash)
            || !(1..=128).contains(&grant.maximum_calls)
            || grant.id == previous
        {
            return Err(invalid("invalid next-phase authorization"));
        }
        self.with_connection(|db| {
            let tx = db.unchecked_transaction()?; owner(&tx, project, grant.task_id)?;
            let saved: Option<(String,Option<String>,String,u32,String)> = tx.query_row("SELECT task_id,previous_id,scope_hash,maximum_calls,expires_at FROM conversation_authorization_revisions WHERE id=?1", [grant.id.to_string()], |row| Ok((row.get(0)?,row.get(1)?,row.get(2)?,row.get(3)?,row.get(4)?))).optional()?;
            if let Some((task,base,scope,maximum,expiry)) = saved {
                if task != grant.task_id.to_string() || base != Some(previous.to_string()) || scope != grant.scope_hash || maximum != grant.maximum_calls || expiry != grant.expires_at.to_rfc3339() { return Err(invalid("authorization revision retry conflicts")); }
                return Ok(());
            }
            let current: Option<(String,u32,bool)> = tx.query_row("SELECT id,maximum_calls,revoked FROM conversation_call_grants WHERE task_id=?1", [grant.task_id.to_string()], |row| Ok((row.get(0)?,row.get(1)?,row.get(2)?))).optional()?;
            let Some((id,maximum,revoked)) = current else { return Err(invalid("initial task authorization required")); };
            if id != previous.to_string() || revoked || grant.maximum_calls < maximum || grant.expires_at <= Utc::now() {
                return Err(invalid("authorization changed, revoked, expired or lowers its cumulative ceiling"));
            }
            let active: bool = tx.query_row("SELECT EXISTS(SELECT 1 FROM conversation_model_calls WHERE task_id=?1 AND status='reserved')",[grant.task_id.to_string()],|row|row.get(0))?;
            if active { return Err(invalid("settle active calls before changing authorization scope")); }
            tx.execute("INSERT INTO conversation_authorization_revisions(id,task_id,previous_id,scope_hash,maximum_calls,expires_at) VALUES(?1,?2,?3,?4,?5,?6)",params![grant.id.to_string(),grant.task_id.to_string(),previous.to_string(),grant.scope_hash,grant.maximum_calls,grant.expires_at.to_rfc3339()])?;
            tx.execute("UPDATE conversation_call_grants SET id=?2,scope_hash=?3,maximum_calls=?4,expires_at=?5 WHERE task_id=?1",params![grant.task_id.to_string(),grant.id.to_string(),grant.scope_hash,grant.maximum_calls,grant.expires_at.to_rfc3339()])?;
            tx.commit()?; Ok(())
        })
    }
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
            let foreign_builder: bool = tx.query_row("SELECT EXISTS(SELECT 1 FROM conversation_builder_operations WHERE id=?1 AND task_id!=?2)",params![call.to_string(),task.to_string()],|row|row.get(0))?;
            if foreign_builder { return Err(invalid("Builder belongs to another task")); }
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
            let historical: Option<(String,String,u32,String)> = tx.query_row("SELECT task_id,scope_hash,maximum_calls,expires_at FROM conversation_authorization_revisions WHERE id=?1 AND previous_id IS NULL", [grant.id.to_string()], |row| Ok((row.get(0)?,row.get(1)?,row.get(2)?,row.get(3)?))).optional()?;
            if let Some((task,scope,maximum,expires)) = historical {
                if task != grant.task_id.to_string() || scope != grant.scope_hash || maximum != grant.maximum_calls || expires != grant.expires_at.to_rfc3339() { return Err(invalid("initial authorization retry conflicts")); }
                return Ok(());
            }
            let existing: Option<(String,String,u32,String)> = tx.query_row("SELECT id,scope_hash,maximum_calls,expires_at FROM conversation_call_grants WHERE task_id=?1", [grant.task_id.to_string()], |row| Ok((row.get(0)?,row.get(1)?,row.get(2)?,row.get(3)?))).optional()?;
            if let Some((id,scope,maximum,expires)) = existing {
                if id != grant.id.to_string() || scope != grant.scope_hash || maximum != grant.maximum_calls || expires != grant.expires_at.to_rfc3339() { return Err(invalid("task authorization already exists; retries cannot change its scope or reset its budget")); }
                return Ok(());
            }
            if grant.expires_at <= Utc::now() { return Err(invalid("task authorization expired")); }
            tx.execute("INSERT INTO conversation_call_grants(task_id,id,scope_hash,maximum_calls,expires_at) VALUES(?1,?2,?3,?4,?5)", params![grant.task_id.to_string(),grant.id.to_string(),grant.scope_hash,grant.maximum_calls,grant.expires_at.to_rfc3339()])?;
            tx.execute("INSERT INTO conversation_authorization_revisions(id,task_id,scope_hash,maximum_calls,expires_at) VALUES(?1,?2,?3,?4,?5)",params![grant.id.to_string(),grant.task_id.to_string(),grant.scope_hash,grant.maximum_calls,grant.expires_at.to_rfc3339()])?;
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
                let original_scope: String = tx.query_row("SELECT g.scope_hash FROM conversation_call_authorizations a JOIN conversation_authorization_revisions g ON g.id=a.grant_id WHERE a.call_id=?1", [id.to_string()], |row| row.get(0))?;
                if original_scope != scope_hash { return Err(invalid("call scope changed")); }
                return Ok(ConversationCallAdmission::Existing(saved));
            }
            let waiting: bool = tx.query_row("SELECT EXISTS(SELECT 1 FROM conversation_human_requests WHERE task_id=?1 AND status='pending')", [task.to_string()], |row| row.get(0))?;
            if waiting { return Err(invalid("Task is waiting for human input; no model call was admitted")); }
            let cancelled: bool = tx.query_row("SELECT EXISTS(SELECT 1 FROM conversation_call_cancellations WHERE call_id=?1)", [id.to_string()], |row| row.get(0))?;
            if cancelled { return Err(invalid("call cancelled before admission; no request sent")); }
            let grant: Option<(String,u32,String,bool)> = tx.query_row("SELECT scope_hash,maximum_calls,expires_at,revoked FROM conversation_call_grants WHERE task_id=?1", [task.to_string()], |row| Ok((row.get(0)?,row.get(1)?,row.get(2)?,row.get(3)?))).optional()?;
            let Some((scope,maximum,expires,revoked)) = grant else { return Err(invalid("explicit task authorization required")); };
            if scope != scope_hash || revoked || DateTime::parse_from_rfc3339(&expires).map_err(|_| invalid("invalid authorization expiry"))? <= Utc::now() { return Err(invalid("task authorization changed, expired or was revoked")); }
            let used: u32 = tx.query_row("SELECT COUNT(*) FROM conversation_model_calls WHERE task_id=?1", [task.to_string()], |row| row.get(0))?;
            if used >= maximum { return Err(invalid("task model-call allowance exhausted; no request sent")); }
            crate::conversation_project_budget::admit(&tx, project)?;
            tx.execute("INSERT INTO conversation_model_calls(id,task_id,request_hash,status,created_at) VALUES(?1,?2,?3,'reserved',?4)", params![id.to_string(),task.to_string(),request_hash,Utc::now().to_rfc3339()])?;
            tx.execute("INSERT INTO conversation_call_authorizations(call_id,grant_id) SELECT ?1,id FROM conversation_call_grants WHERE task_id=?2", params![id.to_string(),task.to_string()])?;
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
    fn next_phase_keeps_spend_and_original_receipts_without_scope_or_retry_reset() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("TEST-phases.db");
        let store = SqliteStore::open(&path).unwrap();
        let project_id = Uuid::new_v4().to_string();
        let project = project_id.as_str();
        let conversation = store.create_conversation(project).unwrap();
        let message = ConversationMessageInput {
            reference: None,
            id: Uuid::new_v4(),
            text: "TEST cups".into(),
            image: None,
        };
        store
            .append_conversation_message(project, conversation, &message)
            .unwrap();
        let task = Uuid::new_v4();
        store
            .begin_conversation_task(
                project,
                conversation,
                &BeginConversationTask {
                    id: task,
                    source_message_id: message.id,
                    schema_revision: "a".repeat(64),
                },
            )
            .unwrap();
        let first = ConversationCallGrant {
            id: Uuid::new_v4(),
            task_id: task,
            scope_hash: "a".repeat(64),
            maximum_calls: 1,
            expires_at: Utc::now() + Duration::minutes(10),
        };
        store.authorize_conversation_calls(project, &first).unwrap();
        let schema_call = Uuid::new_v4();
        store
            .reserve_conversation_call(
                project,
                task,
                schema_call,
                &first.scope_hash,
                &"b".repeat(64),
            )
            .unwrap();
        let next = ConversationCallGrant {
            id: Uuid::new_v4(),
            scope_hash: "c".repeat(64),
            maximum_calls: 3,
            ..first.clone()
        };
        assert!(
            store
                .advance_conversation_authorization(project, first.id, &next)
                .is_err()
        );
        store
            .finish_conversation_call(
                project,
                task,
                schema_call,
                ConversationCallStatus::InDoubt,
                serde_json::json!({"TEST":"unknown not refunded"}),
            )
            .unwrap();
        assert!(
            store
                .advance_conversation_authorization("foreign", first.id, &next)
                .is_err()
        );
        store
            .advance_conversation_authorization(project, first.id, &next)
            .unwrap();
        store
            .advance_conversation_authorization(project, first.id, &next)
            .unwrap();
        store.authorize_conversation_calls(project, &first).unwrap();
        let budget = store
            .conversation_call_budget(project, task)
            .unwrap()
            .unwrap();
        assert_eq!(budget.current_grant, next);
        assert_eq!(budget.used_calls, 1);
        assert!(matches!(
            store
                .reserve_conversation_call(
                    project,
                    task,
                    schema_call,
                    &first.scope_hash,
                    &"b".repeat(64)
                )
                .unwrap(),
            ConversationCallAdmission::Existing(_)
        ));
        assert!(
            store
                .reserve_conversation_call(
                    project,
                    task,
                    schema_call,
                    &next.scope_hash,
                    &"b".repeat(64)
                )
                .is_err()
        );
        assert!(
            store
                .reserve_conversation_call(
                    project,
                    task,
                    Uuid::new_v4(),
                    &first.scope_hash,
                    &"b".repeat(64)
                )
                .is_err()
        );
        for _ in 0..2 {
            let id = Uuid::new_v4();
            assert_eq!(
                store
                    .reserve_conversation_call(project, task, id, &next.scope_hash, &"d".repeat(64))
                    .unwrap(),
                ConversationCallAdmission::Admitted
            );
            store
                .finish_conversation_call(
                    project,
                    task,
                    id,
                    ConversationCallStatus::Failed,
                    serde_json::json!({"TEST":"failed not refunded"}),
                )
                .unwrap();
        }
        assert!(
            store
                .reserve_conversation_call(
                    project,
                    task,
                    Uuid::new_v4(),
                    &next.scope_hash,
                    &"d".repeat(64)
                )
                .is_err()
        );
        drop(store);
        let store = SqliteStore::open(path).unwrap();
        let budget = store
            .conversation_call_budget(project, task)
            .unwrap()
            .unwrap();
        assert_eq!(budget.used_calls, 3);
        assert_eq!(budget.current_grant, next);
        let batch = annotagent_core::BatchId::new();
        let processing = serde_json::json!({"phase":"published","authorization":{"conversation":{"conversation_id":conversation,"task_id":task}}});
        store
            .reserve_processing_operation(
                &batch.to_string(),
                "TEST-project-slug",
                &serde_json::json!({"request":"TEST"}),
                &processing,
            )
            .unwrap();
        store.with_connection(|db| { db.execute("INSERT INTO batch_model_call_allowances(batch_id,maximum,reserved) VALUES(?1,2,0)",[batch.to_string()])?; Ok(()) }).unwrap();
        store.reserve_batch_model_call(batch).unwrap();
        store.reserve_batch_model_call(batch).unwrap();
        assert!(store.reserve_batch_model_call(batch).is_err());
        let combined = store.conversation_task_budget(project, task).unwrap();
        assert_eq!(combined.planning_reserved_calls, 3);
        assert_eq!(combined.processing_reserved_calls, 2);
        assert_eq!(combined.total_reserved_calls, 5);
        assert_eq!(combined.total_authorized_calls, 5);
        assert_eq!(combined.project_authorized_calls, 5);
        assert_eq!(combined.project_reserved_calls, 5);
        // The Batch exists even though the operation has not settled its `started` receipt.
        let reopened = SqliteStore::open(temp.path().join("TEST-phases.db")).unwrap();
        assert_eq!(
            reopened.conversation_task_budget(project, task).unwrap(),
            combined
        );
        assert!(reopened.conversation_task_budget("foreign", task).is_err());
        assert!(reopened.reserve_batch_model_call(batch).is_err());
        // A new conversation/goal has no task spend, but cannot erase Project history.
        let second_conversation = reopened.create_conversation(project).unwrap();
        let second_message = ConversationMessageInput {
            id: Uuid::new_v4(),
            text: "TEST new goal".into(),
            image: None,
            reference: None,
        };
        reopened
            .append_conversation_message(project, second_conversation, &second_message)
            .unwrap();
        let second_task = Uuid::new_v4();
        reopened
            .begin_conversation_task(
                project,
                second_conversation,
                &BeginConversationTask {
                    id: second_task,
                    source_message_id: second_message.id,
                    schema_revision: "a".repeat(64),
                },
            )
            .unwrap();
        let fresh = reopened
            .conversation_task_budget(project, second_task)
            .unwrap();
        assert_eq!(fresh.total_reserved_calls, 0);
        assert_eq!(fresh.project_reserved_calls, 5);
        let second_grant = ConversationCallGrant {
            id: Uuid::new_v4(),
            task_id: second_task,
            scope_hash: "a".repeat(64),
            maximum_calls: 2,
            expires_at: Utc::now() + Duration::minutes(10),
        };
        reopened
            .authorize_conversation_calls(project, &second_grant)
            .unwrap();
        reopened
            .reserve_conversation_call(
                project,
                second_task,
                Uuid::new_v4(),
                &second_grant.scope_hash,
                &"c".repeat(64),
            )
            .unwrap();
        let cross_task = reopened.conversation_task_budget(project, task).unwrap();
        assert_eq!(cross_task.total_reserved_calls, 5);
        assert_eq!(cross_task.project_authorized_calls, 7);
        assert_eq!(cross_task.project_reserved_calls, 6);
        let other_project = Uuid::new_v4().to_string();
        let other_conversation = reopened.create_conversation(&other_project).unwrap();
        let other_message = ConversationMessageInput {
            id: Uuid::new_v4(),
            ..second_message
        };
        reopened
            .append_conversation_message(&other_project, other_conversation, &other_message)
            .unwrap();
        let other_task = Uuid::new_v4();
        reopened
            .begin_conversation_task(
                &other_project,
                other_conversation,
                &BeginConversationTask {
                    id: other_task,
                    source_message_id: other_message.id,
                    schema_revision: "a".repeat(64),
                },
            )
            .unwrap();
        assert_eq!(
            reopened
                .conversation_task_budget(&other_project, other_task)
                .unwrap()
                .project_reserved_calls,
            0
        );
        assert!(matches!(
            store
                .reserve_conversation_call(
                    project,
                    task,
                    schema_call,
                    &first.scope_hash,
                    &"b".repeat(64)
                )
                .unwrap(),
            ConversationCallAdmission::Existing(_)
        ));
        assert!(
            store
                .advance_conversation_authorization(
                    project,
                    first.id,
                    &ConversationCallGrant {
                        id: Uuid::new_v4(),
                        maximum_calls: 4,
                        ..next.clone()
                    }
                )
                .is_err()
        );
        store.revoke_conversation_calls(project, task).unwrap();
        store
            .advance_conversation_authorization(project, first.id, &next)
            .unwrap();
        assert!(
            store
                .conversation_call_budget(project, task)
                .unwrap()
                .unwrap()
                .revoked
        );
        assert!(
            store
                .advance_conversation_authorization(
                    project,
                    next.id,
                    &ConversationCallGrant {
                        id: Uuid::new_v4(),
                        maximum_calls: 4,
                        ..next
                    }
                )
                .is_err()
        );
    }

    #[test]
    fn allowance_and_unknown_outcomes_survive_retries_revocation_and_restart() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("history.db");
        let store = SqliteStore::open(&path).unwrap();
        let project = Uuid::new_v4().to_string();
        let conversation = store.create_conversation(&project).unwrap();
        let message = ConversationMessageInput {
            reference: None,
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
