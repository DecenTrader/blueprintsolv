use std::path::Path;

use image::DynamicImage;
use rayon::prelude::*;

use crate::blueprint::element::ArchitecturalElement;
use crate::blueprint::scale::{LineSegment, ScaleReference};
use crate::correction::history::CorrectionHistory;
use crate::detection::ml::{inference::classify_patch, model_manager};
use crate::detection::rules::patterns::{classify_segment, segment_to_element};

/// Full result from `classify_verbose`, including fallback status (Constitution Principle III).
pub struct ClassifyResult {
    pub elements: Vec<ArchitecturalElement>,
    /// `true` when the ML models were unavailable and rule-based mode was used.
    pub used_fallback: bool,
}

/// Classify segments using the hybrid ML + rule-based pipeline (FR-005, FR-007, FR-019).
///
/// `img` is the full blueprint image used to crop per-segment patches for ML inference.
/// Pass `None` to force rule-based mode (e.g. in tests or when the image is unavailable).
///
/// `model_dir` is `None` to use the default cache directory, or `Some(path)` to override
/// (used in tests to simulate missing models).
///
/// Strategy:
/// 1. Load `CorrectionHistory` to get `adaptive_threshold`.
/// 2. If ML models available and image provided: crop patch → ML inference per segment;
///    use ML result if `confidence ≥ adaptive_threshold`, else fall back to rules.
/// 3. If ML unavailable or image absent: emit warning, use rules for all segments.
pub fn classify(
    segments: &[LineSegment],
    img: Option<&DynamicImage>,
    model_dir: Option<&Path>,
) -> Vec<ArchitecturalElement> {
    classify_verbose(segments, img, model_dir).elements
}

/// Like `classify` but also returns whether rule-based fallback was used.
pub fn classify_verbose(
    segments: &[LineSegment],
    img: Option<&DynamicImage>,
    model_dir: Option<&Path>,
) -> ClassifyResult {
    let history = CorrectionHistory::load_or_default().unwrap_or_default();
    let threshold = history.adaptive_threshold;

    let ml_available = model_manager::is_available(model_dir);
    if !ml_available {
        // FR-019: emit warning and use rules only
        eprintln!(
            "warning: ML models not available — using rule-based classification only (FR-019)"
        );
    }

    // Resolve the first available .onnx model path (used for every segment)
    let model_path = if ml_available {
        find_model_path(model_dir)
    } else {
        None
    };

    let scale_dummy = make_dummy_scale();
    let elements: Vec<ArchitecturalElement> = segments
        .iter()
        .filter_map(|seg| {
            if seg.points.is_empty() {
                return None;
            }

            // Try ML inference when a model and image are available
            let (et, conf) = if let (Some(ref model), Some(image)) = (&model_path, img) {
                // Crop a bounding-box patch from the full image for this segment
                if let Some(patch) = crop_segment_patch(seg, image) {
                    if let Some(result) = classify_patch(&patch, model) {
                        if result.confidence >= threshold {
                            (result.element_type, result.confidence)
                        } else {
                            // ML confidence below threshold — fall back to rules (FR-007)
                            classify_segment(seg, &scale_dummy)
                        }
                    } else {
                        classify_segment(seg, &scale_dummy)
                    }
                } else {
                    classify_segment(seg, &scale_dummy)
                }
            } else {
                classify_segment(seg, &scale_dummy)
            };

            // Only surface elements above half the adaptive threshold (FR-007)
            if conf >= threshold * 0.5
                || matches!(et, crate::blueprint::element::ElementType::Wall)
            {
                Some(segment_to_element(seg, et, conf, &scale_dummy))
            } else {
                None
            }
        })
        .collect();

    ClassifyResult {
        elements,
        used_fallback: model_path.is_none(),
    }
}

/// Crop an image patch covering the bounding box of `seg`'s points, with 8px padding.
///
/// Returns `None` if the segment has no points or the crop would be zero-sized.
fn crop_segment_patch(seg: &LineSegment, img: &DynamicImage) -> Option<DynamicImage> {
    if seg.points.is_empty() {
        return None;
    }

    let pad = 8u32;
    let min_x = seg.points.iter().map(|p| p.x).min().unwrap_or(0);
    let min_y = seg.points.iter().map(|p| p.y).min().unwrap_or(0);
    let max_x = seg.points.iter().map(|p| p.x).max().unwrap_or(0);
    let max_y = seg.points.iter().map(|p| p.y).max().unwrap_or(0);

    let x0 = min_x.saturating_sub(pad);
    let y0 = min_y.saturating_sub(pad);
    let x1 = (max_x + pad).min(img.width().saturating_sub(1));
    let y1 = (max_y + pad).min(img.height().saturating_sub(1));

    let w = x1.saturating_sub(x0);
    let h = y1.saturating_sub(y0);
    if w == 0 || h == 0 {
        return None;
    }

    Some(img.crop_imm(x0, y0, w, h))
}

/// Return the path of the first `.onnx` file found in the model cache directory.
fn find_model_path(model_dir: Option<&Path>) -> Option<std::path::PathBuf> {
    let dir = model_dir
        .map(|p| p.to_path_buf())
        .or_else(model_manager::default_model_dir)?;
    std::fs::read_dir(&dir)
        .ok()?
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .find(|p| p.extension().and_then(|x| x.to_str()) == Some("onnx"))
}

/// Build a unit-scale `ScaleReference` (1 pixel = 1 meter) for rule-based classification
/// when the actual scale is embedded in `LineSegment.real_world_length`.
fn make_dummy_scale() -> ScaleReference {
    use crate::blueprint::{ImagePoint, LengthUnit};
    ScaleReference::new(
        ImagePoint { x: 0, y: 0 },
        ImagePoint { x: 100, y: 0 },
        100.0,
        LengthUnit::Meters,
        u32::MAX,
        u32::MAX,
    )
    .unwrap()
}

/// Wall-clock timeout for ML classification (FR-028): 5 minutes.
const ML_TIMEOUT_SECS: u64 = 300;

/// Confidence cutoff for keeping ML results on timeout (FR-028): 0.7.
const TIMEOUT_CONFIDENCE_CUTOFF: f32 = 0.7;

/// Classify segments with a 5-minute pipeline timeout (FR-028).
///
/// Processes segments in batches of 20, checking `pipeline_start.elapsed()` after
/// each batch. When elapsed ≥ 300 s, ML is interrupted:
/// - Elements with confidence ≥ 0.7 are kept.
/// - Low-confidence elements and unprocessed segments are re-classified with rules.
///
/// Returns `(elements, timed_out)`.
pub fn classify_with_timeout(
    segments: &[LineSegment],
    img: Option<&DynamicImage>,
    model_dir: Option<&Path>,
    pipeline_start: std::time::Instant,
) -> (Vec<ArchitecturalElement>, bool) {
    const BATCH_SIZE: usize = 20;

    let history = CorrectionHistory::load_or_default().unwrap_or_default();
    let threshold = history.adaptive_threshold;
    let ml_available = model_manager::is_available(model_dir);
    if !ml_available {
        eprintln!(
            "warning: ML models not available — using rule-based classification only (FR-019)"
        );
    }
    let model_path = if ml_available { find_model_path(model_dir) } else { None };
    let scale_dummy = make_dummy_scale();

    let mut ml_elements: Vec<ArchitecturalElement> = Vec::new();
    let mut processed = 0usize;
    let mut timed_out = false;

    'outer: for batch in segments.chunks(BATCH_SIZE) {
        if pipeline_start.elapsed() >= std::time::Duration::from_secs(ML_TIMEOUT_SECS) {
            timed_out = true;
            break 'outer;
        }
        for seg in batch {
            if seg.points.is_empty() {
                processed += 1;
                continue;
            }
            let (et, conf) = if let (Some(ref model), Some(image)) = (&model_path, img) {
                if let Some(patch) = crop_segment_patch(seg, image) {
                    if let Some(result) = classify_patch(&patch, model) {
                        if result.confidence >= threshold {
                            (result.element_type, result.confidence)
                        } else {
                            classify_segment(seg, &scale_dummy)
                        }
                    } else {
                        classify_segment(seg, &scale_dummy)
                    }
                } else {
                    classify_segment(seg, &scale_dummy)
                }
            } else {
                classify_segment(seg, &scale_dummy)
            };
            if conf >= threshold * 0.5
                || matches!(et, crate::blueprint::element::ElementType::Wall)
            {
                ml_elements.push(segment_to_element(seg, et, conf, &scale_dummy));
            }
            processed += 1;
        }
    }

    if !timed_out {
        return (ml_elements, false);
    }

    // Timeout: apply partial fallback for low-confidence results and remaining segments.
    let remaining = &segments[processed..];
    let final_elements = classify_partial(ml_elements, remaining, img, model_dir);
    (final_elements, true)
}

/// Keep high-confidence ML results (≥ 0.7); re-classify the rest with rule-based
/// heuristics. Also classifies any segments not yet processed by ML (FR-028).
pub fn classify_partial(
    ml_elements: Vec<ArchitecturalElement>,
    remaining_segments: &[LineSegment],
    _img: Option<&DynamicImage>,
    _model_dir: Option<&Path>,
) -> Vec<ArchitecturalElement> {
    let scale_dummy = make_dummy_scale();

    // Re-classify low-confidence ML elements with rule-based heuristics.
    let mut elements: Vec<ArchitecturalElement> = ml_elements
        .into_iter()
        .map(|elem| {
            if elem.confidence >= TIMEOUT_CONFIDENCE_CUTOFF {
                elem
            } else {
                use crate::blueprint::element::ElementType;
                // Heuristic re-classification based on element geometry.
                let span_x = elem.bounds.max.x - elem.bounds.min.x;
                let span_y = elem.bounds.max.y - elem.bounds.min.y;
                let len_m = span_x.max(span_y);
                let (et, conf) = if len_m > 2.0 {
                    (ElementType::Wall, 0.5)
                } else if len_m > 0.5 {
                    (ElementType::Door, 0.5)
                } else {
                    (ElementType::Unclassified, 0.4)
                };
                ArchitecturalElement { element_type: et, confidence: conf, ..elem }
            }
        })
        .collect();

    // Classify remaining unprocessed segments with rule-based approach (FR-027 rayon).
    let rule_classified: Vec<ArchitecturalElement> = remaining_segments
        .par_iter()
        .filter_map(|seg| {
            if seg.points.is_empty() {
                return None;
            }
            let (et, conf) = classify_segment(seg, &scale_dummy);
            if conf >= 0.0 || matches!(et, crate::blueprint::element::ElementType::Wall) {
                Some(segment_to_element(seg, et, conf, &scale_dummy))
            } else {
                None
            }
        })
        .collect();

    elements.extend(rule_classified);
    elements
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::blueprint::{scale::LineSegment, ImagePoint};
    use uuid::Uuid;

    fn make_wall_seg() -> LineSegment {
        LineSegment {
            id: Uuid::new_v4(),
            points: (0u32..100).map(|x| ImagePoint { x, y: 50 }).collect(),
            length_pixels: 100.0,
            real_world_length: 1.0,
            wall_spacing: Some(8.0),
        }
    }

    #[test]
    fn classify_empty_input_returns_empty() {
        // Pass an explicit nonexistent dir to force rule-based fallback mode.
        let no_models = std::path::Path::new("/tmp/blueprint2mod_no_models_test");
        let result = classify_verbose(&[], None, Some(no_models));
        assert!(result.elements.is_empty());
        assert!(result.used_fallback);
    }

    #[test]
    fn classify_wall_segment_with_double_line() {
        let seg = make_wall_seg();
        let elements = classify(&[seg], None, None);
        assert!(
            !elements.is_empty(),
            "should classify at least one element from wall segment"
        );
    }

    #[test]
    fn fallback_flag_set_when_no_models() {
        // Use explicit nonexistent dir so the test is independent of installed models.
        let no_models = std::path::Path::new("/tmp/blueprint2mod_no_models_test");
        let result = classify_verbose(&[], None, Some(no_models));
        assert!(
            result.used_fallback,
            "fallback must be true when no ML models present"
        );
    }
}
