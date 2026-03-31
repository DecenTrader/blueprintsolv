# blueprint2mod Development Guidelines

Auto-generated from feature plans. Last updated: 2026-03-31

## Active Technologies

**Confirmed versions from Cargo.lock (2026-03-31):**

- **Language**: Rust (stable, 1.75+)
- **GUI**: egui 0.28.1 + eframe 0.28.0 + egui_extras 0.28.0
  - Note: plan.md specified 0.34 but 0.28 was used; `from_id_source` instead of `from_id_salt` for ComboBox
- **Image I/O**: image 0.25.x
- **Image processing**: imageproc 0.25.x (Canny, morphological dilation)
- **OCR**: leptess 0.14.x (Tesseract 4/5 + Leptonica bindings)
  - Requires system `libtesseract` + `libleptonica` + `pkg-config` (`brew install tesseract`)
- **ML inference**: tract-onnx 0.21.10 (pure-Rust ONNX; no ML models currently downloaded)
  - Note: plan.md specified `ort` but `tract-onnx` was used per Cargo.toml
- **STL export**: stl_io 0.8.6
  - Note: plan.md specified 0.10.x but 0.8.x was resolved by Cargo
- **OBJ export**: custom writer (src/export/obj.rs)
- **Serialization**: serde 1.x + serde_json 1.x
- **HTTP (model download)**: reqwest 0.12 (stub only — no models currently downloaded)

## Model Size Observations (SC-007)

- Release binary: **8.4 MB** (stripped, LTO, opt-level=3)
- ML models: **0 MB** (download_models() is a no-op stub; no ONNX files present)
- **Total: 8.4 MB / 1 GB limit** — SC-007 PASS

## Project Structure

```text
blueprint2mod/
├── Cargo.toml
├── CLAUDE.md                      # This file
├── src/
│   ├── main.rs                    # Entry point
│   ├── app/                       # UI state machine (egui)
│   ├── blueprint/                 # Domain types (image, scale, elements)
│   ├── detection/                 # Line tracing + hybrid ML/rule classifier
│   │   ├── ml/                    # ONNX model download + inference (ort)
│   │   └── rules/                 # Rule-based architectural heuristics
│   ├── ocr/                       # leptess OCR + room label/dimension parsing
│   ├── model3d/                   # FloorPlan → 3D mesh generation
│   ├── export/                    # OBJ+MTL (custom) and STL (stl_io) writers
│   ├── session/                   # JSON save/load (.b2m files)
│   └── correction/                # Adaptive threshold + global correction history
├── tests/
│   └── integration/               # End-to-end tests with reference fixtures
└── test_fixtures/                 # Reference blueprints + expected outputs
```

## Commands

```bash
# Build
cargo build
cargo build --release

# Run
./target/release/blueprint2mod [IMAGE] [OPTIONS]
./target/release/blueprint2mod --session project.b2m

# Test
cargo test
cargo test --test integration

# Check (fast feedback, no linking)
cargo check
cargo clippy
```

## Code Style

- Follow standard Rust conventions (`rustfmt`, `clippy`)
- All public types derive `Debug`, `Clone`, `Serialize`, `Deserialize` where applicable
- Use `uuid::Uuid` for all entity IDs
- Error handling: use `anyhow::Result` for application-level errors; domain errors as
  typed enums with `thiserror`
- No `unwrap()` or `expect()` in production paths; use `?` propagation
- Unit tests inline in modules (`#[cfg(test)]`); integration tests in `tests/`

## Constitution

See `.specify/memory/constitution.md` (v1.1.0).

**Non-negotiable rules**:
1. **Test-First**: Write tests before implementation. Confirm Red before Green.
2. **Output Validation**: SC-001–SC-006 must be encoded as runnable assertions.
3. **Hybrid Detection**: ML + rule-based; rule-only fallback is a first-class test scenario.
4. **Accuracy Gates**: ≥90% wall detection, ±5% dimension tolerance, 100% SketchUp import.
5. **Incremental Delivery**: P1 ships before P2 work begins.
6. **YAGNI**: Every line traceable to an FR-xxx or SC-xxx. No speculative abstractions.

## Recent Changes

- **001-blueprint-to-3d** (2026-03-31): 66 tasks done.
  Pipeline: image load → **optional crop (FR-024)** → scale → **OCR on raw image + adaptive text masking (FR-004)** → **NLM denoising/rayon (FR-025, FR-027)** → **adaptive-Canny/rayon (FR-025, FR-027)** → **hybrid ML+rule classifier/rayon (FR-005, FR-027)** with **5-min timeout fallback (FR-028)** → **collinear segment merge (FR-026)** → OCR parse → interior/exterior inference → clarification UI → **3D mesh/rayon (FR-027)** → OBJ+MTL / binary STL export.
  55 unit tests passing. SC-001–SC-008 targets implemented.
  - **T055–T061** (FR-027, SC-008): `rayon` parallelism across all CPU-bound stages (NLM, Canny gradient, classifier batch, mesh generation); `build.rs` links Apple Accelerate (`-framework Accelerate`) on aarch64-macos; vDSP `vDSP_svesq` accelerates NLM patch-distance inner loop; `criterion` benchmark in `benches/pipeline_bench.rs`
  - **T062–T064** (FR-028): 5-minute ML timeout — `pipeline_start: Instant` in `AnalysisState`; `classify_with_timeout()` checks elapsed per 20-element batch; elements ≥ 0.7 confidence kept, remainder re-classified with rules; non-blocking inline warning banner in `render_analyzing()`
  - **T065–T066** (FR-004 reorder): OCR masking now uses `preprocessor::mask_text_regions()` with adaptive padding (½ median char height) and border-median fill; correct pipeline order OCR→mask→denoise→Canny→trace enforced in OCR stage spawn

## OBJ Export: SketchUp Compatibility Notes

- **Coordinate system**: Y-up (standard OBJ); SketchUp importer handles Y→Z axis swap
- **Units**: meters (1 OBJ unit = 1 meter); select "Meters" in SketchUp's import dialog
- **Face winding**: CCW from outside for all triangulated faces
- **MTL file**: must be in same directory as OBJ; referenced by filename only

<!-- MANUAL ADDITIONS START -->
<!-- MANUAL ADDITIONS END -->
