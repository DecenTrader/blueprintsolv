/// Integration test: SC-007 — binary + ML models ≤ 1 GB total.
///
/// The release binary is at `target/release/blueprint2mod`.
/// ML models live in `~/.blueprint2mod/models/` when downloaded.
/// Since `download_models()` is currently a no-op stub (no actual models exist),
/// we validate the binary alone and assert total ≤ 1 GB.

/// Maximum allowed size for binary + all ML models combined (SC-007).
const SC007_MAX_BYTES: u64 = 1024 * 1024 * 1024; // 1 GB

/// Maximum allowed size per individual ML model file (SC-007).
const SC007_MAX_MODEL_BYTES: u64 = 100 * 1024 * 1024; // 100 MB

#[test]
fn sc007_binary_plus_models_under_1gb() {
    // Locate the release binary
    let binary_path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("target")
        .join("release")
        .join("blueprint2mod");

    let binary_size = if binary_path.exists() {
        std::fs::metadata(&binary_path)
            .expect("read binary metadata")
            .len()
    } else {
        // Release binary not built yet — skip with warning (CI must run cargo build --release first)
        eprintln!(
            "warning: SC-007 skipped — release binary not found at {}",
            binary_path.display()
        );
        return;
    };

    // Sum ML model sizes
    let mut model_total: u64 = 0;
    if let Some(models_dir) = blueprint2mod::detection::ml::model_manager::default_model_dir() {
        if models_dir.exists() {
            let entries = std::fs::read_dir(&models_dir).expect("read models dir");
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().and_then(|e| e.to_str()) == Some("onnx") {
                    let size = std::fs::metadata(&path)
                        .map(|m| m.len())
                        .unwrap_or(0);
                    assert!(
                        size <= SC007_MAX_MODEL_BYTES,
                        "ML model {} is {} bytes > SC-007 limit of {} bytes",
                        path.display(),
                        size,
                        SC007_MAX_MODEL_BYTES
                    );
                    model_total += size;
                }
            }
        }
    }

    let total = binary_size + model_total;

    assert!(
        total <= SC007_MAX_BYTES,
        "SC-007 FAIL: binary ({:.1} MB) + models ({:.1} MB) = {:.1} MB > 1024 MB",
        binary_size as f64 / (1024.0 * 1024.0),
        model_total as f64 / (1024.0 * 1024.0),
        total as f64 / (1024.0 * 1024.0),
    );

    eprintln!(
        "SC-007 PASS: binary {:.1} MB + models {:.1} MB = {:.1} MB / 1024 MB",
        binary_size as f64 / (1024.0 * 1024.0),
        model_total as f64 / (1024.0 * 1024.0),
        total as f64 / (1024.0 * 1024.0),
    );
}
