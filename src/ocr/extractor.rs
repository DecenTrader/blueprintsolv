use image::DynamicImage;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::blueprint::{ImageBoundingBox, LengthUnit};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum KnownRoomType {
    Bedroom,
    Kitchen,
    Bathroom,
    LivingRoom,
    DiningRoom,
    Garage,
    Hallway,
    Study,
    Laundry,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum TextAnnotationType {
    RoomLabel(KnownRoomType),
    /// Text recognized but not in the known-room-type list.
    RoomLabelUnknown,
    DimensionValue {
        value: f64,
        unit: LengthUnit,
    },
    /// OCR confidence below threshold; included in summary (FR-023).
    Unreadable,
}

/// A text region detected by the OCR pipeline (FR-020–FR-023).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TextAnnotation {
    pub id: Uuid,
    pub raw_text: String,
    pub annotation_type: TextAnnotationType,
    pub image_bounds: ImageBoundingBox,
    /// OCR confidence score in [0.0, 1.0].
    pub confidence: f32,
}

/// Raw OCR result before classification (output of `OcrExtractor::extract`).
#[derive(Debug, Clone)]
pub struct RawOcrItem {
    pub text: String,
    pub bounds: ImageBoundingBox,
    pub confidence: f32,
}

/// Wraps Tesseract OCR via `leptess` (FR-020).
pub struct OcrExtractor;

impl OcrExtractor {
    pub fn new() -> Self {
        Self
    }

    /// Run OCR on `img` and return raw text items with bounding boxes and confidence.
    ///
    /// Uses Tesseract with English language data. Preprocesses the image (binarize)
    /// before recognition (FR-020).
    ///
    /// Strategy:
    /// 1. Binarize the image via threshold.
    /// 2. Write to a temp PNG for leptess.
    /// 3. Get the full text via `get_utf8_text()` and overall confidence via
    ///    `mean_text_conf()`.
    /// 4. Get word-level bounding boxes via `get_component_boxes(RIL_WORD)`.
    /// 5. Split text into words and zip with boxes (best-effort alignment).
    pub fn extract(&self, img: &DynamicImage) -> anyhow::Result<Vec<RawOcrItem>> {
        use leptess::LepTess;

        let (width, height) = (img.width(), img.height());
        if width == 0 || height == 0 {
            return Ok(Vec::new());
        }

        // Preprocess: grayscale + binarize for better OCR accuracy (FR-020)
        let gray = img.to_luma8();
        let binarized =
            imageproc::contrast::threshold(&gray, 128, imageproc::contrast::ThresholdType::Binary);

        // Write to a temp PNG for leptess
        let tmp = tempfile::NamedTempFile::with_suffix(".png")?;
        image::DynamicImage::ImageLuma8(binarized).save(tmp.path())?;

        let mut api = LepTess::new(None, "eng")
            .map_err(|e| anyhow::anyhow!("Tesseract init failed: {:?}", e))?;
        api.set_image(tmp.path())
            .map_err(|e| anyhow::anyhow!("Failed to set OCR image: {:?}", e))?;

        // Get full text and overall confidence
        let full_text = api.get_utf8_text().unwrap_or_default();
        let confidence = (api.mean_text_conf() as f32 / 100.0).clamp(0.0, 1.0);

        let words: Vec<&str> = full_text.split_whitespace().collect();
        if words.is_empty() {
            return Ok(Vec::new());
        }

        // Get word-level bounding boxes via Boxa iteration
        let boxa = api.get_component_boxes(leptess::capi::TessPageIteratorLevel_RIL_WORD, true);

        let mut items = Vec::new();

        if let Some(boxes) = boxa {
            let box_list: Vec<_> = boxes.into_iter().collect();
            let n = box_list.len().min(words.len());
            for i in 0..n {
                let geom = box_list[i].get_geometry();
                let bounds = ImageBoundingBox {
                    x: geom.x.max(0) as u32,
                    y: geom.y.max(0) as u32,
                    width: geom.w.max(0) as u32,
                    height: geom.h.max(0) as u32,
                };
                items.push(RawOcrItem {
                    text: words[i].to_string(),
                    bounds,
                    confidence,
                });
            }
            // Remaining words without matching boxes
            for w in &words[n..] {
                items.push(RawOcrItem {
                    text: w.to_string(),
                    bounds: ImageBoundingBox {
                        x: 0,
                        y: 0,
                        width,
                        height,
                    },
                    confidence,
                });
            }
        } else {
            // No word boxes available; emit words with full-image bounds
            for w in &words {
                items.push(RawOcrItem {
                    text: w.to_string(),
                    bounds: ImageBoundingBox {
                        x: 0,
                        y: 0,
                        width,
                        height,
                    },
                    confidence,
                });
            }
        }

        Ok(items)
    }
}

impl Default for OcrExtractor {
    fn default() -> Self {
        Self::new()
    }
}
