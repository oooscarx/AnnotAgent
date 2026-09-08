//! Scope validation for human corrections. No model invocation or independent executor.
use crate::LocalApplication;
use annotagent_storage::{
    ConversationHumanRequest, ConversationHumanRequestInput, SampleFeedbackRevision,
    WorkflowSampleTest,
};
use anyhow::{Context, Result, bail};
use uuid::Uuid;

fn first_review_subject(
    result: &annotagent_core::WorkflowDryRunSampleResult,
) -> Option<&annotagent_core::FinalCandidateProjection> {
    result
        .projection
        .review_candidates
        .iter()
        .map(|item| &item.candidate)
        .chain(result.projection.final_candidates.iter())
        .find(|item| {
            item.outcome.status == annotagent_core::SampleTestOutcomeStatus::NeedsReview
                && matches!(
                    item.outcome.value,
                    Some(
                        annotagent_core::VisionArtifactValue::BoundingBox { .. }
                            | annotagent_core::VisionArtifactValue::Classification { .. }
                    )
                )
        })
}

fn needs_reference_target(result: &annotagent_core::WorkflowDryRunSampleResult) -> bool {
    use annotagent_core::AnnotationFailureClass as Failure;
    result.projection.final_candidates.is_empty()
        && result.projection.review_candidates.is_empty()
        && result
            .failure_classes
            .iter()
            .any(|class| matches!(class, Failure::SemanticError | Failure::GeometryError))
        && !result.failure_classes.iter().any(|class| {
            matches!(
                class,
                Failure::InfrastructureFailure
                    | Failure::ProviderFailure
                    | Failure::BudgetLimit
                    | Failure::InvalidArtifact
            )
        })
}

fn validate_subject(
    project: &str,
    test: &WorkflowSampleTest,
    input: &ConversationHumanRequestInput,
    current_hash: &str,
) -> Result<()> {
    if test.project_id != project || test.id != input.sample_test_id || !test.report.sandbox {
        bail!("Human correction requires this Project's saved Sandbox sample");
    }
    let index = test
        .inputs
        .iter()
        .position(|image| {
            image.image_id == input.image_id && image.content_hash == input.content_hash
        })
        .context("Requested image does not match the saved sample")?;
    if current_hash != input.content_hash {
        bail!(
            "Image changed since this sample; the saved result cannot be applied to different pixels"
        );
    }
    let projection = &test
        .report
        .samples
        .get(index)
        .context(
            "Sample has no result; intermediate detections cannot become human correction targets",
        )?
        .projection;
    if let (None, Some(id)) = (&input.outcome_id, &input.addition_id) {
        if Uuid::parse_str(id).is_ok() && input.reason_code == "identify_target" {
            return Ok(());
        }
        bail!("Reference target requires its own UUID and an identification request");
    }
    if input.addition_id.is_some() {
        bail!("A human request cannot target both a model outcome and a new reference");
    }
    let terminal = projection
        .final_candidates
        .iter()
        .chain(
            projection
                .review_candidates
                .iter()
                .map(|review| &review.candidate),
        )
        .any(|candidate| Some(&candidate.outcome.id) == input.outcome_id.as_ref());
    if !terminal {
        bail!("Requested outcome is not a terminal candidate on this sample image");
    }
    Ok(())
}

impl LocalApplication {
    /// Deliver only persisted, opted-in completion work. No inference or historical backfill.
    pub fn recover_conversation_sample_assistance(&self) -> Result<()> {
        for operation in self.store.pending_sample_assistance()? {
            let result = (|| {
                let conversation = serde_json::from_value(
                    operation.request["conversation"]["conversation_id"].clone(),
                )?;
                let task =
                    serde_json::from_value(operation.request["conversation"]["task_id"].clone())?;
                self.prepare_conversation_sample_requests(
                    &operation.project_id,
                    conversation,
                    task,
                    &operation.id,
                )
            })();
            self.store.settle_sample_assistance(
                &operation.id,
                result
                    .as_ref()
                    .err()
                    .map(std::string::ToString::to_string)
                    .as_deref(),
            )?;
        }
        Ok(())
    }

    /// Prepare bounded human work from saved terminal evidence. This command is local only;
    /// it neither predicts new objects nor treats a review flag as proof of inaccuracy.
    pub fn prepare_conversation_sample_requests(
        &self,
        project: &str,
        conversation: Uuid,
        task: Uuid,
        sample_id: &str,
    ) -> Result<Vec<ConversationHumanRequest>> {
        let existing = self.conversation_human_requests(project, conversation, task)?;
        let operation = self
            .store
            .sample_operation(sample_id)?
            .context("Sample operation not found")?;
        if operation.project_id != project
            || operation.request["conversation"]["conversation_id"] != conversation.to_string()
            || operation.request["conversation"]["task_id"] != task.to_string()
        {
            bail!("Sample operation belongs to another conversation task");
        }
        let sample = self
            .store
            .get_workflow_sample_test_by_id(sample_id)?
            .context("Sample report is not saved yet")?;
        if sample.project_id != project
            || !sample.report.sandbox
            || sample.draft_id != operation.draft_id
            || sample.inputs.len() != sample.report.samples.len()
        {
            bail!("Human assistance requires consistent owned Sandbox evidence");
        }
        let reference_supported = self
            .store
            .sample_scope_seal(sample_id)?
            .is_some_and(|seal| {
                matches!(
                    seal["annotation_schema"]["task"]["kind"].as_str(),
                    Some("bounding_box" | "classification")
                )
            });
        let mut requests = Vec::new();
        // One request per image avoids conflicting optimistic feedback sequences on that image.
        // Further questions require a separately evaluated continuation, not an unbounded queue.
        for (image, result) in sample.inputs.iter().zip(&sample.report.samples).take(10) {
            if let Some(saved) = existing.iter().find(|request| {
                request.input.sample_test_id == sample_id
                    && request.input.image_id == image.image_id
            }) {
                requests.push(saved.clone());
                continue;
            }
            let candidate = first_review_subject(result);
            let reference =
                candidate.is_none() && reference_supported && needs_reference_target(result);
            if candidate.is_none() && !reference {
                continue;
            }
            let key = format!(
                "conversation-sample-human-v1:{conversation}:{task}:{sample_id}:{}",
                image.image_id
            );
            let id = Uuid::new_v5(&Uuid::NAMESPACE_URL, key.as_bytes());
            let prior = self.store.sample_feedback(sample_id, &image.image_id)?;
            let input = ConversationHumanRequestInput {
                id, task_id:task, conversation_id:conversation, sample_test_id:sample_id.to_owned(),
                image_id:image.image_id.clone(), content_hash:image.content_hash.clone(),
                outcome_id:candidate.map(|value|value.outcome.id.clone()),
                addition_id:reference.then(||Uuid::new_v5(&id,b"reference-target").to_string()),
                expected_feedback_sequence:prior.last().map_or(0, |revision| revision.sequence),
                reason_code:if reference {"identify_target"} else {"terminal_result_requires_review"}.into(),
                question:if reference {"No usable terminal candidate remained after semantic or geometry checks. If you can identify the target, add a reference box or category. Otherwise defer this request; no target has been inferred and no dataset annotation will be accepted."} else {"This saved sample result requires human review. Check the selected object or class and correct it if needed; this does not accept dataset annotations."}.into(),
                resume_checkpoint_ref:Uuid::new_v5(&id,b"prepared-repair-draft"),
            };
            requests.push(self.create_conversation_human_request(project, &input)?);
        }
        Ok(requests)
    }

    pub(crate) fn recover_conversation_corrections(&self) -> Result<()> {
        for (project, owner, input) in self.store.undelivered_conversation_corrections()? {
            if let Err(error) = self.resume_conversation_correction(
                &project,
                input.conversation_id,
                input.task_id,
                input.id,
            ) {
                self.store.record_conversation_resume_failure(
                    &owner,
                    input.id,
                    &error.to_string(),
                )?;
            }
        }
        Ok(())
    }

    pub fn continue_conversation_correction(
        &self,
        project: &str,
        conversation: Uuid,
        task: Uuid,
        id: Uuid,
    ) -> Result<ConversationHumanRequest> {
        let request = self
            .conversation_human_requests(project, conversation, task)?
            .into_iter()
            .find(|request| request.input.id == id)
            .context("Human request not found in this task")?;
        let owner = self.conversation_project_identity(project)?;
        if request.status == annotagent_storage::ConversationHumanRequestStatus::Applied {
            return Ok(request);
        }
        if request.status != annotagent_storage::ConversationHumanRequestStatus::Answered {
            bail!("Save a human answer before continuing");
        }
        if let Err(error) = self.resume_conversation_correction(project, conversation, task, id) {
            self.store
                .record_conversation_resume_failure(&owner, id, &error.to_string())?;
        }
        Ok(self.store.conversation_human_request(&owner, id)?)
    }
    /// Deterministic continuation of a saved correction: prepare an editable copy with
    /// exactly that answer's evidence, then acknowledge delivery. No model/Publish/Run.
    pub fn resume_conversation_correction(
        &self,
        project: &str,
        conversation: Uuid,
        task: Uuid,
        id: Uuid,
    ) -> Result<annotagent_core::WorkflowDraft> {
        let request = self
            .conversation_human_requests(project, conversation, task)?
            .into_iter()
            .find(|request| request.input.id == id)
            .context("Human request not found in this task")?;
        if !matches!(
            request.status,
            annotagent_storage::ConversationHumanRequestStatus::Answered
                | annotagent_storage::ConversationHumanRequestStatus::Applied
        ) {
            bail!("Human correction is not ready to resume");
        }
        let answer = request
            .answer
            .as_ref()
            .context("Human request has no saved answer")?;
        let owner = self.conversation_project_identity(project)?;
        let event = annotagent_storage::ConversationResumeEvent {
            request_id: id,
            task_id: task,
            checkpoint_ref: request.input.resume_checkpoint_ref,
            feedback_revision_id: answer.revision_id.clone(),
        };
        let copy_id = event.checkpoint_ref.to_string();
        // After a crash between copy and acknowledgment, retain the existing copy and
        // its frozen evidence even if the image or original Draft subsequently changed.
        if self.store.sample_plan_evidence(&copy_id)?.is_none() {
            self.validate_conversation_correction_subject(project, &request.input)?;
        }
        let draft = self.store.copy_sample_plan_for_feedback(
            &request.input.sample_test_id,
            project,
            &copy_id,
            &answer.revision_id,
        )?;
        self.store
            .complete_conversation_plan_resume(&owner, &event, &draft.id)?;
        Ok(draft)
    }
    pub fn cancel_conversation_human_request(
        &self,
        project: &str,
        conversation: Uuid,
        task: Uuid,
        id: Uuid,
    ) -> Result<ConversationHumanRequest> {
        if !self
            .conversation_human_requests(project, conversation, task)?
            .iter()
            .any(|request| request.input.id == id)
        {
            bail!("Human request not found in this task");
        }
        let owner = self.conversation_project_identity(project)?;
        Ok(self
            .store
            .close_conversation_human_request(&owner, id, false)?)
    }
    pub fn set_conversation_human_deferral(
        &self,
        project: &str,
        conversation: Uuid,
        task: Uuid,
        id: Uuid,
        input: &annotagent_storage::ConversationHumanDeferral,
    ) -> Result<ConversationHumanRequest> {
        if !self
            .conversation_human_requests(project, conversation, task)?
            .iter()
            .any(|request| request.input.id == id)
        {
            bail!("Human request not found in this task");
        }
        let owner = self.conversation_project_identity(project)?;
        Ok(self
            .store
            .set_conversation_human_deferral(&owner, id, input)?)
    }
    pub fn conversation_human_requests(
        &self,
        project: &str,
        conversation: Uuid,
        task: Uuid,
    ) -> Result<Vec<ConversationHumanRequest>> {
        let owner = self.conversation_project_identity(project)?;
        Ok(self
            .store
            .conversation_human_requests(&owner, conversation, task)?)
    }

    pub(crate) fn validate_conversation_correction_subject(
        &self,
        project: &str,
        input: &ConversationHumanRequestInput,
    ) -> Result<()> {
        let sample = self
            .store
            .get_workflow_sample_test_by_id(&input.sample_test_id)?
            .context("Sample Test not found")?;
        let image_id = annotagent_core::ImageId(Uuid::parse_str(&input.image_id)?);
        let image = self.project_image_path(project, image_id)?;
        let hash = annotagent_image_tools::sha256(&std::fs::read(image)?);
        validate_subject(project, &sample, input, &hash)
    }

    pub fn create_conversation_human_request(
        &self,
        project: &str,
        input: &ConversationHumanRequestInput,
    ) -> Result<ConversationHumanRequest> {
        let requests =
            self.conversation_human_requests(project, input.conversation_id, input.task_id)?;
        // Lost-response retry restores its frozen request, even if the current image moved on.
        if let Some(saved) = requests
            .into_iter()
            .find(|request| request.input.id == input.id)
        {
            if saved.input != *input {
                bail!("Human request idempotency conflict");
            }
            return Ok(saved);
        }
        self.validate_conversation_correction_subject(project, input)?;
        let owner = self.conversation_project_identity(project)?;
        Ok(self
            .store
            .create_conversation_human_request(&owner, input)?)
    }

    pub fn answer_conversation_human_request(
        &self,
        project: &str,
        conversation: Uuid,
        task: Uuid,
        id: Uuid,
        answer: &SampleFeedbackRevision,
    ) -> Result<ConversationHumanRequest> {
        let request = self
            .conversation_human_requests(project, conversation, task)?
            .into_iter()
            .find(|request| request.input.id == id)
            .context("Human request not found in this task")?;
        if let Some(saved) = &request.answer {
            if saved != answer {
                bail!("Human answer conflicts with the saved correction");
            }
            return Ok(request);
        }
        self.validate_conversation_correction_subject(project, &request.input)?;
        let owner = self.conversation_project_identity(project)?;
        Ok(self
            .store
            .answer_conversation_human_request(&owner, id, answer)?)
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    pub(crate) fn fixture() -> (WorkflowSampleTest, ConversationHumanRequestInput) {
        let now = chrono::Utc::now();
        let candidate = serde_json::json!({"source_artifact_id":Uuid::new_v4(),"source_artifact_ref":"terminal:1","lineage_id":"lineage","outcome":{"id":"final","label":"cup","confidence":0.8,"status":"needs_review","value":{"kind":"bounding_box","rect":[0.1,0.1,0.2,0.2]}},"localization":"test","geometry":"test","final_status":"test"});
        let test: WorkflowSampleTest=serde_json::from_value(serde_json::json!({
            "id":"sample", "draft_id":"draft", "project_id":"TEST", "draft_revision":1,"request_revision":1,
            "draft_content_hash":"draft-hash","image_set_hash":"images","model_snapshot_hash":"models","status":"passed",
            "inputs":[{"image_id":"image","content_hash":"pixels"}],"model_bindings":{},"started_at":now,"completed_at":now,
            "report":{"sandbox":true,"validation":{"valid":true,"issues":[],"execution_order":[]},"samples":[{
                "image_index":0,"image_name":"TEST.png","width":100,"height":100,"nodes":[],"outcomes":[],
                "projection":{"final_candidates":[candidate],"review_candidates":[],"committed_annotations":[],"no_target":false,"intermediate_artifact_ids":[]}
            }],"total_latency_ms":0,"estimated_cost":"unknown"}
        })).unwrap();
        let input = ConversationHumanRequestInput {
            id: Uuid::new_v4(),
            task_id: Uuid::new_v4(),
            conversation_id: Uuid::new_v4(),
            sample_test_id: "sample".into(),
            image_id: "image".into(),
            content_hash: "pixels".into(),
            outcome_id: Some("final".into()),
            addition_id: None,
            expected_feedback_sequence: 0,
            reason_code: "boundary".into(),
            question: "TEST".into(),
            resume_checkpoint_ref: Uuid::new_v4(),
        };
        (test, input)
    }

    #[test]
    fn assistance_uses_only_editable_terminal_review_results() {
        let (test, _) = fixture();
        let mut result = test.report.samples[0].clone();
        assert_eq!(first_review_subject(&result).unwrap().outcome.id, "final");
        result.projection.final_candidates[0].outcome.status =
            annotagent_core::SampleTestOutcomeStatus::ReadyToAccept;
        assert!(first_review_subject(&result).is_none());
        result.projection.final_candidates[0].outcome.status =
            annotagent_core::SampleTestOutcomeStatus::NeedsReview;
        result.projection.final_candidates[0].outcome.value = None;
        assert!(first_review_subject(&result).is_none());
        result.projection.final_candidates[0].outcome.value =
            Some(annotagent_core::VisionArtifactValue::Classification {
                labels: vec!["indoor".into()],
            });
        assert!(first_review_subject(&result).is_some());
        result
            .outcomes
            .push(result.projection.final_candidates[0].outcome.clone());
        result.projection.final_candidates.clear();
        result.projection.no_target = true;
        assert!(
            first_review_subject(&result).is_none(),
            "raw intermediate outcomes are not human correction targets"
        );
    }

    #[test]
    fn correction_subject_requires_owned_unchanged_terminal_sandbox_evidence() {
        let (mut test, mut input) = fixture();
        assert!(validate_subject("TEST", &test, &input, "pixels").is_ok());
        assert!(validate_subject("foreign", &test, &input, "pixels").is_err());
        assert!(validate_subject("TEST", &test, &input, "changed-pixels").is_err());
        input.outcome_id = Some("coarse-intermediate".into());
        assert!(validate_subject("TEST", &test, &input, "pixels").is_err());
        input.outcome_id = Some("final".into());
        test.report.samples[0].projection = annotagent_core::ResultProjection::default();
        assert!(validate_subject("TEST", &test, &input, "pixels").is_err());
    }

    #[test]
    fn reference_trigger_requires_quality_evidence_not_normal_empty_or_transport_failure() {
        use annotagent_core::AnnotationFailureClass as Failure;
        let (test, _) = fixture();
        let mut result = test.report.samples[0].clone();
        result.failure_classes = vec![Failure::GeometryError];
        assert!(!needs_reference_target(&result));
        result.projection.final_candidates.clear();
        result.projection.no_target = true;
        assert!(needs_reference_target(&result));
        for failure in [
            Failure::NoCandidate,
            Failure::MissingScore,
            Failure::InsufficientEvidence,
            Failure::ProviderFailure,
            Failure::InfrastructureFailure,
            Failure::BudgetLimit,
        ] {
            result.failure_classes = vec![failure];
            assert!(!needs_reference_target(&result));
        }
        result.failure_classes = vec![Failure::SemanticError];
        assert!(needs_reference_target(&result));
        result.failure_classes.push(Failure::ProviderFailure);
        assert!(!needs_reference_target(&result));
    }

    #[test]
    fn reference_subject_does_not_require_or_fabricate_a_terminal_outcome() {
        let (mut test, mut input) = fixture();
        test.report.samples[0].projection = annotagent_core::ResultProjection::default();
        test.report.samples[0].outcomes.clear();
        input.outcome_id = None;
        input.addition_id = Some(Uuid::new_v4().to_string());
        input.reason_code = "identify_target".into();
        assert!(validate_subject("TEST", &test, &input, "pixels").is_ok());
        assert!(validate_subject("foreign", &test, &input, "pixels").is_err());
        assert!(validate_subject("TEST", &test, &input, "changed-pixels").is_err());
        input.outcome_id = Some("fabricated".into());
        assert!(validate_subject("TEST", &test, &input, "pixels").is_err());
        input.outcome_id = None;
        input.addition_id = Some("not-an-identity".into());
        assert!(validate_subject("TEST", &test, &input, "pixels").is_err());
    }

    #[test]
    fn correction_commands_validate_live_pixels_and_restore_saved_answers_without_inference() {
        correction_command_scenario(false);
    }

    #[test]
    fn reference_assistance_recovers_after_restart_and_resumes_once_without_inference() {
        correction_command_scenario(true);
    }

    fn correction_command_scenario(reference: bool) {
        let temporary = tempfile::tempdir().unwrap();
        let app = LocalApplication::new(temporary.path()).unwrap();
        let project = "human-test";
        app.create_project(project,"version: 1\nproject:\n  name: TEST human correction\ndataset:\n  root: images\nruntime: {}\ntasks: []\nreview:\n  auto_accept_confidence: 0.9\n  force_review_below: 0.5\nexport:\n  formats: [native]\n").unwrap();
        let source =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/robocup/images");
        let staging = temporary.path().join("TEST-import");
        std::fs::create_dir(&staging).unwrap();
        std::fs::copy(
            source.join("synthetic-robocup.png"),
            staging.join("TEST.png"),
        )
        .unwrap();
        app.import_images(project, &staging).unwrap();
        let image = app.list_project_image_summaries(project).unwrap().remove(0);
        let conversation = app.create_project_conversation(project).unwrap();
        let message = annotagent_storage::ConversationMessageInput {
            reference: None,
            id: Uuid::new_v4(),
            text: "TEST target".into(),
            image: None,
        };
        app.append_project_conversation_message(project, conversation, &message)
            .unwrap();
        let task = annotagent_storage::BeginConversationTask {
            id: Uuid::new_v4(),
            source_message_id: message.id,
            schema_revision: app.project_goal(project).unwrap()["revision"]
                .as_str()
                .unwrap()
                .into(),
        };
        app.begin_conversation_task(project, conversation, &task)
            .unwrap();
        let (mut sample, mut input) = fixture();
        sample.project_id = project.into();
        let baseline:annotagent_core::WorkflowDraft=serde_json::from_value(serde_json::json!({"id":sample.draft_id,"project_id":project,"name":"TEST baseline","status":"editing","nodes":[],"created_at":chrono::Utc::now(),"updated_at":chrono::Utc::now()})).unwrap();
        app.store.save_workflow_draft(&baseline).unwrap();
        let baseline = app.store.get_workflow_draft(&sample.draft_id).unwrap();
        sample.draft_revision = baseline.revision;
        sample.draft_content_hash = baseline.content_hash.clone();
        sample.inputs[0].image_id = image.image_id.to_string();
        sample.inputs[0].content_hash = image.content_hash.clone();
        let outcome = sample.report.samples[0].projection.final_candidates[0]
            .outcome
            .clone();
        sample.report.samples[0].outcomes.push(outcome.clone());
        if reference {
            sample.report.samples[0].outcomes.clear();
            sample.report.samples[0].projection.final_candidates.clear();
            sample.report.samples[0].projection.no_target = true;
            sample.report.samples[0].failure_classes =
                vec![annotagent_core::AnnotationFailureClass::GeometryError];
            app.store
                .save_sample_scope_seal(
                    &sample.id,
                    &serde_json::json!({"annotation_schema":{"task":{"kind":"bounding_box"}}}),
                )
                .unwrap();
        }
        app.store.save_workflow_sample_test(&sample).unwrap();
        app.store.reserve_sample_operation(&annotagent_storage::SampleOperation { id:sample.id.clone(),project_id:project.into(),draft_id:sample.draft_id.clone(),authorization_fingerprint:"TEST".into(),request:serde_json::json!({"conversation":{"conversation_id":conversation,"task_id":task.id,"human_review":true}}),status:"queued".into(),error:None,created_at:chrono::Utc::now().to_rfc3339(),updated_at:chrono::Utc::now().to_rfc3339() }).unwrap();
        input.conversation_id = conversation;
        input.task_id = task.id;
        input.image_id = image.image_id.to_string();
        input.content_hash = image.content_hash;
        assert!(app.store.pending_sample_assistance().unwrap().is_empty());
        app.store.finish_sample_operation(&sample.id, None).unwrap();
        assert_eq!(app.store.pending_sample_assistance().unwrap().len(), 1);
        // Crash after report/operation completion, before local assistance delivery.
        drop(app);
        let app = LocalApplication::new(temporary.path()).unwrap();
        assert!(app.store.pending_sample_assistance().unwrap().is_empty());
        assert_eq!(
            app.store
                .sample_assistance_status(&sample.id)
                .unwrap()
                .unwrap()["status"],
            "completed"
        );
        let prepared = app
            .prepare_conversation_sample_requests(project, conversation, task.id, &sample.id)
            .unwrap();
        assert_eq!(prepared.len(), 1);
        if reference {
            assert!(prepared[0].input.outcome_id.is_none());
            assert!(prepared[0].input.addition_id.is_some());
            assert_eq!(prepared[0].input.reason_code, "identify_target");
        } else {
            assert_eq!(prepared[0].input.outcome_id, input.outcome_id);
        }
        input = prepared[0].input.clone();
        assert_eq!(
            app.prepare_conversation_sample_requests(project, conversation, task.id, &sample.id)
                .unwrap(),
            prepared
        );
        assert!(
            app.prepare_conversation_sample_requests(
                project,
                conversation,
                Uuid::new_v4(),
                &sample.id
            )
            .is_err()
        );
        app.create_conversation_human_request(project, &input)
            .unwrap();
        let answer = SampleFeedbackRevision {
            revision_id: Uuid::new_v4().to_string(),
            sample_test_id: sample.id,
            image_id: input.image_id.clone(),
            sequence: 1,
            reason: if reference {
                annotagent_storage::SampleFeedbackReason::MissingTarget
            } else {
                annotagent_storage::SampleFeedbackReason::Correct
            },
            outcome_id: input.outcome_id.clone(),
            corrected_value: outcome.value,
            corrected_label: reference.then(|| outcome.label.clone()),
            addition_id: input.addition_id.clone(),
            note: "TEST".into(),
            created_at: chrono::Utc::now(),
        };
        let path = app.project_image_path(project, image.image_id).unwrap();
        let original = std::fs::read(&path).unwrap();
        std::fs::write(&path, b"TEST changed pixels").unwrap();
        assert!(
            app.answer_conversation_human_request(
                project,
                conversation,
                task.id,
                input.id,
                &answer
            )
            .is_err()
        );
        assert!(
            app.store
                .sample_feedback(&input.sample_test_id, &input.image_id)
                .unwrap()
                .is_empty()
        );
        std::fs::write(&path, &original).unwrap();
        let saved = app
            .answer_conversation_human_request(project, conversation, task.id, input.id, &answer)
            .unwrap();
        std::fs::write(&path, b"TEST later change").unwrap();
        assert_eq!(
            app.answer_conversation_human_request(
                project,
                conversation,
                task.id,
                input.id,
                &answer
            )
            .unwrap(),
            saved
        );
        assert_eq!(
            app.create_conversation_human_request(project, &input)
                .unwrap(),
            saved
        );
        assert!(
            app.answer_conversation_human_request(
                project,
                conversation,
                Uuid::new_v4(),
                input.id,
                &answer
            )
            .is_err()
        );
        let owner = app.conversation_project_identity(project).unwrap();
        assert!(
            app.store
                .conversation_call_history(&owner, task.id)
                .unwrap()
                .is_empty()
        );
        assert!(
            app.resume_conversation_correction(project, conversation, task.id, input.id)
                .is_err()
        );
        assert_eq!(
            app.store
                .pending_conversation_resumes(&owner, conversation, task.id)
                .unwrap()
                .len(),
            1
        );
        std::fs::write(&path, &original).unwrap();
        // Simulate the crash gap: the copy exists but outbox delivery was not acknowledged.
        let mut later = answer.clone();
        later.revision_id = Uuid::new_v4().to_string();
        later.sequence += 1;
        later.note = "TEST later correction outside this checkpoint".into();
        app.store.save_sample_feedback(&later).unwrap();
        let copy = app
            .store
            .copy_sample_plan_for_feedback(
                &input.sample_test_id,
                project,
                &input.resume_checkpoint_ref.to_string(),
                &answer.revision_id,
            )
            .unwrap();
        let evidence = app.store.sample_plan_evidence(&copy.id).unwrap().unwrap();
        let included: Vec<SampleFeedbackRevision> =
            serde_json::from_value(evidence["feedback"].clone()).unwrap();
        assert_eq!(included, vec![answer.clone()]);
        assert!(
            app.store
                .copy_sample_plan_for_feedback(
                    &input.sample_test_id,
                    project,
                    &copy.id,
                    &later.revision_id
                )
                .is_err()
        );
        drop(app);
        let app = LocalApplication::new(temporary.path()).unwrap();
        let recovered = app
            .store
            .conversation_human_request(&owner, input.id)
            .unwrap();
        assert_eq!(
            recovered.status,
            annotagent_storage::ConversationHumanRequestStatus::Applied
        );
        assert_eq!(recovered.resume_draft_id.as_deref(), Some(copy.id.as_str()));
        app.store
            .record_conversation_resume_failure(&owner, input.id, "TEST late competing failure")
            .unwrap();
        assert_eq!(
            app.store
                .conversation_human_request(&owner, input.id)
                .unwrap(),
            recovered
        );
        let resumed = app
            .resume_conversation_correction(project, conversation, task.id, input.id)
            .unwrap();
        assert_eq!(copy, resumed);
        assert_eq!(
            app.resume_conversation_correction(project, conversation, task.id, input.id)
                .unwrap(),
            resumed
        );
        assert!(
            app.store
                .pending_conversation_resumes(&owner, conversation, task.id)
                .unwrap()
                .is_empty()
        );
        assert_eq!(
            app.store.get_workflow_draft(&baseline.id).unwrap(),
            baseline
        );
        assert!(
            app.store
                .conversation_call_history(&owner, task.id)
                .unwrap()
                .is_empty()
        );
        let mut next = input.clone();
        next.id = Uuid::new_v4();
        next.resume_checkpoint_ref = Uuid::new_v4();
        next.expected_feedback_sequence = later.sequence;
        if reference {
            next.addition_id = Some(Uuid::new_v4().to_string());
        }
        app.create_conversation_human_request(project, &next)
            .unwrap();
        let mut next_answer = later.clone();
        next_answer.revision_id = Uuid::new_v4().to_string();
        next_answer.sequence += 1;
        next_answer.addition_id = next.addition_id.clone();
        app.answer_conversation_human_request(
            project,
            conversation,
            task.id,
            next.id,
            &next_answer,
        )
        .unwrap();
        std::fs::write(&path, b"TEST changed before resume").unwrap();
        let failed = app
            .continue_conversation_correction(project, conversation, task.id, next.id)
            .unwrap();
        assert!(failed.resume_error.is_some());
        assert!(failed.resume_draft_id.is_none());
        assert!(
            app.store
                .undelivered_conversation_corrections()
                .unwrap()
                .is_empty()
        );
        drop(app);
        let app = LocalApplication::new(temporary.path()).unwrap();
        assert_eq!(
            app.store
                .conversation_human_request(&owner, next.id)
                .unwrap(),
            failed
        );
        std::fs::write(&path, &original).unwrap();
        let retried = app
            .continue_conversation_correction(project, conversation, task.id, next.id)
            .unwrap();
        assert!(retried.resume_error.is_none());
        assert_eq!(
            retried.resume_draft_id,
            Some(next.resume_checkpoint_ref.to_string())
        );
    }
}
