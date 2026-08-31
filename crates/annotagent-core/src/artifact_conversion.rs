//! Capability-neutral paths between typed Pipeline Artifacts.
//!
//! The registry describes legal composition. It never assumes a model brand: a prompted
//! segmenter becomes usable only when its capability node and every required Core conversion
//! node are registered.

use std::collections::{BTreeSet, VecDeque};

use serde::{Deserialize, Serialize};

use crate::{ArtifactKind, MaskEncoding, NodeRegistry, NormalizedPoint, NormalizedRect};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArtifactConversion {
    pub from: ArtifactKind,
    pub to: ArtifactKind,
    pub node_id: String,
    #[serde(default)]
    pub additional_inputs: Vec<ArtifactKind>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConversionPath {
    pub from: ArtifactKind,
    pub to: ArtifactKind,
    pub steps: Vec<ArtifactConversion>,
}

#[derive(Debug, Clone)]
pub struct ArtifactConversionRegistry {
    conversions: Vec<ArtifactConversion>,
}

impl Default for ArtifactConversionRegistry {
    fn default() -> Self {
        Self {
            conversions: vec![
                conversion(
                    ArtifactKind::DetectionSet,
                    ArtifactKind::BoxPromptSet,
                    "core.detections_to_box_prompts",
                    &[],
                ),
                conversion(
                    ArtifactKind::BoxPromptSet,
                    ArtifactKind::MaskSet,
                    "capability.segment",
                    &[ArtifactKind::Image],
                ),
                conversion(
                    ArtifactKind::PointPromptSet,
                    ArtifactKind::MaskSet,
                    "capability.segment",
                    &[ArtifactKind::Image],
                ),
                conversion(
                    ArtifactKind::MaskSet,
                    ArtifactKind::DetectionSet,
                    "core.mask_to_bbox",
                    &[],
                ),
                conversion(
                    ArtifactKind::MaskSet,
                    ArtifactKind::PolygonSet,
                    "core.mask_to_polygon",
                    &[],
                ),
                conversion(
                    ArtifactKind::DetectionSet,
                    ArtifactKind::CropSet,
                    "core.crop",
                    &[ArtifactKind::Image],
                ),
                conversion(
                    ArtifactKind::CropSet,
                    ArtifactKind::ClassificationSet,
                    "capability.classify",
                    &[],
                ),
                conversion(
                    ArtifactKind::ClassificationSet,
                    ArtifactKind::AnnotationCandidateSet,
                    "core.attach_result",
                    &[ArtifactKind::DetectionSet],
                ),
            ],
        }
    }
}

fn conversion(
    from: ArtifactKind,
    to: ArtifactKind,
    node_id: &str,
    additional_inputs: &[ArtifactKind],
) -> ArtifactConversion {
    ArtifactConversion {
        from,
        to,
        node_id: node_id.to_owned(),
        additional_inputs: additional_inputs.to_vec(),
    }
}

impl ArtifactConversionRegistry {
    #[must_use]
    pub fn conversions(&self) -> &[ArtifactConversion] {
        &self.conversions
    }

    pub fn register(&mut self, conversion: ArtifactConversion) -> Result<(), String> {
        if conversion.node_id.trim().is_empty() {
            return Err("Artifact conversion node_id cannot be empty".to_owned());
        }
        if self.conversions.iter().any(|existing| {
            existing.from == conversion.from
                && existing.to == conversion.to
                && existing.node_id == conversion.node_id
        }) {
            return Err("Artifact conversion is already registered".to_owned());
        }
        self.conversions.push(conversion);
        Ok(())
    }

    /// Returns every shortest legal path. A same-type request may return a non-empty cycle, which
    /// is how a geometry refinement such as `DetectionSet` -> SAM -> `DetectionSet` is represented.
    #[must_use]
    pub fn find_conversion_path(
        &self,
        from: ArtifactKind,
        to: ArtifactKind,
        available_nodes: &NodeRegistry,
    ) -> Vec<ConversionPath> {
        let maximum_depth = self.conversions.len().min(12);
        let mut queue = VecDeque::from([(from, Vec::<ArtifactConversion>::new())]);
        let mut shortest = None;
        let mut results = Vec::new();
        while let Some((current, path)) = queue.pop_front() {
            if shortest.is_some_and(|length| path.len() >= length) || path.len() >= maximum_depth {
                continue;
            }
            let used_edges = path
                .iter()
                .map(|step| (step.from, step.to, step.node_id.as_str()))
                .collect::<BTreeSet<_>>();
            for next in self.conversions.iter().filter(|step| {
                step.from == current
                    && available_nodes.get(&step.node_id).is_some()
                    && !used_edges.contains(&(step.from, step.to, step.node_id.as_str()))
            }) {
                let mut next_path = path.clone();
                next_path.push(next.clone());
                if next.to == to {
                    shortest = Some(next_path.len());
                    results.push(ConversionPath {
                        from,
                        to,
                        steps: next_path,
                    });
                } else {
                    queue.push_back((next.to, next_path));
                }
            }
        }
        results
    }
}

/// Computes the tight normalized box of a polygon or uncompressed COCO RLE mask.
pub fn mask_tight_bbox(mask: &MaskEncoding) -> Result<NormalizedRect, String> {
    match mask {
        MaskEncoding::Polygon { rings } => bbox_from_points(rings.iter().flatten()),
        MaskEncoding::CocoRle {
            width,
            height,
            counts,
        } => bbox_from_uncompressed_rle(*width, *height, counts),
    }
}

fn bbox_from_points<'a>(
    points: impl Iterator<Item = &'a NormalizedPoint>,
) -> Result<NormalizedRect, String> {
    let mut bounds: Option<(f32, f32, f32, f32)> = None;
    for point in points {
        bounds = Some(bounds.map_or(
            (point.x(), point.y(), point.x(), point.y()),
            |(left, top, right, bottom)| {
                (
                    left.min(point.x()),
                    top.min(point.y()),
                    right.max(point.x()),
                    bottom.max(point.y()),
                )
            },
        ));
    }
    let Some((left, top, right, bottom)) = bounds else {
        return Err("mask contains no foreground geometry".to_owned());
    };
    NormalizedRect::new(left, top, right - left, bottom - top).map_err(|error| error.to_string())
}

fn bbox_from_uncompressed_rle(
    width: u32,
    height: u32,
    counts: &str,
) -> Result<NormalizedRect, String> {
    if width == 0 || height == 0 {
        return Err("mask has zero dimensions".to_owned());
    }
    let runs = counts
        .split_whitespace()
        .map(|value| {
            value.parse::<u64>().map_err(|_| {
                "core.mask_to_bbox requires polygon or uncompressed COCO RLE counts".to_owned()
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    let expected = u64::from(width) * u64::from(height);
    if runs.iter().sum::<u64>() != expected {
        return Err("COCO RLE counts do not cover the declared mask dimensions".to_owned());
    }
    let mut cursor = 0_u64;
    let mut bounds: Option<(u32, u32, u32, u32)> = None;
    for (index, length) in runs.into_iter().enumerate() {
        if index % 2 == 1 {
            for offset in cursor..cursor + length {
                // COCO RLE is column-major.
                let x = (offset / u64::from(height)) as u32;
                let y = (offset % u64::from(height)) as u32;
                bounds = Some(bounds.map_or((x, y, x, y), |(left, top, right, bottom)| {
                    (left.min(x), top.min(y), right.max(x), bottom.max(y))
                }));
            }
        }
        cursor += length;
    }
    let Some((left, top, right, bottom)) = bounds else {
        return Err("mask contains no foreground pixels".to_owned());
    };
    NormalizedRect::new(
        left as f32 / width as f32,
        top as f32 / height as f32,
        (right - left + 1) as f32 / width as f32,
        (bottom - top + 1) as f32 / height as f32,
    )
    .map_err(|error| error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::VisionNodeDescriptor;

    fn nodes(ids: &[&str]) -> NodeRegistry {
        let mut nodes = NodeRegistry::new();
        for id in ids {
            nodes
                .register(VisionNodeDescriptor {
                    id: (*id).to_owned(),
                    display_name: (*id).to_owned(),
                    required_capabilities: Vec::new(),
                    accepts: vec![ArtifactKind::Image],
                    produces: vec![ArtifactKind::Image],
                    deterministic: true,
                })
                .expect("node");
        }
        nodes
    }

    #[test]
    fn finds_explicit_sam_refinement_cycle_only_when_every_node_exists() {
        let registry = ArtifactConversionRegistry::default();
        let complete = nodes(&[
            "core.detections_to_box_prompts",
            "capability.segment",
            "core.mask_to_bbox",
        ]);
        let paths = registry.find_conversion_path(
            ArtifactKind::DetectionSet,
            ArtifactKind::DetectionSet,
            &complete,
        );
        assert_eq!(paths.len(), 1);
        assert_eq!(
            paths[0]
                .steps
                .iter()
                .map(|step| step.node_id.as_str())
                .collect::<Vec<_>>(),
            vec![
                "core.detections_to_box_prompts",
                "capability.segment",
                "core.mask_to_bbox"
            ]
        );

        let incomplete = nodes(&["core.detections_to_box_prompts", "core.mask_to_bbox"]);
        assert!(
            registry
                .find_conversion_path(
                    ArtifactKind::DetectionSet,
                    ArtifactKind::DetectionSet,
                    &incomplete,
                )
                .is_empty()
        );
    }

    #[test]
    fn tight_bbox_supports_polygon_and_uncompressed_coco_rle() {
        let polygon = MaskEncoding::Polygon {
            rings: vec![vec![
                NormalizedPoint::new(0.2, 0.3).expect("point"),
                NormalizedPoint::new(0.6, 0.3).expect("point"),
                NormalizedPoint::new(0.6, 0.8).expect("point"),
            ]],
        };
        let bbox = mask_tight_bbox(&polygon).expect("bbox");
        assert!((bbox.x() - 0.2).abs() < f32::EPSILON);
        assert!((bbox.height() - 0.5).abs() < f32::EPSILON);

        // 2x2 column-major mask with the top-right pixel set.
        let rle = MaskEncoding::CocoRle {
            width: 2,
            height: 2,
            counts: "2 1 1".to_owned(),
        };
        let bbox = mask_tight_bbox(&rle).expect("bbox");
        assert!((bbox.x() - 0.5).abs() < f32::EPSILON);
        assert!(bbox.y().abs() < f32::EPSILON);
        assert!((bbox.width() - 0.5).abs() < f32::EPSILON);
        assert!((bbox.height() - 0.5).abs() < f32::EPSILON);
    }
}
