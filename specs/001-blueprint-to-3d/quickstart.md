# Quickstart: blueprint2mod

**Date**: 2026-03-30

---

## Prerequisites

### Rust

Install the Rust toolchain (stable, 1.75+):

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
rustup update stable
```

### System Dependencies (for OCR)

`leptess` requires Tesseract and Leptonica to be installed at the system level.

**macOS:**
```bash
brew install tesseract leptonica
```

**Ubuntu / Debian:**
```bash
sudo apt-get install libtesseract-dev libleptonica-dev tesseract-ocr
```

**Windows:**
Install Tesseract via the [UB Mannheim installer](https://github.com/UB-Mannheim/tesseract/wiki).
Set `TESSERACT_PATH` environment variable to the install directory.

### Tesseract Language Data

The OCR engine requires the English trained data file:

```bash
# macOS (via brew, included automatically)
# Ubuntu:
sudo apt-get install tesseract-ocr-eng
# Or download manually to Tesseract's tessdata directory:
# https://github.com/tesseract-ocr/tessdata_fast
```

---

## Build

```bash
# Clone the repository
git clone <repo-url> blueprint2mod
cd blueprint2mod

# Build (development)
cargo build

# Build (release — recommended for performance)
cargo build --release
```

### ML Model Download (First Run)

On the first launch, blueprint2mod downloads pre-trained ONNX models to
`~/.blueprint2mod/models/`. This requires an internet connection and may take
1–3 minutes depending on connection speed. Subsequent runs are fully offline.

If download fails, the application continues in rule-based-only mode with a
visible warning (reduced detection accuracy expected).

---

## Run

```bash
# Launch with file picker (recommended for first use)
./target/release/blueprint2mod

# Load a specific blueprint image
./target/release/blueprint2mod path/to/floor_plan.jpg

# Resume a saved session
./target/release/blueprint2mod --session path/to/project.b2m

# Export directly to OBJ
./target/release/blueprint2mod path/to/floor_plan.jpg --output output/model.obj

# Export as STL
./target/release/blueprint2mod path/to/floor_plan.png --format stl --output output/model.stl
```

---

## Workflow

1. **Load image** — provide a JPG or PNG top-down blueprint (up to 4000×4000 px)
2. **Set scale** — click two points on the displayed blueprint; enter the real-world
   distance between them (e.g., "3.66 Meters")
3. **Analyze** — trigger automated line tracing, architectural element classification,
   and OCR. The system will show progress.
4. **Resolve clarifications** — for elements the system is uncertain about, select the
   correct type from the presented options or skip
5. **Confirm wall height** — accept the default (8 ft / 2.44 m) or enter a custom value
6. **Export** — choose OBJ or STL; save to a location of your choice
7. **Open in SketchUp** — File → Import → select your `.obj` or `.stl` file; when
   prompted for units, choose **Meters**

---

## Run Tests

```bash
# All tests
cargo test

# Integration tests only
cargo test --test integration

# Specific test
cargo test test_scaling
```

### Test Fixtures

Reference blueprints are in `test_fixtures/`. Add new fixtures when writing integration
tests for new detection scenarios.

```
test_fixtures/
├── simple_rectangle.jpg           # 2-room plan with 1 door
├── simple_rectangle.expected.json # ground-truth elements + dimensions
├── labeled_plan.jpg               # plan with room text labels
└── labeled_plan.expected.json
```

---

## Troubleshooting

### `leptess` build fails

Ensure Tesseract and Leptonica headers are installed:
```bash
pkg-config --libs tesseract lept   # should print library flags
```

If `pkg-config` is not found: `brew install pkg-config` (macOS) or
`sudo apt-get install pkg-config` (Linux).

### `ort` / ONNX Runtime link error

`ort` downloads ONNX Runtime automatically via the `download-binaries` feature.
If the build environment blocks network access:
1. Download the ONNX Runtime release manually from the GitHub releases page.
2. Set `ORT_DYLIB_PATH=/path/to/libonnxruntime.dylib` before building.

### ML models not downloading

Check internet connectivity. To manually place models:
```bash
mkdir -p ~/.blueprint2mod/models
# Copy ONNX model files to this directory
```

The application will skip the download step if models are already present.

### OBJ appears on its side in SketchUp

SketchUp uses Z-up; the OBJ file uses Y-up (standard). When importing, ensure the
"Swap YZ axes" option is enabled in the importer dialog, or use SketchUp 2021+ which
handles this automatically.
