# Contract: Session File Format

**Feature**: Blueprint Image to 3D Model
**Date**: 2026-03-30
**Extension**: `.b2m`
**Encoding**: UTF-8 JSON

---

## Purpose

A `.b2m` session file captures all processing state at the point of save, allowing the
user to resume work from exactly that point (FR-016, FR-017). It does NOT store raw
pixel data — only the image file path, derived data, and classification results.

## Top-Level Schema

```json
{
  "version": "1.0",
  "created_at": "2026-03-30T10:00:00Z",
  "last_saved_at": "2026-03-30T11:30:00Z",
  "image": { ... },
  "scale": null,
  "line_segments": [],
  "elements": [],
  "floor_plan": null,
  "text_annotations": [],
  "pending_clarifications": [],
  "wall_height_m": null
}
```

### `version`
String. Always `"1.0"` for this release. Future schema changes increment this.

### `created_at` / `last_saved_at`
ISO 8601 UTC strings (e.g., `"2026-03-30T10:00:00Z"`).

### `image`

```json
{
  "path": "/Users/alice/blueprints/floor_plan.jpg",
  "width": 3200,
  "height": 2400,
  "format": "Jpeg"
}
```

- `format`: `"Jpeg"` or `"Png"` (matches `ImageFormat` enum)
- `path`: absolute path at time of session creation. If image is moved, user must
  re-locate it manually.

### `scale`

`null` if the user has not yet completed the scale step. Otherwise:

```json
{
  "point_a": { "x": 120, "y": 340 },
  "point_b": { "x": 440, "y": 340 },
  "real_world_distance": 3.66,
  "unit": "Meters",
  "pixels_per_unit": 87.43
}
```

- `unit`: `"Feet"` or `"Meters"`
- `pixels_per_unit`: derived value stored for convenience; recomputed on load as a
  validation check.

### `line_segments`

Array of detected line segments. Empty until analysis is run.

```json
[
  {
    "id": "550e8400-e29b-41d4-a716-446655440000",
    "points": [{"x": 100, "y": 200}, {"x": 105, "y": 200}, ...],
    "length_pixels": 142.3,
    "real_world_length": 1.628,
    "wall_spacing": 12.5
  }
]
```

- `wall_spacing`: `null` for single-line segments; pixel gap value for double-line walls.

### `elements`

Array of classified architectural elements.

```json
[
  {
    "id": "660e8400-e29b-41d4-a716-446655440001",
    "element_type": "Wall",
    "bounds": {
      "min": { "x": 0.0, "y": 0.0 },
      "max": { "x": 3.66, "y": 0.15 }
    },
    "source_segment_ids": ["550e8400-e29b-41d4-a716-446655440000"],
    "confidence": 0.95,
    "is_interior": true,
    "wall_thickness_m": 0.15
  }
]
```

- `element_type`: one of `"Wall"`, `"Door"`, `"Window"`, `"SlidingDoor"`,
  `"Fireplace"`, `"Closet"`, `"Staircase"`, `"Chimney"`, `"Courtyard"`,
  `"Unclassified"`.
- `wall_thickness_m`: present only for `"Wall"` elements; `null` for all others.

### `text_annotations`

```json
[
  {
    "id": "770e8400-e29b-41d4-a716-446655440002",
    "raw_text": "Kitchen",
    "annotation_type": { "RoomLabel": "Kitchen" },
    "image_bounds": { "x": 500, "y": 300, "width": 80, "height": 20 },
    "confidence": 0.91
  },
  {
    "id": "880e8400-e29b-41d4-a716-446655440003",
    "raw_text": "12'-6\"",
    "annotation_type": { "DimensionValue": { "value": 3.81, "unit": "Meters" } },
    "image_bounds": { "x": 200, "y": 100, "width": 60, "height": 15 },
    "confidence": 0.88
  },
  {
    "id": "990e8400-e29b-41d4-a716-446655440004",
    "raw_text": "",
    "annotation_type": "Unreadable",
    "image_bounds": { "x": 750, "y": 600, "width": 45, "height": 18 },
    "confidence": 0.12
  }
]
```

- `annotation_type` variants: `{ "RoomLabel": "<KnownRoomType>" }`,
  `"RoomLabelUnknown"`, `{ "DimensionValue": { "value": f64, "unit": "Feet"|"Meters" } }`,
  `"Unreadable"`.
- `KnownRoomType` values: `"Bedroom"`, `"Kitchen"`, `"Bathroom"`, `"LivingRoom"`,
  `"DiningRoom"`, `"Garage"`, `"Hallway"`, `"Study"`, `"Laundry"`.

### `floor_plan`

`null` until interior/exterior inference is complete. When present, contains the full
`FloorPlan` structure including `elements`, `rooms`, and `text_annotations` references.

### `pending_clarifications`

Array of elements awaiting manual user classification (FR-007). Empty when all
clarifications are resolved.

```json
[
  {
    "element_id": "660e8400-e29b-41d4-a716-446655440001",
    "suggested_types": ["Door", "Window"],
    "context_snippet": "Gap in south wall, 0.9m wide"
  }
]
```

### `wall_height_m`

`null` until user confirms at the 3D generation step. Default shown to user: `2.44`
(8 ft). Stored value is in meters regardless of user's chosen unit.

---

## Schema Evolution

- New optional fields are added with `#[serde(default)]` in Rust; older session files
  that omit the field deserialize to the default value.
- The `version` field is used for any migration logic required for breaking changes.
- Removing or renaming fields requires a version bump and a migration function.

---

## File Naming Convention

No enforced convention. Suggested: `<project-name>.b2m` (e.g., `my_house.b2m`).
Multiple sessions can exist for the same image using different `.b2m` files.
