use annotagent_core::{IssueSeverity, ReviewContext, ReviewDecision, ReviewPolicy};

pub struct RoboCupReviewPolicy;

impl ReviewPolicy for RoboCupReviewPolicy {
    fn decide(&self, context: &ReviewContext<'_>) -> ReviewDecision {
        let codes: Vec<String> = context
            .issues
            .iter()
            .map(|issue| issue.code.clone())
            .collect();
        let retryable = context.issues.iter().any(|issue| {
            issue.severity == IssueSeverity::Error
                || matches!(
                    issue.code.as_str(),
                    "possible_white_shoe"
                        | "possible_penalty_mark"
                        | "possible_field_line_intersection"
                        | "weak_pixel_support"
                )
        });
        if retryable && context.retry_count < context.max_retries {
            return ReviewDecision::Retry {
                reasons: if codes.is_empty() {
                    vec!["deterministic evidence requires another model/tool step".to_owned()]
                } else {
                    codes
                },
            };
        }
        if context.evidence_conflict
            || context.correction_risk >= 0.2
            || !context.issues.is_empty()
            || context.annotation.confidence.unwrap_or(0.0) < 0.92
        {
            return ReviewDecision::HumanReview {
                reasons: if codes.is_empty() {
                    vec!["confidence or correction-memory risk needs review".to_owned()]
                } else {
                    codes
                },
            };
        }
        ReviewDecision::AutoAccept {
            reasons: vec![
                "high model confidence and all deterministic RoboCup checks passed".to_owned(),
            ],
        }
    }
}
