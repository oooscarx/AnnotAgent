//! Immutable authorization evidence for a bounded Builder -> sample continuation.
//! This record alone permits no inference. Application must resolve real Registry/data
//! snapshots, check permissions, and use the existing call ledger and executors.
use crate::{SqliteStore, StorageError};
use chrono::{DateTime, Utc};
use rusqlite::{OptionalExtension, params};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct JourneyModelScope {
    pub model_id: String,
    /// Hash of server-resolved model, provider destination and permission snapshots;
    /// never a hash supplied by the LLM or a raw credential.
    pub binding_digest: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct JourneyImageScope {
    pub image_id: Uuid,
    pub content_hash: String,
}

/// Exact editable copy approved for repair, never a pointer to its latest revision.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ConversationBuilderRepair {
    pub request_id: Uuid,
    pub draft_id: String,
    pub revision: u64,
    pub content_hash: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ConversationJourneyConsent {
    /// Permission for this one pending request only. It must resolve to an exact
    /// acknowledged repair snapshot before any Builder/sample execution.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub repair_after_answer: Option<crate::ConversationHumanRequestInput>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub repair: Option<ConversationBuilderRepair>,
    /// Present only when the authorized text call must first produce a Schema.
    /// Nil `schema_id` and revision zero mean unresolved, never a fake Schema.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub schema_proposal: Option<crate::ConversationSchemaAuthorization>,
    /// Only newly acknowledged envelopes may resume after a linked human answer.
    /// Older saved consents never acquire this permission through deserialization.
    #[serde(default, skip_serializing_if = "is_false")]
    pub continue_after_clarification: bool,
    pub id: Uuid,
    pub task_id: Uuid,
    pub builder_operation_id: Uuid,
    #[serde(default)]
    pub builder_model_id: Option<annotagent_core::ModelProfileId>,
    #[serde(default)]
    pub previous_grant_id: Option<Uuid>,
    pub sample_operation_id: Uuid,
    pub builder_scope_hash: String,
    pub schema_id: Uuid,
    pub schema_revision: u64,
    /// SHA-256 of the persisted Schema definition JSON.
    pub schema_digest: String,
    pub images: Vec<JourneyImageScope>,
    pub allowed_models: Vec<JourneyModelScope>,
    pub maximum_builder_calls: u32,
    pub maximum_sample_calls: u32,
    pub expires_at: DateTime<Utc>,
    pub allow_unknown_cost: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct JourneySampleScope {
    pub operation_id: Uuid,
    pub draft_id: Uuid,
    pub draft_revision: u64,
    pub authorization_fingerprint: String,
    pub schema_id: Uuid,
    pub schema_revision: u64,
    pub schema_digest: String,
    pub images: Vec<JourneyImageScope>,
    pub models: Vec<JourneyModelScope>,
    pub maximum_calls: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConversationJourneyRecord {
    pub consent: ConversationJourneyConsent,
    pub revoked: bool,
    pub sample: Option<JourneySampleScope>,
    pub resolved_consent: Option<ConversationJourneyConsent>,
}

impl ConversationJourneyRecord {
    #[must_use]
    pub fn effective_consent(&self) -> &ConversationJourneyConsent {
        self.resolved_consent.as_ref().unwrap_or(&self.consent)
    }
}

fn invalid(message: &str) -> StorageError {
    StorageError::InvalidConversation(message.into())
}
#[allow(clippy::trivially_copy_pass_by_ref)] // serde skip predicate requires a reference.
fn is_false(value: &bool) -> bool {
    !value
}
fn digest(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}
fn validate(input: &ConversationJourneyConsent) -> Result<(), StorageError> {
    if input.repair_after_answer.as_ref().is_some_and(|request| {
        request.id.is_nil()
            || request.resume_checkpoint_ref.is_nil()
            || request.task_id != input.task_id
            || input.repair.is_some()
            || input.schema_proposal.is_some()
            || input.continue_after_clarification
            || !input.images.iter().any(|image| {
                Uuid::parse_str(&request.image_id).ok() == Some(image.image_id)
                    && request.content_hash == image.content_hash
            })
    }) {
        return Err(invalid(
            "Answer continuation requires one exact pending request in the authorized image scope",
        ));
    }
    if input.repair.as_ref().is_some_and(|repair| {
        repair.request_id.is_nil()
            || Uuid::parse_str(&repair.draft_id)
                .ok()
                .is_none_or(|id| id.is_nil())
            || repair.revision == 0
            || !digest(&repair.content_hash)
            || input.schema_proposal.is_some()
            || input.continue_after_clarification
    }) {
        return Err(invalid(
            "Repair requires one exact saved Draft and cannot grant initial Schema continuation",
        ));
    }
    let ids = [
        input.id,
        input.task_id,
        input.builder_operation_id,
        input.sample_operation_id,
    ];
    if ids.iter().any(Uuid::is_nil)
        || input.builder_model_id.is_none_or(|id| id.0.is_nil())
        || input.builder_operation_id == input.sample_operation_id
        || match &input.schema_proposal {
            None => input.schema_id.is_nil() || input.schema_revision == 0,
            Some(proposal) => {
                !input.schema_id.is_nil()
                    || input.schema_revision != 0
                    || proposal.call_id.is_nil()
                    || proposal.call_id == input.builder_operation_id
                    || proposal.call_id == input.sample_operation_id
                    || Some(proposal.model_id) != input.builder_model_id
                    || proposal.scope_hash != input.builder_scope_hash
                    || proposal.expires_at != input.expires_at
                    || !proposal.allow_unknown_cost
                    || input.previous_grant_id.is_some()
                    || input.maximum_builder_calls != 8
            }
        }
        || !digest(&input.schema_digest)
        || !digest(&input.builder_scope_hash)
        || !(1..=3).contains(&input.images.len())
        || input
            .images
            .iter()
            .any(|i| i.image_id.is_nil() || !digest(&i.content_hash))
        || input
            .images
            .iter()
            .map(|i| i.image_id)
            .collect::<BTreeSet<_>>()
            .len()
            != input.images.len()
        || !(1..=32).contains(&input.allowed_models.len())
        || input.allowed_models.iter().any(|m| {
            m.model_id.trim().is_empty() || m.model_id.len() > 256 || !digest(&m.binding_digest)
        })
        || input
            .allowed_models
            .iter()
            .map(|m| &m.model_id)
            .collect::<BTreeSet<_>>()
            .len()
            != input.allowed_models.len()
        || !(1..=16).contains(&input.maximum_builder_calls)
        || !(1..=12).contains(&input.maximum_sample_calls)
        || !input.allow_unknown_cost
    {
        return Err(invalid("Invalid bounded journey consent"));
    }
    Ok(())
}

/// The generated plan may choose a subset of the explicitly allowed model bindings,
/// but cannot change their snapshots, the ordered image selection, or the Schema.
fn fits(consent: &ConversationJourneyConsent, sample: &JourneySampleScope) -> bool {
    consent.repair_after_answer.is_none()
        && sample.operation_id == consent.sample_operation_id
        && !sample.draft_id.is_nil()
        && sample.draft_revision > 0
        && consent.repair.as_ref().is_none_or(|repair| {
            Uuid::parse_str(&repair.draft_id).ok() == Some(sample.draft_id)
                && sample.draft_revision >= repair.revision
        })
        && digest(&sample.authorization_fingerprint)
        && sample.schema_id == consent.schema_id
        && sample.schema_revision == consent.schema_revision
        && sample.schema_digest == consent.schema_digest
        && sample.images == consent.images
        && !sample.models.is_empty()
        && sample
            .models
            .iter()
            .all(|m| consent.allowed_models.contains(m))
        && sample
            .models
            .iter()
            .map(|m| &m.model_id)
            .collect::<BTreeSet<_>>()
            .len()
            == sample.models.len()
        && sample.maximum_calls > 0
        && sample.maximum_calls <= consent.maximum_sample_calls
}

fn owned(
    db: &rusqlite::Connection,
    project: &str,
    conversation: Uuid,
    task: Uuid,
) -> Result<(), StorageError> {
    let found:bool=db.query_row("SELECT EXISTS(SELECT 1 FROM conversation_tasks t JOIN project_conversations c ON c.id=t.conversation_id WHERE t.id=?1 AND c.id=?2 AND c.project_id=?3)",params![task.to_string(),conversation.to_string(),project],|r|r.get(0))?;
    if !found {
        return Err(invalid(
            "Journey task belongs to another conversation or Project",
        ));
    }
    Ok(())
}
fn read(
    db: &rusqlite::Connection,
    task: Uuid,
    id: Uuid,
) -> Result<Option<ConversationJourneyRecord>, StorageError> {
    let row:Option<(String,String,bool,Option<String>)>=db.query_row("SELECT task_id,input_json,revoked,sample_scope_json FROM conversation_journey_consents WHERE id=?1",[id.to_string()],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?))).optional()?;
    row.map(|(owner, input, revoked, sample)| {
        if owner != task.to_string() {
            return Err(invalid("Journey belongs to another task"));
        }
        Ok(ConversationJourneyRecord {
            consent: serde_json::from_str(&input)?,
            revoked,
            sample: sample.map(|v| serde_json::from_str(&v)).transpose()?,
            resolved_consent: db.query_row("SELECT resolved_json FROM conversation_journey_schema_resolution WHERE consent_id=?1",[id.to_string()],|row|row.get::<_,String>(0)).optional()?.map(|value|serde_json::from_str(&value)).transpose()?,
        })
    })
    .transpose()
}

impl SqliteStore {
    pub fn conversation_journey_for_builder(
        &self,
        project: &str,
        conversation: Uuid,
        task: Uuid,
        builder: Uuid,
    ) -> Result<Option<ConversationJourneyRecord>, StorageError> {
        self.with_connection(|db| {
            owned(db,project,conversation,task)?;
            let id:Option<String>=db.query_row("SELECT id FROM conversation_journey_consents WHERE task_id=?1 AND builder_operation_id=?2",params![task.to_string(),builder.to_string()],|row|row.get(0)).optional()?;
            id.map(|id|read(db,task,Uuid::parse_str(&id).map_err(|_|invalid("Invalid journey ID"))?)).transpose().map(Option::flatten)
        })
    }
    pub fn conversation_journey_ids(
        &self,
        project: &str,
        conversation: Uuid,
        task: Uuid,
    ) -> Result<Vec<Uuid>, StorageError> {
        self.with_connection(|db| {
            owned(db, project, conversation, task)?;
            let mut query = db.prepare("SELECT id FROM conversation_journey_consents WHERE task_id=?1 ORDER BY created_at DESC,id DESC LIMIT 50")?;
            let ids = query.query_map([task.to_string()], |row| row.get::<_,String>(0))?.collect::<Result<Vec<_>,_>>()?;
            ids.into_iter().map(|id| Uuid::parse_str(&id).map_err(|_| invalid("Invalid saved journey identity"))).collect()
        })
    }
    /// Claim only on explicit POST. Restart recovery never dispatches inference.
    pub fn claim_conversation_journey_dispatch(
        &self,
        project: &str,
        conversation: Uuid,
        task: Uuid,
        id: Uuid,
        attempt: Uuid,
    ) -> Result<bool, StorageError> {
        self.with_connection(|db| {
            let tx = db.unchecked_transaction()?;
            owned(&tx, project, conversation, task)?;
            let saved = read(&tx, task, id)?.ok_or_else(|| invalid("Journey consent not found"))?;
            if attempt.is_nil() || saved.revoked || saved.consent.expires_at <= Utc::now() {
                return Err(invalid("Journey consent is revoked or expired"));
            }
            if saved.effective_consent().repair_after_answer.is_some() {
                return Err(invalid("Journey is waiting for its exact acknowledged human answer"));
            }
            let changed = tx.execute("INSERT INTO conversation_journey_dispatch(consent_id,attempt_id,status,updated_at) VALUES(?1,?2,'running',?3) ON CONFLICT(consent_id) DO UPDATE SET attempt_id=excluded.attempt_id,status='running',error=NULL,updated_at=excluded.updated_at WHERE conversation_journey_dispatch.status!='running'",params![id.to_string(),attempt.to_string(),Utc::now().to_rfc3339()])?;
            tx.commit()?;
            Ok(changed == 1)
        })
    }
    pub fn conversation_journey_dispatch(
        &self,
        project: &str,
        conversation: Uuid,
        task: Uuid,
        id: Uuid,
    ) -> Result<Option<serde_json::Value>, StorageError> {
        self.with_connection(|db| {
            owned(db, project, conversation, task)?;
            read(db, task, id)?.ok_or_else(|| invalid("Journey consent not found"))?;
            Ok(db.query_row("SELECT attempt_id,status,error,updated_at FROM conversation_journey_dispatch WHERE consent_id=?1",[id.to_string()],|row|Ok(serde_json::json!({"attempt_id":row.get::<_,String>(0)?,"status":row.get::<_,String>(1)?,"error":row.get::<_,Option<String>>(2)?,"updated_at":row.get::<_,String>(3)?}))).optional()?)
        })
    }
    /// Attempt CAS prevents an obsolete worker from settling a newer dispatch.
    /// True retains the worker for an explicitly queued answer that arrived
    /// during settlement. Checking and settlement share one transaction.
    pub fn finish_conversation_journey_dispatch(
        &self,
        id: Uuid,
        attempt: Uuid,
        error: Option<&str>,
    ) -> Result<bool, StorageError> {
        self.with_connection(|db| {
            let tx=db.unchecked_transaction()?;
            let resume:bool=tx.query_row("SELECT EXISTS(SELECT 1 FROM conversation_journey_dispatch d JOIN conversation_journey_answer_continuations a ON a.consent_id=d.consent_id JOIN conversation_journey_consents c ON c.id=d.consent_id WHERE d.consent_id=?1 AND d.attempt_id=?2 AND d.status='running' AND c.revoked=0 AND NOT EXISTS(SELECT 1 FROM conversation_journey_schema_resolution r WHERE r.consent_id=d.consent_id))",params![id.to_string(),attempt.to_string()],|row|row.get(0))?;
            if resume && error.is_none() {tx.commit()?;return Ok(true);}
            tx.execute("UPDATE conversation_journey_dispatch SET status='settled',error=?3,updated_at=?4 WHERE consent_id=?1 AND attempt_id=?2 AND status='running'",params![id.to_string(),attempt.to_string(),error,Utc::now().to_rfc3339()])?;
            tx.commit()?;Ok(false)
        })
    }
    pub fn queue_conversation_journey_answer(
        &self,
        project: &str,
        conversation: Uuid,
        task: Uuid,
        id: Uuid,
        schema: Uuid,
    ) -> Result<(), StorageError> {
        self.with_connection(|db|{
            let tx=db.unchecked_transaction()?;
            owned(&tx,project,conversation,task)?;
            let saved=read(&tx,task,id)?.ok_or_else(||invalid("Journey consent not found"))?;
            if saved.revoked || saved.consent.expires_at<=Utc::now() || !saved.consent.continue_after_clarification {return Err(invalid("Answer saved; journey permission is absent, expired or revoked"));}
            let proposal=saved.consent.schema_proposal.as_ref().ok_or_else(||invalid("Journey has no initial clarification"))?;
            let question=crate::conversation_clarifications::read(&tx,project,task,proposal.call_id)?;
            if question.status!="applied" || question.schema_draft_id!=Some(schema) {return Err(invalid("Journey continuation requires its exact saved clarification answer"));}
            tx.execute("INSERT OR IGNORE INTO conversation_journey_answer_continuations(consent_id,schema_draft_id) VALUES(?1,?2)",params![id.to_string(),schema.to_string()])?;
            tx.commit()?;Ok(())
        })
    }
    pub fn recover_conversation_journey_dispatches(&self) -> Result<(), StorageError> {
        self.with_connection(|db| {
            db.execute("UPDATE conversation_journey_dispatch SET status='interrupted',error='Server restarted. Read saved child receipts before explicitly retrying; no automatic inference was started.',updated_at=?1 WHERE status='running'",[Utc::now().to_rfc3339()])?;
            Ok(())
        })
    }
    pub fn conversation_journey(
        &self,
        project: &str,
        conversation: Uuid,
        task: Uuid,
        id: Uuid,
    ) -> Result<Option<ConversationJourneyRecord>, StorageError> {
        self.with_connection(|db| {
            owned(db, project, conversation, task)?;
            read(db, task, id)
        })
    }
    /// Persist an already verified, explicitly accepted envelope. No budget is
    /// incremented here; existing task/Project call admission remains mandatory.
    pub fn save_conversation_journey(
        &self,
        project: &str,
        conversation: Uuid,
        input: &ConversationJourneyConsent,
    ) -> Result<ConversationJourneyRecord, StorageError> {
        validate(input)?;
        self.with_connection(|db|{
            let tx=db.unchecked_transaction()?;
            owned(&tx,project,conversation,input.task_id)?;
            if let Some(saved)=read(&tx,input.task_id,input.id)?{
                if saved.consent!=*input{return Err(invalid("Journey retry changed its original authorization"));}
                return Ok(saved);
            }
            let now=Utc::now();
            if input.expires_at<=now || input.expires_at>now+chrono::Duration::minutes(31){return Err(invalid("Journey consent is expired or exceeds its validity window"));}
            if let Some(expected)=&input.repair_after_answer {
                let request=crate::conversation_human_requests::read(&tx,project,expected.id)?;
                if expected.conversation_id != conversation || request.input != *expected
                    || request.status != crate::ConversationHumanRequestStatus::Pending || request.deferred {
                    return Err(invalid("Answer continuation requires its exact active pending human request"));
                }
            }
            if input.schema_proposal.is_none() {
            let revision=i64::try_from(input.schema_revision).map_err(|_|invalid("Journey Schema revision is out of range"))?;
            let definition:Option<String>=tx.query_row("SELECT r.definition_json FROM conversation_schema_drafts d JOIN conversation_schema_revisions r ON r.draft_id=d.id WHERE d.id=?1 AND d.task_id=?2 AND r.revision=?3",params![input.schema_id.to_string(),input.task_id.to_string(),revision],|row|row.get(0)).optional()?;
            if definition.as_ref().is_none_or(|value|annotagent_image_tools::sha256(value.as_bytes())!=input.schema_digest){return Err(invalid("Journey Schema snapshot is unavailable or belongs to another task"));}
            } else {
                let goal:String=tx.query_row("SELECT schema_revision FROM conversation_tasks WHERE id=?1",[input.task_id.to_string()],|row|row.get(0))?;
                if goal != input.schema_digest {return Err(invalid("Initial journey goal revision changed"));}
            }
            tx.execute("INSERT INTO conversation_journey_consents(id,task_id,input_json,created_at,builder_operation_id,sample_operation_id) VALUES(?1,?2,?3,?4,?5,?6)",params![input.id.to_string(),input.task_id.to_string(),serde_json::to_string(input)?,now.to_rfc3339(),input.builder_operation_id.to_string(),input.sample_operation_id.to_string()])?;
            tx.commit()?;
            Ok(ConversationJourneyRecord {consent:input.clone(),revoked:false,sample:None,resolved_consent:None})
        })
    }
    pub fn resolve_conversation_journey_repair(
        &self,
        project: &str,
        conversation: Uuid,
        resolved: &ConversationJourneyConsent,
    ) -> Result<ConversationJourneyRecord, StorageError> {
        validate(resolved)?;
        self.with_connection(|db| {
            let tx = db.unchecked_transaction()?;
            owned(&tx, project, conversation, resolved.task_id)?;
            let mut saved = read(&tx, resolved.task_id, resolved.id)?
                .ok_or_else(|| invalid("Journey consent not found"))?;
            if saved.revoked || saved.consent.expires_at <= Utc::now() {
                return Err(invalid("Journey consent is revoked or expired"));
            }
            if let Some(previous) = &saved.resolved_consent {
                if previous != resolved {
                    return Err(invalid("Journey answer resolution is immutable"));
                }
                return Ok(saved);
            }
            let pending = saved.consent.repair_after_answer.as_ref()
                .ok_or_else(|| invalid("Journey has no authorized pending answer"))?;
            let repair = resolved.repair.as_ref()
                .ok_or_else(|| invalid("Answer resolution requires an exact repair snapshot"))?;
            let mut expected = saved.consent.clone();
            expected.repair_after_answer = None;
            expected.repair.clone_from(&resolved.repair);
            expected.builder_scope_hash.clone_from(&resolved.builder_scope_hash);
            if expected != *resolved || repair.request_id != pending.id
                || repair.draft_id != pending.resume_checkpoint_ref.to_string() {
                return Err(invalid("Answer resolution expanded the original journey scope"));
            }
            let request = crate::conversation_human_requests::read(&tx, project, pending.id)?;
            if request.input != *pending || request.status != crate::ConversationHumanRequestStatus::Applied
                || request.resume_draft_id.as_ref() != Some(&repair.draft_id) {
                return Err(invalid("Journey requires its exact applied human answer and resume result"));
            }
            let answer = request.answer.as_ref().ok_or_else(|| invalid("Human answer is unavailable"))?;
            if answer.sample_test_id != pending.sample_test_id || answer.image_id != pending.image_id
                || Some(answer.sequence) != pending.expected_feedback_sequence.checked_add(1)
                || answer.outcome_id != pending.outcome_id || answer.addition_id != pending.addition_id {
                return Err(invalid("Human answer no longer matches the authorized subject revision"));
            }
            // The receipt, acknowledged outbox and copied plan must all point to
            // the same persisted Sandbox revision. A status string alone is not evidence.
            let valid: bool = tx.query_row(
                "SELECT EXISTS(SELECT 1 FROM conversation_resume_outbox o JOIN sample_feedback_revisions f ON f.revision_id=o.feedback_revision_id JOIN sample_plan_revisions p ON p.draft_id=o.checkpoint_ref JOIN workflow_drafts d ON d.id=p.draft_id AND d.project_id=p.project_id WHERE o.request_id=?1 AND o.task_id=?2 AND o.checkpoint_ref=?3 AND o.applied_at IS NOT NULL AND f.revision_id=?4 AND f.sample_test_id=?5 AND f.image_id=?6 AND f.sequence=(SELECT MAX(sequence) FROM sample_feedback_revisions WHERE sample_test_id=f.sample_test_id AND image_id=f.image_id) AND json(f.feedback_json)=json(?7) AND p.sample_test_id=f.sample_test_id AND json_array_length(p.feedback_json)=1 AND json(json_extract(p.feedback_json,'$[0]'))=json(?7) AND json_extract(d.draft_json,'$.revision')=?8 AND json_extract(d.draft_json,'$.content_hash')=?9)",
                params![pending.id.to_string(),pending.task_id.to_string(),repair.draft_id,answer.revision_id,pending.sample_test_id,pending.image_id,serde_json::to_string(answer)?,i64::try_from(repair.revision).map_err(|_| invalid("Repair revision is out of range"))?,repair.content_hash],
                |row| row.get(0),
            )?;
            if !valid { return Err(invalid("Repair snapshot lacks exact acknowledged Sandbox evidence")); }
            // Reuse the existing immutable resolution slot; no second execution
            // ledger, additional call budget or answer copy is created here.
            tx.execute("INSERT INTO conversation_journey_schema_resolution(consent_id,resolved_json) VALUES(?1,?2)",params![resolved.id.to_string(),serde_json::to_string(resolved)?])?;
            tx.commit()?;
            saved.resolved_consent = Some(resolved.clone());
            Ok(saved)
        })
    }

    pub fn resolve_conversation_journey_schema(
        &self,
        project: &str,
        conversation: Uuid,
        resolved: &ConversationJourneyConsent,
    ) -> Result<ConversationJourneyRecord, StorageError> {
        validate(resolved)?;
        self.with_connection(|db| {
            let tx=db.unchecked_transaction()?;
            owned(&tx,project,conversation,resolved.task_id)?;
            let mut saved=read(&tx,resolved.task_id,resolved.id)?.ok_or_else(||invalid("Journey consent not found"))?;
            if saved.revoked || saved.consent.expires_at<=Utc::now(){return Err(invalid("Journey consent is revoked or expired"));}
            if let Some(previous)=&saved.resolved_consent {
                if previous!=resolved{return Err(invalid("Journey Schema resolution is immutable"));}
                return Ok(saved);
            }
            let proposal=saved.consent.schema_proposal.as_ref().ok_or_else(||invalid("Journey already starts with a saved Schema"))?;
            let mut expected=saved.consent.clone();
            expected.schema_proposal=None;
            expected.schema_id=resolved.schema_id;expected.schema_revision=resolved.schema_revision;
            expected.schema_digest.clone_from(&resolved.schema_digest);expected.builder_scope_hash.clone_from(&resolved.builder_scope_hash);
            expected.previous_grant_id=Some(proposal.call_id);
            if expected!=*resolved || resolved.schema_revision!=1{return Err(invalid("Schema resolution expanded the original journey scope"));}
            let mut definition:Option<String>=tx.query_row("SELECT r.definition_json FROM conversation_schema_drafts d JOIN conversation_schema_revisions r ON r.draft_id=d.id JOIN conversation_model_calls c ON c.id=d.source_call_id WHERE d.id=?1 AND d.task_id=?2 AND d.source_call_id=?3 AND r.revision=1 AND c.status='completed' AND json_extract(c.evidence_json,'$.decision.Ok.decision')='draft'",params![resolved.schema_id.to_string(),resolved.task_id.to_string(),proposal.call_id.to_string()],|row|row.get(0)).optional()?;
            if definition.is_none() && saved.consent.continue_after_clarification {
                let question=crate::conversation_clarifications::read(&tx,project,resolved.task_id,proposal.call_id)?;
                if question.status=="applied" && question.schema_draft_id==Some(resolved.schema_id) {
                    definition=tx.query_row("SELECT r.definition_json FROM conversation_schema_revisions r JOIN conversation_schema_drafts d ON d.id=r.draft_id WHERE d.id=?1 AND d.task_id=?2 AND r.revision=1 AND (SELECT MAX(revision) FROM conversation_schema_revisions WHERE draft_id=d.id)=1",params![resolved.schema_id.to_string(),resolved.task_id.to_string()],|row|row.get(0)).optional()?;
                }
            }
            if definition.is_none_or(|value|annotagent_image_tools::sha256(value.as_bytes())!=resolved.schema_digest){return Err(invalid("Schema is not the authorized call's valid initial Draft"));}
            tx.execute("INSERT INTO conversation_journey_schema_resolution(consent_id,resolved_json) VALUES(?1,?2)",params![resolved.id.to_string(),serde_json::to_string(resolved)?])?;
            tx.commit()?;saved.resolved_consent=Some(resolved.clone());Ok(saved)
        })
    }
    /// Seal one concrete sample continuation. A retry cannot substitute a later
    /// Draft, a different model destination or a larger image/call scope.
    pub fn seal_conversation_journey_sample(
        &self,
        project: &str,
        conversation: Uuid,
        task: Uuid,
        id: Uuid,
        sample: &JourneySampleScope,
    ) -> Result<ConversationJourneyRecord, StorageError> {
        self.with_connection(|db| {
            let tx = db.unchecked_transaction()?;
            owned(&tx, project, conversation, task)?;
            let mut saved =
                read(&tx, task, id)?.ok_or_else(|| invalid("Journey consent not found"))?;
            if saved.revoked {
                return Err(invalid("Journey consent was revoked"));
            }
            if let Some(previous) = &saved.sample {
                if previous != sample {
                    return Err(invalid("Journey sample continuation is already sealed"));
                }
                return Ok(saved);
            }
            if saved.consent.expires_at <= Utc::now() || !fits(saved.effective_consent(), sample) {
                return Err(invalid(
                    "Generated sample scope is outside the original journey consent",
                ));
            }
            tx.execute(
                "UPDATE conversation_journey_consents SET sample_scope_json=?2 WHERE id=?1",
                params![id.to_string(), serde_json::to_string(sample)?],
            )?;
            tx.commit()?;
            saved.sample = Some(sample.clone());
            Ok(saved)
        })
    }
    pub fn revoke_conversation_journey(
        &self,
        project: &str,
        conversation: Uuid,
        task: Uuid,
        id: Uuid,
    ) -> Result<ConversationJourneyRecord, StorageError> {
        self.with_connection(|db| {
            let tx = db.unchecked_transaction()?;
            owned(&tx, project, conversation, task)?;
            let mut saved =
                read(&tx, task, id)?.ok_or_else(|| invalid("Journey consent not found"))?;
            tx.execute(
                "UPDATE conversation_journey_consents SET revoked=1 WHERE id=?1",
                [id.to_string()],
            )?;
            tx.commit()?;
            saved.revoked = true;
            Ok(saved)
        })
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    #[test]
    fn concurrent_different_drafts_cannot_replace_the_first_sample_seal() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("TEST-journey-race.db");
        let store = SqliteStore::open(&path).unwrap();
        let (project, conversation, consent, sample) = setup(&store);
        store
            .save_conversation_journey(&project, conversation, &consent)
            .unwrap();
        let barrier = std::sync::Arc::new(std::sync::Barrier::new(2));
        let mut handles = Vec::new();
        for _ in 0..2 {
            let connection = SqliteStore::open(&path).unwrap();
            let barrier = barrier.clone();
            let project = project.clone();
            let consent = consent.clone();
            let mut candidate = sample.clone();
            candidate.draft_id = Uuid::new_v4();
            handles.push(std::thread::spawn(move || {
                barrier.wait();
                connection.seal_conversation_journey_sample(
                    &project,
                    conversation,
                    consent.task_id,
                    consent.id,
                    &candidate,
                )
            }));
        }
        let winners: Vec<_> = handles
            .into_iter()
            .filter_map(|handle| handle.join().unwrap().ok())
            .collect();
        assert_eq!(winners.len(), 1);
        assert_eq!(
            Some(winners[0].clone()),
            store
                .conversation_journey(&project, conversation, consent.task_id, consent.id)
                .unwrap()
        );
        let revoked = store
            .revoke_conversation_journey(&project, conversation, consent.task_id, consent.id)
            .unwrap();
        assert!(revoked.revoked);
        assert!(
            store
                .seal_conversation_journey_sample(
                    &project,
                    conversation,
                    consent.task_id,
                    consent.id,
                    revoked.sample.as_ref().unwrap()
                )
                .is_err()
        );
    }

    pub(crate) fn setup(
        store: &SqliteStore,
    ) -> (String, Uuid, ConversationJourneyConsent, JourneySampleScope) {
        let project = Uuid::new_v4().to_string();
        let conversation = store.create_conversation(&project).unwrap();
        let message = crate::ConversationMessageInput {
            id: Uuid::new_v4(),
            text: "TEST bounded journey".into(),
            image: None,
            reference: None,
        };
        store
            .append_conversation_message(&project, conversation, &message)
            .unwrap();
        let task = crate::BeginConversationTask {
            id: Uuid::new_v4(),
            source_message_id: message.id,
            schema_revision: "a".repeat(64),
        };
        store
            .begin_conversation_task(&project, conversation, &task)
            .unwrap();
        let definition = crate::ConversationSchemaDefinition {
            goal: "TEST classify cups".into(),
            task: serde_json::from_value(
                serde_json::json!({"id":"objects","kind":"classification","labels":["cup"]}),
            )
            .unwrap(),
            boundary_rules: vec![],
        };
        let schema = store
            .create_human_conversation_schema_draft(&project, task.id, Uuid::new_v4(), &definition)
            .unwrap();
        let consent = ConversationJourneyConsent {
            repair_after_answer: None,
            repair: None,
            continue_after_clarification: false,
            schema_proposal: None,
            id: Uuid::new_v4(),
            task_id: task.id,
            builder_operation_id: Uuid::new_v4(),
            builder_model_id: Some(annotagent_core::ModelProfileId::new()),
            previous_grant_id: None,
            sample_operation_id: Uuid::new_v4(),
            builder_scope_hash: "b".repeat(64),
            schema_id: schema.id,
            schema_revision: 1,
            schema_digest: annotagent_image_tools::sha256(
                &serde_json::to_vec(&definition).unwrap(),
            ),
            images: vec![JourneyImageScope {
                image_id: Uuid::new_v4(),
                content_hash: "d".repeat(64),
            }],
            allowed_models: vec![
                JourneyModelScope {
                    model_id: "TEST-vlm".into(),
                    binding_digest: "e".repeat(64),
                },
                JourneyModelScope {
                    model_id: "TEST-segmenter".into(),
                    binding_digest: "f".repeat(64),
                },
            ],
            maximum_builder_calls: 8,
            maximum_sample_calls: 12,
            expires_at: Utc::now() + chrono::Duration::minutes(20),
            allow_unknown_cost: true,
        };
        let sample = JourneySampleScope {
            operation_id: consent.sample_operation_id,
            draft_id: Uuid::new_v4(),
            draft_revision: 2,
            authorization_fingerprint: "1".repeat(64),
            schema_id: consent.schema_id,
            schema_revision: consent.schema_revision,
            schema_digest: consent.schema_digest.clone(),
            images: consent.images.clone(),
            models: vec![consent.allowed_models[0].clone()],
            maximum_calls: 10,
        };
        (project, conversation, consent, sample)
    }

    #[test]
    fn pending_answer_permission_is_exact_persistent_and_not_executable() {
        for status in ["pending", "answered", "applied", "cancelled", "stale"] {
            let dir = tempfile::tempdir().unwrap();
            let path = dir.path().join("TEST-pending-permission.db");
            let store = SqliteStore::open(&path).unwrap();
            let (project, conversation, mut consent, sample) = setup(&store);
            let mut answer = crate::sample_feedback::tests::fixture(&store);
            let legacy = serde_json::to_value(&consent).unwrap();
            assert!(legacy.get("repair_after_answer").is_none());
            let request = crate::ConversationHumanRequestInput {
                id: Uuid::new_v4(),
                task_id: consent.task_id,
                conversation_id: conversation,
                sample_test_id: answer.sample_test_id.clone(),
                image_id: consent.images[0].image_id.to_string(),
                content_hash: consent.images[0].content_hash.clone(),
                outcome_id: Some("TEST-candidate".into()),
                addition_id: None,
                expected_feedback_sequence: 2,
                reason_code: "poor_boundary".into(),
                question: "TEST correct the boundary".into(),
                resume_checkpoint_ref: Uuid::new_v4(),
            };
            // Isolate envelope validation from the separately tested Sandbox
            // request-creation/answer transaction. No model or feedback writes.
            store.with_connection(|db| {
                db.execute("INSERT INTO conversation_human_requests(id,task_id,request_json,status,created_at) VALUES(?1,?2,?3,?4,?5)",params![request.id.to_string(),consent.task_id.to_string(),serde_json::to_string(&request)?,status,Utc::now().to_rfc3339()])?;
                Ok(())
            }).unwrap();
            consent.repair_after_answer = Some(request.clone());
            if status != "pending" {
                assert!(
                    store
                        .save_conversation_journey(&project, conversation, &consent)
                        .is_err()
                );
                continue;
            }
            for change in [
                "sequence",
                "checkpoint",
                "candidate",
                "image",
                "conversation",
            ] {
                let mut wrong = consent.clone();
                let pending = wrong.repair_after_answer.as_mut().unwrap();
                match change {
                    "sequence" => pending.expected_feedback_sequence += 1,
                    "checkpoint" => pending.resume_checkpoint_ref = Uuid::new_v4(),
                    "candidate" => pending.outcome_id = Some("TEST-other".into()),
                    "image" => pending.content_hash = "e".repeat(64),
                    _ => pending.conversation_id = Uuid::new_v4(),
                }
                assert!(
                    store
                        .save_conversation_journey(&project, conversation, &wrong)
                        .is_err(),
                    "accepted {change}"
                );
            }
            let saved = store
                .save_conversation_journey(&project, conversation, &consent)
                .unwrap();
            drop(store);
            let store = SqliteStore::open(&path).unwrap();
            assert_eq!(
                store
                    .save_conversation_journey(&project, conversation, &consent)
                    .unwrap(),
                saved
            );
            assert!(
                store
                    .claim_conversation_journey_dispatch(
                        &project,
                        conversation,
                        consent.task_id,
                        consent.id,
                        Uuid::new_v4()
                    )
                    .is_err()
            );
            assert!(
                store
                    .seal_conversation_journey_sample(
                        &project,
                        conversation,
                        consent.task_id,
                        consent.id,
                        &sample
                    )
                    .is_err()
            );
            let mut widened = consent.clone();
            widened.repair_after_answer = None;
            assert!(
                store
                    .save_conversation_journey(&project, conversation, &widened)
                    .is_err()
            );
            let mut resolved = consent.clone();
            resolved.repair_after_answer = None;
            resolved.repair = Some(ConversationBuilderRepair {
                request_id: request.id,
                draft_id: request.resume_checkpoint_ref.to_string(),
                revision: 1,
                content_hash: "c".repeat(64),
            });
            resolved.builder_scope_hash = "9".repeat(64);
            assert!(
                store
                    .resolve_conversation_journey_repair(&project, conversation, &resolved)
                    .is_err()
            );
            answer.image_id.clone_from(&request.image_id);
            answer.sequence = request.expected_feedback_sequence + 1;
            answer.outcome_id.clone_from(&request.outcome_id);
            store.with_connection(|db| {
                let json = serde_json::to_string(&answer)?;
                db.execute("INSERT INTO sample_feedback_revisions(revision_id,sample_test_id,image_id,sequence,feedback_json) VALUES(?1,?2,?3,?4,?5)",params![answer.revision_id,answer.sample_test_id,answer.image_id,i64::try_from(answer.sequence).unwrap(),json])?;
                db.execute("UPDATE conversation_human_requests SET status='applied',answer_json=?2 WHERE id=?1",params![request.id.to_string(),json])?;
                db.execute("INSERT INTO conversation_resume_outbox(request_id,task_id,checkpoint_ref,feedback_revision_id,applied_at) VALUES(?1,?2,?3,?4,?5)",params![request.id.to_string(),request.task_id.to_string(),request.resume_checkpoint_ref.to_string(),answer.revision_id,Utc::now().to_rfc3339()])?;
                db.execute("INSERT INTO conversation_resume_results(request_id,draft_id) VALUES(?1,?2)",params![request.id.to_string(),request.resume_checkpoint_ref.to_string()])?;
                db.execute("INSERT INTO sample_plan_revisions(draft_id,project_id,sample_test_id,feedback_json,created_at) VALUES(?1,?2,?3,?4,?5)",params![request.resume_checkpoint_ref.to_string(),project,answer.sample_test_id,serde_json::to_string(&vec![&answer])?,Utc::now().to_rfc3339()])?;
                db.execute("INSERT INTO workflow_drafts(id,project_id,status,draft_json,created_at,updated_at) VALUES(?1,?2,'editing',?3,?4,?4)",params![request.resume_checkpoint_ref.to_string(),project,serde_json::json!({"revision":1,"content_hash":"c".repeat(64)}).to_string(),Utc::now().to_rfc3339()])?;
                Ok(())
            }).unwrap();
            for change in [
                "calls", "model", "image", "schema", "revision", "hash", "request",
            ] {
                let mut wrong = resolved.clone();
                match change {
                    "calls" => wrong.maximum_builder_calls += 1,
                    "model" => wrong.allowed_models[0].binding_digest = "1".repeat(64),
                    "image" => wrong.images[0].content_hash = "2".repeat(64),
                    "schema" => wrong.schema_revision += 1,
                    "revision" => wrong.repair.as_mut().unwrap().revision += 1,
                    "hash" => wrong.repair.as_mut().unwrap().content_hash = "3".repeat(64),
                    _ => wrong.repair.as_mut().unwrap().request_id = Uuid::new_v4(),
                }
                assert!(
                    store
                        .resolve_conversation_journey_repair(&project, conversation, &wrong)
                        .is_err(),
                    "resolution accepted {change}"
                );
            }
            for (invalidate, restore) in [
                (
                    "UPDATE conversation_journey_consents SET revoked=1",
                    "UPDATE conversation_journey_consents SET revoked=0",
                ),
                (
                    "UPDATE conversation_resume_outbox SET applied_at=NULL",
                    "UPDATE conversation_resume_outbox SET applied_at='TEST-applied'",
                ),
                (
                    "UPDATE conversation_human_requests SET status='answered'",
                    "UPDATE conversation_human_requests SET status='applied'",
                ),
                ("UPDATE sample_plan_revisions SET feedback_json='[]'", ""),
            ] {
                store
                    .with_connection(|db| {
                        db.execute(invalidate, [])?;
                        Ok(())
                    })
                    .unwrap();
                assert!(
                    store
                        .resolve_conversation_journey_repair(&project, conversation, &resolved)
                        .is_err()
                );
                assert!(
                    store
                        .conversation_journey(&project, conversation, consent.task_id, consent.id)
                        .unwrap()
                        .unwrap()
                        .resolved_consent
                        .is_none()
                );
                store
                    .with_connection(|db| {
                        if restore.is_empty() {
                            db.execute(
                                "UPDATE sample_plan_revisions SET feedback_json=?1",
                                [serde_json::to_string(&vec![&answer])?],
                            )?;
                        } else {
                            db.execute(restore, [])?;
                        }
                        Ok(())
                    })
                    .unwrap();
            }
            let resolution = store
                .resolve_conversation_journey_repair(&project, conversation, &resolved)
                .unwrap();
            assert_eq!(resolution.consent, consent);
            assert_eq!(resolution.effective_consent(), &resolved);
            drop(store);
            let store = SqliteStore::open(&path).unwrap();
            assert_eq!(
                store
                    .resolve_conversation_journey_repair(&project, conversation, &resolved)
                    .unwrap(),
                resolution
            );
            let mut changed = resolved.clone();
            changed.builder_scope_hash = "4".repeat(64);
            assert!(
                store
                    .resolve_conversation_journey_repair(&project, conversation, &changed)
                    .is_err()
            );
            assert!(
                store
                    .claim_conversation_journey_dispatch(
                        &project,
                        conversation,
                        consent.task_id,
                        consent.id,
                        Uuid::new_v4()
                    )
                    .unwrap()
            );
        }
    }

    #[test]
    fn repair_sample_cannot_switch_drafts_or_regress_the_authorized_revision() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("TEST-repair-seal.db");
        let store = SqliteStore::open(&path).unwrap();
        let (project, conversation, mut consent, mut sample) = setup(&store);
        consent.repair = Some(ConversationBuilderRepair {
            request_id: Uuid::new_v4(),
            draft_id: sample.draft_id.to_string(),
            revision: sample.draft_revision,
            content_hash: "a".repeat(64),
        });
        store
            .save_conversation_journey(&project, conversation, &consent)
            .unwrap();
        for change in ["draft", "older revision"] {
            let mut wrong = sample.clone();
            if change == "draft" {
                wrong.draft_id = Uuid::new_v4();
            } else {
                wrong.draft_revision -= 1;
            }
            assert!(
                store
                    .seal_conversation_journey_sample(
                        &project,
                        conversation,
                        consent.task_id,
                        consent.id,
                        &wrong
                    )
                    .is_err(),
                "repair accepted {change}"
            );
            assert!(
                store
                    .conversation_journey(&project, conversation, consent.task_id, consent.id)
                    .unwrap()
                    .unwrap()
                    .sample
                    .is_none()
            );
        }
        // Builder may advance the same editable copy; its exact receipt is
        // checked by Application before this storage-level scope fence.
        sample.draft_revision += 1;
        let sealed = store
            .seal_conversation_journey_sample(
                &project,
                conversation,
                consent.task_id,
                consent.id,
                &sample,
            )
            .unwrap();
        drop(store);
        let store = SqliteStore::open(&path).unwrap();
        assert_eq!(
            store
                .seal_conversation_journey_sample(
                    &project,
                    conversation,
                    consent.task_id,
                    consent.id,
                    &sample
                )
                .unwrap(),
            sealed
        );
        sample.draft_revision += 1;
        assert!(
            store
                .seal_conversation_journey_sample(
                    &project,
                    conversation,
                    consent.task_id,
                    consent.id,
                    &sample
                )
                .is_err()
        );
    }

    #[test]
    fn repair_consent_preserves_exact_snapshot_and_never_upgrades_legacy_grants() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("TEST-repair.db");
        let store = SqliteStore::open(&path).unwrap();
        let (project, conversation, mut consent, _) = setup(&store);
        let legacy = serde_json::to_value(&consent).unwrap();
        assert!(legacy.get("repair").is_none());
        assert!(
            serde_json::from_value::<ConversationJourneyConsent>(legacy)
                .unwrap()
                .repair
                .is_none()
        );
        consent.repair = Some(ConversationBuilderRepair {
            request_id: Uuid::new_v4(),
            draft_id: Uuid::new_v4().to_string(),
            revision: 3,
            content_hash: "a".repeat(64),
        });
        let saved = store
            .save_conversation_journey(&project, conversation, &consent)
            .unwrap();
        drop(store);
        let store = SqliteStore::open(&path).unwrap();
        assert_eq!(
            store
                .save_conversation_journey(&project, conversation, &consent)
                .unwrap(),
            saved
        );
        for change in ["revision", "hash", "request", "draft", "remove"] {
            let mut changed = consent.clone();
            let repair = changed.repair.as_mut().unwrap();
            match change {
                "revision" => repair.revision += 1,
                "hash" => repair.content_hash = "b".repeat(64),
                "request" => repair.request_id = Uuid::new_v4(),
                "draft" => repair.draft_id = Uuid::new_v4().to_string(),
                _ => changed.repair = None,
            }
            assert!(
                store
                    .save_conversation_journey(&project, conversation, &changed)
                    .is_err()
            );
        }
        let mut widened = consent.clone();
        widened.id = Uuid::new_v4();
        widened.continue_after_clarification = true;
        assert!(
            store
                .save_conversation_journey(&project, conversation, &widened)
                .is_err()
        );
        assert_eq!(
            store
                .conversation_journey(&project, conversation, consent.task_id, consent.id)
                .unwrap(),
            Some(saved)
        );
    }

    #[test]
    fn clarification_resolution_requires_opt_in_link_and_unedited_answer() {
        for case in ["allowed", "legacy", "edited", "revoked"] {
            let dir = tempfile::tempdir().unwrap();
            let store = SqliteStore::open(dir.path().join("TEST-answer.db")).unwrap();
            let (project, conversation, human, _) = setup(&store);
            let call = Uuid::new_v4();
            let mut initial = human.clone();
            initial.continue_after_clarification = case != "legacy";
            initial.schema_proposal = Some(crate::ConversationSchemaAuthorization {
                call_id: call,
                model_id: human.builder_model_id.unwrap(),
                scope_hash: human.builder_scope_hash.clone(),
                expires_at: human.expires_at,
                allow_unknown_cost: true,
            });
            initial.schema_id = Uuid::nil();
            initial.schema_revision = 0;
            initial.schema_digest = "a".repeat(64);
            store
                .save_conversation_journey(&project, conversation, &initial)
                .unwrap();
            store
                .authorize_conversation_calls(
                    &project,
                    &crate::ConversationCallGrant {
                        id: call,
                        task_id: initial.task_id,
                        scope_hash: initial.builder_scope_hash.clone(),
                        maximum_calls: 1,
                        expires_at: initial.expires_at,
                    },
                )
                .unwrap();
            store
                .reserve_conversation_call(
                    &project,
                    initial.task_id,
                    call,
                    &initial.builder_scope_hash,
                    &"c".repeat(64),
                )
                .unwrap();
            store.finish_conversation_call(&project, initial.task_id, call, crate::ConversationCallStatus::Completed,
                serde_json::json!({"decision":{"Ok":{"decision":"clarify","question":"TEST labels?"}}})).unwrap();
            let mut resolved = human.clone();
            resolved.continue_after_clarification = initial.continue_after_clarification;
            resolved.previous_grant_id = Some(call);
            // A valid human Schema in the same task is insufficient without the answer link.
            assert!(
                store
                    .resolve_conversation_journey_schema(&project, conversation, &resolved)
                    .is_err()
            );
            let definition = store
                .conversation_schema_draft(&project, human.schema_id, None)
                .unwrap()
                .definition;
            let answer = store
                .create_human_schema_with_clarification(
                    &project,
                    initial.task_id,
                    Uuid::new_v4(),
                    &definition,
                    Some(&crate::SchemaClarificationRef {
                        call_id: call,
                        expected_schema_revision: "a".repeat(64),
                    }),
                )
                .unwrap();
            resolved.schema_id = answer.id;
            let attempt = Uuid::new_v4();
            if case == "allowed" {
                store
                    .claim_conversation_journey_dispatch(
                        &project,
                        conversation,
                        initial.task_id,
                        initial.id,
                        attempt,
                    )
                    .unwrap();
                assert!(
                    store
                        .queue_conversation_journey_answer(
                            &project,
                            conversation,
                            initial.task_id,
                            initial.id,
                            human.schema_id
                        )
                        .is_err()
                );
                store
                    .queue_conversation_journey_answer(
                        &project,
                        conversation,
                        initial.task_id,
                        initial.id,
                        answer.id,
                    )
                    .unwrap();
                // An answer queued while the original worker is settling retains
                // that worker. A stale worker cannot consume or settle it.
                assert!(
                    !store
                        .finish_conversation_journey_dispatch(initial.id, Uuid::new_v4(), None)
                        .unwrap()
                );
                assert!(
                    store
                        .finish_conversation_journey_dispatch(initial.id, attempt, None)
                        .unwrap()
                );
            }
            if case == "edited" {
                store
                    .revise_conversation_schema_draft(
                        &project,
                        answer.id,
                        Uuid::new_v4(),
                        1,
                        &definition,
                    )
                    .unwrap();
            }
            if case == "revoked" {
                store
                    .revoke_conversation_journey(
                        &project,
                        conversation,
                        initial.task_id,
                        initial.id,
                    )
                    .unwrap();
            }
            let result =
                store.resolve_conversation_journey_schema(&project, conversation, &resolved);
            assert_eq!(result.is_ok(), case == "allowed", "{case}: {result:?}");
            if let Ok(saved) = result {
                assert_eq!(saved.consent, initial);
                assert_eq!(saved.effective_consent(), &resolved);
                assert_eq!(
                    store
                        .resolve_conversation_journey_schema(&project, conversation, &resolved)
                        .unwrap(),
                    saved
                );
                assert!(
                    !store
                        .finish_conversation_journey_dispatch(initial.id, attempt, None)
                        .unwrap()
                );
                assert_eq!(
                    store
                        .conversation_journey_dispatch(
                            &project,
                            conversation,
                            initial.task_id,
                            initial.id
                        )
                        .unwrap()
                        .unwrap()["status"],
                    "settled"
                );
            }
            let legacy = serde_json::to_value(&human).unwrap();
            assert!(legacy.get("continue_after_clarification").is_none());
            assert!(
                !serde_json::from_value::<ConversationJourneyConsent>(legacy)
                    .unwrap()
                    .continue_after_clarification
            );
        }
    }

    #[test]
    fn initial_schema_resolution_is_owned_immutable_and_does_not_expand_consent() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("TEST-initial.db");
        let store = SqliteStore::open(&path).unwrap();
        let (project, conversation, human, _) = setup(&store);
        let call = Uuid::new_v4();
        let mut initial = human.clone();
        initial.schema_proposal = Some(crate::ConversationSchemaAuthorization {
            call_id: call,
            model_id: human.builder_model_id.unwrap(),
            scope_hash: human.builder_scope_hash.clone(),
            expires_at: human.expires_at,
            allow_unknown_cost: true,
        });
        initial.schema_id = Uuid::nil();
        initial.schema_revision = 0;
        initial.schema_digest = "a".repeat(64);
        let before = store
            .save_conversation_journey(&project, conversation, &initial)
            .unwrap();
        assert!(before.resolved_consent.is_none());
        assert_eq!(before.consent, initial);
        let mut unrelated = human.clone();
        unrelated.previous_grant_id = Some(call);
        assert!(
            store
                .resolve_conversation_journey_schema(&project, conversation, &unrelated)
                .is_err()
        );
        store
            .authorize_conversation_calls(
                &project,
                &crate::ConversationCallGrant {
                    id: call,
                    task_id: initial.task_id,
                    scope_hash: initial.builder_scope_hash.clone(),
                    maximum_calls: 1,
                    expires_at: initial.expires_at,
                },
            )
            .unwrap();
        store
            .reserve_conversation_call(
                &project,
                initial.task_id,
                call,
                &initial.builder_scope_hash,
                &"c".repeat(64),
            )
            .unwrap();
        store
            .finish_conversation_call(
                &project,
                initial.task_id,
                call,
                crate::ConversationCallStatus::Completed,
                serde_json::json!({"decision":{"Ok":{"decision":"draft"}}}),
            )
            .unwrap();
        let definition = crate::ConversationSchemaDefinition {
            goal: "TEST classify cups".into(),
            task: serde_json::from_value(
                serde_json::json!({"id":"objects","kind":"classification","labels":["cup"]}),
            )
            .unwrap(),
            boundary_rules: vec![],
        };
        let schema = store
            .create_conversation_schema_draft(&project, initial.task_id, call, &definition)
            .unwrap();
        let mut resolved = human;
        resolved.schema_id = schema.id;
        resolved.previous_grant_id = Some(call);
        resolved.builder_scope_hash = "d".repeat(64);
        let mut expanded = resolved.clone();
        expanded.maximum_sample_calls = 11;
        assert!(
            store
                .resolve_conversation_journey_schema(&project, conversation, &expanded)
                .is_err()
        );
        assert!(
            store
                .resolve_conversation_journey_schema("foreign", conversation, &resolved)
                .is_err()
        );
        let saved = store
            .resolve_conversation_journey_schema(&project, conversation, &resolved)
            .unwrap();
        assert_eq!(saved.consent, initial);
        assert_eq!(saved.effective_consent(), &resolved);
        assert_eq!(
            store
                .resolve_conversation_journey_schema(&project, conversation, &resolved)
                .unwrap(),
            saved
        );
        let mut replaced = resolved.clone();
        replaced.builder_scope_hash = "e".repeat(64);
        assert!(
            store
                .resolve_conversation_journey_schema(&project, conversation, &replaced)
                .is_err()
        );
        drop(store);
        let reopened = SqliteStore::open(path).unwrap();
        assert_eq!(
            reopened
                .conversation_journey(&project, conversation, initial.task_id, initial.id)
                .unwrap()
                .unwrap(),
            saved
        );
        assert_eq!(
            reopened
                .conversation_call_budget(&project, initial.task_id)
                .unwrap()
                .unwrap()
                .used_calls,
            1
        );
    }

    #[test]
    fn dispatch_claim_recovery_and_stale_worker_settlement_are_safe() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("TEST-dispatch.db");
        let store = SqliteStore::open(&path).unwrap();
        let (project, conversation, consent, _) = setup(&store);
        assert!(
            store
                .conversation_journey_ids(&project, conversation, consent.task_id)
                .unwrap()
                .is_empty()
        );
        store
            .save_conversation_journey(&project, conversation, &consent)
            .unwrap();
        assert_eq!(
            store
                .conversation_journey_ids(&project, conversation, consent.task_id)
                .unwrap(),
            vec![consent.id]
        );
        assert!(
            store
                .conversation_journey_ids("foreign", conversation, consent.task_id)
                .is_err()
        );
        let first = Uuid::new_v4();
        assert!(
            store
                .claim_conversation_journey_dispatch(
                    &project,
                    conversation,
                    consent.task_id,
                    consent.id,
                    first
                )
                .unwrap()
        );
        let second = SqliteStore::open(&path).unwrap();
        assert!(
            !second
                .claim_conversation_journey_dispatch(
                    &project,
                    conversation,
                    consent.task_id,
                    consent.id,
                    Uuid::new_v4()
                )
                .unwrap()
        );
        assert!(
            second
                .conversation_journey_dispatch("foreign", conversation, consent.task_id, consent.id)
                .is_err()
        );
        second.recover_conversation_journey_dispatches().unwrap();
        assert_eq!(
            second
                .conversation_journey_dispatch(&project, conversation, consent.task_id, consent.id)
                .unwrap()
                .unwrap()["status"],
            "interrupted"
        );
        let next = Uuid::new_v4();
        assert!(
            second
                .claim_conversation_journey_dispatch(
                    &project,
                    conversation,
                    consent.task_id,
                    consent.id,
                    next
                )
                .unwrap()
        );
        store
            .finish_conversation_journey_dispatch(consent.id, first, Some("obsolete worker"))
            .unwrap();
        assert_eq!(
            second
                .conversation_journey_dispatch(&project, conversation, consent.task_id, consent.id)
                .unwrap()
                .unwrap()["status"],
            "running"
        );
        second
            .finish_conversation_journey_dispatch(consent.id, next, Some("TEST failure"))
            .unwrap();
        let saved = second
            .conversation_journey_dispatch(&project, conversation, consent.task_id, consent.id)
            .unwrap()
            .unwrap();
        assert_eq!(saved["status"], "settled");
        assert_eq!(saved["error"], "TEST failure");
        second
            .revoke_conversation_journey(&project, conversation, consent.task_id, consent.id)
            .unwrap();
        assert!(
            second
                .claim_conversation_journey_dispatch(
                    &project,
                    conversation,
                    consent.task_id,
                    consent.id,
                    Uuid::new_v4()
                )
                .is_err()
        );
    }

    #[test]
    fn immutable_consent_and_sample_seal_survive_restart_without_granting_calls() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("TEST-journey.db");
        let store = SqliteStore::open(&path).unwrap();
        let (project, conversation, consent, sample) = setup(&store);
        assert!(
            store
                .conversation_journey(&project, conversation, consent.task_id, consent.id)
                .unwrap()
                .is_none()
        );
        let saved = store
            .save_conversation_journey(&project, conversation, &consent)
            .unwrap();
        assert_eq!(
            saved,
            store
                .save_conversation_journey(&project, conversation, &consent)
                .unwrap()
        );
        let mut changed = consent.clone();
        changed.id = Uuid::new_v4();
        assert!(
            store
                .save_conversation_journey(&project, conversation, &changed)
                .is_err()
        );
        changed = consent.clone();
        changed.expires_at += chrono::Duration::minutes(1);
        assert!(
            store
                .save_conversation_journey(&project, conversation, &changed)
                .is_err()
        );
        changed = consent.clone();
        changed.maximum_sample_calls -= 1;
        assert!(
            store
                .save_conversation_journey(&project, conversation, &changed)
                .is_err()
        );
        let sealed = store
            .seal_conversation_journey_sample(
                &project,
                conversation,
                consent.task_id,
                consent.id,
                &sample,
            )
            .unwrap();
        drop(store);
        let store = SqliteStore::open(&path).unwrap();
        assert_eq!(
            Some(sealed.clone()),
            store
                .conversation_journey(&project, conversation, consent.task_id, consent.id)
                .unwrap()
        );
        assert_eq!(
            sealed,
            store
                .seal_conversation_journey_sample(
                    &project,
                    conversation,
                    consent.task_id,
                    consent.id,
                    &sample
                )
                .unwrap()
        );
        assert_eq!(
            sealed,
            store
                .save_conversation_journey(&project, conversation, &consent)
                .unwrap()
        );
        let mut changed = sample.clone();
        changed.draft_revision += 1;
        assert!(
            store
                .seal_conversation_journey_sample(
                    &project,
                    conversation,
                    consent.task_id,
                    consent.id,
                    &changed
                )
                .is_err()
        );
        assert_eq!(
            store
                .conversation_task_budget(&project, consent.task_id)
                .unwrap()
                .total_authorized_calls,
            0
        );
        assert!(
            store
                .conversation_call_budget(&project, consent.task_id)
                .unwrap()
                .is_none()
        );
    }

    #[test]
    fn reject_scope_expansion_and_invalid_limits_before_sealing() {
        let store = SqliteStore::open_in_memory().unwrap();
        let (project, conversation, consent, sample) = setup(&store);
        let mut invalid_inputs = Vec::new();
        let mut value = consent.clone();
        value.schema_revision = u64::MAX;
        invalid_inputs.push(value);
        let mut value = consent.clone();
        value.schema_id = Uuid::new_v4();
        invalid_inputs.push(value);
        let mut value = consent.clone();
        value.schema_digest = "0".repeat(64);
        invalid_inputs.push(value);
        let mut value = consent.clone();
        value.allow_unknown_cost = false;
        invalid_inputs.push(value);
        let mut value = consent.clone();
        value.expires_at = Utc::now() - chrono::Duration::seconds(1);
        invalid_inputs.push(value);
        let mut value = consent.clone();
        value.expires_at = Utc::now() + chrono::Duration::hours(2);
        invalid_inputs.push(value);
        let mut value = consent.clone();
        value.images.push(value.images[0].clone());
        invalid_inputs.push(value);
        let mut value = consent.clone();
        value.allowed_models.push(value.allowed_models[0].clone());
        invalid_inputs.push(value);
        let mut value = consent.clone();
        value.maximum_builder_calls = 17;
        invalid_inputs.push(value);
        let mut value = consent.clone();
        value.maximum_sample_calls = 13;
        invalid_inputs.push(value);
        for value in invalid_inputs {
            assert!(
                store
                    .save_conversation_journey(&project, conversation, &value)
                    .is_err()
            );
        }
        store
            .save_conversation_journey(&project, conversation, &consent)
            .unwrap();
        let mut outside = Vec::new();
        let mut value = sample.clone();
        value.models[0].binding_digest = "a".repeat(64);
        outside.push(value);
        let mut value = sample.clone();
        value.models[0].model_id = "another-recipient".into();
        outside.push(value);
        let mut value = sample.clone();
        value.images[0].content_hash = "a".repeat(64);
        outside.push(value);
        let mut value = sample.clone();
        value.images.push(JourneyImageScope {
            image_id: Uuid::new_v4(),
            content_hash: "d".repeat(64),
        });
        outside.push(value);
        let mut value = sample.clone();
        value.schema_revision += 1;
        outside.push(value);
        let mut value = sample.clone();
        value.schema_digest = "a".repeat(64);
        outside.push(value);
        let mut value = sample.clone();
        value.operation_id = Uuid::new_v4();
        outside.push(value);
        let mut value = sample.clone();
        value.maximum_calls = 13;
        outside.push(value);
        let mut value = sample.clone();
        value.models.push(value.models[0].clone());
        outside.push(value);
        for value in outside {
            assert!(
                store
                    .seal_conversation_journey_sample(
                        &project,
                        conversation,
                        consent.task_id,
                        consent.id,
                        &value
                    )
                    .is_err()
            );
        }
        assert!(
            store
                .conversation_journey(&project, conversation, consent.task_id, consent.id)
                .unwrap()
                .unwrap()
                .sample
                .is_none()
        );
        store
            .seal_conversation_journey_sample(
                &project,
                conversation,
                consent.task_id,
                consent.id,
                &sample,
            )
            .unwrap();
    }

    #[test]
    fn ownership_and_revocation_apply_to_reads_and_late_continuations() {
        let store = SqliteStore::open_in_memory().unwrap();
        let (project, conversation, consent, sample) = setup(&store);
        store
            .save_conversation_journey(&project, conversation, &consent)
            .unwrap();
        assert!(
            store
                .save_conversation_journey("foreign", conversation, &consent)
                .is_err()
        );
        assert!(
            store
                .conversation_journey("foreign", conversation, consent.task_id, consent.id)
                .is_err()
        );
        assert!(
            store
                .conversation_journey(&project, Uuid::new_v4(), consent.task_id, consent.id)
                .is_err()
        );
        assert!(
            store
                .revoke_conversation_journey("foreign", conversation, consent.task_id, consent.id)
                .is_err()
        );
        let revoked = store
            .revoke_conversation_journey(&project, conversation, consent.task_id, consent.id)
            .unwrap();
        assert!(revoked.revoked);
        assert_eq!(
            revoked,
            store
                .save_conversation_journey(&project, conversation, &consent)
                .unwrap()
        );
        assert_eq!(
            revoked,
            store
                .revoke_conversation_journey(&project, conversation, consent.task_id, consent.id)
                .unwrap()
        );
        assert!(
            store
                .seal_conversation_journey_sample(
                    &project,
                    conversation,
                    consent.task_id,
                    consent.id,
                    &sample
                )
                .is_err()
        );
    }
}
