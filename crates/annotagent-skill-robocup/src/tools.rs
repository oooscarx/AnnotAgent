use annotagent_core::{
    AgentTool, ArtifactId, ArtifactProvenance, ArtifactRole, ArtifactValidationState, CoreError,
    CoreResult, LabelId, NormalizedPoint, NormalizedRect, ToolContext, ToolDefinition, ToolResult,
    VisionArtifact, VisionArtifactValue,
};
use annotagent_image_tools::color_statistics;
use async_trait::async_trait;
use serde::Deserialize;
use serde_json::{Value, json};

use crate::{field::refine_points, robot::torso_rect};

pub struct RoboCupFieldLineTool;

#[async_trait]
impl AgentTool for RoboCupFieldLineTool {
    fn applicable_tasks(&self) -> Vec<annotagent_core::TaskId> {
        vec![annotagent_core::TaskId::from("field_line")]
    }

    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "refine_robocup_field_line".to_owned(),
            description: "Snap a coarse normalized polyline to nearby white field-line pixels"
                .to_owned(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "points": {
                        "type": "array",
                        "description": "Coarse polyline of normalized [x,y] pairs; never pixel coordinates",
                        "minItems": 2,
                        "items": {"type": "array", "minItems": 2, "maxItems": 2, "items": {"type": "number", "minimum": 0, "maximum": 1}}
                    }
                },
                "required": ["points"],
                "additionalProperties": false
            }),
            read_only: true,
        }
    }

    async fn execute(&self, context: &ToolContext, arguments: Value) -> CoreResult<ToolResult> {
        #[derive(Deserialize)]
        struct Input {
            points: Vec<NormalizedPoint>,
        }
        let input: Input = serde_json::from_value(arguments)
            .map_err(|error| CoreError::Tool(format!("invalid line arguments: {error}")))?;
        let image = context
            .image
            .as_deref()
            .ok_or_else(|| CoreError::Tool("current image is unavailable".to_owned()))?;
        let (points, support, continuity) =
            refine_points(image, &input.points, None, 12, 12, 0.62)?;
        let image_id = context
            .image_id
            .ok_or_else(|| CoreError::Tool("current image id is unavailable".to_owned()))?;
        let original_id = ArtifactId::new();
        let original = VisionArtifact {
            id: original_id,
            image_id,
            task_id: context.task_id.clone(),
            label: Some(LabelId::from("white_field_line")),
            role: ArtifactRole::Candidate,
            value: VisionArtifactValue::Polyline {
                points: input.points,
            },
            source_node: "refine_robocup_field_line.input".to_owned(),
            confidence: None,
            metadata: BTreeMap::default(),
            validation_state: ArtifactValidationState::Unvalidated,
            provenance: ArtifactProvenance::default(),
            created_at: chrono::Utc::now(),
        };
        let refined = VisionArtifact {
            id: ArtifactId::new(),
            image_id,
            task_id: context.task_id.clone(),
            label: Some(LabelId::from("white_field_line")),
            role: ArtifactRole::RefinedCandidate,
            value: VisionArtifactValue::Polyline { points },
            source_node: "refine_robocup_field_line".to_owned(),
            confidence: Some(f32::midpoint(support, continuity)),
            metadata: [
                ("pixel_support".to_owned(), json!(support)),
                ("continuity".to_owned(), json!(continuity)),
            ]
            .into_iter()
            .collect(),
            validation_state: ArtifactValidationState::Unvalidated,
            provenance: ArtifactProvenance {
                tool: Some("refine_robocup_field_line".to_owned()),
                input_artifact_ids: vec![original_id],
                ..ArtifactProvenance::default()
            },
            created_at: chrono::Utc::now(),
        };
        Ok(ToolResult::with_artifacts(
            format!(
                "created original and refined polyline artifacts; support={support:.3}, continuity={continuity:.3}"
            ),
            vec![original, refined],
            &json!({"pixel_support": support, "continuity": continuity}),
        ))
    }
}

pub struct BallEvidenceTool;

#[async_trait]
impl AgentTool for BallEvidenceTool {
    fn applicable_tasks(&self) -> Vec<annotagent_core::TaskId> {
        vec![annotagent_core::TaskId::from("objects")]
    }

    fn definition(&self) -> ToolDefinition {
        bbox_tool_definition(
            "evaluate_ball_hard_negative",
            "Measure white-pixel evidence and geometry for a controlled candidate crop",
        )
    }

    async fn execute(&self, context: &ToolContext, arguments: Value) -> CoreResult<ToolResult> {
        let rect = parse_bbox(arguments)?;
        let image = context
            .image
            .as_deref()
            .ok_or_else(|| CoreError::Tool("current image is unavailable".to_owned()))?;
        let statistics = color_statistics(image, rect)?;
        Ok(ToolResult::structured(
            format!(
                "ball crop white_ratio={:.3}, aspect_ratio={:.3}, relative_area={:.5}",
                statistics.white_ratio,
                rect.width() / rect.height(),
                rect.area()
            ),
            json!({
                "white_ratio": statistics.white_ratio,
                "aspect_ratio": rect.width() / rect.height(),
                "relative_area": rect.area()
            }),
        ))
    }
}

pub struct TeamColorEvidenceTool;

#[async_trait]
impl AgentTool for TeamColorEvidenceTool {
    fn applicable_tasks(&self) -> Vec<annotagent_core::TaskId> {
        vec![annotagent_core::TaskId::from("robot_attributes")]
    }

    fn definition(&self) -> ToolDefinition {
        bbox_tool_definition(
            "evaluate_robot_team_color",
            "Measure red and blue evidence inside the robot torso ROI",
        )
    }

    async fn execute(&self, context: &ToolContext, arguments: Value) -> CoreResult<ToolResult> {
        let rect = torso_rect(parse_bbox(arguments)?)?;
        let image = context
            .image
            .as_deref()
            .ok_or_else(|| CoreError::Tool("current image is unavailable".to_owned()))?;
        let statistics = color_statistics(image, rect)?;
        let recommendation = if statistics.red_ratio > statistics.blue_ratio * 1.5
            && statistics.red_ratio > 0.08
        {
            "red"
        } else if statistics.blue_ratio > statistics.red_ratio * 1.5 && statistics.blue_ratio > 0.08
        {
            "blue"
        } else {
            "unknown"
        };
        Ok(ToolResult::structured(
            format!(
                "torso evidence red={:.3}, blue={:.3}, recommendation={recommendation}",
                statistics.red_ratio, statistics.blue_ratio
            ),
            json!({
                "red_ratio": statistics.red_ratio,
                "blue_ratio": statistics.blue_ratio,
                "recommendation": recommendation
            }),
        ))
    }
}

fn bbox_tool_definition(name: &str, description: &str) -> ToolDefinition {
    ToolDefinition {
        name: name.to_owned(),
        description: description.to_owned(),
        parameters: json!({
            "type": "object",
            "properties": {
                "bbox": {
                    "type": "array",
                    "description": "Normalized [x,y,width,height], never [x1,y1,x2,y2] and never pixels; x+width<=1, y+height<=1",
                    "minItems": 4,
                    "maxItems": 4,
                    "items": {"type": "number", "minimum": 0, "maximum": 1}
                }
            },
            "required": ["bbox"],
            "additionalProperties": false
        }),
        read_only: true,
    }
}

fn parse_bbox(arguments: Value) -> CoreResult<NormalizedRect> {
    #[derive(Deserialize)]
    struct Input {
        bbox: NormalizedRect,
    }
    serde_json::from_value::<Input>(arguments)
        .map(|input| input.bbox)
        .map_err(|error| CoreError::Tool(format!("invalid bbox arguments: {error}")))
}
use std::collections::BTreeMap;
