/// Integration test: STL export produces valid binary STL with at least one triangle.
use blueprint2mod::blueprint::floor_plan::build_floor_plan;
use blueprint2mod::blueprint::image::BlueprintImage;
use blueprint2mod::blueprint::scale::ScaleReference;
use blueprint2mod::blueprint::{ImagePoint, LengthUnit};
use blueprint2mod::detection::classifier::classify;
use blueprint2mod::detection::line_tracer::trace_lines;
use blueprint2mod::detection::preprocessor::adaptive_canny_thresholds;
use blueprint2mod::export::stl::export_stl;
use blueprint2mod::model3d::generator::generate;

#[test]
fn stl_export_produces_valid_binary_stl() {
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
    let stl_path = tmp_dir.path().join("test.stl");

    export_stl(&model, &stl_path).expect("STL export succeeds");

    // Binary STL layout: 80-byte header + 4-byte triangle count + N × 50 bytes per triangle.
    let bytes = std::fs::read(&stl_path).expect("read STL file");
    assert!(
        bytes.len() >= 84,
        "binary STL must be at least 84 bytes (header + count), got {}",
        bytes.len()
    );

    let triangle_count = u32::from_le_bytes([bytes[80], bytes[81], bytes[82], bytes[83]]);
    assert!(triangle_count > 0, "STL must contain at least one triangle");

    // Verify file size matches header (84 + N * 50)
    let expected_size = 84 + (triangle_count as usize) * 50;
    assert_eq!(
        bytes.len(),
        expected_size,
        "STL file size {} does not match expected {} for {} triangles",
        bytes.len(),
        expected_size,
        triangle_count
    );
}
