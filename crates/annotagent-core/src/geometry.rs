//! Checked normalized geometry.

use serde::{Deserialize, Deserializer, Serialize, Serializer, de};

use crate::{CoreError, CoreResult};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct NormalizedPoint {
    x: f32,
    y: f32,
}

impl NormalizedPoint {
    pub fn new(x: f32, y: f32) -> CoreResult<Self> {
        validate_unit(x, "x")?;
        validate_unit(y, "y")?;
        Ok(Self { x, y })
    }

    #[must_use]
    pub const fn x(self) -> f32 {
        self.x
    }

    #[must_use]
    pub const fn y(self) -> f32 {
        self.y
    }

    #[must_use]
    pub fn to_pixel(self, width: u32, height: u32) -> (f32, f32) {
        (self.x * width as f32, self.y * height as f32)
    }

    pub fn from_pixel(x: f32, y: f32, width: u32, height: u32) -> CoreResult<Self> {
        if width == 0 || height == 0 {
            return Err(CoreError::InvalidGeometry(
                "image dimensions must be non-zero".to_owned(),
            ));
        }
        Self::new(x / width as f32, y / height as f32)
    }
}

impl Serialize for NormalizedPoint {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        [self.x, self.y].serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for NormalizedPoint {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let [x, y] = <[f32; 2]>::deserialize(deserializer)?;
        Self::new(x, y).map_err(de::Error::custom)
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct NormalizedRect {
    x: f32,
    y: f32,
    width: f32,
    height: f32,
}

impl NormalizedRect {
    pub fn new(x: f32, y: f32, width: f32, height: f32) -> CoreResult<Self> {
        validate_unit(x, "x")?;
        validate_unit(y, "y")?;
        validate_positive_unit(width, "width")?;
        validate_positive_unit(height, "height")?;
        if x + width > 1.0 + f32::EPSILON || y + height > 1.0 + f32::EPSILON {
            return Err(CoreError::InvalidGeometry(
                "rectangle must remain inside normalized image bounds".to_owned(),
            ));
        }
        Ok(Self {
            x,
            y,
            width,
            height,
        })
    }

    #[must_use]
    pub const fn x(self) -> f32 {
        self.x
    }

    #[must_use]
    pub const fn y(self) -> f32 {
        self.y
    }

    #[must_use]
    pub const fn width(self) -> f32 {
        self.width
    }

    #[must_use]
    pub const fn height(self) -> f32 {
        self.height
    }

    #[must_use]
    pub fn center(self) -> NormalizedPoint {
        // Safe because a checked rectangle is inside the unit square.
        NormalizedPoint {
            x: self.x + self.width / 2.0,
            y: self.y + self.height / 2.0,
        }
    }

    #[must_use]
    pub fn area(self) -> f32 {
        self.width * self.height
    }

    #[must_use]
    pub fn intersection_area(self, other: Self) -> f32 {
        let left = self.x.max(other.x);
        let top = self.y.max(other.y);
        let right = (self.x + self.width).min(other.x + other.width);
        let bottom = (self.y + self.height).min(other.y + other.height);
        (right - left).max(0.0) * (bottom - top).max(0.0)
    }

    #[must_use]
    pub fn contains(self, point: NormalizedPoint, tolerance: f32) -> bool {
        point.x >= self.x - tolerance
            && point.y >= self.y - tolerance
            && point.x <= self.x + self.width + tolerance
            && point.y <= self.y + self.height + tolerance
    }
}

impl Serialize for NormalizedRect {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        [self.x, self.y, self.width, self.height].serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for NormalizedRect {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let [x, y, width, height] = <[f32; 4]>::deserialize(deserializer)?;
        Self::new(x, y, width, height).map_err(de::Error::custom)
    }
}

/// One reversible mapping from a model-facing raster back to the normalized root image.
/// `content_region` describes the non-padding area inside the submitted raster; it is the full
/// unit rectangle for ordinary stretch/resize and a smaller rectangle for letterbox input.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CoordinateTransform {
    pub coordinate_frame_id: String,
    pub source_region: NormalizedRect,
    pub submitted_width: u32,
    pub submitted_height: u32,
    pub content_region: NormalizedRect,
}

impl CoordinateTransform {
    pub fn stretch(
        coordinate_frame_id: impl Into<String>,
        source_region: NormalizedRect,
        submitted_width: u32,
        submitted_height: u32,
    ) -> CoreResult<Self> {
        if submitted_width == 0 || submitted_height == 0 {
            return Err(CoreError::InvalidGeometry(
                "submitted image dimensions must be non-zero".to_owned(),
            ));
        }
        Ok(Self {
            coordinate_frame_id: coordinate_frame_id.into(),
            source_region,
            submitted_width,
            submitted_height,
            content_region: NormalizedRect::new(0.0, 0.0, 1.0, 1.0)?,
        })
    }

    pub fn letterbox(
        coordinate_frame_id: impl Into<String>,
        source_region: NormalizedRect,
        submitted_width: u32,
        submitted_height: u32,
        padding: [u32; 4],
    ) -> CoreResult<Self> {
        if submitted_width == 0 || submitted_height == 0 {
            return Err(CoreError::InvalidGeometry(
                "submitted image dimensions must be non-zero".to_owned(),
            ));
        }
        let [left, top, right, bottom] = padding;
        let content_width = submitted_width.saturating_sub(left.saturating_add(right));
        let content_height = submitted_height.saturating_sub(top.saturating_add(bottom));
        if content_width == 0 || content_height == 0 {
            return Err(CoreError::InvalidGeometry(
                "letterbox padding leaves no image content".to_owned(),
            ));
        }
        Ok(Self {
            coordinate_frame_id: coordinate_frame_id.into(),
            source_region,
            submitted_width,
            submitted_height,
            content_region: NormalizedRect::new(
                left as f32 / submitted_width as f32,
                top as f32 / submitted_height as f32,
                content_width as f32 / submitted_width as f32,
                content_height as f32 / submitted_height as f32,
            )?,
        })
    }

    /// Projects one model-space normalized rectangle into root-image normalized coordinates.
    /// Floating-point geometry is retained; callers rasterize only at an output boundary.
    pub fn project_rect(&self, rect: NormalizedRect) -> CoreResult<NormalizedRect> {
        let content = self.content_region;
        let left = rect.x().max(content.x());
        let top = rect.y().max(content.y());
        let right = (rect.x() + rect.width()).min(content.x() + content.width());
        let bottom = (rect.y() + rect.height()).min(content.y() + content.height());
        if right <= left || bottom <= top {
            return Err(CoreError::InvalidGeometry(
                "model rectangle lies entirely inside letterbox padding".to_owned(),
            ));
        }
        let local_x = (left - content.x()) / content.width();
        let local_y = (top - content.y()) / content.height();
        let local_width = (right - left) / content.width();
        let local_height = (bottom - top) / content.height();
        NormalizedRect::new(
            self.source_region.x() + local_x * self.source_region.width(),
            self.source_region.y() + local_y * self.source_region.height(),
            local_width * self.source_region.width(),
            local_height * self.source_region.height(),
        )
    }
}

fn validate_unit(value: f32, name: &str) -> CoreResult<()> {
    if !value.is_finite() || !(0.0..=1.0).contains(&value) {
        return Err(CoreError::InvalidGeometry(format!(
            "{name} must be finite and within [0, 1], got {value}"
        )));
    }
    Ok(())
}

fn validate_positive_unit(value: f32, name: &str) -> CoreResult<()> {
    validate_unit(value, name)?;
    if value <= 0.0 {
        return Err(CoreError::InvalidGeometry(format!(
            "{name} must be greater than zero"
        )));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;

    #[test]
    fn rejects_non_finite_points() {
        assert!(NormalizedPoint::new(f32::NAN, 0.5).is_err());
        assert!(NormalizedPoint::new(0.5, f32::INFINITY).is_err());
    }

    #[test]
    fn rectangle_must_stay_in_bounds() {
        assert!(NormalizedRect::new(0.8, 0.2, 0.3, 0.2).is_err());
        assert!(NormalizedRect::new(0.1, 0.2, 0.3, 0.4).is_ok());
    }

    #[test]
    fn crop_resize_and_letterbox_project_through_one_transform() {
        let source = NormalizedRect::new(0.2, 0.1, 0.5, 0.6).expect("source region");
        let stretched = CoordinateTransform::stretch("stretch", source, 400, 200)
            .expect("non-uniform resize transform");
        let projected = stretched
            .project_rect(NormalizedRect::new(0.25, 0.5, 0.5, 0.25).expect("model rect"))
            .expect("projected rect");
        assert!((projected.x() - 0.325).abs() < 1e-6);
        assert!((projected.y() - 0.4).abs() < 1e-6);
        assert!((projected.width() - 0.25).abs() < 1e-6);
        assert!((projected.height() - 0.15).abs() < 1e-6);

        let letterboxed =
            CoordinateTransform::letterbox("letterbox", source, 400, 400, [0, 100, 0, 100])
                .expect("letterbox transform");
        let projected = letterboxed
            .project_rect(NormalizedRect::new(0.25, 0.375, 0.5, 0.125).expect("model rect"))
            .expect("projected letterbox rect");
        assert!((projected.x() - 0.325).abs() < 1e-6);
        assert!((projected.y() - 0.25).abs() < 1e-6);
        assert!((projected.width() - 0.25).abs() < 1e-6);
        assert!((projected.height() - 0.15).abs() < 1e-6);
        assert!(
            letterboxed
                .project_rect(NormalizedRect::new(0.1, 0.01, 0.2, 0.1).expect("padding-only rect"))
                .is_err()
        );
    }

    proptest! {
        #[test]
        fn pixel_round_trip(x in 0.0_f32..=1.0, y in 0.0_f32..=1.0) {
            let point = NormalizedPoint::new(x, y).expect("generated point is valid");
            let (px, py) = point.to_pixel(1920, 1080);
            let round_trip = NormalizedPoint::from_pixel(px, py, 1920, 1080)
                .expect("round trip is valid");
            prop_assert!((round_trip.x() - x).abs() < 1e-5);
            prop_assert!((round_trip.y() - y).abs() < 1e-5);
        }
    }
}
