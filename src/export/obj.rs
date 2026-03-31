/// Custom OBJ + MTL writer — Y-up, CCW winding, triangulated faces (FR-012).
///
/// Coordinate system: Y-up. 1 OBJ unit = 1 meter.
/// SketchUp import: select "Meters" in the import dialog.
use std::path::Path;

use anyhow::{Context, Result};

use crate::blueprint::floor_plan::FloorPlan;
use crate::model3d::generator::{material_diffuse_rgb, material_name, Model3D};

/// Export `model` to `obj_path` and write the companion `.mtl` file alongside it.
///
/// `floor_plan` provides the list of `ElementType`s for the MTL material table.
pub fn export_obj(model: &Model3D, _floor_plan: &FloorPlan, obj_path: &Path) -> Result<()> {
    let mtl_filename = obj_path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("model");
    let mtl_filename = format!("{}.mtl", mtl_filename);
    let mtl_path = obj_path.with_file_name(&mtl_filename);

    // ── Write OBJ ──────────────────────────────────────────────────────────────
    let mut obj = String::new();
    obj.push_str("# blueprint2mod export\n");
    obj.push_str("# units: meters, Y-up coordinate system\n");
    obj.push_str(&format!("# wall height: {:.3} m\n", model.wall_height_m));
    obj.push_str(&format!("mtllib {}\n\n", mtl_filename));

    // Collect all vertices globally; OBJ indices are 1-based and cumulative.
    let mut vertex_offset = 0usize;

    for mesh in &model.meshes {
        obj.push_str(&format!("g {}\n", mesh.group_name));
        obj.push_str(&format!("usemtl {}\n", mesh.material_name));

        // Vertices for this mesh
        for tri in &mesh.triangles {
            for v in &tri.vertices {
                obj.push_str(&format!("v {:.6} {:.6} {:.6}\n", v[0], v[1], v[2]));
            }
        }

        // Faces — all triangles, 1-based, relative to global vertex list
        let mut local_vertex = vertex_offset + 1;
        for _tri in &mesh.triangles {
            obj.push_str(&format!(
                "f {} {} {}\n",
                local_vertex,
                local_vertex + 1,
                local_vertex + 2
            ));
            local_vertex += 3;
        }
        obj.push('\n');
        vertex_offset += mesh.triangles.len() * 3;
    }

    std::fs::write(obj_path, &obj)
        .with_context(|| format!("Failed to write OBJ to {}", obj_path.display()))?;

    // ── Write MTL ──────────────────────────────────────────────────────────────
    let mut mtl = String::new();
    mtl.push_str("# blueprint2mod material library\n\n");

    // Emit one material definition per unique ElementType in the floor plan
    use crate::blueprint::element::ElementType;
    let all_types = [
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
    // Only emit materials that are actually present in the model
    let mut emitted = std::collections::HashSet::new();
    for mesh in &model.meshes {
        if emitted.insert(mesh.material_name.clone()) {
            // Find the matching ElementType for this material name
            if let Some(et) = all_types
                .iter()
                .find(|et| material_name(et) == mesh.material_name)
            {
                let rgb = material_diffuse_rgb(et);
                mtl.push_str(&format!("newmtl {}\n", mesh.material_name));
                mtl.push_str(&format!("Kd {:.4} {:.4} {:.4}\n", rgb[0], rgb[1], rgb[2]));
                mtl.push_str("Ka 0.1 0.1 0.1\n");
                mtl.push_str("Ks 0.0 0.0 0.0\n");
                mtl.push_str("d 1.0\n");
                mtl.push_str("illum 1\n\n");
            }
        }
    }

    std::fs::write(&mtl_path, &mtl)
        .with_context(|| format!("Failed to write MTL to {}", mtl_path.display()))?;

    Ok(())
}
