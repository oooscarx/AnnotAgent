//! Scope validation for human corrections. No model invocation or independent executor.
use crate::LocalApplication;
use annotagent_storage::{
    ConversationHumanRequest, ConversationHumanRequestInput, SampleFeedbackRevision,
    WorkflowSampleTest,
};
use anyhow::{Context, Result, bail};
use uuid::Uuid;

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
    let terminal = projection
        .final_candidates
        .iter()
        .chain(
            projection
                .review_candidates
                .iter()
                .map(|review| &review.candidate),
        )
        .any(|candidate| candidate.outcome.id == input.outcome_id);
    if !terminal {
        bail!("Requested outcome is not a terminal candidate on this sample image");
    }
    Ok(())
}

impl LocalApplication {
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

    fn validate_conversation_correction_subject(
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
mod tests {
    use super::*;
    fn fixture() -> (WorkflowSampleTest, ConversationHumanRequestInput) {
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
            outcome_id: "final".into(),
            expected_feedback_sequence: 0,
            reason_code: "boundary".into(),
            question: "TEST".into(),
            resume_checkpoint_ref: Uuid::new_v4(),
        };
        (test, input)
    }

    #[test]
    fn correction_subject_requires_owned_unchanged_terminal_sandbox_evidence() {
        let (mut test, mut input) = fixture();
        assert!(validate_subject("TEST", &test, &input, "pixels").is_ok());
        assert!(validate_subject("foreign", &test, &input, "pixels").is_err());
        assert!(validate_subject("TEST", &test, &input, "changed-pixels").is_err());
        input.outcome_id = "coarse-intermediate".into();
        assert!(validate_subject("TEST", &test, &input, "pixels").is_err());
        input.outcome_id = "final".into();
        test.report.samples[0].projection = annotagent_core::ResultProjection::default();
        assert!(validate_subject("TEST", &test, &input, "pixels").is_err());
    }

    #[test]
    fn correction_commands_validate_live_pixels_and_restore_saved_answers_without_inference() {
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
        sample.inputs[0].image_id = image.image_id.to_string();
        sample.inputs[0].content_hash = image.content_hash.clone();
        let outcome = sample.report.samples[0].projection.final_candidates[0]
            .outcome
            .clone();
        sample.report.samples[0].outcomes.push(outcome.clone());
        app.store.save_workflow_sample_test(&sample).unwrap();
        app.store.reserve_sample_operation(&annotagent_storage::SampleOperation { id:sample.id.clone(),project_id:project.into(),draft_id:sample.draft_id.clone(),authorization_fingerprint:"TEST".into(),request:serde_json::json!({"conversation":{"conversation_id":conversation,"task_id":task.id}}),status:"queued".into(),error:None,created_at:chrono::Utc::now().to_rfc3339(),updated_at:chrono::Utc::now().to_rfc3339() }).unwrap();
        input.conversation_id = conversation;
        input.task_id = task.id;
        input.image_id = image.image_id.to_string();
        input.content_hash = image.content_hash;
        app.create_conversation_human_request(project, &input)
            .unwrap();
        let answer = SampleFeedbackRevision {
            revision_id: Uuid::new_v4().to_string(),
            sample_test_id: sample.id,
            image_id: input.image_id.clone(),
            sequence: 1,
            reason: annotagent_storage::SampleFeedbackReason::Correct,
            outcome_id: Some(input.outcome_id.clone()),
            corrected_value: outcome.value,
            corrected_label: None,
            addition_id: None,
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
    }
}
