/// Binary STL writer using `stl_io` (FR-013).
use std::path::Path;

use anyhow::{Context, Result};

use crate::model3d::generator::Model3D;

/// Export `model` as a binary STL file at `path`.
///
/// All meshes are merged into one STL solid. Normals are taken from `Triangle.normal`.
pub fn export_stl(model: &Model3D, path: &Path) -> Result<()> {
    use stl_io::{Normal, Triangle, Vertex};

    let triangles: Vec<stl_io::Triangle> = model
        .meshes
        .iter()
        .flat_map(|mesh| {
            mesh.triangles.iter().map(|t| Triangle {
                normal: Normal::new(t.normal),
                vertices: [
                    Vertex::new(t.vertices[0]),
                    Vertex::new(t.vertices[1]),
                    Vertex::new(t.vertices[2]),
                ],
            })
        })
        .collect();

    let mut file = std::fs::File::create(path)
        .with_context(|| format!("Failed to create STL file: {}", path.display()))?;

    stl_io::write_stl(&mut file, triangles.iter())
        .with_context(|| format!("Failed to write STL to {}", path.display()))?;

    Ok(())
}
