# blueprint2mod

Convert architectural blueprint images (JPG/PNG) into 3D models (OBJ/STL) ready for import into SketchUp.

## What it does

1. **Load** a top-down blueprint image (JPG or PNG)
2. **Crop** (optional) — drag to remove title blocks, legends, or border annotations before processing
3. **Scale** — click two reference points on the image and enter the known real-world distance between them
4. **Analyze** — the pipeline automatically:
   - Runs OCR to locate and mask dimension text (so text strokes are not traced as walls)
   - Denoises the image with Non-Local Means (NLM)
   - Traces dark line segments using adaptive Canny edge detection
   - Classifies segments as walls, doors, windows, sliding doors, fireplaces, closets, staircases, chimneys, courtyards, or unclassified using a hybrid ML + rule-based approach
   - Merges collinear segments of the same type
   - Infers interior vs exterior regions
5. **Clarify** — for low-confidence elements the app pauses and shows the element highlighted on the image in red so you can identify it
6. **Generate** — extrude the floor plan to a 3D model at a chosen wall height
7. **Export** — write an OBJ+MTL file (recommended for SketchUp) or a binary STL file

## System requirements

- macOS (Apple Silicon recommended; Apple Accelerate is used for NLM/Canny acceleration on aarch64)
- Rust stable 1.75+
- Tesseract OCR + Leptonica (for text detection)

```bash
brew install tesseract
```

## Build

```bash
# Development build
cargo build

# Optimised release build (uses LTO + Apple Accelerate on Apple Silicon)
cargo build --release
```

## Run

```bash
# Open the GUI
./target/release/blueprint2mod

# Open with a blueprint image pre-loaded
./target/release/blueprint2mod path/to/blueprint.jpg

# Resume a saved session
./target/release/blueprint2mod --session project.b2m

# Pre-select output format and path (GUI still required for scale/clarification)
./target/release/blueprint2mod blueprint.jpg --format obj --output /tmp/model.obj
```

## Workflow walkthrough

### 1. Open a blueprint image

Use **File > Open Image** or pass a path on the command line. Supported formats: JPG, PNG.

### 2. Optional crop

Drag a rectangle on the image to select the area you want to keep (e.g. cut out the title block or border). Click **Confirm Crop** — the display immediately updates to show only the selected region. A **Reset Crop** button appears in the toolbar if you want to undo the crop and re-select.

### 3. Scale calibration

Click two points on the image whose real-world distance you know (e.g. the ends of a labelled wall), then type the distance and choose the unit (meters or feet). Click **Confirm Scale**.

### 4. Analysis

Click **Analyze**. A progress bar advances through four named stages:

| Stage | What happens |
|---|---|
| Step 1/4: OCR & masking | Tesseract reads all text; bounding boxes are masked before line tracing |
| Step 2/4: Denoising & tracing | NLM denoising + adaptive Canny edge detection + line segment extraction |
| Step 3/4: Classifying | ML classifier (MobileNetV2) + rule-based heuristics; 5-minute timeout with rule-based fallback |
| Step 4/4: Floor plan | Collinear segment merging + interior/exterior inference |

If ML models are not installed the app runs in rule-based-only mode (a warning banner is shown).

### 5. Clarification prompts

For each element whose classification confidence is below the adaptive threshold you will see the ambiguous element highlighted in red on the blueprint image. Select the correct type from the button list, or click **Skip** to mark it as unclassified.

### 6. 3D generation

Enter the wall height (default 2.44 m / 8 ft) and click **Generate & Export**. A file-save dialog opens.

### 7. SketchUp import

- For OBJ export: place the `.obj` and `.mtl` files in the same directory. In SketchUp choose **File > Import**, select the `.obj` file, and set the import unit to **Meters** in the options dialog.
- For STL export: import the `.stl` file and set units to **Meters**.

## Saving and resuming work

**File > Save Session** writes a `.b2m` file containing the scale reference, detected elements, clarification answers, and crop region. Reload with **File > Load Session** or `--session project.b2m`.

## Tests

```bash
# Unit tests (59 tests)
cargo test --lib

# Integration tests (requires test_fixtures/)
cargo test --test integration

# Performance benchmark (SC-008: ≥2× speedup on Apple Silicon)
cargo bench --bench pipeline_bench
```

## Project structure

```
src/
├── app/           UI state machine (egui/eframe)
├── blueprint/     Domain types: image, scale, elements, floor plan
├── detection/     Line tracing, ML + rule classifier, preprocessor
├── ocr/           Tesseract OCR, room label + dimension parsing
├── model3d/       Floor plan → 3D mesh extrusion
├── export/        OBJ+MTL writer, binary STL writer
├── session/       JSON save/load (.b2m)
└── correction/    Adaptive confidence threshold, correction history
```

## Known limitations

- Single-story floor plans only (v1)
- Apple Silicon Mac recommended; Intel Mac builds work but without vDSP acceleration
- ML model (MobileNetV2-12) is a general-purpose ImageNet classifier adapted for architectural elements — accuracy improves significantly if replaced with a domain-fine-tuned model
- Multi-building blueprints may produce incomplete results
