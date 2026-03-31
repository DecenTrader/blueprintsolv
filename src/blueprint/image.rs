use std::path::{Path, PathBuf};

use anyhow::{bail, Result};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum ImageFormat {
    Jpeg,
    Png,
}

/// Metadata for the loaded blueprint image. Raw pixel data is NOT stored here — it is
/// loaded on demand via `load_pixels()` (FR-001).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlueprintImage {
    pub path: PathBuf,
    pub width: u32,
    pub height: u32,
    pub format: ImageFormat,
}

impl BlueprintImage {
    /// Open the image at `path`, validate the format, and return metadata.
    ///
    /// Only `.jpg`, `.jpeg`, and `.png` are accepted (FR-001). Returns a typed error for
    /// unsupported formats.
    pub fn load(path: &Path) -> Result<Self> {
        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_ascii_lowercase();
        let format = match ext.as_str() {
            "jpg" | "jpeg" => ImageFormat::Jpeg,
            "png" => ImageFormat::Png,
            other => bail!(
                "Unsupported image format '.{}'. Only JPG and PNG are supported (FR-001).",
                other
            ),
        };
        let img = image::open(path)
            .map_err(|e| anyhow::anyhow!("Failed to open image '{}': {}", path.display(), e))?;
        Ok(Self {
            path: path.to_path_buf(),
            width: img.width(),
            height: img.height(),
            format,
        })
    }

    /// Load raw pixel data from disk (used by the detection pipeline; not cached).
    pub fn load_pixels(&self) -> Result<image::DynamicImage> {
        image::open(&self.path)
            .map_err(|e| anyhow::anyhow!("Failed to reload image '{}': {}", self.path.display(), e))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn load_rejects_unsupported_format() {
        let path = PathBuf::from("foo.bmp");
        let result = BlueprintImage::load(&path);
        assert!(result.is_err());
        let msg = result.unwrap_err().to_string();
        assert!(msg.contains("Unsupported image format"));
    }

    #[test]
    fn load_rejects_no_extension() {
        let path = PathBuf::from("no_extension");
        let result = BlueprintImage::load(&path);
        assert!(result.is_err());
    }

    #[test]
    fn load_accepts_jpg_extension() {
        // Non-existent file — format detection should happen before file I/O
        // In this implementation image::open fails after format check passes.
        // A real file path is tested in integration tests.
        let path = PathBuf::from("test.jpg");
        // We only check that the error is NOT about format (it should be about missing file)
        let result = BlueprintImage::load(&path);
        if let Err(e) = result {
            assert!(
                !e.to_string().contains("Unsupported image format"),
                "Should not fail on format for .jpg"
            );
        }
    }

    #[test]
    fn load_accepts_png_extension() {
        let path = PathBuf::from("test.png");
        let result = BlueprintImage::load(&path);
        if let Err(e) = result {
            assert!(
                !e.to_string().contains("Unsupported image format"),
                "Should not fail on format for .png"
            );
        }
    }

    #[test]
    fn load_rejects_uppercase_bmp() {
        let path = PathBuf::from("blueprint.BMP");
        assert!(BlueprintImage::load(&path).is_err());
    }
}
