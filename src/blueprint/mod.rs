use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum LengthUnit {
    Feet,
    Meters,
}

impl LengthUnit {
    /// Multiply a value in this unit by this factor to convert it to meters.
    /// Returns `0.3048` for `Feet` (1 ft = 0.3048 m) and `1.0` for `Meters`.
    pub fn to_meters_factor(self) -> f64 {
        match self {
            LengthUnit::Feet => 0.3048,
            LengthUnit::Meters => 1.0,
        }
    }
}

/// A point in image (pixel) space. Origin is top-left; x increases right, y increases down.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct ImagePoint {
    pub x: u32,
    pub y: u32,
}

/// A point in real-world space, in the units specified by the session's `ScaleReference`.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct WorldPoint {
    pub x: f64,
    pub y: f64,
}

/// Axis-aligned bounding box in world coordinates.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct BoundingBox {
    pub min: WorldPoint,
    pub max: WorldPoint,
}

impl BoundingBox {
    pub fn width(&self) -> f64 {
        self.max.x - self.min.x
    }

    pub fn height(&self) -> f64 {
        self.max.y - self.min.y
    }
}

/// Axis-aligned bounding box in image (pixel) coordinates.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct ImageBoundingBox {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}

/// Optional axis-aligned crop region applied to the blueprint image before any processing (FR-024).
/// All downstream pipeline stages (line tracing, OCR, classification, 3D generation) operate
/// on the sub-image bounded by this region.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct CropRegion {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}

pub mod element;
pub mod floor_plan;
pub mod image;
pub mod scale;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bounding_box_dimensions() {
        let bb = BoundingBox {
            min: WorldPoint { x: 1.0, y: 2.0 },
            max: WorldPoint { x: 4.0, y: 7.0 },
        };
        assert!((bb.width() - 3.0).abs() < f64::EPSILON);
        assert!((bb.height() - 5.0).abs() < f64::EPSILON);
    }
}
