# Implementation Plan: Blueprint Image to 3D Model

**Branch**: `001-blueprint-to-3d` | **Date**: 2026-03-30 | **Spec**: [spec.md](spec.md)
**Input**: Feature specification from `/specs/001-blueprint-to-3d/spec.md`

## Summary

A Rust desktop application that converts top-down architectural blueprint images (JPG/PNG)
into 3D models (OBJ/STL) for SketchUp. The pipeline combines egui-based interactive image
viewing, Canny + Hough-based line tracing, hybrid ONNX ML + rule-based architectural element
classification, Tesseract OCR for room labels and dimension extraction, and a custom
OBJ/MTL exporter with SketchUp-compatible Y-up coordinates.

## Technical Context

**Language/Version**: Rust (stable, 1.75+)
**Primary Dependencies**:
- GUI: `egui` 0.34 + `eframe` 0.34 + `egui_extras` 0.34 (image display with click-to-select)
- Image I/O: `image` 0.25.x
- Image processing: `imageproc` 0.25.x (Canny, Hough, morphological ops)
- OCR: `leptess` 0.14.x (Tesseract 4/5 + Leptonica bindings)
- ML inference: `tract-onnx` 0.22.x preferred (pure Rust, zero runtime overhead, saves ~100 MB vs `ort`); `ort` 2.0.0-rc.12 as fallback if a chosen model requires unsupported ONNX ops. SC-007 (≤1 GB total) constrains model selection to lightweight architectures (MobileNet-class, EfficientNet-B0, YOLO-nano; individually ≤100 MB).
- STL export: `stl_io` 0.10.x
- OBJ+MTL export: custom writer (no suitable maintained crate exists)
- Serialization: `serde` 1.x + `serde_json` 1.x
- Model download: `reqwest` (async HTTP, for first-run model fetch)

**Storage**: Local filesystem only — session files (`.b2m` JSON), ML model cache
(`~/.blueprint2mod/models/`), correction history (`~/.blueprint2mod/corrections.json`)

**Testing**: `cargo test` — integration tests in `tests/integration/`, unit tests inline per
module (`#[cfg(test)]`), reference blueprint fixtures in `test_fixtures/`

**Target Platform**: Desktop — macOS (10.15+), Linux (Ubuntu 22.04+), Windows 10+

**Project Type**: desktop-app (single binary, local processing only)

**Performance Goals**: Full workflow in under 10 minutes for a 4000×4000 px blueprint
on a modern consumer CPU (SC-001)

**Constraints**:
- Offline after first-run model download (FR-018, FR-019)
- OBJ files must import into SketchUp 2020+ without errors (SC-003, FR-014)
- OBJ exported in Y-up coordinates (standard Wavefront spec); SketchUp importer handles
  axis swap. Faces triangulated, CCW winding, MTL in same directory as OBJ.
- OCR requires system Tesseract + Leptonica libraries (build-time dependency)

**Scale/Scope**: Single user, local processing, single floor plan per session

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

- [x] **Principle I — Test-First**: All user story tasks include test tasks written
  before implementation tasks; Red phase confirmed for each.
- [x] **Principle II — Output Validation**: All success criteria (SC-001 through SC-006)
  encoded as runnable test assertions with reference blueprints in `test_fixtures/`.
- [x] **Principle III — Hybrid Detection**: Rule-based fallback mode listed as an
  explicit test scenario in `tests/integration/test_detection_fallback.rs`.
  Adaptive threshold tests included.
- [x] **Principle IV — Accuracy Gates**: ≥90% wall detection, ≥90% interior/exterior
  inference, ±5% dimension tolerance, and 100% SketchUp import success are each
  represented as pass/fail assertions in integration tests.
- [x] **Principle V — Incremental Delivery**: User stories sequenced P1 → P4;
  P1 (Import + Scale) ships before P2 (Detection) work begins.
- [x] **Principle VI — YAGNI**: All planned code is traceable to FR-xxx or SC-xxx;
  no plugin system, extension API, or speculative abstractions in this plan.

## Project Structure

### Documentation (this feature)

```text
specs/001-blueprint-to-3d/
├── plan.md              # This file
├── research.md          # Phase 0 output
├── data-model.md        # Phase 1 output
├── quickstart.md        # Phase 1 output
├── contracts/
│   ├── cli.md           # CLI argument contract
│   ├── session-format.md  # Session file JSON schema
│   └── export-format.md   # OBJ/STL format notes and SketchUp requirements
└── tasks.md             # Phase 2 output (/speckit.tasks command)
```

### Source Code (repository root)

```text
blueprint2mod/
├── Cargo.toml
├── src/
│   ├── main.rs                    # Entry point — parse CLI args, launch eframe
│   ├── app/
│   │   ├── mod.rs
│   │   ├── state.rs               # AppState enum (Welcome, Scaling, Detecting, …)
│   │   └── ui.rs                  # egui rendering per state, event dispatch
│   ├── blueprint/                 # Core domain types (FR-001, FR-002, FR-003)
│   │   ├── mod.rs
│   │   ├── image.rs               # BlueprintImage load + validate
│   │   ├── scale.rs               # ScaleReference, pixel-to-unit ratio
│   │   ├── element.rs             # ArchitecturalElement, ElementType
│   │   └── floor_plan.rs          # FloorPlan, Room, interior/exterior
│   ├── detection/                 # Line tracing + classification (FR-004–FR-007)
│   │   ├── mod.rs
│   │   ├── line_tracer.rs         # Canny + Hough + contour tracing
│   │   ├── classifier.rs          # Hybrid dispatcher: ML then rule-based
│   │   ├── ml/
│   │   │   ├── mod.rs
│   │   │   ├── model_manager.rs   # Download, cache, load ONNX models (FR-018, FR-019)
│   │   │   └── inference.rs       # ort session, confidence scores
│   │   └── rules/
│   │       ├── mod.rs
│   │       └── patterns.rs        # Rule-based heuristics per element type
│   ├── ocr/                       # Text extraction (FR-020–FR-023)
│   │   ├── mod.rs
│   │   ├── extractor.rs           # leptess OCR wrapper + preprocessing
│   │   └── parser.rs              # Room label matching, dimension parsing
│   ├── model3d/                   # 3D generation (FR-009, FR-010)
│   │   ├── mod.rs
│   │   └── generator.rs           # FloorPlan → Model3D (extrusion, element shapes)
│   ├── export/                    # File export (FR-011–FR-014)
│   │   ├── mod.rs
│   │   ├── obj.rs                 # Custom OBJ + MTL writer (Y-up, CCW, triangulated)
│   │   └── stl.rs                 # stl_io wrapper
│   ├── session/                   # Save/load (FR-016, FR-017)
│   │   ├── mod.rs
│   │   └── serialization.rs       # serde_json Session encode/decode
│   └── correction/                # Adaptive threshold (FR-007, CorrectionHistory entity)
│       ├── mod.rs
│       └── history.rs             # Global CorrectionHistory persist/load
├── tests/
│   ├── integration/
│   │   ├── test_scaling.rs        # SC-004: dimension accuracy within ±5%
│   │   ├── test_detection.rs      # SC-002: ≥90% wall detection on reference blueprints
│   │   ├── test_detection_fallback.rs  # Principle III: rule-based-only mode
│   │   ├── test_interior_exterior.rs   # SC-005: ≥90% region inference
│   │   ├── test_ocr.rs            # OCR accuracy on reference text
│   │   ├── test_export_obj.rs     # SC-003: valid OBJ/MTL; SketchUp import smoke test
│   │   ├── test_export_stl.rs     # SC-003: valid STL
│   │   └── test_session.rs        # Session save/load round-trip
│   └── (unit tests inline in src/ modules via #[cfg(test)])
└── test_fixtures/
    ├── simple_rectangle.jpg       # Minimal floor plan: 2 rooms, 1 door
    ├── simple_rectangle.expected.json   # Ground-truth elements + dimensions
    ├── labeled_plan.jpg           # Blueprint with room text labels
    └── labeled_plan.expected.json
```

**Structure Decision**: Single Rust binary crate. No workspace needed for v1 — all
subsystems are internal modules. This satisfies Principle VI (YAGNI) — no premature
library/workspace split.

## Complexity Tracking

| Violation | Why Needed | Simpler Alternative Rejected Because |
|-----------|------------|--------------------------------------|
| Native C++ dependency: `leptess` (Tesseract + Leptonica) | OCR is a hard requirement (FR-020); no production-quality pure-Rust OCR engine exists as of 2026 | Pure-Rust alternatives do not exist at production quality |
| ML inference backend choice | SC-007 (≤1 GB) favors `tract-onnx` (pure Rust, no runtime binary, saves ~100 MB) over `ort`; `tract-onnx` passes ~85% of ONNX tests which is sufficient for MobileNet/EfficientNet-class models. `ort` retained as fallback only if a required ONNX op is unsupported by tract. |
| Custom OBJ writer (not a crate) | Only OBJ crate (`obj-exporter`) is 8 years unmaintained; SketchUp compatibility requires precise control of winding order, face triangulation, and MTL format | All available OBJ crates are unmaintained or unsuitable |
