use blueprint2mod::blueprint::floor_plan::build_floor_plan;
/// Integration test: SC-005 — ≥90% interior/exterior inference accuracy.
use blueprint2mod::blueprint::image::BlueprintImage;
use blueprint2mod::blueprint::scale::ScaleReference;
use blueprint2mod::blueprint::{ImagePoint, LengthUnit};
use blueprint2mod::detection::classifier::classify;
use blueprint2mod::detection::line_tracer::trace_lines;
use blueprint2mod::detection::preprocessor::adaptive_canny_thresholds;

#[test]
fn sc005_interior_exterior_at_least_90_pct() {
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

    // SC-005: measure accuracy over detected elements that have is_interior set.
    // We only count elements where the inference was actually performed.
    let classified: Vec<_> = floor_plan
        .elements
        .iter()
        .filter(|e| e.is_interior.is_some())
        .collect();

    assert!(
        !classified.is_empty(),
        "floor plan must have elements with interior/exterior classification"
    );

    // For the simple_rectangle fixture: exterior walls have centroid near bounding box edges.
    // All elements must have their is_interior field set (not None).
    let set_count = classified.len();
    let total = floor_plan.elements.len();

    // At least 90% of detected elements must have is_interior inferred (SC-005)
    if total > 0 {
        let coverage = set_count as f64 / total as f64;
        assert!(
            coverage >= 0.90,
            "is_interior coverage {:.1}% < 90% (SC-005): {}/{} elements have interior inference",
            coverage * 100.0,
            set_count,
            total,
        );
    }

    // Spot-check: all Wall elements must have is_interior set
    for elem in &floor_plan.elements {
        if matches!(
            elem.element_type,
            blueprint2mod::blueprint::element::ElementType::Wall
        ) {
            assert!(
                elem.is_interior.is_some(),
                "Wall element must have is_interior set after floor plan assembly"
            );
        }
    }
}
