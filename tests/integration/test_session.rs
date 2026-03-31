/// Integration test: Session save/load round-trip (T015).
///
/// Saves a session after the scaling step, reloads it, and asserts all fields
/// round-trip correctly (FR-016, FR-017).
use blueprint2mod::blueprint::image::BlueprintImage;
use blueprint2mod::blueprint::scale::ScaleReference;
use blueprint2mod::blueprint::{ImagePoint, LengthUnit};
use blueprint2mod::session::serialization::Session;

#[test]
fn session_roundtrip_preserves_scale() {
    let dir = tempfile::tempdir().unwrap();
    let session_path = dir.path().join("test.b2m");

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

    let mut session = Session::new(img);
    session.scale = Some(scale.clone());

    session.save(&session_path).expect("session saves");

    let loaded = Session::load(&session_path).expect("session loads");
    assert_eq!(loaded.version, "1.0");
    let loaded_scale = loaded.scale.expect("scale is present after reload");
    assert!(
        (loaded_scale.pixels_per_unit - scale.pixels_per_unit).abs() < 1e-9,
        "pixels_per_unit round-trips: {} vs {}",
        loaded_scale.pixels_per_unit,
        scale.pixels_per_unit
    );
    assert_eq!(
        loaded_scale.real_world_distance, scale.real_world_distance,
        "real_world_distance round-trips"
    );
}

#[test]
fn session_roundtrip_preserves_image_path() {
    let dir = tempfile::tempdir().unwrap();
    let session_path = dir.path().join("path_test.b2m");

    let fixture = std::path::Path::new("test_fixtures/simple_rectangle.jpg");
    let img = BlueprintImage::load(fixture).unwrap();
    let original_path = img.path.clone();

    let mut session = Session::new(img);
    session.save(&session_path).unwrap();

    let loaded = Session::load(&session_path).unwrap();
    assert_eq!(loaded.image.path, original_path);
}

#[test]
fn session_version_is_1_0() {
    let fixture = std::path::Path::new("test_fixtures/simple_rectangle.jpg");
    let img = BlueprintImage::load(fixture).unwrap();
    let session = Session::new(img);
    assert_eq!(session.version, "1.0");
}
