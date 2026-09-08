//! Immutable text-feedback consent; execution uses the existing shared call ledger.
use crate::{
    ConversationCallGrant, ConversationMessage, ConversationMessageInput, ConversationSelectionRef,
    SqliteStore, StorageError,
};
use chrono::{DateTime, Utc};
use rusqlite::{Connection, OptionalExtension, params};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ConversationFeedbackAuthorization {
    pub call_id: Uuid,
    pub message_id: Uuid,
    pub model_id: annotagent_core::ModelProfileId,
    pub previous_grant_id: Option<Uuid>,
    pub scope_hash: String,
    pub expires_at: DateTime<Utc>,
    pub allow_unknown_cost: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ConversationFeedbackAuthorizationRecord {
    pub consent: ConversationFeedbackAuthorization,
    /// Application validates live pixels and terminal Artifact identity. Storage
    /// binds the embedded saved message and optimistic image feedback sequence.
    pub context: Value,
    /// Server-resolved display metadata; never refresh old consent from new settings.
    pub summary: Value,
    pub grant: ConversationCallGrant,
}

fn invalid(message: &str) -> StorageError {
    StorageError::InvalidConversation(message.into())
}

pub(crate) fn owned(db: &Connection, project: &str, task: Uuid) -> Result<Uuid, StorageError> {
    let conversation: Option<String> = db.query_row(
        "SELECT t.conversation_id FROM conversation_tasks t JOIN project_conversations c ON c.id=t.conversation_id WHERE t.id=?1 AND c.project_id=?2",
        params![task.to_string(), project], |row| row.get(0),
    ).optional()?;
    Uuid::parse_str(
        &conversation.ok_or_else(|| invalid("Feedback task belongs to another Project"))?,
    )
    .map_err(|_| invalid("Invalid saved feedback conversation"))
}

pub(crate) fn read(
    db: &Connection,
    task: Uuid,
    call: Uuid,
) -> Result<Option<ConversationFeedbackAuthorizationRecord>, StorageError> {
    let row: Option<(String, String)> = db
        .query_row(
            "SELECT task_id,record_json FROM conversation_feedback_authorizations WHERE call_id=?1",
            [call.to_string()],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()?;
    row.map(|(owner, json)| {
        if owner != task.to_string() {
            return Err(invalid("Feedback authorization belongs to another task"));
        }
        Ok(serde_json::from_str(&json)?)
    })
    .transpose()
}

fn validate_shape(record: &ConversationFeedbackAuthorizationRecord) -> Result<(), StorageError> {
    let consent = &record.consent;
    if consent.call_id.is_nil()
        || consent.message_id.is_nil()
        || consent.model_id.0.is_nil()
        || consent
            .previous_grant_id
            .is_some_and(|id| id.is_nil() || id == consent.call_id)
        || record.grant.task_id.is_nil()
        || record.grant.id != consent.call_id
        || record.grant.scope_hash != consent.scope_hash
        || record.grant.expires_at != consent.expires_at
        || !consent.allow_unknown_cost
        || !record.context.is_object()
        || !record.summary.is_object()
        || serde_json::to_vec(&record.context)?.len() > 65_536
        || serde_json::to_vec(&record.summary)?.len() > 8_192
    {
        return Err(invalid("Invalid bounded feedback authorization record"));
    }
    Ok(())
}

impl SqliteStore {
    pub fn conversation_feedback_authorization(
        &self,
        project: &str,
        task: Uuid,
        call: Uuid,
    ) -> Result<Option<ConversationFeedbackAuthorizationRecord>, StorageError> {
        self.with_connection(|db| {
            owned(db, project, task)?;
            read(db, task, call)
        })
    }

    pub fn conversation_feedback_for_message(
        &self,
        project: &str,
        task: Uuid,
        message: Uuid,
    ) -> Result<Option<ConversationFeedbackAuthorizationRecord>, StorageError> {
        self.with_connection(|db| {
            owned(db, project, task)?;
            let call: Option<String> = db.query_row(
                "SELECT call_id FROM conversation_feedback_authorizations WHERE task_id=?1 AND message_id=?2",
                params![task.to_string(), message.to_string()], |row| row.get(0),
            ).optional()?;
            call.map(|id| {
                read(db, task, Uuid::parse_str(&id).map_err(|_| invalid("Invalid saved feedback call"))?)
            }).transpose().map(Option::flatten)
        })
    }

    /// Saves the original consent and cumulative grant in one transaction. A row
    /// without a call receipt is authorized but not started, never an automatic job.
    pub fn authorize_conversation_feedback(
        &self,
        project: &str,
        conversation: Uuid,
        task: Uuid,
        record: &ConversationFeedbackAuthorizationRecord,
    ) -> Result<ConversationFeedbackAuthorizationRecord, StorageError> {
        validate_shape(record)?;
        if record.grant.task_id != task {
            return Err(invalid("Feedback grant does not match the route task"));
        }
        let persist = |tx: &rusqlite::Transaction<'_>, created_grant: bool| {
            if owned(tx, project, task)? != conversation {
                return Err(invalid("Feedback task belongs to another conversation"));
            }
            if let Some(saved) = read(tx, task, record.consent.call_id)? {
                if saved != *record {
                    return Err(invalid(
                        "Feedback authorization retry changed its frozen record",
                    ));
                }
                return Ok(());
            }
            if !created_grant {
                return Err(invalid(
                    "Feedback call identity belongs to another authorization",
                ));
            }
            let collision: bool = tx.query_row(
                "SELECT EXISTS(SELECT 1 FROM conversation_feedback_authorizations WHERE conversation_id=?1 AND message_id=?2)",
                params![conversation.to_string(), record.consent.message_id.to_string()], |row| row.get(0),
            )?;
            if collision {
                return Err(invalid(
                    "This saved message already has a feedback authorization; restore its original call",
                ));
            }
            let saved: Option<(i64, String)> = tx.query_row(
                "SELECT sequence,input_json FROM conversation_messages WHERE conversation_id=?1 AND message_id=?2",
                params![conversation.to_string(),record.consent.message_id.to_string()],
                |row| Ok((row.get(0)?, row.get(1)?)),
            ).optional()?;
            let (sequence, input) = saved
                .ok_or_else(|| invalid("Feedback requires a saved message in this conversation"))?;
            let input: ConversationMessageInput = serde_json::from_str(&input)?;
            let Some(ConversationSelectionRef::SampleCandidate {
                task_id,
                project_schema_revision,
                sample_test_id,
                ..
            }) = &input.reference
            else {
                return Err(invalid(
                    "Feedback authorization requires an explicit saved candidate reference",
                ));
            };
            let task_revision: String = tx.query_row(
                "SELECT schema_revision FROM conversation_tasks WHERE id=?1",
                [task.to_string()],
                |row| row.get(0),
            )?;
            if *task_id != task || *project_schema_revision != task_revision {
                return Err(invalid(
                    "Feedback message refers to another task or Schema revision",
                ));
            }
            let frozen: ConversationMessage =
                serde_json::from_value(record.context["message"].clone())?;
            if frozen
                != (ConversationMessage {
                    conversation_id: conversation,
                    sequence,
                    input: input.clone(),
                })
            {
                return Err(invalid("Feedback context does not match its saved message"));
            }
            if record.context["pixels_supplied"] != false {
                return Err(invalid(
                    "Feedback interpretation context must remain text only",
                ));
            }
            let image = input
                .image
                .as_ref()
                .ok_or_else(|| invalid("Feedback message lacks its frozen image"))?;
            let current_hash: Option<String> = tx
                .query_row(
                    "SELECT sha256 FROM images WHERE id=?1 AND project_id=?2",
                    params![image.image_id, project],
                    |row| row.get(0),
                )
                .optional()?;
            if current_hash.as_deref() != Some(image.sha256.as_str()) {
                return Err(invalid(
                    "Feedback image was removed or changed before authorization",
                ));
            }
            let sequence: i64 = tx.query_row("SELECT COALESCE(MAX(sequence),0) FROM sample_feedback_revisions WHERE sample_test_id=?1 AND image_id=?2",params![sample_test_id,image.image_id],|row|row.get(0))?;
            let sequence =
                u64::try_from(sequence).map_err(|_| invalid("Invalid saved feedback sequence"))?;
            if record.context["expected_feedback_sequence"].as_u64() != Some(sequence) {
                return Err(invalid("Feedback changed before authorization was saved"));
            }
            if record.consent.expires_at <= Utc::now()
                || record.consent.expires_at > Utc::now() + chrono::Duration::minutes(31)
            {
                return Err(invalid(
                    "Feedback authorization expiry is outside the bounded window",
                ));
            }
            let previous_maximum: u32 = if let Some(previous) = record.consent.previous_grant_id {
                tx.query_row("SELECT maximum_calls FROM conversation_authorization_revisions WHERE id=?1 AND task_id=?2",params![previous.to_string(),task.to_string()],|row|row.get(0))?
            } else {
                0
            };
            if record.grant.maximum_calls != previous_maximum.saturating_add(1).min(128) {
                return Err(invalid(
                    "Feedback consent must preserve the cumulative ceiling and add at most one call",
                ));
            }
            let used: u32 = tx.query_row(
                "SELECT COUNT(*) FROM conversation_model_calls WHERE task_id=?1",
                [task.to_string()],
                |row| row.get(0),
            )?;
            if used >= record.grant.maximum_calls {
                return Err(invalid("Task cumulative model-call allowance exhausted"));
            }
            crate::conversation_calls::require_call_admission_clear(
                tx,
                task,
                record.consent.call_id,
            )?;
            crate::conversation_project_budget::admit(tx, project)?;
            let used_id: bool = tx.query_row("SELECT EXISTS(SELECT 1 FROM conversation_model_calls WHERE id=?1 UNION ALL SELECT 1 FROM conversation_builder_operations WHERE id=?1)",[record.consent.call_id.to_string()],|row|row.get(0))?;
            if used_id {
                return Err(invalid(
                    "Feedback operation identity already belongs to another operation",
                ));
            }
            tx.execute("INSERT INTO conversation_feedback_authorizations(call_id,task_id,conversation_id,message_id,record_json) VALUES(?1,?2,?3,?4,?5)",params![record.consent.call_id.to_string(),task.to_string(),conversation.to_string(),record.consent.message_id.to_string(),serde_json::to_string(record)?])?;
            Ok(())
        };
        if let Some(previous) = record.consent.previous_grant_id {
            self.advance_conversation_authorization_with(
                project,
                previous,
                &record.grant,
                persist,
            )?;
        } else {
            self.authorize_initial_request_with(project, &record.grant, None, persist)?;
        }
        Ok(record.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        BeginConversationTask, ConversationCallAdmission, ConversationCallStatus,
        ConversationImageRef,
    };
    use serde_json::json;

    fn message_record(
        store: &SqliteStore,
        project: &str,
        conversation: Uuid,
        task: Uuid,
        previous: Option<&ConversationCallGrant>,
    ) -> ConversationFeedbackAuthorizationRecord {
        let image = store
            .ensure_project_image(project.parse().unwrap(), "TEST.png", "TEST-pixels", "{}")
            .unwrap()
            .image_id;
        let input = ConversationMessageInput {
            id: Uuid::new_v4(),
            text: "this box is too big".into(),
            image: Some(ConversationImageRef {
                image_id: image.to_string(),
                sha256: "TEST-pixels".into(),
            }),
            reference: Some(ConversationSelectionRef::SampleCandidate {
                task_id: task,
                project_schema_revision: "a".repeat(64),
                draft_id: "TEST-draft".into(),
                draft_revision: 1,
                sample_test_id: "TEST-sample".into(),
                candidate_id: "TEST-candidate".into(),
                source_artifact_id: Uuid::new_v4(),
            }),
        };
        let message = store
            .append_conversation_message(project, conversation, &input)
            .unwrap();
        let call = Uuid::new_v4();
        let expiry = Utc::now() + chrono::Duration::minutes(10);
        let scope_hash = format!("{}{}", call.simple(), call.simple());
        ConversationFeedbackAuthorizationRecord {
            consent: ConversationFeedbackAuthorization {
                call_id: call,
                message_id: input.id,
                model_id: annotagent_core::ModelProfileId::new(),
                previous_grant_id: previous.map(|grant| grant.id),
                scope_hash: scope_hash.clone(),
                expires_at: expiry,
                allow_unknown_cost: true,
            },
            context: json!({"message":message,"candidate":{"outcome":{"id":"TEST-candidate","value":{"kind":"bounding_box"}}},"expected_feedback_sequence":0,"pixels_supplied":false}),
            summary: json!({"model_name":"TEST configured model","remote_model":"TEST model","destination":"TEST endpoint","data_scope":"Saved message and candidate metadata","operation":"Propose a human question","maximum_output_tokens":2048}),
            grant: ConversationCallGrant {
                id: call,
                task_id: task,
                scope_hash,
                maximum_calls: previous
                    .map_or(1, |grant| grant.maximum_calls.saturating_add(1).min(128)),
                expires_at: expiry,
            },
        }
    }

    fn setup(store: &SqliteStore) -> (String, Uuid, Uuid, ConversationFeedbackAuthorizationRecord) {
        let project = Uuid::new_v4().to_string();
        let conversation = store.create_conversation(&project).unwrap();
        let message = ConversationMessageInput {
            id: Uuid::new_v4(),
            text: "TEST goal".into(),
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
        let record = message_record(store, &project, conversation, task, None);
        (project, conversation, task, record)
    }

    #[test]
    fn authorization_and_original_summary_survive_restart_without_inference() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("TEST-feedback.sqlite");
        let store = SqliteStore::open(&path).unwrap();
        let (project, conversation, task, record) = setup(&store);
        assert_eq!(
            store
                .authorize_conversation_feedback(&project, conversation, task, &record)
                .unwrap(),
            record
        );
        assert_eq!(
            store
                .conversation_call_budget(&project, task)
                .unwrap()
                .unwrap()
                .current_grant,
            record.grant
        );
        assert!(
            store
                .conversation_call_history(&project, task)
                .unwrap()
                .is_empty()
        );
        drop(store);
        let store = SqliteStore::open(&path).unwrap();
        assert_eq!(
            store
                .conversation_feedback_authorization(&project, task, record.consent.call_id)
                .unwrap(),
            Some(record.clone())
        );
        assert_eq!(
            store
                .conversation_feedback_for_message(&project, task, record.consent.message_id)
                .unwrap(),
            Some(record.clone())
        );
        store
            .with_connection(|db| {
                db.execute(
                    "UPDATE images SET sha256='TEST changed pixels' WHERE project_id=?1",
                    [&project],
                )?;
                Ok(())
            })
            .unwrap();
        store
            .request_conversation_call_cancel(&project, task, record.consent.call_id)
            .unwrap();
        assert_eq!(
            store
                .authorize_conversation_feedback(&project, conversation, task, &record)
                .unwrap(),
            record
        );
        assert!(
            store
                .conversation_call_history(&project, task)
                .unwrap()
                .is_empty()
        );
        assert_eq!(
            store
                .conversation_call_budget(&project, task)
                .unwrap()
                .unwrap()
                .used_calls,
            0
        );
    }

    #[test]
    fn foreign_identity_changed_context_and_same_message_new_call_are_rejected() {
        let store = SqliteStore::open_in_memory().unwrap();
        let (project, conversation, task, record) = setup(&store);
        assert!(
            store
                .authorize_conversation_feedback("foreign", conversation, task, &record)
                .is_err()
        );
        assert!(
            store
                .authorize_conversation_feedback(&project, Uuid::new_v4(), task, &record)
                .is_err()
        );
        assert!(
            store
                .authorize_conversation_feedback(&project, conversation, Uuid::new_v4(), &record)
                .is_err()
        );
        for kind in 0..5 {
            let mut invalid = record.clone();
            match kind {
                0 => {
                    invalid.context["message"]["input"]["text"] = json!("TEST substituted message");
                }
                1 => invalid.context["expected_feedback_sequence"] = json!(1),
                2 => invalid.context["pixels_supplied"] = json!(true),
                3 => invalid.consent.allow_unknown_cost = false,
                _ => invalid.summary = json!("TEST missing display metadata"),
            }
            assert!(
                store
                    .authorize_conversation_feedback(&project, conversation, task, &invalid)
                    .is_err()
            );
            assert!(
                store
                    .conversation_call_budget(&project, task)
                    .unwrap()
                    .is_none()
            );
        }
        store
            .authorize_conversation_feedback(&project, conversation, task, &record)
            .unwrap();
        let before = store.conversation_call_budget(&project, task).unwrap();
        let mut changed = record.clone();
        changed.summary["destination"] = json!("TEST different endpoint");
        assert!(
            store
                .authorize_conversation_feedback(&project, conversation, task, &changed)
                .is_err()
        );
        let mut duplicate = record.clone();
        duplicate.consent.call_id = Uuid::new_v4();
        duplicate.grant.id = duplicate.consent.call_id;
        duplicate.consent.previous_grant_id = Some(record.grant.id);
        duplicate.grant.maximum_calls = 2;
        assert!(
            store
                .authorize_conversation_feedback(&project, conversation, task, &duplicate)
                .is_err()
        );
        assert_eq!(
            store.conversation_call_budget(&project, task).unwrap(),
            before
        );
        assert!(
            store
                .conversation_feedback_authorization(&project, task, duplicate.consent.call_id)
                .unwrap()
                .is_none()
        );
        assert!(
            store
                .conversation_feedback_authorization("foreign", task, record.consent.call_id)
                .is_err()
        );
    }

    #[test]
    fn pending_humans_deferred_work_cancellation_and_record_failure_roll_back_grant() {
        for blocker in ["pending", "deferred", "cancelled", "record_failure"] {
            let store = SqliteStore::open_in_memory().unwrap();
            let (project, conversation, task, record) = setup(&store);
            let human = Uuid::new_v4();
            match blocker {
                "pending"|"deferred" => store.with_connection(|db| {
                    db.execute("INSERT INTO conversation_human_requests(id,task_id,request_json,status,created_at) VALUES(?1,?2,'{}','pending',?3)",params![human.to_string(),task.to_string(),Utc::now().to_rfc3339()])?;
                    if blocker=="deferred" {
                        db.execute("INSERT INTO conversation_human_deferrals(command_id,request_id,revision,deferred) VALUES(?1,?2,1,1)",params![Uuid::new_v4().to_string(),human.to_string()])?;
                    }
                    Ok(())
                }).unwrap(),
                "cancelled" => {store.request_conversation_call_cancel(&project,task,record.consent.call_id).unwrap();},
                _ => store.with_connection(|db|{db.execute_batch("CREATE TRIGGER TEST_reject_feedback BEFORE INSERT ON conversation_feedback_authorizations BEGIN SELECT RAISE(ABORT,'TEST atomic row failure'); END;")?;Ok(())}).unwrap(),
            }
            assert!(
                store
                    .authorize_conversation_feedback(&project, conversation, task, &record)
                    .is_err(),
                "accepted {blocker}"
            );
            assert!(
                store
                    .conversation_call_budget(&project, task)
                    .unwrap()
                    .is_none()
            );
            assert!(
                store
                    .conversation_feedback_authorization(&project, task, record.consent.call_id)
                    .unwrap()
                    .is_none()
            );
            assert!(
                store
                    .conversation_call_history(&project, task)
                    .unwrap()
                    .is_empty()
            );
        }
    }

    #[test]
    fn cumulative_allowance_preserves_spent_calls_and_historical_retry_cannot_reactivate_scope() {
        let store = SqliteStore::open_in_memory().unwrap();
        let (project, conversation, task, first) = setup(&store);
        let baseline = ConversationCallGrant {
            id: Uuid::new_v4(),
            task_id: task,
            scope_hash: "a".repeat(64),
            maximum_calls: 5,
            expires_at: Utc::now() + chrono::Duration::minutes(10),
        };
        store
            .authorize_conversation_calls(&project, &baseline)
            .unwrap();
        let call = Uuid::new_v4();
        assert_eq!(
            store
                .reserve_conversation_call(
                    &project,
                    task,
                    call,
                    &baseline.scope_hash,
                    &"a".repeat(64)
                )
                .unwrap(),
            ConversationCallAdmission::Admitted
        );
        let mut next = first;
        next.consent.previous_grant_id = Some(baseline.id);
        next.grant.maximum_calls = 6;
        assert!(
            store
                .authorize_conversation_feedback(&project, conversation, task, &next)
                .is_err(),
            "active calls must block changing scope"
        );
        store
            .finish_conversation_call(
                &project,
                task,
                call,
                ConversationCallStatus::InDoubt,
                json!({"error":"TEST unknown paid outcome"}),
            )
            .unwrap();
        let mut lowered = next.clone();
        lowered.grant.maximum_calls = 2;
        assert!(
            store
                .authorize_conversation_feedback(&project, conversation, task, &lowered)
                .is_err()
        );
        store
            .authorize_conversation_feedback(&project, conversation, task, &next)
            .unwrap();
        let budget = store
            .conversation_call_budget(&project, task)
            .unwrap()
            .unwrap();
        assert_eq!(budget.used_calls, 1);
        assert_eq!(budget.current_grant.maximum_calls, 6);
        let later = message_record(&store, &project, conversation, task, Some(&next.grant));
        store
            .authorize_conversation_feedback(&project, conversation, task, &later)
            .unwrap();
        assert_eq!(
            store
                .authorize_conversation_feedback(&project, conversation, task, &next)
                .unwrap(),
            next
        );
        let budget = store
            .conversation_call_budget(&project, task)
            .unwrap()
            .unwrap();
        assert_eq!(budget.current_grant, later.grant);
        assert_eq!(budget.used_calls, 1);
        assert!(
            store
                .reserve_conversation_call(
                    &project,
                    task,
                    next.consent.call_id,
                    &next.grant.scope_hash,
                    &"b".repeat(64)
                )
                .is_err()
        );
        assert!(
            store
                .conversation_feedback_authorization(&project, task, next.consent.call_id)
                .unwrap()
                .is_some()
        );
    }

    #[test]
    fn another_task_cannot_cancel_saved_feedback_before_a_call_receipt_exists() {
        let store = SqliteStore::open_in_memory().unwrap();
        let (project, conversation, task, record) = setup(&store);
        store
            .authorize_conversation_feedback(&project, conversation, task, &record)
            .unwrap();
        let other_goal = ConversationMessageInput {
            id: Uuid::new_v4(),
            text: "TEST another task".into(),
            image: None,
            reference: None,
        };
        store
            .append_conversation_message(&project, conversation, &other_goal)
            .unwrap();
        let other_task = Uuid::new_v4();
        store
            .begin_conversation_task(
                &project,
                conversation,
                &BeginConversationTask {
                    id: other_task,
                    source_message_id: other_goal.id,
                    schema_revision: "a".repeat(64),
                },
            )
            .unwrap();
        assert!(
            store
                .conversation_call(&project, task, record.consent.call_id)
                .unwrap()
                .is_none()
        );
        assert!(
            store
                .request_conversation_call_cancel(&project, other_task, record.consent.call_id)
                .is_err()
        );
        assert!(
            store
                .conversation_call_cancellations(&project, task)
                .unwrap()
                .is_empty()
        );
        assert!(
            store
                .conversation_call_cancellations(&project, other_task)
                .unwrap()
                .is_empty()
        );
        assert!(
            store
                .request_conversation_call_cancel(&project, task, record.consent.call_id)
                .is_ok()
        );
        assert!(
            store
                .request_conversation_call_cancel(&project, other_task, Uuid::new_v4())
                .is_ok(),
            "Unregistered call IDs still allow an owned pre-admission cancellation"
        );
    }

    #[test]
    fn concurrent_tabs_cannot_save_two_call_identities_for_one_message() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("TEST-race.sqlite");
        let store = SqliteStore::open(&path).unwrap();
        let (project, conversation, task, left) = setup(&store);
        let mut right = left.clone();
        right.consent.call_id = Uuid::new_v4();
        right.grant.id = right.consent.call_id;
        let other = SqliteStore::open(&path).unwrap();
        let start = std::sync::Barrier::new(3);
        let results = std::thread::scope(|threads| {
            let a = threads.spawn(|| {
                start.wait();
                store.authorize_conversation_feedback(&project, conversation, task, &left)
            });
            let b = threads.spawn(|| {
                start.wait();
                other.authorize_conversation_feedback(&project, conversation, task, &right)
            });
            start.wait();
            [a.join().unwrap(), b.join().unwrap()]
        });
        assert_eq!(results.iter().filter(|result| result.is_ok()).count(), 1);
        let saved = store
            .conversation_feedback_for_message(&project, task, left.consent.message_id)
            .unwrap()
            .unwrap();
        let budget = store
            .conversation_call_budget(&project, task)
            .unwrap()
            .unwrap();
        assert_eq!(budget.current_grant, saved.grant);
        assert_eq!(budget.current_grant.maximum_calls, 1);
        assert_eq!(budget.used_calls, 0);
        assert!(
            store
                .conversation_call_history(&project, task)
                .unwrap()
                .is_empty()
        );
    }
}
