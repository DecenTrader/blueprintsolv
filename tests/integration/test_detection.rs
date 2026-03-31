use blueprint2mod::blueprint::element::ElementType;
/// Integration test: SC-002 — ≥90% wall detection accuracy on reference blueprints.
///
/// Runs the full detection pipeline on `simple_rectangle.jpg` and compares output
/// elements against `simple_rectangle.expected.json`.
use blueprint2mod::blueprint::image::BlueprintImage;
use blueprint2mod::blueprint::scale::ScaleReference;
use blueprint2mod::blueprint::{ImagePoint, LengthUnit};
use blueprint2mod::detection::classifier::classify;
use blueprint2mod::detection::line_tracer::trace_lines;
use blueprint2mod::detection::preprocessor::adaptive_canny_thresholds;

#[test]
fn sc002_wall_detection_at_least_90_pct() {
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

    let expected_json = std::fs::read_to_string("test_fixtures/simple_rectangle.expected.json")
        .expect("expected.json loads");
    let expected: serde_json::Value =
        serde_json::from_str(&expected_json).expect("expected.json parses");

    let expected_walls = expected["elements"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|e| e["element_type"] == "Wall")
        .count();

    let detected_walls = elements
        .iter()
        .filter(|e| matches!(e.element_type, ElementType::Wall))
        .count();

    assert!(expected_walls > 0, "test fixture must have walls");
    let detection_rate = detected_walls as f64 / expected_walls as f64;
    assert!(
        detection_rate >= 0.90,
        "Wall detection {:.1}% < 90% (SC-002): detected {}/{} walls",
        detection_rate * 100.0,
        detected_walls,
        expected_walls,
    );
}
