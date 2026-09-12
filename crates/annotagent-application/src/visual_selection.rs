//! Canonical read-only Sample selections for safe Conversation references.
use crate::LocalApplication;
use anyhow::{Result, ensure};
use serde_json::{Value, json};
use uuid::Uuid;

impl LocalApplication {
    pub fn conversation_visual_selections(
        &self,
        project: &str,
        conversation: Uuid,
        task: Uuid,
        cursor: usize,
        limit: usize,
    ) -> Result<Value> {
        ensure!((1..=20).contains(&limit), "Selection limit must be 1..20");
        let task_record = self
            .conversation_tasks(project, conversation)?
            .into_iter()
            .find(|record| record.input.id == task)
            .ok_or_else(|| anyhow::anyhow!("Task belongs to another conversation"))?;
        let operations = self
            .store
            .conversation_sample_operations(project, conversation, task)?;
        ensure!(
            cursor <= operations.len(),
            "Selection cursor is outside history"
        );
        let end = cursor.saturating_add(limit).min(operations.len());
        let mut items = Vec::new();
        for operation in &operations[cursor..end] {
            let Some(sample) = self.store.get_workflow_sample_test_by_id(&operation.id)? else {
                items.push(json!({
                    "sample_test_id":operation.id,"status":operation.status,
                    "error":operation.error,"result_available":false,"images":[]
                }));
                continue;
            };
            ensure!(
                sample.project_id == project && sample.draft_id == operation.draft_id,
                "Sample result ownership changed"
            );
            ensure!(
                sample.inputs.len() == sample.report.samples.len(),
                "Sample input/result cardinality changed"
            );
            let images = sample
                .inputs
                .iter()
                .zip(&sample.report.samples)
                .map(|(input, result)| {
                    let candidates = result
                        .projection
                        .final_candidates
                        .iter()
                        .chain(
                            result
                                .projection
                                .review_candidates
                                .iter()
                                .map(|review| &review.candidate),
                        )
                        .map(|candidate| {
                            let artifact = (!candidate.source_artifact_id.0.is_nil())
                                .then_some(candidate.source_artifact_id.0);
                            let value = serde_json::to_value(&candidate.outcome.value)?;
                            Ok(json!({
                                "candidate_id":candidate.outcome.id,
                                "annotation_kind":value.get("kind").and_then(Value::as_str),
                                "label":candidate.outcome.label,
                                "source_artifact_id":artifact,
                                "feedback_available":artifact.is_some()
                            }))
                        })
                        .collect::<Result<Vec<_>>>()?;
                    let result_revision = annotagent_image_tools::sha256(&serde_json::to_vec(
                        &json!({"input":input,"result":result}),
                    )?);
                    Ok(json!({
                        "image_id":input.image_id,"image_sha256":input.content_hash,
                        "result_revision":result_revision,"candidates":candidates
                    }))
                })
                .collect::<Result<Vec<_>>>()?;
            items.push(json!({
                "project_id":project,"conversation_id":conversation,"task_id":task,
                "project_schema_revision":task_record.input.schema_revision,
                "draft_id":sample.draft_id,"draft_revision":sample.draft_revision,
                "sample_test_id":sample.id,"sample_status":sample.status,
                "operation_status":operation.status,"result_available":true,"images":images
            }));
        }
        Ok(json!({
            "project_id":project,"conversation_id":conversation,"task_id":task,
            "items":items,"next_cursor":(end<operations.len()).then_some(end.to_string())
        }))
    }
}
