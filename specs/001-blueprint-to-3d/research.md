# Rust Ecosystem Research for blueprint2mod

**Date:** 2026-03-30  
**Purpose:** Evaluate current Rust crates for each major subsystem of the blueprint2mod desktop application.

---

## A. GUI Framework

### Requirements
- Display raster images (JPG/PNG) at arbitrary zoom
- Detect mouse click positions in image-pixel coordinates
- Overlay vector annotations (points, lines, polygons) on top of images
- Reasonably native-looking desktop app on macOS/Windows/Linux

### Candidates

#### egui + eframe
- **Crates:** `egui`, `eframe`, `egui_extras`
- **Current version:** 0.31.0 (released 2025-02-04), with 0.34.x latest as of early 2026
- **Stability:** Beta/production-quality; API changes occur between minor versions but are well-documented
- **Strengths:**
  - Immediate-mode paradigm makes per-frame interactive overlays trivial to implement
  - `Image::new(...).sense(Sense::click())` returns a `Response` with cursor position; use `RectTransform` to map screen coords back to image-pixel coords
  - `egui::Painter` (via `ui.allocate_painter`) lets you draw circles, lines, and polygons on top of the image in the same frame
  - egui 0.31 added a `Scene` container — a pannable, zoomable canvas that can host widgets — ideal for zooming into blueprint images
  - `egui_extras::install_image_loaders()` handles JPG/PNG loading with the `image` crate backend
  - Works on desktop and WASM with the same code; tiny binary sizes
  - Active development, large community, many third-party widget crates
- **Weaknesses:**
  - Immediate-mode redraws every frame; not perfectly suited to GPU-heavy rendering pipelines (but fine for this use case)
  - Default font lacks full Unicode coverage (non-Latin scripts need manual system font setup — not relevant here)
  - Occasional breaking API changes between minor versions
- **Verdict for this app:** **Recommended.** The combination of `Sense::click()` on images, `Painter` overlay, and the new `Scene` zoomable container makes this an excellent fit.

#### iced
- **Crate:** `iced`
- **Current version:** ~0.13.x (late 2024/early 2025)
- **Stability:** Beta; API has broken several times across minor versions; currently stabilizing
- **Strengths:**
  - Elm-inspired architecture (Model-Update-View) — cleaner separation of state
  - `Canvas` widget supports drawing 2D graphics and handling mouse events
  - `Image` widget can display raster images; `canvas::Image` allows raster images inside a Canvas
  - Good cross-platform support
- **Weaknesses:**
  - Retained-mode architecture means managing image click coordinates requires more boilerplate compared to egui
  - No built-in pannable/zoomable image canvas (must be implemented manually in `Canvas`)
  - API has been historically unstable; fewer third-party examples for image annotation use cases
  - Screen reader accessibility is poor (known issue)
- **Verdict for this app:** Viable but more work. egui's `Scene` + `Painter` + `Sense::click()` combination is more directly suited to interactive image annotation.

#### Slint / Tauri / GTK-rs
- **Slint:** Good for embedded/constrained UIs; no first-class image-pixel interaction
- **Tauri:** WebView-based (HTML/JS frontend); trivially handles image annotation but adds JS complexity
- **GTK-rs:** Native GTK widgets; mature but verbose, poor macOS support
- **Verdict:** Not recommended for this use case.

### Recommendation
**Use `egui` + `eframe` + `egui_extras`.**  
Pattern for image interaction:
```rust
let response = ui.add(egui::Image::new(&texture).sense(egui::Sense::click()));
if response.clicked() {
    if let Some(pos) = response.interact_pointer_pos() {
        let image_pos = RectTransform::from_to(response.rect, image_rect).transform_pos(pos);
        // image_pos is now in image-pixel coordinates
    }
}
// Overlay annotations
let painter = ui.painter_at(response.rect);
painter.circle_filled(screen_pos, 5.0, egui::Color32::RED);
```

---

## B. Image I/O and Processing

### Requirements
- Load JPG and PNG files from disk
- Canny edge detection
- Hough line transform
- Morphological operations (dilate, erode)

### Candidates

#### `image` crate
- **Crate:** `image`
- **Current version:** 0.25.x (actively maintained by image-rs org)
- **Stability:** Stable/production
- **Strengths:**
  - De-facto standard for image I/O in Rust
  - Supports PNG, JPEG, BMP, GIF, TIFF, WebP, and more
  - `DynamicImage` and `ImageBuffer` types integrate with nearly every other crate in the ecosystem
  - Lossless and lossy JPEG decoding/encoding
- **Weaknesses:**
  - Image processing algorithms (edge detection, etc.) are NOT included — that is `imageproc`'s domain
  - API changes occasionally between minor versions (e.g., v0.24 → v0.25 broke some loading paths with `egui`)
- **Known issue:** There was a compatibility issue between `image` >0.24 and older `egui` versions (egui issue #4464). Ensure `image` and `egui_extras` versions are compatible (use same major `image` version as egui_extras depends on).

#### `imageproc` crate
- **Crate:** `imageproc`
- **Current version:** 0.25.0 (latest as of early 2026)
- **Stability:** Beta — API is not yet 1.0; breaking changes possible but infrequent
- **Strengths:**
  - Built on top of `image`; accepts `ImageBuffer` / `DynamicImage` directly
  - **Canny edge detection:** `imageproc::edges::canny(img, low_threshold, high_threshold)` — returns binary `GrayImage`
  - **Hough line transform:** `imageproc::hough` module — `detect_lines()` and `draw_polar_lines()`
  - **Morphological operations:** `imageproc::morphology::dilate()` and `erode()` with configurable kernels
  - Includes contrast enhancement, geometric transforms, contour finding, template matching, and more
  - Multi-threaded variants for several operations
  - Well-documented with examples
- **Weaknesses:**
  - Pre-1.0; some advanced operations (e.g., multi-scale Hough) require manual implementation
  - Slower than OpenCV for large images (no SIMD-optimized kernels for all operations)
  - Hough transform implementation is basic compared to OpenCV's — returns lines in polar form, no probabilistic Hough
- **Caveats for blueprints:**
  - Blueprint images often have low contrast or faint lines; consider preprocessing with `imageproc::contrast::equalize_histogram()` before Canny
  - For thin architectural lines, Canny thresholds need tuning; the `subpixel-edge` crate (builds on imageproc) offers subpixel-precision edges if needed

### Recommendation
**Use `image` (0.25.x) + `imageproc` (0.25.0).**  
These two crates are the established standard and cover all required operations. No alternatives in pure Rust come close in breadth.

---

## C. OCR

### Requirements
- Extract text labels and numeric values (dimensions, room names) from raster blueprint images
- Acceptable accuracy on printed/digital blueprint fonts

### State of Pure-Rust OCR
There are **no production-quality pure-Rust OCR engines** as of early 2026. All viable options are bindings to Tesseract (C++ library) or shell-out wrappers.

### Candidates

#### `leptess`
- **Crate:** `leptess`
- **Current version:** 0.14.x (recently updated; packaged in Ubuntu 2025)
- **Stability:** Stable bindings; Tesseract 4/5 is the underlying stable engine
- **Strengths:**
  - Safe Rust bindings to both Leptonica (image preprocessing) and Tesseract (OCR)
  - Provides `LepTess` high-level struct for productivity and raw `TessApi` for fine control
  - Highest download count among Rust OCR crates (~110K all-time, ~8K recent)
  - Supports word-level bounding boxes, confidence scores, page segmentation modes
  - Can pass `image::DynamicImage` pixels directly via raw buffer
- **Weaknesses:**
  - Requires system Tesseract + Leptonica libraries installed (`libtesseract-dev`, `libleptonica-dev`)
  - Cross-compilation and static linking are non-trivial
  - Build system complexity: uses `bindgen` at compile time
- **System requirement:** Tesseract 4.x or 5.x with trained language data (`tessdata`)

#### `tesseract` crate
- **Crate:** `tesseract`
- **Current version:** 0.1.20
- **Stability:** Stable but lower usage (~3K downloads/month)
- **Strengths:** Higher-level API than leptess for simple use cases
- **Weaknesses:** Less flexible; fewer low-level controls; smaller community

#### `rusty-tesseract`
- **Crate:** `rusty-tesseract`
- **Stability:** Lower usage (~9K all-time); invokes Tesseract CLI as subprocess
- **Weaknesses:** Subprocess invocation is fragile and slow; not recommended for production

#### No pure-Rust alternatives
There is no mature pure-Rust OCR engine. Projects like `tesseract-rs` are still thin wrappers. Deep learning-based OCR (EasyOCR, docTR) are Python-only. Running a lightweight OCR ONNX model via `ort` is theoretically possible but requires finding/training a suitable model and is significant extra work.

### Recommendation
**Use `leptess` (0.14.x)** as the primary OCR crate.  
For blueprints, preprocess with `imageproc` (binarization, deskew, morphological cleanup) before passing to Tesseract to improve accuracy. Use Tesseract's `PSM_SINGLE_BLOCK` or `PSM_SINGLE_LINE` page segmentation mode for isolated text regions.

---

## D. ML Inference (ONNX)

### Requirements
- Load a pre-trained ONNX model for architectural element classification
- Run inference on image patches or full images
- Ideally: no Python dependency at runtime, reasonable latency on CPU (GPU acceleration a bonus)

### Candidates

#### `ort` (ONNX Runtime Rust bindings)
- **Crate:** `ort`
- **Current version:** 2.0.0-rc.12 (wraps ONNX Runtime 1.24)
- **Stability:** Release candidate — API is described as "production-ready, just not API stable yet"
- **Strengths:**
  - Full ONNX Runtime feature set: runs ResNet, YOLO, BERT, vision transformers, and virtually any ONNX model
  - Hardware acceleration: CUDA, TensorRT, CoreML (Apple), DirectML (Windows), OpenVINO, QNN, CANN
  - Demonstrated real-world use: YOLO v8 real-time webcam inference examples exist
  - `ModelCompiler` for ahead-of-time graph optimization (reduced startup latency)
  - Supports both inference and training
  - Most compatible with the broadest range of ONNX opsets and model architectures
  - Strong documentation at `ort.pyke.io`; actively maintained by `pykeio`
- **Weaknesses:**
  - Links against ONNX Runtime shared library (C++); must ship the `.dylib`/`.dll`/`.so` or use static linking
  - Not pure Rust — introduces a significant C++ dependency
  - 2.0.0 API is not yet stable (rc.12 as of March 2026); expect minor API changes before 2.0.0 release
- **Best for:** Any model that needs GPU acceleration or broad ONNX opset coverage

#### `tract` (pure Rust ONNX)
- **Crate:** `tract-onnx`, `tract`
- **Maintained by:** Sonos
- **Current version:** 0.22.1
- **Stability:** Stable for production CPU inference; passes ~85% of ONNX backend tests
- **Strengths:**
  - Zero C/C++ dependencies — pure Rust, compiles to WebAssembly
  - No shared library to ship
  - Good model compatibility guarantees (semver patch-level backward compat)
  - Suitable for simple ConvNet classifiers and common vision model architectures
- **Weaknesses:**
  - CPU-only; no GPU/hardware acceleration
  - Passes 85% (not 100%) of ONNX backend tests — some advanced ops may fail
  - Smaller community than ONNX Runtime
  - Version 0.x — API can change
- **Best for:** Deployment where binary simplicity matters more than speed (no native deps)

#### `candle` (Hugging Face)
- **Crate:** `candle-core`, `candle-nn`, `candle-transformers`
- **Maintained by:** Hugging Face
- **Current version:** 0.x (actively developed, 2025/2026 updates include Qwen3, recent VLMs)
- **Stability:** Beta; active HuggingFace backing ensures long-term support
- **Strengths:**
  - Supports ResNet, ViT, EfficientNet, VGG, DINOv2, MobileNetV4, and many others natively
  - CUDA and Metal (Apple Silicon) acceleration
  - Loads safetensors, npz, PyTorch checkpoints directly (not only ONNX)
  - Minimalist design — no Python runtime dependency
- **Weaknesses:**
  - ONNX support (`candle-onnx`) is limited — primarily for interop, not a first-class ONNX runtime
  - API is not yet 1.0; active development means breaking changes
  - More complex to use than ort for a pre-trained ONNX model (best when model is in safetensors format)
- **Best for:** When models are in safetensors/PyTorch format, or when building/fine-tuning in Rust

#### `burn`
- **Crate:** `burn`, `burn-import`
- **Maintained by:** Tracel-AI
- **Current version:** ~0.15.x (active releases throughout 2024-2025)
- **Stability:** Beta; opset 16+ required for ONNX import; generates Rust source code from ONNX
- **Strengths:**
  - Converts ONNX → native Burn Rust code at build time; zero runtime dependency on ONNX format
  - Backend-agnostic: runs on CPU (NdArray), GPU (WGPU), CUDA, embedded
  - WebAssembly support; demo includes image classification in browser
  - Generates readable, modifiable Rust code (not a black-box binary)
- **Weaknesses:**
  - Requires ONNX opset 16+; older models need upconversion
  - The build-time code generation step adds friction in development workflow
  - ONNX operator coverage is incomplete for some architectures
  - Most complex setup among the options

### Recommendation
**Use `ort` (2.0.0-rc.12)** for maximum model compatibility and performance.  
Given that the project uses pre-trained ONNX models for architectural classification, `ort` offers the widest ONNX opset coverage and access to hardware acceleration (CUDA on Linux/Windows, CoreML on macOS). Pin to `ort = "2.0.0-rc.12"` and plan to migrate to `2.0.0` stable when released.

**Fallback:** If binary simplicity (no C++ deps) is a hard requirement, use `tract-onnx`. For typical ConvNet classifiers, tract's 85% ONNX compatibility is likely sufficient.

---

## E. 3D Export

### Requirements
- Write OBJ files with accompanying MTL material file
- Write binary STL files
- Compatible with SketchUp 2020+ import

### OBJ Export

#### `obj-exporter`
- **Crate:** `obj-exporter`
- **Last published:** ~2017 (8 years ago) — **unmaintained**
- **Verdict:** Do not use; stale

#### `wavefront_obj` + manual writing
- **Crate:** `wavefront_obj`
- **Status:** Parser/serializer for OBJ+MTL; moderately maintained
- **Caveat:** Primarily a parser; export requires extra care

#### Custom OBJ writer (recommended)
The OBJ and MTL formats are simple ASCII text formats. For blueprint2mod's use case (extruded 2D floor plans → basic 3D geometry), **writing a custom OBJ exporter is the most practical and robust approach**:

```
# OBJ: vertices, texture coords, normals, faces
v 0.0 0.0 0.0
...
f 1/1/1 2/2/1 3/3/1
mtllib model.mtl

# MTL: material definition
newmtl wall_material
Kd 0.8 0.8 0.8
```

This requires fewer than 200 lines of Rust code and gives full control over the coordinate system and winding order for SketchUp compatibility (see Section F).

### STL Export

#### `stl_io`
- **Crate:** `stl_io`
- **Current version:** 0.10.0 (released March 2026 — actively maintained, 26 versions published)
- **Stability:** Stable; production-quality
- **Strengths:**
  - Reads binary and ASCII STL; writes binary STL (more compact)
  - Simple API: build a `Vec<Triangle>` and call `write_stl()`
  - Actively maintained with recent release
- **Weaknesses:**
  - ASCII STL write not supported (binary only) — not an issue for modern tools
  - Minimal ecosystem; just a utility crate

### Recommendation
- **STL:** Use `stl_io` (0.10.0).  
- **OBJ+MTL:** Write a custom exporter directly. Use `std::io::Write` / `std::fmt::Write`. This gives exact control over SketchUp-compatibility requirements.

---

## F. Session File Serialization

### Requirements
- Save/load application state: image paths, selected points, detected lines, OCR results, ML classifications, material assignments
- Human-readability is a nice-to-have but not required
- Reasonable file size; fast load/save

### Options

#### `serde_json`
- **Crate:** `serde_json`
- **Stability:** Stable / 1.0
- **Strengths:**
  - Human-readable, hand-editable session files
  - Trivial debugging: open in any text editor
  - Forward-compatible: adding new optional fields with `#[serde(default)]` just works
  - Universal — any external tool can read/write sessions
  - No format versioning ceremony needed for simple schema evolution
- **Weaknesses:**
  - Larger file size (~3-5x vs binary for numeric data)
  - Slower for very large serialization payloads (image buffers, etc.)
  - Should NOT be used to store raw pixel data — store paths/references instead

#### `bincode` (v2.x)
- **Crate:** `bincode`
- **Current version:** 2.0 (stable; breaking change from 1.x — introduces native `Encode`/`Decode` traits; `serde` is now optional)
- **Stability:** Stable
- **Strengths:**
  - Fastest encode/decode among Rust serialization crates
  - Most compact binary format for numeric data
  - Ideal for large in-memory structures
- **Weaknesses:**
  - Not self-describing — adding/removing fields breaks compatibility (no built-in schema evolution)
  - Not human-readable; difficult to debug
  - Bincode 2.x is not wire-compatible with 1.x — cannot migrate old session files without a converter
  - `serde` attribute caveats: some `#[serde(...)]` field attributes cause silent data loss in bincode

#### `rmp-serde` (MessagePack)
- **Crate:** `rmp-serde`
- **Stability:** Stable; well-maintained
- **Strengths:**
  - More compact than JSON, cross-language compatible (Python, JS can read/write sessions)
  - Self-describing enough for basic schema evolution
  - Works via standard `#[derive(Serialize, Deserialize)]`
- **Weaknesses:**
  - Slower than bincode for large data
  - Less common in Rust ecosystem; more debugging friction than JSON

### Tradeoffs Summary

| Format | Size | Speed | Human-readable | Schema evolution | Debuggability |
|--------|------|-------|----------------|-----------------|---------------|
| JSON | Large | Medium | Yes | Easy | Excellent |
| bincode | Smallest | Fastest | No | Manual versioning | Poor |
| MessagePack | Small | Fast | No | Moderate | Poor |

### Recommendation
**Use `serde_json`** for session files.  

Rationale for this app:
- Session files contain primarily metadata (point lists, line arrays, text strings, file paths, classification labels) — not bulk image pixel data. At this scale, JSON file sizes are not a concern.
- Human-readable sessions help with debugging during development.
- Schema evolution (adding new detection results, new material properties) is handled transparently with `#[serde(default)]`.
- If performance profiling later shows JSON serialization as a bottleneck (unlikely), migrate to `rmp-serde`.

---

## G. SketchUp 2020+ OBJ Import Compatibility

### Key Requirements and Known Gotchas

#### 1. Native OBJ Import
SketchUp Pro does not natively import OBJ files — an **extension is required** (e.g., "OBJ Importer" by SimLab, or the built-in OBJ importer in SketchUp 2021+). SketchUp 2021+ includes a native OBJ importer.

#### 2. Coordinate System: Y-up vs. Z-up
- **OBJ format default:** Right-handed coordinate system, **Y-up** (standard per the Wavefront spec)
- **SketchUp internal system:** **Z-up** (Z is the vertical axis)
- **Result:** OBJ files imported into SketchUp will appear lying on their side unless the axes are swapped
- **Fix:** Either export with Y as up (standard OBJ) and let SketchUp's importer handle the swap (most importers offer a "swap YZ" checkbox), or manually write the geometry with Z as the vertical axis and document this in the export options

#### 3. Face Winding Order
- **OBJ spec:** Vertices listed in **counter-clockwise** order define the front face (outward normal)
- **SketchUp:** Respects this convention; faces with reversed winding will appear as "back faces" (shown in blue-grey in SketchUp's default style)
- **Fix:** Ensure all exported faces have counter-clockwise vertex order when viewed from outside the solid. For extruded floor plans, this means: top face CCW when viewed from above, bottom face CW when viewed from above (or CCW when viewed from below), and wall faces CCW when viewed from outside.

#### 4. MTL File Requirements
- The MTL file must be in the **same directory** as the OBJ file
- The `mtllib` directive in the OBJ file must match the exact filename (case-sensitive on macOS/Linux)
- SketchUp applies materials per face group (`usemtl` directives); use `g group_name` to separate geometry groups
- Texture image paths in MTL (`map_Kd`) must be relative paths, pointing to files in the same directory

#### 5. Polygon vs. Triangle Faces
- SketchUp can import n-gon faces (not just triangles), but complex n-gons (non-planar) may not triangulate correctly
- **Recommendation:** Triangulate all faces before export, or ensure faces are guaranteed planar

#### 6. Large File Performance
- SketchUp can be slow importing OBJ files with thousands of faces
- Group related geometry and use materials efficiently to minimize face count

#### 7. Units
- OBJ files have no explicit unit specification
- SketchUp's OBJ importer asks for the unit on import (inches, meters, etc.)
- Document the unit used in the export (recommend meters; scale factor 1.0 = 1 meter)

---

## Cargo.toml Dependency Summary

```toml
[dependencies]
# GUI
egui = "0.34"
eframe = { version = "0.34", features = ["default"] }
egui_extras = { version = "0.34", features = ["image"] }

# Image I/O and processing
image = "0.25"
imageproc = "0.25"

# OCR (requires system libtesseract + leptonica)
leptess = "0.14"

# ML inference (ONNX)
ort = "2.0.0-rc.12"

# 3D export (STL)
stl_io = "0.10"

# Session serialization
serde = { version = "1", features = ["derive"] }
serde_json = "1"
```

**Note on `ort` build:** Must either use the `download-binaries` feature (auto-downloads ONNX Runtime shared lib) or set `ORT_DYLIB_PATH` at build time. See ort documentation at https://ort.pyke.io/.

**Note on `leptess` build:** Requires `pkg-config`, `libtesseract-dev`, and `libleptonica-dev` installed on the build machine. On macOS: `brew install tesseract leptonica`.

---

## Sources

- [A 2025 Survey of Rust GUI Libraries — boringcactus](https://www.boringcactus.com/2025/04/13/2025-survey-of-rust-gui-libraries.html)
- [Rust GUI Libraries Compared: egui vs iced vs druid — AN4T](https://an4t.com/rust-gui-libraries-compared/)
- [egui Image widget docs — docs.rs](https://docs.rs/egui/latest/egui/widgets/struct.Image.html)
- [egui 0.31.0 release notes — GitHub](https://github.com/emilk/egui/releases/tag/0.31.0)
- [Draw on top of an image — egui Discussions #2967](https://github.com/emilk/egui/discussions/2967)
- [imageproc::edges::canny — docs.rs](https://docs.rs/imageproc/latest/imageproc/edges/fn.canny.html)
- [imageproc::hough — docs.rs](https://docs.rs/imageproc/0.15.0/imageproc/hough/index.html)
- [imageproc GitHub — image-rs/imageproc](https://github.com/image-rs/imageproc)
- [leptess GitHub — houqp/leptess](https://github.com/houqp/leptess)
- [rusty-tesseract GitHub — thomasgruebl/rusty-tesseract](https://github.com/thomasgruebl/rusty-tesseract)
- [ort crate introduction — ort.pyke.io](https://ort.pyke.io/)
- [ort GitHub — pykeio/ort](https://github.com/pykeio/ort)
- [Rust, ORT, ONNX: Real-Time YOLO — Medium](https://medium.com/@alfred.weirich/rust-ort-onnx-real-time-yolo-on-a-live-webcam-part-1-b6edfb50bf9b)
- [tract GitHub — sonos/tract](https://github.com/sonos/tract)
- [candle GitHub — huggingface/candle](https://github.com/huggingface/candle)
- [burn GitHub — tracel-ai/burn](https://github.com/tracel-ai/burn)
- [burn-onnx: ONNX Model import — burn.dev](https://burn.dev/books/burn/import/onnx-model.html)
- [Building Sentence Transformers: Burn, ONNX Runtime, Candle — DEV Community](https://dev.to/mayu2008/building-sentence-transformers-in-rust-a-practical-guide-with-burn-onnx-runtime-and-candle-281k)
- [stl_io crates.io](https://crates.io/crates/stl_io)
- [obj-exporter crates.io](https://crates.io/crates/obj-exporter)
- [3D import/export crates — Rust Forum](https://users.rust-lang.org/t/3d-import-export-manipulation-crates/15784)
- [Rust serialization: what's ready for production — LogRocket](https://blog.logrocket.com/rust-serialization-whats-ready-for-production-today/)
- [bincode serde tradeoffs — Rust Forum](https://users.rust-lang.org/t/what-purpose-does-the-crate-bincode-serve-in-binary-serialization-that-serde-does-not/73981)
- [Exporting OBJ Files — SketchUp Help](https://help.sketchup.com/en/sketchup/exporting-obj-files)
- [OBJ import SketchUp: coordinate system — SketchUp Community](https://forums.sketchup.com/t/import-export-in-one-coordinate-system/12597)
- [How to Import OBJ into SketchUp — Meshy Blog](https://www.meshy.ai/blog/how-to-import-obj-into-sketchup)
- [Rust for ML: Writing High-Performance Inference Engines in 2025 — Markaicode](https://markaicode.com/rust-ml-inference-engines-2025/)
