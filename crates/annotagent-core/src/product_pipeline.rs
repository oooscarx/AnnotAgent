//! Lean public vocabulary projected over the existing typed Workflow node model.

use serde::{Deserialize, Serialize};

use crate::{CoreError, CoreResult};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DecisionMode {
    Confidence,
    Evidence,
    DomainPolicy,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GuidedPipelineConcept {
    ImageInput,
    FindObjects,
    SelectDetections,
    Crop,
    Classify,
    Validate,
    CombineModelEvidence,
    Decision,
    HumanReview,
    Commit,
    Other,
}

impl GuidedPipelineConcept {
    #[must_use]
    pub const fn display_name(self) -> &'static str {
        match self {
            Self::ImageInput => "Read each image",
            Self::FindObjects => "Find objects",
            Self::SelectDetections => "Select detections",
            Self::Crop => "Crop candidates",
            Self::Classify => "Classify crops or images",
            Self::Validate => "Check the result",
            Self::CombineModelEvidence => "Combine model evidence",
            Self::Decision => "Decision",
            Self::HumanReview => "Send uncertain results to Review",
            Self::Commit => "Save annotation",
            Self::Other => "Processing step",
        }
    }
}

#[must_use]
pub fn guided_concept_for_node_type(node_type: &str) -> GuidedPipelineConcept {
    match node_type {
        "core.image_input" => GuidedPipelineConcept::ImageInput,
        "core.filter"
        | "core.map_label"
        | "core.project_detection_candidates"
        | "core.select_and_map" => GuidedPipelineConcept::SelectDetections,
        "core.crop" => GuidedPipelineConcept::Crop,
        "core.match_detection_sets"
        | "core.candidate_merge"
        | "core.combine_evidence"
        | "core.attach_result" => GuidedPipelineConcept::CombineModelEvidence,
        "core.confidence_gate" | "core.evidence_gate" | "core.decision" => {
            GuidedPipelineConcept::Decision
        }
        "core.human_review" | "review_gate" => GuidedPipelineConcept::HumanReview,
        "core.commit" | "commit" => GuidedPipelineConcept::Commit,
        value if value.contains("detect") || value.contains("ground") => {
            GuidedPipelineConcept::FindObjects
        }
        value if value.contains("classif") => GuidedPipelineConcept::Classify,
        value if value.contains("valid") => GuidedPipelineConcept::Validate,
        _ => GuidedPipelineConcept::Other,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GroundingAssistMode {
    Grid,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GroundingAssistConfig {
    pub mode: GroundingAssistMode,
    pub enabled: bool,
    pub rows: u32,
    pub columns: u32,
}

impl Default for GroundingAssistConfig {
    fn default() -> Self {
        Self {
            mode: GroundingAssistMode::Grid,
            enabled: false,
            rows: 10,
            columns: 10,
        }
    }
}

impl GroundingAssistConfig {
    pub fn validate(&self) -> CoreResult<()> {
        if !(2..=16).contains(&self.rows) || !(2..=16).contains(&self.columns) {
            return Err(CoreError::Validation(
                "grounding_assist grid rows and columns must each be within [2,16]".to_owned(),
            ));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lean_concepts_merge_internal_nodes_without_changing_node_ids() {
        assert_eq!(
            guided_concept_for_node_type("core.filter"),
            GuidedPipelineConcept::SelectDetections
        );
        assert_eq!(
            guided_concept_for_node_type("core.map_label"),
            GuidedPipelineConcept::SelectDetections
        );
        assert_eq!(
            guided_concept_for_node_type("core.confidence_gate"),
            GuidedPipelineConcept::Decision
        );
        assert_eq!(
            guided_concept_for_node_type("core.evidence_gate"),
            GuidedPipelineConcept::Decision
        );
        assert_eq!(
            guided_concept_for_node_type("core.match_detection_sets"),
            GuidedPipelineConcept::CombineModelEvidence
        );
    }

    #[test]
    fn grounding_assist_is_bounded_configuration() {
        GroundingAssistConfig::default()
            .validate()
            .expect("default");
        assert!(
            GroundingAssistConfig {
                rows: 1,
                ..GroundingAssistConfig::default()
            }
            .validate()
            .is_err()
        );
    }
}
