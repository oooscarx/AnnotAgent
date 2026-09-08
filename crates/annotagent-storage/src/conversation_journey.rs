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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ConversationJourneyConsent {
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
    sample.operation_id == consent.sample_operation_id
        && !sample.draft_id.is_nil()
        && sample.draft_revision > 0
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
