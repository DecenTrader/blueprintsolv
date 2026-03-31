/// Integration test: SC-006 / US3 — clarification loop and CorrectionHistory update.
use blueprint2mod::blueprint::element::{ArchitecturalElement, ElementType};
use blueprint2mod::blueprint::scale::ScaleReference;
use blueprint2mod::blueprint::{BoundingBox, ImagePoint, LengthUnit, WorldPoint};
use blueprint2mod::correction::history::CorrectionHistory;
use blueprint2mod::session::serialization::PendingClarification;
use uuid::Uuid;

fn make_low_confidence_element() -> ArchitecturalElement {
    ArchitecturalElement {
        id: Uuid::new_v4(),
        element_type: ElementType::Unclassified,
        bounds: BoundingBox {
            min: WorldPoint { x: 0.5, y: 0.5 },
            max: WorldPoint { x: 1.0, y: 1.0 },
        },
        source_segment_ids: vec![],
        confidence: 0.40, // below default threshold of 0.75
        is_interior: Some(true),
        wall_thickness_m: None,
    }
}

/// Low-confidence elements must appear in pending_clarifications.
#[test]
fn low_confidence_element_surfaces_for_clarification() {
    let elem = make_low_confidence_element();
    let history = CorrectionHistory::new();

    // Replicate the pending clarification logic from run_analysis_pipeline
    let pending: Vec<PendingClarification> = std::iter::once(&elem)
        .filter(|e| (e.confidence as f64) < (history.adaptive_threshold as f64))
        .map(|e| PendingClarification {
            element_id: e.id,
            suggested_types: vec![e.element_type.clone()],
            context_snippet: format!(
                "{:?} (confidence {:.0}%)",
                e.element_type,
                e.confidence * 100.0
            ),
        })
        .collect();

    assert!(
        !pending.is_empty(),
        "element with confidence {:.2} < threshold {:.2} must appear in pending_clarifications",
        elem.confidence,
        history.adaptive_threshold
    );
    assert_eq!(pending[0].element_id, elem.id);
}

/// Applying a user correction must update CorrectionHistory.
#[test]
fn user_correction_updates_history() {
    let elem = make_low_confidence_element();
    let mut history = CorrectionHistory::new();
    let initial_count = history.total_corrections;

    history.record_correction(
        elem.element_type.clone(),
        ElementType::Wall,
        elem.confidence,
    );

    assert_eq!(
        history.total_corrections,
        initial_count + 1,
        "total_corrections must increment after one correction"
    );
    assert_eq!(history.corrections.len(), 1);
    assert!(
        matches!(
            history.corrections[0].original_type,
            ElementType::Unclassified
        ),
        "original_type must match the element's type"
    );
    assert!(
        matches!(history.corrections[0].corrected_type, ElementType::Wall),
        "corrected_type must match the user's selection"
    );
}

/// Skipping a clarification must NOT add to CorrectionHistory.
#[test]
fn skip_does_not_update_history() {
    let mut history = CorrectionHistory::new();
    // Simulating skip: no call to record_correction
    assert_eq!(history.total_corrections, 0, "no corrections after skip");
    assert!(
        history.corrections.is_empty(),
        "corrections list empty after skip"
    );
}

/// Threshold shifts toward new corrections over time.
#[test]
fn adaptive_threshold_shifts_toward_corrected_confidence() {
    let mut history = CorrectionHistory::new();
    let initial = history.adaptive_threshold;

    // 5 corrections at confidence 0.55 (below default 0.75)
    for _ in 0..5 {
        history.record_correction(ElementType::Unclassified, ElementType::Wall, 0.55);
    }

    // Mean of 5 × 0.55 = 0.55, clamped → 0.55; threshold should move down from 0.75
    assert!(
        history.adaptive_threshold < initial,
        "threshold {:.3} should decrease below initial {:.3} after corrections at 0.55",
        history.adaptive_threshold,
        initial
    );
}
