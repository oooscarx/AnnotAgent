//! Bounded, terminal-only observations for the existing sample-plan repair Builder.
use annotagent_storage::{SampleFeedbackRevision, WorkflowSampleTest};
use anyhow::{Result, bail};
use serde_json::{Value, json};
use std::collections::BTreeSet;

pub(crate) fn observations(test: &WorkflowSampleTest, evidence: &Value) -> Result<Vec<Value>> {
    let feedback: Vec<SampleFeedbackRevision> =
        serde_json::from_value(evidence["feedback"].clone())?;
    if test.inputs.len() != test.report.samples.len() {
        bail!("Repair evidence cannot pair saved image inputs with sample results");
    }
    if feedback.iter().any(|item| {
        item.sample_test_id != test.id
            || !test
                .inputs
                .iter()
                .any(|image| image.image_id == item.image_id)
    }) {
        bail!("Repair feedback refers to a different Sample Test or image");
    }
    let mut observations = Vec::new();
    for (image, sample) in test.inputs.iter().zip(&test.report.samples) {
        let relevant = feedback
            .iter()
            .filter(|item| item.image_id == image.image_id)
            .collect::<Vec<_>>();
        if relevant.is_empty() {
            continue;
        }
        let ids = relevant
            .iter()
            .filter_map(|item| item.outcome_id.as_deref())
            .collect::<BTreeSet<_>>();
        let whole_image = relevant.iter().any(|item| item.outcome_id.is_none());
        let mut seen = BTreeSet::new();
        let terminal = sample
            .projection
            .final_candidates
            .iter()
            .chain(
                sample
                    .projection
                    .review_candidates
                    .iter()
                    .map(|item| &item.candidate),
            )
            .filter(|item| seen.insert(item.outcome.id.as_str()))
            .collect::<Vec<_>>();
        let scoped = terminal
            .iter()
            .filter(|item| whole_image || ids.contains(item.outcome.id.as_str()))
            .collect::<Vec<_>>();
        let outcomes = scoped
            .iter()
            .take(64)
            .map(|candidate| {
                let outcome = &candidate.outcome;
                let value = outcome.value.as_ref().filter(|value| {
                    matches!(
                        value,
                        annotagent_core::VisionArtifactValue::BoundingBox { .. }
                            | annotagent_core::VisionArtifactValue::Classification { .. }
                    )
                });
                json!({
                    "id": outcome.id,
                    "label": outcome.label,
                    "status": outcome.status,
                    "semantic_confidence": outcome.confidence,
                    "failure_classes": outcome.failure_classes,
                    "value": value,
                    "geometry_value_omitted": outcome.value.is_some() && value.is_none(),
                    "localization": candidate.localization,
                    "geometry": candidate.geometry,
                    "final_status": candidate.final_status,
                    "source_artifact_ref": candidate.source_artifact_ref,
                    "lineage_id": candidate.lineage_id,
                })
            })
            .collect::<Vec<_>>();
        observations.push(json!({
            "image_id":image.image_id,"content_hash":image.content_hash,"failed":sample.failed,
            "evidence_scope":if whole_image {"feedback_image_terminal_results"} else {"feedback_subjects_only"},
            "terminal_candidate_count":terminal.len(),"scoped_candidate_count":scoped.len(),
            "outcomes_truncated":scoped.len()>64,"no_target":sample.projection.no_target,
            "requested_subjects_not_in_terminal_projection":ids.iter().filter(|id|!terminal.iter().any(|item|item.outcome.id==**id)).collect::<Vec<_>>(),
            "pixels_supplied":false,
            "outcomes": outcomes,
        }));
        if observations.len() == 10 {
            break;
        }
    }
    Ok(observations)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn repair_observations_exclude_intermediate_and_unrelated_outputs() {
        let now = chrono::Utc::now();
        let outcome = json!({"id":"final","label":"cup","confidence":0.8,"status":"needs_review","value":{"kind":"bounding_box","rect":[0.1,0.1,0.2,0.2]}});
        let candidate = json!({"source_artifact_id":uuid::Uuid::new_v4(),"source_artifact_ref":"terminal:1","lineage_id":"lineage","outcome":outcome,"localization":"test","geometry":"uncertain","final_status":"review"});
        let mut other = candidate.clone();
        other["outcome"]["id"] = json!("unrelated-final");
        let mut intermediate = outcome.clone();
        intermediate["id"] = json!("coarse");
        let sample = json!({"image_index":0,"image_name":"TEST.png","width":100,"height":100,"nodes":[],"outcomes":[intermediate,outcome],"projection":{"final_candidates":[candidate,other],"review_candidates":[],"committed_annotations":[],"no_target":false,"intermediate_artifact_ids":[]}});
        let test:WorkflowSampleTest=serde_json::from_value(json!({"id":"test","draft_id":"draft","project_id":"TEST","draft_revision":1,"request_revision":1,"draft_content_hash":"hash","image_set_hash":"images","model_snapshot_hash":"models","status":"passed","inputs":[{"image_id":"image","content_hash":"pixels"},{"image_id":"unrelated-image","content_hash":"other"}],"model_bindings":{},"started_at":now,"completed_at":now,"report":{"sandbox":true,"validation":{"valid":true,"issues":[],"execution_order":[]},"samples":[sample,sample],"total_latency_ms":0,"estimated_cost":"unknown"}})).unwrap();
        let mut evidence = json!({"feedback":[{"revision_id":"answer","sample_test_id":"test","image_id":"image","sequence":1,"reason":"poor_boundary","outcome_id":"final","corrected_value":null,"note":"TEST","created_at":now}]});
        let values = observations(&test, &evidence).unwrap();
        assert_eq!(values.len(), 1);
        assert_eq!(values[0]["outcomes"].as_array().unwrap().len(), 1);
        assert_eq!(values[0]["outcomes"][0]["id"], "final");
        assert_eq!(values[0]["terminal_candidate_count"], 2);
        assert_eq!(values[0]["pixels_supplied"], false);
        assert_eq!(values[0]["outcomes"][0]["value"]["kind"], "bounding_box");
        evidence["feedback"][0]["outcome_id"] = json!("coarse");
        let absent = observations(&test, &evidence).unwrap();
        assert!(absent[0]["outcomes"].as_array().unwrap().is_empty());
        assert_eq!(
            absent[0]["requested_subjects_not_in_terminal_projection"],
            json!(["coarse"])
        );
        evidence["feedback"][0]["outcome_id"] = json!("final");
        let mut classification = test.clone();
        classification.report.samples[0].projection.final_candidates[0]
            .outcome
            .value = Some(
            serde_json::from_value(json!({"kind":"classification","labels":["indoor"]})).unwrap(),
        );
        let repeated = classification.report.samples[0].projection.final_candidates[0].clone();
        classification.report.samples[0]
            .projection
            .final_candidates
            .push(repeated);
        let classified = observations(&classification, &evidence).unwrap();
        assert_eq!(
            classified[0]["outcomes"][0]["value"]["labels"],
            json!(["indoor"])
        );
        assert_eq!(classified[0]["scoped_candidate_count"], 1);
        evidence["feedback"][0]["outcome_id"] = Value::Null;
        assert_eq!(
            observations(&classification, &evidence).unwrap()[0]["scoped_candidate_count"],
            2
        );
        evidence["feedback"][0]["sample_test_id"] = json!("foreign");
        assert!(observations(&test, &evidence).is_err());
        evidence["feedback"][0]["sample_test_id"] = json!("test");
        let mut incomplete = test.clone();
        incomplete.report.samples.pop();
        assert!(observations(&incomplete, &evidence).is_err());

        let mut bounded = test.clone();
        let exemplar = bounded.report.samples[0].projection.final_candidates[0].clone();
        bounded.report.samples[0].projection.final_candidates = (0..70)
            .map(|index| {
                let mut candidate = exemplar.clone();
                candidate.outcome.id = format!("terminal-{index}");
                candidate
            })
            .collect();
        let limited = observations(&bounded, &evidence).unwrap();
        assert_eq!(limited[0]["outcomes"].as_array().unwrap().len(), 64);
        assert_eq!(limited[0]["scoped_candidate_count"], 70);
        assert_eq!(limited[0]["outcomes_truncated"], true);
        bounded.report.samples[0]
            .projection
            .final_candidates
            .clear();
        bounded.report.samples[0].projection.no_target = true;
        let empty = observations(&bounded, &evidence).unwrap();
        assert_eq!(empty[0]["no_target"], true);
        assert!(empty[0]["outcomes"].as_array().unwrap().is_empty());
    }
}
