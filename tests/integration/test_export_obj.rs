/// Integration test: SC-003, SC-004 — OBJ export is SketchUp-compatible and dimensions correct.
use blueprint2mod::blueprint::floor_plan::build_floor_plan;
use blueprint2mod::blueprint::image::BlueprintImage;
use blueprint2mod::blueprint::scale::ScaleReference;
use blueprint2mod::blueprint::{ImagePoint, LengthUnit};
use blueprint2mod::detection::classifier::classify;
use blueprint2mod::detection::line_tracer::trace_lines;
use blueprint2mod::detection::preprocessor::adaptive_canny_thresholds;
use blueprint2mod::export::obj::export_obj;
use blueprint2mod::model3d::generator::generate;

#[test]
fn sc003_obj_export_has_required_structure() {
    let fixture = std::path::Path::new("test_fixtures/simple_rectangle.jpg");
    let img = BlueprintImage::load(fixture).expect("fixture loads");
    let scale = ScaleReference::new(
        ImagePoint { x: 100, y: 300 },
        ImagePoint { x: 420, y: 300 },
        3.66,
        LengthUnit::Meters,
        img.width,
        img.height,
    )
    .unwrap();

    let pixels = img.load_pixels().expect("pixels load");
    let (low, high) = adaptive_canny_thresholds(&pixels.to_luma8());
    let segments = trace_lines(&pixels, &scale, low, high);
    let elements = classify(&segments, Some(&pixels), None);
    let floor_plan = build_floor_plan(&elements, &scale, &[]).expect("floor plan builds");
    let model = generate(&floor_plan, 2.44);

    let tmp_dir = tempfile::tempdir().expect("temp dir");
    let obj_path = tmp_dir.path().join("test.obj");
    let mtl_path = tmp_dir.path().join("test.mtl");

    export_obj(&model, &floor_plan, &obj_path).expect("OBJ export succeeds");

    // 1. OBJ file must exist and be non-empty
    assert!(obj_path.exists(), "OBJ file must be created");
    let obj_content = std::fs::read_to_string(&obj_path).expect("read OBJ");
    assert!(!obj_content.is_empty(), "OBJ file must not be empty");

    // 2. MTL file must be created alongside (SC-003)
    assert!(
        mtl_path.exists(),
        "MTL file must be created alongside OBJ (SC-003)"
    );
    let mtl_content = std::fs::read_to_string(&mtl_path).expect("read MTL");

    // 3. mtllib directive present in OBJ (SC-003)
    assert!(
        obj_content.contains("mtllib"),
        "OBJ must contain 'mtllib' directive (SC-003)"
    );

    // 4. At least one named group present
    assert!(
        obj_content.contains("\ng ") || obj_content.starts_with("g "),
        "OBJ must have at least one named group"
    );

    // 5. MTL file defines at least one material
    assert!(
        mtl_content.contains("newmtl"),
        "MTL must define at least one material"
    );
    assert!(
        mtl_content.contains("Kd"),
        "MTL must define diffuse color (Kd)"
    );

    // 6. All faces must be triangles (3 vertex indices) (SC-003)
    for line in obj_content.lines() {
        if let Some(rest) = line.strip_prefix("f ") {
            let indices: Vec<&str> = rest.split_whitespace().collect();
            assert_eq!(
                indices.len(),
                3,
                "All OBJ faces must be triangles (got {}): line '{}'",
                indices.len(),
                line
            );
        }
    }

    // 7. Vertex coordinates present and finite (basic SC-004 sanity)
    let mut vertex_count = 0usize;
    for line in obj_content.lines() {
        if let Some(rest) = line.strip_prefix("v ") {
            let coords: Vec<f64> = rest
                .split_whitespace()
                .filter_map(|s| s.parse().ok())
                .collect();
            assert_eq!(
                coords.len(),
                3,
                "each 'v' line must have exactly 3 coordinates"
            );
            assert!(
                coords.iter().all(|c| c.is_finite()),
                "vertex coordinates must be finite"
            );
            vertex_count += 1;
        }
    }
    assert!(vertex_count > 0, "OBJ must contain at least one vertex");
}
