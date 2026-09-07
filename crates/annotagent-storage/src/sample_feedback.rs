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
    pub note: String,
    pub created_at: DateTime<Utc>,
}

impl SqliteStore {
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
        if let Some(id) = &feedback.outcome_id {
            let original = sample
                .outcomes
                .iter()
                .find(|outcome| &outcome.id == id)
                .ok_or_else(|| {
                    StorageError::InvalidEnum("Outcome does not belong to Sample Test image".into())
                })?;
            if let Some(value) = &feedback.corrected_value {
                // First editable geometry is a box. Other feedback remains supported without edits.
                let (
                    annotagent_core::VisionArtifactValue::BoundingBox { rect },
                    Some(annotagent_core::VisionArtifactValue::BoundingBox { .. }),
                ) = (value, &original.value)
                else {
                    return Err(StorageError::InvalidEnum(
                        "Only bounding-box corrections are currently supported".into(),
                    ));
                };
                if rect.width() <= 0.0 || rect.height() <= 0.0 {
                    return Err(StorageError::InvalidEnum(
                        "Corrected box must be within the original image".into(),
                    ));
                }
            }
        } else if feedback.corrected_value.is_some() {
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
