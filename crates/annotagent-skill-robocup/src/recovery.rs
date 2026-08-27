use std::{path::PathBuf, sync::Arc};

use annotagent_core::{
    AgentBudget, AgentKind, AgentSession, AgentTool, Annotation, AnnotationValue, CorrectionRecord,
    ImageFrame, ProjectId, ToolContext, ValidationIssue,
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use tokio_util::sync::CancellationToken;

use crate::BallEvidenceTool;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RecoveryDisposition {
    Accept,
    Reject,
    HumanReview,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RoboCupBallRecoveryReport {
    pub fast_path: bool,
    pub disposition: RecoveryDisposition,
    pub reasons: Vec<String>,
    pub memory_matches: usize,
    pub memory_changed_decision: bool,
    pub session: Option<AgentSession>,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct RoboCupBallRecoveryPolicy;

impl RoboCupBallRecoveryPolicy {
    #[must_use]
    pub fn decide(
        &self,
        issues: &[ValidationIssue],
        memory: &[CorrectionRecord],
        white_ratio: Option<f64>,
    ) -> (RecoveryDisposition, Vec<String>, bool) {
        let codes = issues
            .iter()
            .map(|issue| issue.code.as_str())
            .collect::<Vec<_>>();
        let memory_reject = memory.iter().any(|record| {
            matches!(
                record.reason_code.as_str(),
                "white_shoe_as_ball"
                    | "white_sock_as_ball"
                    | "penalty_mark_as_ball"
                    | "line_intersection_as_ball"
                    | "duplicate_ball"
            )
        });
        if memory_reject {
            return (
                RecoveryDisposition::Reject,
                vec![
                    "matching project correction memory identifies a known hard negative"
                        .to_owned(),
                ],
                true,
            );
        }
        if codes.contains(&"possible_penalty_mark")
            || codes.contains(&"missing_field_evidence")
            || codes.contains(&"ball_outside_field")
        {
            return (
                RecoveryDisposition::HumanReview,
                codes.into_iter().map(ToOwned::to_owned).collect(),
                false,
            );
        }
        if (codes.contains(&"possible_white_shoe") || codes.contains(&"possible_white_sock"))
            && white_ratio.is_some_and(|ratio| ratio >= 0.25)
        {
            return (
                RecoveryDisposition::Reject,
                vec!["bounded crop statistics support a white-footwear hard negative".to_owned()],
                false,
            );
        }
        if issues.is_empty() {
            (
                RecoveryDisposition::Accept,
                vec!["no deterministic or learned risk remains".to_owned()],
                false,
            )
        } else {
            (
                RecoveryDisposition::HumanReview,
                codes.into_iter().map(ToOwned::to_owned).collect(),
                false,
            )
        }
    }
}

pub struct RoboCupBallRecoveryRequest {
    pub project_id: ProjectId,
    pub project_root: PathBuf,
    pub candidate: Annotation,
    pub related_annotations: Vec<Annotation>,
    pub issues: Vec<ValidationIssue>,
    pub correction_memory: Vec<CorrectionRecord>,
    pub image: Option<Arc<ImageFrame>>,
    pub budget: AgentBudget,
    pub cancellation: CancellationToken,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct RoboCupBallRecoveryAgent;

impl RoboCupBallRecoveryAgent {
    pub async fn run(
        &self,
        request: RoboCupBallRecoveryRequest,
    ) -> Result<RoboCupBallRecoveryReport, String> {
        if request.issues.is_empty() && request.correction_memory.is_empty() {
            return Ok(RoboCupBallRecoveryReport {
                fast_path: true,
                disposition: RecoveryDisposition::Accept,
                reasons: vec!["normal candidate bypassed Recovery Agent".to_owned()],
                memory_matches: 0,
                memory_changed_decision: false,
                session: None,
            });
        }
        let mut session = AgentSession::start(AgentKind::AnnotationRecovery, request.budget)
            .with_project(request.project_id.to_string());
        if request.cancellation.is_cancelled() {
            session.cancel();
            return Ok(RoboCupBallRecoveryReport {
                fast_path: false,
                disposition: RecoveryDisposition::HumanReview,
                reasons: vec!["Recovery Agent was cancelled".to_owned()],
                memory_matches: request.correction_memory.len(),
                memory_changed_decision: false,
                session: Some(session),
            });
        }
        if record(
            &mut session,
            "load_domain_resource",
            json!({"skill_id": "robocup.ball", "resource": "ball/resources/hard-negatives.md"}),
            json!({"loaded": true, "bytes": include_str!("../../../skills/robocup/ball/resources/hard-negatives.md").len()}),
        )
        .is_err()
        {
            return Ok(stopped_report(session, request.correction_memory.len()));
        }
        if record(
            &mut session,
            "inspect_candidate",
            json!({"annotation_id": request.candidate.id}),
            json!({
                "candidate": request.candidate,
                "parent_count": request.related_annotations.len(),
                "issues": request.issues,
            }),
        )
        .is_err()
        {
            return Ok(stopped_report(session, request.correction_memory.len()));
        }
        if record(
            &mut session,
            "query_correction_memory",
            json!({
                "project_id": request.project_id,
                "skill_id": "robocup.ball",
                "task_id": request.candidate.task_id,
                "label": request.candidate.label,
            }),
            json!({
                "matches": request.correction_memory.iter().map(|record| json!({
                    "reason_code": record.reason_code,
                    "created_at": record.created_at,
                })).collect::<Vec<_>>()
            }),
        )
        .is_err()
        {
            return Ok(stopped_report(session, request.correction_memory.len()));
        }

        let mut white_ratio = None;
        if let (Some(image), AnnotationValue::BoundingBox { rect }) =
            (request.image.clone(), &request.candidate.value)
        {
            let tool = BallEvidenceTool;
            let result = tool
                .execute(
                    &ToolContext {
                        project_root: request.project_root,
                        run_id: annotagent_core::RunId::new(),
                        image_id: Some(request.candidate.image_id),
                        image: Some(image),
                        task_id: Some(request.candidate.task_id.clone()),
                        cancellation: request.cancellation.clone(),
                    },
                    json!({"bbox": rect}),
                )
                .await
                .map_err(|error| error.to_string())?;
            white_ratio = result
                .model_result
                .get("white_ratio")
                .and_then(serde_json::Value::as_f64);
            if record(
                &mut session,
                "evaluate_ball_hard_negative",
                json!({"annotation_id": request.candidate.id}),
                result.model_result,
            )
            .is_err()
            {
                return Ok(stopped_report(session, request.correction_memory.len()));
            }
        }
        if record(
            &mut session,
            "compare_candidates",
            json!({"annotation_id": request.candidate.id}),
            json!({
                "issue_count": request.issues.len(),
                "memory_matches": request.correction_memory.len(),
                "white_ratio": white_ratio,
            }),
        )
        .is_err()
        {
            return Ok(stopped_report(session, request.correction_memory.len()));
        }
        let (disposition, reasons, memory_changed_decision) = RoboCupBallRecoveryPolicy.decide(
            &request.issues,
            &request.correction_memory,
            white_ratio,
        );
        if record(
            &mut session,
            "decide_recovery",
            json!({"allowed": ["accept", "reject", "human_review"]}),
            json!({"disposition": disposition, "reasons": reasons}),
        )
        .is_err()
        {
            return Ok(stopped_report(session, request.correction_memory.len()));
        }
        match disposition {
            RecoveryDisposition::HumanReview => session.wait_for_human("review_annotation"),
            RecoveryDisposition::Accept | RecoveryDisposition::Reject => {
                session.succeed("bounded recovery decision completed");
            }
        }
        Ok(RoboCupBallRecoveryReport {
            fast_path: false,
            disposition,
            reasons,
            memory_matches: request.correction_memory.len(),
            memory_changed_decision,
            session: Some(session),
        })
    }
}

fn record(
    session: &mut AgentSession,
    tool: &str,
    arguments: serde_json::Value,
    result: serde_json::Value,
) -> Result<(), String> {
    session.record_tool(tool, arguments, result, true)
}

fn stopped_report(session: AgentSession, memory_matches: usize) -> RoboCupBallRecoveryReport {
    RoboCupBallRecoveryReport {
        fast_path: false,
        disposition: RecoveryDisposition::HumanReview,
        reasons: vec![
            session
                .stop_reason
                .clone()
                .unwrap_or_else(|| "Recovery Agent stopped before a safe decision".to_owned()),
        ],
        memory_matches,
        memory_changed_decision: false,
        session: Some(session),
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use annotagent_core::{
        AgentSessionStatus, AnnotationId, AnnotationProvenance, AnnotationSource,
        CorrectionFeatures, IssueSeverity, LabelId, NormalizedRect, ReviewStatus, SuggestedAction,
        TaskId,
    };
    use annotagent_image_tools::{generate_synthetic_robocup, load_image};
    use chrono::Utc;
    use uuid::Uuid;

    use super::*;

    fn candidate() -> Annotation {
        Annotation {
            id: AnnotationId::new(),
            image_id: annotagent_core::ImageId::new(),
            task_id: TaskId::from("objects"),
            label: Some(LabelId::from("ball")),
            value: AnnotationValue::BoundingBox {
                rect: NormalizedRect::new(0.218, 0.615, 0.04, 0.03).expect("rect"),
            },
            attributes: BTreeMap::new(),
            confidence: Some(0.94),
            source: AnnotationSource::Model,
            review_status: ReviewStatus::Draft,
            provenance: AnnotationProvenance::default(),
            created_at: Utc::now(),
        }
    }

    fn issue(code: &str) -> ValidationIssue {
        ValidationIssue {
            code: code.to_owned(),
            severity: IssueSeverity::Warning,
            annotation_ids: Vec::new(),
            message: code.to_owned(),
            suggested_action: SuggestedAction::HumanReview,
            evidence: annotagent_core::ValidationEvidence::Rule {
                facts: BTreeMap::from([("fixture".to_owned(), code.to_owned())]),
            },
        }
    }

    fn memory(project_id: ProjectId, candidate: &Annotation, reason: &str) -> CorrectionRecord {
        CorrectionRecord {
            id: Uuid::new_v4(),
            project_id,
            skill_id: "robocup.ball".to_owned(),
            task_id: candidate.task_id.clone(),
            predicted_label: candidate.label.clone(),
            corrected_label: None,
            reason_code: reason.to_owned(),
            original_annotation: None,
            corrected_annotation: None,
            note: None,
            image_features: CorrectionFeatures {
                geometry: BTreeMap::new(),
                colors: BTreeMap::new(),
            },
            created_at: Utc::now(),
        }
    }

    #[tokio::test]
    async fn fast_path_recovery_and_memory_changed_second_decision_are_distinct() {
        let temporary = tempfile::tempdir().expect("temp");
        let image_path = temporary.path().join("fixture.png");
        generate_synthetic_robocup(&image_path).expect("image");
        let image = Arc::new(load_image(&image_path, 1_000_000).expect("load"));
        let project_id = ProjectId::new();
        let candidate = candidate();
        let request = |issues, correction_memory| RoboCupBallRecoveryRequest {
            project_id,
            project_root: temporary.path().to_path_buf(),
            candidate: candidate.clone(),
            related_annotations: Vec::new(),
            issues,
            correction_memory,
            image: Some(image.clone()),
            budget: AgentBudget::default(),
            cancellation: CancellationToken::new(),
        };
        let fast = RoboCupBallRecoveryAgent
            .run(request(Vec::new(), Vec::new()))
            .await
            .expect("fast path");
        assert!(fast.fast_path && fast.session.is_none());

        let penalty = RoboCupBallRecoveryAgent
            .run(request(vec![issue("possible_penalty_mark")], Vec::new()))
            .await
            .expect("penalty review");
        assert_eq!(penalty.disposition, RecoveryDisposition::HumanReview);
        assert_eq!(
            penalty.session.as_ref().map(|session| session.status),
            Some(AgentSessionStatus::WaitingForHuman)
        );

        let first = RoboCupBallRecoveryAgent
            .run(request(vec![issue("inaccurate_ball_bbox")], Vec::new()))
            .await
            .expect("first decision");
        assert_eq!(first.disposition, RecoveryDisposition::HumanReview);
        let second = RoboCupBallRecoveryAgent
            .run(request(
                vec![issue("inaccurate_ball_bbox")],
                vec![memory(project_id, &candidate, "white_shoe_as_ball")],
            ))
            .await
            .expect("memory decision");
        assert_eq!(second.disposition, RecoveryDisposition::Reject);
        assert!(second.memory_changed_decision);
        assert!(second.session.as_ref().is_some_and(|session| {
            session
                .steps
                .iter()
                .any(|step| step.tool_name == "query_correction_memory")
        }));

        let bounded = RoboCupBallRecoveryAgent
            .run(RoboCupBallRecoveryRequest {
                budget: AgentBudget {
                    max_steps: 0,
                    max_tool_calls: 0,
                    ..AgentBudget::default()
                },
                ..request(vec![issue("possible_penalty_mark")], Vec::new())
            })
            .await
            .expect("bounded recovery");
        assert_eq!(bounded.disposition, RecoveryDisposition::HumanReview);
        assert_eq!(
            bounded.session.as_ref().map(|session| session.status),
            Some(AgentSessionStatus::BudgetExceeded)
        );
    }
}
