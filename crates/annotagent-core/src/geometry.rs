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
