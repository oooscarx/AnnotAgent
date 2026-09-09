//! Frozen, explicitly selected cancellation commands over existing executors.
use crate::{
    ConversationMessage, ConversationMessageInput, ConversationSelectionRef, SqliteStore,
    StorageError,
};
use chrono::{DateTime, Utc};
use rusqlite::{Connection, OptionalExtension, params};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConversationStopTargetKind {
    Call,
    Builder,
    Journey,
    Sample,
    Processing,
    Authorization,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ConversationStopTargetRef {
    pub kind: ConversationStopTargetKind,
    pub id: String,
    pub task_id: Uuid,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ConversationStopTarget {
    pub kind: ConversationStopTargetKind,
    pub id: String,
    pub task_id: Uuid,
    pub state: String,
    pub parent_journey_ids: Vec<Uuid>,
}
impl ConversationStopTarget {
    #[must_use]
    pub fn reference(&self) -> ConversationStopTargetRef {
        ConversationStopTargetRef {
            kind: self.kind,
            id: self.id.clone(),
            task_id: self.task_id,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConversationStopStatus {
    NoActiveWork,
    NeedsSelection,
    CancelRequested,
    Finished,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ConversationStopRequest {
    pub message: ConversationMessage,
    pub targets: Vec<ConversationStopTarget>,
    pub selected_target: Option<ConversationStopTargetRef>,
    pub status: ConversationStopStatus,
    pub created_at: String,
}

/// Exact immutable authorization ancestry, independent of the current task grant.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ConversationOperationCallState {
    pub reserved: u64,
    pub in_doubt: u64,
}

fn invalid(message: &str) -> StorageError {
    StorageError::InvalidConversation(message.into())
}
fn uuid(value: &str) -> Result<Uuid, StorageError> {
    Uuid::parse_str(value).map_err(|_| invalid("invalid persisted stop target identity"))
}

fn owned(
    db: &Connection,
    project: &str,
    conversation: Uuid,
    task: Option<Uuid>,
) -> Result<(), StorageError> {
    crate::conversations::require_owner(db, project, conversation)?;
    if let Some(task) = task {
        let exists: bool = db.query_row(
            "SELECT EXISTS(SELECT 1 FROM conversation_tasks WHERE id=?1 AND conversation_id=?2)",
            params![task.to_string(), conversation.to_string()],
            |r| r.get(0),
        )?;
        if task.is_nil() || !exists {
            return Err(invalid("stop task belongs to another conversation"));
        }
    }
    Ok(())
}

fn read(
    db: &Connection,
    conversation: Uuid,
    message: Uuid,
) -> Result<Option<(String, ConversationStopRequest)>, StorageError> {
    let value:Option<(String,String,String)>=db.query_row("SELECT conversation_id,project_id,record_json FROM conversation_stop_requests WHERE message_id=?1",[message.to_string()],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?))).optional()?;
    value
        .map(|(owner, project, json)| {
            if owner != conversation.to_string() {
                return Err(invalid("stop command belongs to another conversation"));
            }
            Ok((project, serde_json::from_str(&json)?))
        })
        .transpose()
}

/// Transaction-local exact marker used by Batch creation and spending admission.
pub(crate) fn processing_requested(db: &Connection, id: &str) -> Result<bool, StorageError> {
    Ok(db.query_row("SELECT EXISTS(SELECT 1 FROM conversation_stop_requests WHERE json_extract(record_json,'$.status')='cancel_requested' AND json_extract(record_json,'$.selected_target.kind')='processing' AND json_extract(record_json,'$.selected_target.id')=?1)",[id],|r|r.get(0))?)
}

fn operation_requested(db: &Connection, task: Uuid, id: &str) -> Result<bool, StorageError> {
    Ok(db.query_row("SELECT EXISTS(
        SELECT 1 FROM conversation_call_cancellations WHERE task_id=?1 AND call_id=?2
        UNION ALL SELECT 1 FROM conversation_stop_requests WHERE json_extract(record_json,'$.status')='cancel_requested' AND json_extract(record_json,'$.selected_target.task_id')=?1 AND json_extract(record_json,'$.selected_target.id')=?2 AND json_extract(record_json,'$.selected_target.kind') IN ('call','builder','sample','authorization')
        UNION ALL SELECT 1 FROM conversation_journey_consents WHERE task_id=?1 AND revoked=1 AND (builder_operation_id=?2 OR sample_operation_id=?2)
    )",params![task.to_string(),id],|r|r.get(0))?)
}

/// Fresh admission only; historical receipts are restored by their original APIs first.
pub(crate) fn require_admission_clear(
    db: &Connection,
    task: Uuid,
    id: &str,
    include_current_grant: bool,
) -> Result<(), StorageError> {
    if operation_requested(db, task, id)? {
        return Err(invalid("operation was stopped; no new work was admitted"));
    }
    if include_current_grant {
        let grant: Option<String> = db
            .query_row(
                "SELECT id FROM conversation_call_grants WHERE task_id=?1",
                [task.to_string()],
                |r| r.get(0),
            )
            .optional()?;
        if let Some(grant) = grant {
            if operation_requested(db, task, &grant)? {
                return Err(invalid(
                    "the exact task authorization was stopped; no model call was admitted",
                ));
            }
        }
    }
    Ok(())
}

fn active_sample(status: &str) -> bool {
    matches!(status, "queued" | "running" | "cancelling")
}
fn active_batch(status: &str) -> bool {
    matches!(status, "pending" | "running" | "paused")
}
fn future(expiry: &str) -> Result<bool, StorageError> {
    Ok(DateTime::parse_from_rfc3339(expiry)
        .map_err(|_| invalid("invalid saved authorization expiry"))?
        > Utc::now())
}

#[derive(Clone)]
struct Journey {
    consent: crate::ConversationJourneyConsent,
    revoked: bool,
    dispatch: Option<String>,
}

/// No history limits: every query belongs to the caller's single transaction snapshot.
pub(crate) fn discover(
    db: &Connection,
    project: &str,
    conversation: Uuid,
    task: Option<Uuid>,
) -> Result<Vec<ConversationStopTarget>, StorageError> {
    let mut query=db.prepare("SELECT id FROM conversation_tasks WHERE conversation_id=?1 AND (?2 IS NULL OR id=?2) ORDER BY id")?;
    let tasks = query
        .query_map(
            params![conversation.to_string(), task.map(|v| v.to_string())],
            |r| r.get::<_, String>(0),
        )?
        .collect::<Result<Vec<_>, _>>()?;
    let mut targets = Vec::new();
    for task in tasks {
        discover_task(db, project, conversation, uuid(&task)?, &mut targets)?;
        if targets.len() > 4096 {
            return Err(invalid(
                "too many active stop targets; select a task explicitly",
            ));
        }
    }
    targets.sort_by_key(ConversationStopTarget::reference);
    Ok(targets)
}

#[allow(clippy::too_many_lines)]
fn discover_task(
    db: &Connection,
    project: &str,
    conversation: Uuid,
    task: Uuid,
    output: &mut Vec<ConversationStopTarget>,
) -> Result<(), StorageError> {
    use ConversationStopTargetKind as Kind;
    let task_text = task.to_string();
    let mut query=db.prepare("SELECT m.id,m.status,a.grant_id FROM conversation_model_calls m LEFT JOIN conversation_call_authorizations a ON a.call_id=m.id WHERE m.task_id=?1")?;
    let calls = query
        .query_map([&task_text], |r| {
            Ok((
                r.get::<_, String>(0)?,
                (r.get::<_, String>(1)?, r.get::<_, Option<String>>(2)?),
            ))
        })?
        .collect::<Result<BTreeMap<_, _>, _>>()?;
    let mut query=db.prepare("SELECT id,status,json_extract(evidence_json,'$.outcome') FROM conversation_builder_operations WHERE task_id=?1")?;
    let builders = query
        .query_map([&task_text], |r| {
            Ok((
                r.get::<_, String>(0)?,
                (r.get::<_, String>(1)?, r.get::<_, Option<String>>(2)?),
            ))
        })?
        .collect::<Result<BTreeMap<_, _>, _>>()?;
    let mut query=db.prepare("SELECT id,status FROM sample_operations WHERE project_id=?1 AND json_extract(request_json,'$.conversation.conversation_id')=?2 AND json_extract(request_json,'$.conversation.task_id')=?3")?;
    let samples = query
        .query_map(params![project, conversation.to_string(), task_text], |r| {
            Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?))
        })?
        .collect::<Result<BTreeMap<_, _>, _>>()?;
    let mut query=db.prepare("SELECT c.input_json,c.revoked,d.status FROM conversation_journey_consents c LEFT JOIN conversation_journey_dispatch d ON d.consent_id=c.id WHERE c.task_id=?1 ORDER BY c.id")?;
    let journey_rows = query
        .query_map([&task_text], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, bool>(1)?,
                r.get::<_, Option<String>>(2)?,
            ))
        })?
        .collect::<Result<Vec<_>, _>>()?;
    let journeys = journey_rows
        .into_iter()
        .map(|(input, revoked, dispatch)| {
            Ok(Journey {
                consent: serde_json::from_str(&input)?,
                revoked,
                dispatch,
            })
        })
        .collect::<Result<Vec<_>, StorageError>>()?;
    let mut parents: BTreeMap<String, Vec<Uuid>> = BTreeMap::new();
    let mut active_journeys = BTreeSet::new();
    for journey in &journeys {
        let c = &journey.consent;
        for id in [
            Some(c.builder_operation_id),
            Some(c.sample_operation_id),
            c.schema_proposal.as_ref().map(|v| v.call_id),
        ]
        .into_iter()
        .flatten()
        {
            parents.entry(id.to_string()).or_default().push(c.id);
        }
        let builder = builders.get(&c.builder_operation_id.to_string());
        let sample = samples.get(&c.sample_operation_id.to_string());
        let schema = c
            .schema_proposal
            .as_ref()
            .and_then(|s| calls.get(&s.call_id.to_string()));
        let reserved_child = builder.is_some_and(|b| b.0 == "reserved")
            || sample.is_some_and(|s| active_sample(s))
            || schema.is_some_and(|s| s.0 == "reserved");
        let can_continue = !journey.revoked
            && c.expires_at > Utc::now()
            && sample.is_none()
            && builder.is_none_or(|b| {
                b.0 == "reserved"
                    || (b.0 == "completed"
                        && b.1.as_deref() == Some("draft_ready_for_human_review"))
            })
            && schema.is_none_or(|s| matches!(s.0.as_str(), "reserved" | "completed"));
        if reserved_child || journey.dispatch.as_deref() == Some("running") || can_continue {
            active_journeys.insert(c.id);
            let state = if journey.revoked {
                "cancelling"
            } else if reserved_child || journey.dispatch.as_deref() == Some("running") {
                "running"
            } else {
                "authorized_or_waiting"
            };
            output.push(ConversationStopTarget {
                kind: Kind::Journey,
                id: c.id.to_string(),
                task_id: task,
                state: state.into(),
                parent_journey_ids: vec![],
            });
        }
    }
    let mut local: BTreeMap<(Kind, String), String> = BTreeMap::new();
    for (id, (status, grant)) in &calls {
        if status != "reserved" {
            continue;
        }
        // Immutable authorization ancestry, never the task's latest grant.
        if grant.as_ref().is_some_and(|g| {
            builders.get(g).is_some_and(|b| b.0 == "reserved")
                || samples.get(g).is_some_and(|s| active_sample(s))
        }) {
            continue;
        }
        local.insert(
            (Kind::Call, id.clone()),
            if operation_requested(db, task, id)? {
                "cancelling"
            } else {
                "reserved"
            }
            .into(),
        );
    }
    for (id, (status, _)) in &builders {
        if status == "reserved" {
            local.insert(
                (Kind::Builder, id.clone()),
                if operation_requested(db, task, id)? {
                    "cancelling"
                } else {
                    "reserved"
                }
                .into(),
            );
        }
    }
    for (id, status) in &samples {
        if active_sample(status) {
            local.insert((Kind::Sample, id.clone()), status.clone());
        }
    }
    let grant: Option<(String, String, bool)> = db
        .query_row(
            "SELECT id,expires_at,revoked FROM conversation_call_grants WHERE task_id=?1",
            [&task_text],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .optional()?;
    if let Some((id, expiry, revoked)) = grant {
        if !revoked
            && future(&expiry)?
            && !operation_requested(db, task, &id)?
            && !calls.contains_key(&id)
            && !builders.contains_key(&id)
            && !samples.contains_key(&id)
            && !calls
                .values()
                .any(|(_, grant)| grant.as_deref() == Some(&id))
        {
            let typed:bool=db.query_row("SELECT EXISTS(SELECT 1 FROM conversation_schema_authorizations WHERE call_id=?1 AND task_id=?2 UNION ALL SELECT 1 FROM conversation_feedback_authorizations WHERE call_id=?1 AND task_id=?2 UNION ALL SELECT 1 FROM conversation_future_schema_proposal_authorizations WHERE call_id=?1 AND task_id=?2)",params![id,task_text],|r|r.get(0))?;
            local.insert(
                (
                    if typed {
                        Kind::Call
                    } else {
                        Kind::Authorization
                    },
                    id,
                ),
                "authorized_not_started".into(),
            );
        }
    }
    for ((kind, id), state) in local {
        let parent_journey_ids = parents.get(&id).cloned().unwrap_or_default();
        // Shared initial calls retain their own choice; never choose an arbitrary parent.
        if parent_journey_ids.len() == 1 && active_journeys.contains(&parent_journey_ids[0]) {
            continue;
        }
        output.push(ConversationStopTarget {
            kind,
            id,
            task_id: task,
            state,
            parent_journey_ids,
        });
    }
    let mut query=db.prepare("SELECT p.id,json_extract(p.state_json,'$.phase'),b.status,b.project_id FROM processing_operations p LEFT JOIN dataset_batches b ON b.id=p.id WHERE p.project_id=?1 AND json_extract(p.state_json,'$.authorization.conversation.conversation_id')=?2 AND json_extract(p.state_json,'$.authorization.conversation.task_id')=?3")?;
    let processing = query
        .query_map(params![project, conversation.to_string(), task_text], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, Option<String>>(1)?,
                r.get::<_, Option<String>>(2)?,
                r.get::<_, Option<String>>(3)?,
            ))
        })?
        .collect::<Result<Vec<_>, _>>()?;
    for (id, phase, batch_status, batch_project) in processing {
        if batch_project.as_deref().is_some_and(|p| p != project) {
            return Err(invalid("processing Batch belongs to another Project"));
        }
        let active = match &batch_status {
            Some(status) => active_batch(status),
            None => {
                matches!(
                    phase.as_deref(),
                    Some(
                        "confirmed"
                            | "publishing"
                            | "published"
                            | "published_start_failed"
                            | "failed"
                    )
                ) && !processing_requested(db, &id)?
            }
        };
        if active {
            output.push(ConversationStopTarget {
                kind: Kind::Processing,
                id,
                task_id: task,
                state: batch_status.or(phase).unwrap_or_default(),
                parent_journey_ids: vec![],
            });
        }
    }
    Ok(())
}

fn save(
    db: &Connection,
    project: &str,
    record: &ConversationStopRequest,
) -> Result<(), StorageError> {
    db.execute("INSERT INTO conversation_stop_requests(message_id,conversation_id,project_id,record_json,created_at) VALUES(?1,?2,?3,?4,?5) ON CONFLICT(message_id) DO UPDATE SET record_json=excluded.record_json",params![record.message.input.id.to_string(),record.message.conversation_id.to_string(),project,serde_json::to_string(record)?,record.created_at])?;
    Ok(())
}

fn pending_grant(db: &Connection, task: Uuid, id: &str) -> Result<bool, StorageError> {
    let expiry:Option<String>=db.query_row("SELECT expires_at FROM conversation_call_grants g WHERE g.task_id=?1 AND g.id=?2 AND g.revoked=0 AND NOT EXISTS(SELECT 1 FROM conversation_model_calls m JOIN conversation_call_authorizations a ON a.call_id=m.id WHERE a.grant_id=g.id) AND NOT EXISTS(SELECT 1 FROM conversation_builder_operations WHERE id=g.id) AND NOT EXISTS(SELECT 1 FROM sample_operations WHERE id=g.id)",params![task.to_string(),id],|r|r.get(0)).optional()?;
    Ok(match expiry {
        Some(expiry) => future(&expiry)? && !operation_requested(db, task, id)?,
        None => false,
    })
}

fn cancel_selected(
    tx: &rusqlite::Transaction<'_>,
    owner: &str,
    project: &str,
    record: &mut ConversationStopRequest,
    target: &ConversationStopTargetRef,
) -> Result<(), StorageError> {
    use ConversationStopTargetKind as Kind;
    let active = discover(
        tx,
        project,
        record.message.conversation_id,
        Some(target.task_id),
    )?;
    // The exact frozen identity may have finished. Never redirect to a new target.
    let still_active=match target.kind {
        // Parent relationships may have appeared after the snapshot. They cannot
        // hide this exact call, or turn selecting it into revoking its parents.
        Kind::Call=>{
            let status:Option<String>=tx.query_row("SELECT status FROM conversation_model_calls WHERE id=?1 AND task_id=?2",params![target.id,target.task_id.to_string()],|r|r.get(0)).optional()?;
            match status {Some(status)=>status=="reserved",None=>pending_grant(tx,target.task_id,&target.id)?}
        }
        Kind::Authorization=>{
            let reserved:bool=tx.query_row("SELECT EXISTS(SELECT 1 FROM conversation_model_calls c JOIN conversation_call_authorizations a ON a.call_id=c.id WHERE c.task_id=?1 AND a.grant_id=?2 AND c.status='reserved' UNION ALL SELECT 1 FROM conversation_builder_operations WHERE task_id=?1 AND id=?2 AND status='reserved' UNION ALL SELECT 1 FROM sample_operations WHERE id=?2 AND project_id=?3 AND status IN ('queued','running','cancelling') AND json_extract(request_json,'$.conversation.task_id')=?1)",params![target.task_id.to_string(),target.id,project],|r|r.get(0))?;
            reserved || pending_grant(tx,target.task_id,&target.id)?
        }
        Kind::Builder=>tx.query_row("SELECT EXISTS(SELECT 1 FROM conversation_builder_operations WHERE id=?1 AND task_id=?2 AND status='reserved')",params![target.id,target.task_id.to_string()],|r|r.get(0))?,
        Kind::Sample=>tx.query_row("SELECT EXISTS(SELECT 1 FROM sample_operations WHERE id=?1 AND project_id=?2 AND json_extract(request_json,'$.conversation.task_id')=?3 AND status IN ('queued','running','cancelling'))",params![target.id,project,target.task_id.to_string()],|r|r.get(0))?,
        Kind::Journey|Kind::Processing=>active.iter().any(|v|v.reference()==*target),
    };
    record.selected_target = Some(target.clone());
    record.status = if still_active {
        ConversationStopStatus::CancelRequested
    } else {
        ConversationStopStatus::Finished
    };
    save(tx, project, record)?;
    if !still_active {
        return Ok(());
    }
    match target.kind {
        Kind::Call | Kind::Builder | Kind::Authorization => {
            crate::conversation_calls::request_cancel_in(
                tx,
                owner,
                target.task_id,
                uuid(&target.id)?,
            )?;
            // An untyped grant can acquire its exact Sample row while awaiting selection.
            if target.kind == Kind::Authorization {
                crate::sample_operations::cancel_in(tx, &target.id, project)?;
            }
        }
        Kind::Sample => crate::sample_operations::cancel_in(tx, &target.id, project)?,
        Kind::Processing => {
            let exists: bool = tx.query_row(
                "SELECT EXISTS(SELECT 1 FROM dataset_batches WHERE id=?1 AND project_id=?2)",
                params![target.id, project],
                |r| r.get(0),
            )?;
            if exists {
                crate::batch::cancel_active_batch_in(
                    tx,
                    annotagent_core::BatchId(uuid(&target.id)?),
                    Utc::now(),
                )?;
            }
        }
        Kind::Journey => {
            let input: String = tx.query_row(
                "SELECT input_json FROM conversation_journey_consents WHERE id=?1 AND task_id=?2",
                params![target.id, target.task_id.to_string()],
                |r| r.get(0),
            )?;
            let consent: crate::ConversationJourneyConsent = serde_json::from_str(&input)?;
            tx.execute(
                "UPDATE conversation_journey_consents SET revoked=1 WHERE id=?1 AND task_id=?2",
                params![target.id, target.task_id.to_string()],
            )?;
            crate::conversation_calls::request_cancel_in(
                tx,
                owner,
                target.task_id,
                consent.builder_operation_id,
            )?;
            crate::sample_operations::cancel_in(
                tx,
                &consent.sample_operation_id.to_string(),
                project,
            )?;
            if let Some(schema) = consent.schema_proposal {
                let shared:bool=tx.query_row("SELECT EXISTS(SELECT 1 FROM conversation_journey_consents WHERE id!=?1 AND revoked=0 AND json_extract(input_json,'$.schema_proposal.call_id')=?2)",params![target.id,schema.call_id.to_string()],|r|r.get(0))?;
                if !shared {
                    crate::conversation_calls::request_cancel_in(
                        tx,
                        owner,
                        target.task_id,
                        schema.call_id,
                    )?;
                }
            }
        }
    }
    Ok(())
}

impl SqliteStore {
    pub fn conversation_operation_call_state(
        &self,
        project: &str,
        conversation: Uuid,
        task: Uuid,
        operation_id: &str,
    ) -> Result<ConversationOperationCallState, StorageError> {
        self.with_connection(|db| {
            owned(db, project, conversation, Some(task))?;
            let foreign:bool=db.query_row("SELECT EXISTS(
                SELECT 1 FROM conversation_authorization_revisions WHERE id=?1 AND task_id!=?2
                UNION ALL SELECT 1 FROM conversation_builder_operations WHERE id=?1 AND task_id!=?2
                UNION ALL SELECT 1 FROM sample_operations WHERE id=?1 AND (json_extract(request_json,'$.conversation.task_id') IS NOT ?2 OR json_extract(request_json,'$.conversation.conversation_id') IS NOT ?3)
            )",params![operation_id,task.to_string(),conversation.to_string()],|r|r.get(0))?;
            if foreign {return Err(invalid("operation belongs to another task or conversation"));}
            let (reserved,in_doubt):(i64,i64)=db.query_row("SELECT COALESCE(SUM(m.status='reserved'),0),COALESCE(SUM(m.status='in_doubt'),0) FROM conversation_call_authorizations a JOIN conversation_model_calls m ON m.id=a.call_id JOIN conversation_authorization_revisions g ON g.id=a.grant_id WHERE a.grant_id=?1 AND m.task_id=?2 AND g.task_id=?2",params![operation_id,task.to_string()],|r|Ok((r.get(0)?,r.get(1)?)))?;
            Ok(ConversationOperationCallState {
                reserved:u64::try_from(reserved).map_err(|_|invalid("invalid reserved call count"))?,
                in_doubt:u64::try_from(in_doubt).map_err(|_|invalid("invalid indeterminate call count"))?,
            })
        })
    }
    pub fn processing_stop_requested(&self, id: &str) -> Result<bool, StorageError> {
        self.with_connection(|db| processing_requested(db, id))
    }
    pub fn conversation_stop_target_requested(
        &self,
        project: &str,
        conversation: Uuid,
        task: Uuid,
        kind: ConversationStopTargetKind,
        id: &str,
    ) -> Result<bool, StorageError> {
        self.with_connection(|db| {
            owned(db,project,conversation,Some(task))?;
            if kind==ConversationStopTargetKind::Processing {
                return Ok(db.query_row("SELECT EXISTS(SELECT 1 FROM conversation_stop_requests WHERE conversation_id=?1 AND json_extract(record_json,'$.status')='cancel_requested' AND json_extract(record_json,'$.selected_target.kind')='processing' AND json_extract(record_json,'$.selected_target.id')=?2 AND json_extract(record_json,'$.selected_target.task_id')=?3)",params![conversation.to_string(),id,task.to_string()],|r|r.get(0))?);
            }
            if kind==ConversationStopTargetKind::Journey {
                return Ok(db.query_row("SELECT EXISTS(SELECT 1 FROM conversation_journey_consents WHERE id=?1 AND task_id=?2 AND revoked=1)",params![id,task.to_string()],|r|r.get(0))?);
            }
            operation_requested(db,task,id)
        })
    }
    pub fn conversation_stop_request(
        &self,
        project: &str,
        conversation: Uuid,
        message: Uuid,
    ) -> Result<Option<ConversationStopRequest>, StorageError> {
        self.with_connection(|db| {
            owned(db, project, conversation, None)?;
            Ok(read(db, conversation, message)?.map(|(_, record)| record))
        })
    }
    pub fn begin_conversation_stop(
        &self,
        owner: &str,
        project: &str,
        conversation: Uuid,
        input: &ConversationMessageInput,
    ) -> Result<ConversationStopRequest, StorageError> {
        let Some(ConversationSelectionRef::StopRequest { task_id }) = &input.reference else {
            return Err(invalid(
                "stop command requires an explicit stop_request reference",
            ));
        };
        if input.id.is_nil()
            || input.text.len() > 65_536
            || input.image.is_some()
            || !(input.text.trim() == "停止" || input.text.trim().eq_ignore_ascii_case("stop"))
        {
            return Err(invalid(
                "stop command must be standalone stop or 停止, without an image",
            ));
        }
        self.with_connection(|db| {
            let tx=db.unchecked_transaction()?;
            owned(&tx,owner,conversation,*task_id)?;
            if let Some((saved_project,saved))=read(&tx,conversation,input.id)? {
                if saved_project!=project || saved.message.input!=*input {return Err(invalid("stop command ID already has different content or scope"));}
                return Ok(saved);
            }
            let collision:bool=tx.query_row("SELECT EXISTS(SELECT 1 FROM conversation_messages WHERE conversation_id=?1 AND message_id=?2)",params![conversation.to_string(),input.id.to_string()],|r|r.get(0))?;
            if collision {return Err(invalid("message ID already exists without a stop command"));}
            let sequence:i64=tx.query_row("SELECT COALESCE(MAX(sequence),0)+1 FROM conversation_messages WHERE conversation_id=?1",[conversation.to_string()],|r|r.get(0))?;
            let created_at=Utc::now().to_rfc3339();
            tx.execute("INSERT INTO conversation_messages(conversation_id,sequence,message_id,input_json,created_at) VALUES(?1,?2,?3,?4,?5)",params![conversation.to_string(),sequence,input.id.to_string(),serde_json::to_string(input)?,created_at])?;
            let targets=discover(&tx,project,conversation,*task_id)?;
            let mut record=ConversationStopRequest{message:ConversationMessage{conversation_id:conversation,sequence,input:input.clone()},status:if targets.is_empty(){ConversationStopStatus::NoActiveWork}else{ConversationStopStatus::NeedsSelection},targets,selected_target:None,created_at};
            if record.targets.len()==1 {let target=record.targets[0].reference();cancel_selected(&tx,owner,project,&mut record,&target)?;} else {save(&tx,project,&record)?;}
            tx.commit()?;Ok(record)
        })
    }
    pub fn select_conversation_stop(
        &self,
        owner: &str,
        project: &str,
        conversation: Uuid,
        message: Uuid,
        target: &ConversationStopTargetRef,
    ) -> Result<ConversationStopRequest, StorageError> {
        self.with_connection(|db| {
            let tx = db.unchecked_transaction()?;
            owned(&tx, owner, conversation, Some(target.task_id))?;
            let (saved_project, mut record) = read(&tx, conversation, message)?
                .ok_or_else(|| invalid("stop command not found"))?;
            if saved_project != project {
                return Err(invalid("stop command Project binding changed"));
            }
            if let Some(saved) = &record.selected_target {
                if saved != target {
                    return Err(invalid("stop command already selected a different target"));
                }
                return Ok(record);
            }
            if !record.targets.iter().any(|v| v.reference() == *target) {
                return Err(invalid(
                    "selected target was not in the frozen stop snapshot",
                ));
            }
            cancel_selected(&tx, owner, project, &mut record, target)?;
            tx.commit()?;
            Ok(record)
        })
    }
}

#[cfg(test)]
#[path = "conversation_stop_tests.rs"]
mod tests;
