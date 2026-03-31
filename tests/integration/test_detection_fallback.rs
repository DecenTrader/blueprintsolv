/// Integration test: Constitution Principle III — rule-based-only fallback.
///
/// When ML models are unavailable, the classifier must fall back to rule-based mode
/// without panicking and must emit a warning (FR-019).
use blueprint2mod::blueprint::image::BlueprintImage;
use blueprint2mod::blueprint::scale::ScaleReference;
use blueprint2mod::blueprint::{ImagePoint, LengthUnit};
use blueprint2mod::detection::classifier::{classify, ClassifyResult};
use blueprint2mod::detection::line_tracer::trace_lines;
use blueprint2mod::detection::preprocessor::adaptive_canny_thresholds;

#[test]
fn fallback_to_rules_when_no_ml_models() {
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

    // Pass `None` for model path to simulate unavailable ML models
    let result: ClassifyResult = classify_with_result(&segments, Some(&pixels), None);

    assert!(
        result.used_fallback,
        "classifier must report fallback mode when ML models are unavailable"
    );
    // Rule-based fallback must not produce zero elements on a non-empty image
    assert!(
        !result.elements.is_empty(),
        "rule-based fallback must produce at least some elements"
    );
}

#[test]
fn fallback_produces_no_panic_on_empty_segments() {
    // Edge case: no segments traced (e.g. blank image) — must not panic
    let result: ClassifyResult = classify_with_result(&[], None, None);
    assert!(result.elements.is_empty());
    // No ML models → fallback = true even for empty input
    assert!(result.used_fallback);
}

// Thin wrapper to get fallback status alongside elements
fn classify_with_result(
    segments: &[blueprint2mod::blueprint::scale::LineSegment],
    img: Option<&image::DynamicImage>,
    model_dir: Option<&std::path::Path>,
) -> ClassifyResult {
    blueprint2mod::detection::classifier::classify_verbose(segments, img, model_dir)
}
