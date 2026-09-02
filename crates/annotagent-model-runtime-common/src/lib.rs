#![forbid(unsafe_code)]

//! Model-neutral image, tensor, geometry, and mask operations shared by expert plugins.

use std::collections::VecDeque;

use image::{DynamicImage, GenericImageView, Rgb, RgbImage, imageops::FilterType};
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Error, PartialEq)]
pub enum RuntimeCommonError {
    #[error(
        "tensor shape cannot contain zero and must match {actual} values (shape requires {expected})"
    )]
    InvalidTensorShape { expected: usize, actual: usize },
    #[error("image dimensions must be non-zero")]
    EmptyImage,
    #[error("target dimensions must be non-zero")]
    EmptyTarget,
    #[error("normalization standard deviation at channel {channel} must be finite and non-zero")]
    InvalidStandardDeviation { channel: usize },
    #[error("numeric input contains a non-finite value at index {index}")]
    NonFinite { index: usize },
    #[error("mask dimensions do not match its data")]
    InvalidMask,
}

pub type Result<T> = std::result::Result<T, RuntimeCommonError>;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TensorLayout {
    Nchw,
    Nhwc,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TensorF32 {
    pub shape: Vec<usize>,
    pub values: Vec<f32>,
}

impl TensorF32 {
    pub fn new(shape: Vec<usize>, values: Vec<f32>) -> Result<Self> {
        let expected = element_count(&shape).unwrap_or(0);
        if expected == 0 || expected != values.len() {
            return Err(RuntimeCommonError::InvalidTensorShape {
                expected,
                actual: values.len(),
            });
        }
        validate_finite(&values)?;
        Ok(Self { shape, values })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct BoundingBox {
    pub x_min: f32,
    pub y_min: f32,
    pub x_max: f32,
    pub y_max: f32,
}

impl BoundingBox {
    #[must_use]
    pub fn from_cxcywh(center_x: f32, center_y: f32, width: f32, height: f32) -> Self {
        Self {
            x_min: center_x - width / 2.0,
            y_min: center_y - height / 2.0,
            x_max: center_x + width / 2.0,
            y_max: center_y + height / 2.0,
        }
    }

    #[must_use]
    pub fn to_cxcywh(self) -> [f32; 4] {
        [
            self.x_min.mul_add(0.5, self.x_max * 0.5),
            self.y_min.mul_add(0.5, self.y_max * 0.5),
            (self.x_max - self.x_min).max(0.0),
            (self.y_max - self.y_min).max(0.0),
        ]
    }

    #[must_use]
    pub fn width(self) -> f32 {
        (self.x_max - self.x_min).max(0.0)
    }

    #[must_use]
    pub fn height(self) -> f32 {
        (self.y_max - self.y_min).max(0.0)
    }

    #[must_use]
    pub fn area(self) -> f32 {
        self.width() * self.height()
    }

    #[must_use]
    pub fn clip(self, width: f32, height: f32) -> Self {
        Self {
            x_min: self.x_min.clamp(0.0, width),
            y_min: self.y_min.clamp(0.0, height),
            x_max: self.x_max.clamp(0.0, width),
            y_max: self.y_max.clamp(0.0, height),
        }
    }

    #[must_use]
    pub fn intersection_over_union(self, other: Self) -> f32 {
        let intersection = Self {
            x_min: self.x_min.max(other.x_min),
            y_min: self.y_min.max(other.y_min),
            x_max: self.x_max.min(other.x_max),
            y_max: self.y_max.min(other.y_max),
        }
        .area();
        let union = self.area() + other.area() - intersection;
        if union > 0.0 {
            intersection / union
        } else {
            0.0
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ScoredBox {
    pub bbox: BoundingBox,
    pub score: f32,
    pub class_id: i64,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct LetterboxTransform {
    pub source_width: u32,
    pub source_height: u32,
    pub target_width: u32,
    pub target_height: u32,
    pub scale: f32,
    pub pad_left: u32,
    pub pad_top: u32,
    pub resized_width: u32,
    pub resized_height: u32,
}

impl LetterboxTransform {
    #[must_use]
    pub fn to_source(self, bbox: BoundingBox) -> BoundingBox {
        BoundingBox {
            x_min: (bbox.x_min - self.pad_left as f32) / self.scale,
            y_min: (bbox.y_min - self.pad_top as f32) / self.scale,
            x_max: (bbox.x_max - self.pad_left as f32) / self.scale,
            y_max: (bbox.y_max - self.pad_top as f32) / self.scale,
        }
        .clip(self.source_width as f32, self.source_height as f32)
    }

    #[must_use]
    pub fn to_target(self, bbox: BoundingBox) -> BoundingBox {
        BoundingBox {
            x_min: bbox.x_min * self.scale + self.pad_left as f32,
            y_min: bbox.y_min * self.scale + self.pad_top as f32,
            x_max: bbox.x_max * self.scale + self.pad_left as f32,
            y_max: bbox.y_max * self.scale + self.pad_top as f32,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct BinaryMask {
    pub width: u32,
    pub height: u32,
    pub data: Vec<u8>,
}

impl BinaryMask {
    pub fn new(width: u32, height: u32, data: Vec<u8>) -> Result<Self> {
        if width == 0 || height == 0 || data.len() != width as usize * height as usize {
            return Err(RuntimeCommonError::InvalidMask);
        }
        Ok(Self {
            width,
            height,
            data: data.into_iter().map(|value| u8::from(value != 0)).collect(),
        })
    }

    #[must_use]
    pub fn get(&self, x: u32, y: u32) -> bool {
        self.data[y as usize * self.width as usize + x as usize] != 0
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConnectedComponent {
    pub area: usize,
    pub bbox: [u32; 4],
    pub pixels: Vec<[u32; 2]>,
}

pub fn resize(image: &DynamicImage, width: u32, height: u32) -> Result<DynamicImage> {
    if image.width() == 0 || image.height() == 0 {
        return Err(RuntimeCommonError::EmptyImage);
    }
    if width == 0 || height == 0 {
        return Err(RuntimeCommonError::EmptyTarget);
    }
    Ok(image.resize_exact(width, height, FilterType::Triangle))
}

pub fn letterbox(
    image: &DynamicImage,
    target_width: u32,
    target_height: u32,
    fill: [u8; 3],
) -> Result<(RgbImage, LetterboxTransform)> {
    let (source_width, source_height) = image.dimensions();
    if source_width == 0 || source_height == 0 {
        return Err(RuntimeCommonError::EmptyImage);
    }
    if target_width == 0 || target_height == 0 {
        return Err(RuntimeCommonError::EmptyTarget);
    }
    let scale = (target_width as f32 / source_width as f32)
        .min(target_height as f32 / source_height as f32);
    let resized_width = ((source_width as f32 * scale).round() as u32).clamp(1, target_width);
    let resized_height = ((source_height as f32 * scale).round() as u32).clamp(1, target_height);
    let pad_left = (target_width - resized_width) / 2;
    let pad_top = (target_height - resized_height) / 2;
    let resized = image
        .resize_exact(resized_width, resized_height, FilterType::Triangle)
        .to_rgb8();
    let mut output = RgbImage::from_pixel(target_width, target_height, Rgb(fill));
    image::imageops::replace(
        &mut output,
        &resized,
        i64::from(pad_left),
        i64::from(pad_top),
    );
    Ok((
        output,
        LetterboxTransform {
            source_width,
            source_height,
            target_width,
            target_height,
            scale,
            pad_left,
            pad_top,
            resized_width,
            resized_height,
        },
    ))
}

pub fn image_to_tensor(
    image: &RgbImage,
    layout: TensorLayout,
    scale: f32,
    mean: [f32; 3],
    standard_deviation: [f32; 3],
) -> Result<TensorF32> {
    for (channel, value) in standard_deviation.iter().enumerate() {
        if !value.is_finite() || *value == 0.0 {
            return Err(RuntimeCommonError::InvalidStandardDeviation { channel });
        }
    }
    let (width, height) = image.dimensions();
    if width == 0 || height == 0 {
        return Err(RuntimeCommonError::EmptyImage);
    }
    let pixel_count = width as usize * height as usize;
    let mut values = vec![0.0; pixel_count * 3];
    for (index, pixel) in image.pixels().enumerate() {
        for channel in 0..3 {
            let normalized =
                (f32::from(pixel[channel]) * scale - mean[channel]) / standard_deviation[channel];
            let destination = match layout {
                TensorLayout::Nchw => channel * pixel_count + index,
                TensorLayout::Nhwc => index * 3 + channel,
            };
            values[destination] = normalized;
        }
    }
    let shape = match layout {
        TensorLayout::Nchw => vec![1, 3, height as usize, width as usize],
        TensorLayout::Nhwc => vec![1, height as usize, width as usize, 3],
    };
    TensorF32::new(shape, values)
}

pub fn transpose_nchw_nhwc(tensor: &TensorF32) -> Result<TensorF32> {
    if tensor.shape.len() != 4 {
        return Err(RuntimeCommonError::InvalidTensorShape {
            expected: tensor.values.len(),
            actual: tensor.values.len(),
        });
    }
    let [batch, channels, height, width] = [
        tensor.shape[0],
        tensor.shape[1],
        tensor.shape[2],
        tensor.shape[3],
    ];
    let mut values = vec![0.0; tensor.values.len()];
    for n in 0..batch {
        for c in 0..channels {
            for y in 0..height {
                for x in 0..width {
                    let source = ((n * channels + c) * height + y) * width + x;
                    let destination = ((n * height + y) * width + x) * channels + c;
                    values[destination] = tensor.values[source];
                }
            }
        }
    }
    TensorF32::new(vec![batch, height, width, channels], values)
}

#[must_use]
pub fn non_maximum_suppression(
    boxes: &[ScoredBox],
    score_threshold: f32,
    iou_threshold: f32,
    class_agnostic: bool,
) -> Vec<ScoredBox> {
    let mut candidates: Vec<_> = boxes
        .iter()
        .filter(|candidate| candidate.score.is_finite() && candidate.score >= score_threshold)
        .cloned()
        .collect();
    candidates.sort_by(|left, right| right.score.total_cmp(&left.score));
    let mut kept: Vec<ScoredBox> = Vec::with_capacity(candidates.len());
    for candidate in candidates {
        let suppressed = kept.iter().any(|existing| {
            (class_agnostic || existing.class_id == candidate.class_id)
                && existing.bbox.intersection_over_union(candidate.bbox) > iou_threshold
        });
        if !suppressed {
            kept.push(candidate);
        }
    }
    kept
}

pub fn threshold_mask(
    values: &[f32],
    width: u32,
    height: u32,
    threshold: f32,
) -> Result<BinaryMask> {
    validate_finite(values)?;
    BinaryMask::new(
        width,
        height,
        values
            .iter()
            .map(|value| u8::from(*value >= threshold))
            .collect(),
    )
}

pub fn resize_mask(mask: &BinaryMask, target_width: u32, target_height: u32) -> Result<BinaryMask> {
    if target_width == 0 || target_height == 0 {
        return Err(RuntimeCommonError::EmptyTarget);
    }
    let mut output = vec![0; target_width as usize * target_height as usize];
    for y in 0..target_height {
        let source_y = (u64::from(y) * u64::from(mask.height) / u64::from(target_height)) as u32;
        for x in 0..target_width {
            let source_x = (u64::from(x) * u64::from(mask.width) / u64::from(target_width)) as u32;
            output[y as usize * target_width as usize + x as usize] =
                u8::from(mask.get(source_x.min(mask.width - 1), source_y.min(mask.height - 1)));
        }
    }
    BinaryMask::new(target_width, target_height, output)
}

#[must_use]
pub fn connected_components(mask: &BinaryMask, minimum_area: usize) -> Vec<ConnectedComponent> {
    let mut seen = vec![false; mask.data.len()];
    let mut output = Vec::new();
    for start_y in 0..mask.height {
        for start_x in 0..mask.width {
            let start = start_y as usize * mask.width as usize + start_x as usize;
            if seen[start] || !mask.get(start_x, start_y) {
                continue;
            }
            let mut queue = VecDeque::from([(start_x, start_y)]);
            seen[start] = true;
            let mut pixels = Vec::new();
            let mut bounds = [start_x, start_y, start_x + 1, start_y + 1];
            while let Some((x, y)) = queue.pop_front() {
                pixels.push([x, y]);
                bounds[0] = bounds[0].min(x);
                bounds[1] = bounds[1].min(y);
                bounds[2] = bounds[2].max(x + 1);
                bounds[3] = bounds[3].max(y + 1);
                for (next_x, next_y) in four_neighbors(x, y, mask.width, mask.height) {
                    let index = next_y as usize * mask.width as usize + next_x as usize;
                    if !seen[index] && mask.get(next_x, next_y) {
                        seen[index] = true;
                        queue.push_back((next_x, next_y));
                    }
                }
            }
            if pixels.len() >= minimum_area {
                output.push(ConnectedComponent {
                    area: pixels.len(),
                    bbox: bounds,
                    pixels,
                });
            }
        }
    }
    output.sort_by_key(|component| std::cmp::Reverse(component.area));
    output
}

#[must_use]
pub fn contour(component: &ConnectedComponent, mask: &BinaryMask) -> Vec<[f32; 2]> {
    let mut boundary: Vec<_> = component
        .pixels
        .iter()
        .filter(|point| {
            let [x, y] = **point;
            x == 0
                || y == 0
                || x + 1 == mask.width
                || y + 1 == mask.height
                || four_neighbors(x, y, mask.width, mask.height)
                    .iter()
                    .any(|(next_x, next_y)| !mask.get(*next_x, *next_y))
        })
        .map(|[x, y]| [*x as f32 + 0.5, *y as f32 + 0.5])
        .collect();
    if boundary.len() < 3 {
        return boundary;
    }
    let center_x = boundary.iter().map(|point| point[0]).sum::<f32>() / boundary.len() as f32;
    let center_y = boundary.iter().map(|point| point[1]).sum::<f32>() / boundary.len() as f32;
    boundary.sort_by(|left, right| {
        (left[1] - center_y)
            .atan2(left[0] - center_x)
            .total_cmp(&(right[1] - center_y).atan2(right[0] - center_x))
    });
    boundary
}

#[must_use]
pub fn simplify_polygon(points: &[[f32; 2]], epsilon: f32) -> Vec<[f32; 2]> {
    if points.len() <= 2 || epsilon <= 0.0 {
        return points.to_vec();
    }
    let mut closed = points.to_vec();
    if closed.first() != closed.last() {
        closed.push(closed[0]);
    }
    let mut simplified = douglas_peucker(&closed, epsilon);
    if simplified.len() > 1 && simplified.first() == simplified.last() {
        simplified.pop();
    }
    simplified
}

pub fn validate_finite(values: &[f32]) -> Result<()> {
    if let Some(index) = values.iter().position(|value| !value.is_finite()) {
        return Err(RuntimeCommonError::NonFinite { index });
    }
    Ok(())
}

fn element_count(shape: &[usize]) -> Option<usize> {
    shape.iter().try_fold(1_usize, |count, dimension| {
        if *dimension == 0 {
            None
        } else {
            count.checked_mul(*dimension)
        }
    })
}

fn four_neighbors(x: u32, y: u32, width: u32, height: u32) -> Vec<(u32, u32)> {
    let mut neighbors = Vec::with_capacity(4);
    if x > 0 {
        neighbors.push((x - 1, y));
    }
    if x + 1 < width {
        neighbors.push((x + 1, y));
    }
    if y > 0 {
        neighbors.push((x, y - 1));
    }
    if y + 1 < height {
        neighbors.push((x, y + 1));
    }
    neighbors
}

fn douglas_peucker(points: &[[f32; 2]], epsilon: f32) -> Vec<[f32; 2]> {
    if points.len() <= 2 {
        return points.to_vec();
    }
    let first = points[0];
    let last = points[points.len() - 1];
    let mut maximum_distance = 0.0;
    let mut split_index = 0;
    for (index, point) in points.iter().enumerate().take(points.len() - 1).skip(1) {
        let distance = perpendicular_distance(*point, first, last);
        if distance > maximum_distance {
            maximum_distance = distance;
            split_index = index;
        }
    }
    if maximum_distance <= epsilon {
        return vec![first, last];
    }
    let mut left = douglas_peucker(&points[..=split_index], epsilon);
    let right = douglas_peucker(&points[split_index..], epsilon);
    left.pop();
    left.extend(right);
    left
}

fn perpendicular_distance(point: [f32; 2], start: [f32; 2], end: [f32; 2]) -> f32 {
    let delta_x = end[0] - start[0];
    let delta_y = end[1] - start[1];
    if delta_x == 0.0 && delta_y == 0.0 {
        return ((point[0] - start[0]).powi(2) + (point[1] - start[1]).powi(2)).sqrt();
    }
    ((delta_y * point[0] - delta_x * point[1] + end[0] * start[1] - end[1] * start[0]).abs())
        / delta_x.hypot(delta_y)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn letterbox_round_trips_geometry() {
        let image = DynamicImage::ImageRgb8(RgbImage::new(200, 100));
        let (_, transform) = letterbox(&image, 640, 640, [114; 3]).expect("letterbox");
        let source = BoundingBox {
            x_min: 20.0,
            y_min: 10.0,
            x_max: 120.0,
            y_max: 80.0,
        };
        let round_trip = transform.to_source(transform.to_target(source));
        assert!((round_trip.x_min - source.x_min).abs() < 0.001);
        assert!((round_trip.y_max - source.y_max).abs() < 0.001);
        assert_eq!(transform.pad_top, 160);
    }

    #[test]
    fn tensor_layouts_and_normalization_are_deterministic() {
        let image = RgbImage::from_pixel(2, 1, Rgb([255, 128, 0]));
        let nchw = image_to_tensor(&image, TensorLayout::Nchw, 1.0 / 255.0, [0.0; 3], [1.0; 3])
            .expect("tensor");
        let nhwc = transpose_nchw_nhwc(&nchw).expect("transpose");
        assert_eq!(nhwc.shape, [1, 1, 2, 3]);
        assert!((nhwc.values[1] - 128.0 / 255.0).abs() < 0.0001);
    }

    #[test]
    fn nms_is_class_aware() {
        let boxes = vec![
            ScoredBox {
                bbox: BoundingBox::from_cxcywh(10.0, 10.0, 10.0, 10.0),
                score: 0.9,
                class_id: 1,
            },
            ScoredBox {
                bbox: BoundingBox::from_cxcywh(10.0, 10.0, 10.0, 10.0),
                score: 0.8,
                class_id: 1,
            },
            ScoredBox {
                bbox: BoundingBox::from_cxcywh(10.0, 10.0, 10.0, 10.0),
                score: 0.7,
                class_id: 2,
            },
        ];
        assert_eq!(non_maximum_suppression(&boxes, 0.5, 0.5, false).len(), 2);
        assert_eq!(non_maximum_suppression(&boxes, 0.5, 0.5, true).len(), 1);
    }

    #[test]
    fn mask_operations_preserve_components() {
        let mask = BinaryMask::new(4, 3, vec![0, 1, 1, 0, 0, 1, 1, 0, 0, 0, 0, 1]).expect("mask");
        let components = connected_components(&mask, 2);
        assert_eq!(components.len(), 1);
        assert_eq!(components[0].area, 4);
        assert!(contour(&components[0], &mask).len() >= 3);
        assert_eq!(resize_mask(&mask, 8, 6).expect("resize").data.len(), 48);
    }
}
