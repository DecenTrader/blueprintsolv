# Contract: Export Format (OBJ/STL)

**Feature**: Blueprint Image to 3D Model
**Date**: 2026-03-30

---

## OBJ Format

### File Structure

An OBJ export produces two files in the same directory:
- `<name>.obj` — geometry (vertices, faces, material references)
- `<name>.mtl` — material definitions (one per element type)

### Coordinate System

**Convention used**: Y-up (standard Wavefront OBJ spec).

```
Y = up (vertical)
X = right (east in floor plan)
Z = depth (south in floor plan)
```

SketchUp's OBJ importer (2021+ native, or SimLab extension for 2020) presents an
"Import Units" dialog and handles the Y→Z axis swap automatically. Users should select
"Meters" when prompted.

**All coordinates are in meters.** Scale factor: 1 OBJ unit = 1 meter.

### Face Winding

All faces use **counter-clockwise (CCW) vertex order** when viewed from outside the
solid. This is the Wavefront OBJ standard and is respected by SketchUp:
- Top face of an extruded wall: CCW when viewed from above (+Y direction)
- Bottom face: CW when viewed from above (CCW from below)
- Side faces: CCW when viewed from outside

### Face Triangulation

All faces are triangulated (no quads or n-gons). This prevents non-planar polygon
failures in SketchUp.

### Material File

The `.mtl` file MUST be in the same directory as the `.obj` file. The `mtllib` directive
in the OBJ references it by filename only (no directory path).

```
# example.obj
mtllib example.mtl
```

One material is defined per architectural element type. See `data-model.md` for the
complete material name → RGB color mapping.

```
# example.mtl
newmtl mat_wall
Kd 0.75 0.75 0.75
Ka 0.0 0.0 0.0
Ks 0.0 0.0 0.0
d 1.0
illum 1
```

Material properties: diffuse color (`Kd`) only. No textures, no specular highlights.
`d 1.0` = fully opaque. `illum 1` = diffuse shading without specular.

### Geometry Grouping

Each architectural element is exported as a named group:

```
g wall_550e8400
usemtl mat_wall
v ...
f ...
```

Group names use format: `<element_type_snake_case>_<first_8_chars_of_uuid>`.

### OBJ File Example (minimal wall)

```
# blueprint2mod export — blueprint2mod v1.0.0
# Units: meters | Coordinate system: Y-up
mtllib output.mtl

g wall_550e8400
usemtl mat_wall
v 0.00 0.00 0.00
v 3.66 0.00 0.00
v 3.66 2.44 0.00
v 0.00 2.44 0.00
v 0.00 0.00 0.15
v 3.66 0.00 0.15
v 3.66 2.44 0.15
v 0.00 2.44 0.15
# ... triangulated faces (CCW) ...
f 1 2 3
f 1 3 4
```

---

## STL Format

### Binary STL

The `stl_io` crate writes **binary STL** (not ASCII). Binary STL is more compact and
is universally supported including SketchUp.

### Coordinate System

Same Y-up convention as OBJ. SketchUp's STL importer also handles axis conversion.

### Units

STL has no unit specification. The export writes values in meters. SketchUp prompts for
units on STL import; user should select "Meters."

### Materials / Colors

STL does not support materials or colors. The exported file is a single mesh with no
color information. If color-coded visualization is required, use OBJ format instead.

### Triangle Winding

All triangles use CCW winding (outward normals), consistent with OBJ output.

---

## SketchUp 2020+ Compatibility Checklist

| Requirement | OBJ | STL |
|-------------|-----|-----|
| Native import (no extension needed) | SketchUp 2021+ | SketchUp 2020+ |
| Extension required for SketchUp 2020 | OBJ Importer by SimLab | None |
| Units dialog on import | Yes — select "Meters" | Yes — select "Meters" |
| Axis swap handled by importer | Yes (Y→Z) | Yes (Y→Z) |
| Color/material support | Yes (via MTL) | No |
| Face count | Per element (grouped) | Single unified mesh |

---

## Known Limitations

- No texture map support in v1 (FR-012 specifies diffuse color only).
- No roof geometry generated — the 3D model represents walls and element volumes only.
- Courtyard elements are exported as recessed floor areas (lowered plane), not as
  enclosed vertical geometry.
- Multi-story buildings are out of scope for v1 (see Assumptions in spec.md).
