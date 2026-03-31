use std::path::{Path, PathBuf};

/// Maximum individual model size allowed by SC-007 (100 MB).
pub const MAX_MODEL_SIZE_BYTES: u64 = 100 * 1024 * 1024;

/// Default cache directory for ONNX models: `~/.blueprint2mod/models/`.
pub fn default_model_dir() -> Option<PathBuf> {
    dirs::home_dir().map(|h| h.join(".blueprint2mod").join("models"))
}

/// Returns `true` if the model cache directory exists and contains at least one `.onnx` file.
///
/// SC-007: total model budget ≤ 700 MB (7 × 100 MB). Individual files > 100 MB are rejected.
pub fn is_available(model_dir: Option<&Path>) -> bool {
    let dir = match model_dir
        .map(|p| p.to_path_buf())
        .or_else(default_model_dir)
    {
        Some(d) => d,
        None => return false,
    };
    if !dir.exists() {
        return false;
    }
    std::fs::read_dir(&dir)
        .map(|entries| {
            entries
                .filter_map(|e| e.ok())
                .any(|e| e.path().extension().and_then(|x| x.to_str()) == Some("onnx"))
        })
        .unwrap_or(false)
}

/// MobileNetV2-12 from ONNX Model Zoo (Hugging Face mirror). ~13 MB, opset 12.
/// Output adapter maps 1000 ImageNet classes → 10 architectural element classes.
const MOBILENETV2_URL: &str =
    "https://huggingface.co/onnxmodelzoo/mobilenetv2-12/resolve/main/mobilenetv2-12.onnx";
const MOBILENETV2_FILENAME: &str = "mobilenetv2-12.onnx";

/// Download ML models to the cache directory on first run (FR-018).
///
/// Skips any model file that already exists. Rejects files > `MAX_MODEL_SIZE_BYTES`.
/// Returns `Ok(())` on success, `Err` on download failure. The caller must handle
/// the error by falling back to rule-based mode (FR-019).
pub fn download_models(model_dir: Option<&Path>) -> anyhow::Result<()> {
    let dir = model_dir
        .map(|p| p.to_path_buf())
        .or_else(default_model_dir)
        .ok_or_else(|| anyhow::anyhow!("Cannot determine model cache directory"))?;
    std::fs::create_dir_all(&dir)?;

    let dest = dir.join(MOBILENETV2_FILENAME);
    if dest.exists() {
        return Ok(()); // already downloaded
    }

    eprintln!("Downloading ML model: {} …", MOBILENETV2_FILENAME);

    let response = reqwest::blocking::get(MOBILENETV2_URL)
        .and_then(|r| r.error_for_status())
        .map_err(|e| anyhow::anyhow!("Model download failed: {}", e))?;

    let bytes = response
        .bytes()
        .map_err(|e| anyhow::anyhow!("Failed to read model bytes: {}", e))?;

    if bytes.len() as u64 > MAX_MODEL_SIZE_BYTES {
        anyhow::bail!(
            "Downloaded model exceeds size limit ({} MB > {} MB)",
            bytes.len() / 1024 / 1024,
            MAX_MODEL_SIZE_BYTES / 1024 / 1024
        );
    }

    std::fs::write(&dest, &bytes)
        .map_err(|e| anyhow::anyhow!("Failed to save model to {}: {}", dest.display(), e))?;

    eprintln!("Model saved to {}", dest.display());
    Ok(())
}
