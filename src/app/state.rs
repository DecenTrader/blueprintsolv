/// Application state machine.
///
/// Valid transitions:
///   Welcome → Cropping → ImageLoaded → Scaled → Analyzing → Analyzed → Clarifying → ModelReady → Exported
///
/// Save is available from `Scaled` onward.
/// Load from `Welcome` returns to whatever state the session was saved from.
#[derive(Debug, Clone, PartialEq)]
pub enum AppState {
    /// No image loaded; file picker or CLI arg expected.
    Welcome,
    /// Image loaded; user may drag a crop region or skip — optional step (FR-024).
    Cropping,
    /// Crop step complete (or skipped); user prompted to set scale reference points.
    ImageLoaded,
    /// Scale reference confirmed; ready to run detection pipeline.
    Scaled,
    /// Detection pipeline running (line tracing + classification + OCR).
    Analyzing,
    /// Detection complete; clarifications may be pending.
    Analyzed,
    /// One or more pending clarifications being resolved by user.
    Clarifying,
    /// All clarifications resolved; 3D generation and export available.
    ModelReady,
    /// OBJ or STL file written; processing summary displayed.
    Exported,
}

impl AppState {
    /// Returns `true` if transitioning to `next` is valid from the current state.
    pub fn can_transition_to(&self, next: &AppState) -> bool {
        use AppState::*;
        matches!(
            (self, next),
            (Welcome, Cropping)
                | (Cropping, ImageLoaded)
                | (ImageLoaded, Scaled)
                | (Scaled, Analyzing)
                | (Analyzing, Analyzed)
                | (Analyzed, Clarifying)
                | (Analyzed, ModelReady)
                | (Clarifying, Clarifying)
                | (Clarifying, ModelReady)
                | (ModelReady, Exported)
                | (ModelReady, Analyzing) // re-run detection
                | (Exported, Analyzing) // re-export
        )
    }

    /// Returns `true` if session save is permitted in this state (FR-016).
    pub fn can_save(&self) -> bool {
        use AppState::*;
        matches!(
            self,
            Scaled | Analyzing | Analyzed | Clarifying | ModelReady | Exported
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn valid_transitions() {
        assert!(AppState::Welcome.can_transition_to(&AppState::Cropping));
        assert!(AppState::Cropping.can_transition_to(&AppState::ImageLoaded));
        assert!(AppState::ImageLoaded.can_transition_to(&AppState::Scaled));
        assert!(AppState::Scaled.can_transition_to(&AppState::Analyzing));
        assert!(AppState::Analyzed.can_transition_to(&AppState::Clarifying));
        assert!(AppState::Analyzed.can_transition_to(&AppState::ModelReady));
        assert!(AppState::ModelReady.can_transition_to(&AppState::Exported));
    }

    #[test]
    fn invalid_transitions() {
        assert!(!AppState::Welcome.can_transition_to(&AppState::Exported));
        assert!(!AppState::Exported.can_transition_to(&AppState::Welcome));
        assert!(!AppState::ImageLoaded.can_transition_to(&AppState::Analyzed));
        assert!(!AppState::Welcome.can_transition_to(&AppState::ImageLoaded));
    }

    #[test]
    fn save_permitted_from_scaled_onward() {
        assert!(!AppState::Welcome.can_save());
        assert!(!AppState::ImageLoaded.can_save());
        assert!(AppState::Scaled.can_save());
        assert!(AppState::Analyzed.can_save());
        assert!(AppState::Exported.can_save());
    }
}
