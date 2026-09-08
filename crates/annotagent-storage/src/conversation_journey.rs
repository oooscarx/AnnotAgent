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
    pub id: Uuid,
    pub task_id: Uuid,
    pub builder_operation_id: Uuid,
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
}

fn invalid(message: &str) -> StorageError {
    StorageError::InvalidConversation(message.into())
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
        input.schema_id,
    ];
    if ids.iter().any(Uuid::is_nil)
        || input.builder_operation_id == input.sample_operation_id
        || input.schema_revision == 0
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
        })
    })
    .transpose()
}

impl SqliteStore {
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
            let revision=i64::try_from(input.schema_revision).map_err(|_|invalid("Journey Schema revision is out of range"))?;
            let definition:Option<String>=tx.query_row("SELECT r.definition_json FROM conversation_schema_drafts d JOIN conversation_schema_revisions r ON r.draft_id=d.id WHERE d.id=?1 AND d.task_id=?2 AND r.revision=?3",params![input.schema_id.to_string(),input.task_id.to_string(),revision],|row|row.get(0)).optional()?;
            if definition.as_ref().is_none_or(|value|annotagent_image_tools::sha256(value.as_bytes())!=input.schema_digest){return Err(invalid("Journey Schema snapshot is unavailable or belongs to another task"));}
            tx.execute("INSERT INTO conversation_journey_consents(id,task_id,input_json,created_at,builder_operation_id,sample_operation_id) VALUES(?1,?2,?3,?4,?5,?6)",params![input.id.to_string(),input.task_id.to_string(),serde_json::to_string(input)?,now.to_rfc3339(),input.builder_operation_id.to_string(),input.sample_operation_id.to_string()])?;
            tx.commit()?;
            Ok(ConversationJourneyRecord {consent:input.clone(),revoked:false,sample:None})
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
            if saved.consent.expires_at <= Utc::now() || !fits(&saved.consent, sample) {
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
mod tests {
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

    fn setup(
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
            id: Uuid::new_v4(),
            task_id: task.id,
            builder_operation_id: Uuid::new_v4(),
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
