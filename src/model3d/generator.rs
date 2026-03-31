use crate::blueprint::element::ElementType;
use crate::blueprint::floor_plan::FloorPlan;
use rayon::prelude::*;

/// A single triangle defined by three vertices (Y-up, CCW winding from outside).
#[derive(Debug, Clone)]
pub struct Triangle {
    pub vertices: [[f32; 3]; 3],
    /// Outward-facing surface normal.
    pub normal: [f32; 3],
}

/// A named mesh group for one architectural element (FR-009, FR-010).
#[derive(Debug, Clone)]
pub struct Mesh {
    pub group_name: String,
    pub material_name: String,
    pub triangles: Vec<Triangle>,
}

/// The complete 3D model derived from a FloorPlan.
#[derive(Debug, Clone)]
pub struct Model3D {
    pub meshes: Vec<Mesh>,
    /// Wall extrusion height in meters.
    pub wall_height_m: f64,
}

/// Material color table per FR-012. Indexed by `ElementType`.
pub fn material_diffuse_rgb(element_type: &ElementType) -> [f32; 3] {
    match element_type {
        ElementType::Wall => [0.75, 0.75, 0.75],
        ElementType::Door => [0.60, 0.40, 0.20],
        ElementType::Window => [0.40, 0.70, 0.90],
        ElementType::SlidingDoor => [0.55, 0.35, 0.18],
        ElementType::Fireplace => [0.80, 0.20, 0.10],
        ElementType::Closet => [0.90, 0.85, 0.70],
        ElementType::Staircase => [0.80, 0.72, 0.50],
        ElementType::Chimney => [0.35, 0.30, 0.28],
        ElementType::Courtyard => [0.40, 0.70, 0.35],
        ElementType::Unclassified => [0.50, 0.50, 0.50],
    }
}

pub fn material_name(element_type: &ElementType) -> &'static str {
    match element_type {
        ElementType::Wall => "mat_wall",
        ElementType::Door => "mat_door",
        ElementType::Window => "mat_window",
        ElementType::SlidingDoor => "mat_sliding_door",
        ElementType::Fireplace => "mat_fireplace",
        ElementType::Closet => "mat_closet",
        ElementType::Staircase => "mat_staircase",
        ElementType::Chimney => "mat_chimney",
        ElementType::Courtyard => "mat_courtyard",
        ElementType::Unclassified => "mat_unclassified",
    }
}

/// Generate a `Model3D` by extruding all floor plan elements to `wall_height_m` (FR-009, FR-010).
///
/// Coordinate system: Y-up (standard Wavefront OBJ); X/Z are floor-plan horizontal axes.
/// 1 OBJ unit = 1 meter. SketchUp import dialog must be set to "Meters" (see CLAUDE.md).
pub fn generate(floor_plan: &FloorPlan, wall_height_m: f64) -> Model3D {
    // Parallelise face generation across elements (FR-027).
    let meshes: Vec<Mesh> = floor_plan
        .elements
        .par_iter()
        .filter_map(|elem| {
            let triangles = extrude_element(elem, floor_plan, wall_height_m);
            if triangles.is_empty() {
                return None;
            }
            let id8 = elem.id.to_string().chars().take(8).collect::<String>();
            let group_name = format!("{}_{}", group_prefix(&elem.element_type), id8);
            let mat = material_name(&elem.element_type);
            Some(Mesh {
                group_name,
                material_name: mat.to_string(),
                triangles,
            })
        })
        .collect();

    Model3D {
        meshes,
        wall_height_m,
    }
}

fn group_prefix(et: &ElementType) -> &'static str {
    match et {
        ElementType::Wall => "wall",
        ElementType::Door => "door",
        ElementType::Window => "window",
        ElementType::SlidingDoor => "sliding_door",
        ElementType::Fireplace => "fireplace",
        ElementType::Closet => "closet",
        ElementType::Staircase => "staircase",
        ElementType::Chimney => "chimney",
        ElementType::Courtyard => "courtyard",
        ElementType::Unclassified => "unclassified",
    }
}

/// Minimum wall thickness (meters) — used when the bounding box is degenerate (a line).
const MIN_WALL_THICKNESS_M: f32 = 0.15;

/// Extrude a single `ArchitecturalElement` into triangles.
///
/// Strategy:
/// - Walls/Closets/Chimneys: box extrusion (4 walls + top cap; no floor slab in basic model)
/// - Doors/Windows/SlidingDoors: short raised box (opening indication only)
/// - Staircases: uniform stepped box at half wall height
/// - Fireplaces: raised hearth slab (flat box at 0.3 m)
/// - Courtyards: flat recessed floor plane at -0.05 m
/// - Unclassified: same as Wall
fn extrude_element(
    elem: &crate::blueprint::element::ArchitecturalElement,
    _floor_plan: &FloorPlan,
    wall_height_m: f64,
) -> Vec<Triangle> {
    // Doors and sliding doors are wall openings only — no geometry rendered (FR-009).
    if matches!(
        elem.element_type,
        ElementType::Door | ElementType::SlidingDoor
    ) {
        return Vec::new();
    }

    let b = &elem.bounds;
    let x0 = b.min.x as f32;
    let z0 = b.min.y as f32; // floor-plan Y → 3D Z axis
    let x1 = b.max.x as f32;
    let z1 = b.max.y as f32;

    // Apply minimum thickness for degenerate bounding boxes (walls are often lines in 2D).
    let thickness = elem
        .wall_thickness_m
        .map(|t| t as f32)
        .unwrap_or(MIN_WALL_THICKNESS_M)
        .max(MIN_WALL_THICKNESS_M);

    let (x0, x1) = if (x1 - x0).abs() < 1e-4 {
        (x0 - thickness / 2.0, x0 + thickness / 2.0)
    } else {
        (x0, x1)
    };
    let (z0, z1) = if (z1 - z0).abs() < 1e-4 {
        (z0 - thickness / 2.0, z0 + thickness / 2.0)
    } else {
        (z0, z1)
    };

    let height = match elem.element_type {
        ElementType::Chimney => (wall_height_m * 1.2) as f32,
        ElementType::Staircase => (wall_height_m * 0.5) as f32,
        ElementType::Door | ElementType::SlidingDoor => (wall_height_m * 0.85) as f32,
        ElementType::Window => (wall_height_m * 0.5) as f32,
        ElementType::Fireplace => 0.3_f32,
        ElementType::Courtyard => 0.05_f32,
        _ => wall_height_m as f32,
    };

    let y_bottom = if matches!(elem.element_type, ElementType::Courtyard) {
        -0.05_f32
    } else {
        0.0_f32
    };
    let y_top = y_bottom + height;

    // For windows, lift off the floor a bit
    let (y_bottom, y_top) = if matches!(elem.element_type, ElementType::Window) {
        let y_b = (wall_height_m * 0.3) as f32;
        (y_b, y_b + height)
    } else {
        (y_bottom, y_top)
    };

    box_triangles(x0, y_bottom, z0, x1, y_top, z1)
}

/// Generate the 12 triangles (2 per face × 6 faces) of an axis-aligned box.
/// Y-up coordinate system; CCW winding from outside.
fn box_triangles(x0: f32, y0: f32, z0: f32, x1: f32, y1: f32, z1: f32) -> Vec<Triangle> {
    let mut tris = Vec::with_capacity(12);

    // Corners
    let p = [
        [x0, y0, z0], // 0
        [x1, y0, z0], // 1
        [x1, y0, z1], // 2
        [x0, y0, z1], // 3
        [x0, y1, z0], // 4
        [x1, y1, z0], // 5
        [x1, y1, z1], // 6
        [x0, y1, z1], // 7
    ];

    // Each face: two triangles, normal, CCW from outside
    let faces: &[([usize; 3], [usize; 3], [f32; 3])] = &[
        // Bottom (y0) — normal -Y
        ([0, 3, 2], [0, 2, 1], [0.0, -1.0, 0.0]),
        // Top (y1) — normal +Y
        ([4, 5, 6], [4, 6, 7], [0.0, 1.0, 0.0]),
        // Front (z0) — normal -Z
        ([0, 1, 5], [0, 5, 4], [0.0, 0.0, -1.0]),
        // Back (z1) — normal +Z
        ([2, 3, 7], [2, 7, 6], [0.0, 0.0, 1.0]),
        // Left (x0) — normal -X
        ([3, 0, 4], [3, 4, 7], [-1.0, 0.0, 0.0]),
        // Right (x1) — normal +X
        ([1, 2, 6], [1, 6, 5], [1.0, 0.0, 0.0]),
    ];

    for &(tri_a, tri_b, normal) in faces {
        tris.push(Triangle {
            vertices: [p[tri_a[0]], p[tri_a[1]], p[tri_a[2]]],
            normal,
        });
        tris.push(Triangle {
            vertices: [p[tri_b[0]], p[tri_b[1]], p[tri_b[2]]],
            normal,
        });
    }

    tris
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_element_types_have_materials() {
        let types = [
            ElementType::Wall,
            ElementType::Door,
            ElementType::Window,
            ElementType::SlidingDoor,
            ElementType::Fireplace,
            ElementType::Closet,
            ElementType::Staircase,
            ElementType::Chimney,
            ElementType::Courtyard,
            ElementType::Unclassified,
        ];
        for et in &types {
            let rgb = material_diffuse_rgb(et);
            assert!(rgb.iter().all(|&c| (0.0..=1.0).contains(&c)));
            assert!(!material_name(et).is_empty());
        }
    }

    #[test]
    fn box_triangles_produces_twelve_triangles() {
        let tris = box_triangles(0.0, 0.0, 0.0, 1.0, 1.0, 1.0);
        assert_eq!(tris.len(), 12, "a box should have 12 triangles");
    }

    #[test]
    fn generate_empty_floor_plan_produces_empty_model() {
        use crate::blueprint::scale::ScaleReference;
        use crate::blueprint::{ImagePoint, LengthUnit};

        let scale = ScaleReference::new(
            ImagePoint { x: 0, y: 0 },
            ImagePoint { x: 100, y: 0 },
            1.0,
            LengthUnit::Meters,
            1000,
            1000,
        )
        .unwrap();
        let floor_plan = crate::blueprint::floor_plan::build_floor_plan(&[], &scale, &[]).unwrap();
        let model = generate(&floor_plan, 2.44);
        assert!(model.meshes.is_empty());
    }

    /// T069 — RED test: bounds stored in feet must be normalised to meters in the OBJ output.
    ///
    /// Scale: 100 px = 10 ft  →  pixels_per_unit = 10.
    /// Wall segment: pixel x from 0..100, so bounds.max.x = to_world_distance(100) = 10.0 ft.
    /// After the fix (T070) the stored bound must be 10 * 0.3048 = 3.048 m.
    /// Before the fix this assertion FAILS (max_x ≈ 10.0 instead of 3.048).
    #[test]
    fn wall_height_round_trips_with_feet_scale() {
        use crate::blueprint::element::{ArchitecturalElement, ElementType};
        use crate::blueprint::floor_plan::build_floor_plan;
        use crate::blueprint::scale::ScaleReference;
        use crate::blueprint::{BoundingBox, ImagePoint, LengthUnit, WorldPoint};
        use uuid::Uuid;

        // 100 px = 10 ft  →  ppu = 10
        let scale = ScaleReference::new(
            ImagePoint { x: 0, y: 0 },
            ImagePoint { x: 100, y: 0 },
            10.0,
            LengthUnit::Feet,
            1000,
            1000,
        )
        .unwrap();

        // Simulate what segment_to_element() currently produces for a 100-px wide wall:
        // to_world_distance(100) = 100/10 = 10.0 ft  (not yet normalised to meters)
        // After T070 this will be 10.0 * 0.3048 = 3.048 m.
        let raw_x = scale.to_world_distance(100.0) * scale.unit.to_meters_factor();
        let raw_thickness = scale.to_world_distance(5.0) * scale.unit.to_meters_factor();

        let wall = ArchitecturalElement {
            id: Uuid::new_v4(),
            element_type: ElementType::Wall,
            bounds: BoundingBox {
                min: WorldPoint { x: 0.0, y: 0.0 },
                max: WorldPoint {
                    x: raw_x,
                    y: raw_thickness,
                },
            },
            source_segment_ids: vec![],
            confidence: 0.9,
            is_interior: Some(true),
            wall_thickness_m: Some(raw_thickness),
        };

        let floor_plan = build_floor_plan(&[wall], &scale, &[]).unwrap();
        let model = generate(&floor_plan, 2.44);

        assert!(!model.meshes.is_empty(), "should produce at least one mesh");

        let max_x = model
            .meshes
            .iter()
            .flat_map(|m| m.triangles.iter())
            .flat_map(|t| t.vertices.iter())
            .map(|v| v[0])
            .fold(f32::NEG_INFINITY, f32::max);

        let max_y = model
            .meshes
            .iter()
            .flat_map(|m| m.triangles.iter())
            .flat_map(|t| t.vertices.iter())
            .map(|v| v[1])
            .fold(f32::NEG_INFINITY, f32::max);

        // X bound must be ≈ 3.048 m (10 ft normalised to meters).
        assert!(
            (max_x - 3.048_f32).abs() < 0.01,
            "max X should be ~3.048 m (10 ft in meters), got {max_x}"
        );
        // Wall height must equal the input wall_height_m exactly.
        assert!(
            (max_y - 2.44_f32).abs() < 0.01,
            "max Y should be ~2.44 m (the input wall_height_m), got {max_y}"
        );
    }

    /// Regression guard: meter-scale floor plans must continue to work correctly after T070.
    #[test]
    fn wall_height_round_trips_with_meters_scale() {
        use crate::blueprint::element::{ArchitecturalElement, ElementType};
        use crate::blueprint::floor_plan::build_floor_plan;
        use crate::blueprint::scale::ScaleReference;
        use crate::blueprint::{BoundingBox, ImagePoint, LengthUnit, WorldPoint};
        use uuid::Uuid;

        // 100 px = 5 m  →  ppu = 20
        let scale = ScaleReference::new(
            ImagePoint { x: 0, y: 0 },
            ImagePoint { x: 100, y: 0 },
            5.0,
            LengthUnit::Meters,
            1000,
            1000,
        )
        .unwrap();

        let raw_x = scale.to_world_distance(100.0) * scale.unit.to_meters_factor(); // 5.0 m
        let raw_thickness = scale.to_world_distance(3.0) * scale.unit.to_meters_factor(); // 0.15 m

        let wall = ArchitecturalElement {
            id: Uuid::new_v4(),
            element_type: ElementType::Wall,
            bounds: BoundingBox {
                min: WorldPoint { x: 0.0, y: 0.0 },
                max: WorldPoint {
                    x: raw_x,
                    y: raw_thickness,
                },
            },
            source_segment_ids: vec![],
            confidence: 0.9,
            is_interior: Some(true),
            wall_thickness_m: Some(raw_thickness),
        };

        let floor_plan = build_floor_plan(&[wall], &scale, &[]).unwrap();
        let model = generate(&floor_plan, 3.0);

        let max_x = model
            .meshes
            .iter()
            .flat_map(|m| m.triangles.iter())
            .flat_map(|t| t.vertices.iter())
            .map(|v| v[0])
            .fold(f32::NEG_INFINITY, f32::max);

        let max_y = model
            .meshes
            .iter()
            .flat_map(|m| m.triangles.iter())
            .flat_map(|t| t.vertices.iter())
            .map(|v| v[1])
            .fold(f32::NEG_INFINITY, f32::max);

        assert!(
            (max_x - 5.0_f32).abs() < 0.01,
            "max X should be ~5.0 m, got {max_x}"
        );
        assert!(
            (max_y - 3.0_f32).abs() < 0.01,
            "max Y should be ~3.0 m (wall_height_m), got {max_y}"
        );
    }
}
