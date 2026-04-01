---
description: "Task list for blueprint2mod — Blueprint Image to 3D Model"
---

# Tasks: Blueprint Image to 3D Model

**Input**: Design documents from `/specs/001-blueprint-to-3d/`
**Prerequisites**: plan.md ✅ spec.md ✅ research.md ✅ data-model.md ✅ contracts/ ✅

**Tests**: MANDATORY per Constitution Principle I (Test-First Development).
Tests MUST be written before implementation and confirmed to FAIL before any
implementation code is produced.

**Organization**: Tasks are grouped by user story to enable independent implementation
and testing of each story. P1 (US1) must ship before P2 (US2) work begins.

## Format: `[ID] [P?] [Story?] Description`

- **[P]**: Can run in parallel (different files, no dependencies)
- **[Story]**: Which user story this task belongs to (US1, US2, US3, US4)
- Include exact file paths in all descriptions

---

## Phase 1: Setup

**Purpose**: Project initialization and directory structure

- [X] T001 Create `Cargo.toml` with full dependency list: egui/eframe/egui_extras 0.34, image 0.25, imageproc 0.25, leptess 0.14, tract-onnx 0.22, stl_io 0.10, serde/serde_json 1.x, reqwest, uuid, anyhow, thiserror, chrono
- [X] T002 [P] Create source directory structure per plan.md: `src/app/`, `src/blueprint/`, `src/detection/ml/`, `src/detection/rules/`, `src/ocr/`, `src/model3d/`, `src/export/`, `src/session/`, `src/correction/`
- [X] T003 [P] Create `tests/integration/` directory and `test_fixtures/` with placeholder reference files (`simple_rectangle.jpg`, `simple_rectangle.expected.json`, `labeled_plan.jpg`, `labeled_plan.expected.json`)

---

## Phase 2: Foundational (Blocking Prerequisites)

**Purpose**: Shared types and skeleton that all user stories depend on

**⚠️ CRITICAL**: No user story work can begin until this phase is complete

- [X] T004 Define shared primitive types in `src/blueprint/mod.rs`: `ImagePoint`, `WorldPoint`, `LengthUnit` (Feet/Meters), `BoundingBox`, `ImageBoundingBox` with `#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]`
- [X] T005 [P] Define `ElementType` enum and `ArchitecturalElement` struct in `src/blueprint/element.rs` (all variants: Wall, Door, Window, SlidingDoor, Fireplace, Closet, Staircase, Chimney, Courtyard, Unclassified; fields: id, element_type, bounds, source_segment_ids, confidence, is_interior, wall_thickness_m)
- [X] T006 [P] Define `AppState` enum in `src/app/state.rs` (Welcome, ImageLoaded, Scaled, Analyzing, Analyzed, Clarifying, ModelReady, Exported) and transition validity rules
- [X] T007 Implement `eframe::App` skeleton in `src/main.rs` + `src/app/ui.rs`: window launches, dispatches to per-state render functions (all states render placeholder text), CLI args parsed (`IMAGE`, `--session`, `--format`, `--output` per contracts/cli.md)
- [X] T008 [P] Define `Session` and `PendingClarification` structs in `src/session/serialization.rs` with full serde derives; define `CorrectionHistory` and `CorrectionEntry` structs in `src/correction/history.rs`

**Checkpoint**: Foundation ready — all types compile, window opens, user story phases can begin

---

## Phase 3: User Story 1 — Import and Scale a Blueprint (Priority: P1) 🎯 MVP

**Goal**: User loads a JPG/PNG blueprint; GUI displays it; user clicks two reference
points; enters real-world distance; system computes and stores scale.

**Independent Test**: Load `simple_rectangle.jpg`, click two known points 320 px apart,
enter 3.66 m → verify `pixels_per_unit` = 87.43 within ±5% tolerance. App transitions
to `AppState::Scaled`.

### Tests for User Story 1 (MANDATORY — Constitution Principle I) ⚠️

> **REQUIRED: Write these tests FIRST, confirm they FAIL before any implementation begins (Red phase)**

- [X] T009 [P] [US1] Write integration test `tests/integration/test_scaling.rs`: load `simple_rectangle.jpg`, programmatically supply two reference points and a known distance, assert `pixels_per_unit` is within ±5% of expected value (SC-004 gate)
- [X] T010 [P] [US1] Write unit tests in `src/blueprint/scale.rs` (`#[cfg(test)]`): assert `ScaleReference::new()` rejects identical points, zero distance, and out-of-bounds coordinates with typed errors

### Implementation for User Story 1

- [X] T011 [P] [US1] Implement `BlueprintImage::load()` in `src/blueprint/image.rs`: open file, validate extension is `.jpg`/`.jpeg`/`.png` (case-insensitive), extract width/height, return typed error for unsupported formats (FR-001)
- [X] T012 [US1] Implement `ScaleReference::new()` in `src/blueprint/scale.rs`: validate point_a ≠ point_b, distance > 0, both points in bounds; compute and store `pixels_per_unit`; expose `to_world_distance(pixels: f64) → f64` helper (FR-002, FR-003)
- [X] T013 [US1] Implement scaling UI in `src/app/ui.rs` for `AppState::ImageLoaded`: render blueprint image via `egui_extras` image loader with `Sense::click()`, capture first and second click positions using `RectTransform` for pixel-accurate coordinates, render red circle overlays on selected points via `ui.painter()`, show distance input field and unit selector (Feet/Meters) after second point is selected (FR-002)
- [X] T014 [US1] Wire scale confirmation in `src/app/ui.rs`: on user confirming distance, construct `ScaleReference`, transition to `AppState::Scaled`; reject invalid inputs and re-prompt with inline error (FR-003)

### Session Save/Load (US1 save point — first opportunity after scaling)

- [X] T015 [US1] Write integration test `tests/integration/test_session.rs`: save a session after scaling step, reload from `.b2m` file, assert all fields round-trip correctly including `scale.pixels_per_unit` and `image.path`
- [X] T016 [US1] Implement `Session::save()` and `Session::load()` in `src/session/serialization.rs`: serialize to JSON via `serde_json`, write to user-chosen path with `.b2m` extension, deserialize on load, validate `version` field (FR-016, FR-017)
- [X] T017 [US1] Implement save/load UI triggers in `src/app/ui.rs`: "Save Session" button available from `AppState::Scaled` onward (opens native save-file dialog), "Load Session" button in `AppState::Welcome` (opens open-file dialog, restores saved state)

**Checkpoint**: User Story 1 fully functional — load image, click to scale, save and reload session

---

## Phase 4: User Story 2 — Line Tracing and Architectural Element Detection (Priority: P2)

**Goal**: System automatically traces dark lines, classifies all architectural elements,
infers interior/exterior, extracts room labels and dimension values via OCR.

**Independent Test**: Run full detection pipeline on `simple_rectangle.jpg`; assert ≥90%
of known wall segments detected and typed correctly (SC-002); assert interior regions
correctly identified (SC-005); verify OCR extracts room labels from `labeled_plan.jpg`.

### Tests for User Story 2 (MANDATORY — Constitution Principle I) ⚠️

> **REQUIRED: Write these tests FIRST, confirm they FAIL before any implementation begins (Red phase)**

- [X] T018 [P] [US2] Write integration test `tests/integration/test_detection.rs`: run detection on `simple_rectangle.jpg`, compare output elements against `simple_rectangle.expected.json`, assert ≥90% wall detection rate (SC-002 gate)
- [X] T019 [P] [US2] Write integration test `tests/integration/test_detection_fallback.rs`: temporarily disable ML models (set model path to nonexistent dir), run detection, assert rule-based mode activates without panic, verify warning emitted (Constitution Principle III gate)
- [X] T020 [P] [US2] Write integration test `tests/integration/test_interior_exterior.rs`: run full pipeline on `simple_rectangle.jpg`, assert interior rooms correctly marked and exterior correctly excluded (SC-005 gate, ≥90%)
- [X] T021 [P] [US2] Write integration test `tests/integration/test_ocr.rs`: run OCR on `labeled_plan.jpg`, assert known room labels extracted with correct `KnownRoomType` mappings, assert dimension value parsed to expected float

### Implementation for User Story 2

- [X] T022 [US2] Implement line tracer in `src/detection/line_tracer.rs`: convert to grayscale, apply histogram equalization (`imageproc::contrast`), Canny edge detection (`imageproc::edges::canny`), Hough line transform (`imageproc::hough`), morphological dilation to connect nearby segments, trace contiguous pixel paths into `Vec<LineSegment>` with wall spacing detection for double-lines (FR-004)
- [X] T023 [P] [US2] Implement rule-based patterns in `src/detection/rules/patterns.rs`: classify `LineSegment` into `ElementType` using geometric heuristics (door arc radius, window segment length/gap pattern, stair hatch density, wall double-line spacing ≤ 20px, chimney isolated rectangle) (FR-005)
- [X] T024 [US2] Implement ML model manager in `src/detection/ml/model_manager.rs`: on first call check `~/.blueprint2mod/models/` for cached ONNX files; if absent, download via `reqwest` to temp file then move to cache; expose `is_available() → bool`; respect SC-007 (models ≤100 MB each, total model budget ≤700 MB) (FR-018)
- [X] T025 [US2] Implement ML inference in `src/detection/ml/inference.rs`: load ONNX model via `tract_onnx::onnx().model_for_path()`, preprocess image patches to model input tensor, run inference, return `(ElementType, f32)` confidence pairs (FR-004, FR-005)
- [X] T026 [US2] Implement hybrid classifier in `src/detection/classifier.rs`: load `CorrectionHistory` to get `adaptive_threshold`; for each `LineSegment`, call ML inference if models available — use ML result if `confidence ≥ adaptive_threshold`, else call rule-based; if ML unavailable emit warning and use rules only (FR-005, FR-007, FR-019)
- [X] T027 [P] [US2] Implement OCR extractor in `src/ocr/extractor.rs`: initialize `leptess::LepTess` with `eng` language data, preprocess image region (binarize via `imageproc::contrast::threshold`, denoise via morphological opening), run OCR, return `Vec<(String, ImageBoundingBox, f32)>` (raw text, bounds, confidence) (FR-020)
- [X] T028 [US2] Implement OCR parser in `src/ocr/parser.rs`: match extracted text against `KnownRoomType` list (case-insensitive); parse dimension strings (regex for `12'-6"`, `3.75m`, `3750mm`) into `(f64, LengthUnit)` tuples; tag unreadable items as `TextAnnotationType::Unreadable` (FR-021, FR-022, FR-023)
- [X] T029 [US2] Implement OCR scale validation warning in `src/blueprint/scale.rs`: add `validate_against_ocr()` method — compare OCR-derived `pixels_per_unit` vs user-provided; if divergence > 5%, return `ScaleWarning` that the UI surfaces to the user (FR-022)
- [X] T030 [US2] Implement interior/exterior inference in `src/blueprint/floor_plan.rs`: flood-fill from known exterior seed point, classify enclosed regions as `is_interior = true`; build `Vec<Room>` with boundary element IDs and associated `TextAnnotation` IDs; produce complete `FloorPlan` (FR-006, FR-021)
- [X] T031 [US2] Implement model download first-run flow in `src/app/ui.rs` for `AppState::Welcome`: check `model_manager.is_available()` on startup; if false, show progress bar during async download; if download fails, show persistent warning banner and proceed in rule-based-only mode (FR-018, FR-019)
- [X] T032 [US2] Integrate detection pipeline into app: `AppState::Analyzed` transition — wire `line_tracer → classifier → ocr_extractor → ocr_parser → floor_plan` in sequence; show progress indicator in `src/app/ui.rs` during analysis; update `Session.elements`, `Session.text_annotations`, `Session.floor_plan` (FR-004 through FR-006)

**Checkpoint**: User Story 2 fully functional — detection, OCR, and interior/exterior inference all pass independently

---

## Phase 5: User Story 3 — User-Assisted Clarification (Priority: P3)

**Goal**: System surfaces ambiguous segments one at a time; user selects correct type or
skips; adaptive threshold tunes over time; global correction history persists.

**Independent Test**: Inject a segment with `confidence = 0.4` (below default threshold)
into the pipeline; assert it surfaces for user review; simulate user correction; assert
`CorrectionHistory` updated; assert `adaptive_threshold` shifts; assert skip marks segment
as `ElementType::Unclassified`.

### Tests for User Story 3 (MANDATORY — Constitution Principle I) ⚠️

> **REQUIRED: Write these tests FIRST, confirm they FAIL before any implementation begins (Red phase)**

- [X] T033 [P] [US3] Write integration test `tests/integration/test_clarification.rs`: inject low-confidence element into `FloorPlan`, assert it appears in `pending_clarifications`, simulate user selection, assert `ArchitecturalElement.element_type` updated, assert `CorrectionHistory.corrections` has one new entry
- [X] T034 [P] [US3] Write unit tests in `src/correction/history.rs` (`#[cfg(test)]`): simulate 20 corrections, assert `adaptive_threshold` converges toward the running average of corrected confidence scores; assert save/load round-trips correctly from JSON

### Implementation for User Story 3

- [X] T035 [US3] Implement `CorrectionHistory` save/load and threshold adaptation in `src/correction/history.rs`: load from `~/.blueprint2md/corrections.json` at startup (create with defaults if absent), append `CorrectionEntry` on each correction, recompute `adaptive_threshold` as exponential moving average of corrected confidence values, persist after each update (FR-007)
- [X] T036 [US3] Implement clarification UI in `src/app/ui.rs` for `AppState::Clarifying`: display one `PendingClarification` at a time — show segment context snippet, render clickable `ElementType` selection buttons, provide "Skip" button; on selection update `ArchitecturalElement`, add to `CorrectionHistory`, advance to next pending; on final resolution transition to `AppState::ModelReady` (FR-007, FR-008)

**Checkpoint**: User Story 3 fully functional — clarification loop, skip, and adaptive threshold all independently testable

---

## Phase 6: User Story 4 — 3D Model Generation and Export (Priority: P4)

**Goal**: System generates 3D model from `FloorPlan`; user confirms wall height; exports
as OBJ+MTL or STL; summary report displayed.

**Independent Test**: Run generator on `simple_rectangle.jpg` pipeline output; load
resulting OBJ in SketchUp 2020+; assert no import errors (SC-003); measure a wall in
SketchUp and verify it matches the known real-world dimension within ±5% (SC-004).

### Tests for User Story 4 (MANDATORY — Constitution Principle I) ⚠️

> **REQUIRED: Write these tests FIRST, confirm they FAIL before any implementation begins (Red phase)**

- [X] T037 [P] [US4] Write integration test `tests/integration/test_export_obj.rs`: run full pipeline on `simple_rectangle.jpg`, export OBJ+MTL, parse resulting files — assert `mtllib` directive present, assert each `ElementType` has a named group (`g wall_…`, `g door_…`), assert MTL defines all materials, assert all faces are triangles (3 vertex indices), assert vertex coordinates are within expected real-world bounds (SC-003, SC-004 gates)
- [X] T038 [P] [US4] Write integration test `tests/integration/test_export_stl.rs`: export STL from same pipeline, parse binary STL header and triangle count, assert file is valid binary STL with at least one triangle

### Implementation for User Story 4

- [X] T039 [US4] Implement 3D generator in `src/model3d/generator.rs`: for each `ArchitecturalElement` in `FloorPlan` produce `Mesh` geometry — walls extruded to `wall_height_m` with derived `wall_thickness_m`, doors/windows as openings (missing face in wall mesh), staircases as uniform stepped geometry, chimneys as raised box, fireplaces as raised hearth slab, closets as enclosed box, courtyards as floor-level recessed plane; use Y-up coordinate system per contracts/export-format.md (FR-009, FR-010)
- [X] T040 [US4] Implement custom OBJ+MTL writer in `src/export/obj.rs`: write header comment (units: meters, Y-up), `mtllib <filename>.mtl` directive, per-element named groups (`g <type>_<uuid8>`), `usemtl mat_<type>` per group, triangulated CCW vertex + face data; write companion `.mtl` file with `newmtl`, `Kd`, `d`, `illum` per element type using RGB values from data-model.md material table (FR-012, contracts/export-format.md)
- [X] T041 [P] [US4] Implement STL writer in `src/export/stl.rs`: collect all `Mesh` triangles from `Model3D`, compute outward normals (CCW), call `stl_io::write_stl()` to binary file (FR-013)
- [X] T042 [US4] Implement wall height confirmation UI in `src/app/ui.rs` for `AppState::ModelReady`: display numeric input pre-filled with 2.44 m (8 ft), accept feet or meters, convert to meters before storing in `Session.wall_height_m`, button to trigger `generator::generate()` (FR-009)
- [X] T043 [US4] Implement export selection UI in `src/app/ui.rs` after generation: radio buttons for OBJ vs STL format, native save-file dialog, call appropriate exporter, transition to `AppState::Exported` on success (FR-011, FR-012, FR-013)
- [X] T044 [US4] Implement end-of-processing summary panel in `src/app/ui.rs` for `AppState::Exported`: display count of each detected element type, list all unclassified segment IDs, list all `TextAnnotation::Unreadable` regions with image coordinates, show export file path with "Open in Finder/Explorer" button (FR-015)

**Checkpoint**: All user stories fully functional and independently testable

---

## Phase 7: Polish & Cross-Cutting Concerns

**Purpose**: Quality gates, size validation, and cleanup across all stories

- [X] T045 [P] Verify SC-007 size gate: run `cargo build --release`, download all ML models to temp dir, measure combined size of binary + model files; assert total ≤ 1 GB; add this check to integration test suite as `tests/integration/test_size_gate.rs`
- [X] T046 [P] Add `cargo clippy -- -D warnings` and `cargo fmt --check` gate to project — fix all warnings exposed
- [X] T047 Validate SC-001 performance target: run full pipeline on a 4000×4000 px test fixture (or nearest available), measure wall-clock time from image load to export complete; assert ≤ 10 minutes; document result in `test_fixtures/performance_results.md`
- [X] T048 [P] Update `CLAUDE.md` with final crate versions confirmed by `Cargo.lock`, any deviation from plan.md (e.g. if `ort` was needed instead of `tract-onnx`), and any model size observations

---

## Phase 8: Clarification-Driven Fixes (2026-03-31)

**Source**: spec.md Session 2026-03-31 clarifications

- [X] T049 Fix door/sliding door 3D geometry in `src/model3d/generator.rs`: return empty `Vec<Triangle>` for `Door` and `SlidingDoor` so they appear as gaps in the wall rather than extruded blocks (FR-009)
- [X] T050 Restructure detection pipeline in `src/app/mod.rs`: run `OcrExtractor::extract()` first to obtain text bounding boxes, mask those regions with white pixels via `mask_text_regions()` using `imageproc::drawing::draw_filled_rect_mut`, then run `trace_lines()` on the masked image (FR-004)
- [X] T051 Add optional image crop preprocessing step (FR-024): `CropRegion` type in `src/blueprint/mod.rs`; `Cropping` state in `src/app/state.rs` (Welcome → Cropping → ImageLoaded); `crop_region: Option<CropRegion>` in `Session`; `CropUiState` and `action_open_image → Cropping` in `src/app/mod.rs`; `render_cropping()` with drag-to-select UI and confirm/skip in `src/app/ui.rs`; crop applied in both texture loading and `run_analysis_pipeline()`
- [X] T052 Add named-stage progress indicator to analysis pipeline: `PipelineStage` enum with `label()`, `estimated_secs()`, `TOTAL=4`; `StageResult` enum; `AnalysisState` struct with `stage`, `stage_started: Instant`, `result_rx: Receiver<StageResult>`, `base_img`, `scale`, `raw_ocr`, `segments`, `elements`; replace `analysis_pending: bool` + `run_analysis_pipeline()` with `analysis_state: Option<AnalysisState>` + `advance_analysis_pipeline()`; rewrite `action_analyze()` to spawn OCR thread; update `render_analyzing()` to poll channel + display named stage label + `egui::ProgressBar` (elapsed/estimated_secs)

---

## Phase 9: Image Quality Improvements (2026-03-31)

**Source**: spec.md Session 2026-03-31 clarifications (FR-025, FR-026)

- [X] T053 Add image denoising and adaptive Canny thresholds (FR-025): create `src/detection/preprocessor.rs` with `pub fn denoise(img: &DynamicImage) -> DynamicImage` implementing a fast non-local means filter (search window 21 px, patch size 7 px, h=10); add `pub fn adaptive_canny_thresholds(img: &GrayImage) -> (f64, f64)` computing high threshold at 90th-percentile and low threshold at 70th-percentile of the gradient magnitude histogram; update `trace_lines()` in `src/detection/line_tracer.rs` to accept precomputed thresholds rather than hardcoded values; call `denoise()` on the masked image inside the Trace stage thread in `src/app/mod.rs` (after OCR masking, before `trace_lines()`); update `estimated_secs` for `PipelineStage::Trace` from `2.0` to `8.0` to account for NLM processing time
- [X] T054 Add collinear segment merging (FR-026): create `src/detection/merger.rs` with `pub fn merge_collinear_segments(segments: &[LineSegment], elements: &[ArchitecturalElement]) -> (Vec<LineSegment>, Vec<ArchitecturalElement>)` — for each same-classified-type element pair whose source segments are ≤2° of alignment and ≤10 px gap (scaled by `scale.pixels_per_unit`), replace both with a single merged segment spanning their combined endpoints; call `merge_collinear_segments()` inside the FloorPlan stage thread in `src/app/mod.rs` after `classify()` returns, passing both segments and elements; store merged results in `AnalysisState`; add unit tests in `src/detection/merger.rs` covering: parallel walls not merged, collinear walls within tolerance merged, cross-type segments not merged

---

## Phase 10: Performance Optimisation — rayon + Apple Accelerate (2026-03-31)

**Source**: spec.md Session 2026-03-31 clarifications (FR-027, SC-008)

- [X] T055 Add `rayon = "1"` and `criterion = { version = "0.5", features = ["html_reports"] }` to `Cargo.toml`; create `build.rs` in repo root that emits `cargo:rustc-link-lib=framework=Accelerate` when `cfg(target_arch = "aarch64")` and `cfg(target_os = "macos")` are both true
- [X] T056 [P] Migrate NLM denoising in `src/detection/preprocessor.rs` from `std::thread::scope` row-band loop to `rayon::par_iter`: replace the `handles` collection + `scope` block with `(0..n_threads).into_par_iter().filter_map(…).collect::<Vec<Vec<u8>>>()`; verify all 4 existing unit tests still pass
- [X] T057 [P] Add `rayon` parallelism to Canny gradient magnitude computation in `src/detection/line_tracer.rs`: replace the sequential `for y in 1..(height-1)` loop that builds the gradient buffer with `(1..(height-1)).into_par_iter()` collecting into a pre-allocated `Vec` via `rayon::iter::ParallelIterator::flat_map`
- [X] T058 [P] Add `rayon` parallelism to classifier feature-extraction batch in `src/detection/classifier.rs`: replace the sequential per-element scoring loop with `.par_iter().map(|seg| score_segment(seg, img)).collect()`; wrap any shared ML model handle in `Arc` if needed for `Send` safety
- [X] T059 [P] Add `rayon` parallelism to 3D mesh face-generation loops in `src/model3d/generator.rs`: replace sequential `elements.iter().flat_map(generate_element_faces)` with `elements.par_iter().flat_map(generate_element_faces).collect()`
- [X] T060 Add `#[cfg(all(target_arch = "aarch64", target_os = "macos"))]` vDSP-accelerated inner loops to `src/detection/preprocessor.rs` (NLM patch-distance sum via `vDSP_sve` / `vDSP_dotpr`) and `src/detection/line_tracer.rs` (Sobel convolution accumulation via `vDSP_conv`); provide plain-Rust fallback paths for the non-aarch64 case; declare the C FFI signatures at the top of each file under `#[link(name = "Accelerate", kind = "framework")]`
- [X] T061 Create `benches/pipeline_bench.rs` with `criterion` benchmark group `"pipeline"`: define `bench_baseline` (single-threaded NLM + sequential Canny, no vDSP) and `bench_optimised` (rayon + vDSP) functions, each running the full OCR-mask-denoise-trace-classify-merge pipeline on `test_fixtures/simple_rectangle.jpg`; add `[[bench]] name = "pipeline_bench" harness = false` to `Cargo.toml`; document SC-008 ≥2× threshold in a comment; run with `cargo bench --bench pipeline_bench`

---

## Phase 11: ML Timeout Graceful Fallback (2026-03-31)

**Source**: spec.md Session 2026-03-31 clarifications (FR-028)

- [X] T062 Add `pipeline_start: Instant` field to `AnalysisState` in `src/app/mod.rs`; set it to `Instant::now()` in `action_analyze()`; thread the elapsed check into the Classify stage spawn: pass `pipeline_start` into the classify thread and check `pipeline_start.elapsed() >= Duration::from_secs(300)` before processing each element batch; when the timeout fires, set a `timed_out: bool` flag in `StageResult::ClassifyDone { elements, timed_out }`
- [X] T063 Add `pub fn classify_partial(ml_elements: Vec<ArchitecturalElement>, remaining_segments: &[LineSegment], img: Option<&DynamicImage>, scale: Option<&ScaleReference>) -> Vec<ArchitecturalElement>` to `src/detection/classifier.rs`: keep elements from `ml_elements` where `confidence >= 0.7`; call the existing rule-based `classify()` path for the remainder; merge and return the combined list; call `classify_partial` from the `ClassifyDone` handler in `src/app/mod.rs` when `timed_out == true`
- [X] T064 In `src/app/ui.rs`, add a non-blocking inline warning banner `"ML timeout — continuing with rule-based detection"` rendered inside `render_analyzing()` when `analysis_state.ml_timed_out` is true; in `src/app/mod.rs`, propagate the timeout flag into the `FloorPlan` summary data so `render_summary()` appends a note per FR-015: `"Note: ML classification was interrupted at 5-minute timeout; rule-based fallback applied to [N] elements"`

---

## Phase 12: Preprocessing Pipeline Reorder + Adaptive Masking (2026-03-31)

**Source**: spec.md Session 2026-03-31 clarifications (FR-004 updated)

- [X] T065 Reorder preprocessing in `src/app/mod.rs` OcrDone handler: move the `mask_text_regions()` call to run on the raw (undenoised) image BEFORE the `preprocessor::denoise()` call — the correct order is `mask_text_regions(raw_img, &ocr_boxes)` → `preprocessor::denoise(&masked)` → `preprocessor::adaptive_canny_thresholds(&denoised.to_luma8())` → `trace_lines(&denoised, …)`; update `PipelineStage::Trace` description string to reflect the new order
- [X] T066 Upgrade `mask_text_regions()` in `src/detection/preprocessor.rs`: replace the current hardcoded white-fill with adaptive masking — (a) compute `pad = max(1, median_char_height / 2)` where `median_char_height` is the median height of all OCR bounding boxes in the set; (b) expand each bounding box by `pad` pixels on all sides (clamped to image bounds); (c) fill the expanded region with the median pixel value sampled from the 1-pixel border of the expanded box; add unit tests covering: padding equals half median char height, fill value matches border median (not 255), two overlapping expanded boxes both filled correctly

---

## Phase 13: Crop Display Update + Clarification Highlight (2026-03-31)

**Source**: spec.md Session 2026-03-31 clarifications (FR-024 updated, FR-029 new)

## Phase 14: Bug Fix — Wall Height Unit Mismatch (FR-009, SC-004)

**Source**: Bug report — walls in 3D output are not the height the user typed; root cause is that element bounds are stored in the user's scale unit (feet or meters) but `wall_height_m` is always in meters, so OBJ X/Z coordinates are in feet while Y is in meters when the user's scale reference uses feet.

- [X] T069 [US4] Write a RED (failing) unit test `wall_height_round_trips_with_feet_scale` in `src/model3d/generator.rs` `#[cfg(test)]` block: create a `ScaleReference` with `LengthUnit::Feet` (100 px = 10 ft, ppu=10); manually construct one `ArchitecturalElement` (type Wall) with `bounds = BoundingBox { min: WorldPoint{x:0.0,y:0.0}, max: WorldPoint{x:10.0,y:0.15} }` representing 10 ft × 0.15 ft in feet-unit space; build a `FloorPlan` via `build_floor_plan`; call `generate(&fp, 2.44)`; assert the maximum X vertex coordinate across ALL triangles is within 0.01 of `3.048` (10 ft in meters) — this assertion MUST FAIL before T070 is applied; also assert max Y vertex ≈ 2.44 (this should already pass); confirm test is RED then commit it as-is
- [X] T070 [US4] Fix the unit normalization bug: (1) add `pub fn to_meters_factor(self) -> f64` method to `LengthUnit` in `src/blueprint/mod.rs` returning `0.3048_f64` for `Feet` and `1.0` for `Meters`; (2) in `segment_to_element()` in `src/detection/rules/patterns.rs`, multiply every `scale.to_world_distance(…)` call result by `scale.unit.to_meters_factor()` — this normalises `bounds.min.x`, `bounds.min.y`, `bounds.max.x`, `bounds.max.y`, and the `wall_thickness_m` from `wall_spacing` to meters regardless of user scale unit; (3) run T069 test and confirm it is now GREEN; (4) run `cargo test` and confirm no regressions; also add a second test `wall_height_round_trips_with_meters_scale` verifying no regression for meter-based scales
- [X] T071 [P] [US4] Add SC-004 height assertion to `tests/integration/test_export_obj.rs`: after parsing all `v` lines from the OBJ, collect Y coordinates (index 1 of each vertex triple); assert `max_y.abs() - 2.44 < 0.01` confirming the wall height in the exported file matches the input `wall_height_m = 2.44` within ±1 cm (SC-004)

---

- [X] T067 [US1] Update crop confirmation in `src/app/ui.rs` and `src/app/mod.rs`: after the user confirms the crop bounding box, immediately replace the displayed egui texture with a re-cropped version of the original image (call `image::DynamicImage::crop_imm` on the in-memory original and re-upload via `RetainedImage` or `ColorImage`); add a "Reset Crop" button to `render_cropping()` (and to any post-crop header bar) that clears `Session.crop_region`, re-uploads the full original texture, and returns the app to the pre-confirm crop state; the "Reset Crop" button MUST remain visible for as long as an active crop is set and analysis has not yet been started (FR-024)
- [X] T068 [P] [US3] Implement low-confidence element highlight in `src/app/ui.rs` `render_clarifying()`: for each `PendingClarification`, compute the element's bounding box in screen coordinates using the same image-to-screen transform used for the crop drag overlay; call `ui.painter().rect_filled(screen_rect, 0.0, egui::Color32::from_rgba_unmultiplied(220, 30, 30, 120))` to draw a semi-transparent red fill over the element; the highlight MUST be rendered on the image before the classification selection widget so both are visible simultaneously (FR-029)

---

## Phase 15: Bug Fix — Original Image Used Instead of Cropped Image in Pipeline (FR-024)

**Source**: Bug report — after crop confirmation, the analysis pipeline and scale-reference bounds check still reference the original uncropped image in some paths. Root cause: each callsite independently applies `crop_imm()`; any callsite that omits it silently processes the full original image. Fix: centralise pixel loading in `Session::load_working_image()` and update all pipeline callsites; fix the scale-reference bounds validation to use cropped dimensions.

- [X] T072 [US1] Add `pub fn load_working_image(&self) -> anyhow::Result<image::DynamicImage>` to `Session` in `src/session/serialization.rs`: call `self.image.load_pixels()` then apply `crop_region` via `crop_imm` if `Some`; add unit test `load_working_image_applies_crop` in that file: create a `Session` whose `image.width/height` differs from `crop_region` dimensions, call `load_working_image()` (using a real fixture or a synthetic white image saved to a temp file), assert returned image dimensions equal the crop dimensions, not the original
- [X] T073 [US1] Refactor `action_analyze()` in `src/app/mod.rs` to use `session.load_working_image()`: replace the current `img.load_pixels()` call and the subsequent conditional `crop_imm()` block with a single `session.load_working_image()?`; remove the now-unused `img` and `session_crop` locals; keep `base_img = Arc::new(working_img)`
- [X] T074 [US1] Fix scale-reference bounds validation in `action_confirm_scale()` in `src/app/ui.rs`: replace `s.image.width` / `s.image.height` with the effective (cropped) dimensions — `(crop.width, crop.height)` when `session.crop_region` is `Some`, `(session.image.width, session.image.height)` otherwise — so `ScaleReference::new()` validates clicked pixel coordinates against the image the user actually sees
- [X] T075 [US1] Refactor texture-loading in `render_image_with_clicks()` and `action_confirm_crop()` in `src/app/ui.rs` to use `session.load_working_image()` instead of the manual `session.image.load_pixels()` + conditional `crop_imm()` pattern; keep `render_image_with_crop_drag()` using `session.image.load_pixels()` without crop (it intentionally shows the full image for crop selection)

---

## Dependencies & Execution Order

### Phase Dependencies

- **Setup (Phase 1)**: No dependencies — start immediately
- **Foundational (Phase 2)**: Depends on Phase 1 — BLOCKS all user stories
- **US1 (Phase 3)**: Depends on Foundational — no other story dependency
- **US2 (Phase 4)**: Depends on Foundational AND US1 complete (constitution Principle V)
- **US3 (Phase 5)**: Depends on US2 complete
- **US4 (Phase 6)**: Depends on US3 complete
- **Polish (Phase 7)**: Depends on all desired user stories complete

### User Story Dependencies

- **US1**: No story dependencies — start after Phase 2
- **US2**: Requires US1 (detection runs on a scaled image; `ScaleReference` is prerequisite)
- **US3**: Requires US2 (classifies elements produced by detection)
- **US4**: Requires US3 (generates 3D from clarified `FloorPlan`)

### Within Each User Story

- **Tests MUST be written and confirmed FAIL before any implementation task begins** (Constitution Principle I)
- Domain types and validation before service/pipeline logic
- Core processing logic before UI integration
- Session save/load reflects whatever state exists at that story's completion point
- Story complete and all tests passing before advancing to next priority

### Parallel Opportunities

- T002, T003 can run in parallel with each other (Phase 1)
- T005, T006, T008 can run in parallel within Phase 2
- T009, T010 (US1 tests) can run in parallel before any US1 implementation
- T011, T012 can run in parallel (different files) within US1
- T015 (session test) can be written in parallel with T013, T014
- T018, T019, T020, T021 (all US2 tests) can run in parallel
- T022, T023 can run in parallel within US2 (different modules)
- T024, T025 can run in parallel within US2 (different modules)
- T027, T028 can run in parallel within US2 (different modules)
- T033, T034 (US3 tests) can run in parallel
- T037, T038 (US4 tests) can run in parallel
- T040, T041 can run in parallel within US4 (different files)
- T045, T046, T047, T048 (Polish) can all run in parallel
- T056, T057, T058, T059 (Phase 10 rayon tasks) can all run in parallel — different files
- T062, T065 can run in parallel (different handlers in mod.rs)

---

## Parallel Example: User Story 2 Tests

```bash
# Launch all US2 tests in parallel (all must fail before implementation):
Task: "Write test_detection.rs — ≥90% wall detection rate (SC-002)"        # T018
Task: "Write test_detection_fallback.rs — rule-based mode works"           # T019
Task: "Write test_interior_exterior.rs — ≥90% region inference (SC-005)"  # T020
Task: "Write test_ocr.rs — room label + dimension extraction"              # T021

# Then launch parallel implementation tasks within US2:
Task: "Implement rule-based patterns in src/detection/rules/patterns.rs"   # T023
Task: "Implement OCR extractor in src/ocr/extractor.rs"                   # T027
```

---

## Implementation Strategy

### MVP First (User Story 1 Only)

1. Complete Phase 1: Setup
2. Complete Phase 2: Foundational (CRITICAL — blocks all stories)
3. Complete Phase 3: US1 — Import and Scale + Session Save/Load
4. **STOP and VALIDATE**: Run `cargo test test_scaling` and `cargo test test_session`
5. Demo: load a blueprint, click to scale, verify pixel→world conversion, save and reload session

### Incremental Delivery

1. Setup + Foundational → skeleton app compiles and opens
2. US1 complete → can load and scale any blueprint; save/load works
3. US2 complete → automated detection, OCR, and region inference
4. US3 complete → full clarification loop with adaptive learning
5. US4 complete → 3D model generation and SketchUp-ready export

### Parallel Strategy (Two Developers)

After Foundational:
- Dev A: US1 tests → US1 implementation → session save/load
- Dev B: begin writing US2 tests (can start test stubs against expected interfaces)
- Once US1 passes: both work on US2 implementation in parallel (T022–T032 have many [P] tasks)

---

## Notes

- `[P]` = different files, no incomplete-task dependencies — can run truly in parallel
- `[USn]` label maps each task to its user story for traceability to spec acceptance scenarios
- **Red phase is required**: every test task must be run and confirmed FAIL before the first
  implementation task in that story begins (Constitution Principle I, non-negotiable)
- Commit after each story phase completes with all tests passing
- SC-007 (≤1 GB total size) constrains model selection: prefer MobileNet-class/EfficientNet-B0
  ONNX models ≤100 MB each; verify with T045 before considering any story done
- If `tract-onnx` lacks an op needed by a chosen model, switch that model to `ort` and update
  Complexity Tracking in plan.md
