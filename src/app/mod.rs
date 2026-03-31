pub mod state;
pub mod ui;

use eframe::egui;

use std::path::PathBuf;
use std::sync::mpsc;
use std::time::Instant;

use crate::blueprint::image::BlueprintImage;
use crate::blueprint::ImagePoint;
use crate::detection::ml::model_manager;
use crate::session::serialization::Session;

// ── Progress indicator types (T052) ─────────────────────────────────────────

/// Named stages of the detection pipeline, shown in the UI during analysis.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PipelineStage {
    Ocr,
    Trace,
    Classify,
    FloorPlan,
}

impl PipelineStage {
    pub const TOTAL: usize = 4;

    pub fn index(self) -> usize {
        match self {
            Self::Ocr => 0,
            Self::Trace => 1,
            Self::Classify => 2,
            Self::FloorPlan => 3,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Ocr => "Step 1/4: Running OCR & masking text…",
            Self::Trace => "Step 2/4: Denoising & tracing lines…",
            Self::Classify => "Step 3/4: Classifying elements…",
            Self::FloorPlan => "Step 4/4: Building floor plan…",
        }
    }

    /// Estimated wall-clock duration for progress bar animation (seconds).
    /// Trace is budgeted at 8 s to account for NLM denoising (FR-025).
    pub fn estimated_secs(self) -> f32 {
        match self {
            Self::Ocr => 5.0,
            Self::Trace => 8.0,
            Self::Classify => 3.0,
            Self::FloorPlan => 0.5,
        }
    }
}

/// Results sent from background threads back to the main thread.
pub enum StageResult {
    OcrDone {
        raw_ocr: Vec<crate::ocr::extractor::RawOcrItem>,
        masked_img: image::DynamicImage,
    },
    TraceDone {
        segments: Vec<crate::blueprint::scale::LineSegment>,
    },
    ClassifyDone {
        elements: Vec<crate::blueprint::element::ArchitecturalElement>,
        /// `true` when the 5-minute ML timeout (FR-028) fired during this stage.
        timed_out: bool,
    },
    FloorPlanDone {
        floor_plan: Option<crate::blueprint::floor_plan::FloorPlan>,
        annotations: Vec<crate::ocr::extractor::TextAnnotation>,
        pending: Vec<crate::session::serialization::PendingClarification>,
    },
    StageFailed(String),
}

/// Transient state for the in-progress analysis pipeline.
pub struct AnalysisState {
    pub stage: PipelineStage,
    pub stage_started: Instant,
    /// Wall-clock time when "Analyze" was clicked — used for FR-028 timeout (T062).
    pub pipeline_start: Instant,
    pub result_rx: mpsc::Receiver<StageResult>,
    // Accumulated data passed between stages:
    pub base_img: std::sync::Arc<image::DynamicImage>,
    pub scale: crate::blueprint::scale::ScaleReference,
    pub raw_ocr: Vec<crate::ocr::extractor::RawOcrItem>,
    pub masked_img: Option<std::sync::Arc<image::DynamicImage>>,
    pub segments: Vec<crate::blueprint::scale::LineSegment>,
    pub elements: Vec<crate::blueprint::element::ArchitecturalElement>,
    /// Set to `true` when the 5-minute ML timeout fires (FR-028).
    pub ml_timed_out: bool,
}

/// Export format selected via CLI `--format` flag.
#[derive(Debug, Clone, PartialEq)]
pub enum ExportFormat {
    Obj,
    Stl,
}

/// Transient state for the optional image-crop drag interaction (FR-024).
#[derive(Debug, Default)]
pub struct CropUiState {
    /// Drag start in image pixel coordinates (set when pointer pressed on image).
    pub start_px: Option<(u32, u32)>,
    /// Drag end in image pixel coordinates (updated as pointer moves).
    pub end_px: Option<(u32, u32)>,
    /// Drag start in screen coordinates (for overlay rendering).
    pub start_screen: Option<egui::Pos2>,
    /// Drag end in screen coordinates (for overlay rendering).
    pub end_screen: Option<egui::Pos2>,
}

impl CropUiState {
    pub fn reset(&mut self) {
        *self = Self::default();
    }

    pub fn has_selection(&self) -> bool {
        self.start_px.is_some() && self.end_px.is_some()
    }
}

/// Transient state for the two-click scale reference workflow (T013–T014).
#[derive(Debug, Default)]
pub struct ScaleUiState {
    pub point_a: Option<ImagePoint>,
    pub point_b: Option<ImagePoint>,
    /// Distance field value as the user types it.
    pub distance_input: String,
    /// `true` = meters, `false` = feet.
    pub use_meters: bool,
}

impl ScaleUiState {
    pub fn new() -> Self {
        Self {
            use_meters: true,
            ..Default::default()
        }
    }

    pub fn reset(&mut self) {
        *self = Self::new();
    }
}

/// Top-level application state passed to every egui frame.
pub struct BlueprintApp {
    pub state: state::AppState,
    pub session: Option<Session>,
    pub error_message: Option<String>,
    /// Preselected output path from CLI `--output` flag.
    pub output_path: Option<PathBuf>,
    pub export_format: ExportFormat,
    /// Set after a successful export.
    pub last_export_path: Option<PathBuf>,
    /// Transient crop drag UI state (not persisted to session).
    pub crop_ui: CropUiState,
    /// Transient scaling UI state (not persisted to session).
    pub scale_ui: ScaleUiState,
    /// Cached egui texture for the loaded blueprint image.
    pub image_texture: Option<egui::TextureHandle>,
    /// `true` when ML models are unavailable and the app is in rule-based-only mode (FR-019).
    pub rule_based_only: bool,
    /// Non-None while the analysis pipeline is running (replaces `analysis_pending`).
    pub analysis_state: Option<AnalysisState>,
    /// Wall height input field text (ModelReady state).
    pub wall_height_input: String,
    /// Use meters for wall height input (true) or feet (false).
    pub wall_height_use_meters: bool,
    /// Generated 3D model, available after ModelReady confirmation.
    pub model3d: Option<crate::model3d::generator::Model3D>,
}

impl BlueprintApp {
    pub fn new_empty() -> Self {
        // Check model availability once on startup (FR-018, FR-019).
        // download_models() is a no-op stub; if it fails or no .onnx files are present
        // afterwards, we set rule_based_only = true.
        let _ = model_manager::download_models(None); // best-effort; errors ignored
        let rule_based_only = !model_manager::is_available(None);
        Self {
            state: state::AppState::Welcome,
            session: None,
            error_message: None,
            output_path: None,
            export_format: ExportFormat::Obj,
            last_export_path: None,
            crop_ui: CropUiState::default(),
            scale_ui: ScaleUiState::new(),
            image_texture: None,
            rule_based_only,
            analysis_state: None,
            wall_height_input: "2.44".to_string(),
            wall_height_use_meters: true,
            model3d: None,
        }
    }

    pub fn action_open_image(&mut self) {
        if let Some(path) = rfd::FileDialog::new()
            .add_filter("Blueprint images", &["jpg", "jpeg", "png"])
            .pick_file()
        {
            match BlueprintImage::load(&path) {
                Ok(img) => {
                    self.session = Some(Session::new(img));
                    self.state = state::AppState::Cropping;
                    self.crop_ui.reset();
                    self.error_message = None;
                }
                Err(e) => {
                    self.error_message = Some(format!("Could not open image: {}", e));
                }
            }
        }
        self.image_texture = None; // invalidate cached texture on image change
    }

    pub fn action_load_session(&mut self) {
        if let Some(path) = rfd::FileDialog::new()
            .add_filter("Blueprint sessions", &["b2m"])
            .pick_file()
        {
            match Session::load(&path) {
                Ok(s) => {
                    self.session = Some(s);
                    self.state = state::AppState::Scaled;
                    self.error_message = None;
                }
                Err(e) => {
                    self.error_message = Some(format!("Could not load session: {}", e));
                }
            }
        }
    }

    pub fn action_save_session(&mut self) {
        if let Some(ref mut session) = self.session {
            if let Some(path) = rfd::FileDialog::new()
                .add_filter("Blueprint sessions", &["b2m"])
                .set_file_name("session.b2m")
                .save_file()
            {
                if let Err(e) = session.save(&path) {
                    self.error_message = Some(format!("Save failed: {}", e));
                }
            }
        }
    }

    /// Kick off the detection pipeline: load pixels, apply crop, then spawn the OCR thread.
    /// Transitions to `AppState::Analyzing` immediately (T052).
    pub fn action_analyze(&mut self) {
        use crate::ocr::extractor::OcrExtractor;

        // Extract the data we need before spawning.
        let (img, scale, session_crop) = match &self.session {
            Some(s) => match (&s.image, &s.scale) {
                (img, Some(scale)) => (img.clone(), scale.clone(), s.crop_region),
                _ => {
                    self.error_message =
                        Some("No scale reference — please set scale first.".to_string());
                    return;
                }
            },
            None => {
                self.error_message =
                    Some("No session — please open a blueprint first.".to_string());
                return;
            }
        };

        let raw_pixels = match img.load_pixels() {
            Ok(p) => p,
            Err(e) => {
                self.error_message = Some(format!("Failed to load image pixels: {}", e));
                return;
            }
        };

        let dyn_img = if let Some(crop) = session_crop {
            raw_pixels.crop_imm(crop.x, crop.y, crop.width, crop.height)
        } else {
            raw_pixels
        };

        let base_img = std::sync::Arc::new(dyn_img);
        let base_img_thread = base_img.clone();

        let (tx, rx) = mpsc::channel::<StageResult>();

        let pipeline_start = Instant::now();

        // Stage 1: OCR on raw image → mask text regions (FR-004 pipeline order).
        std::thread::spawn(move || {
            let ocr = OcrExtractor::new();
            let raw_ocr = ocr.extract(&*base_img_thread).unwrap_or_default();
            // T066: use preprocessor::mask_text_regions (adaptive padding + border-median fill)
            let masked_img =
                crate::detection::preprocessor::mask_text_regions((*base_img_thread).clone(), &raw_ocr);
            let _ = tx.send(StageResult::OcrDone { raw_ocr, masked_img });
        });

        self.analysis_state = Some(AnalysisState {
            stage: PipelineStage::Ocr,
            stage_started: Instant::now(),
            pipeline_start,
            result_rx: rx,
            base_img,
            scale,
            raw_ocr: Vec::new(),
            masked_img: None,
            segments: Vec::new(),
            elements: Vec::new(),
            ml_timed_out: false,
        });
        self.state = state::AppState::Analyzing;
        self.error_message = None;
    }

    /// Poll the background thread and advance the pipeline to the next stage.
    /// Called every frame from `render_analyzing` (T052).
    pub fn advance_analysis_pipeline(&mut self) {
        use crate::blueprint::floor_plan::build_floor_plan;
        use crate::detection::line_tracer::trace_lines;
        use crate::ocr::parser::parse_annotations;
        use crate::session::serialization::PendingClarification;

        let result = {
            let st = match self.analysis_state.as_ref() {
                Some(s) => s,
                None => return,
            };
            st.result_rx.try_recv().ok()
        };

        let result = match result {
            Some(r) => r,
            None => return, // still running — wait for next frame
        };

        match result {
            StageResult::StageFailed(msg) => {
                self.error_message = Some(msg);
                self.analysis_state = None;
                self.state = state::AppState::Scaled;
            }

            StageResult::OcrDone { raw_ocr, masked_img } => {
                // Spawn Stage 2: Trace (includes NLM denoising + adaptive Canny, FR-025)
                let (tx, rx) = mpsc::channel::<StageResult>();
                let scale = self.analysis_state.as_ref().unwrap().scale.clone();
                let masked_arc = std::sync::Arc::new(masked_img.clone());
                let masked_for_thread = masked_arc.clone();
                std::thread::spawn(move || {
                    use crate::detection::preprocessor;
                    // 1. Non-local means denoising before edge detection (FR-025)
                    let denoised = preprocessor::denoise(&*masked_for_thread);
                    // 2. Per-image adaptive Canny thresholds (FR-025)
                    let gray = denoised.to_luma8();
                    let (low, high) = preprocessor::adaptive_canny_thresholds(&gray);
                    let segments = trace_lines(&denoised, &scale, low, high);
                    let _ = tx.send(StageResult::TraceDone { segments });
                });

                let st = self.analysis_state.as_mut().unwrap();
                st.raw_ocr = raw_ocr;
                st.masked_img = Some(masked_arc);
                st.result_rx = rx;
                st.stage = PipelineStage::Trace;
                st.stage_started = Instant::now();
            }

            StageResult::TraceDone { segments } => {
                // Spawn Stage 3: Classify with timeout (FR-028)
                let (tx, rx) = mpsc::channel::<StageResult>();
                let st = self.analysis_state.as_mut().unwrap();
                let masked_arc = st.masked_img.clone().unwrap();
                let segs = segments.clone();
                let pipeline_start = st.pipeline_start;
                std::thread::spawn(move || {
                    use crate::detection::classifier::classify_with_timeout;
                    let (elements, timed_out) =
                        classify_with_timeout(&segs, Some(&*masked_arc), None, pipeline_start);
                    let _ = tx.send(StageResult::ClassifyDone { elements, timed_out });
                });

                st.segments = segments;
                st.result_rx = rx;
                st.stage = PipelineStage::Classify;
                st.stage_started = Instant::now();
            }

            StageResult::ClassifyDone { elements, timed_out } => {
                // Store timeout flag for UI banner (FR-028, T064).
                if let Some(st) = self.analysis_state.as_mut() {
                    st.ml_timed_out = timed_out;
                }

                // Merge collinear same-type segments before building floor plan (FR-026).
                let current_segments =
                    self.analysis_state.as_ref().unwrap().segments.clone();
                let (merged_segs, merged_elems) =
                    crate::detection::merger::merge_collinear_segments(
                        &current_segments,
                        &elements,
                    );

                // Spawn Stage 4: FloorPlan
                let (tx, rx) = mpsc::channel::<StageResult>();
                let st = self.analysis_state.as_mut().unwrap();
                let scale = st.scale.clone();
                let raw_ocr = st.raw_ocr.clone();
                let elems = merged_elems.clone();
                std::thread::spawn(move || {
                    let annotations = parse_annotations(&raw_ocr);
                    let floor_plan = build_floor_plan(&elems, &scale, &annotations).ok();
                    let history = crate::correction::history::CorrectionHistory::load_or_default()
                        .unwrap_or_default();
                    let pending: Vec<PendingClarification> = elems
                        .iter()
                        .filter(|e| (e.confidence as f64) < history.adaptive_threshold.into())
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
                    let _ = tx.send(StageResult::FloorPlanDone {
                        floor_plan,
                        annotations,
                        pending,
                    });
                });

                st.segments = merged_segs;
                st.elements = merged_elems;
                st.result_rx = rx;
                st.stage = PipelineStage::FloorPlan;
                st.stage_started = Instant::now();
            }

            StageResult::FloorPlanDone { floor_plan, annotations, pending } => {
                // All stages complete — update session and transition
                let st = self.analysis_state.take().unwrap();
                if let Some(ref mut session) = self.session {
                    session.line_segments = st.segments;
                    session.elements = st.elements;
                    session.text_annotations = annotations;
                    session.floor_plan = floor_plan;
                    session.pending_clarifications = pending;
                }
                self.state = state::AppState::Analyzed;
                self.error_message = None;
            }

        }
    }

    pub fn action_export(&mut self) {
        // Export pipeline — implemented in US4 (T037–T044).
        self.state = state::AppState::Exported;
    }

    /// Generate the 3D model from the current floor plan and confirmed wall height (T042).
    pub fn action_generate_model(&mut self) {
        use crate::model3d::generator::generate;

        let wall_height_m: f64 = match self.wall_height_input.trim().parse::<f64>() {
            Ok(h) if h > 0.0 => {
                if self.wall_height_use_meters {
                    h
                } else {
                    h * 0.3048 // feet to meters
                }
            }
            _ => {
                self.error_message = Some("Wall height must be a positive number.".to_string());
                return;
            }
        };

        let floor_plan = match self.session.as_ref().and_then(|s| s.floor_plan.as_ref()) {
            Some(fp) => fp.clone(),
            None => {
                self.error_message = Some("No floor plan — run analysis first.".to_string());
                return;
            }
        };

        if let Some(ref mut session) = self.session {
            session.wall_height_m = Some(wall_height_m);
        }

        self.model3d = Some(generate(&floor_plan, wall_height_m));
        self.error_message = None;
    }

    /// Export the generated model to the chosen format and path (T043).
    pub fn action_export_file(&mut self) {
        use crate::export::obj::export_obj;
        use crate::export::stl::export_stl;

        let model = match &self.model3d {
            Some(m) => m.clone(),
            None => {
                self.error_message = Some("Generate the 3D model first.".to_string());
                return;
            }
        };

        let floor_plan = match self.session.as_ref().and_then(|s| s.floor_plan.as_ref()) {
            Some(fp) => fp.clone(),
            None => {
                self.error_message = Some("No floor plan available.".to_string());
                return;
            }
        };

        let (ext, desc) = match self.export_format {
            ExportFormat::Obj => ("obj", "OBJ files"),
            ExportFormat::Stl => ("stl", "STL files"),
        };

        let path = match rfd::FileDialog::new()
            .add_filter(desc, &[ext])
            .set_file_name(format!("blueprint.{}", ext))
            .save_file()
        {
            Some(p) => p,
            None => return, // user cancelled
        };

        let result = match self.export_format {
            ExportFormat::Obj => export_obj(&model, &floor_plan, &path),
            ExportFormat::Stl => export_stl(&model, &path),
        };

        match result {
            Ok(()) => {
                self.last_export_path = Some(path);
                self.state = state::AppState::Exported;
                self.error_message = None;
            }
            Err(e) => {
                self.error_message = Some(format!("Export failed: {}", e));
            }
        }
    }
}

/// Fill every OCR-detected text bounding box with white pixels so that text strokes
/// are not traced as structural line segments (FR-004).
impl eframe::App for BlueprintApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        ui::render(self, ctx);
    }
}
