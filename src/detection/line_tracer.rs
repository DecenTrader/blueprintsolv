use image::{DynamicImage, GrayImage};
use imageproc::distance_transform::Norm;
use imageproc::edges::canny;
use imageproc::morphology::dilate;
use rayon::prelude::*;
use uuid::Uuid;

use crate::blueprint::{scale::LineSegment, scale::ScaleReference, ImagePoint};

/// Trace dark lines in `img` using Canny edge detection and contour following (FR-004).
///
/// `low_thresh` and `high_thresh` are the Canny hysteresis thresholds; callers should
/// compute these via `preprocessor::adaptive_canny_thresholds` (FR-025).
///
/// Returns a list of `LineSegment`s with `real_world_length` computed from `scale`.
pub fn trace_lines(
    img: &DynamicImage,
    scale: &ScaleReference,
    low_thresh: f64,
    high_thresh: f64,
) -> Vec<LineSegment> {
    let gray = img.to_luma8();
    let equalized = equalize_histogram(&gray);
    let edges = canny(&equalized, low_thresh as f32, high_thresh as f32);
    let dilated = dilate(&edges, Norm::L1, 1);
    extract_segments(&dilated, scale)
}

/// Simple histogram equalization to improve contrast (FR-004 preprocessing).
fn equalize_histogram(gray: &GrayImage) -> GrayImage {
    let (width, height) = gray.dimensions();
    let total = (width * height) as f32;

    // Build histogram
    let mut hist = [0u32; 256];
    for p in gray.pixels() {
        hist[p.0[0] as usize] += 1;
    }

    // Build CDF
    let mut cdf = [0f32; 256];
    let mut cumulative = 0u32;
    for (i, &count) in hist.iter().enumerate() {
        cumulative += count;
        cdf[i] = cumulative as f32 / total;
    }

    // Map pixel values — parallelised with rayon (FR-027)
    let raw: Vec<u8> = gray
        .as_raw()
        .par_iter()
        .map(|&v| (cdf[v as usize] * 255.0).round() as u8)
        .collect();
    GrayImage::from_raw(width, height, raw).unwrap_or_else(|| gray.clone())
}

/// Extract contiguous dark pixel runs from a binary edge image into `LineSegment`s.
///
/// Uses a horizontal scan + run-length encoding to identify pixel sequences, then
/// groups them into segments. Wall spacing (double-line detection) is computed for
/// parallel pairs within 20px of each other (FR-009).
fn extract_segments(edges: &GrayImage, scale: &ScaleReference) -> Vec<LineSegment> {
    let (width, height) = edges.dimensions();
    let mut segments: Vec<LineSegment> = Vec::new();

    // Horizontal runs
    for y in 0..height {
        let mut run_start: Option<u32> = None;
        for x in 0..=width {
            let is_edge = x < width && edges.get_pixel(x, y).0[0] > 128;
            match (run_start, is_edge) {
                (None, true) => run_start = Some(x),
                (Some(start), false) if x - start >= 3 => {
                    let points: Vec<ImagePoint> =
                        (start..x).map(|px| ImagePoint { x: px, y }).collect();
                    let length_pixels = (x - start) as f64;
                    segments.push(LineSegment {
                        id: Uuid::new_v4(),
                        real_world_length: scale.to_world_distance(length_pixels),
                        length_pixels,
                        wall_spacing: None,
                        points,
                    });
                    run_start = None;
                }
                (Some(_), false) => run_start = None,
                _ => {}
            }
        }
    }

    // Vertical runs
    for x in 0..width {
        let mut run_start: Option<u32> = None;
        for y in 0..=height {
            let is_edge = y < height && edges.get_pixel(x, y).0[0] > 128;
            match (run_start, is_edge) {
                (None, true) => run_start = Some(y),
                (Some(start), false) if y - start >= 3 => {
                    let points: Vec<ImagePoint> =
                        (start..y).map(|py| ImagePoint { x, y: py }).collect();
                    let length_pixels = (y - start) as f64;
                    segments.push(LineSegment {
                        id: Uuid::new_v4(),
                        real_world_length: scale.to_world_distance(length_pixels),
                        length_pixels,
                        wall_spacing: None,
                        points,
                    });
                    run_start = None;
                }
                (Some(_), false) => run_start = None,
                _ => {}
            }
        }
    }

    detect_double_lines(&mut segments);
    segments
}

/// Detect parallel double-line pairs (wall thickness detection, FR-009).
///
/// For horizontal segments within 20px of each other that share ≥50% x-overlap,
/// annotate both with `wall_spacing = pixel_gap`.
fn detect_double_lines(segments: &mut [LineSegment]) {
    let n = segments.len();
    for i in 0..n {
        for j in (i + 1)..n {
            let a = &segments[i];
            let b = &segments[j];

            if a.points.is_empty() || b.points.is_empty() {
                continue;
            }

            let a_y = a.points[0].y;
            let b_y = b.points[0].y;

            // Same orientation (both horizontal: same y for all points)
            let a_horiz = a.points.iter().all(|p| p.y == a_y);
            let b_horiz = b.points.iter().all(|p| p.y == b_y);

            if a_horiz && b_horiz {
                let y_gap = (a_y as i64 - b_y as i64).unsigned_abs();
                if y_gap > 0 && y_gap <= 20 {
                    let a_x_min = a.points.iter().map(|p| p.x).min().unwrap_or(0);
                    let a_x_max = a.points.iter().map(|p| p.x).max().unwrap_or(0);
                    let b_x_min = b.points.iter().map(|p| p.x).min().unwrap_or(0);
                    let b_x_max = b.points.iter().map(|p| p.x).max().unwrap_or(0);

                    let overlap_start = a_x_min.max(b_x_min);
                    let overlap_end = a_x_max.min(b_x_max);
                    if overlap_end > overlap_start {
                        let overlap = (overlap_end - overlap_start) as f64;
                        let a_len = (a_x_max - a_x_min) as f64;
                        if a_len > 0.0 && overlap / a_len >= 0.5 {
                            let spacing = y_gap as f64;
                            segments[i].wall_spacing = Some(spacing);
                            segments[j].wall_spacing = Some(spacing);
                        }
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::blueprint::LengthUnit;

    fn test_scale() -> ScaleReference {
        ScaleReference::new(
            ImagePoint { x: 0, y: 0 },
            ImagePoint { x: 100, y: 0 },
            1.0,
            LengthUnit::Meters,
            1000,
            1000,
        )
        .unwrap()
    }

    #[test]
    fn trace_returns_segments_for_blueprint_fixture() {
        use crate::detection::preprocessor::adaptive_canny_thresholds;
        let img = image::open("test_fixtures/simple_rectangle.jpg").expect("fixture loads");
        let gray = img.to_luma8();
        let (low, high) = adaptive_canny_thresholds(&gray);
        let scale = test_scale();
        let segments = trace_lines(&img, &scale, low, high);
        assert!(
            !segments.is_empty(),
            "should find line segments in blueprint"
        );
    }

    #[test]
    fn trace_returns_empty_for_blank_image() {
        let blank = DynamicImage::ImageLuma8(GrayImage::new(100, 100));
        let scale = test_scale();
        // Blank image: adaptive thresholds fall back to defaults; no edges expected.
        let segments = trace_lines(&blank, &scale, 30.0, 80.0);
        assert!(segments.is_empty());
    }

    #[test]
    fn segments_have_positive_real_world_length() {
        use crate::detection::preprocessor::adaptive_canny_thresholds;
        let img = image::open("test_fixtures/simple_rectangle.jpg").unwrap();
        let gray = img.to_luma8();
        let (low, high) = adaptive_canny_thresholds(&gray);
        let scale = test_scale();
        let segments = trace_lines(&img, &scale, low, high);
        for seg in &segments {
            assert!(
                seg.real_world_length > 0.0,
                "segment has non-positive real_world_length"
            );
        }
    }
}
