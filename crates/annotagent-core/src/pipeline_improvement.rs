//! Evidence-bounded comparison and approval contracts for improving an existing Pipeline.

use std::collections::{BTreeMap, BTreeSet};

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

use crate::{
    AnnotationFailureClass, LabelId, ObjectSizeBucket, PipelineDraftDiff, PipelineImprovementId,
    RunId, TaskId, WorkflowValidationReport,
};

pub const PIPELINE_IMPROVEMENT_SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PipelineImprovementStatus {
    DraftCreated,
    Compared,
    AwaitingHumanApproval,
    AppliedToDraft,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ImprovementEvidenceSufficiency {
    Insufficient,
    Provisional,
    Sufficient,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PipelineImprovementRecommendation {
    RecommendCandidate,
    DoNotRecommend,
    InsufficientEvidence,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct PipelineGeometrySizeMetrics {
    pub reference_count: u32,
    pub matched_count: u32,
    pub mean_iou: Option<f32>,
    pub median_iou: Option<f32>,
    pub p10_iou: Option<f32>,
    pub median_center_shift: Option<f32>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct PipelineGeometryMetrics {
    pub image_count: u32,
    pub reference_count: u32,
    pub prediction_count: u32,
    pub matched_count: u32,
    pub semantic_precision: Option<f32>,
    pub semantic_recall: Option<f32>,
    pub mean_iou: Option<f32>,
    pub median_iou: Option<f32>,
    pub p10_iou: Option<f32>,
    pub median_center_shift: Option<f32>,
    pub p90_center_shift: Option<f32>,
    pub manual_resize_rate: Option<f32>,
    pub too_loose_rate: Option<f32>,
    pub too_tight_rate: Option<f32>,
    pub no_candidate_rate: f32,
    pub review_rate: f32,
    pub cost_per_image: Decimal,
    pub latency_per_image_ms: u64,
    pub failure_count: u32,
    #[serde(default)]
    pub failure_classes: BTreeMap<AnnotationFailureClass, u32>,
    #[serde(default)]
    pub size_buckets: BTreeMap<ObjectSizeBucket, PipelineGeometrySizeMetrics>,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct PipelineImprovementPolicy {
    pub minimum_independent_references: u32,
    pub minimum_provisional_references: u32,
    pub maximum_recall_drop: f32,
    pub minimum_median_iou_gain: f32,
    pub maximum_review_rate_increase: f32,
    pub maximum_cost_per_image: Option<Decimal>,
    pub maximum_latency_per_image_ms: Option<u64>,
}

impl Default for PipelineImprovementPolicy {
    fn default() -> Self {
        Self {
            minimum_independent_references: 10,
            minimum_provisional_references: 5,
            maximum_recall_drop: 0.02,
            minimum_median_iou_gain: 0.02,
            maximum_review_rate_increase: 0.10,
            maximum_cost_per_image: None,
            maximum_latency_per_image_ms: None,
        }
    }
}

impl PipelineImprovementPolicy {
    pub fn validate(self) -> Result<(), String> {
        if self.minimum_independent_references == 0
            || self.minimum_provisional_references == 0
            || self.minimum_provisional_references > self.minimum_independent_references
            || !self.maximum_recall_drop.is_finite()
            || !(0.0..=1.0).contains(&self.maximum_recall_drop)
            || !self.minimum_median_iou_gain.is_finite()
            || !(0.0..=1.0).contains(&self.minimum_median_iou_gain)
            || !self.maximum_review_rate_increase.is_finite()
            || !(0.0..=1.0).contains(&self.maximum_review_rate_increase)
        {
            return Err("invalid Pipeline improvement policy".to_owned());
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PipelineGeometryComparison {
    pub baseline: PipelineGeometryMetrics,
    pub candidate: PipelineGeometryMetrics,
    pub evidence_sufficiency: ImprovementEvidenceSufficiency,
    pub independent_holdout: bool,
    pub recommendation: PipelineImprovementRecommendation,
    pub reasons: Vec<String>,
    pub regressions: Vec<String>,
}

#[must_use]
pub fn compare_pipeline_geometry_metrics(
    baseline: PipelineGeometryMetrics,
    candidate: PipelineGeometryMetrics,
    independent_holdout: bool,
    policy: PipelineImprovementPolicy,
) -> PipelineGeometryComparison {
    let policy_valid = policy.validate().is_ok();
    let reference_count = baseline
        .reference_count
        .min(candidate.reference_count)
        .min(baseline.image_count)
        .min(candidate.image_count);
    let evidence_sufficiency = if policy_valid
        && independent_holdout
        && reference_count >= policy.minimum_independent_references
    {
        ImprovementEvidenceSufficiency::Sufficient
    } else if policy_valid
        && independent_holdout
        && reference_count >= policy.minimum_provisional_references
    {
        ImprovementEvidenceSufficiency::Provisional
    } else {
        ImprovementEvidenceSufficiency::Insufficient
    };
    let mut reasons = Vec::new();
    let mut regressions = Vec::new();

    if !independent_holdout {
        reasons.push(
            "Diagnosis evidence and evaluation holdout are not independent; the same images cannot prove the proposed repair.".to_owned(),
        );
    }
    if reference_count < policy.minimum_independent_references {
        reasons.push(format!(
            "Only {reference_count} independent reference(s) are available; {} are required for a recommendation.",
            policy.minimum_independent_references
        ));
    }
    let recall_safe = metric_not_lower(
        baseline.semantic_recall,
        candidate.semantic_recall,
        policy.maximum_recall_drop,
    );
    if !recall_safe {
        regressions.push("semantic recall decreased beyond the allowed margin".to_owned());
    }
    let geometry_improved = metric_gain(
        baseline.median_iou,
        candidate.median_iou,
        policy.minimum_median_iou_gain,
    ) && metric_not_lower(baseline.p10_iou, candidate.p10_iou, 0.0);
    if !geometry_improved {
        regressions.push("independent median/P10 IoU did not demonstrate improvement".to_owned());
    }
    let manual_adjustment_safe = rate_not_higher(
        baseline.manual_resize_rate,
        candidate.manual_resize_rate,
        0.0,
    );
    if !manual_adjustment_safe {
        regressions.push("manual resize rate increased".to_owned());
    }
    if candidate.review_rate
        > baseline.review_rate + policy.maximum_review_rate_increase + f32::EPSILON
    {
        regressions.push("review rate increased beyond the Project allowance".to_owned());
    }
    if policy
        .maximum_cost_per_image
        .is_some_and(|maximum| candidate.cost_per_image > maximum)
    {
        regressions.push("cost per image exceeds the Project hard constraint".to_owned());
    }
    if policy
        .maximum_latency_per_image_ms
        .is_some_and(|maximum| candidate.latency_per_image_ms > maximum)
    {
        regressions.push("latency per image exceeds the Project hard constraint".to_owned());
    }
    if candidate.failure_count > baseline.failure_count {
        regressions.push("candidate introduced additional failed samples".to_owned());
    }
    for (class, count) in &candidate.failure_classes {
        if *count > baseline.failure_classes.get(class).copied().unwrap_or(0) {
            regressions.push(format!("candidate introduced more {class:?} failures"));
        }
    }
    regressions.sort();
    regressions.dedup();

    let recommendation = if evidence_sufficiency != ImprovementEvidenceSufficiency::Sufficient {
        PipelineImprovementRecommendation::InsufficientEvidence
    } else if regressions.is_empty() && recall_safe && geometry_improved && manual_adjustment_safe {
        reasons.push(
            "Independent holdout evidence satisfies recall, geometry, review, cost, latency and failure guards."
                .to_owned(),
        );
        PipelineImprovementRecommendation::RecommendCandidate
    } else {
        PipelineImprovementRecommendation::DoNotRecommend
    };

    PipelineGeometryComparison {
        baseline,
        candidate,
        evidence_sufficiency,
        independent_holdout,
        recommendation,
        reasons,
        regressions,
    }
}

fn metric_not_lower(baseline: Option<f32>, candidate: Option<f32>, margin: f32) -> bool {
    matches!((baseline, candidate), (Some(left), Some(right)) if right + margin >= left)
}

fn metric_gain(baseline: Option<f32>, candidate: Option<f32>, minimum_gain: f32) -> bool {
    matches!((baseline, candidate), (Some(left), Some(right)) if right >= left + minimum_gain)
}

fn rate_not_higher(baseline: Option<f32>, candidate: Option<f32>, margin: f32) -> bool {
    matches!((baseline, candidate), (Some(left), Some(right)) if right <= left + margin)
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PipelineImprovementDiagnosis {
    pub primary_failure_class: AnnotationFailureClass,
    pub evidence_run_ids: Vec<RunId>,
    pub evidence_statements: Vec<String>,
    pub semantic_target_correct_count: u32,
    pub geometry_correction_count: u32,
    pub provider_failure_count: u32,
    pub no_candidate_count: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PipelineImprovementSession {
    pub schema_version: u32,
    pub id: PipelineImprovementId,
    pub project_id: String,
    pub baseline_workflow_id: String,
    pub baseline_workflow_version: u32,
    pub target_task_id: TaskId,
    pub target_label: LabelId,
    pub diagnosis: PipelineImprovementDiagnosis,
    pub evaluation_run_ids: Vec<RunId>,
    pub baseline_draft_id: String,
    pub candidate_draft_id: String,
    pub diff: PipelineDraftDiff,
    pub validation: WorkflowValidationReport,
    pub comparison: Option<PipelineGeometryComparison>,
    pub status: PipelineImprovementStatus,
    pub setup_requirements: Vec<String>,
    pub applied_draft_id: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl PipelineImprovementSession {
    pub fn validate(&self) -> Result<(), String> {
        if self.schema_version != PIPELINE_IMPROVEMENT_SCHEMA_VERSION
            || self.project_id.trim().is_empty()
            || self.baseline_workflow_id.trim().is_empty()
            || self.baseline_workflow_version == 0
            || self.baseline_draft_id.trim().is_empty()
            || self.candidate_draft_id.trim().is_empty()
            || self.diagnosis.evidence_run_ids.is_empty()
        {
            return Err("invalid Pipeline improvement session".to_owned());
        }
        if self
            .diagnosis
            .evidence_run_ids
            .iter()
            .any(|run_id| self.evaluation_run_ids.contains(run_id))
        {
            return Err(
                "diagnosis Evidence Runs and evaluation holdout Runs must be disjoint".to_owned(),
            );
        }
        if self.diagnosis.evidence_run_ids.len() > 100
            || self.evaluation_run_ids.len() > 100
            || self
                .diagnosis
                .evidence_run_ids
                .iter()
                .copied()
                .collect::<BTreeSet<_>>()
                .len()
                != self.diagnosis.evidence_run_ids.len()
            || self
                .evaluation_run_ids
                .iter()
                .copied()
                .collect::<BTreeSet<_>>()
                .len()
                != self.evaluation_run_ids.len()
        {
            return Err(
                "Pipeline improvement Run selections must be bounded and unique".to_owned(),
            );
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn metrics(references: u32, median_iou: f32, p10_iou: f32) -> PipelineGeometryMetrics {
        PipelineGeometryMetrics {
            image_count: references,
            reference_count: references,
            prediction_count: references,
            matched_count: references,
            semantic_precision: Some(1.0),
            semantic_recall: Some(1.0),
            mean_iou: Some(median_iou),
            median_iou: Some(median_iou),
            p10_iou: Some(p10_iou),
            median_center_shift: Some(0.02),
            p90_center_shift: Some(0.04),
            manual_resize_rate: Some(0.1),
            ..PipelineGeometryMetrics::default()
        }
    }

    #[test]
    fn independent_improvement_can_be_recommended() {
        let comparison = compare_pipeline_geometry_metrics(
            metrics(12, 0.55, 0.30),
            metrics(12, 0.75, 0.50),
            true,
            PipelineImprovementPolicy::default(),
        );
        assert_eq!(
            comparison.recommendation,
            PipelineImprovementRecommendation::RecommendCandidate
        );
        assert_eq!(
            comparison.evidence_sufficiency,
            ImprovementEvidenceSufficiency::Sufficient
        );
    }

    #[test]
    fn four_images_cannot_recommend_even_with_many_objects() {
        let mut baseline = metrics(20, 0.30, 0.10);
        baseline.image_count = 4;
        let mut candidate = metrics(20, 0.95, 0.90);
        candidate.image_count = 4;
        let comparison = compare_pipeline_geometry_metrics(
            baseline,
            candidate,
            true,
            PipelineImprovementPolicy::default(),
        );
        assert_eq!(
            comparison.recommendation,
            PipelineImprovementRecommendation::InsufficientEvidence
        );
        assert_eq!(
            comparison.evidence_sufficiency,
            ImprovementEvidenceSufficiency::Insufficient
        );
    }

    #[test]
    fn five_independent_images_are_provisional_but_not_recommended() {
        let comparison = compare_pipeline_geometry_metrics(
            metrics(5, 0.30, 0.10),
            metrics(5, 0.95, 0.90),
            true,
            PipelineImprovementPolicy::default(),
        );
        assert_eq!(
            comparison.evidence_sufficiency,
            ImprovementEvidenceSufficiency::Provisional
        );
        assert_eq!(
            comparison.recommendation,
            PipelineImprovementRecommendation::InsufficientEvidence
        );
    }

    #[test]
    fn geometry_gain_cannot_hide_recall_or_cost_regression() {
        let baseline = metrics(12, 0.40, 0.20);
        let mut candidate = metrics(12, 0.80, 0.60);
        candidate.semantic_recall = Some(0.80);
        candidate.cost_per_image = Decimal::new(25, 3);
        let comparison = compare_pipeline_geometry_metrics(
            baseline,
            candidate,
            true,
            PipelineImprovementPolicy {
                maximum_cost_per_image: Some(Decimal::new(10, 3)),
                ..PipelineImprovementPolicy::default()
            },
        );
        assert_eq!(
            comparison.recommendation,
            PipelineImprovementRecommendation::DoNotRecommend
        );
        assert!(comparison.regressions.len() >= 2);
    }
}
