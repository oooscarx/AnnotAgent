//! Ground-truth-only evaluation for `RoboCup` workflow outputs.

use std::collections::{BTreeMap, BTreeSet};

use annotagent_core::{CoreError, CoreResult};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EvaluationGroundTruth {
    pub schema_version: u32,
    pub dataset_name: String,
    /// Must be true. Unlabelled data is never treated as an accuracy fixture.
    pub labeled: bool,
    pub images: Vec<GroundTruthImage>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EvaluationPredictions {
    pub schema_version: u32,
    pub dataset_name: String,
    pub images: Vec<PredictionImage>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct GroundTruthImage {
    pub id: String,
    #[serde(default)]
    pub boxes: Vec<EvaluationBox>,
    #[serde(default)]
    pub masks: Vec<EvaluationMask>,
    #[serde(default)]
    pub keypoints: Vec<EvaluationPoints>,
    #[serde(default)]
    pub polylines: Vec<EvaluationPoints>,
    #[serde(default)]
    pub classifications: BTreeMap<String, String>,
    #[serde(default)]
    pub attributes: BTreeMap<String, BTreeMap<String, String>>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct PredictionImage {
    pub id: String,
    #[serde(default)]
    pub boxes: Vec<EvaluationBox>,
    #[serde(default)]
    pub masks: Vec<EvaluationMask>,
    #[serde(default)]
    pub keypoints: Vec<EvaluationPoints>,
    #[serde(default)]
    pub polylines: Vec<EvaluationPoints>,
    #[serde(default)]
    pub classifications: BTreeMap<String, String>,
    #[serde(default)]
    pub attributes: BTreeMap<String, BTreeMap<String, String>>,
    #[serde(default)]
    pub review_required: bool,
    #[serde(default)]
    pub failed: bool,
    #[serde(default)]
    pub cost: f64,
    #[serde(default)]
    pub latency_ms: u64,
    #[serde(default)]
    pub model_calls: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EvaluationBox {
    pub id: String,
    pub label: String,
    /// Normalized `[x, y, width, height]`.
    pub rect: [f64; 4],
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EvaluationMask {
    pub id: String,
    pub width: u32,
    pub height: u32,
    pub pixels: Vec<bool>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EvaluationPoints {
    pub id: String,
    pub points: Vec<[f64; 2]>,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct MetricValue {
    pub value: Option<f64>,
    pub samples: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct BoundingBoxMetrics {
    pub iou_threshold: f64,
    pub mean_matched_iou: Option<f64>,
    pub precision: Option<f64>,
    pub recall: Option<f64>,
    pub true_positive: u64,
    pub false_positive: u64,
    pub false_negative: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EvaluationReport {
    pub schema_version: u32,
    pub ground_truth_dataset: String,
    pub prediction_dataset: String,
    pub image_count: u64,
    pub thresholds: EvaluationThresholds,
    pub quality_gates: EvaluationQualityGates,
    pub bbox: BoundingBoxMetrics,
    pub mask_iou: MetricValue,
    pub keypoint_distance: MetricValue,
    pub polyline_point_to_line_distance: MetricValue,
    pub classification_accuracy: MetricValue,
    pub attribute_accuracy: MetricValue,
    pub review_rate: MetricValue,
    pub failure_rate: MetricValue,
    pub cost_per_image: MetricValue,
    pub latency_ms_per_image: MetricValue,
    pub model_calls_per_image: MetricValue,
    pub missing_prediction_images: Vec<String>,
    pub extra_prediction_images: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct EvaluationThresholds {
    pub bbox_iou: f64,
    pub minimum_field_region_mask_iou: Option<f64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvaluationQualityGates {
    pub field_region_mask_iou_passed: Option<bool>,
}

pub fn evaluate(
    ground_truth: &EvaluationGroundTruth,
    predictions: &EvaluationPredictions,
    bbox_iou_threshold: f64,
) -> CoreResult<EvaluationReport> {
    evaluate_with_thresholds(
        ground_truth,
        predictions,
        EvaluationThresholds {
            bbox_iou: bbox_iou_threshold,
            minimum_field_region_mask_iou: None,
        },
    )
}

pub fn evaluate_with_thresholds(
    ground_truth: &EvaluationGroundTruth,
    predictions: &EvaluationPredictions,
    thresholds: EvaluationThresholds,
) -> CoreResult<EvaluationReport> {
    if !ground_truth.labeled {
        return Err(CoreError::Validation(
            "accuracy evaluation requires labeled ground truth; use run telemetry for unlabeled real data"
                .to_owned(),
        ));
    }
    if ground_truth.schema_version != 1 || predictions.schema_version != 1 {
        return Err(CoreError::Validation(
            "evaluation schema_version must be 1".to_owned(),
        ));
    }
    if !thresholds.bbox_iou.is_finite() || !(0.0..=1.0).contains(&thresholds.bbox_iou) {
        return Err(CoreError::Validation(
            "bbox IoU threshold must be within [0,1]".to_owned(),
        ));
    }
    if thresholds
        .minimum_field_region_mask_iou
        .is_some_and(|threshold| !threshold.is_finite() || !(0.0..=1.0).contains(&threshold))
    {
        return Err(CoreError::Validation(
            "minimum field-region mask IoU must be within [0,1]".to_owned(),
        ));
    }
    let predicted = predictions
        .images
        .iter()
        .map(|image| (image.id.as_str(), image))
        .collect::<BTreeMap<_, _>>();
    let truth_ids = ground_truth
        .images
        .iter()
        .map(|image| image.id.as_str())
        .collect::<BTreeSet<_>>();
    let mut missing = Vec::new();
    let mut bbox = BoundingAccumulator::default();
    let mut mask = Average::default();
    let mut keypoints = Average::default();
    let mut polylines = Average::default();
    let mut classifications = Correctness::default();
    let mut attributes = Correctness::default();
    let mut review = 0_u64;
    let mut failed = 0_u64;
    let mut cost = 0.0;
    let mut latency = 0_u64;
    let mut calls = 0_u64;

    for truth in &ground_truth.images {
        let prediction = predicted.get(truth.id.as_str()).copied();
        let empty = PredictionImage {
            id: truth.id.clone(),
            failed: true,
            ..PredictionImage::default()
        };
        let prediction = prediction.unwrap_or_else(|| {
            missing.push(truth.id.clone());
            &empty
        });
        bbox.add(&truth.boxes, &prediction.boxes, thresholds.bbox_iou);
        compare_masks(&truth.masks, &prediction.masks, &mut mask)?;
        compare_point_sets(&truth.keypoints, &prediction.keypoints, &mut keypoints);
        compare_polylines(&truth.polylines, &prediction.polylines, &mut polylines);
        classifications.add_maps(&truth.classifications, &prediction.classifications);
        for (object_id, expected) in &truth.attributes {
            let actual = prediction.attributes.get(object_id);
            for (key, expected_value) in expected {
                attributes.add(actual.and_then(|values| values.get(key)) == Some(expected_value));
            }
        }
        review += u64::from(prediction.review_required);
        failed += u64::from(prediction.failed);
        cost += prediction.cost;
        latency += prediction.latency_ms;
        calls += prediction.model_calls;
    }
    let extra_prediction_images = predictions
        .images
        .iter()
        .filter(|image| !truth_ids.contains(image.id.as_str()))
        .map(|image| image.id.clone())
        .collect();
    let image_count = u64::try_from(ground_truth.images.len()).unwrap_or(u64::MAX);
    let mask_iou = mask.finish();
    let field_region_mask_iou_passed = thresholds
        .minimum_field_region_mask_iou
        .map(|threshold| mask_iou.value.is_some_and(|value| value >= threshold));
    Ok(EvaluationReport {
        schema_version: 1,
        ground_truth_dataset: ground_truth.dataset_name.clone(),
        prediction_dataset: predictions.dataset_name.clone(),
        image_count,
        thresholds,
        quality_gates: EvaluationQualityGates {
            field_region_mask_iou_passed,
        },
        bbox: bbox.finish(thresholds.bbox_iou),
        mask_iou,
        keypoint_distance: keypoints.finish(),
        polyline_point_to_line_distance: polylines.finish(),
        classification_accuracy: classifications.finish(),
        attribute_accuracy: attributes.finish(),
        review_rate: ratio_metric(review, image_count),
        failure_rate: ratio_metric(failed, image_count),
        cost_per_image: average_metric(cost, image_count),
        latency_ms_per_image: average_metric(latency as f64, image_count),
        model_calls_per_image: average_metric(calls as f64, image_count),
        missing_prediction_images: missing,
        extra_prediction_images,
    })
}

#[derive(Default)]
struct BoundingAccumulator {
    tp: u64,
    fp: u64,
    fn_count: u64,
    matched_iou: Average,
}

impl BoundingAccumulator {
    fn add(&mut self, truth: &[EvaluationBox], predicted: &[EvaluationBox], threshold: f64) {
        let mut used = BTreeSet::new();
        for candidate in predicted {
            let best = truth
                .iter()
                .enumerate()
                .filter(|(index, expected)| {
                    !used.contains(index) && expected.label == candidate.label
                })
                .map(|(index, expected)| (index, rect_iou(expected.rect, candidate.rect)))
                .max_by(|left, right| left.1.total_cmp(&right.1));
            if let Some((index, iou)) = best.filter(|(_, iou)| *iou >= threshold) {
                used.insert(index);
                self.tp += 1;
                self.matched_iou.add(iou);
            } else {
                self.fp += 1;
            }
        }
        self.fn_count += u64::try_from(truth.len().saturating_sub(used.len())).unwrap_or(u64::MAX);
    }

    fn finish(self, threshold: f64) -> BoundingBoxMetrics {
        BoundingBoxMetrics {
            iou_threshold: threshold,
            mean_matched_iou: self.matched_iou.value(),
            precision: ratio(self.tp, self.tp + self.fp),
            recall: ratio(self.tp, self.tp + self.fn_count),
            true_positive: self.tp,
            false_positive: self.fp,
            false_negative: self.fn_count,
        }
    }
}

#[derive(Default)]
struct Average {
    sum: f64,
    samples: u64,
}

impl Average {
    fn add(&mut self, value: f64) {
        if value.is_finite() {
            self.sum += value;
            self.samples += 1;
        }
    }

    fn value(&self) -> Option<f64> {
        (self.samples > 0).then(|| self.sum / self.samples as f64)
    }

    fn finish(self) -> MetricValue {
        MetricValue {
            value: self.value(),
            samples: self.samples,
        }
    }
}

#[derive(Default)]
struct Correctness {
    correct: u64,
    total: u64,
}

impl Correctness {
    fn add(&mut self, correct: bool) {
        self.correct += u64::from(correct);
        self.total += 1;
    }

    fn add_maps(&mut self, expected: &BTreeMap<String, String>, actual: &BTreeMap<String, String>) {
        for (key, value) in expected {
            self.add(actual.get(key) == Some(value));
        }
    }

    fn finish(self) -> MetricValue {
        MetricValue {
            value: ratio(self.correct, self.total),
            samples: self.total,
        }
    }
}

fn compare_masks(
    truth: &[EvaluationMask],
    predicted: &[EvaluationMask],
    result: &mut Average,
) -> CoreResult<()> {
    let predicted = predicted
        .iter()
        .map(|mask| (mask.id.as_str(), mask))
        .collect::<BTreeMap<_, _>>();
    for expected in truth {
        validate_mask(expected)?;
        let Some(actual) = predicted.get(expected.id.as_str()) else {
            result.add(0.0);
            continue;
        };
        validate_mask(actual)?;
        if expected.width != actual.width || expected.height != actual.height {
            result.add(0.0);
            continue;
        }
        let intersection = expected
            .pixels
            .iter()
            .zip(&actual.pixels)
            .filter(|(left, right)| **left && **right)
            .count();
        let union = expected
            .pixels
            .iter()
            .zip(&actual.pixels)
            .filter(|(left, right)| **left || **right)
            .count();
        result.add(if union == 0 {
            1.0
        } else {
            intersection as f64 / union as f64
        });
    }
    Ok(())
}

fn validate_mask(mask: &EvaluationMask) -> CoreResult<()> {
    let expected = u64::from(mask.width) * u64::from(mask.height);
    if u64::try_from(mask.pixels.len()).unwrap_or(u64::MAX) != expected {
        return Err(CoreError::Validation(format!(
            "mask {:?} dimensions do not match its pixel array",
            mask.id
        )));
    }
    Ok(())
}

fn compare_point_sets(
    truth: &[EvaluationPoints],
    predicted: &[EvaluationPoints],
    result: &mut Average,
) {
    let predicted = predicted
        .iter()
        .map(|set| (set.id.as_str(), set))
        .collect::<BTreeMap<_, _>>();
    for expected in truth {
        let actual = predicted.get(expected.id.as_str());
        for (index, point) in expected.points.iter().enumerate() {
            result.add(
                actual
                    .and_then(|set| set.points.get(index))
                    .map_or(f64::sqrt(2.0), |candidate| distance(*point, *candidate)),
            );
        }
    }
}

fn compare_polylines(
    truth: &[EvaluationPoints],
    predicted: &[EvaluationPoints],
    result: &mut Average,
) {
    let predicted = predicted
        .iter()
        .map(|line| (line.id.as_str(), line))
        .collect::<BTreeMap<_, _>>();
    for expected in truth {
        let Some(actual) = predicted.get(expected.id.as_str()) else {
            for _ in &expected.points {
                result.add(f64::sqrt(2.0));
            }
            continue;
        };
        for point in &actual.points {
            let nearest = expected
                .points
                .windows(2)
                .map(|segment| point_segment_distance(*point, segment[0], segment[1]))
                .min_by(f64::total_cmp)
                .unwrap_or(f64::sqrt(2.0));
            result.add(nearest);
        }
    }
}

fn rect_iou(left: [f64; 4], right: [f64; 4]) -> f64 {
    let x1 = left[0].max(right[0]);
    let y1 = left[1].max(right[1]);
    let x2 = (left[0] + left[2]).min(right[0] + right[2]);
    let y2 = (left[1] + left[3]).min(right[1] + right[3]);
    let intersection = (x2 - x1).max(0.0) * (y2 - y1).max(0.0);
    let union = left[2] * left[3] + right[2] * right[3] - intersection;
    if union > 0.0 {
        intersection / union
    } else {
        0.0
    }
}

fn distance(left: [f64; 2], right: [f64; 2]) -> f64 {
    (left[0] - right[0]).hypot(left[1] - right[1])
}

fn point_segment_distance(point: [f64; 2], start: [f64; 2], end: [f64; 2]) -> f64 {
    let dx = end[0] - start[0];
    let dy = end[1] - start[1];
    let length = dx * dx + dy * dy;
    if length == 0.0 {
        return distance(point, start);
    }
    let t = (((point[0] - start[0]) * dx + (point[1] - start[1]) * dy) / length).clamp(0.0, 1.0);
    distance(point, [start[0] + t * dx, start[1] + t * dy])
}

fn ratio(numerator: u64, denominator: u64) -> Option<f64> {
    (denominator > 0).then(|| numerator as f64 / denominator as f64)
}

fn ratio_metric(numerator: u64, denominator: u64) -> MetricValue {
    MetricValue {
        value: ratio(numerator, denominator),
        samples: denominator,
    }
}

fn average_metric(total: f64, samples: u64) -> MetricValue {
    MetricValue {
        value: (samples > 0).then(|| total / samples as f64),
        samples,
    }
}
