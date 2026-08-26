use annotagent_core::{
    AnnotationValue, AttributeValue, CoreResult, IssueSeverity, NormalizedRect, SuggestedAction,
    ValidationContext, ValidationEvidence, ValidationIssue,
};
use annotagent_image_tools::color_statistics;

use crate::field::measurements;

pub struct RobotAttributeValidator;

impl annotagent_core::AnnotationValidator for RobotAttributeValidator {
    fn id(&self) -> &str {
        "robot_attribute_rules"
    }

    fn validate(&self, context: &ValidationContext<'_>) -> CoreResult<Vec<ValidationIssue>> {
        if context
            .candidate
            .label
            .as_ref()
            .is_none_or(|label| label.as_str() != "robot")
        {
            return Ok(Vec::new());
        }
        let mut issues = Vec::new();
        let team = string_attribute(context, "team_color");
        if !matches!(team, Some("red" | "blue" | "unknown")) {
            issues.push(attribute_issue(
                context,
                "missing_team_color",
                "team_color must be red, blue, or unknown",
            ));
        }
        let state = string_attribute(context, "state");
        if !matches!(
            state,
            Some("standing" | "fallen" | "goalkeeper" | "unavailable" | "unknown")
        ) {
            issues.push(attribute_issue(
                context,
                "missing_robot_state",
                "state is required and must use an allowed RoboCup value",
            ));
        }
        if let (
            Some(image),
            AnnotationValue::BoundingBox { rect },
            Some(predicted @ ("red" | "blue")),
        ) = (context.image, &context.candidate.value, team)
        {
            let torso = torso_rect(*rect)?;
            let statistics = color_statistics(image, torso)?;
            let evidence = if statistics.red_ratio > statistics.blue_ratio * 1.5
                && statistics.red_ratio > 0.08
            {
                Some("red")
            } else if statistics.blue_ratio > statistics.red_ratio * 1.5
                && statistics.blue_ratio > 0.08
            {
                Some("blue")
            } else {
                None
            };
            if evidence.is_some_and(|evidence| evidence != predicted) {
                issues.push(ValidationIssue {
                    code: "team_color_conflict".to_owned(),
                    severity: IssueSeverity::Warning,
                    annotation_ids: vec![context.candidate.id],
                    message: format!(
                        "model predicted {predicted}, deterministic torso evidence indicates {}",
                        evidence.unwrap_or("unknown")
                    ),
                    suggested_action: SuggestedAction::HumanReview,
                    evidence: ValidationEvidence::ImageStatistics {
                        region: "robot_torso".to_owned(),
                        measurements: measurements(&[
                            ("red_ratio", f64::from(statistics.red_ratio)),
                            ("blue_ratio", f64::from(statistics.blue_ratio)),
                        ]),
                    },
                });
            }
            if state == Some("standing") && rect.width() > rect.height() * 1.4 {
                issues.push(ValidationIssue {
                    code: "robot_state_geometry_warning".to_owned(),
                    severity: IssueSeverity::Info,
                    annotation_ids: vec![context.candidate.id],
                    message:
                        "wide geometry is weak evidence against standing; VLM decision needs review"
                            .to_owned(),
                    suggested_action: SuggestedAction::HumanReview,
                    evidence: ValidationEvidence::Geometry {
                        metric: "width_height_ratio".to_owned(),
                        value: f64::from(rect.width() / rect.height()),
                        threshold: 1.4,
                    },
                });
            }
        }
        Ok(issues)
    }
}

pub(crate) fn torso_rect(rect: NormalizedRect) -> CoreResult<NormalizedRect> {
    NormalizedRect::new(
        rect.x() + rect.width() * 0.15,
        rect.y() + rect.height() * 0.1,
        rect.width() * 0.7,
        rect.height() * 0.5,
    )
}

fn string_attribute<'a>(context: &'a ValidationContext<'_>, name: &str) -> Option<&'a str> {
    match context.candidate.attributes.get(name) {
        Some(AttributeValue::String(value)) => Some(value),
        _ => None,
    }
}

fn attribute_issue(context: &ValidationContext<'_>, code: &str, message: &str) -> ValidationIssue {
    ValidationIssue {
        code: code.to_owned(),
        severity: IssueSeverity::Error,
        annotation_ids: vec![context.candidate.id],
        message: message.to_owned(),
        suggested_action: SuggestedAction::Retry,
        evidence: ValidationEvidence::Rule {
            facts: [("required".to_owned(), "true".to_owned())]
                .into_iter()
                .collect(),
        },
    }
}
