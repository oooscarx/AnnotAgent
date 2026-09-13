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
#[serde(deny_unknown_fields)]
pub struct ConversationSchemaAuthorization {
    pub call_id: Uuid,
    pub model_id: annotagent_core::ModelProfileId,
    pub scope_hash: String,
    pub expires_at: DateTime<Utc>,
    pub allow_unknown_cost: bool,
}

/// One explicitly re-authorized successor of a settled invalid Schema response.
/// The cumulative allowance is exact and the original receipt remains immutable.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ConversationSchemaRetryAuthorization {
    pub call_id: Uuid,
    pub retry_of: Uuid,
    pub model_id: annotagent_core::ModelProfileId,
    pub previous_grant_id: Uuid,
    pub scope_hash: String,
    pub maximum_calls: u32,
    pub expires_at: DateTime<Utc>,
    pub allow_unknown_cost: bool,
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
    #[serde(default)]
    pub started_at: Option<String>,
    #[serde(default)]
    pub completed_at: Option<String>,
    #[serde(default)]
    pub duration_ms: Option<i64>,
    #[serde(default)]
    pub stage: Option<String>,
    #[serde(default)]
    pub failure: Option<annotagent_core::ModelFailure>,
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
pub(crate) fn request_cancel_in(
    db: &rusqlite::Connection,
    project: &str,
    task: Uuid,
    call: Uuid,
) -> Result<ConversationCallCancellation, StorageError> {
    owner(db, project, task)?;
    if receipt(db, call)?.is_some_and(|saved| saved.task_id != task) {
        return Err(invalid("call belongs to another task"));
    }
    let foreign:bool=db.query_row("SELECT EXISTS(SELECT 1 FROM conversation_builder_operations WHERE id=?1 AND task_id!=?2 UNION ALL SELECT 1 FROM conversation_authorization_revisions WHERE id=?1 AND task_id!=?2 UNION ALL SELECT 1 FROM conversation_feedback_authorizations WHERE call_id=?1 AND task_id!=?2 UNION ALL SELECT 1 FROM conversation_future_schema_proposal_authorizations WHERE call_id=?1 AND task_id!=?2)",params![call.to_string(),task.to_string()],|r|r.get(0))?;
    if foreign {
        return Err(invalid(
            "operation or authorization belongs to another task",
        ));
    }
    let existing: Option<(String, String)> = db
        .query_row(
            "SELECT task_id,requested_at FROM conversation_call_cancellations WHERE call_id=?1",
            [call.to_string()],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional()?;
    if let Some((owner, requested_at)) = existing {
        if owner != task.to_string() {
            return Err(invalid("cancellation belongs to another task"));
        }
        return Ok(ConversationCallCancellation {
            call_id: call,
            task_id: task,
            requested_at,
        });
    }
    let requested_at = Utc::now().to_rfc3339();
    db.execute("INSERT INTO conversation_call_cancellations(call_id,task_id,requested_at) VALUES(?1,?2,?3)",params![call.to_string(),task.to_string(),requested_at])?;
    Ok(ConversationCallCancellation {
        call_id: call,
        task_id: task,
        requested_at,
    })
}
pub(crate) fn receipt(
    db: &rusqlite::Connection,
    id: Uuid,
) -> Result<Option<ConversationCallReceipt>, StorageError> {
    let row: Option<(String,String,String,Option<String>)> = db.query_row("SELECT task_id,request_hash,status,evidence_json FROM conversation_model_calls WHERE id=?1", [id.to_string()], |row| Ok((row.get(0)?,row.get(1)?,row.get(2)?,row.get(3)?))).optional()?;
    row.map(|(task, request_hash, status, evidence)| {
        let started: String = db.query_row("SELECT created_at FROM conversation_model_calls WHERE id=?1", [id.to_string()], |r| r.get(0))?;
        let progress: Option<(String,Option<String>,Option<String>)> = db.query_row("SELECT stage,completed_at,failure_json FROM conversation_call_progress WHERE call_id=?1", [id.to_string()], |r| Ok((r.get(0)?,r.get(1)?,r.get(2)?))).optional()?;
        let (stage, completed_at, failure) = progress.map_or((None,None,None), |(s,c,f)| (Some(s),c,f));
        let duration_ms = completed_at.as_ref().and_then(|end| Some((DateTime::parse_from_rfc3339(end).ok()? - DateTime::parse_from_rfc3339(&started).ok()?).num_milliseconds().max(0)));
        Ok(ConversationCallReceipt {
            started_at: Some(started), completed_at, duration_ms, stage,
            failure: failure.map(|f| serde_json::from_str(&f)).transpose()?,
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

fn settle_interrupted(
    db: &rusqlite::Connection,
    target: Option<(Uuid, Uuid)>,
    stage: annotagent_core::ModelFailureStage,
) -> Result<(), StorageError> {
    let tx = db.unchecked_transaction()?;
    let failure = annotagent_core::ModelFailure {
        stage,
        category: annotagent_core::ModelFailureCategory::Interrupted,
        http_status: None,
    };
    let evidence = serde_json::json!({"error":"The local handler or server stopped before saving a complete response. Remote outcome and cost are unknown; no automatic retry was started.","failure":failure});
    let task = target.map(|(t, _)| t.to_string());
    let call = target.map(|(_, c)| c.to_string());
    tx.execute("INSERT INTO conversation_call_progress(call_id,stage,completed_at,failure_json) SELECT id,'settled',?3,?4 FROM conversation_model_calls WHERE status='reserved' AND (?1 IS NULL OR (task_id=?1 AND id=?2)) ON CONFLICT(call_id) DO UPDATE SET stage='settled',completed_at=excluded.completed_at,failure_json=excluded.failure_json",params![task,call,Utc::now().to_rfc3339(),serde_json::to_string(&failure)?])?;
    tx.execute("UPDATE conversation_model_calls SET status='in_doubt',evidence_json=?3 WHERE status='reserved' AND (?1 IS NULL OR (task_id=?1 AND id=?2))",params![task,call,serde_json::to_string(&evidence)?])?;
    tx.commit()?;
    Ok(())
}

pub(crate) fn require_call_admission_clear(
    db: &rusqlite::Connection,
    task: Uuid,
    call: Uuid,
) -> Result<(), StorageError> {
    let waiting: bool = db.query_row("SELECT EXISTS(SELECT 1 FROM conversation_human_requests WHERE task_id=?1 AND status='pending' UNION ALL SELECT 1 FROM conversation_image_class_reviews WHERE task_id=?1 AND status='pending')", [task.to_string()], |row| row.get(0))?;
    if waiting {
        return Err(StorageError::ConversationContract {
            code: "human_input_pending",
            message: "Task is waiting for human input; no model call was admitted".into(),
        });
    }
    let clarification:bool=db.query_row("SELECT EXISTS(SELECT 1 FROM conversation_model_calls m WHERE m.task_id=?1 AND m.status='completed' AND json_extract(m.evidence_json,'$.decision.Ok.decision')='clarify' AND NOT EXISTS(SELECT 1 FROM conversation_schema_clarification_answers a WHERE a.call_id=m.id))",[task.to_string()],|row|row.get(0))?;
    if clarification {
        return Err(StorageError::ConversationContract {
            code: "schema_clarification_pending",
            message:
                "Task is waiting for its Schema clarification answer; no model call was admitted"
                    .into(),
        });
    }
    let cancelled: bool = db.query_row(
        "SELECT EXISTS(SELECT 1 FROM conversation_call_cancellations WHERE call_id=?1)",
        [call.to_string()],
        |row| row.get(0),
    )?;
    if cancelled {
        return Err(invalid("call cancelled before admission; no request sent"));
    }
    Ok(())
}

impl SqliteStore {
    pub fn authorize_conversation_schema_retry(
        &self,
        project: &str,
        task: Uuid,
        input: &ConversationSchemaRetryAuthorization,
    ) -> Result<(), StorageError> {
        if input.call_id.is_nil()
            || input.retry_of.is_nil()
            || input.call_id == input.retry_of
            || !input.allow_unknown_cost
            || !digest(&input.scope_hash)
            || !(2..=128).contains(&input.maximum_calls)
        {
            return Err(invalid("invalid Schema retry authorization"));
        }
        let grant = ConversationCallGrant {
            id: input.call_id,
            task_id: task,
            scope_hash: input.scope_hash.clone(),
            maximum_calls: input.maximum_calls,
            expires_at: input.expires_at,
        };
        self.advance_conversation_authorization_with(
            project,
            input.previous_grant_id,
            &grant,
            |tx, _created| {
                let source = receipt(tx, input.retry_of)?
                    .ok_or_else(|| invalid("Schema retry source receipt not found"))?;
                if source.task_id != task
                    || source.status != ConversationCallStatus::Completed
                    || source
                        .evidence
                        .as_ref()
                        .and_then(|value| value.get("decision"))
                        .and_then(|value| value.get("Err"))
                        .is_none()
                    || source
                        .evidence
                        .as_ref()
                        .and_then(|value| value.pointer("/diagnostic/failure_code"))
                        .and_then(serde_json::Value::as_str)
                        == Some("provider_filtered")
                {
                    return Err(invalid(
                        "Schema retry source is not a settled retryable structured-output failure",
                    ));
                }
                let cancelled: bool = tx.query_row(
                    "SELECT EXISTS(SELECT 1 FROM conversation_call_cancellations WHERE call_id=?1)",
                    [input.retry_of.to_string()],
                    |row| row.get(0),
                )?;
                if cancelled {
                    return Err(invalid("Cancelled Schema calls cannot be retry sources"));
                }
                let saved: Option<(String, String, String)> = tx
                    .query_row(
                        "SELECT task_id,retry_of,input_json FROM conversation_schema_retries WHERE call_id=?1",
                        [input.call_id.to_string()],
                        |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
                    )
                    .optional()?;
                if let Some((saved_task, retry_of, value)) = saved {
                    if saved_task != task.to_string()
                        || retry_of != input.retry_of.to_string()
                        || serde_json::from_str::<ConversationSchemaRetryAuthorization>(&value)?
                            != *input
                    {
                        return Err(invalid("Schema retry command changed its saved scope"));
                    }
                    return Ok(());
                }
                let previous_maximum: u32 = tx.query_row(
                    "SELECT maximum_calls FROM conversation_authorization_revisions WHERE id=?1 AND task_id=?2",
                    params![input.previous_grant_id.to_string(), task.to_string()],
                    |row| row.get(0),
                )?;
                let used: u32 = tx.query_row(
                    "SELECT COUNT(*) FROM conversation_model_calls WHERE task_id=?1",
                    [task.to_string()],
                    |row| row.get(0),
                )?;
                let exact = previous_maximum.max(used.saturating_add(1));
                if input.maximum_calls != exact {
                    return Err(invalid(
                        "Schema retry must add exactly one cumulative call allowance",
                    ));
                }
                tx.execute(
                    "INSERT INTO conversation_schema_retries(call_id,task_id,retry_of,input_json,created_at) VALUES(?1,?2,?3,?4,?5)",
                    params![
                        input.call_id.to_string(),
                        task.to_string(),
                        input.retry_of.to_string(),
                        serde_json::to_string(input)?,
                        Utc::now().to_rfc3339()
                    ],
                )?;
                Ok(())
            },
        )
    }

    pub fn conversation_schema_retry(
        &self,
        project: &str,
        task: Uuid,
        call: Uuid,
    ) -> Result<Option<ConversationSchemaRetryAuthorization>, StorageError> {
        self.with_connection(|db| {
            owner(db, project, task)?;
            let saved: Option<(String, String)> = db
                .query_row(
                    "SELECT task_id,input_json FROM conversation_schema_retries WHERE call_id=?1",
                    [call.to_string()],
                    |row| Ok((row.get(0)?, row.get(1)?)),
                )
                .optional()?;
            saved
                .map(|(saved_task, value)| {
                    if saved_task != task.to_string() {
                        return Err(invalid("Schema retry belongs to another task"));
                    }
                    serde_json::from_str(&value).map_err(Into::into)
                })
                .transpose()
        })
    }

    /// Return the newest successor in the single retry chain, if any.
    pub fn latest_conversation_schema_retry(
        &self,
        project: &str,
        task: Uuid,
        source: Uuid,
    ) -> Result<Option<ConversationSchemaRetryAuthorization>, StorageError> {
        self.with_connection(|db| {
            owner(db, project, task)?;
            let value: Option<String> = db
                .query_row(
                    "WITH RECURSIVE chain(call_id,depth) AS (
                        SELECT call_id,1 FROM conversation_schema_retries WHERE task_id=?1 AND retry_of=?2
                        UNION ALL
                        SELECT r.call_id,c.depth+1 FROM conversation_schema_retries r JOIN chain c ON r.retry_of=c.call_id WHERE r.task_id=?1
                    ) SELECT r.input_json FROM chain c JOIN conversation_schema_retries r ON r.call_id=c.call_id ORDER BY c.depth DESC LIMIT 1",
                    params![task.to_string(), source.to_string()],
                    |row| row.get(0),
                )
                .optional()?;
            value
                .map(|value| serde_json::from_str(&value).map_err(Into::into))
                .transpose()
        })
    }

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
        self.advance_conversation_authorization_with(project, previous, grant, |_, _| Ok(()))
    }

    pub(crate) fn advance_conversation_authorization_with(
        &self,
        project: &str,
        previous: Uuid,
        grant: &ConversationCallGrant,
        after_write: impl FnOnce(&rusqlite::Transaction<'_>, bool) -> Result<(), StorageError>,
    ) -> Result<(), StorageError> {
        if !digest(&grant.scope_hash)
            || !(1..=128).contains(&grant.maximum_calls)
            || grant.id == previous
        {
            return Err(invalid("invalid next-phase authorization"));
        }
        self.with_connection(|db| {
            let tx = db.unchecked_transaction()?; owner(&tx, project, grant.task_id)?;
            crate::conversation_task_lifecycle::require_active_in(&tx, grant.task_id)?;
            let saved: Option<(String,Option<String>,String,u32,String)> = tx.query_row("SELECT task_id,previous_id,scope_hash,maximum_calls,expires_at FROM conversation_authorization_revisions WHERE id=?1", [grant.id.to_string()], |row| Ok((row.get(0)?,row.get(1)?,row.get(2)?,row.get(3)?,row.get(4)?))).optional()?;
            if let Some((task,base,scope,maximum,expiry)) = saved {
                if task != grant.task_id.to_string() || base != Some(previous.to_string()) || scope != grant.scope_hash || maximum != grant.maximum_calls || expiry != grant.expires_at.to_rfc3339() { return Err(invalid("authorization revision retry conflicts")); }
                after_write(&tx, false)?;
                tx.commit()?;
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
            after_write(&tx, true)?;
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
            let saved = request_cancel_in(&tx, project, task, call)?;
            tx.commit()?;
            Ok(saved)
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
            owner(db, project, task)?;
            settle_interrupted(
                db,
                Some((task, call)),
                annotagent_core::ModelFailureStage::Handler,
            )?;
            Ok(())
        })
    }
    /// Startup recovery never resends an indeterminate Provider request.
    pub fn recover_conversation_calls(&self) -> Result<(), StorageError> {
        self.with_connection(|db| {
            settle_interrupted(db, None, annotagent_core::ModelFailureStage::Recovery)?;
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
        self.authorize_initial_request(project, grant, None)
    }
    pub fn authorize_conversation_schema(
        &self,
        project: &str,
        task: Uuid,
        input: &ConversationSchemaAuthorization,
    ) -> Result<(), StorageError> {
        if !input.allow_unknown_cost {
            return Err(invalid("Explicit unknown-cost consent is required"));
        }
        let grant = ConversationCallGrant {
            id: input.call_id,
            task_id: task,
            scope_hash: input.scope_hash.clone(),
            maximum_calls: 1,
            expires_at: input.expires_at,
        };
        self.authorize_initial_request(project, &grant, Some(input))
    }
    pub fn pending_conversation_schema_authorization(
        &self,
        project: &str,
        task: Uuid,
    ) -> Result<Option<ConversationSchemaAuthorization>, StorageError> {
        self.with_connection(|db|{
            owner(db,project,task)?;
            let saved:Option<String>=db.query_row("SELECT a.input_json FROM conversation_schema_authorizations a JOIN conversation_call_grants g ON g.id=a.call_id WHERE a.task_id=?1 AND NOT EXISTS(SELECT 1 FROM conversation_model_calls m WHERE m.id=a.call_id) AND NOT EXISTS(SELECT 1 FROM conversation_call_cancellations c WHERE c.call_id=a.call_id)",[task.to_string()],|row|row.get(0)).optional()?;
            saved.map(|value|serde_json::from_str(&value).map_err(Into::into)).transpose()
        })
    }
    pub fn conversation_schema_authorization(
        &self,
        project: &str,
        task: Uuid,
        call: Uuid,
    ) -> Result<Option<ConversationSchemaAuthorization>, StorageError> {
        self.with_connection(|db| {
            owner(db, project, task)?;
            let saved: Option<(String, String)> = db
                .query_row(
                    "SELECT task_id,input_json FROM conversation_schema_authorizations WHERE call_id=?1",
                    [call.to_string()],
                    |row| Ok((row.get(0)?, row.get(1)?)),
                )
                .optional()?;
            saved
                .map(|(saved_task, value)| {
                    if saved_task != task.to_string() {
                        return Err(invalid("Schema authorization belongs to another task"));
                    }
                    serde_json::from_str(&value).map_err(Into::into)
                })
                .transpose()
        })
    }
    fn authorize_initial_request(
        &self,
        project: &str,
        grant: &ConversationCallGrant,
        request: Option<&ConversationSchemaAuthorization>,
    ) -> Result<(), StorageError> {
        self.authorize_initial_request_with(project, grant, request, |_, _| Ok(()))
    }

    pub(crate) fn authorize_initial_request_with(
        &self,
        project: &str,
        grant: &ConversationCallGrant,
        request: Option<&ConversationSchemaAuthorization>,
        after_write: impl FnOnce(&rusqlite::Transaction<'_>, bool) -> Result<(), StorageError>,
    ) -> Result<(), StorageError> {
        if !digest(&grant.scope_hash) || !(1..=128).contains(&grant.maximum_calls) {
            return Err(invalid("invalid bounded call authorization"));
        }
        self.with_connection(|db| {
            let tx = db.unchecked_transaction()?;
            owner(&tx, project, grant.task_id)?;
            crate::conversation_task_lifecycle::require_active_in(&tx, grant.task_id)?;
            let authorize = || -> Result<bool, StorageError> {
            let historical: Option<(String,String,u32,String)> = tx.query_row("SELECT task_id,scope_hash,maximum_calls,expires_at FROM conversation_authorization_revisions WHERE id=?1 AND previous_id IS NULL", [grant.id.to_string()], |row| Ok((row.get(0)?,row.get(1)?,row.get(2)?,row.get(3)?))).optional()?;
            if let Some((task,scope,maximum,expires)) = historical {
                if task != grant.task_id.to_string() || scope != grant.scope_hash || maximum != grant.maximum_calls || expires != grant.expires_at.to_rfc3339() { return Err(invalid("initial authorization retry conflicts")); }
                return Ok(false);
            }
            let existing: Option<(String,String,u32,String)> = tx.query_row("SELECT id,scope_hash,maximum_calls,expires_at FROM conversation_call_grants WHERE task_id=?1", [grant.task_id.to_string()], |row| Ok((row.get(0)?,row.get(1)?,row.get(2)?,row.get(3)?))).optional()?;
            if let Some((id,scope,maximum,expires)) = existing {
                if id != grant.id.to_string() || scope != grant.scope_hash || maximum != grant.maximum_calls || expires != grant.expires_at.to_rfc3339() { return Err(invalid("task authorization already exists; retries cannot change its scope or reset its budget")); }
                return Ok(false);
            }
            if grant.expires_at <= Utc::now() { return Err(invalid("task authorization expired")); }
            tx.execute("INSERT INTO conversation_call_grants(task_id,id,scope_hash,maximum_calls,expires_at) VALUES(?1,?2,?3,?4,?5)", params![grant.task_id.to_string(),grant.id.to_string(),grant.scope_hash,grant.maximum_calls,grant.expires_at.to_rfc3339()])?;
            tx.execute("INSERT INTO conversation_authorization_revisions(id,task_id,scope_hash,maximum_calls,expires_at) VALUES(?1,?2,?3,?4,?5)",params![grant.id.to_string(),grant.task_id.to_string(),grant.scope_hash,grant.maximum_calls,grant.expires_at.to_rfc3339()])?;
            Ok(true)
            };
            let created = authorize()?;
            if let Some(input)=request {
                let saved:Option<String>=tx.query_row("SELECT input_json FROM conversation_schema_authorizations WHERE call_id=?1",[input.call_id.to_string()],|row|row.get(0)).optional()?;
                if let Some(saved)=saved {
                    if serde_json::from_str::<ConversationSchemaAuthorization>(&saved)?!=*input{return Err(invalid("Schema authorization retry changed the original consent"));}
                }else{
                    tx.execute("INSERT INTO conversation_schema_authorizations(call_id,task_id,input_json) VALUES(?1,?2,?3)",params![input.call_id.to_string(),grant.task_id.to_string(),serde_json::to_string(input)?])?;
                }
            }
            after_write(&tx, created)?;
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
            crate::conversation_task_lifecycle::require_active_in(&tx, task)?;
            require_call_admission_clear(&tx, task, id)?;
            crate::conversation_queued_planning::require_call(&tx,project,task,id,request_hash)?;
            crate::conversation_future_schema_proposal::require_current_source_for_call(&tx,project,task,id)?;
            crate::conversation_stop::require_admission_clear(&tx,task,&id.to_string(),true)?;
            let grant: Option<(String,u32,String,bool)> = tx.query_row("SELECT scope_hash,maximum_calls,expires_at,revoked FROM conversation_call_grants WHERE task_id=?1", [task.to_string()], |row| Ok((row.get(0)?,row.get(1)?,row.get(2)?,row.get(3)?))).optional()?;
            let Some((scope,maximum,expires,revoked)) = grant else { return Err(invalid("explicit task authorization required")); };
            if scope != scope_hash || revoked || DateTime::parse_from_rfc3339(&expires).map_err(|_| invalid("invalid authorization expiry"))? <= Utc::now() { return Err(invalid("task authorization changed, expired or was revoked")); }
            let used: u32 = tx.query_row("SELECT COUNT(*) FROM conversation_model_calls WHERE task_id=?1", [task.to_string()], |row| row.get(0))?;
            if used >= maximum { return Err(invalid("task model-call allowance exhausted; no request sent")); }
            crate::conversation_project_budget::admit(&tx, project)?;
            tx.execute("INSERT INTO conversation_model_calls(id,task_id,request_hash,status,created_at) VALUES(?1,?2,?3,'reserved',?4)", params![id.to_string(),task.to_string(),request_hash,Utc::now().to_rfc3339()])?;
            tx.execute("INSERT INTO conversation_call_progress(call_id,stage) VALUES(?1,'reserved')", [id.to_string()])?;
            tx.execute("INSERT INTO conversation_call_authorizations(call_id,grant_id) SELECT ?1,id FROM conversation_call_grants WHERE task_id=?2", params![id.to_string(),task.to_string()])?;
            tx.commit()?; Ok(ConversationCallAdmission::Admitted)
        })
    }

    pub fn mark_conversation_call_stage(
        &self,
        project: &str,
        task: Uuid,
        id: Uuid,
        stage: &str,
    ) -> Result<(), StorageError> {
        let previous = match stage {
            "provider_request" => "reserved",
            "response_received" => "provider_request",
            _ => return Err(invalid("unsupported call stage")),
        };
        self.with_connection(|db| {
            owner(db,project,task)?;
            db.execute("UPDATE conversation_call_progress SET stage=?4 WHERE call_id=?1 AND stage=?3 AND EXISTS(SELECT 1 FROM conversation_model_calls WHERE id=?1 AND task_id=?2 AND status='reserved')",params![id.to_string(),task.to_string(),previous,stage])?;
            Ok(())
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
            let saved =
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
            tx.execute("INSERT INTO conversation_call_progress(call_id,stage,completed_at,failure_json) VALUES(?1,'settled',?2,?3) ON CONFLICT(call_id) DO UPDATE SET stage='settled',completed_at=excluded.completed_at,failure_json=excluded.failure_json",params![id.to_string(),Utc::now().to_rfc3339(),evidence.get("failure").filter(|v| !v.is_null()).map(serde_json::to_string).transpose()?])?;
            let mut result = receipt(&tx,id)?.ok_or_else(|| invalid("model call missing"))?;
            tx.commit()?;
            result.evidence = Some(evidence);
            Ok(result)
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
    fn schema_consent_survives_pre_admission_failure_without_a_new_grant_or_call() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("TEST-schema-consent.db");
        let store = SqliteStore::open(&path).unwrap();
        let project = Uuid::new_v4().to_string();
        let conversation = store.create_conversation(&project).unwrap();
        let message = ConversationMessageInput {
            id: Uuid::new_v4(),
            text: "TEST saved consent".into(),
            image: None,
            reference: None,
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
        let consent = ConversationSchemaAuthorization {
            call_id: Uuid::new_v4(),
            model_id: annotagent_core::ModelProfileId::new(),
            scope_hash: "a".repeat(64),
            expires_at: Utc::now() + Duration::minutes(10),
            allow_unknown_cost: true,
        };
        let limit = crate::ProjectCallLimitInput {
            id: Uuid::new_v4(),
            expected_revision: 0,
            maximum_calls: 0,
        };
        store
            .set_project_conversation_call_limit(&project, &limit)
            .unwrap();
        // A failed envelope write cannot leave a stranded authorization behind.
        store.with_connection(|db|{db.execute_batch("CREATE TRIGGER fail_schema_consent BEFORE INSERT ON conversation_schema_authorizations BEGIN SELECT RAISE(ABORT,'TEST atomic consent'); END;")?;Ok(())}).unwrap();
        assert!(
            store
                .authorize_conversation_schema(&project, task, &consent)
                .is_err()
        );
        assert!(
            store
                .conversation_call_budget(&project, task)
                .unwrap()
                .is_none()
        );
        store
            .with_connection(|db| {
                db.execute_batch("DROP TRIGGER fail_schema_consent;")?;
                Ok(())
            })
            .unwrap();
        store
            .authorize_conversation_schema(&project, task, &consent)
            .unwrap();
        assert!(
            store
                .reserve_conversation_call(
                    &project,
                    task,
                    consent.call_id,
                    &consent.scope_hash,
                    &"b".repeat(64)
                )
                .is_err()
        );
        assert!(
            store
                .conversation_call(&project, task, consent.call_id)
                .unwrap()
                .is_none()
        );
        drop(store);
        let store = SqliteStore::open(path).unwrap();
        assert_eq!(
            store
                .pending_conversation_schema_authorization(&project, task)
                .unwrap(),
            Some(consent.clone())
        );
        store
            .authorize_conversation_schema(&project, task, &consent)
            .unwrap();
        assert!(
            store
                .authorize_conversation_schema(
                    &project,
                    task,
                    &ConversationSchemaAuthorization {
                        model_id: annotagent_core::ModelProfileId::new(),
                        ..consent.clone()
                    }
                )
                .is_err()
        );
        assert!(
            store
                .authorize_conversation_schema(
                    &project,
                    task,
                    &ConversationSchemaAuthorization {
                        call_id: Uuid::new_v4(),
                        ..consent.clone()
                    }
                )
                .is_err()
        );
        assert!(
            store
                .pending_conversation_schema_authorization("foreign", task)
                .is_err()
        );
        store
            .set_project_conversation_call_limit(
                &project,
                &crate::ProjectCallLimitInput {
                    id: Uuid::new_v4(),
                    expected_revision: 1,
                    maximum_calls: 1,
                },
            )
            .unwrap();
        assert_eq!(
            store
                .reserve_conversation_call(
                    &project,
                    task,
                    consent.call_id,
                    &consent.scope_hash,
                    &"b".repeat(64)
                )
                .unwrap(),
            ConversationCallAdmission::Admitted
        );
        assert!(
            store
                .pending_conversation_schema_authorization(&project, task)
                .unwrap()
                .is_none()
        );
        assert!(matches!(
            store
                .reserve_conversation_call(
                    &project,
                    task,
                    consent.call_id,
                    &consent.scope_hash,
                    &"b".repeat(64)
                )
                .unwrap(),
            ConversationCallAdmission::Existing(_)
        ));
        assert_eq!(
            store
                .project_conversation_call_limit(&project)
                .unwrap()
                .reserved_calls,
            1
        );
    }

    #[test]
    fn schema_retry_adds_one_exact_allowance_and_is_restart_idempotent() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("TEST-schema-retry.db");
        let store = SqliteStore::open(&path).unwrap();
        let project = Uuid::new_v4().to_string();
        let conversation = store.create_conversation(&project).unwrap();
        let message = ConversationMessageInput {
            id: Uuid::new_v4(),
            text: "TEST retry".into(),
            image: None,
            reference: None,
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
        let original = Uuid::new_v4();
        let first = ConversationCallGrant {
            id: original,
            task_id: task,
            scope_hash: "b".repeat(64),
            maximum_calls: 1,
            expires_at: Utc::now() + Duration::minutes(10),
        };
        store
            .authorize_conversation_calls(&project, &first)
            .unwrap();
        assert_eq!(
            store
                .reserve_conversation_call(
                    &project,
                    task,
                    original,
                    &first.scope_hash,
                    &"c".repeat(64)
                )
                .unwrap(),
            ConversationCallAdmission::Admitted
        );
        store
            .finish_conversation_call(
                &project,
                task,
                original,
                ConversationCallStatus::Completed,
                serde_json::json!({
                    "decision":{"Err":"no final action"},
                    "diagnostic":{"failure_code":"length_terminated_without_action"},
                    "response":{"usage":{"input_tokens":1639,"output_tokens":2048}}
                }),
            )
            .unwrap();
        let retry = ConversationSchemaRetryAuthorization {
            call_id: Uuid::new_v4(),
            retry_of: original,
            model_id: annotagent_core::ModelProfileId::new(),
            previous_grant_id: original,
            scope_hash: "d".repeat(64),
            maximum_calls: 2,
            expires_at: Utc::now() + Duration::minutes(10),
            allow_unknown_cost: true,
        };
        store
            .authorize_conversation_schema_retry(&project, task, &retry)
            .unwrap();
        store
            .authorize_conversation_schema_retry(&project, task, &retry)
            .unwrap();
        assert_eq!(
            store
                .conversation_schema_retry(&project, task, retry.call_id)
                .unwrap(),
            Some(retry.clone())
        );
        assert_eq!(
            store
                .latest_conversation_schema_retry(&project, task, original)
                .unwrap(),
            Some(retry.clone())
        );
        assert_eq!(
            store
                .reserve_conversation_call(
                    &project,
                    task,
                    retry.call_id,
                    &retry.scope_hash,
                    &"e".repeat(64)
                )
                .unwrap(),
            ConversationCallAdmission::Admitted
        );
        store
            .finish_conversation_call(
                &project,
                task,
                retry.call_id,
                ConversationCallStatus::Completed,
                serde_json::json!({"decision":{"Ok":{"decision":"draft"}}}),
            )
            .unwrap();
        store
            .authorize_conversation_schema_retry(&project, task, &retry)
            .unwrap();
        let mut changed = retry.clone();
        changed.scope_hash = "f".repeat(64);
        assert!(
            store
                .authorize_conversation_schema_retry(&project, task, &changed)
                .is_err()
        );
        let second = ConversationSchemaRetryAuthorization {
            call_id: Uuid::new_v4(),
            previous_grant_id: retry.call_id,
            maximum_calls: 3,
            ..retry.clone()
        };
        assert!(
            store
                .authorize_conversation_schema_retry(&project, task, &second)
                .is_err()
        );
        drop(store);
        let reopened = SqliteStore::open(path).unwrap();
        assert_eq!(
            reopened
                .conversation_schema_retry(&project, task, retry.call_id)
                .unwrap(),
            Some(retry)
        );
        assert_eq!(
            reopened
                .conversation_call_budget(&project, task)
                .unwrap()
                .unwrap()
                .used_calls,
            2
        );
    }

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
    fn progress_recovery_and_legacy_nulls_survive_reopen_without_fabricated_end_times() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("TEST-progress.db");
        let store = SqliteStore::open(&path).unwrap();
        let project_id = Uuid::new_v4().to_string();
        let project = project_id.as_str();
        let conversation = store.create_conversation(project).unwrap();
        let message = ConversationMessageInput {
            id: Uuid::new_v4(),
            text: "TEST progress".into(),
            image: None,
            reference: None,
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
        let scope = "b".repeat(64);
        let request = "c".repeat(64);
        store
            .authorize_conversation_calls(
                project,
                &ConversationCallGrant {
                    id: Uuid::new_v4(),
                    task_id: task,
                    scope_hash: scope.clone(),
                    maximum_calls: 2,
                    expires_at: Utc::now() + Duration::minutes(10),
                },
            )
            .unwrap();
        let call = Uuid::new_v4();
        store
            .reserve_conversation_call(project, task, call, &scope, &request)
            .unwrap();
        store
            .mark_conversation_call_stage(project, task, call, "provider_request")
            .unwrap();
        let active = store
            .conversation_call(project, task, call)
            .unwrap()
            .unwrap();
        assert_eq!(active.stage.as_deref(), Some("provider_request"));
        assert!(active.completed_at.is_none());
        assert!(
            store
                .mark_conversation_call_stage("foreign", task, call, "response_received")
                .is_err()
        );
        drop(store);
        let store = SqliteStore::open(&path).unwrap();
        store.recover_conversation_calls().unwrap();
        let recovered = store
            .conversation_call(project, task, call)
            .unwrap()
            .unwrap();
        assert_eq!(recovered.started_at, active.started_at);
        assert!(recovered.duration_ms.is_some());
        assert_eq!(
            recovered.failure.as_ref().unwrap().stage,
            annotagent_core::ModelFailureStage::Recovery
        );
        assert_eq!(recovered.status, ConversationCallStatus::InDoubt);
        store.recover_conversation_calls().unwrap();
        store
            .mark_conversation_call_stage(project, task, call, "response_received")
            .unwrap();
        assert_eq!(
            store
                .conversation_call(project, task, call)
                .unwrap()
                .unwrap(),
            recovered
        );
        assert_eq!(
            store
                .reserve_conversation_call(project, task, call, &scope, &request)
                .unwrap(),
            ConversationCallAdmission::Existing(recovered)
        );
        // Model an old completed row by dropping only the new metadata table in this TEST database.
        store
            .with_connection(|db| {
                db.execute_batch("DROP TABLE conversation_call_progress;")?;
                Ok(())
            })
            .unwrap();
        drop(store);
        let store = SqliteStore::open(&path).unwrap();
        let legacy = store
            .conversation_call(project, task, call)
            .unwrap()
            .unwrap();
        assert!(legacy.started_at.is_some());
        assert!(
            legacy.completed_at.is_none()
                && legacy.stage.is_none()
                && legacy.duration_ms.is_none()
                && legacy.failure.is_none()
        );
        assert_eq!(legacy.status, ConversationCallStatus::InDoubt);
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
