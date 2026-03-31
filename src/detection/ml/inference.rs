use std::path::Path;

use tract_onnx::prelude::*;

use crate::blueprint::element::ElementType;

/// Result from running the ONNX classifier on an image patch.
pub struct InferenceResult {
    pub element_type: ElementType,
    pub confidence: f32,
}

/// Run ML inference on a pre-cropped image patch using tract-onnx (FR-004, FR-005).
///
/// Returns `None` if the model is not available or inference fails.
/// Returns `Some(result)` with the best-matching `ElementType` and confidence.
pub fn classify_patch(
    img_patch: &image::DynamicImage,
    model_path: &Path,
) -> Option<InferenceResult> {
    if !model_path.exists() {
        return None;
    }

    // Load and optimize the ONNX model
    let model = tract_onnx::onnx()
        .model_for_path(model_path)
        .ok()?
        .into_optimized()
        .ok()?
        .into_runnable()
        .ok()?;

    // Preprocess: resize to 224×224, normalize to [0,1], convert to NCHW tensor
    let resized = img_patch.resize_exact(224, 224, image::imageops::FilterType::Triangle);
    let rgb = resized.to_rgb8();
    let tensor_data: Vec<f32> = rgb
        .pixels()
        .flat_map(|p| {
            [
                p.0[0] as f32 / 255.0,
                p.0[1] as f32 / 255.0,
                p.0[2] as f32 / 255.0,
            ]
        })
        .collect();

    // NCHW: [1, 3, 224, 224]
    let tensor = tract_ndarray::Array4::from_shape_vec((1, 3, 224, 224), tensor_data).ok()?;
    let input: Tensor = tensor.into();
    let outputs = model.run(tvec!(input.into())).ok()?;
    let output = outputs[0].to_array_view::<f32>().ok()?;

    // Collect output probabilities
    let probs: Vec<f32> = output.iter().copied().collect();
    let n_classes = probs.len();

    // Adapt any output size to our 10 architectural element classes.
    // For MobileNetV2-12 (1000-class ImageNet output), group consecutive classes
    // into 10 buckets and sum probabilities within each bucket (FR-005 adapter).
    let adapted = if n_classes == 10 {
        probs
    } else {
        adapt_to_ten_classes(&probs)
    };

    let (best_idx, &best_conf) = adapted
        .iter()
        .enumerate()
        .max_by(|a, b| a.1.partial_cmp(b.1).unwrap_or(std::cmp::Ordering::Equal))?;

    let element_type = index_to_element_type(best_idx);
    Some(InferenceResult {
        element_type,
        confidence: best_conf,
    })
}

/// Reduce an N-class probability vector to exactly 10 classes by grouping and summing.
///
/// Used as an output adapter for MobileNetV2-12 (N=1000 ImageNet classes → 10
/// architectural element classes). Each of the 10 output buckets accumulates the sum of
/// N/10 consecutive input probabilities. The result is then softmax-normalised so
/// confidence scores remain in [0, 1] and sum to 1.
fn adapt_to_ten_classes(probs: &[f32]) -> Vec<f32> {
    let n = probs.len();
    let mut out = vec![0.0f32; 10];
    for (i, &p) in probs.iter().enumerate() {
        // Map source class index i → bucket in [0, 9]
        let bucket = (i * 10 / n).min(9);
        out[bucket] += p;
    }
    // Softmax-normalise to keep confidences well-scaled
    let max = out.iter().cloned().fold(f32::NEG_INFINITY, f32::max);
    let exp_sum: f32 = out.iter().map(|&x| (x - max).exp()).sum();
    out.iter().map(|&x| (x - max).exp() / exp_sum).collect()
}

/// Map model output index to `ElementType`.
///
/// Indices match the training label order:
/// 0=Wall, 1=Door, 2=Window, 3=SlidingDoor, 4=Fireplace, 5=Closet,
/// 6=Staircase, 7=Chimney, 8=Courtyard, 9=Unclassified
fn index_to_element_type(idx: usize) -> ElementType {
    match idx {
        0 => ElementType::Wall,
        1 => ElementType::Door,
        2 => ElementType::Window,
        3 => ElementType::SlidingDoor,
        4 => ElementType::Fireplace,
        5 => ElementType::Closet,
        6 => ElementType::Staircase,
        7 => ElementType::Chimney,
        8 => ElementType::Courtyard,
        _ => ElementType::Unclassified,
    }
}
