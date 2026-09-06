//! Bounded image loading, crops, statistics, geometry, and synthetic fixtures.

use std::{io::Cursor, path::Path};

use annotagent_core::{
    ArtifactRef, CoordinateTransform, CoreError, CoreResult, ImageFrame, ImageId, ImageMetadata,
    ModelImage, ModelInputTrace, NormalizedPoint, NormalizedRect, ProviderEffectiveDimensions,
};
use base64::{Engine as _, engine::general_purpose::STANDARD};
use image::{
    DynamicImage, ImageBuffer, ImageFormat, ImageReader, Rgb, RgbImage,
    imageops::{self, FilterType},
};
use sha2::{Digest, Sha256};

pub fn load_image(path: &Path, max_decode_pixels: u64) -> CoreResult<ImageFrame> {
    let reader = ImageReader::open(path)
        .map_err(|error| CoreError::InvalidGeometry(format!("cannot open image: {error}")))?
        .with_guessed_format()
        .map_err(|error| {
            CoreError::InvalidGeometry(format!("cannot detect image format: {error}"))
        })?;
    let (width, height) = reader
        .into_dimensions()
        .map_err(|error| CoreError::InvalidGeometry(format!("cannot read dimensions: {error}")))?;
    if u64::from(width).saturating_mul(u64::from(height)) > max_decode_pixels {
        return Err(CoreError::InvalidGeometry(format!(
            "image has {width}x{height} pixels, exceeding configured limit {max_decode_pixels}"
        )));
    }
    let bytes = std::fs::read(path)
        .map_err(|error| CoreError::InvalidGeometry(format!("cannot read image: {error}")))?;
    let decoded = image::load_from_memory(&bytes)
        .map_err(|error| CoreError::InvalidGeometry(format!("cannot decode image: {error}")))?
        .to_rgb8();
    let mime_type = match image::guess_format(&bytes) {
        Ok(ImageFormat::Jpeg) => "image/jpeg",
        Ok(ImageFormat::Png) => "image/png",
        _ => "application/octet-stream",
    };
    Ok(ImageFrame {
        metadata: ImageMetadata {
            width: decoded.width(),
            height: decoded.height(),
            mime_type: mime_type.to_owned(),
            sha256: sha256(&bytes),
        },
        rgb: decoded.into_raw(),
    })
}

#[must_use]
pub fn sha256(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

pub fn thumbnail(frame: &ImageFrame, max_dimension: u32) -> CoreResult<ImageFrame> {
    frame.validate()?;
    if max_dimension == 0 {
        return Err(CoreError::InvalidGeometry(
            "thumbnail dimension must be non-zero".to_owned(),
        ));
    }
    let image = rgb_image(frame)?;
    let resized = if image.width().max(image.height()) <= max_dimension {
        image
    } else {
        DynamicImage::ImageRgb8(image)
            .thumbnail(max_dimension, max_dimension)
            .to_rgb8()
    };
    Ok(ImageFrame {
        metadata: ImageMetadata {
            width: resized.width(),
            height: resized.height(),
            mime_type: "image/png".to_owned(),
            sha256: frame.metadata.sha256.clone(),
        },
        rgb: resized.into_raw(),
    })
}

pub fn to_model_image(
    id: impl Into<String>,
    frame: &ImageFrame,
    max_dimension: u32,
) -> CoreResult<ModelImage> {
    let frame = thumbnail(frame, max_dimension)?;
    let png = encode_png(&frame)?;
    Ok(ModelImage {
        id: id.into(),
        mime_type: "image/png".to_owned(),
        data_base64: STANDARD.encode(png),
    })
}

/// Decode the exact bytes carried by a `ModelImage`. This is used by authenticated local preview
/// endpoints after reconstructing and digest-checking a persisted model-input trace.
pub fn model_image_bytes(image: &ModelImage) -> CoreResult<Vec<u8>> {
    STANDARD.decode(&image.data_base64).map_err(|error| {
        CoreError::Validation(format!("ModelImage payload is not valid base64: {error}"))
    })
}

pub fn resize_exact(frame: &ImageFrame, width: u32, height: u32) -> CoreResult<ImageFrame> {
    frame.validate()?;
    if width == 0 || height == 0 {
        return Err(CoreError::InvalidGeometry(
            "resize dimensions must be non-zero".to_owned(),
        ));
    }
    let resized = imageops::resize(&rgb_image(frame)?, width, height, FilterType::CatmullRom);
    let rgb = resized.into_raw();
    Ok(ImageFrame {
        metadata: ImageMetadata {
            width,
            height,
            mime_type: "image/png".to_owned(),
            sha256: sha256(&rgb),
        },
        rgb,
    })
}

/// Materializes a model request from decoded root pixels. Virtual Crop/Resize metadata is not
/// trusted as payload evidence: this function crops the root frame, performs the requested resize,
/// encodes the exact outbound PNG and returns a redacted trace over those bytes.
#[allow(clippy::too_many_arguments)]
pub fn materialize_model_input(
    id: impl Into<String>,
    source_image_id: ImageId,
    source_image_artifact: Option<ArtifactRef>,
    source: &ImageFrame,
    source_region: Option<NormalizedRect>,
    submitted_width: u32,
    submitted_height: u32,
) -> CoreResult<(ModelImage, ModelInputTrace)> {
    source.validate()?;
    if submitted_width == 0 || submitted_height == 0 {
        return Err(CoreError::InvalidGeometry(
            "submitted model dimensions must be non-zero".to_owned(),
        ));
    }
    let region = source_region.unwrap_or(NormalizedRect::new(0.0, 0.0, 1.0, 1.0)?);
    let source_region_pixels = pixel_region(region, source.metadata.width, source.metadata.height)?;
    let crop = crop(source, region, 0.0)?;
    let crop_dimensions = [crop.metadata.width, crop.metadata.height];
    let (submitted, interpolation) =
        if crop.metadata.width == submitted_width && crop.metadata.height == submitted_height {
            (crop, "none")
        } else {
            (
                resize_exact(&crop, submitted_width, submitted_height)?,
                "catmull_rom",
            )
        };
    let encoded = encode_png(&submitted)?;
    let submitted_image_sha256 = sha256(&encoded);
    let normalized_pixel_digest = sha256(&submitted.rgb);
    let coordinate_frame_id = format!(
        "sha256:{}",
        sha256(
            format!(
                "{}:{source_region_pixels:?}:{submitted_width}:{submitted_height}",
                source.metadata.sha256
            )
            .as_bytes()
        )
    );
    let transform_to_original = CoordinateTransform::stretch(
        coordinate_frame_id,
        region,
        submitted_width,
        submitted_height,
    )?;
    let image = ModelImage {
        id: id.into(),
        mime_type: "image/png".to_owned(),
        data_base64: STANDARD.encode(encoded),
    };
    let trace = ModelInputTrace {
        source_image_id,
        source_image_sha256: source.metadata.sha256.clone(),
        source_image_artifact,
        source_region_pixels,
        crop_dimensions,
        submitted_dimensions: [submitted_width, submitted_height],
        submitted_image_sha256,
        normalized_pixel_digest,
        interpolation: interpolation.to_owned(),
        letterbox_padding: [0; 4],
        color_format: "rgb8".to_owned(),
        transform_to_original,
        provider_image_parameters: std::collections::BTreeMap::new(),
        provider_effective_dimensions: ProviderEffectiveDimensions::Unknown,
    };
    Ok((image, trace))
}

pub fn pixel_region(
    rect: NormalizedRect,
    image_width: u32,
    image_height: u32,
) -> CoreResult<[u32; 4]> {
    if image_width == 0 || image_height == 0 {
        return Err(CoreError::InvalidGeometry(
            "pixel region requires non-zero image dimensions".to_owned(),
        ));
    }
    let x = normalized_to_pixel(rect.x(), image_width);
    let y = normalized_to_pixel(rect.y(), image_height);
    let right = normalized_to_pixel(rect.x() + rect.width(), image_width)
        .max(x + 1)
        .min(image_width);
    let bottom = normalized_to_pixel(rect.y() + rect.height(), image_height)
        .max(y + 1)
        .min(image_height);
    Ok([x, y, right - x, bottom - y])
}

pub fn encode_png(frame: &ImageFrame) -> CoreResult<Vec<u8>> {
    let image = DynamicImage::ImageRgb8(rgb_image(frame)?);
    let mut output = Cursor::new(Vec::new());
    image
        .write_to(&mut output, ImageFormat::Png)
        .map_err(|error| CoreError::InvalidGeometry(format!("cannot encode PNG: {error}")))?;
    Ok(output.into_inner())
}

pub fn crop(frame: &ImageFrame, rect: NormalizedRect, padding: f32) -> CoreResult<ImageFrame> {
    frame.validate()?;
    if !padding.is_finite() || !(0.0..=2.0).contains(&padding) {
        return Err(CoreError::InvalidGeometry(
            "crop padding must be finite and within [0, 2]".to_owned(),
        ));
    }
    let width = frame.metadata.width;
    let height = frame.metadata.height;
    let pad_x = rect.width() * padding;
    let pad_y = rect.height() * padding;
    let left = (rect.x() - pad_x).max(0.0);
    let top = (rect.y() - pad_y).max(0.0);
    let right = (rect.x() + rect.width() + pad_x).min(1.0);
    let bottom = (rect.y() + rect.height() + pad_y).min(1.0);
    let x = normalized_to_pixel(left, width);
    let y = normalized_to_pixel(top, height);
    let right_px = normalized_to_pixel(right, width).max(x + 1).min(width);
    let bottom_px = normalized_to_pixel(bottom, height).max(y + 1).min(height);
    let cropped =
        imageops::crop_imm(&rgb_image(frame)?, x, y, right_px - x, bottom_px - y).to_image();
    Ok(ImageFrame {
        metadata: ImageMetadata {
            width: cropped.width(),
            height: cropped.height(),
            mime_type: "image/png".to_owned(),
            sha256: format!(
                "{}:{x}:{y}:{}:{}",
                frame.metadata.sha256,
                right_px - x,
                bottom_px - y
            ),
        },
        rgb: cropped.into_raw(),
    })
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ColorStatistics {
    pub mean_brightness: f32,
    pub white_ratio: f32,
    pub red_ratio: f32,
    pub blue_ratio: f32,
}

pub fn color_statistics(frame: &ImageFrame, rect: NormalizedRect) -> CoreResult<ColorStatistics> {
    let crop = crop(frame, rect, 0.0)?;
    let mut brightness_sum = 0.0;
    let mut white = 0_u64;
    let mut red = 0_u64;
    let mut blue = 0_u64;
    let mut count = 0_u64;
    for pixel in crop.rgb.chunks_exact(3) {
        let r = f32::from(pixel[0]) / 255.0;
        let g = f32::from(pixel[1]) / 255.0;
        let b = f32::from(pixel[2]) / 255.0;
        let max = r.max(g).max(b);
        let min = r.min(g).min(b);
        let saturation = if max <= f32::EPSILON {
            0.0
        } else {
            (max - min) / max
        };
        brightness_sum += max;
        white += u64::from(max >= 0.72 && saturation <= 0.25);
        red += u64::from(r >= 0.45 && r > g * 1.25 && r > b * 1.2);
        blue += u64::from(b >= 0.35 && b > r * 1.2 && b > g * 1.05);
        count += 1;
    }
    if count == 0 {
        return Err(CoreError::InvalidGeometry("empty color region".to_owned()));
    }
    let denominator = count as f32;
    Ok(ColorStatistics {
        mean_brightness: brightness_sum / denominator,
        white_ratio: white as f32 / denominator,
        red_ratio: red as f32 / denominator,
        blue_ratio: blue as f32 / denominator,
    })
}

#[must_use]
pub fn white_response(frame: &ImageFrame, pixel_x: i32, pixel_y: i32) -> f32 {
    let Ok(pixel_x) = u32::try_from(pixel_x) else {
        return 0.0;
    };
    let Ok(pixel_y) = u32::try_from(pixel_y) else {
        return 0.0;
    };
    if pixel_x >= frame.metadata.width || pixel_y >= frame.metadata.height {
        return 0.0;
    }
    let Some(index) = usize::try_from(
        (u64::from(pixel_y) * u64::from(frame.metadata.width) + u64::from(pixel_x)) * 3,
    )
    .ok() else {
        return 0.0;
    };
    let Some(pixel) = frame.rgb.get(index..index + 3) else {
        return 0.0;
    };
    let red = f32::from(pixel[0]) / 255.0;
    let green = f32::from(pixel[1]) / 255.0;
    let blue = f32::from(pixel[2]) / 255.0;
    let max = red.max(green).max(blue);
    let min = red.min(green).min(blue);
    let saturation = if max <= f32::EPSILON {
        0.0
    } else {
        (max - min) / max
    };
    (max * (1.0 - saturation)).clamp(0.0, 1.0)
}

#[must_use]
pub fn point_in_polygon(point: NormalizedPoint, ring: &[NormalizedPoint]) -> bool {
    if ring.len() < 3 {
        return false;
    }
    let mut inside = false;
    let mut previous = ring[ring.len() - 1];
    for &current in ring {
        let crosses = (current.y() > point.y()) != (previous.y() > point.y())
            && point.x()
                < (previous.x() - current.x()) * (point.y() - current.y())
                    / (previous.y() - current.y())
                    + current.x();
        if crosses {
            inside = !inside;
        }
        previous = current;
    }
    inside
}

#[must_use]
pub fn point_segment_distance(
    point: NormalizedPoint,
    start: NormalizedPoint,
    end: NormalizedPoint,
) -> f32 {
    let dx = end.x() - start.x();
    let dy = end.y() - start.y();
    let length_squared = dx * dx + dy * dy;
    if length_squared <= f32::EPSILON {
        return ((point.x() - start.x()).powi(2) + (point.y() - start.y()).powi(2)).sqrt();
    }
    let projection = ((point.x() - start.x()) * dx + (point.y() - start.y()) * dy) / length_squared;
    let projection = projection.clamp(0.0, 1.0);
    let nearest_x = start.x() + projection * dx;
    let nearest_y = start.y() + projection * dy;
    ((point.x() - nearest_x).powi(2) + (point.y() - nearest_y).powi(2)).sqrt()
}

#[must_use]
pub fn simplify_polyline(points: &[NormalizedPoint], epsilon: f32) -> Vec<NormalizedPoint> {
    if points.len() <= 2 {
        return points.to_vec();
    }
    let first = points[0];
    let last = points[points.len() - 1];
    let (index, distance) = points[1..points.len() - 1]
        .iter()
        .enumerate()
        .map(|(index, point)| (index + 1, point_segment_distance(*point, first, last)))
        .max_by(|left, right| left.1.total_cmp(&right.1))
        .unwrap_or((0, 0.0));
    if distance <= epsilon {
        return vec![first, last];
    }
    let mut left = simplify_polyline(&points[..=index], epsilon);
    let right = simplify_polyline(&points[index..], epsilon);
    left.pop();
    left.extend(right);
    left
}

pub fn generate_synthetic_robocup(path: &Path) -> CoreResult<()> {
    let mut image: RgbImage = ImageBuffer::from_pixel(640, 400, Rgb([28, 126, 58]));
    // Field lines and penalty mark.
    for y in 194..=206 {
        for x in 24..616 {
            image.put_pixel(x, y, Rgb([242, 242, 235]));
        }
    }
    for y in 80..320 {
        for x in 315..=325 {
            image.put_pixel(x, y, Rgb([242, 242, 235]));
        }
    }
    for y in 273..=281 {
        for x in 492..=500 {
            image.put_pixel(x, y, Rgb([245, 245, 238]));
        }
    }
    // Simplified red and blue robots with white shoes.
    draw_robot(&mut image, 145, 178, Rgb([205, 42, 48]));
    draw_robot(&mut image, 435, 142, Rgb([38, 72, 210]));
    // Ball: white disc-like square with dark patches.
    for y in 300..324 {
        for x in 350..374 {
            let dx = i32::try_from(x).unwrap_or_default() - 362;
            let dy = i32::try_from(y).unwrap_or_default() - 312;
            if dx * dx + dy * dy <= 144 {
                let color = if (x + y) % 11 < 3 { 45 } else { 236 };
                image.put_pixel(x, y, Rgb([color, color, color]));
            }
        }
    }
    image
        .save(path)
        .map_err(|error| CoreError::InvalidGeometry(format!("cannot save fixture: {error}")))
}

/// Deterministic small-object regression evidence for localization recovery.
///
/// The rectangles are generator-owned observations rather than model predictions. The deliberately
/// bad coarse box sits below the target and reproduces the failure mode where a prompted segmenter
/// can only refine background inside an invalid prompt.
#[derive(Debug, Clone, PartialEq)]
pub struct SmallObjectLocalizationFixture {
    pub width: u32,
    pub height: u32,
    pub ground_truth: NormalizedRect,
    pub coarse_detection: NormalizedRect,
    pub legacy_refined_detection: NormalizedRect,
}

impl SmallObjectLocalizationFixture {
    /// Reproduces the old full-image-fraction box-prompt expansion.
    pub fn legacy_prompt(&self, padding: f32) -> CoreResult<NormalizedRect> {
        if !padding.is_finite() || !(0.0..=0.5).contains(&padding) {
            return Err(CoreError::InvalidGeometry(
                "fixture prompt padding must be finite and within [0,0.5]".to_owned(),
            ));
        }
        let left = (self.coarse_detection.x() - padding).max(0.0);
        let top = (self.coarse_detection.y() - padding).max(0.0);
        let right = (self.coarse_detection.x() + self.coarse_detection.width() + padding).min(1.0);
        let bottom =
            (self.coarse_detection.y() + self.coarse_detection.height() + padding).min(1.0);
        NormalizedRect::new(left, top, right - left, bottom - top)
    }
}

/// Writes a non-square image with a known 16 px ball, white field-line and white-shoe distractors.
/// The returned coordinates are the authoritative fixture truth used by recovery tests.
pub fn generate_small_object_localization_fixture(
    path: &Path,
) -> CoreResult<SmallObjectLocalizationFixture> {
    const WIDTH: u32 = 544;
    const HEIGHT: u32 = 448;
    const BALL_LEFT: u32 = 272;
    const BALL_TOP: u32 = 194;
    const BALL_SIZE: u32 = 16;

    let mut image: RgbImage = ImageBuffer::from_pixel(WIDTH, HEIGHT, Rgb([27, 128, 59]));

    // A bright field line and shoe-like rectangles deliberately resemble the tiny white target.
    for y in 286..=292 {
        for x in 20..524 {
            image.put_pixel(x, y, Rgb([242, 242, 235]));
        }
    }
    for y in 218..=232 {
        for x in 252..=269 {
            image.put_pixel(x, y, Rgb([245, 245, 240]));
        }
    }
    for y in 209..=231 {
        for x in 248..=253 {
            image.put_pixel(x, y, Rgb([52, 54, 58]));
        }
    }

    // The target lies above and does not overlap the deliberately wrong coarse prediction.
    let radius = i32::try_from(BALL_SIZE / 2).unwrap_or_default();
    for y in BALL_TOP..BALL_TOP + BALL_SIZE {
        for x in BALL_LEFT..BALL_LEFT + BALL_SIZE {
            let dx = i32::try_from(x - BALL_LEFT).unwrap_or_default() - radius;
            let dy = i32::try_from(y - BALL_TOP).unwrap_or_default() - radius;
            if dx * dx + dy * dy <= radius * radius {
                let value = if (x + y) % 7 < 2 { 48 } else { 238 };
                image.put_pixel(x, y, Rgb([value, value, value]));
            }
        }
    }
    image
        .save(path)
        .map_err(|error| CoreError::InvalidGeometry(format!("cannot save fixture: {error}")))?;

    Ok(SmallObjectLocalizationFixture {
        width: WIDTH,
        height: HEIGHT,
        ground_truth: NormalizedRect::new(
            BALL_LEFT as f32 / WIDTH as f32,
            BALL_TOP as f32 / HEIGHT as f32,
            BALL_SIZE as f32 / WIDTH as f32,
            BALL_SIZE as f32 / HEIGHT as f32,
        )?,
        coarse_detection: NormalizedRect::new(
            262.0 / WIDTH as f32,
            225.0 / HEIGHT as f32,
            19.0 / WIDTH as f32,
            16.0 / HEIGHT as f32,
        )?,
        legacy_refined_detection: NormalizedRect::new(
            255.0 / WIDTH as f32,
            216.0 / HEIGHT as f32,
            34.0 / WIDTH as f32,
            42.0 / HEIGHT as f32,
        )?,
    })
}

pub fn generate_synthetic_inspection(path: &Path) -> CoreResult<()> {
    let mut image: RgbImage = ImageBuffer::from_pixel(160, 100, Rgb([24, 28, 34]));
    for y in 28..72 {
        for x in 48..112 {
            image.put_pixel(x, y, Rgb([224, 190, 72]));
        }
    }
    image
        .save(path)
        .map_err(|error| CoreError::InvalidGeometry(format!("cannot save fixture: {error}")))
}

fn draw_robot(image: &mut RgbImage, x: u32, y: u32, torso: Rgb<u8>) {
    for row in y..y + 72 {
        for column in x..x + 42 {
            let color = if row < y + 44 {
                torso
            } else {
                Rgb([48, 48, 52])
            };
            image.put_pixel(column, row, color);
        }
    }
    for row in y + 68..y + 80 {
        for column in x.saturating_sub(5)..x + 18 {
            image.put_pixel(column, row, Rgb([245, 245, 240]));
        }
    }
}

fn rgb_image(frame: &ImageFrame) -> CoreResult<RgbImage> {
    frame.validate()?;
    RgbImage::from_raw(
        frame.metadata.width,
        frame.metadata.height,
        frame.rgb.clone(),
    )
    .ok_or_else(|| CoreError::InvalidGeometry("invalid RGB frame storage".to_owned()))
}

fn normalized_to_pixel(value: f32, dimension: u32) -> u32 {
    (value * dimension as f32).floor() as u32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn synthetic_image_is_bounded_and_decodable() {
        let temporary = tempfile::tempdir().expect("temporary directory");
        let path = temporary.path().join("demo.png");
        generate_synthetic_robocup(&path).expect("generate image");
        let frame = load_image(&path, 1_000_000).expect("load generated image");
        assert_eq!((frame.metadata.width, frame.metadata.height), (640, 400));
        assert!(load_image(&path, 10).is_err());
    }

    #[test]
    fn small_object_fixture_reproduces_missed_prompt_and_refiner_drift() {
        let temporary = tempfile::tempdir().expect("temporary directory");
        let path = temporary.path().join("small-ball-regression.png");
        let fixture = generate_small_object_localization_fixture(&path).expect("generate fixture");
        let frame = load_image(&path, 1_000_000).expect("load generated fixture");
        assert_eq!(
            (frame.metadata.width, frame.metadata.height),
            (fixture.width, fixture.height)
        );

        assert!(
            fixture
                .coarse_detection
                .intersection_area(fixture.ground_truth)
                <= f32::EPSILON,
            "the regression requires a genuine coarse-localization miss"
        );
        assert!(
            fixture
                .legacy_prompt(0.02)
                .expect("legacy prompt")
                .intersection_area(fixture.ground_truth)
                <= f32::EPSILON,
            "full-image padding must not make this invalid prompt look usable"
        );
        assert!(
            fixture
                .legacy_refined_detection
                .intersection_area(fixture.ground_truth)
                <= f32::EPSILON,
            "the historical refinement follows the distractor rather than the ball"
        );
        let area_ratio = fixture.legacy_refined_detection.area() / fixture.coarse_detection.area();
        assert!(
            (area_ratio - 4.697_368_6).abs() < 0.000_01,
            "unexpected refiner-drift ratio: {area_ratio}"
        );
    }

    #[test]
    fn model_thumbnail_never_upscales_below_the_maximum_dimension() {
        let frame = ImageFrame {
            metadata: ImageMetadata {
                width: 8,
                height: 4,
                mime_type: "image/png".to_owned(),
                sha256: "fixture".to_owned(),
            },
            rgb: vec![64; 8 * 4 * 3],
        };
        let thumbnail = thumbnail(&frame, 32).expect("thumbnail");
        assert_eq!(
            (thumbnail.metadata.width, thumbnail.metadata.height),
            (8, 4)
        );
        let encoded = to_model_image("fixture", &frame, 32).expect("ModelImage");
        let bytes = STANDARD.decode(encoded.data_base64).expect("base64");
        let decoded = image::load_from_memory(&bytes).expect("PNG");
        assert_eq!((decoded.width(), decoded.height()), (8, 4));
    }

    #[test]
    fn materialized_model_input_contains_real_resized_pixels_and_transform() {
        let mut rgb = Vec::new();
        for y in 0..48_u8 {
            for x in 0..96_u8 {
                rgb.extend([x, y, x.wrapping_add(y)]);
            }
        }
        let frame = ImageFrame {
            metadata: ImageMetadata {
                width: 96,
                height: 48,
                mime_type: "image/png".to_owned(),
                sha256: "source-sha".to_owned(),
            },
            rgb,
        };
        let image_id = ImageId::new();
        let (model_image, trace) =
            materialize_model_input("4x", image_id, None, &frame, None, 384, 192)
                .expect("materialized model input");
        let bytes = STANDARD.decode(model_image.data_base64).expect("base64");
        let decoded = image::load_from_memory(&bytes).expect("outbound PNG");
        assert_eq!((decoded.width(), decoded.height()), (384, 192));
        assert_eq!(trace.crop_dimensions, [96, 48]);
        assert_eq!(trace.submitted_dimensions, [384, 192]);
        assert_eq!(trace.interpolation, "catmull_rom");
        assert_eq!(trace.source_region_pixels, [0, 0, 96, 48]);
        assert_eq!(trace.submitted_image_sha256, sha256(&bytes));
        assert_eq!(
            trace.normalized_pixel_digest,
            sha256(decoded.to_rgb8().as_raw())
        );
        let local = NormalizedRect::new(0.25, 0.25, 0.5, 0.5).expect("local rect");
        assert_eq!(
            trace
                .transform_to_original
                .project_rect(local)
                .expect("root rect"),
            local
        );
    }

    #[test]
    fn edge_crop_pixel_region_matches_materialized_payload() {
        let frame = ImageFrame {
            metadata: ImageMetadata {
                width: 11,
                height: 7,
                mime_type: "image/png".to_owned(),
                sha256: "edge-source".to_owned(),
            },
            rgb: vec![32; 11 * 7 * 3],
        };
        let region = NormalizedRect::new(0.82, 0.72, 0.18, 0.28).expect("edge region");
        let (_, trace) =
            materialize_model_input("edge", ImageId::new(), None, &frame, Some(region), 8, 8)
                .expect("edge materialization");
        assert_eq!(trace.source_region_pixels, [9, 5, 2, 2]);
        assert_eq!(trace.crop_dimensions, [2, 2]);
        assert_eq!(trace.submitted_dimensions, [8, 8]);
    }

    #[test]
    fn polygon_and_simplification_are_deterministic() {
        let points = [
            NormalizedPoint::new(0.0, 0.0).expect("point"),
            NormalizedPoint::new(0.5, 0.01).expect("point"),
            NormalizedPoint::new(1.0, 0.0).expect("point"),
        ];
        assert_eq!(simplify_polyline(&points, 0.02).len(), 2);
        let ring = [
            NormalizedPoint::new(0.0, 0.0).expect("point"),
            NormalizedPoint::new(1.0, 0.0).expect("point"),
            NormalizedPoint::new(1.0, 1.0).expect("point"),
            NormalizedPoint::new(0.0, 1.0).expect("point"),
        ];
        assert!(point_in_polygon(
            NormalizedPoint::new(0.5, 0.5).expect("point"),
            &ring
        ));
    }

    #[test]
    fn decode_limit_is_checked_before_full_image_decode() {
        let temporary = tempfile::tempdir().expect("temporary directory");
        let path = temporary.path().join("bounded.png");
        let image = RgbImage::from_pixel(11, 10, Rgb([0, 0, 0]));
        image.save(&path).expect("fixture image");
        let error = load_image(&path, 100).expect_err("110 pixels must exceed limit");
        assert!(error.to_string().contains("exceeding configured limit 100"));
    }
}
