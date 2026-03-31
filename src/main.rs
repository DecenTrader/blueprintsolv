use std::path::PathBuf;

use anyhow::Result;
use eframe::egui;

use blueprint2mod::app::{BlueprintApp, ExportFormat};
#[allow(unused_imports)]
use blueprint2mod::blueprint::image::BlueprintImage;
use blueprint2mod::session::serialization::Session;

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().collect();
    let mut image_path: Option<PathBuf> = None;
    let mut session_path: Option<PathBuf> = None;
    let mut output_path: Option<PathBuf> = None;
    let mut export_format = ExportFormat::Obj;

    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "-h" | "--help" => {
                print_help();
                return Ok(());
            }
            "-V" | "--version" => {
                println!("blueprint2mod {}", env!("CARGO_PKG_VERSION"));
                return Ok(());
            }
            "--session" => {
                i += 1;
                if i >= args.len() {
                    eprintln!("Error: --session requires a path argument");
                    std::process::exit(1);
                }
                session_path = Some(PathBuf::from(&args[i]));
            }
            "-o" | "--output" => {
                i += 1;
                if i >= args.len() {
                    eprintln!("Error: --output requires a path argument");
                    std::process::exit(1);
                }
                output_path = Some(PathBuf::from(&args[i]));
            }
            "--format" => {
                i += 1;
                if i >= args.len() {
                    eprintln!("Error: --format requires a value (obj or stl)");
                    std::process::exit(1);
                }
                export_format = match args[i].to_ascii_lowercase().as_str() {
                    "obj" => ExportFormat::Obj,
                    "stl" => ExportFormat::Stl,
                    other => {
                        eprintln!("Error: unsupported format '{}'. Use 'obj' or 'stl'.", other);
                        std::process::exit(1);
                    }
                };
            }
            arg if !arg.starts_with('-') => {
                if image_path.is_some() {
                    eprintln!("Error: only one IMAGE argument is allowed");
                    std::process::exit(1);
                }
                image_path = Some(PathBuf::from(arg));
            }
            other => {
                eprintln!("Error: unknown argument '{}'", other);
                std::process::exit(1);
            }
        }
        i += 1;
    }

    // IMAGE and --session are mutually exclusive (contracts/cli.md)
    if image_path.is_some() && session_path.is_some() {
        eprintln!("Error: IMAGE and --session are mutually exclusive");
        std::process::exit(1);
    }

    let mut app = BlueprintApp::new_empty();
    app.output_path = output_path;
    app.export_format = export_format;

    // Load image or session from CLI args
    if let Some(session_file) = session_path {
        match Session::load(&session_file) {
            Ok(s) => {
                app.session = Some(s);
                app.state = blueprint2mod::app::state::AppState::Scaled;
            }
            Err(e) => {
                eprintln!("Failed to load session: {}", e);
                std::process::exit(3);
            }
        }
    } else if let Some(ref img_path) = image_path {
        match BlueprintImage::load(img_path) {
            Ok(img) => {
                app.session = Some(Session::new(img));
                app.state = blueprint2mod::app::state::AppState::ImageLoaded;
            }
            Err(e) => {
                eprintln!("Failed to load image: {}", e);
                std::process::exit(2);
            }
        }
    }

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("blueprint2mod")
            .with_min_inner_size([800.0, 600.0]),
        ..Default::default()
    };
    eframe::run_native("blueprint2mod", options, Box::new(|_cc| Ok(Box::new(app))))
        .map_err(|e| anyhow::anyhow!("GUI error: {}", e))
}

fn print_help() {
    println!(
        "blueprint2mod {}
Convert architectural blueprint images to 3D models (OBJ/STL)

USAGE:
    blueprint2mod [OPTIONS] [IMAGE]

ARGS:
    [IMAGE]    Path to a JPG or PNG blueprint image

OPTIONS:
    -o, --output <PATH>      Output path for the exported 3D model
        --format <FORMAT>    Export format: obj (default) or stl
        --session <PATH>     Load a saved .b2m session file
    -h, --help               Print help
    -V, --version            Print version",
        env!("CARGO_PKG_VERSION")
    );
}
