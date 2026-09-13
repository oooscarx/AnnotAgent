//! Atomic message-to-task admission. No model, budget grant or execution is created.
use crate::{
    ConversationMessage, ConversationMessageInput, ConversationSelectionRef, SqliteStore,
    StorageError,
};
use rusqlite::{OptionalExtension, params};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Requested behavior for this message, not an execution grant. Actual model and
/// mutation operations still require their separately scoped authorizations.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConversationSendMode {
    Plan,
    Execute,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ConversationSendInput {
    pub message: ConversationMessageInput,
    /// Exact images attached to a newly created Task. These are task data only;
    /// inference still requires a separate Journey consent.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub task_images: Vec<crate::ConversationImageRef>,
    pub task_id: Option<Uuid>,
    pub schema_revision: String,
    /// Optional observed preference for CAS admission; absent for older clients.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub agent_model: Option<crate::ConversationAgentModel>,
    /// Missing means a legacy command, not implicit execution permission.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mode: Option<ConversationSendMode>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConversationSendDisposition {
    NewTask,
    TaskMessage,
    CandidateFeedback,
    FormalFeedback,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConversationSendReceipt {
    pub message: ConversationMessage,
    pub task_id: Uuid,
    pub disposition: ConversationSendDisposition,
    /// None means a historical receipt predating model snapshots, not default.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub agent_model: Option<crate::ConversationAgentModel>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mode: Option<ConversationSendMode>,
    /// Resolved at Send when setup exists. Historical/unconfigured messages have None.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resolved_agent_model_id: Option<annotagent_core::ModelProfileId>,
}

fn invalid(message: &str) -> StorageError {
    StorageError::InvalidConversation(message.into())
}

impl SqliteStore {
    pub fn conversation_send_receipt(
        &self,
        project: &str,
        conversation: Uuid,
        message: Uuid,
    ) -> Result<Option<(ConversationSendInput, ConversationSendReceipt)>, StorageError> {
        self.with_connection(|db| {
            crate::conversations::require_owner(db, project, conversation)?;
            let row: Option<(String, String)> = db.query_row("SELECT input_json,receipt_json FROM conversation_send_receipts WHERE conversation_id=?1 AND message_id=?2", params![conversation.to_string(),message.to_string()], |row| Ok((row.get(0)?,row.get(1)?))).optional()?;
            row.map(|(input, receipt)| Ok((serde_json::from_str(&input)?,serde_json::from_str(&receipt)?))).transpose()
        })
    }

    /// Application validates current Project schema and terminal artifact provenance before
    /// first admission, under its schema lock. Transaction resolves identity, never UI state.
    pub fn send_conversation_message(
        &self,
        project: &str,
        conversation: Uuid,
        input: &ConversationSendInput,
    ) -> Result<ConversationSendReceipt, StorageError> {
        self.send_conversation_message_with_model(project, conversation, input, None, None)
    }

    /// Application supplies the passive resolved default and observed preference.
    /// CAS is in the same transaction as the message, receipt and queue insertion.
    pub fn send_conversation_message_with_model(
        &self,
        project: &str,
        conversation: Uuid,
        input: &ConversationSendInput,
        resolved_agent_model_id: Option<annotagent_core::ModelProfileId>,
        observed: Option<&crate::ConversationAgentModel>,
    ) -> Result<ConversationSendReceipt, StorageError> {
        self.with_connection(|db| {
            let tx = db.unchecked_transaction()?;
            crate::conversations::require_owner(&tx, project, conversation)?;
            let saved: Option<(String,String)> = tx.query_row("SELECT input_json,receipt_json FROM conversation_send_receipts WHERE conversation_id=?1 AND message_id=?2",params![conversation.to_string(),input.message.id.to_string()],|r|Ok((r.get(0)?,r.get(1)?))).optional()?;
            if let Some((original,receipt))=saved {
                if serde_json::from_str::<ConversationSendInput>(&original)? != *input { return Err(invalid("Send ID conflicts with its frozen task, message or schema")); }
                return Ok(serde_json::from_str(&receipt)?);
            }
            if input.schema_revision.len()!=64 || !input.schema_revision.bytes().all(|b|b.is_ascii_hexdigit()) { return Err(invalid("Send requires a schema revision digest")); }
            let agent_model=crate::conversation_agent_model::read(&tx,conversation)?;
            if observed.is_some_and(|value| *value != agent_model) || input.agent_model.as_ref().is_some_and(|observed| *observed!=agent_model) {
                return Err(StorageError::StaleConversationAgentModel);
            }
            // A legacy journal ID cannot silently acquire a new dispatch meaning.
            let existing:bool=tx.query_row("SELECT EXISTS(SELECT 1 FROM conversation_messages WHERE conversation_id=?1 AND message_id=?2)",params![conversation.to_string(),input.message.id.to_string()],|r|r.get(0))?;
            if existing { return Err(invalid("Message already belongs to the legacy journal; create a new send command")); }
            let (task_id,disposition)=match &input.message.reference {
                Some(ConversationSelectionRef::StopRequest{..})=>return Err(invalid("Use the explicit stop endpoint")),
                Some(ConversationSelectionRef::SampleCandidate{task_id,project_schema_revision,..})=>{
                    if input.task_id!=Some(*task_id) || input.schema_revision!=*project_schema_revision { return Err(invalid("Candidate reference conflicts with send task or schema")); }
                    (*task_id,ConversationSendDisposition::CandidateFeedback)
                },
                Some(ConversationSelectionRef::FormalAnnotation{task_id,project_schema_revision,..})=>{
                    if input.task_id!=Some(*task_id) || input.schema_revision!=*project_schema_revision { return Err(invalid("Formal annotation reference conflicts with send task or schema")); }
                    (*task_id,ConversationSendDisposition::FormalFeedback)
                },
                None=>match input.task_id {
                    Some(task)=>(task,ConversationSendDisposition::TaskMessage),
                    None=>(Uuid::new_v4(),ConversationSendDisposition::NewTask),
                },
            };
            if disposition!=ConversationSendDisposition::NewTask {
                let owned:bool=tx.query_row("SELECT EXISTS(SELECT 1 FROM conversation_tasks WHERE id=?1 AND conversation_id=?2 AND schema_revision=?3)",params![task_id.to_string(),conversation.to_string(),input.schema_revision],|r|r.get(0))?;
                if !owned { return Err(invalid("Send task is foreign, missing or has a different frozen schema")); }
                crate::conversation_task_lifecycle::require_active_in(&tx, task_id)?;
            }
            let message=crate::conversations::append_message_in_transaction(&tx,project,conversation,&input.message)?;
            if disposition==ConversationSendDisposition::NewTask {
                tx.execute("INSERT INTO conversation_tasks(id,conversation_id,source_message_id,schema_revision,created_at) VALUES(?1,?2,?3,?4,?5)",params![task_id.to_string(),conversation.to_string(),message.input.id.to_string(),input.schema_revision,chrono::Utc::now().to_rfc3339()])?;
            }
            let receipt=ConversationSendReceipt{message,task_id,disposition,agent_model:Some(agent_model),mode:input.mode,resolved_agent_model_id};
            tx.execute("INSERT INTO conversation_send_receipts(conversation_id,message_id,input_json,receipt_json) VALUES(?1,?2,?3,?4)",params![conversation.to_string(),input.message.id.to_string(),serde_json::to_string(input)?,serde_json::to_string(&receipt)?])?;
            // Only modern, ordinary follow-ups enter this coordinator inbox.
            // Legacy history is not backfilled; candidate-scoped feedback keeps
            // its existing separately authorized interpretation path for now.
            if receipt.disposition==ConversationSendDisposition::TaskMessage && input.mode.is_some() {
                tx.execute("INSERT INTO conversation_message_queue(conversation_id,message_id,task_id,sequence) VALUES(?1,?2,?3,?4)",params![conversation.to_string(),input.message.id.to_string(),task_id.to_string(),receipt.message.sequence])?;
            }
            tx.commit()?;
            Ok(receipt)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn followup_queue_is_atomic_ordered_owned_and_cancellation_is_terminal() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("TEST-queue.sqlite");
        let store = SqliteStore::open(&path).unwrap();
        let project = Uuid::new_v4().to_string();
        let conversation = store.create_conversation(&project).unwrap();
        let first = store
            .send_conversation_message(&project, conversation, &input())
            .unwrap();
        let mut followup = input();
        followup.task_id = Some(first.task_id);
        followup.mode = Some(ConversationSendMode::Plan);
        let sent = store
            .send_conversation_message(&project, conversation, &followup)
            .unwrap();
        let mut second = followup.clone();
        second.message.id = Uuid::new_v4();
        second.message.text = "TEST later instruction".into();
        second.mode = Some(ConversationSendMode::Execute);
        store
            .send_conversation_message(&project, conversation, &second)
            .unwrap();
        let queue = store
            .conversation_message_queue(&project, conversation, first.task_id, 0)
            .unwrap();
        assert_eq!(queue.len(), 2);
        assert_eq!(queue[0].receipt, sent);
        assert_eq!(queue[0].input, followup);
        assert_eq!(queue[1].input, second);
        assert!(
            store
                .conversation_message_queue("foreign", conversation, first.task_id, 0)
                .is_err()
        );
        assert!(
            store
                .conversation_message_queue(&project, conversation, Uuid::new_v4(), 0)
                .is_err()
        );
        assert!(
            store
                .conversation_message_queue(&project, conversation, first.task_id, -1)
                .is_err()
        );
        assert!(
            store
                .cancel_queued_conversation_message(
                    &project,
                    conversation,
                    Uuid::new_v4(),
                    followup.message.id
                )
                .is_err()
        );
        let cancelled = store
            .cancel_queued_conversation_message(
                &project,
                conversation,
                first.task_id,
                followup.message.id,
            )
            .unwrap();
        assert_eq!(
            cancelled.status,
            crate::ConversationQueuedMessageStatus::Cancelled
        );
        assert!(cancelled.cancelled_at.is_some());
        drop(store);
        let store = SqliteStore::open(&path).unwrap();
        assert_eq!(
            store
                .cancel_queued_conversation_message(
                    &project,
                    conversation,
                    first.task_id,
                    followup.message.id
                )
                .unwrap(),
            cancelled
        );
        assert_eq!(
            store
                .send_conversation_message(&project, conversation, &followup)
                .unwrap(),
            sent
        );
        let queue = store
            .conversation_message_queue(&project, conversation, first.task_id, 0)
            .unwrap();
        assert_eq!(queue[0], cancelled);
        assert_eq!(
            queue[1].status,
            crate::ConversationQueuedMessageStatus::WaitingForDispatch
        );
        assert_eq!(
            store
                .conversation_message_queue(
                    &project,
                    conversation,
                    first.task_id,
                    sent.message.sequence
                )
                .unwrap()
                .len(),
            1
        );
        assert_eq!(
            store
                .conversation_messages(&project, conversation, 0, 100)
                .unwrap()
                .len(),
            3
        );
        // Queue failure must not leave behind a logged-but-unqueued modern Send.
        store.with_connection(|db| {db.execute_batch("CREATE TRIGGER TEST_queue_failure BEFORE INSERT ON conversation_message_queue BEGIN SELECT RAISE(ABORT,'TEST queue failure'); END;")?;Ok(())}).unwrap();
        let mut failed = followup.clone();
        failed.message.id = Uuid::new_v4();
        assert!(
            store
                .send_conversation_message(&project, conversation, &failed)
                .is_err()
        );
        assert!(
            store
                .conversation_send_receipt(&project, conversation, failed.message.id)
                .unwrap()
                .is_none()
        );
        assert!(
            store
                .conversation_message(&project, conversation, failed.message.id)
                .unwrap()
                .is_none()
        );
    }

    #[test]
    fn send_mode_is_immutable_per_message_and_survives_restart() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("TEST-send-mode.db");
        let store = SqliteStore::open(&path).unwrap();
        let owner = Uuid::new_v4().to_string();
        let conversation = store.create_conversation(&owner).unwrap();
        let mut command = input();
        command.mode = Some(ConversationSendMode::Plan);
        let receipt = store
            .send_conversation_message(&owner, conversation, &command)
            .unwrap();
        assert_eq!(receipt.mode, Some(ConversationSendMode::Plan));
        let mut changed = command.clone();
        changed.mode = Some(ConversationSendMode::Execute);
        assert!(
            store
                .send_conversation_message(&owner, conversation, &changed)
                .is_err()
        );
        let mut next = changed.clone();
        next.message.id = Uuid::new_v4();
        next.task_id = Some(receipt.task_id);
        assert_eq!(
            store
                .send_conversation_message(&owner, conversation, &next)
                .unwrap()
                .mode,
            Some(ConversationSendMode::Execute)
        );
        drop(store);
        let store = SqliteStore::open(&path).unwrap();
        assert_eq!(
            store
                .send_conversation_message(&owner, conversation, &command)
                .unwrap(),
            receipt
        );
        assert_eq!(
            store
                .conversation_messages(&owner, conversation, 0, 100)
                .unwrap()
                .len(),
            2
        );
        let mut legacy = serde_json::to_value(&command).unwrap();
        legacy.as_object_mut().unwrap().remove("mode");
        assert_eq!(
            serde_json::from_value::<ConversationSendInput>(legacy.clone())
                .unwrap()
                .mode,
            None
        );
        legacy["mode"] = serde_json::json!("unrestricted");
        assert!(serde_json::from_value::<ConversationSendInput>(legacy).is_err());
        let mut legacy_receipt = serde_json::to_value(&receipt).unwrap();
        legacy_receipt.as_object_mut().unwrap().remove("mode");
        assert_eq!(
            serde_json::from_value::<ConversationSendReceipt>(legacy_receipt)
                .unwrap()
                .mode,
            None
        );
    }

    #[test]
    fn send_freezes_model_and_stale_choice_rolls_back_before_message_admission() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("TEST-send-model.db");
        let store = SqliteStore::open(&path).unwrap();
        let owner = Uuid::new_v4().to_string();
        let conversation = store.create_conversation(&owner).unwrap();
        let selection = crate::SelectConversationAgentModel {
            request_id: Uuid::new_v4(),
            expected_revision: 0,
            model_profile_id: Some(annotagent_core::ModelProfileId::new()),
        };
        let first = store
            .select_conversation_agent_model(&owner, conversation, &selection)
            .unwrap();
        let mut command = input();
        command.agent_model = Some(first.clone());
        let admitted = store
            .send_conversation_message(&owner, conversation, &command)
            .unwrap();
        assert_eq!(admitted.agent_model, Some(first));
        store
            .select_conversation_agent_model(
                &owner,
                conversation,
                &crate::SelectConversationAgentModel {
                    request_id: Uuid::new_v4(),
                    expected_revision: 1,
                    model_profile_id: Some(annotagent_core::ModelProfileId::new()),
                },
            )
            .unwrap();
        let mut stale = command.clone();
        stale.message.id = Uuid::new_v4();
        assert!(
            store
                .send_conversation_message(&owner, conversation, &stale)
                .is_err()
        );
        assert!(
            store
                .conversation_message(&owner, conversation, stale.message.id)
                .unwrap()
                .is_none()
        );
        assert!(
            store
                .conversation_send_receipt(&owner, conversation, stale.message.id)
                .unwrap()
                .is_none()
        );
        drop(store);
        let store = SqliteStore::open(&path).unwrap();
        assert_eq!(
            store
                .send_conversation_message(&owner, conversation, &command)
                .unwrap(),
            admitted
        );
        let mut legacy = serde_json::to_value(&admitted).unwrap();
        legacy.as_object_mut().unwrap().remove("agent_model");
        assert!(
            serde_json::from_value::<ConversationSendReceipt>(legacy)
                .unwrap()
                .agent_model
                .is_none()
        );
    }

    #[test]
    fn queued_planning_preserves_budget_and_requires_exact_fifo_call() {
        use crate::{
            ConversationCallAdmission, ConversationCallGrant, ConversationCallStatus,
            QueuedPlanningAuthorization,
        };
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("TEST-queued-planning.sqlite");
        let store = SqliteStore::open(&path).unwrap();
        let owner = Uuid::new_v4().to_string();
        let conversation = store.create_conversation(&owner).unwrap();
        let root = store
            .send_conversation_message(&owner, conversation, &input())
            .unwrap();
        let task = root.task_id;
        let mut first = input();
        first.task_id = Some(task);
        first.mode = Some(ConversationSendMode::Plan);
        store
            .send_conversation_message(&owner, conversation, &first)
            .unwrap();
        let mut second = first.clone();
        second.message.id = Uuid::new_v4();
        store
            .send_conversation_message(&owner, conversation, &second)
            .unwrap();
        let initial = ConversationCallGrant {
            id: Uuid::new_v4(),
            task_id: task,
            scope_hash: "b".repeat(64),
            maximum_calls: 1,
            expires_at: chrono::Utc::now() + chrono::Duration::minutes(10),
        };
        store
            .authorize_conversation_calls(&owner, &initial)
            .unwrap();
        let initial_call = Uuid::new_v4();
        store
            .reserve_conversation_call(
                &owner,
                task,
                initial_call,
                &initial.scope_hash,
                &"c".repeat(64),
            )
            .unwrap();
        let mut approval = QueuedPlanningAuthorization {
            conversation_id: conversation,
            message_id: first.message.id,
            previous_grant_id: Some(initial.id),
            grant: ConversationCallGrant {
                id: Uuid::new_v4(),
                maximum_calls: 3,
                ..initial.clone()
            },
            model_id: annotagent_core::ModelProfileId::new(),
            request_hash: "d".repeat(64),
        };
        assert!(
            store.authorize_queued_planning(&owner, &approval).is_err(),
            "cannot override active request"
        );
        store
            .finish_conversation_call(
                &owner,
                task,
                initial_call,
                ConversationCallStatus::Completed,
                serde_json::json!({"TEST":true}),
            )
            .unwrap();
        approval.message_id = second.message.id;
        assert!(
            store.authorize_queued_planning(&owner, &approval).is_err(),
            "FIFO"
        );
        assert_eq!(
            store
                .conversation_call_budget(&owner, task)
                .unwrap()
                .unwrap()
                .current_grant,
            initial,
            "failed binding rolls back grant"
        );
        approval.message_id = first.message.id;
        store.authorize_queued_planning(&owner, &approval).unwrap();
        let queue = store
            .conversation_message_queue(&owner, conversation, task, 0)
            .unwrap();
        assert_eq!(
            queue[0].status,
            crate::ConversationQueuedMessageStatus::Authorized
        );
        assert_eq!(queue[0].planning_call_id, Some(approval.grant.id));
        assert!(
            store
                .authorize_queued_planning("foreign", &approval)
                .is_err()
        );
        assert_eq!(
            store
                .conversation_call_budget(&owner, task)
                .unwrap()
                .unwrap()
                .used_calls,
            1
        );
        assert!(
            store
                .reserve_conversation_call(
                    &owner,
                    task,
                    Uuid::new_v4(),
                    &approval.grant.scope_hash,
                    &approval.request_hash
                )
                .is_err()
        );
        assert!(
            store
                .reserve_conversation_call(
                    &owner,
                    task,
                    approval.grant.id,
                    &approval.grant.scope_hash,
                    &"e".repeat(64)
                )
                .is_err()
        );
        let admissions = std::thread::scope(|scope| {
            let reserve = || {
                store
                    .reserve_conversation_call(
                        &owner,
                        task,
                        approval.grant.id,
                        &approval.grant.scope_hash,
                        &approval.request_hash,
                    )
                    .unwrap()
            };
            let first = scope.spawn(reserve);
            let second = scope.spawn(reserve);
            [first.join().unwrap(), second.join().unwrap()]
        });
        assert_eq!(
            admissions
                .iter()
                .filter(|result| matches!(result, ConversationCallAdmission::Admitted))
                .count(),
            1
        );
        assert_eq!(
            admissions
                .iter()
                .filter(|result| matches!(result, ConversationCallAdmission::Existing(_)))
                .count(),
            1
        );
        assert!(
            store
                .cancel_queued_conversation_message(&owner, conversation, task, first.message.id)
                .is_err(),
            "running requires task stop"
        );
        assert_eq!(
            store
                .conversation_message_queue(&owner, conversation, task, 0)
                .unwrap()[0]
                .status,
            crate::ConversationQueuedMessageStatus::Running
        );
        store
            .finish_conversation_call(
                &owner,
                task,
                approval.grant.id,
                ConversationCallStatus::Completed,
                serde_json::json!({"TEST":true}),
            )
            .unwrap();
        let next = QueuedPlanningAuthorization {
            message_id: second.message.id,
            previous_grant_id: Some(approval.grant.id),
            grant: ConversationCallGrant {
                id: Uuid::new_v4(),
                ..approval.grant.clone()
            },
            ..approval.clone()
        };
        store.authorize_queued_planning(&owner, &next).unwrap();
        assert_eq!(
            store
                .conversation_message_queue(&owner, conversation, task, 0)
                .unwrap()[0]
                .status,
            crate::ConversationQueuedMessageStatus::Completed
        );
        store
            .cancel_queued_conversation_message(&owner, conversation, task, second.message.id)
            .unwrap();
        drop(store);
        let store = SqliteStore::open(&path).unwrap();
        store.authorize_queued_planning(&owner, &next).unwrap();
        assert!(
            store
                .reserve_conversation_call(
                    &owner,
                    task,
                    next.grant.id,
                    &next.grant.scope_hash,
                    &next.request_hash
                )
                .is_err(),
            "restart cannot resurrect cancelled instruction"
        );
        assert_eq!(
            store
                .conversation_call_budget(&owner, task)
                .unwrap()
                .unwrap()
                .used_calls,
            2
        );
        assert!(matches!(
            store
                .reserve_conversation_call(
                    &owner,
                    task,
                    approval.grant.id,
                    &approval.grant.scope_hash,
                    &approval.request_hash
                )
                .unwrap(),
            ConversationCallAdmission::Existing(_)
        ));
        println!(
            "AGENT_UI_TRACE {}",
            serde_json::json!({"fixture":true,"test":"queued_planning_preserves_budget_and_requires_exact_fifo_call","dispatch_admitted_count":1,"duplicate_dispatch_existing_count":1,"after_restart":store.conversation_message_queue(&owner,conversation,task,0).unwrap(),"budget":store.conversation_call_budget(&owner,task).unwrap()})
        );
    }

    fn input() -> ConversationSendInput {
        ConversationSendInput {
            message: ConversationMessageInput {
                id: Uuid::new_v4(),
                text: "TEST find cups".into(),
                image: None,
                reference: None,
            },
            task_images: vec![],
            task_id: None,
            schema_revision: "a".repeat(64),
            agent_model: None,
            mode: None,
        }
    }

    #[test]
    fn send_restores_exact_task_and_rejects_retargeting_without_a_second_message() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("TEST-send.sqlite");
        let store = SqliteStore::open(&path).unwrap();
        let owner = Uuid::new_v4().to_string();
        let conversation = store.create_conversation(&owner).unwrap();
        let mut command = input();
        command.task_images = vec![crate::ConversationImageRef {
            image_id: Uuid::new_v4().to_string(),
            sha256: "b".repeat(64),
        }];
        let first = store
            .send_conversation_message(&owner, conversation, &command)
            .unwrap();
        assert_eq!(first.disposition, ConversationSendDisposition::NewTask);
        drop(store);
        let store = SqliteStore::open(&path).unwrap();
        assert_eq!(
            store
                .send_conversation_message(&owner, conversation, &command)
                .unwrap(),
            first
        );
        let mut conflict = command.clone();
        conflict.task_id = Some(first.task_id);
        assert!(
            store
                .send_conversation_message(&owner, conversation, &conflict)
                .is_err()
        );
        let mut changed_scope = command.clone();
        changed_scope.task_images[0].sha256 = "c".repeat(64);
        assert!(
            store
                .send_conversation_message(&owner, conversation, &changed_scope)
                .is_err()
        );
        let mut followup = input();
        followup.task_id = Some(first.task_id);
        let reply = store
            .send_conversation_message(&owner, conversation, &followup)
            .unwrap();
        assert_eq!(reply.disposition, ConversationSendDisposition::TaskMessage);
        assert_eq!(reply.task_id, first.task_id);
        assert_eq!(
            store
                .conversation_tasks(&owner, conversation)
                .unwrap()
                .len(),
            1
        );
        assert_eq!(
            store
                .conversation_messages(&owner, conversation, 0, 100)
                .unwrap()
                .len(),
            2
        );
        assert!(
            store
                .conversation_send_receipt("foreign", conversation, command.message.id)
                .is_err()
        );
        let mut foreign = input();
        foreign.task_id = Some(Uuid::new_v4());
        assert!(
            store
                .send_conversation_message(&owner, conversation, &foreign)
                .is_err()
        );
        assert_eq!(
            store
                .conversation_messages(&owner, conversation, 0, 100)
                .unwrap()
                .len(),
            2
        );
    }

    #[test]
    fn failed_receipt_insert_rolls_back_message_and_task_together() {
        let temp = tempfile::tempdir().unwrap();
        let store = SqliteStore::open(temp.path().join("TEST-rollback.sqlite")).unwrap();
        let owner = Uuid::new_v4().to_string();
        let conversation = store.create_conversation(&owner).unwrap();
        store.with_connection(|db| { db.execute_batch("CREATE TRIGGER fail_send BEFORE INSERT ON conversation_send_receipts BEGIN SELECT RAISE(ABORT,'TEST receipt failure'); END;")?; Ok(()) }).unwrap();
        let command = input();
        assert!(
            store
                .send_conversation_message(&owner, conversation, &command)
                .is_err()
        );
        assert!(
            store
                .conversation_messages(&owner, conversation, 0, 100)
                .unwrap()
                .is_empty()
        );
        assert!(
            store
                .conversation_tasks(&owner, conversation)
                .unwrap()
                .is_empty()
        );
        store
            .with_connection(|db| {
                db.execute_batch("DROP TRIGGER fail_send")?;
                Ok(())
            })
            .unwrap();
        let receipt = store
            .send_conversation_message(&owner, conversation, &command)
            .unwrap();
        assert_eq!(receipt.message.sequence, 1);
    }
}
