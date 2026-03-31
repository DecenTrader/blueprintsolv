use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use crate::blueprint::element::ElementType;

#[derive(Debug, Serialize, Deserialize)]
pub struct CorrectionHistory {
    pub version: String,
    /// Adaptive confidence threshold in [0.0, 1.0]. Elements below this are surfaced for review.
    pub adaptive_threshold: f32,
    pub total_corrections: u32,
    pub last_updated: String,
    pub corrections: Vec<CorrectionEntry>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CorrectionEntry {
    pub timestamp: String,
    pub original_type: ElementType,
    pub corrected_type: ElementType,
    pub original_confidence: f32,
}

impl CorrectionHistory {
    const VERSION: &'static str = "1.0";
    const DEFAULT_THRESHOLD: f32 = 0.75;

    pub fn new() -> Self {
        Self {
            version: Self::VERSION.to_string(),
            adaptive_threshold: Self::DEFAULT_THRESHOLD,
            total_corrections: 0,
            last_updated: chrono::Utc::now().to_rfc3339(),
            corrections: Vec::new(),
        }
    }

    /// Path to the global corrections file: `~/.blueprint2mod/corrections.json`.
    pub fn default_path() -> Option<std::path::PathBuf> {
        dirs::home_dir().map(|h| h.join(".blueprint2mod").join("corrections.json"))
    }

    /// Load from disk, or return a fresh history if the file does not exist.
    pub fn load_or_default() -> Result<Self> {
        let path =
            Self::default_path().context("Cannot determine home directory for corrections file")?;
        if !path.exists() {
            return Ok(Self::new());
        }
        let data = std::fs::read_to_string(&path)
            .with_context(|| format!("Failed to read corrections file: {}", path.display()))?;
        let history: Self = serde_json::from_str(&data)
            .with_context(|| format!("Failed to parse corrections file: {}", path.display()))?;
        Ok(history)
    }

    /// Persist to `~/.blueprint2mod/corrections.json`.
    pub fn save(&self) -> Result<()> {
        let path =
            Self::default_path().context("Cannot determine home directory for corrections file")?;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("Failed to create directory: {}", parent.display()))?;
        }
        let json = serde_json::to_string_pretty(self)?;
        std::fs::write(&path, json)
            .with_context(|| format!("Failed to write corrections file: {}", path.display()))?;
        Ok(())
    }

    /// Record a user correction and update the adaptive threshold.
    ///
    /// The threshold is recomputed as the mean `original_confidence` of all correction entries,
    /// then clamped to [0.5, 0.95] to keep useful defaults.
    pub fn record_correction(
        &mut self,
        original_type: ElementType,
        corrected_type: ElementType,
        original_confidence: f32,
    ) {
        self.corrections.push(CorrectionEntry {
            timestamp: chrono::Utc::now().to_rfc3339(),
            original_type,
            corrected_type,
            original_confidence,
        });
        self.total_corrections += 1;
        self.last_updated = chrono::Utc::now().to_rfc3339();
        self.recompute_threshold();
    }

    /// EMA smoothing factor α (FR-007). Higher α → faster adaptation.
    const EMA_ALPHA: f32 = 0.3;

    fn recompute_threshold(&mut self) {
        if self.corrections.is_empty() {
            self.adaptive_threshold = Self::DEFAULT_THRESHOLD;
            return;
        }
        // Exponential moving average over all correction confidences (FR-007).
        // Seed with the first correction, then apply EMA for subsequent ones.
        let mut ema = self.corrections[0].original_confidence;
        for entry in &self.corrections[1..] {
            ema = Self::EMA_ALPHA * entry.original_confidence + (1.0 - Self::EMA_ALPHA) * ema;
        }
        self.adaptive_threshold = ema.clamp(0.5, 0.95);
    }
}

impl Default for CorrectionHistory {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_history_has_default_threshold() {
        let h = CorrectionHistory::new();
        assert_eq!(h.adaptive_threshold, CorrectionHistory::DEFAULT_THRESHOLD);
        assert_eq!(h.total_corrections, 0);
        assert!(h.corrections.is_empty());
    }

    #[test]
    fn threshold_adapts_after_corrections() {
        let mut h = CorrectionHistory::new();
        h.record_correction(ElementType::Unclassified, ElementType::Wall, 0.60);
        h.record_correction(ElementType::Unclassified, ElementType::Door, 0.65);
        // EMA: ema_1 = 0.60; ema_2 = 0.3 * 0.65 + 0.7 * 0.60 = 0.195 + 0.42 = 0.615
        // clamped to [0.5, 0.95] → 0.615
        assert!((h.adaptive_threshold - 0.615).abs() < 1e-5);
        assert_eq!(h.total_corrections, 2);
    }

    #[test]
    fn threshold_clamped_at_minimum() {
        let mut h = CorrectionHistory::new();
        // Very low confidence corrections should not drive threshold below 0.5
        for _ in 0..10 {
            h.record_correction(ElementType::Unclassified, ElementType::Wall, 0.10);
        }
        assert!(h.adaptive_threshold >= 0.5);
    }

    #[test]
    fn threshold_clamped_at_maximum() {
        let mut h = CorrectionHistory::new();
        // Very high confidence corrections should not exceed 0.95
        for _ in 0..10 {
            h.record_correction(ElementType::Unclassified, ElementType::Wall, 0.99);
        }
        assert!(h.adaptive_threshold <= 0.95);
    }

    /// T034: after 20 identical corrections the EMA threshold converges close to the
    /// correction confidence (clamped to [0.5, 0.95]).
    #[test]
    fn threshold_converges_over_twenty_corrections() {
        let mut h = CorrectionHistory::new();
        let correction_confidence = 0.62_f32;
        for i in 0..20u32 {
            // Alternate between two element types to avoid trivial patterns
            let orig = if i % 2 == 0 {
                ElementType::Unclassified
            } else {
                ElementType::Door
            };
            h.record_correction(orig, ElementType::Wall, correction_confidence);
        }
        assert_eq!(h.total_corrections, 20);
        // EMA with α=0.3 applied 20 times to constant 0.62 converges very close to 0.62.
        // The EMA approaches the steady state: threshold → 0.62 as n → ∞.
        // After 20 steps the error is < 0.62 × (0.7^20) ≈ 0.62 × 0.0008 ≈ 0.001.
        let target = correction_confidence.clamp(0.5, 0.95);
        assert!(
            (h.adaptive_threshold - target).abs() < 0.02,
            "EMA threshold {:.4} should be within 0.02 of {:.4} after 20 corrections",
            h.adaptive_threshold,
            target
        );
    }

    /// T034: save/load round-trip preserves threshold, count, and entries.
    #[test]
    fn save_load_roundtrip_preserves_state() {
        let mut h = CorrectionHistory::new();
        h.record_correction(ElementType::Unclassified, ElementType::Wall, 0.70);
        h.record_correction(ElementType::Door, ElementType::Window, 0.65);

        // Serialize to JSON and back (in-memory, avoids filesystem side-effects)
        let json = serde_json::to_string(&h).expect("serialization should succeed");
        let loaded: CorrectionHistory =
            serde_json::from_str(&json).expect("deserialization should succeed");

        assert_eq!(loaded.total_corrections, h.total_corrections);
        assert_eq!(loaded.corrections.len(), h.corrections.len());
        assert!(
            (loaded.adaptive_threshold - h.adaptive_threshold).abs() < 1e-6,
            "threshold must survive round-trip"
        );
        assert_eq!(loaded.version, h.version);
    }
}
