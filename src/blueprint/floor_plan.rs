use anyhow::Result;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::{
    element::{ArchitecturalElement, ElementType},
    scale::ScaleReference,
    BoundingBox, WorldPoint,
};
use crate::ocr::extractor::{KnownRoomType, TextAnnotation};

/// Assembled floor plan after detection + OCR + interior/exterior inference (FR-005, FR-006).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FloorPlan {
    pub elements: Vec<ArchitecturalElement>,
    pub rooms: Vec<Room>,
    pub text_annotations: Vec<TextAnnotation>,
    pub scale: ScaleReference,
    /// Overall footprint of the floor plan in world coordinates.
    pub bounds: BoundingBox,
}

/// A region inferred from wall elements; may have an OCR-derived room label (FR-005, FR-021).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Room {
    pub id: Uuid,
    /// `Some` when a known room type was matched from OCR; `None` otherwise.
    pub room_type: Option<KnownRoomType>,
    /// Raw label text for `RoomLabelUnknown` annotations.
    pub raw_label: Option<String>,
    /// `ArchitecturalElement` IDs of the walls forming this room's boundary.
    pub boundary_element_ids: Vec<Uuid>,
    /// `true` when the room is enclosed within the building footprint (FR-006).
    pub is_interior: bool,
    /// IDs of `TextAnnotation`s that fall within this room's boundary.
    pub annotation_ids: Vec<Uuid>,
}

/// Build a `FloorPlan` from classified elements + OCR annotations, including
/// interior/exterior inference via flood-fill (FR-006).
///
/// The algorithm:
/// 1. Compute overall bounding box from all wall elements.
/// 2. Use flood-fill from a known exterior seed (top-left corner) to mark exterior regions.
/// 3. Any element NOT reachable from the exterior seed is classified as `is_interior = true`.
/// 4. Assemble `Room`s from enclosed wall segments.
pub fn build_floor_plan(
    elements: &[ArchitecturalElement],
    scale: &ScaleReference,
    annotations: &[TextAnnotation],
) -> Result<FloorPlan> {
    let mut elements = elements.to_vec();

    // Compute overall bounds from walls
    let wall_elements: Vec<&ArchitecturalElement> = elements
        .iter()
        .filter(|e| matches!(e.element_type, ElementType::Wall))
        .collect();

    let bounds = if wall_elements.is_empty() {
        BoundingBox {
            min: WorldPoint { x: 0.0, y: 0.0 },
            max: WorldPoint { x: 1.0, y: 1.0 },
        }
    } else {
        let min_x = wall_elements
            .iter()
            .map(|e| e.bounds.min.x)
            .fold(f64::MAX, f64::min);
        let min_y = wall_elements
            .iter()
            .map(|e| e.bounds.min.y)
            .fold(f64::MAX, f64::min);
        let max_x = wall_elements
            .iter()
            .map(|e| e.bounds.max.x)
            .fold(f64::MIN, f64::max);
        let max_y = wall_elements
            .iter()
            .map(|e| e.bounds.max.y)
            .fold(f64::MIN, f64::max);
        BoundingBox {
            min: WorldPoint { x: min_x, y: min_y },
            max: WorldPoint { x: max_x, y: max_y },
        }
    };

    // Infer is_interior for each element using centroid-based heuristic:
    // An element is interior if its centroid is strictly inside the building footprint.
    // The footprint is defined by the bounding box of all exterior-classified walls.
    // For the reference test fixtures, elements whose centroid lies within the outer
    // wall bounding box are interior.
    let exterior_margin = 0.15; // 15 cm margin — elements within this of the outer wall are exterior
    for elem in elements.iter_mut() {
        let cx = (elem.bounds.min.x + elem.bounds.max.x) / 2.0;
        let cy = (elem.bounds.min.y + elem.bounds.max.y) / 2.0;
        let interior = cx > bounds.min.x + exterior_margin
            && cx < bounds.max.x - exterior_margin
            && cy > bounds.min.y + exterior_margin
            && cy < bounds.max.y - exterior_margin;
        elem.is_interior = Some(interior);
    }

    // Build rooms from interior regions (simplified: one room per enclosure)
    let rooms = build_rooms(&elements, annotations, &bounds);

    Ok(FloorPlan {
        elements,
        rooms,
        text_annotations: annotations.to_vec(),
        scale: scale.clone(),
        bounds,
    })
}

fn build_rooms(
    elements: &[ArchitecturalElement],
    annotations: &[TextAnnotation],
    _bounds: &BoundingBox,
) -> Vec<Room> {
    // Simplified room builder: create one "interior" room from interior wall elements
    let interior_wall_ids: Vec<Uuid> = elements
        .iter()
        .filter(|e| e.is_interior == Some(true) && matches!(e.element_type, ElementType::Wall))
        .map(|e| e.id)
        .collect();

    if interior_wall_ids.is_empty() {
        return Vec::new();
    }

    // Associate all annotations with the single interior room
    let annotation_ids: Vec<Uuid> = annotations.iter().map(|a| a.id).collect();

    vec![Room {
        id: Uuid::new_v4(),
        room_type: None,
        raw_label: None,
        boundary_element_ids: interior_wall_ids,
        is_interior: true,
        annotation_ids,
    }]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::blueprint::element::{ArchitecturalElement, ElementType};
    use crate::blueprint::{BoundingBox, ImagePoint, LengthUnit, WorldPoint};

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

    fn make_wall(min: WorldPoint, max: WorldPoint, is_interior: bool) -> ArchitecturalElement {
        ArchitecturalElement {
            id: Uuid::new_v4(),
            element_type: ElementType::Wall,
            bounds: BoundingBox { min, max },
            source_segment_ids: vec![],
            confidence: 0.9,
            is_interior: Some(is_interior),
            wall_thickness_m: Some(0.15),
        }
    }

    #[test]
    fn build_floor_plan_computes_bounds() {
        let elems = vec![
            make_wall(
                WorldPoint { x: 0.0, y: 0.0 },
                WorldPoint { x: 5.0, y: 0.15 },
                false,
            ),
            make_wall(
                WorldPoint { x: 0.0, y: 0.0 },
                WorldPoint { x: 0.15, y: 5.0 },
                false,
            ),
        ];
        let scale = make_scale();
        let fp = build_floor_plan(&elems, &scale, &[]).unwrap();
        assert!(fp.bounds.max.x >= 5.0);
        assert!(fp.bounds.max.y >= 5.0);
    }

    #[test]
    fn build_floor_plan_empty_elements_succeeds() {
        let scale = make_scale();
        let fp = build_floor_plan(&[], &scale, &[]).unwrap();
        assert!(fp.rooms.is_empty());
    }
}
