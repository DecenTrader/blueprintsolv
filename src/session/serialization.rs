use std::path::Path;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::blueprint::{
    element::{ArchitecturalElement, ElementType},
    floor_plan::FloorPlan,
    image::BlueprintImage,
    scale::ScaleReference,
};
use crate::ocr::extractor::TextAnnotation;

pub use crate::blueprint::element::ArchitecturalElement as _ArchElem; // re-export convenience

/// A pending clarification presented to the user during the `Clarifying` state (FR-007, FR-008).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PendingClarification {
    pub element_id: Uuid,
    pub suggested_types: Vec<ElementType>,
    /// Human-readable context shown in the clarification UI.
    pub context_snippet: String,
}

/// Full session state persisted to a `.b2m` JSON file (FR-016, FR-017).
///
/// New optional fields must use `#[serde(default)]` for forward compatibility.
#[derive(Debug, Serialize, Deserialize)]
pub struct Session {
    /// Schema version — always `"1.0"` for this implementation.
    pub version: String,
    /// ISO 8601 UTC timestamp when this session was first created.
    pub created_at: String,
    /// ISO 8601 UTC timestamp of the last save.
    pub last_saved_at: String,
    /// Blueprint image metadata (path, dimensions, format). Raw pixels are NOT stored.
    pub image: BlueprintImage,
    /// `None` until the user completes the scale reference step.
    pub scale: Option<ScaleReference>,
    /// Detected line segments from the tracing pipeline.
    #[serde(default)]
    pub line_segments: Vec<crate::blueprint::scale::LineSegment>,
    /// Classified architectural elements.
    #[serde(default)]
    pub elements: Vec<ArchitecturalElement>,
    /// Assembled floor plan (rooms, interior/exterior inference).
    #[serde(default)]
    pub floor_plan: Option<FloorPlan>,
    /// OCR-detected text annotations (room labels, dimensions).
    #[serde(default)]
    pub text_annotations: Vec<TextAnnotation>,
    /// Elements waiting for user type confirmation.
    #[serde(default)]
    pub pending_clarifications: Vec<PendingClarification>,
    /// User-confirmed extrusion height; `None` until confirmed in export step.
    #[serde(default)]
    pub wall_height_m: Option<f64>,
    /// Optional crop region applied to the blueprint image before processing (FR-024).
    #[serde(default)]
    pub crop_region: Option<crate::blueprint::CropRegion>,
}

impl Session {
    const VERSION: &'static str = "1.0";

    pub fn new(image: BlueprintImage) -> Self {
        let now = chrono::Utc::now().to_rfc3339();
        Self {
            version: Self::VERSION.to_string(),
            created_at: now.clone(),
            last_saved_at: now,
            image,
            scale: None,
            line_segments: Vec::new(),
            elements: Vec::new(),
            floor_plan: None,
            text_annotations: Vec::new(),
            pending_clarifications: Vec::new(),
            wall_height_m: None,
            crop_region: None,
        }
    }

    /// Serialize to JSON and write to `path`. The file extension must be `.b2m` (enforced by
    /// the UI dialog, not here — callers are responsible for the correct extension).
    pub fn save(&mut self, path: &Path) -> Result<()> {
        self.last_saved_at = chrono::Utc::now().to_rfc3339();
        let json = serde_json::to_string_pretty(self).context("Failed to serialize session")?;
        std::fs::write(path, json)
            .with_context(|| format!("Failed to write session to {}", path.display()))?;
        Ok(())
    }

    /// Load image pixels with `crop_region` automatically applied (FR-024).
    ///
    /// All pipeline stages MUST use this method instead of calling `image.load_pixels()`
    /// directly, to guarantee that the original image is never processed when a crop is active.
    /// `render_image_with_crop_drag` is the only intentional exception (it shows the full
    /// image so the user can draw a new crop selection).
    pub fn load_working_image(&self) -> Result<image::DynamicImage> {
        let raw = self.image.load_pixels()?;
        Ok(match self.crop_region {
            Some(crop) => raw.crop_imm(crop.x, crop.y, crop.width, crop.height),
            None => raw,
        })
    }

    /// Effective (post-crop) image dimensions. Use for bounds validation wherever
    /// pixel coordinates are expected to lie within the working image, not the original.
    pub fn working_image_size(&self) -> (u32, u32) {
        match self.crop_region {
            Some(crop) => (crop.width, crop.height),
            None => (self.image.width, self.image.height),
        }
    }

    /// Deserialize a session from a `.b2m` JSON file.
    pub fn load(path: &Path) -> Result<Self> {
        let data = std::fs::read_to_string(path)
            .with_context(|| format!("Failed to read session file: {}", path.display()))?;
        let session: Self = serde_json::from_str(&data)
            .with_context(|| format!("Failed to parse session file: {}", path.display()))?;
        Ok(session)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::blueprint::CropRegion;

    fn make_session_from_fixture() -> Option<Session> {
        let fixture = std::path::Path::new("test_fixtures/simple_rectangle.jpg");
        let img = crate::blueprint::image::BlueprintImage::load(fixture).ok()?;
        Some(Session::new(img))
    }

    /// T072: load_working_image() must return cropped dimensions when a crop_region is set.
    #[test]
    fn load_working_image_applies_crop() {
        let mut session = match make_session_from_fixture() {
            Some(s) => s,
            None => return, // fixture missing — skip (CI environment check)
        };

        let full = session.load_working_image().expect("full image loads");
        let orig_w = full.width();
        let orig_h = full.height();

        // Apply a crop: remove a 10-pixel border on each side.
        let pad = 10u32;
        session.crop_region = Some(CropRegion {
            x: pad,
            y: pad,
            width: orig_w.saturating_sub(pad * 2),
            height: orig_h.saturating_sub(pad * 2),
        });

        let cropped = session.load_working_image().expect("cropped image loads");
        assert_eq!(
            cropped.width(),
            orig_w - pad * 2,
            "cropped width must be (orig - 2*pad)"
        );
        assert_eq!(
            cropped.height(),
            orig_h - pad * 2,
            "cropped height must be (orig - 2*pad)"
        );
        assert_ne!(
            cropped.width(),
            orig_w,
            "cropped image must not equal original width"
        );

        // working_image_size() must agree with the actual image dimensions.
        let (ws_w, ws_h) = session.working_image_size();
        assert_eq!(ws_w, cropped.width());
        assert_eq!(ws_h, cropped.height());
    }

    /// Regression: without a crop, load_working_image() returns the full original image.
    #[test]
    fn load_working_image_without_crop_returns_original() {
        let session = match make_session_from_fixture() {
            Some(s) => s,
            None => return,
        };
        assert!(session.crop_region.is_none());
        let working = session.load_working_image().expect("loads");
        assert_eq!(working.width(), session.image.width);
        assert_eq!(working.height(), session.image.height);
        let (ws_w, ws_h) = session.working_image_size();
        assert_eq!(ws_w, session.image.width);
        assert_eq!(ws_h, session.image.height);
    }
}
