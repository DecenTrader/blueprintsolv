/// Integration test: OCR accuracy on reference blueprint with room labels and dimensions.
use blueprint2mod::blueprint::image::BlueprintImage;
use blueprint2mod::ocr::extractor::{KnownRoomType, OcrExtractor, TextAnnotationType};
use blueprint2mod::ocr::parser::parse_annotations;

#[test]
fn ocr_extracts_known_room_labels() {
    let fixture = std::path::Path::new("test_fixtures/labeled_plan.jpg");
    let img = BlueprintImage::load(fixture).expect("fixture loads");
    let pixels = img.load_pixels().expect("pixels load");

    let extractor = OcrExtractor::new();
    let raw = extractor.extract(&pixels).expect("OCR runs without error");
    let annotations = parse_annotations(&raw);

    let room_labels: Vec<&KnownRoomType> = annotations
        .iter()
        .filter_map(|a| {
            if let TextAnnotationType::RoomLabel(ref rt) = a.annotation_type {
                Some(rt)
            } else {
                None
            }
        })
        .collect();

    assert!(
        room_labels.contains(&&KnownRoomType::Bedroom),
        "expected BEDROOM label to be detected"
    );
    assert!(
        room_labels.contains(&&KnownRoomType::Kitchen),
        "expected KITCHEN label to be detected"
    );
    assert!(
        room_labels.contains(&&KnownRoomType::LivingRoom),
        "expected LIVING ROOM label to be detected"
    );
}

#[test]
fn ocr_extracts_dimension_value() {
    let fixture = std::path::Path::new("test_fixtures/labeled_plan.jpg");
    let img = BlueprintImage::load(fixture).expect("fixture loads");
    let pixels = img.load_pixels().expect("pixels load");

    let extractor = OcrExtractor::new();
    let raw = extractor.extract(&pixels).expect("OCR runs");
    let annotations = parse_annotations(&raw);

    let dims: Vec<_> = annotations
        .iter()
        .filter(|a| matches!(a.annotation_type, TextAnnotationType::DimensionValue { .. }))
        .collect();

    assert!(
        !dims.is_empty(),
        "expected at least one dimension value to be extracted from labeled_plan.jpg"
    );
}

#[test]
fn ocr_does_not_panic_on_blank_region() {
    // Blank white 100x100 image should return empty annotations without panic
    let blank = image::DynamicImage::ImageRgba8(image::RgbaImage::from_pixel(
        100,
        100,
        image::Rgba([255, 255, 255, 255]),
    ));
    let extractor = OcrExtractor::new();
    let raw = extractor.extract(&blank).expect("OCR handles blank image");
    assert!(raw.is_empty(), "blank image should produce no OCR output");
}
