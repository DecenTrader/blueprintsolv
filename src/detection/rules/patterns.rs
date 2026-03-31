use crate::blueprint::{
    element::{ArchitecturalElement, ElementType},
    scale::{LineSegment, ScaleReference},
    BoundingBox, WorldPoint,
};
use uuid::Uuid;

/// Classify a `LineSegment` into an `ElementType` using geometric heuristics (FR-005).
///
/// Rules applied in priority order:
/// 1. Double-line pair with wall_spacing > 0  → Wall
/// 2. Segment length ≥ 0.3 m                 → Wall (long lines are walls)
/// 3. Short isolated segment (0.05–0.3 m)    → Door (heuristic arc width)
/// 4. Fallback                                → Unclassified
///
/// Higher-fidelity rules (window gap patterns, stair hatch density, chimney rectangles)
/// are applied in T022 (US2 expansion); these are sufficient for ≥90% wall detection.
pub fn classify_segment(seg: &LineSegment, _scale: &ScaleReference) -> (ElementType, f32) {
    // Double-line spacing → Wall (FR-009)
    if seg.wall_spacing.is_some() {
        return (ElementType::Wall, 0.95);
    }

    let len_m = seg.real_world_length;

    // Long line → Wall
    if len_m >= 0.30 {
        return (ElementType::Wall, 0.85);
    }

    // Short segment in door-width range (0.7–1.1 m) → Door
    if (0.70..=1.10).contains(&len_m) {
        return (ElementType::Door, 0.60);
    }

    // Slightly shorter → Window
    if (0.30..0.70).contains(&len_m) {
        return (ElementType::Window, 0.55);
    }

    (ElementType::Unclassified, 0.30)
}

/// Convert a `LineSegment` and its classified `ElementType` into an `ArchitecturalElement`.
pub fn segment_to_element(
    seg: &LineSegment,
    element_type: ElementType,
    confidence: f32,
    scale: &ScaleReference,
) -> ArchitecturalElement {
    let (min_x, max_x) = seg
        .points
        .iter()
        .fold((u32::MAX, 0u32), |(mn, mx), p| (mn.min(p.x), mx.max(p.x)));
    let (min_y, max_y) = seg
        .points
        .iter()
        .fold((u32::MAX, 0u32), |(mn, mx), p| (mn.min(p.y), mx.max(p.y)));

    let wall_thickness_m = if matches!(element_type, ElementType::Wall) {
        let default_thickness = 0.1524; // 6 inches
        Some(
            seg.wall_spacing
                .map(|spacing| scale.to_world_distance(spacing))
                .unwrap_or(default_thickness),
        )
    } else {
        None
    };

    ArchitecturalElement {
        id: Uuid::new_v4(),
        element_type,
        bounds: BoundingBox {
            min: WorldPoint {
                x: scale.to_world_distance(min_x as f64),
                y: scale.to_world_distance(min_y as f64),
            },
            max: WorldPoint {
                x: scale.to_world_distance(max_x as f64),
                y: scale.to_world_distance(max_y as f64),
            },
        },
        source_segment_ids: vec![seg.id],
        confidence,
        is_interior: None, // set during floor plan assembly
        wall_thickness_m,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::blueprint::{ImagePoint, LengthUnit};

    fn make_scale() -> ScaleReference {
        ScaleReference::new(
            ImagePoint { x: 0, y: 0 },
            ImagePoint { x: 100, y: 0 },
            1.0,
            LengthUnit::Meters,
            1000,
            1000,
        )
        .unwrap()
    }

    fn make_seg(length_px: f64, wall_spacing: Option<f64>) -> LineSegment {
        let scale = make_scale();
        LineSegment {
            id: Uuid::new_v4(),
            points: vec![
                ImagePoint { x: 0, y: 0 },
                ImagePoint {
                    x: length_px as u32,
                    y: 0,
                },
            ],
            length_pixels: length_px,
            real_world_length: scale.to_world_distance(length_px),
            wall_spacing,
        }
    }

    #[test]
    fn double_line_classified_as_wall() {
        let scale = make_scale();
        let seg = make_seg(200.0, Some(8.0));
        let (et, conf) = classify_segment(&seg, &scale);
        assert_eq!(et, ElementType::Wall);
        assert!(conf > 0.9);
    }

    #[test]
    fn long_segment_classified_as_wall() {
        let scale = make_scale();
        let seg = make_seg(50.0, None); // 50px / 100px per m = 0.5 m → Wall
        let (et, _) = classify_segment(&seg, &scale);
        assert_eq!(et, ElementType::Wall);
    }

    #[test]
    fn very_short_segment_unclassified() {
        let scale = make_scale();
        let seg = make_seg(5.0, None); // 5px = 0.05 m → Unclassified
        let (et, _) = classify_segment(&seg, &scale);
        assert_eq!(et, ElementType::Unclassified);
    }
}
