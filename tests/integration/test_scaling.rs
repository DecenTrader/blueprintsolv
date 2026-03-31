use blueprint2mod::blueprint::image::BlueprintImage;
use blueprint2mod::blueprint::scale::ScaleReference;
/// Integration test: SC-004 dimension accuracy within ±5% tolerance.
///
/// Loads `test_fixtures/simple_rectangle.jpg`, supplies two reference points 320 px apart,
/// specifies 3.66 m real-world distance, and asserts `pixels_per_unit` is within ±5%.
use blueprint2mod::blueprint::{ImagePoint, LengthUnit};

#[test]
fn sc004_scale_within_5pct_tolerance() {
    let fixture = std::path::Path::new("test_fixtures/simple_rectangle.jpg");
    assert!(
        fixture.exists(),
        "test fixture must exist: {}",
        fixture.display()
    );

    let img = BlueprintImage::load(fixture).expect("fixture image loads successfully");

    let scale = ScaleReference::new(
        ImagePoint { x: 100, y: 300 },
        ImagePoint { x: 420, y: 300 },
        3.66,
        LengthUnit::Meters,
        img.width,
        img.height,
    )
    .expect("valid scale reference");

    let expected_ppu = 87.43_f64;
    let tolerance = expected_ppu * 0.05;
    assert!(
        (scale.pixels_per_unit - expected_ppu).abs() <= tolerance,
        "pixels_per_unit {:.4} not within ±5% of {:.4} (SC-004)",
        scale.pixels_per_unit,
        expected_ppu,
    );
}

#[test]
fn scale_converts_known_pixel_distance_to_world() {
    let scale = ScaleReference::new(
        ImagePoint { x: 100, y: 300 },
        ImagePoint { x: 420, y: 300 },
        3.66,
        LengthUnit::Meters,
        800,
        600,
    )
    .unwrap();

    // 320 px / pixels_per_unit should equal 3.66 m within 5%
    let world = scale.to_world_distance(320.0);
    assert!(
        (world - 3.66).abs() < 3.66 * 0.05,
        "world distance {:.4} not within 5% of 3.66 m",
        world
    );
}

#[test]
fn scale_loads_from_jpeg_fixture() {
    let fixture = std::path::Path::new("test_fixtures/simple_rectangle.jpg");
    let img = BlueprintImage::load(fixture).expect("loads jpg");
    assert_eq!(img.width, 800);
    assert_eq!(img.height, 600);
}
