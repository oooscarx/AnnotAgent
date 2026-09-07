//! Human feedback on immutable sample results; never writes formal annotations.
use super::{DateTime, OptionalExtension, SqliteStore, StorageError, Utc, params};

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SampleFeedbackReason {
    Correct,
    WrongTarget,
    PoorBoundary,
    MissingTarget,
    CannotJudge,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{WorkflowSampleTest, WorkflowSampleTestInput, WorkflowSampleTestStatus};
    use annotagent_core::WorkflowDryRunReport;
    use std::collections::BTreeMap;

    fn fixture(store: &SqliteStore) -> SampleFeedbackRevision {
        let sample: annotagent_core::WorkflowDryRunSampleResult = serde_json::from_value(serde_json::json!({
            "image_index": 7, "image_name": "same-name.png", "width": 640, "height": 400, "nodes": [],
            "outcomes": [{"id": "final-1", "label": "ball", "confidence": 0.9, "status": "needs_review", "value": {"kind": "bounding_box", "rect": [0.5, 0.5, 0.1, 0.1]}}]
        })).unwrap();
        let test = WorkflowSampleTest {
            id: "test-1".into(),
            draft_id: "draft-1".into(),
            project_id: "project-1".into(),
            draft_revision: 1,
            request_revision: 1,
            draft_content_hash: "draft-hash".into(),
            image_set_hash: "image-hash".into(),
            model_snapshot_hash: "model-hash".into(),
            status: WorkflowSampleTestStatus::Passed,
            inputs: vec![WorkflowSampleTestInput {
                image_id: "stable-image-1".into(),
                content_hash: "content-hash".into(),
            }],
            model_bindings: BTreeMap::new(),
            report: WorkflowDryRunReport {
                sandbox: true,
                validation: annotagent_core::WorkflowValidationReport {
                    valid: true,
                    issues: Vec::new(),
                    execution_order: Vec::new(),
                },
                samples: vec![sample],
                summary: annotagent_core::WorkflowDryRunSummary::default(),
                total_latency_ms: 0,
                estimated_cost: "0".into(),
            },
            started_at: Utc::now(),
            completed_at: Utc::now(),
        };
        store.save_workflow_sample_test(&test).unwrap();
        SampleFeedbackRevision {
            revision_id: "feedback-1".into(),
            sample_test_id: test.id,
            image_id: "stable-image-1".into(),
            sequence: 1,
            reason: SampleFeedbackReason::PoorBoundary,
            outcome_id: Some("final-1".into()),
            corrected_label: None,
            corrected_value: Some(
                serde_json::from_value(
                    serde_json::json!({"kind": "bounding_box", "rect": [0.51, 0.52, 0.08, 0.09]}),
                )
                .unwrap(),
            ),
            note: "Tighter boundary".into(),
            created_at: Utc::now(),
        }
    }

    #[test]
    fn sample_plan_copy_preserves_parent_feedback_and_retry_progress() {
        let store = SqliteStore::open_in_memory().unwrap();
        let feedback = fixture(&store);
        let draft: annotagent_core::WorkflowDraft = serde_json::from_value(serde_json::json!({
            "id":"draft-1", "project_id":"project-1", "name":"Original", "status":"editing",
            "nodes":[], "created_at":Utc::now(), "updated_at":Utc::now()
        }))
        .unwrap();
        store.save_workflow_draft(&draft).unwrap();
        let parent = store.get_workflow_draft("draft-1").unwrap();
        // The saved test fixture is immutable; create a separate exact-revision sample.
        let mut sample = store
            .get_workflow_sample_test_by_id("test-1")
            .unwrap()
            .unwrap();
        sample.id = "exact-test".into();
        sample.draft_content_hash = parent.content_hash.clone();
        sample.draft_revision = parent.revision;
        store.save_workflow_sample_test(&sample).unwrap();
        assert!(
            store
                .copy_sample_plan("exact-test", "project-1", "copy")
                .is_err()
        );
        let mut feedback = feedback;
        feedback.sample_test_id = "exact-test".into();
        store.save_sample_feedback(&feedback).unwrap();
        assert!(
            store
                .copy_sample_plan("exact-test", "wrong-project", "copy")
                .is_err()
        );
        assert!(
            store
                .copy_sample_plan("exact-test", "project-1", "draft-1")
                .is_err()
        );
        let mut copy = store
            .copy_sample_plan("exact-test", "project-1", "copy")
            .unwrap();
        assert_eq!(copy.nodes, parent.nodes);
        let evidence = store.sample_plan_evidence("copy").unwrap().unwrap();
        copy.name = "Revised by Builder".into();
        store.save_workflow_draft(&copy).unwrap();
        let revised = store.get_workflow_draft("copy").unwrap();
        assert_eq!(
            store
                .copy_sample_plan("exact-test", "project-1", "copy")
                .unwrap(),
            revised
        );
        feedback.sequence += 1;
        feedback.revision_id = "later-feedback".into();
        feedback.note = "Subsequent evidence".into();
        store.save_sample_feedback(&feedback).unwrap();
        assert_eq!(
            store.sample_plan_evidence("copy").unwrap().unwrap(),
            evidence
        );
        assert_eq!(store.get_workflow_draft("draft-1").unwrap(), parent);
        let mut changed = parent;
        changed.name = "Concurrent edit".into();
        store.save_workflow_draft(&changed).unwrap();
        assert!(
            store
                .copy_sample_plan("exact-test", "project-1", "new-copy")
                .is_err()
        );
        assert!(
            store
                .get_workflow_draft_optional("new-copy")
                .unwrap()
                .is_none()
        );
    }

    #[test]
    fn sample_feedback_persists_revisions_without_mutating_predictions() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("feedback.sqlite");
        let store = SqliteStore::open(&path).unwrap();
        let mut feedback = fixture(&store);
        let original = store.get_workflow_sample_test_by_id("test-1").unwrap();
        store.save_sample_feedback(&feedback).unwrap();
        store.save_sample_feedback(&feedback).unwrap();
        assert_eq!(
            store
                .sample_feedback("test-1", "stable-image-1")
                .unwrap()
                .len(),
            1
        );
        for reason in [
            SampleFeedbackReason::WrongTarget,
            SampleFeedbackReason::MissingTarget,
            SampleFeedbackReason::CannotJudge,
            SampleFeedbackReason::Correct,
        ] {
            feedback.sequence += 1;
            feedback.revision_id = format!("feedback-{}", feedback.sequence);
            feedback.reason = reason;
            feedback.outcome_id = None;
            feedback.corrected_value = None;
            store.save_sample_feedback(&feedback).unwrap();
        }
        assert_eq!(
            store.get_workflow_sample_test_by_id("test-1").unwrap(),
            original
        );
        drop(store);
        let restored = SqliteStore::open(path)
            .unwrap()
            .sample_feedback("test-1", "stable-image-1")
            .unwrap();
        assert_eq!(restored.len(), 5);
        assert_eq!(restored.last(), Some(&feedback));
    }

    #[test]
    fn sample_feedback_rejects_wrong_image_outcome_and_stale_revision() {
        let store = SqliteStore::open_in_memory().unwrap();
        let feedback = fixture(&store);
        let mut invalid = feedback.clone();
        invalid.image_id = "another-stable-image".into();
        assert!(store.save_sample_feedback(&invalid).is_err());
        invalid = feedback.clone();
        invalid.outcome_id = Some("intermediate-box".into());
        assert!(store.save_sample_feedback(&invalid).is_err());
        store.save_sample_feedback(&feedback).unwrap();
        invalid = feedback;
        invalid.revision_id = "concurrent-edit".into();
        assert!(store.save_sample_feedback(&invalid).is_err());
        assert_eq!(
            store
                .sample_feedback("test-1", "stable-image-1")
                .unwrap()
                .len(),
            1
        );
    }

    #[test]
    fn sample_feedback_accepts_typed_label_and_polygon_edits_not_type_changes() {
        for (original, correction) in [
            (
                serde_json::json!({"kind":"classification","labels":["day"]}),
                serde_json::json!({"kind":"classification","labels":["night"]}),
            ),
            (
                serde_json::json!({"kind":"polygon","rings":[[[0.1,0.1],[0.4,0.1],[0.3,0.4]]]}),
                serde_json::json!({"kind":"polygon","rings":[[[0.2,0.1],[0.4,0.1],[0.3,0.4]]]}),
            ),
        ] {
            let store = SqliteStore::open_in_memory().unwrap();
            let mut feedback = fixture(&store);
            let mut sample = store
                .get_workflow_sample_test_by_id("test-1")
                .unwrap()
                .unwrap();
            sample.id = "typed-test".into();
            sample.report.samples[0].outcomes[0].value =
                Some(serde_json::from_value(original).unwrap());
            store.save_workflow_sample_test(&sample).unwrap();
            feedback.sample_test_id = sample.id.clone();
            feedback.corrected_label = Some("Corrected target".into());
            feedback.corrected_value = Some(serde_json::from_value(correction).unwrap());
            store.save_sample_feedback(&feedback).unwrap();
            assert_eq!(
                store
                    .sample_feedback(&sample.id, &feedback.image_id)
                    .unwrap(),
                vec![feedback.clone()]
            );
            assert_eq!(
                store.get_workflow_sample_test_by_id(&sample.id).unwrap(),
                Some(sample)
            );
            feedback.revision_id = "invalid-edit".into();
            feedback.sequence = 2;
            feedback.corrected_value = Some(
                serde_json::from_value(
                    serde_json::json!({"kind":"bounding_box","rect":[0.1,0.1,0.2,0.2]}),
                )
                .unwrap(),
            );
            assert!(store.save_sample_feedback(&feedback).is_err());
            feedback.corrected_value = None;
            feedback.corrected_label = Some(" ".into());
            assert!(store.save_sample_feedback(&feedback).is_err());
        }
    }
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct SampleFeedbackRevision {
    pub revision_id: String,
    pub sample_test_id: String,
    pub image_id: String,
    pub sequence: u64,
    pub reason: SampleFeedbackReason,
    pub outcome_id: Option<String>,
    pub corrected_value: Option<annotagent_core::VisionArtifactValue>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub corrected_label: Option<String>,
    pub note: String,
    pub created_at: DateTime<Utc>,
}

impl SqliteStore {
    /// Frozen authoring evidence, not a claim that the model improved its predictions.
    pub fn sample_plan_evidence(
        &self,
        draft_id: &str,
    ) -> Result<Option<serde_json::Value>, StorageError> {
        self.with_connection(|connection| {
            let value: Option<(String, String, String)> = connection.query_row(
                "SELECT project_id,sample_test_id,feedback_json FROM sample_plan_revisions WHERE draft_id=?1", [draft_id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            ).optional()?;
            value.map(|(project, test, feedback)| Ok(serde_json::json!({"project_id": project, "sample_test_id": test, "feedback": serde_json::from_str::<serde_json::Value>(&feedback)?}))).transpose()
        })
    }

    /// Preserve a tested plan before invoking the existing Builder in repair mode.
    /// The UUID is an idempotency key; retries never overwrite the working copy.
    pub fn copy_sample_plan(
        &self,
        test_id: &str,
        project_id: &str,
        copy_id: &str,
    ) -> Result<annotagent_core::WorkflowDraft, StorageError> {
        let test = self
            .get_workflow_sample_test_by_id(test_id)?
            .ok_or_else(|| StorageError::InvalidEnum("Sample Test not found".into()))?;
        if test.project_id != project_id {
            return Err(StorageError::InvalidEnum(
                "Sample Test belongs to another Project".into(),
            ));
        }
        if let Some(evidence) = self.sample_plan_evidence(copy_id)? {
            if evidence["sample_test_id"] != test_id || evidence["project_id"] != project_id {
                return Err(StorageError::InvalidEnum(
                    "Copy key belongs to another sample".into(),
                ));
            }
            return self.get_workflow_draft(copy_id);
        }
        let mut draft = self.get_workflow_draft(&test.draft_id)?;
        if draft.revision != test.draft_revision || draft.content_hash != test.draft_content_hash {
            return Err(StorageError::InvalidEnum(
                "The sample plan changed; test its current revision before improving it".into(),
            ));
        }
        let mut feedback = Vec::new();
        for image in &test.inputs {
            feedback.extend(self.sample_feedback(test_id, &image.image_id)?);
        }
        if feedback.is_empty() {
            return Err(StorageError::InvalidEnum(
                "Save sample feedback before requesting an improvement".into(),
            ));
        }
        let evidence = serde_json::to_string(&feedback)?;
        if evidence.len() > 32_000 {
            return Err(StorageError::InvalidEnum(
                "Sample feedback is too large for bounded planning; use a smaller Sample Test"
                    .into(),
            ));
        }
        copy_id.clone_into(&mut draft.id);
        draft.name = format!("{} · sample revision", draft.name);
        draft.status = annotagent_core::WorkflowDraftStatus::Editing;
        draft.revision = 1;
        draft.content_hash.clear();
        draft.geometry_risk_acceptance = None;
        draft.created_at = Utc::now();
        draft.updated_at = draft.created_at;
        let draft = super::persisted_workflow_draft(&draft, None)?;
        let expected_revision = i64::try_from(test.draft_revision).map_err(|_| {
            StorageError::InvalidEnum("Sample revision exceeds storage range".into())
        })?;
        self.with_connection(|connection| {
            let transaction = connection.unchecked_transaction()?;
            let valid: bool = transaction.query_row("SELECT EXISTS(SELECT 1 FROM workflow_drafts WHERE id=?1 AND project_id=?2 AND revision=?3 AND content_hash=?4 AND deleted_at IS NULL AND archived_at IS NULL)", params![test.draft_id, project_id, expected_revision, test.draft_content_hash], |row| row.get(0))?;
            if !valid { return Err(StorageError::InvalidEnum("The original sample plan is no longer current".into())); }
            transaction.execute("INSERT INTO workflow_drafts (id,project_id,status,draft_json,created_at,updated_at,revision,content_hash) VALUES (?1,?2,'editing',?3,?4,?4,1,?5)", params![copy_id,project_id,serde_json::to_string(&draft)?,draft.created_at.to_rfc3339(),draft.content_hash])?;
            transaction.execute("INSERT INTO workflow_pipelines (workflow_id,project_id,display_name,lifecycle_revision,created_at,updated_at) VALUES (?1,?2,?3,1,?4,?4)",params![copy_id,project_id,draft.name,draft.created_at.to_rfc3339()])?;
            transaction.execute("INSERT INTO sample_plan_revisions VALUES (?1,?2,?3,?4,?5)", params![copy_id,project_id,test_id,evidence,Utc::now().to_rfc3339()])?;
            transaction.commit()?;
            Ok(())
        })?;
        self.get_workflow_draft(copy_id)
    }

    pub fn sample_feedback(
        &self,
        test_id: &str,
        image_id: &str,
    ) -> Result<Vec<SampleFeedbackRevision>, StorageError> {
        self.with_connection(|connection| {
            let mut statement = connection.prepare("SELECT feedback_json FROM sample_feedback_revisions WHERE sample_test_id = ?1 AND image_id = ?2 ORDER BY sequence")?;
            let rows = statement.query_map(params![test_id, image_id], |row| row.get::<_, String>(0))?;
            rows.map(|row| Ok(serde_json::from_str(&row?)?)).collect()
        })
    }

    /// Optimistic sequence prevents silent overwrites; a retried identical revision is a no-op.
    pub fn save_sample_feedback(
        &self,
        feedback: &SampleFeedbackRevision,
    ) -> Result<(), StorageError> {
        let test = self
            .get_workflow_sample_test_by_id(&feedback.sample_test_id)?
            .ok_or_else(|| StorageError::InvalidEnum("Sample Test not found".into()))?;
        let position = test
            .inputs
            .iter()
            .position(|input| input.image_id == feedback.image_id)
            .ok_or_else(|| {
                StorageError::InvalidEnum("Image does not belong to Sample Test".into())
            })?;
        let sample = test
            .report
            .samples
            .get(position)
            .ok_or_else(|| StorageError::InvalidEnum("Sample result not found".into()))?;
        if feedback.note.len() > 4000 || feedback.sequence == 0 {
            return Err(StorageError::InvalidEnum(
                "Invalid feedback note or sequence".into(),
            ));
        }
        if feedback
            .corrected_label
            .as_ref()
            .is_some_and(|label| label.trim().is_empty() || label.len() > 256)
        {
            return Err(StorageError::InvalidEnum(
                "A corrected label must contain 1–256 bytes".into(),
            ));
        }
        if let Some(id) = &feedback.outcome_id {
            let original = sample
                .outcomes
                .iter()
                .find(|outcome| &outcome.id == id)
                .ok_or_else(|| {
                    StorageError::InvalidEnum("Outcome does not belong to Sample Test image".into())
                })?;
            if let Some(value) = &feedback.corrected_value {
                if original.value.as_ref().is_none_or(|original| {
                    std::mem::discriminant(value) != std::mem::discriminant(original)
                }) {
                    return Err(StorageError::InvalidEnum(
                        "A sample correction must retain its original output type".into(),
                    ));
                }
                value.validate().map_err(|error| {
                    StorageError::InvalidEnum(format!("Invalid sample correction: {error}"))
                })?;
                if let annotagent_core::VisionArtifactValue::BoundingBox { rect } = value
                    && (rect.width() <= 0.0 || rect.height() <= 0.0)
                {
                    return Err(StorageError::InvalidEnum(
                        "Corrected box must have positive dimensions".into(),
                    ));
                }
            }
        } else if feedback.corrected_value.is_some() || feedback.corrected_label.is_some() {
            return Err(StorageError::InvalidEnum(
                "A correction requires a source outcome".into(),
            ));
        }
        self.with_connection(|connection| {
            let transaction = connection.unchecked_transaction()?;
            let json = serde_json::to_string(feedback)?;
            let existing: Option<String> = transaction.query_row("SELECT feedback_json FROM sample_feedback_revisions WHERE revision_id = ?1", [&feedback.revision_id], |row| row.get(0)).optional()?;
            if let Some(existing) = existing {
                if existing == json { return Ok(()); }
                return Err(StorageError::InvalidEnum("Feedback revision conflict".into()));
            }
            let sequence: i64 = transaction.query_row("SELECT COALESCE(MAX(sequence), 0) FROM sample_feedback_revisions WHERE sample_test_id = ?1 AND image_id = ?2", params![feedback.sample_test_id, feedback.image_id], |row| row.get(0))?;
            let next = i64::try_from(feedback.sequence).map_err(|_| StorageError::InvalidEnum("Feedback sequence is too large".into()))?;
            if next != sequence + 1 {
                return Err(StorageError::InvalidEnum("Feedback changed in another window; reload before saving".into()));
            }
            transaction.execute("INSERT INTO sample_feedback_revisions VALUES (?1, ?2, ?3, ?4, ?5)", params![feedback.revision_id, feedback.sample_test_id, feedback.image_id, next, json])?;
            transaction.commit()?;
            Ok(())
        })
    }
}
