# Feature Specification: Blueprint Image to 3D Model

**Feature Branch**: `001-blueprint-to-3d`  
**Created**: 2026-03-30  
**Status**: Draft  
**Input**: User description: "This is a rust based program that takes an image of a blueprint in JPG format or PNG format that is top-down. It imports the picture. IT then scales the blueprint by asking the user for a known dimension. The program then traces the 2 dimensional lines along the visible contiguous dark lines on the image. Using common architectural conventions, it figures out the placement of fireplaces, windows, doors, walls, sliding doors, closets, staircases, chimneys, and courtyards. It can infer what is inside the house and what is outside. It may ask the user to identify certain line segments to aid it in its discovery of the outlines of the blueprint. After converting the image to the vector object representation fo the blueprint, it will create a basic 3 dimensional model. the output format should be researched and easy to load and visualize in Sketchup. The format can be an obj or stl."

## Clarifications

### Session 2026-03-30

- Q: Can the user save their work mid-session and resume later, or is the workflow single-session only? → A: Manual save/load — user explicitly saves progress at any point and can reload to resume from where they left off.

- Q: Can the user override the default 8 ft wall extrusion height, and if so, how? → A: User specifies a single custom wall height once before 3D generation begins; applies uniformly to all walls.
- Q: Should walls in the 3D model have thickness, and if so, how is it determined? → A: Derive wall thickness from detected double-line spacing in the blueprint where available; fall back to a default thickness (6 in / 15 cm) for single-line walls.
- Q: Should the OBJ export visually differentiate element types with colors/materials? → A: OBJ export includes an MTL file assigning a distinct color per element type; STL is uncolored by nature of the format.
- Q: What image resolution does the 10-minute performance target in SC-001 apply to? → A: Standard scan quality — up to 4000×4000 pixels (typical 300 DPI architectural scan).
- Q: Must ML models be bundled with the app for offline use, or can they be downloaded? → A: Downloaded on first run — internet required for initial model setup; fully offline after that.
- Q: Can OCR-extracted dimension values replace the manual scale calibration step? → A: No — manual scale entry (FR-002) is always required; OCR dimensions supplement and validate the user-provided scale but never replace it.
- Q: Should room label recognition use a fixed type list or be fully open? → A: Fixed list with open fallback — known room types get full classification benefits; unrecognized labels are stored as raw text without type inference.
- Q: How should OCR failures (unreadable text) be handled? → A: Silent skip during processing; unreadable text regions are included in the end-of-processing summary report.
- Q: Do room labels from OCR carry through into the 3D export? → A: No — room labels are 2D floor plan data only and are not represented in the 3D model or export files.
- Q: What discrepancy threshold triggers the OCR vs user-scale warning in FR-022? → A: ±5% — aligned with the SC-004 dimension tolerance so the warning fires precisely when the model risks failing that criterion.
- Q: Is there a total size constraint on the installed application including ML models? → A: Yes — total footprint (binary + all downloaded ML models) MUST NOT exceed 1 GB. Individual models ≤100 MB each; combined model budget ≤700 MB. Prefer lightweight architectures (MobileNet-class, EfficientNet-B0, YOLO-nano). Large transformer models are excluded. System deps (Tesseract, Leptonica) are not counted.
- Q: How is the confidence threshold for triggering user classification prompts determined? → A: Adaptive — system starts with a default threshold and auto-tunes it based on accumulated user corrections over time.
- Q: Should the 85% wall detection accuracy target in SC-002 be revised given ML augmentation? → A: Raise to 90% — reflects expected improvement from the hybrid ML + rule-based approach.
- Q: Is correction history for adaptive threshold tuning scoped per session or global across all projects? → A: Global — correction history persists across all sessions and projects, improving threshold accuracy over time.
- Q: Is rule-based-only fallback a deliberate user choice or automatic only? → A: Automatic only — rule-based mode activates only when ML models are unavailable; not a user-selectable mode.

### Session 2026-03-31

- Q: Which M1 hardware acceleration mechanism should be used for performance optimisation? → A: `rayon` for data-parallel CPU computation across all pipeline stages + Apple Accelerate/vDSP framework (via FFI, `-framework Accelerate`) for SIMD-accelerated math operations (NLM patch distances, Canny gradient convolutions).
- Q: What is the measurable performance target for the optimised pipeline? → A: ≥2× faster wall-clock time on the full analysis pipeline for a 2000×2000 px input image compared to the single-threaded baseline.
- Q: Must the M1-optimised build still run on Intel Macs? → A: No — M1-only (Apple Silicon, `aarch64-apple-darwin`); no Intel fallback is required.
- Q: Which pipeline stages should receive `rayon` data parallelism in addition to the existing NLM threading? → A: All CPU-bound stages: Canny gradient map computation, classifier feature extraction (batch element scoring), and 3D mesh/face generation loops.
- Q: How should the ≥2× speedup requirement be validated? → A: Automated `criterion` benchmark in `benches/pipeline_bench.rs` measuring baseline (single-threaded, no vDSP) vs optimised (rayon + vDSP) runs; baseline must be ≥2× slower.
- Q: What does the 5-minute ML timeout clock measure? → A: Total elapsed wall-clock time from when the user clicks "Analyze" (covers OCR + denoise + trace + ML classify).
- Q: What happens to ML-classified elements when the timeout fires? → A: Keep ML results for high-confidence elements (confidence ≥ 0.7); re-classify low-confidence elements using rule-based heuristics.
- Q: How should the ML timeout be surfaced to the user? → A: Non-blocking inline notice in the analysis progress UI + a note included in the end-of-processing summary (FR-015).
- Q: Is the 5-minute timeout value fixed or user-configurable? → A: Fixed compile-time constant (5 minutes); not exposed in the UI or as a CLI flag.
- Q: Which confidence threshold separates "keep ML result" from "re-classify with rules" on timeout? → A: Separate fixed threshold of 0.7 confidence specifically for the timeout re-classification decision (distinct from FR-007's adaptive threshold).

- Q: At which stage should OCR-detected text strokes be suppressed to prevent them from appearing as 3D objects? → A: Mask OCR text bounding boxes in the raster image before line tracing begins.
- Q: Should OCR run on the raw image before NLM denoising so text is masked prior to the most expensive preprocessing step? → A: Yes — pipeline order is OCR on raw image → apply text masks → NLM denoise → Canny → trace.
- Q: Should mask boxes be padded beyond the tight OCR bounding box? → A: Adaptive padding of half the median character height for that text region, applied on all sides.
- Q: What pixel value should masked areas be filled with? → A: Median of the 1-pixel border pixels of the padded bounding box — adapts to local background colour.
- Q: What 3D geometry should represent detected door openings? → A: Wall opening only — a gap through the full wall thickness; no arc or door slab geometry is rendered.
- Q: Is the image crop step mandatory before analysis, or optional? → A: Optional — user may skip and proceed with the full original image.
- Q: How does the user define the crop bounding box? → A: Click and drag on the displayed image to draw the crop rectangle.
- Q: How should analysis progress be shown given that the pipeline runs synchronously? → A: Named pipeline stages, one per frame, with a time-based percentage progress bar — each stage runs in a background thread; the main thread animates a progress bar based on elapsed time vs estimated stage duration and advances to the next named stage label when the thread completes.
- Q: Which specific ML model should be downloaded and installed as the architectural element classifier? → A: MobileNetV2-12 from the ONNX Model Zoo (via Hugging Face mirror at `https://huggingface.co/onnxmodelzoo/mobilenetv2-12/resolve/main/mobilenetv2-12.onnx`, ~13 MB). Output adapter maps the 1000 ImageNet classes to the 10 architectural element classes by grouping; confidence will be low until a fine-tuned model replaces it.
- Q: Which denoising algorithm should be applied as the image preprocessing step before edge detection? → A: Non-local means — highest quality denoising; removes scan noise while preserving structural line edges.
- Q: How should low-contrast edges be suppressed during line tracing? → A: Per-image adaptive Canny thresholds derived from the image's own gradient magnitude distribution (e.g., only edges above the 70th-percentile gradient magnitude are retained); no fixed global threshold, no user control.
- Q: How strictly should collinear segments of the same element type be merged after detection? → A: Strict — merge only same-type segments that are ≤2° of alignment and within ≤10 pixels gap (scaled to image resolution); prevents false merges of closely parallel walls.
- Q: How should the low-confidence element be highlighted on the image during clarification prompts? → A: Semi-transparent red filled rectangle over the element bounds.
- Q: After the user confirms the crop selection, how should the displayed image update? → A: Immediately replace the full image with the cropped region (no transition).
- Q: After the crop is confirmed and the cropped image is shown, can the user go back to view the full original image? → A: Yes — a "Reset Crop" button is available to revert to the full original image.

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Import and Scale a Blueprint (Priority: P1)

A user has a scanned or photographed top-down blueprint image (JPG or PNG) and wants to bring it into the tool. They load the file. The program first offers an optional crop step where the user can drag a bounding box to remove extraneous content such as title blocks, legends, or border annotations before proceeding. The user may skip this step. The program then prompts them to identify two points whose real-world distance is known (e.g., "I know this wall is 12 feet long"). The system uses that input to establish a pixel-to-real-world scale so all subsequent measurements are accurate.

**Why this priority**: Without a correctly scaled blueprint, the output 3D model has no meaningful real-world dimensions. This is the entry point for all subsequent processing.

**Independent Test**: Can be fully tested by loading a blueprint image, providing two reference points and a known distance, and verifying the computed scale matches the expected ratio. Delivers value as a standalone measurement and calibration tool.

**Acceptance Scenarios**:

1. **Given** a valid JPG or PNG blueprint file path, **When** the user loads it, **Then** the image is displayed and the user is presented with an optional crop step offering a drag-to-select crop region and a "Skip" button.
2. **Given** the crop step is displayed, **When** the user clicks and drags to draw a bounding box and confirms, **Then** the displayed image is immediately replaced with the cropped region only (no transition), a "Reset Crop" button becomes visible, and all further processing uses only the cropped image.
3. **Given** the crop step is displayed, **When** the user clicks "Skip", **Then** the full original image is used unchanged and the user proceeds to scale calibration.
4. **Given** a crop has been confirmed and the cropped image is displayed, **When** the user clicks "Reset Crop", **Then** the display immediately reverts to the full original image and the crop region is cleared.
4. **Given** the user identifies two reference points on the image and enters the real-world distance between them, **When** the scale is confirmed, **Then** the system records the pixel-to-unit ratio and uses it for all subsequent measurements.
5. **Given** an unsupported file format is provided, **When** the user attempts to load it, **Then** the system reports a clear error message indicating supported formats (JPG, PNG).
6. **Given** the user provides an invalid reference input (zero distance, identical points, or out-of-bounds coordinates), **When** submitted, **Then** the system rejects the input with an informative error and re-prompts.

---

### User Story 2 - Automated Line Tracing and Architectural Element Detection (Priority: P2)

After scaling, the user initiates analysis. The program automatically traces all visible contiguous dark lines in the blueprint image to produce a vector representation of the line work. It then applies architectural conventions to classify the detected elements: walls, doors, windows, sliding doors, fireplaces, closets, staircases, chimneys, and courtyards. The system also distinguishes which areas are interior (inside the building) versus exterior.

**Why this priority**: This is the core intelligence of the tool. Without automated detection, the user would have no advantage over manual redrawing.

**Independent Test**: Can be tested by providing a known blueprint and verifying that the set of detected elements matches the expected ground truth. The vector output can be rendered as an overlay on the original image to visually confirm accuracy.

**Acceptance Scenarios**:

1. **Given** a scaled blueprint image with clearly visible dark lines, **When** analysis is triggered, **Then** the system produces a vector line map covering all major contiguous dark line segments.
2. **Given** the vector line map, **When** architectural classification is applied, **Then** each segment or region is labeled as one of: wall, door, window, sliding door, fireplace, closet, staircase, chimney, courtyard, or unclassified.
3. **Given** the classified elements, **When** interior/exterior inference is applied, **Then** enclosed regions forming the building footprint are marked as interior and everything outside is marked exterior.

---

### User Story 3 - User-Assisted Clarification of Ambiguous Segments (Priority: P3)

When automated analysis is uncertain about a particular line segment or region — for example, whether a gap in a wall is a door or a window — the system pauses and presents the ambiguous element to the user, asking them to confirm or correct the classification. The user provides the correct label and processing continues.

**Why this priority**: Real-world blueprints vary widely in quality and style. User-assisted correction ensures the final model is accurate even when automated inference fails.

**Independent Test**: Can be tested by providing a blueprint with deliberately unusual or ambiguous features and verifying the system surfaces those ambiguities for user input, incorporates the user's answers, and excludes skipped segments from the 3D model with a summary.

**Acceptance Scenarios**:

1. **Given** a line segment whose type cannot be determined with confidence, **When** the system flags it, **Then** the user is shown the element highlighted with a semi-transparent red fill over its bounding box on the original image so they can clearly see what needs identification, and asked to select from a list of possible element types.
2. **Given** the user provides a classification, **When** confirmed, **Then** the system updates the element map and continues processing.
3. **Given** the user skips a clarification prompt, **When** processing continues, **Then** the segment is marked as unclassified and excluded from 3D generation, with a summary of all unclassified segments reported at the end.

---

### User Story 4 - 3D Model Generation and Export (Priority: P4)

Once the 2D vector blueprint is complete and all elements are classified, the user initiates 3D model generation. The system extrudes walls to a standard architectural height, represents door and window elements as openings, and adds basic geometry for other elements (fireplaces, staircases, closets, etc.). The resulting model is exported as an OBJ or STL file that opens in SketchUp without errors.

**Why this priority**: This is the final deliverable. However it depends entirely on the success of all preceding steps.

**Independent Test**: Can be tested end-to-end using a simple rectangular floor plan with known walls and a door opening, then loading the OBJ or STL output in SketchUp and verifying wall geometry, dimensions, and door opening are present and correctly scaled.

**Acceptance Scenarios**:

1. **Given** a complete classified vector blueprint, **When** 3D generation is triggered, **Then** walls are extruded to a standard height and door/window elements are represented as openings in the wall geometry.
2. **Given** the 3D model is ready and the user selects OBJ export, **When** the export is written, **Then** a valid OBJ file is produced that SketchUp can import without errors.
3. **Given** the 3D model is ready and the user selects STL export, **When** the export is written, **Then** a valid STL file is produced that SketchUp can import without errors.
4. **Given** the exported file is opened in SketchUp, **When** dimensions are measured, **Then** real-world dimensions match the scaled measurements from the original blueprint within ±5% tolerance.

---

### Edge Cases

- What happens when the blueprint image is very low contrast (light lines on a light background)? *(Resolved: per-image adaptive Canny thresholds derived from the image's gradient magnitude distribution (FR-025) suppress low-contrast edges automatically. Non-local means denoising (FR-025) further reduces noise before edge detection. Regions where contrast is insufficient will produce no traced segments; the user may need to pre-process the source image externally.)*
- How does the system handle blueprints where walls are drawn as double lines representing wall thickness? *(Resolved: double-line spacing is used to derive wall thickness; single-line walls fall back to 6 in default.)*
- What if the user provides an incorrect reference dimension (e.g., a negative or zero value)?
- How does the system handle blueprints with dimension text or annotations overlaid on top of line work? *(Resolved: OCR runs first to locate all text bounding boxes; those regions are masked in the raster image before line tracing begins, preventing text strokes from being detected as structural segments. Text content is parsed separately for scaling and room labeling but never rendered as 3D geometry.)*
- What if ML classification is still running when the 5-minute wall-clock timeout expires? *(Resolved: ML stage is interrupted; elements with confidence ≥ 0.7 are kept; remainder are re-classified with rule-based heuristics (FR-028). A non-blocking inline notice is shown and the event is recorded in the FR-015 summary.)*
- What if the image is rotated or skewed rather than aligned with horizontal/vertical axes?
- How does the system behave when a blueprint contains multiple separate building footprints?

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: System MUST accept JPG and PNG format raster images as input via a file path.
- **FR-024**: After loading the image, system MUST display a crop step where the user can click and drag on the displayed image to define a rectangular crop region, then confirm to crop the image to that region before proceeding. The crop step is optional — a "Skip" button MUST be available so the user can proceed with the full original image unchanged. Upon confirmation, the image panel MUST immediately replace the full image with the cropped region only (no fade or transition). A "Reset Crop" button MUST be visible whenever a crop is active; clicking it MUST immediately revert the display to the full original image and clear the crop region, allowing the user to re-crop or skip before analysis begins. All subsequent processing (scale calibration, line tracing, OCR, 3D generation) operates on the post-crop image. The crop region MUST be saved as part of the session file (FR-016) so the same crop is restored on reload.
- **FR-002**: System MUST display the blueprint image in an interactive graphical window and allow the user to click two points on the image to identify the reference positions, then enter the real-world distance between those points to establish scale.
- **FR-003**: System MUST reject invalid scale reference inputs (zero distance, identical points, out-of-bounds coordinates) with informative error messages and re-prompt the user.
- **FR-004**: System MUST trace all visible contiguous dark line segments from the blueprint image and represent them internally as a vector line map, using a hybrid approach that combines rule-based image processing with machine learning models to improve detection accuracy. The preprocessing pipeline MUST execute in this exact order before tracing begins: (1) OCR MUST run on the raw (undenoised) image as early as possible to locate all text bounding boxes (FR-020); (2) each OCR text bounding box MUST be expanded on all sides by adaptive padding equal to half the median character height of that text region, then the padded region MUST be filled with the median pixel value of its 1-pixel border — eliminating text strokes from subsequent processing; (3) NLM denoising MUST be applied to the text-masked image (FR-025); (4) Canny edge detection thresholds MUST be derived per-image from the masked-and-denoised image's gradient magnitude distribution — only edges above the 70th-percentile gradient magnitude are retained (FR-025). After tracing, collinear segments of the same classified type MUST be merged per FR-026 to minimize object count.
- **FR-005**: System MUST classify detected line segments and regions into architectural element types: walls, doors, windows, sliding doors, fireplaces, closets, staircases, chimneys, and courtyards, using a hybrid approach where ML-based classifiers complement rule-based architectural convention matching.
- **FR-020**: System MUST perform optical character recognition (OCR) on the blueprint image to extract all readable text, including room labels (e.g., "Kitchen", "Bedroom") and numerical dimension values (e.g., "12'-6\"", "3.75m").
- **FR-021**: System MUST match recognized room labels against a fixed list of known room types (e.g., bedroom, kitchen, bathroom, living room, dining room, garage, hallway, study, laundry) to annotate the corresponding enclosed regions in the floor plan, improving the accuracy of architectural element classification in those areas. Labels not matching a known type MUST be stored as raw text annotations on the region without triggering type-specific inference. Room label data is 2D floor plan data only and is not carried into the 3D model or export files.
- **FR-022**: System MUST extract numerical dimension values found on the blueprint and use them to validate the user-provided scale reference. If OCR-derived scale differs from the user-provided scale by more than ±5%, the system MUST warn the user before proceeding. Manual scale entry (FR-002) is always required; OCR dimensions supplement but never replace it.
- **FR-023**: When OCR cannot confidently read a text region, the system MUST silently skip it during processing and include all such regions in the end-of-processing summary report (FR-015) for user awareness.
- **FR-025**: After OCR text masking (FR-004 step 2), system MUST apply a non-local means denoising pass to the text-masked image before edge detection or line tracing begins. This step reduces scan noise and compression artifacts while preserving sharp line edges. Following denoising, Canny edge detection thresholds MUST be computed per-image from the denoised image: the high threshold is set to the 90th percentile of the gradient magnitude distribution and the low threshold to the 70th percentile, so that only edges with clear contrast demarcation are retained and low-contrast ambiguous areas are ignored.
- **FR-028**: The analysis pipeline MUST enforce a 5-minute (300-second) wall-clock timeout measured from when the user initiates analysis ("Analyze" click). If the timeout expires while ML-based element classification is still running, the ML stage MUST be interrupted immediately. Elements already classified by ML with confidence ≥ 0.7 MUST be retained as-is. Elements classified below 0.7 confidence, and any elements not yet reached by the ML stage, MUST be re-classified using the rule-based heuristic classifier (FR-005). The timeout threshold value (300 s) and the confidence cutoff (0.7) MUST be compile-time constants, not user-configurable. When the timeout fires, the system MUST display a non-blocking inline notice in the analysis progress UI (e.g., "ML timeout — continuing with rule-based detection") and MUST include a note in the end-of-processing summary (FR-015) stating that ML classification was cut short and which fallback was applied.
- **FR-027**: The analysis pipeline MUST use `rayon` data parallelism for all CPU-bound stages: Canny gradient map computation, classifier feature extraction (per-element batch scoring), and 3D mesh/face generation loops. The NLM denoising stage MUST also be migrated from `std::thread::scope` to `rayon::par_iter` to unify the threading model. Apple's Accelerate/vDSP framework (linked via `-framework Accelerate`) MUST be used for the inner patch-distance and convolution math in the NLM and Canny stages on `aarch64-apple-darwin` targets; no Intel fallback is required. All shared mutable state across rayon threads MUST be protected by appropriate synchronisation primitives (`Mutex`, `RwLock`, or lock-free atomics).
- **FR-026**: After line tracing and classification, the system MUST merge collinear segments of the same architectural element type to reduce the total object count in the vector representation. Two segments are eligible for merging if and only if: (a) they have the same classified type; (b) their angular alignment difference is ≤2°; and (c) the gap between their nearest endpoints is ≤10 pixels (scaled to the image resolution). Merged segments replace their constituents in the vector line map and in the exported 3D geometry.
- **FR-006**: System MUST infer interior versus exterior regions based on the building footprint defined by the classified wall elements.
- **FR-029**: During each low-confidence clarification prompt (FR-007), the image panel MUST render a semi-transparent red filled rectangle over the bounds of the ambiguous element so the user can clearly see which element requires identification. The highlight MUST be drawn on the original (pre-crop or post-crop as applicable) image and MUST be visible at the same time as the classification selection UI.
- **FR-007**: System MUST identify line segments or regions whose classification confidence falls below an adaptive threshold and prompt the user to provide a manual classification for each. The threshold starts at a sensible default and is automatically adjusted over time based on a global correction history that persists across all sessions and projects, reducing unnecessary prompts as the system accumulates corrections.
- **FR-008**: System MUST allow the user to skip a manual classification prompt, marking the segment as unclassified.
- **FR-009**: System MUST prompt the user to confirm or override the default wall extrusion height (8 ft / 2.44 m) before 3D generation begins; the chosen height applies uniformly to all walls. System MUST extrude wall elements to that height with thickness derived from detected double-line spacing in the blueprint; where no double-line is detected, a default wall thickness of 6 inches (15 cm) is applied. Door elements MUST be represented as openings (gaps) through the full wall thickness only; no arc, door slab, or swing-path geometry is rendered. Window elements MUST likewise be represented as openings through the full wall thickness.
- **FR-010**: System MUST represent non-wall classified elements as recognizable basic shapes in the 3D model: staircases as stepped geometry, chimneys as raised rectangular boxes, fireplaces as raised hearth slabs, closets as enclosed box volumes, and courtyards as recessed or open floor areas.
- **FR-011**: System MUST allow the user to choose between OBJ and STL as the export format.
- **FR-012**: System MUST export the 3D model as a valid OBJ file when OBJ format is selected, accompanied by an MTL material file that assigns a distinct color to each architectural element type (e.g., walls=grey, doors=brown, windows=blue, staircases=tan, fireplaces=red, closets=beige, chimneys=dark grey, courtyards=green).
- **FR-013**: System MUST export the 3D model as a valid STL file when STL format is selected.
- **FR-014**: Exported files MUST be importable into SketchUp 2020 or later without import errors.
- **FR-015**: System MUST produce a summary at the end of processing listing all detected and classified elements, recognized room labels and their associated regions, any unclassified segments, any text regions where OCR could not confidently extract readable content, the path to the exported file, and — if the 5-minute ML timeout (FR-028) was triggered — a note stating that ML classification was interrupted and rule-based fallback was applied to the affected elements.
- **FR-018**: On first launch, system MUST detect whether required ML models are present locally; if not, it MUST download and cache them before allowing analysis to proceed, displaying clear progress feedback during the download.
- **FR-019**: System MUST inform the user if ML model download fails (e.g., no internet connection) and automatically fall back to rule-based detection only, with a visible warning that detection accuracy may be reduced. Rule-based fallback is not a user-selectable mode; it activates only when ML models are unavailable.
- **FR-016**: System MUST allow the user to explicitly save their current session state (scaled image, vector line map, element classifications, and any clarification answers) to a file at any point during the workflow.
- **FR-017**: System MUST allow the user to load a previously saved session file and resume processing from the point at which it was saved.

### Key Entities

- **BlueprintImage**: The source raster image (JPG or PNG) representing a top-down architectural floor plan.
- **CropRegion**: An optional axis-aligned rectangle (x, y, width, height in image pixels) defined by the user via click-and-drag after loading. When present, all processing operates on the sub-image bounded by this region. Stored in the session file so it is restored on reload.
- **ScaleReference**: A pair of positions on the image paired with a real-world distance and unit, used to derive the pixels-per-unit conversion ratio.
- **LineSegment**: A detected contiguous dark path in the raster image, represented as a sequence of points in image coordinates.
- **ArchitecturalElement**: A classified line segment or region with an assigned type (wall, door, window, sliding door, fireplace, closet, staircase, chimney, courtyard, or unclassified), real-world position, and dimensions derived from the scale reference.
- **FloorPlan**: The complete 2D vector representation of all classified architectural elements and their spatial relationships, including labeled interior and exterior regions.
- **Model3D**: A 3D mesh derived from the floor plan by extruding and geometrically representing architectural elements.
- **ExportFile**: The final OBJ or STL file written to disk containing the complete 3D model geometry.
- **SessionFile**: A saved snapshot of all processing state — scale reference, vector line map, element classifications, and clarification answers — allowing the user to resume work at a later time.
- **CorrectionHistory**: A global, persistent record of all user-provided manual classifications across all sessions and projects, used to tune the adaptive confidence threshold over time. Stored locally on the user's machine independently of any individual session.
- **TextAnnotation**: A piece of text recognized by OCR from the blueprint image, with its position, content, and type (room label or dimension value). Room labels are associated with enclosed floor plan regions; dimension values are associated with line segments or overall scale derivation.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: A user can load a blueprint image (up to 4000×4000 pixels), complete scaling, and receive an exported 3D model file in under 10 minutes for a typical single-story residential floor plan.
- **SC-002**: At least 90% of wall segments in a clear, high-contrast blueprint are automatically detected and correctly classified without requiring user intervention, reflecting the improvement expected from the hybrid ML + rule-based detection approach.
- **SC-003**: Exported OBJ and STL files open in SketchUp 2020 or later without import errors for 100% of successfully processed blueprints.
- **SC-004**: Wall dimensions in the exported 3D model match the real-world dimensions specified by the scale reference within ±5% tolerance.
- **SC-005**: The system correctly distinguishes interior from exterior regions in at least 90% of floor plans with a clearly enclosed building footprint.
- **SC-006**: Users with no prior experience with the tool can complete the full workflow (load, scale, review clarifications, export) on their first attempt for a straightforward floor plan.
- **SC-007**: The total installed footprint of the application — application binary plus all downloaded ML model files — MUST NOT exceed 1 GB. ML models MUST be lightweight (individually ≤100 MB; combined ≤700 MB) to preserve headroom for the binary and OCR language data.
- **SC-008**: The full analysis pipeline (OCR → denoise → trace → classify → floor plan) on a 2000×2000 px input image MUST run at least 2× faster wall-clock time in the optimised build (rayon + vDSP) compared to the single-threaded baseline. Validated by the `criterion` benchmark in `benches/pipeline_bench.rs`; the benchmark MUST NOT regress below the 2× threshold in CI.

## Assumptions

- Blueprint images are top-down (plan view) floor plans, not elevation drawings or 3D perspective images.
- Input blueprints have sufficient contrast — dark lines on a substantially lighter background — for automated line detection to function effectively.
- Only single-story floor plans are in scope for v1; multi-story buildings depicted in a single image are out of scope.
- Standard residential architectural conventions are used for element classification (e.g., door arc symbols, window line patterns, stair hatch symbols).
- The default standard wall extrusion height is 8 feet (approximately 2.44 m); the user may override this with a single custom value that applies uniformly to all walls before 3D generation begins.
- The tool targets single building footprints; blueprints containing multiple separate structures may yield incomplete or partially incorrect results.
- The user has SketchUp 2020 or later installed to open the exported files.
- Real-world units may be feet or meters; the user specifies the unit when providing the reference dimension.
- The program runs as a local desktop application with direct access to the local file system.
- Annotations, dimension lines, and text overlaid on the blueprint image are treated as noise to be filtered where possible; they may degrade detection quality if heavily overlapping structural line work.
- The performance target of 10 minutes applies to images up to 4000×4000 pixels (standard 300 DPI architectural scan); larger images may take longer.
- The optimised build targets Apple Silicon Macs (`aarch64-apple-darwin`) only; Intel Mac support is not required. Performance-sensitive code paths MAY use `#[cfg(target_arch = "aarch64")]` guards and Apple Accelerate/vDSP without providing an x86_64 fallback.
- Line tracing and architectural element classification use a hybrid approach: rule-based methods (geometric heuristics, architectural convention matching) are complemented by pre-trained machine learning models. The initial model is MobileNetV2-12 (ONNX opset 12, ~13 MB) downloaded from the Hugging Face ONNX Model Zoo mirror. Its 1000 ImageNet output classes are adapted to the 10 architectural element classes via a grouping adapter; accuracy improves when a domain-fine-tuned replacement model is installed.
- ML models are not bundled with the application. On first launch, the application downloads required models from the internet and caches them locally. All subsequent runs are fully offline.
- ML model selection MUST prioritize small, efficient architectures (e.g., MobileNet-class, EfficientNet-B0, YOLO-nano) to remain within the 1 GB total size budget (SC-007). Large transformer-based models are excluded.
- The 1 GB size budget covers: application binary + all downloaded ML model files. System-level dependencies installed separately by the user (e.g., Tesseract, Leptonica) are not counted toward this budget.
