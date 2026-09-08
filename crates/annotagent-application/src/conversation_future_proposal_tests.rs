//! Reuses the existing sealed sample fixture; no real workspace or external Provider.
use super::*;
use annotagent_storage::ConversationFutureSchemaProposalAuthorizationRecord;

struct PatchProvider {
    requests: Mutex<Vec<ModelRequest>>,
    arguments: serde_json::Value,
    cancel: bool,
    unknown: bool,
}
#[async_trait::async_trait]
impl VisionModelProvider for PatchProvider {
    fn name(&self) -> &str {
        "TEST future rules"
    }
    fn capabilities(&self) -> ModelCapabilities {
        provider().capabilities()
    }
    async fn complete(
        &self,
        request: ModelRequest,
        cancellation: CancellationToken,
    ) -> CoreResult<ModelResponse> {
        self.requests.lock().unwrap().push(request);
        if self.cancel {
            cancellation.cancel();
        }
        if self.unknown {
            return Err(annotagent_core::CoreError::Provider(
                "TEST unknown network outcome".into(),
            ));
        }
        Ok(ModelResponse {
            content: None,
            tool_calls: vec![ModelToolCall {
                id: "TEST future proposal".into(),
                name: "propose_future_annotation_schema".into(),
                arguments: self.arguments.clone(),
            }],
            usage: TokenUsage::known(33, 15, UsageSource::Mock),
            request_id: Some("TEST saved proposal".into()),
            provider_metadata: std::collections::BTreeMap::default(),
        })
    }
}
fn patch_provider(kind: &str) -> PatchProvider {
    PatchProvider {
        requests: Mutex::new(vec![]),
        arguments: json!({"goal":"TEST future task, retain partly visible targets","decision":"draft","kind":kind,"labels":["cup"],"multi_label":true,"attributes":{"occluded":{"type":"boolean","required":false,"values":[]}},"boundary_rules":["TEST annotate visible extent only"],"rationale":"TEST explicit future semantic changes"}),
        cancel: false,
        unknown: false,
    }
}
async fn setup() -> (Fixture, ConversationFutureSchemaProposalAuthorizationRecord) {
    let (f, _, feedback) = scope_fixture_from(fixture_with_schema(false, true)).await;
    let answer = scope_input(&feedback, &json!({"scope":"project_future_rule"}));
    f.app
        .answer_conversation_feedback_scope(
            PROJECT,
            f.conversation,
            f.task.id,
            feedback.consent.call_id,
            &answer,
        )
        .unwrap();
    let context = f
        .app
        .future_schema_proposal_context(
            PROJECT,
            f.conversation,
            f.task.id,
            feedback.consent.call_id,
        )
        .unwrap();
    let call = Uuid::new_v4();
    let scope = "e".repeat(64);
    let expires = chrono::Utc::now() + chrono::Duration::minutes(5);
    let record = ConversationFutureSchemaProposalAuthorizationRecord {
        consent: annotagent_storage::ConversationFeedbackAuthorization {
            call_id: call,
            message_id: f.message.id,
            model_id: annotagent_core::ModelProfileId::new(),
            previous_grant_id: Some(feedback.grant.id),
            scope_hash: scope.clone(),
            expires_at: expires,
            allow_unknown_cost: true,
        },
        grant: ConversationCallGrant {
            id: call,
            task_id: f.task.id,
            scope_hash: scope,
            maximum_calls: feedback.grant.maximum_calls + 1,
            expires_at: expires,
        },
        source: serde_json::from_value(context["source"].clone()).unwrap(),
        context,
        summary: json!({"remote_model":"TEST-future-model","model_name":"TEST future rules","destination":"TEST offline","maximum_output_tokens":2048}),
    };
    (f, record)
}
fn execution(
    f: &Fixture,
    r: &ConversationFutureSchemaProposalAuthorizationRecord,
) -> ConversationSchemaExecution {
    ConversationSchemaExecution {
        conversation_id: f.conversation,
        task_id: f.task.id,
        call_id: r.consent.call_id,
        remote_model: "TEST-future-model".into(),
        scope_hash: r.consent.scope_hash.clone(),
    }
}
fn save_request(
    r: &ConversationFutureSchemaProposalAuthorizationRecord,
    status: &serde_json::Value,
) -> crate::ConversationFutureSchemaRequest {
    serde_json::from_value(json!({"command_id":Uuid::new_v4(),"expected_scope_answer_command_id":r.source.scope_answer_command_id,"expected_context_digest":r.source.context_digest,"base_schema_id":r.source.base_schema_id,"base_schema_revision":r.source.base_schema_revision,"goal":status["proposal"]["Ok"]["goal"],"decision":status["proposal"]["Ok"]["decision"],"proposal_call_id":r.consent.call_id,"proposal_digest":status["proposal_digest"]})).unwrap()
}

#[tokio::test]
async fn model_proposal_stays_a_suggestion_until_human_confirmation_and_restores_after_restart() {
    for kind in ["bounding_box", "classification"] {
        let (f, r) = setup().await;
        let app = &f.app;
        let feedback = r.source.feedback_call_id;
        let provider = patch_provider(kind);
        let base = app
            .conversation_schema_draft(PROJECT, r.source.base_schema_id, None)
            .unwrap();
        let old_goal = app.project_goal(PROJECT).unwrap();
        let old_draft = app.store.get_workflow_draft(&f.sample.draft_id).unwrap();
        let budget = app
            .conversation_task_budget(PROJECT, f.conversation, f.task.id)
            .unwrap();
        app.authorize_future_schema_proposal(PROJECT, f.conversation, f.task.id, &r)
            .unwrap();
        let pending = app
            .future_schema_proposal_status(PROJECT, f.conversation, f.task.id, feedback)
            .unwrap()
            .unwrap();
        assert!(pending["receipt"].is_null());
        assert!(provider.requests.lock().unwrap().is_empty());
        assert_eq!(
            app.conversation_task_budget(PROJECT, f.conversation, f.task.id)
                .unwrap()
                .total_reserved_calls,
            budget.total_reserved_calls
        );
        let status = app
            .execute_future_schema_proposal(
                PROJECT,
                &execution(&f, &r),
                feedback,
                &provider,
                CancellationToken::new(),
            )
            .await
            .unwrap();
        assert!(status["proposal"]["Ok"].is_object(), "{status}");
        assert!(
            app.conversation_future_schema(PROJECT, f.conversation, f.task.id, feedback)
                .unwrap()
                .record
                .is_none()
        );
        assert_eq!(
            app.conversation_schema_draft(PROJECT, base.id, None)
                .unwrap(),
            base
        );
        assert!(
            app.save_conversation_schema_draft(
                PROJECT,
                f.conversation,
                f.task.id,
                r.consent.call_id
            )
            .is_err(),
            "a future proposal cannot pass through initial Schema materialization"
        );
        {
            let requests = provider.requests.lock().unwrap();
            assert_eq!(requests.len(), 1);
            let sent = &requests[0];
            assert!(sent.images.is_empty());
            assert_eq!(sent.tools.len(), 1);
            assert_eq!(sent.tools[0].name, "propose_future_annotation_schema");
            assert_eq!(
                serde_json::from_str::<serde_json::Value>(&sent.messages[1].content).unwrap(),
                r.context
            );
            assert_eq!(sent.max_output_tokens, 2048);
        }
        let mut input = save_request(&r, &status);
        input.goal = "TEST human reviewed future goal".into();
        if let crate::ConversationSchemaDecision::Draft { boundary_rules, .. } = &mut input.decision
        {
            boundary_rules.push("TEST human edited rule".into());
        }
        let saved = app
            .save_conversation_future_schema(PROJECT, f.conversation, f.task.id, feedback, &input)
            .unwrap();
        let child = saved.schema.unwrap();
        assert_ne!(child.id, base.id);
        assert!(child.definition.task.multi_label);
        assert!(child.definition.task.attributes.contains_key("occluded"));
        assert_eq!(child.definition.goal, input.goal);
        assert_eq!(
            saved.record.unwrap().input.proposal_call_id,
            Some(r.consent.call_id)
        );
        assert_eq!(app.project_goal(PROJECT).unwrap(), old_goal);
        assert_eq!(
            app.store.get_workflow_draft(&f.sample.draft_id).unwrap(),
            old_draft
        );
        assert_eq!(
            app.store
                .get_workflow_sample_test_by_id(&f.sample.id)
                .unwrap(),
            Some(f.sample.clone())
        );
        let used = app
            .conversation_task_budget(PROJECT, f.conversation, f.task.id)
            .unwrap();
        assert_eq!(used.total_reserved_calls, budget.total_reserved_calls + 1);
        assert_eq!(
            app.execute_future_schema_proposal(
                PROJECT,
                &execution(&f, &r),
                feedback,
                &provider,
                CancellationToken::new()
            )
            .await
            .unwrap(),
            status
        );
        let reopened = LocalApplication::new(f.temporary.path()).unwrap();
        assert_eq!(
            reopened
                .future_schema_proposal_status(PROJECT, f.conversation, f.task.id, feedback)
                .unwrap(),
            Some(status)
        );
        assert_eq!(
            reopened
                .save_conversation_future_schema(
                    PROJECT,
                    f.conversation,
                    f.task.id,
                    feedback,
                    &input
                )
                .unwrap()
                .schema
                .unwrap(),
            child
        );
        assert_eq!(
            reopened
                .conversation_task_budget(PROJECT, f.conversation, f.task.id)
                .unwrap(),
            used
        );
        assert_eq!(provider.requests.lock().unwrap().len(), 1);
    }
}

#[tokio::test]
async fn invalid_unknown_cancelled_or_stale_future_proposals_do_not_create_schema_or_retry() {
    for mode in ["invalid", "unknown", "cancelled", "stale", "pending-cancel"] {
        let (f, r) = setup().await;
        let app = &f.app;
        let feedback = r.source.feedback_call_id;
        let mut provider = patch_provider("bounding_box");
        if mode == "invalid" {
            provider.arguments["publish"] = json!(true);
        }
        provider.cancel = mode == "cancelled";
        provider.unknown = mode == "unknown";
        app.authorize_future_schema_proposal(PROJECT, f.conversation, f.task.id, &r)
            .unwrap();
        let budget = app
            .conversation_task_budget(PROJECT, f.conversation, f.task.id)
            .unwrap();
        if mode == "stale" {
            app.revise_conversation_schema_draft(PROJECT,r.source.base_schema_id,Uuid::new_v4(),r.source.base_schema_revision,&serde_json::from_value(json!({"decision":"draft","kind":"bounding_box","labels":["plate"],"multi_label":false,"attributes":{},"boundary_rules":[],"rationale":"TEST newer base"})).unwrap()).unwrap();
        }
        if mode == "pending-cancel" {
            app.cancel_conversation_schema(PROJECT, f.conversation, f.task.id, r.consent.call_id)
                .unwrap();
        }
        let result = app
            .execute_future_schema_proposal(
                PROJECT,
                &execution(&f, &r),
                feedback,
                &provider,
                CancellationToken::new(),
            )
            .await;
        if mode == "stale" {
            assert!(result.is_err());
        } else {
            let status = result.unwrap();
            assert!(status["proposal"]["Ok"].is_null(), "{mode}: {status}");
            let read = app
                .future_schema_proposal_status(PROJECT, f.conversation, f.task.id, feedback)
                .unwrap()
                .unwrap();
            assert_eq!(status, read);
            assert_eq!(
                app.execute_future_schema_proposal(
                    PROJECT,
                    &execution(&f, &r),
                    feedback,
                    &provider,
                    CancellationToken::new()
                )
                .await
                .unwrap(),
                read
            );
        }
        let calls = usize::from(!matches!(mode, "stale" | "pending-cancel"));
        assert_eq!(provider.requests.lock().unwrap().len(), calls);
        assert_eq!(
            app.conversation_task_budget(PROJECT, f.conversation, f.task.id)
                .unwrap()
                .total_reserved_calls,
            budget.total_reserved_calls + calls as u64
        );
        assert!(
            app.store
                .future_schema_draft(
                    &app.conversation_project_identity(PROJECT).unwrap(),
                    f.task.id,
                    feedback
                )
                .unwrap()
                .is_none()
        );
    }
}
