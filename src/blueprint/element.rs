use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::BoundingBox;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ElementType {
    Wall,
    Door,
    Window,
    SlidingDoor,
    Fireplace,
    Closet,
    Staircase,
    Chimney,
    Courtyard,
    Unclassified,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArchitecturalElement {
    pub id: Uuid,
    pub element_type: ElementType,
    /// Axis-aligned bounding box in world coordinates.
    pub bounds: BoundingBox,
    /// IDs of `LineSegment`s this element was derived from.
    pub source_segment_ids: Vec<Uuid>,
    /// Classification confidence in [0.0, 1.0].
    pub confidence: f32,
    /// `None` = not yet inferred; `Some(true)` = inside building footprint.
    pub is_interior: Option<bool>,
    /// Present only for `Wall` elements; derived from double-line spacing or default 0.1524 m.
    pub wall_thickness_m: Option<f64>,
}
