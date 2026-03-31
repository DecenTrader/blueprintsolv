use anyhow::{bail, Result};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::{ImagePoint, LengthUnit};

/// User-defined scale reference: two points on the image and the real-world distance between them.
///
/// `pixels_per_unit` is always derived — never set directly (FR-002, FR-003).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScaleReference {
    pub point_a: ImagePoint,
    pub point_b: ImagePoint,
    /// Must be > 0.
    pub real_world_distance: f64,
    pub unit: LengthUnit,
    /// Derived: `pixel_distance / real_world_distance`.
    pub pixels_per_unit: f64,
}

impl ScaleReference {
    /// Construct and validate a scale reference.
    ///
    /// Validation rules (FR-003):
    /// - `point_a != point_b`
    /// - `real_world_distance > 0.0`
    /// - Both points within image bounds `(img_width, img_height)`
    pub fn new(
        point_a: ImagePoint,
        point_b: ImagePoint,
        real_world_distance: f64,
        unit: LengthUnit,
        img_width: u32,
        img_height: u32,
    ) -> Result<Self> {
        if point_a == point_b {
            bail!("Scale reference points must be distinct (FR-003)");
        }
        if real_world_distance <= 0.0 {
            bail!(
                "Real-world distance must be greater than zero, got {} (FR-003)",
                real_world_distance
            );
        }
        if point_a.x >= img_width || point_a.y >= img_height {
            bail!(
                "point_a ({},{}) is outside image bounds {}×{} (FR-003)",
                point_a.x,
                point_a.y,
                img_width,
                img_height
            );
        }
        if point_b.x >= img_width || point_b.y >= img_height {
            bail!(
                "point_b ({},{}) is outside image bounds {}×{} (FR-003)",
                point_b.x,
                point_b.y,
                img_width,
                img_height
            );
        }
        let dx = point_b.x as f64 - point_a.x as f64;
        let dy = point_b.y as f64 - point_a.y as f64;
        let pixel_distance = (dx * dx + dy * dy).sqrt();
        let pixels_per_unit = pixel_distance / real_world_distance;
        Ok(Self {
            point_a,
            point_b,
            real_world_distance,
            unit,
            pixels_per_unit,
        })
    }

    /// Convert a pixel distance to real-world distance in this scale's unit.
    pub fn to_world_distance(&self, pixels: f64) -> f64 {
        pixels / self.pixels_per_unit
    }

    /// Convert a real-world distance to pixels.
    pub fn to_pixel_distance(&self, world: f64) -> f64 {
        world * self.pixels_per_unit
    }

    /// Validate the user-provided scale against an OCR-derived pixels_per_unit (FR-022).
    ///
    /// Returns `Some(divergence_pct)` if the two scales diverge by more than 5%;
    /// returns `None` if the scales are within tolerance or the OCR value is unavailable.
    pub fn validate_against_ocr(&self, ocr_pixels_per_unit: f64) -> Option<f64> {
        if ocr_pixels_per_unit <= 0.0 {
            return None;
        }
        let divergence =
            ((self.pixels_per_unit - ocr_pixels_per_unit) / self.pixels_per_unit).abs();
        if divergence > 0.05 {
            Some(divergence * 100.0)
        } else {
            None
        }
    }
}

/// A traced contour segment from the line detection pipeline (FR-004).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LineSegment {
    pub id: Uuid,
    /// Ordered sequence of pixels along the segment.
    pub points: Vec<ImagePoint>,
    /// Total length in pixels.
    pub length_pixels: f64,
    /// Derived from `ScaleReference` (set after scale is confirmed).
    pub real_world_length: f64,
    /// Pixel gap to a parallel companion segment — present for double-line walls (FR-009).
    pub wall_spacing: Option<f64>,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pt(x: u32, y: u32) -> ImagePoint {
        ImagePoint { x, y }
    }

    #[test]
    fn rejects_identical_points() {
        let result =
            ScaleReference::new(pt(10, 10), pt(10, 10), 3.66, LengthUnit::Meters, 800, 600);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("distinct"));
    }

    #[test]
    fn rejects_zero_distance() {
        let result =
            ScaleReference::new(pt(10, 10), pt(100, 10), 0.0, LengthUnit::Meters, 800, 600);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("greater than zero"));
    }

    #[test]
    fn rejects_negative_distance() {
        let result =
            ScaleReference::new(pt(10, 10), pt(100, 10), -1.0, LengthUnit::Meters, 800, 600);
        assert!(result.is_err());
    }

    #[test]
    fn rejects_out_of_bounds_point_a() {
        let result =
            ScaleReference::new(pt(900, 10), pt(100, 10), 3.0, LengthUnit::Meters, 800, 600);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("outside image bounds"));
    }

    #[test]
    fn rejects_out_of_bounds_point_b() {
        let result =
            ScaleReference::new(pt(10, 10), pt(100, 700), 3.0, LengthUnit::Meters, 800, 600);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("outside image bounds"));
    }

    #[test]
    fn correct_pixels_per_unit() {
        // 320 px apart, 3.66 m real-world → pixels_per_unit ≈ 87.43
        let scale = ScaleReference::new(
            pt(100, 300),
            pt(420, 300),
            3.66,
            LengthUnit::Meters,
            800,
            600,
        )
        .unwrap();
        let expected = 320.0 / 3.66;
        assert!(
            (scale.pixels_per_unit - expected).abs() < 0.01,
            "pixels_per_unit = {:.4}, expected {:.4}",
            scale.pixels_per_unit,
            expected
        );
    }

    #[test]
    fn to_world_distance_roundtrip() {
        let scale = ScaleReference::new(
            pt(100, 300),
            pt(420, 300),
            3.66,
            LengthUnit::Meters,
            800,
            600,
        )
        .unwrap();
        let world = scale.to_world_distance(320.0);
        assert!((world - 3.66).abs() < 0.001);
    }

    #[test]
    fn sc004_within_5pct_tolerance() {
        // SC-004: dimension tolerance ±5%
        let scale = ScaleReference::new(
            pt(100, 300),
            pt(420, 300),
            3.66,
            LengthUnit::Meters,
            800,
            600,
        )
        .unwrap();
        let expected_ppu = 87.43;
        let tolerance = expected_ppu * 0.05;
        assert!(
            (scale.pixels_per_unit - expected_ppu).abs() <= tolerance,
            "pixels_per_unit {:.4} not within ±5% of {:.4}",
            scale.pixels_per_unit,
            expected_ppu
        );
    }
}
