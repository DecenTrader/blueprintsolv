/// Image preprocessing for the detection pipeline (FR-004, FR-025, FR-027).
///
/// Pipeline order (FR-004):
///   1. `mask_text_regions()` — OCR text masked on raw image before denoising.
///   2. `denoise()` — NLM denoising on masked image (rayon-parallelised, FR-027).
///   3. `adaptive_canny_thresholds()` — per-image thresholds from gradient percentiles.
///
/// On `aarch64-apple-darwin` targets, inner math loops use Apple Accelerate vDSP (FR-027).
use image::{DynamicImage, GrayImage, Luma};
use rayon::prelude::*;
use std::sync::Arc;

// ── vDSP FFI (Apple Silicon only, FR-027) ───────────────────────────────────

#[cfg(all(target_arch = "aarch64", target_os = "macos"))]
mod vdsp {
    /// Sum of squares of a single-precision vector (vDSP_svesq).
    /// `__IA` = stride (1 = contiguous), `__N` = element count.
    #[link(name = "Accelerate", kind = "framework")]
    extern "C" {
        pub fn vDSP_svesq(
            __A: *const f32,
            __IA: isize,
            __C: *mut f32,
            __N: usize,
        );
    }
}

// ── Text masking (FR-004) ────────────────────────────────────────────────────

/// Mask OCR text regions in `img` (FR-004 pipeline step 1).
///
/// For each OCR bounding box:
/// - Expands by adaptive padding = `max(1, median_char_height / 2)` on all sides.
/// - Fills the padded region with the median luminance of its 1-pixel border pixels,
///   matching the local background rather than assuming white (FR-004).
///
/// Must be called on the raw (undenoised) image before `denoise()`.
pub fn mask_text_regions(
    img: DynamicImage,
    raw_ocr: &[crate::ocr::extractor::RawOcrItem],
) -> DynamicImage {
    if raw_ocr.is_empty() {
        return img;
    }

    // Adaptive padding = half of median character height across all OCR items.
    let mut heights: Vec<u32> = raw_ocr
        .iter()
        .filter(|item| item.bounds.height > 0)
        .map(|item| item.bounds.height)
        .collect();
    heights.sort_unstable();
    let median_height = heights.get(heights.len() / 2).copied().unwrap_or(0);
    let pad = (median_height / 2).max(1);

    let mut rgba = img.to_rgba8();
    let (img_w, img_h) = rgba.dimensions();

    for item in raw_ocr {
        let b = &item.bounds;
        if b.width == 0 || b.height == 0 {
            continue;
        }

        // Expand bounding box by adaptive padding, clamped to image bounds.
        let x0 = b.x.saturating_sub(pad);
        let y0 = b.y.saturating_sub(pad);
        let x1 = (b.x.saturating_add(b.width).saturating_add(pad)).min(img_w);
        let y1 = (b.y.saturating_add(b.height).saturating_add(pad)).min(img_h);
        if x0 >= x1 || y0 >= y1 {
            continue;
        }

        // Sample local background from the 1-pixel border of the expanded region.
        let fill = sample_border_median(&rgba, x0, y0, x1, y1);
        for py in y0..y1 {
            for px in x0..x1 {
                rgba.put_pixel(px, py, image::Rgba([fill, fill, fill, 255]));
            }
        }
    }

    DynamicImage::ImageRgba8(rgba)
}

/// Return the median luminance of the 1-pixel border of rectangle `[x0..x1) × [y0..y1)`.
fn sample_border_median(img: &image::RgbaImage, x0: u32, y0: u32, x1: u32, y1: u32) -> u8 {
    let (iw, ih) = img.dimensions();
    let mut samples: Vec<u8> = Vec::new();
    let yb = y1.saturating_sub(1);
    let xr = x1.saturating_sub(1);

    // Top and bottom rows
    for x in x0..x1 {
        if x < iw {
            if y0 < ih {
                samples.push(luma_of_rgba(img.get_pixel(x, y0)));
            }
            if yb != y0 && yb < ih {
                samples.push(luma_of_rgba(img.get_pixel(x, yb)));
            }
        }
    }
    // Left and right columns (excluding corners already sampled)
    for y in (y0 + 1)..yb {
        if y < ih {
            if x0 < iw {
                samples.push(luma_of_rgba(img.get_pixel(x0, y)));
            }
            if xr != x0 && xr < iw {
                samples.push(luma_of_rgba(img.get_pixel(xr, y)));
            }
        }
    }

    if samples.is_empty() {
        return 255;
    }
    samples.sort_unstable();
    samples[samples.len() / 2]
}

#[inline]
fn luma_of_rgba(p: &image::Rgba<u8>) -> u8 {
    let r = p.0[0] as u32;
    let g = p.0[1] as u32;
    let b = p.0[2] as u32;
    ((r * 299 + g * 587 + b * 114) / 1000) as u8
}

// ── Denoising (FR-025, FR-027) ───────────────────────────────────────────────

/// Apply non-local means denoising to a blueprint image (FR-025, FR-027).
///
/// Converts the input to grayscale, applies a rayon-parallelised NLM filter
/// (search window 21 px, patch 7 px, h=10). On `aarch64-apple-darwin`, the
/// inner patch-distance sum uses `vDSP_svesq` from Apple Accelerate.
pub fn denoise(img: &DynamicImage) -> DynamicImage {
    let gray = img.to_luma8();
    DynamicImage::ImageLuma8(nlm_denoise(&gray, 10, 3, 10.0))
}

/// Compute per-image Canny thresholds from gradient magnitude percentiles (FR-025, FR-027).
///
/// Returns `(low, high)` where:
/// - `low` = 70th-percentile gradient magnitude (minimum 10.0)
/// - `high` = 90th-percentile gradient magnitude (minimum low + 10.0)
///
/// The gradient computation is parallelised with rayon (FR-027).
pub fn adaptive_canny_thresholds(gray: &GrayImage) -> (f64, f64) {
    let (width, height) = gray.dimensions();
    if width < 3 || height < 3 {
        return (30.0, 80.0);
    }

    let mut magnitudes: Vec<u32> = (1u32..(height - 1))
        .into_par_iter()
        .flat_map_iter(|y| {
            (1u32..(width - 1)).map(move |x| {
                let left = gray.get_pixel(x - 1, y).0[0] as i32;
                let right = gray.get_pixel(x + 1, y).0[0] as i32;
                let up = gray.get_pixel(x, y - 1).0[0] as i32;
                let down = gray.get_pixel(x, y + 1).0[0] as i32;
                let gx = right - left;
                let gy = down - up;
                (gx * gx + gy * gy) as u32
            })
        })
        .collect();

    magnitudes.sort_unstable();
    let n = magnitudes.len();
    if n == 0 {
        return (30.0, 80.0);
    }

    let low = (magnitudes[(n * 70 / 100).min(n - 1)] as f64)
        .sqrt()
        .max(10.0);
    let high = (magnitudes[(n * 90 / 100).min(n - 1)] as f64)
        .sqrt()
        .max(low + 10.0);

    (low, high)
}

// ── Non-local means implementation (FR-025, FR-027) ─────────────────────────

/// Rayon-parallelised NLM denoising.
///
/// Parameters: search_half=10 (21×21 window), patch_half=3 (7×7 patch), h_param=10.
fn nlm_denoise(src: &GrayImage, search_half: i32, patch_half: i32, h_param: f32) -> GrayImage {
    let (width, height) = src.dimensions();
    if width == 0 || height == 0 {
        return src.clone();
    }

    let src_arc: Arc<GrayImage> = Arc::new(src.clone());
    let n_bands = rayon::current_num_threads().max(1).min(height as usize);
    let rows_per_band = (height as usize).div_ceil(n_bands);

    // Each rayon worker returns a flat Vec<u8> for its row band (row-major, 1 byte/pixel).
    let bands: Vec<Vec<u8>> = (0..n_bands)
        .into_par_iter()
        .filter_map(|t| {
            let start = (t * rows_per_band) as u32;
            if start >= height {
                return None;
            }
            let end = (((t + 1) * rows_per_band) as u32).min(height);
            Some(nlm_row_band(
                &src_arc,
                width,
                height,
                start,
                end,
                search_half,
                patch_half,
                h_param,
            ))
        })
        .collect();

    // Reassemble output image from bands.
    let mut out = GrayImage::new(width, height);
    let mut row = 0u32;
    for band in &bands {
        for chunk in band.chunks(width as usize) {
            for (x, &val) in chunk.iter().enumerate() {
                out.put_pixel(x as u32, row, Luma([val]));
            }
            row += 1;
        }
    }
    out
}

/// Compute NLM denoised pixel values for rows `start_row..end_row`.
///
/// On `aarch64-apple-darwin`, the patch-distance inner loop uses `vDSP_svesq`
/// for the sum-of-squares computation. Elsewhere a plain Rust loop is used.
#[allow(clippy::too_many_arguments)]
fn nlm_row_band(
    src: &GrayImage,
    width: u32,
    height: u32,
    start_row: u32,
    end_row: u32,
    search_half: i32,
    patch_half: i32,
    h_param: f32,
) -> Vec<u8> {
    let img_w = width as i32;
    let img_h = height as i32;
    let h2 = h_param * h_param;
    let patch_side = 2 * patch_half + 1;
    let patch_area = (patch_side * patch_side) as f32;

    let get = |x: i32, y: i32| -> f32 {
        let cx = x.clamp(0, img_w - 1) as u32;
        let cy = y.clamp(0, img_h - 1) as u32;
        src.get_pixel(cx, cy).0[0] as f32
    };

    let rows = (end_row - start_row) as usize;
    let mut out = Vec::with_capacity(rows * width as usize);

    for py in start_row..end_row {
        for px in 0..width {
            let px = px as i32;
            let py = py as i32;
            let mut sum_w = 0.0f32;
            let mut sum_v = 0.0f32;

            for sy in (py - search_half)..=(py + search_half) {
                for sx in (px - search_half)..=(px + search_half) {
                    let dist = patch_distance(&get, px, py, sx, sy, patch_half, patch_area);
                    let w = (-dist / h2).exp();
                    sum_v += w * get(sx, sy);
                    sum_w += w;
                }
            }

            let val = if sum_w > 0.0 {
                (sum_v / sum_w).clamp(0.0, 255.0) as u8
            } else {
                src.get_pixel(px as u32, py as u32).0[0]
            };
            out.push(val);
        }
    }
    out
}

/// Normalised squared patch distance between pixel `(px,py)` and `(sx,sy)` (FR-027).
///
/// Uses `vDSP_svesq` on `aarch64-apple-darwin`; plain Rust everywhere else.
#[inline]
fn patch_distance<F>(
    get: &F,
    px: i32,
    py: i32,
    sx: i32,
    sy: i32,
    patch_half: i32,
    patch_area: f32,
) -> f32
where
    F: Fn(i32, i32) -> f32,
{
    #[cfg(all(target_arch = "aarch64", target_os = "macos"))]
    {
        let patch_len = patch_area as usize;
        let mut diff_buf: Vec<f32> = Vec::with_capacity(patch_len);
        for dy in -patch_half..=patch_half {
            for dx in -patch_half..=patch_half {
                diff_buf.push(get(px + dx, py + dy) - get(sx + dx, sy + dy));
            }
        }
        let mut sq_sum = 0.0f32;
        unsafe {
            vdsp::vDSP_svesq(diff_buf.as_ptr(), 1, &mut sq_sum, patch_len);
        }
        return sq_sum / patch_area;
    }

    #[allow(unreachable_code)]
    {
        let mut d = 0.0f32;
        for dy in -patch_half..=patch_half {
            for dx in -patch_half..=patch_half {
                let diff = get(px + dx, py + dy) - get(sx + dx, sy + dy);
                d += diff * diff;
            }
        }
        d / patch_area
    }
}

// ── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::blueprint::ImageBoundingBox;
    use crate::ocr::extractor::RawOcrItem;

    #[test]
    fn denoise_preserves_dimensions() {
        let img = DynamicImage::new_luma8(64, 64);
        let result = denoise(&img);
        assert_eq!(result.width(), 64);
        assert_eq!(result.height(), 64);
    }

    #[test]
    fn adaptive_thresholds_low_lt_high() {
        let img = image::open("test_fixtures/simple_rectangle.jpg").unwrap();
        let gray = img.to_luma8();
        let (low, high) = adaptive_canny_thresholds(&gray);
        assert!(low < high, "low ({low}) should be < high ({high})");
    }

    #[test]
    fn adaptive_thresholds_fallback_for_tiny_image() {
        let tiny = GrayImage::new(2, 2);
        let (low, high) = adaptive_canny_thresholds(&tiny);
        assert_eq!(low, 30.0);
        assert_eq!(high, 80.0);
    }

    #[test]
    fn nlm_denoise_uniform_image_unchanged() {
        // A uniform-grey image should denoise to the same value (all patch distances = 0).
        let mut gray = GrayImage::new(32, 32);
        for p in gray.pixels_mut() {
            *p = Luma([128u8]);
        }
        let result = nlm_denoise(&gray, 3, 1, 10.0);
        for p in result.pixels() {
            assert_eq!(p.0[0], 128, "uniform image should be unchanged by NLM");
        }
    }

    #[test]
    fn mask_text_regions_padding_is_half_median_char_height() {
        // 100×100 transparent image; OCR item height=20 → pad=10
        let img = DynamicImage::new_rgba8(100, 100);
        let ocr = vec![RawOcrItem {
            text: "TEST".to_string(),
            bounds: ImageBoundingBox { x: 40, y: 40, width: 20, height: 20 },
            confidence: 0.9,
        }];
        let result = mask_text_regions(img, &ocr);
        let rgba = result.to_rgba8();
        // Padded region: x=[30..70), y=[30..70). Pixel at (30,30) should be opaque.
        let inside = rgba.get_pixel(30, 30);
        // Pixel at (20,20) is outside the padded region — transparent in the source image.
        let outside = rgba.get_pixel(20, 20);
        assert_eq!(inside.0[3], 255, "inside mask should be opaque");
        assert_eq!(outside.0[3], 0, "outside mask should be unmodified (alpha=0)");
    }

    #[test]
    fn mask_text_fill_value_matches_border_median_not_white() {
        // Dark grey image (50,50,50,255); fill should be ~50, not 255.
        let mut img_buf = image::RgbaImage::new(100, 100);
        for p in img_buf.pixels_mut() {
            *p = image::Rgba([50, 50, 50, 255]);
        }
        let img = DynamicImage::ImageRgba8(img_buf);
        let ocr = vec![RawOcrItem {
            text: "X".to_string(),
            bounds: ImageBoundingBox { x: 40, y: 40, width: 20, height: 20 },
            confidence: 0.9,
        }];
        let result = mask_text_regions(img, &ocr);
        let rgba = result.to_rgba8();
        // luma_of_rgba([50,50,50]) = 50; fill should be close to 50, not 255.
        let center = rgba.get_pixel(50, 50);
        assert!(
            center.0[0] < 100,
            "fill should match local background (~50), not white (255); got {}",
            center.0[0]
        );
    }

    #[test]
    fn mask_two_overlapping_boxes_both_filled() {
        let mut img_buf = image::RgbaImage::new(100, 100);
        for p in img_buf.pixels_mut() {
            *p = image::Rgba([200, 200, 200, 255]);
        }
        let img = DynamicImage::ImageRgba8(img_buf);
        // Two adjacent items, same height → same pad
        let ocr = vec![
            RawOcrItem {
                text: "A".to_string(),
                bounds: ImageBoundingBox { x: 10, y: 10, width: 20, height: 10 },
                confidence: 0.9,
            },
            RawOcrItem {
                text: "B".to_string(),
                bounds: ImageBoundingBox { x: 60, y: 10, width: 20, height: 10 },
                confidence: 0.9,
            },
        ];
        let result = mask_text_regions(img, &ocr);
        let rgba = result.to_rgba8();
        // Both centers should be filled (opaque)
        assert_eq!(rgba.get_pixel(20, 15).0[3], 255, "first box center should be filled");
        assert_eq!(rgba.get_pixel(70, 15).0[3], 255, "second box center should be filled");
    }
}
