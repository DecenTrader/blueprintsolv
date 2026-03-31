# Data Model: Blueprint Image to 3D Model

**Branch**: `001-blueprint-to-3d` | **Date**: 2026-03-30
**Source**: spec.md Key Entities + plan.md Technical Context

---

## Core Types

### `LengthUnit`

```rust
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum LengthUnit {
    Feet,
    Meters,
}
```

**Constraint**: User specifies unit when providing the scale reference (FR-002).

---

### `ImagePoint`

```rust
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct ImagePoint {
    pub x: u32,   // pixel column (0 = left edge)
    pub y: u32,   // pixel row (0 = top edge)
}
```

**Constraint**: Must be within image bounds `(0..width, 0..height)`.
Validated on input (FR-003).

---

### `WorldPoint`

```rust
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct WorldPoint {
    pub x: f64,   // horizontal distance in real-world units
    pub y: f64,   // vertical distance in real-world units (Y-axis = depth in plan)
}
```

**Derived from**: `ImagePoint` + `ScaleReference`.

---

## Entity: BlueprintImage

Spec entity: **BlueprintImage**

```rust
#[derive(Debug, Serialize, Deserialize)]
pub struct BlueprintImage {
    pub path: PathBuf,
    pub width: u32,
    pub height: u32,
    pub format: ImageFormat,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum ImageFormat {
    Jpeg,
    Png,
}
```

**Constraints**:
- `format` MUST be `Jpeg` or `Png` (FR-001); other formats rejected with error.
- `width` and `height` should be ≤ 4000 px each for the SC-001 performance target.
- Raw pixel data (`DynamicImage`) is NOT stored in the session file — only `path` is
  persisted. Image is reloaded from `path` on session resume (FR-017).

---

## Entity: ScaleReference

Spec entity: **ScaleReference**

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScaleReference {
    pub point_a: ImagePoint,
    pub point_b: ImagePoint,
    pub real_world_distance: f64,     // must be > 0
    pub unit: LengthUnit,
    pub pixels_per_unit: f64,         // derived: pixel_distance / real_world_distance
}
```

**Validation rules** (FR-003):
- `point_a != point_b` (no identical points)
- `real_world_distance > 0.0` (no zero or negative distance)
- Both points must be within image bounds
- `pixels_per_unit` is always derived — never set directly

**OCR validation**: After OCR extracts dimension values, `pixels_per_unit` is cross-checked.
If OCR-derived ratio differs from user-provided ratio by > 5%, a warning is shown (FR-022).

---

## Entity: LineSegment

Spec entity: **LineSegment**

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LineSegment {
    pub id: Uuid,
    pub points: Vec<ImagePoint>,     // ordered sequence of pixels along the segment
    pub length_pixels: f64,          // total length in pixels
    pub real_world_length: f64,      // derived from ScaleReference
    pub wall_spacing: Option<f64>,   // pixel gap to a parallel companion segment
                                     // (present when double-line wall detected)
}
```

**Derived fields**: `length_pixels` and `real_world_length` computed after scale is set.
`wall_spacing` computed during line classification (FR-009 wall thickness detection).

---

## Entity: ArchitecturalElement

Spec entity: **ArchitecturalElement**

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArchitecturalElement {
    pub id: Uuid,
    pub element_type: ElementType,
    pub bounds: BoundingBox,           // axis-aligned bounding box in world coords
    pub source_segment_ids: Vec<Uuid>, // line segments this element was derived from
    pub confidence: f32,               // classification confidence [0.0, 1.0]
    pub is_interior: Option<bool>,     // None = not yet inferred
    pub wall_thickness_m: Option<f64>, // Some(_) only for Wall elements; in meters
}

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
pub struct BoundingBox {
    pub min: WorldPoint,
    pub max: WorldPoint,
}
```

**Wall thickness logic** (FR-009):
- If `wall_spacing` is present on source `LineSegment`: `wall_thickness_m = wall_spacing *
  (1.0 / pixels_per_unit)`
- Otherwise: `wall_thickness_m = 0.1524` (6 inches / 0.1524 m default)

**Unclassified elements**: Elements where `confidence < adaptive_threshold` are surfaced
for user review (FR-007). On skip (FR-008): `element_type = ElementType::Unclassified`,
excluded from 3D generation.

---

## Entity: TextAnnotation

Spec entity: **TextAnnotation**

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TextAnnotation {
    pub id: Uuid,
    pub raw_text: String,
    pub annotation_type: TextAnnotationType,
    pub image_bounds: ImageBoundingBox,     // pixel bounding box of detected text region
    pub confidence: f32,                    // OCR confidence score [0.0, 1.0]
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum TextAnnotationType {
    RoomLabel(KnownRoomType),
    RoomLabelUnknown,              // text recognized but not in known-room-type list
    DimensionValue {
        value: f64,
        unit: LengthUnit,
    },
    Unreadable,                    // OCR confidence below threshold; included in summary
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum KnownRoomType {
    Bedroom,
    Kitchen,
    Bathroom,
    LivingRoom,
    DiningRoom,
    Garage,
    Hallway,
    Study,
    Laundry,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImageBoundingBox {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}
```

**OCR failure handling** (FR-023): Text regions with `confidence` below threshold have
`annotation_type = TextAnnotationType::Unreadable`. These are silently skipped during
processing and included in the end-of-processing summary (FR-015).

**Room label scope** (FR-021): Room labels annotate 2D `FloorPlan` regions only.
They are **not** carried into `Model3D` or exported files.

---

## Entity: FloorPlan

Spec entity: **FloorPlan**

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FloorPlan {
    pub elements: Vec<ArchitecturalElement>,
    pub rooms: Vec<Room>,
    pub text_annotations: Vec<TextAnnotation>,
    pub scale: ScaleReference,
    pub bounds: BoundingBox,              // overall floor plan footprint in world coords
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Room {
    pub id: Uuid,
    pub room_type: Option<KnownRoomType>,
    pub raw_label: Option<String>,         // for RoomLabelUnknown annotations
    pub boundary_element_ids: Vec<Uuid>,   // wall ArchitecturalElement IDs
    pub is_interior: bool,
    pub annotation_ids: Vec<Uuid>,         // TextAnnotation IDs for this room
}
```

**Interior/exterior inference** (FR-006): Rooms are classified `is_interior = true` when
enclosed within the building footprint defined by the wall elements. Exterior regions are
marked `is_interior = false`.

---

## Entity: Model3D

Spec entity: **Model3D**

```rust
#[derive(Debug, Clone)]
pub struct Model3D {
    pub meshes: Vec<Mesh>,
    pub materials: Vec<Material>,
    pub wall_height_m: f64,    // user-confirmed height (default 2.44 m / 8 ft)
}

#[derive(Debug, Clone)]
pub struct Mesh {
    pub name: String,
    pub element_type: ElementType,
    pub vertices: Vec<[f64; 3]>,   // [x, y, z] — Y-up (standard OBJ convention)
                                   // z = vertical (up); x/y = floor plane
    pub faces: Vec<[u32; 3]>,      // triangle indices; CCW winding from outside
    pub material_name: String,
}

#[derive(Debug, Clone)]
pub struct Material {
    pub name: String,
    pub diffuse_rgb: [f32; 3],     // [R, G, B] in [0.0, 1.0]
}
```

**Coordinate convention**: Y-up (standard Wavefront OBJ spec). SketchUp's OBJ importer
handles Y→Z axis swap on import. See `contracts/export-format.md`.

**Element type → color mapping** (FR-012):

| Element Type | Material Name     | Diffuse RGB          |
|--------------|-------------------|----------------------|
| Wall         | `mat_wall`        | [0.75, 0.75, 0.75]   |
| Door         | `mat_door`        | [0.60, 0.40, 0.20]   |
| Window       | `mat_window`      | [0.40, 0.70, 0.90]   |
| SlidingDoor  | `mat_sliding_door`| [0.55, 0.35, 0.18]   |
| Fireplace    | `mat_fireplace`   | [0.80, 0.20, 0.10]   |
| Closet       | `mat_closet`      | [0.90, 0.85, 0.70]   |
| Staircase    | `mat_staircase`   | [0.80, 0.72, 0.50]   |
| Chimney      | `mat_chimney`     | [0.35, 0.30, 0.28]   |
| Courtyard    | `mat_courtyard`   | [0.40, 0.70, 0.35]   |

**`Model3D` is NOT serialized to session files** — it is always re-generated from `FloorPlan`
on demand. Only `FloorPlan` is persisted.

---

## Entity: SessionFile

Spec entity: **SessionFile**  
Format: JSON (`.b2m` extension)

```rust
#[derive(Debug, Serialize, Deserialize)]
pub struct Session {
    pub version: String,                         // "1.0"
    pub created_at: String,                      // ISO 8601 UTC timestamp
    pub last_saved_at: String,                   // ISO 8601 UTC timestamp
    pub image: BlueprintImage,
    pub scale: Option<ScaleReference>,           // None = not yet set
    pub line_segments: Vec<LineSegment>,
    pub elements: Vec<ArchitecturalElement>,
    pub floor_plan: Option<FloorPlan>,           // None = not yet generated
    pub text_annotations: Vec<TextAnnotation>,
    pub pending_clarifications: Vec<PendingClarification>,
    pub wall_height_m: Option<f64>,              // None = not yet confirmed by user
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PendingClarification {
    pub element_id: Uuid,
    pub suggested_types: Vec<ElementType>,
    pub context_snippet: String,   // human-readable description for UI display
}
```

**Schema evolution**: New optional fields use `#[serde(default)]`. Session version `"1.0"`
is stored to enable migration logic in future versions.

---

## Entity: CorrectionHistory

Spec entity: **CorrectionHistory**  
Storage: `~/.blueprint2mod/corrections.json` (global, persists across all sessions)

```rust
#[derive(Debug, Serialize, Deserialize)]
pub struct CorrectionHistory {
    pub version: String,                           // "1.0"
    pub adaptive_threshold: f32,                   // current threshold [0.0, 1.0]
    pub total_corrections: u32,
    pub last_updated: String,                      // ISO 8601 UTC
    pub corrections: Vec<CorrectionEntry>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CorrectionEntry {
    pub timestamp: String,                         // ISO 8601 UTC
    pub original_type: ElementType,
    pub corrected_type: ElementType,
    pub original_confidence: f32,
}
```

**Threshold adaptation** (FR-007): After each correction, `adaptive_threshold` is
recomputed as a running average of `original_confidence` values for corrected entries,
shifted toward reducing unnecessary prompts for the user's classification patterns.

---

## State Transitions

The application follows a linear state machine with save/load entry points:

```
Welcome ──► ImageLoaded ──► Scaled ──► Analyzed ──► Clarifying ──► ModelReady ──► Exported
               │                │         │              │              │
               │                │         │              └──────────────► (back to Clarifying)
               │                └─────────────────────────────────────► (saved → SessionFile)
               └────────────────────────────────────────────────────► (loaded ← SessionFile)
```

- **Welcome**: No image loaded. CLI arg or file picker opens image.
- **ImageLoaded**: Image displayed. User prompted to set scale reference.
- **Scaled**: Scale reference confirmed. Analysis can begin.
- **Analyzed**: Line tracing + classification + OCR complete.
- **Clarifying**: Pending clarifications being resolved one at a time.
- **ModelReady**: All clarifications resolved. 3D generation can begin.
- **Exported**: OBJ or STL file written. Summary displayed.

Save is available from `Scaled` onward. Load returns to whichever state the session
was saved from.
