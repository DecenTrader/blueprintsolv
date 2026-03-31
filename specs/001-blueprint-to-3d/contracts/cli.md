# Contract: CLI Interface

**Feature**: Blueprint Image to 3D Model
**Date**: 2026-03-30

---

## Command

```
blueprint2mod [OPTIONS] [IMAGE]
```

## Arguments

| Argument  | Type   | Required | Description |
|-----------|--------|----------|-------------|
| `IMAGE`   | path   | No       | Path to a JPG or PNG blueprint image. If omitted, a native file-picker dialog opens on launch. |

## Options

| Flag                    | Type   | Default | Description |
|-------------------------|--------|---------|-------------|
| `-o, --output <PATH>`   | path   | None    | Output file path for the exported 3D model. Extension determines format unless `--format` is also provided. If omitted, a save-file dialog opens when exporting. |
| `--format <FORMAT>`     | string | `obj`   | Export format. Accepted values: `obj`, `stl`. |
| `--session <PATH>`      | path   | None    | Load a previously saved `.b2m` session file and resume processing from where it was saved. Cannot be combined with `IMAGE`. |
| `-h, --help`            | flag   | —       | Print help and exit. |
| `-V, --version`         | flag   | —       | Print version and exit. |

## Validation Rules

- `IMAGE` and `--session` are mutually exclusive. If both are provided, the program
  exits with an error message and a non-zero exit code.
- `IMAGE` must be a readable file with a `.jpg`, `.jpeg`, or `.png` extension
  (case-insensitive). Any other extension results in an error before the GUI opens.
- `--format` values are case-insensitive (`OBJ`, `Obj`, `obj` are all valid).
- If `--output` is provided with `--format`, the `--format` flag takes precedence for
  determining the file format (the output extension is adjusted accordingly).

## Exit Codes

| Code | Meaning |
|------|---------|
| 0    | Success — export written or session saved cleanly |
| 1    | Invalid arguments or unsupported file format |
| 2    | Image file not found or not readable |
| 3    | Session file not found, not readable, or invalid format |
| 4    | ML model download failed and fallback detection also failed |

## Examples

```bash
# Open a blueprint with a file picker
blueprint2mod

# Load a specific blueprint image
blueprint2mod floor_plan.jpg

# Load and export directly to OBJ (GUI still opens for interactive steps)
blueprint2mod floor_plan.jpg --output ./output/my_house.obj

# Load a saved session and resume
blueprint2mod --session ./my_project.b2m

# Export as STL
blueprint2mod floor_plan.png --format stl --output ./output/my_house.stl
```

## First-Run Behavior

On first launch (no ML models cached), the application displays a progress bar while
downloading required ONNX models to `~/.blueprint2mod/models/`. If download fails,
a warning is shown and the app continues in rule-based-only mode (FR-019).

## Notes

- The GUI always opens for interactive steps (scale point selection, clarification
  prompts). There is no fully headless/batch mode in v1.
- All file paths support both absolute and relative paths; relative paths are resolved
  from the current working directory.
