use eframe::egui;
use egui::{ColorImage, Pos2, Rect, Response, Sense, TextureOptions, Vec2};

use super::{state::AppState, BlueprintApp};
use crate::blueprint::{scale::ScaleReference, ImagePoint, LengthUnit};

/// Top-level egui rendering dispatch.
pub fn render(app: &mut BlueprintApp, ctx: &egui::Context) {
    egui::TopBottomPanel::top("menu_bar").show(ctx, |ui| {
        render_menu_bar(app, ui);
    });

    egui::CentralPanel::default().show(ctx, |ui| match app.state.clone() {
        AppState::Welcome => render_welcome(app, ui),
        AppState::Cropping => render_cropping(app, ui, ctx),
        AppState::ImageLoaded => render_image_loaded(app, ui, ctx),
        AppState::Scaled => render_scaled(app, ui),
        AppState::Analyzing => render_analyzing(app, ui),
        AppState::Analyzed => render_analyzed(app, ui),
        AppState::Clarifying => render_clarifying(app, ui),
        AppState::ModelReady => render_model_ready(app, ui),
        AppState::Exported => render_exported(app, ui),
    });
}

// ── Menu bar ────────────────────────────────────────────────────────────────

fn render_menu_bar(app: &mut BlueprintApp, ui: &mut egui::Ui) {
    egui::menu::bar(ui, |ui| {
        ui.menu_button("File", |ui| {
            if ui.button("Open Image…").clicked() {
                app.action_open_image();
                ui.close_menu();
            }
            if ui.button("Load Session…").clicked() {
                app.action_load_session();
                ui.close_menu();
            }
            ui.separator();
            let can_save = app.state.can_save();
            if ui
                .add_enabled(can_save, egui::Button::new("Save Session…"))
                .clicked()
            {
                app.action_save_session();
                ui.close_menu();
            }
        });
    });
}

// ── Welcome ─────────────────────────────────────────────────────────────────

fn render_welcome(app: &mut BlueprintApp, ui: &mut egui::Ui) {
    // FR-019: persistent warning banner when ML models are unavailable
    if app.rule_based_only {
        egui::Frame::none()
            .fill(egui::Color32::from_rgb(255, 220, 120))
            .inner_margin(egui::Margin::symmetric(8.0, 4.0))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label(
                        egui::RichText::new(
                            "⚠ ML models not found — running in rule-based-only mode (FR-019)",
                        )
                        .color(egui::Color32::from_rgb(100, 60, 0)),
                    );
                });
            });
        ui.add_space(4.0);
    }
    ui.vertical_centered(|ui| {
        ui.add_space(60.0);
        ui.heading("blueprint2mod");
        ui.label("Convert architectural blueprint images to 3D models (OBJ / STL)");
        ui.add_space(20.0);
        if ui.button("Open Blueprint Image…").clicked() {
            app.action_open_image();
        }
        ui.add_space(8.0);
        if ui.button("Load Session…").clicked() {
            app.action_load_session();
        }
        if let Some(ref err) = app.error_message.clone() {
            ui.add_space(12.0);
            ui.colored_label(egui::Color32::RED, err);
        }
    });
}

// ── Cropping: optional pre-scale crop step (FR-024) ─────────────────────────

fn render_cropping(app: &mut BlueprintApp, ui: &mut egui::Ui, ctx: &egui::Context) {
    egui::TopBottomPanel::bottom("crop_controls")
        .resizable(false)
        .show_inside(ui, |ui| {
            ui.add_space(4.0);
            ui.label("Optional: drag to crop out title blocks, legends, or annotations. The crop is applied before all analysis.");
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                let has_sel = app.crop_ui.has_selection();
                if ui
                    .add_enabled(has_sel, egui::Button::new("Confirm Crop"))
                    .clicked()
                {
                    action_confirm_crop(app);
                }
                if ui.button("Skip — use full image").clicked() {
                    app.state = AppState::ImageLoaded;
                    app.crop_ui.reset();
                }
            });
            ui.add_space(4.0);
        });

    egui::CentralPanel::default().show_inside(ui, |ui| {
        render_image_with_crop_drag(app, ui, ctx);
    });
}

/// Confirm the selected crop region: compute pixel coords, store in session, advance state.
fn action_confirm_crop(app: &mut BlueprintApp) {
    use crate::blueprint::CropRegion;

    if let (Some(start), Some(end)) = (app.crop_ui.start_px, app.crop_ui.end_px) {
        let x = start.0.min(end.0);
        let y = start.1.min(end.1);
        let width = start.0.max(end.0) - x;
        let height = start.1.max(end.1) - y;
        if width > 0 && height > 0 {
            if let Some(ref mut session) = app.session {
                session.crop_region = Some(CropRegion { x, y, width, height });
            }
        }
    }
    app.state = AppState::ImageLoaded;
    app.image_texture = None; // force reload with crop applied
    app.crop_ui.reset();
}

/// Display the original (uncropped) image with a drag-to-select overlay (FR-024).
fn render_image_with_crop_drag(app: &mut BlueprintApp, ui: &mut egui::Ui, ctx: &egui::Context) {
    // Load original texture if not yet cached (no crop applied at this stage)
    if app.image_texture.is_none() {
        if let Some(ref session) = app.session {
            if let Ok(dyn_img) = session.image.load_pixels() {
                let rgba = dyn_img.to_rgba8();
                let size = [rgba.width() as usize, rgba.height() as usize];
                let color_image = ColorImage::from_rgba_unmultiplied(size, &rgba);
                app.image_texture =
                    Some(ctx.load_texture("blueprint", color_image, TextureOptions::LINEAR));
            }
        }
    }

    if let Some(ref texture) = app.image_texture.clone() {
        let tex_size = texture.size_vec2();
        let available = ui.available_size();

        let scale_factor = (available.x / tex_size.x)
            .min(available.y / tex_size.y)
            .min(1.0);
        let display_size = tex_size * scale_factor;
        let offset = (available - display_size) * 0.5;
        let image_rect = Rect::from_min_size(ui.min_rect().min + offset, display_size);

        // Draw image
        ui.painter().image(
            texture.id(),
            image_rect,
            Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
            egui::Color32::WHITE,
        );

        // Drag-sensing overlay
        let resp = ui.allocate_rect(image_rect, Sense::drag());

        let tex_w = texture.size()[0] as f32;
        let tex_h = texture.size()[1] as f32;

        let screen_to_px = |screen_pos: egui::Pos2| -> (u32, u32) {
            let norm = (screen_pos - image_rect.min) / display_size;
            let px = (norm.x * tex_w).clamp(0.0, tex_w - 1.0) as u32;
            let py = (norm.y * tex_h).clamp(0.0, tex_h - 1.0) as u32;
            (px, py)
        };

        if resp.drag_started() {
            if let Some(pos) = resp.interact_pointer_pos() {
                let px = screen_to_px(pos);
                app.crop_ui.start_px = Some(px);
                app.crop_ui.end_px = Some(px);
                app.crop_ui.start_screen = Some(pos);
                app.crop_ui.end_screen = Some(pos);
            }
        }
        if resp.dragged() {
            if let Some(pos) = resp.interact_pointer_pos() {
                let px = screen_to_px(pos);
                app.crop_ui.end_px = Some(px);
                app.crop_ui.end_screen = Some(pos);
            }
        }

        // Draw selection rectangle overlay
        if let (Some(s), Some(e)) = (app.crop_ui.start_screen, app.crop_ui.end_screen) {
            let sel_rect = Rect::from_two_pos(s, e);
            ui.painter().rect(
                sel_rect,
                0.0,
                egui::Color32::from_rgba_unmultiplied(80, 140, 255, 40),
                egui::Stroke::new(2.0, egui::Color32::from_rgb(80, 140, 255)),
            );
        }
    } else {
        ui.centered_and_justified(|ui| {
            ui.label("Loading image…");
        });
    }
}

// ── ImageLoaded: scale reference UI (T013, T014) ────────────────────────────

fn render_image_loaded(app: &mut BlueprintApp, ui: &mut egui::Ui, ctx: &egui::Context) {
    // Instructions panel at top
    egui::TopBottomPanel::bottom("scale_controls")
        .resizable(false)
        .show_inside(ui, |ui| {
            render_scale_controls(app, ui);
        });

    // Image display with click detection in remaining space
    egui::CentralPanel::default().show_inside(ui, |ui| {
        render_image_with_clicks(app, ui, ctx);
    });
}

fn render_scale_controls(app: &mut BlueprintApp, ui: &mut egui::Ui) {
    // T067: "Reset Crop" button — visible whenever a crop is active (FR-024).
    let has_crop = app.session.as_ref().and_then(|s| s.crop_region).is_some();
    if has_crop {
        ui.horizontal(|ui| {
            egui::Frame::none()
                .fill(egui::Color32::from_rgb(230, 245, 255))
                .inner_margin(egui::Margin::symmetric(6.0, 2.0))
                .show(ui, |ui| {
                    ui.label(egui::RichText::new("Crop active").small());
                    if ui.small_button("Reset Crop").clicked() {
                        if let Some(ref mut session) = app.session {
                            session.crop_region = None;
                        }
                        app.image_texture = None; // force full-image reload
                        app.scale_ui.reset();
                        app.state = AppState::Cropping;
                    }
                });
        });
        ui.add_space(2.0);
    }

    ui.add_space(4.0);
    match (&app.scale_ui.point_a, &app.scale_ui.point_b) {
        (None, _) => {
            ui.label("Step 1 of 3 — Click the first reference point on the image.");
        }
        (Some(_), None) => {
            ui.label("Step 2 of 3 — Click the second reference point on the image.");
        }
        (Some(_), Some(_)) => {
            ui.horizontal(|ui| {
                ui.label("Step 3 of 3 — Enter the real-world distance between the two points:");
                ui.add(
                    egui::TextEdit::singleline(&mut app.scale_ui.distance_input)
                        .desired_width(80.0)
                        .hint_text("e.g. 3.66"),
                );
                egui::ComboBox::from_id_source("unit_selector")
                    .selected_text(if app.scale_ui.use_meters { "m" } else { "ft" })
                    .show_ui(ui, |ui| {
                        ui.selectable_value(&mut app.scale_ui.use_meters, true, "Meters (m)");
                        ui.selectable_value(&mut app.scale_ui.use_meters, false, "Feet (ft)");
                    });
                let can_confirm = app
                    .scale_ui
                    .distance_input
                    .trim()
                    .parse::<f64>()
                    .is_ok_and(|d| d > 0.0);
                if ui
                    .add_enabled(can_confirm, egui::Button::new("Confirm Scale"))
                    .clicked()
                {
                    action_confirm_scale(app);
                }
                if ui.button("Reset").clicked() {
                    app.scale_ui.reset();
                }
            });
            if let Some(ref err) = app.error_message.clone() {
                ui.colored_label(egui::Color32::RED, err);
            }
        }
    }
    ui.add_space(4.0);
}

/// Load (or reuse cached) the blueprint image as an egui texture, then render it
/// in the available space with click detection (FR-002).
fn render_image_with_clicks(app: &mut BlueprintApp, ui: &mut egui::Ui, ctx: &egui::Context) {
    // Ensure texture is loaded (apply crop region if set, FR-024)
    if app.image_texture.is_none() {
        if let Some(ref session) = app.session {
            if let Ok(dyn_img) = session.image.load_pixels() {
                let dyn_img = if let Some(crop) = session.crop_region {
                    dyn_img.crop_imm(crop.x, crop.y, crop.width, crop.height)
                } else {
                    dyn_img
                };
                let rgba = dyn_img.to_rgba8();
                let size = [rgba.width() as usize, rgba.height() as usize];
                let color_image = ColorImage::from_rgba_unmultiplied(size, &rgba);
                app.image_texture =
                    Some(ctx.load_texture("blueprint", color_image, TextureOptions::LINEAR));
            }
        }
    }

    if let Some(ref texture) = app.image_texture.clone() {
        let tex_size = texture.size_vec2();
        let available = ui.available_size();

        // Scale to fit while preserving aspect ratio
        let scale = (available.x / tex_size.x)
            .min(available.y / tex_size.y)
            .min(1.0);
        let display_size = tex_size * scale;

        // Center the image
        let offset = (available - display_size) * 0.5;
        let image_rect = Rect::from_min_size(ui.min_rect().min + offset, display_size);

        // Draw image
        ui.painter().image(
            texture.id(),
            image_rect,
            Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
            egui::Color32::WHITE,
        );

        // Transparent click-sensing overlay
        let resp: Response = ui.allocate_rect(image_rect, Sense::click());

        if resp.clicked() {
            if let Some(cursor) = resp.interact_pointer_pos() {
                // Map display coords back to image pixel coords
                let norm = (cursor - image_rect.min) / display_size;
                let img_w = texture.size()[0] as f32;
                let img_h = texture.size()[1] as f32;
                let px = (norm.x * img_w).clamp(0.0, img_w - 1.0) as u32;
                let py = (norm.y * img_h).clamp(0.0, img_h - 1.0) as u32;
                let pt = ImagePoint { x: px, y: py };
                if app.scale_ui.point_a.is_none() {
                    app.scale_ui.point_a = Some(pt);
                } else if app.scale_ui.point_b.is_none() {
                    app.scale_ui.point_b = Some(pt);
                }
                app.error_message = None;
            }
        }

        // Draw red circles on selected points
        let painter = ui.painter();
        let dot_radius = 6.0;
        let stroke = egui::Stroke::new(2.0, egui::Color32::RED);

        for pt in [&app.scale_ui.point_a, &app.scale_ui.point_b]
            .into_iter()
            .flatten()
        {
            {
                let norm = Vec2::new(
                    pt.x as f32 / (texture.size()[0] as f32),
                    pt.y as f32 / (texture.size()[1] as f32),
                );
                let screen_pos = image_rect.min + norm * display_size;
                painter.circle(
                    screen_pos,
                    dot_radius,
                    egui::Color32::from_rgba_unmultiplied(255, 0, 0, 80),
                    stroke,
                );
            }
        }

        // Draw a line between the two selected points
        if let (Some(pa), Some(pb)) = (&app.scale_ui.point_a, &app.scale_ui.point_b) {
            let tex_wh = Vec2::new(texture.size()[0] as f32, texture.size()[1] as f32);
            let sa = image_rect.min + Vec2::new(pa.x as f32, pa.y as f32) / tex_wh * display_size;
            let sb = image_rect.min + Vec2::new(pb.x as f32, pb.y as f32) / tex_wh * display_size;
            painter.line_segment(
                [sa, sb],
                egui::Stroke::new(1.5, egui::Color32::from_rgb(255, 80, 80)),
            );
        }
    } else {
        ui.centered_and_justified(|ui| {
            ui.label("Loading image…");
        });
    }
}

/// Validate inputs and construct a `ScaleReference`; transition to `Scaled` on success (T014).
fn action_confirm_scale(app: &mut BlueprintApp) {
    let distance: f64 = match app.scale_ui.distance_input.trim().parse() {
        Ok(d) if d > 0.0 => d,
        _ => {
            app.error_message = Some("Distance must be a positive number.".to_string());
            return;
        }
    };
    let (pa, pb) = match (&app.scale_ui.point_a, &app.scale_ui.point_b) {
        (Some(a), Some(b)) => (*a, *b),
        _ => {
            app.error_message = Some("Please select two reference points first.".to_string());
            return;
        }
    };
    let (img_w, img_h) = app
        .session
        .as_ref()
        .map(|s| (s.image.width, s.image.height))
        .unwrap_or((u32::MAX, u32::MAX));
    let unit = if app.scale_ui.use_meters {
        LengthUnit::Meters
    } else {
        LengthUnit::Feet
    };
    match ScaleReference::new(pa, pb, distance, unit, img_w, img_h) {
        Ok(scale) => {
            if let Some(ref mut session) = app.session {
                session.scale = Some(scale);
            }
            app.state = AppState::Scaled;
            app.error_message = None;
            app.scale_ui.reset();
        }
        Err(e) => {
            app.error_message = Some(format!("Invalid scale reference: {}", e));
        }
    }
}

// ── Scaled ───────────────────────────────────────────────────────────────────

fn render_scaled(app: &mut BlueprintApp, ui: &mut egui::Ui) {
    ui.heading("Scale confirmed");
    if let Some(ref session) = app.session {
        if let Some(ref scale) = session.scale {
            ui.label(format!(
                "Scale: {:.2} pixels per {}",
                scale.pixels_per_unit,
                if matches!(scale.unit, LengthUnit::Meters) {
                    "meter"
                } else {
                    "foot"
                }
            ));
        }
    }
    ui.add_space(12.0);
    if ui.button("Analyze Blueprint").clicked() {
        app.action_analyze();
    }
    ui.add_space(4.0);
    if ui.button("Save Session…").clicked() {
        app.action_save_session();
    }
}

// ── Analyzing ────────────────────────────────────────────────────────────────

fn render_analyzing(app: &mut BlueprintApp, ui: &mut egui::Ui) {

    // Poll the background thread and advance stages.
    app.advance_analysis_pipeline();

    ui.vertical_centered(|ui| {
        ui.add_space(40.0);

        if let Some(ref st) = app.analysis_state {
            let label = st.stage.label();
            let elapsed = st.stage_started.elapsed().as_secs_f32();
            let estimated = st.stage.estimated_secs();
            // Clamp to [0, 1); never reaches 1.0 until the stage actually completes.
            let progress = (elapsed / estimated).min(0.99);

            ui.label(egui::RichText::new(label).strong());
            ui.add_space(8.0);
            ui.add(egui::ProgressBar::new(progress).animate(true).desired_width(300.0));
            ui.add_space(4.0);
            ui.label(
                egui::RichText::new(format!("{:.0}%", progress * 100.0))
                    .color(egui::Color32::GRAY),
            );

            // FR-028: non-blocking inline warning when ML timeout has fired.
            if st.ml_timed_out {
                ui.add_space(8.0);
                egui::Frame::none()
                    .fill(egui::Color32::from_rgb(255, 200, 100))
                    .inner_margin(egui::Margin::symmetric(8.0, 4.0))
                    .show(ui, |ui| {
                        ui.label(
                            egui::RichText::new(
                                "⚠ ML timeout — continuing with rule-based detection (FR-028)",
                            )
                            .color(egui::Color32::from_rgb(100, 60, 0)),
                        );
                    });
            }
        } else {
            // Transitioning between states — brief spinner
            ui.label("Analyzing blueprint…");
            ui.spinner();
        }
    });

    // Drive continuous repaints so elapsed time updates.
    ui.ctx().request_repaint();
}

// ── Analyzed ─────────────────────────────────────────────────────────────────

fn render_analyzed(app: &mut BlueprintApp, ui: &mut egui::Ui) {
    ui.heading("Detection complete");

    // Show element-type summary
    if let Some(ref session) = app.session {
        use crate::blueprint::element::ElementType;
        let elems = &session.elements;
        if !elems.is_empty() {
            ui.add_space(6.0);
            ui.label(format!("{} elements detected:", elems.len()));
            ui.indent("elem_counts", |ui| {
                for et in [
                    ElementType::Wall,
                    ElementType::Door,
                    ElementType::Window,
                    ElementType::SlidingDoor,
                    ElementType::Staircase,
                    ElementType::Fireplace,
                    ElementType::Closet,
                    ElementType::Chimney,
                    ElementType::Courtyard,
                    ElementType::Unclassified,
                ] {
                    let count = elems.iter().filter(|e| e.element_type == et).count();
                    if count > 0 {
                        ui.label(format!("{:?}: {}", et, count));
                    }
                }
            });
        }
        let pending_count = session.pending_clarifications.len();
        if pending_count > 0 {
            ui.add_space(4.0);
            ui.colored_label(
                egui::Color32::from_rgb(200, 100, 0),
                format!("{} element(s) need clarification", pending_count),
            );
        }
    }

    ui.add_space(12.0);
    if ui.button("Review & Clarify Elements").clicked() {
        app.state = AppState::Clarifying;
    }
    ui.add_space(4.0);
    if ui
        .button("Generate 3D Model (skip clarification)")
        .clicked()
    {
        app.state = AppState::ModelReady;
    }
}

// ── Clarifying ───────────────────────────────────────────────────────────────

fn render_clarifying(app: &mut BlueprintApp, ui: &mut egui::Ui) {
    use crate::blueprint::element::ElementType;

    ui.heading("Clarify element types");
    ui.add_space(4.0);

    // Find the first pending clarification
    let pending = app
        .session
        .as_ref()
        .and_then(|s| s.pending_clarifications.first().cloned());

    if let Some(ref clarification) = pending {
        // Find the element bounds and type for display
        let elem_info = app
            .session
            .as_ref()
            .and_then(|s| s.elements.iter().find(|e| e.id == clarification.element_id))
            .map(|e| (e.element_type.clone(), e.confidence, e.bounds));

        // T068: image panel with semi-transparent red fill over element bounds (FR-029).
        // Show the blueprint image with the ambiguous element highlighted in red so the
        // user can see exactly what they are being asked to identify.
        if let Some((_, _, elem_bounds)) = &elem_info {
            if let Some(ref texture) = app.image_texture.clone() {
                let ctx = ui.ctx().clone();
                let available = ui.available_size();
                // Reserve the top 40% of the available height for the image panel.
                let img_panel_height = (available.y * 0.40).max(120.0);
                let tex_size = texture.size_vec2();
                let scale_factor = (available.x / tex_size.x)
                    .min(img_panel_height / tex_size.y)
                    .min(1.0);
                let display_size = tex_size * scale_factor;
                let offset_x = (available.x - display_size.x) * 0.5;
                let image_rect =
                    Rect::from_min_size(ui.cursor().min + Vec2::new(offset_x, 0.0), display_size);

                // Draw the blueprint image
                ui.painter().image(
                    texture.id(),
                    image_rect,
                    Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
                    egui::Color32::WHITE,
                );

                // Compute element bounds in texture-pixel space then in screen space.
                // Element bounds are always stored in meters (post T070 fix).
                // To recover pixel coordinates we reverse the to_world_distance * to_meters_factor path.
                let scale_ref = app.session.as_ref().and_then(|s| s.scale.as_ref()).cloned();
                let crop = app.session.as_ref().and_then(|s| s.crop_region);

                if let Some(ref scale_ref) = scale_ref {
                    let m_factor = scale_ref.unit.to_meters_factor();

                    // Convert world-meters back to original image pixel coords.
                    let orig_px = |world_m: f64| -> f32 {
                        (world_m / m_factor * scale_ref.pixels_per_unit) as f32
                    };

                    let mut px_min_x = orig_px(elem_bounds.min.x);
                    let mut px_min_y = orig_px(elem_bounds.min.y);
                    let mut px_max_x = orig_px(elem_bounds.max.x);
                    let mut px_max_y = orig_px(elem_bounds.max.y);

                    // Adjust for crop offset if a crop was applied.
                    if let Some(crop) = crop {
                        px_min_x -= crop.x as f32;
                        px_min_y -= crop.y as f32;
                        px_max_x -= crop.x as f32;
                        px_max_y -= crop.y as f32;
                    }

                    let tex_w = tex_size.x;
                    let tex_h = tex_size.y;

                    // Normalise to [0,1] texture space then map to screen space.
                    let norm_to_screen = |norm_x: f32, norm_y: f32| -> Pos2 {
                        Pos2::new(
                            image_rect.min.x + norm_x * display_size.x,
                            image_rect.min.y + norm_y * display_size.y,
                        )
                    };

                    let screen_min = norm_to_screen(
                        (px_min_x / tex_w).clamp(0.0, 1.0),
                        (px_min_y / tex_h).clamp(0.0, 1.0),
                    );
                    let screen_max = norm_to_screen(
                        (px_max_x / tex_w).clamp(0.0, 1.0),
                        (px_max_y / tex_h).clamp(0.0, 1.0),
                    );

                    let highlight_rect = Rect::from_min_max(screen_min, screen_max);
                    // Semi-transparent red fill (FR-029): Color32::from_rgba_unmultiplied(220,30,30,120)
                    ui.painter().rect_filled(
                        highlight_rect,
                        0.0,
                        egui::Color32::from_rgba_unmultiplied(220, 30, 30, 120),
                    );
                    ui.painter().rect_stroke(
                        highlight_rect,
                        0.0,
                        egui::Stroke::new(2.0, egui::Color32::from_rgb(220, 30, 30)),
                    );
                }

                // Advance the UI cursor past the image area.
                ui.allocate_space(Vec2::new(available.x, display_size.y));
                let _ = ctx; // ctx used for repaint only via painter above
            }
        }

        ui.add_space(4.0);
        ui.group(|ui| {
            ui.label(format!("Pending: {}", clarification.context_snippet));
            if let Some((et, conf, _)) = &elem_info {
                ui.label(format!(
                    "Current type: {:?} ({:.0}% confidence)",
                    et,
                    conf * 100.0
                ));
            }
            ui.add_space(8.0);
            ui.label("Select the correct type:");
            ui.horizontal_wrapped(|ui| {
                for et in [
                    ElementType::Wall,
                    ElementType::Door,
                    ElementType::Window,
                    ElementType::SlidingDoor,
                    ElementType::Staircase,
                    ElementType::Fireplace,
                    ElementType::Closet,
                    ElementType::Chimney,
                    ElementType::Courtyard,
                    ElementType::Unclassified,
                ] {
                    if ui.button(format!("{:?}", et)).clicked() {
                        apply_clarification(app, clarification.element_id, et);
                        return;
                    }
                }
            });
            ui.add_space(8.0);
            if ui.button("Skip (mark as Unclassified)").clicked() {
                // Skip: remove from pending without updating history
                if let Some(ref mut session) = app.session {
                    session
                        .pending_clarifications
                        .retain(|p| p.element_id != clarification.element_id);
                }
                // Check if we're done
                advance_after_clarification(app);
            }
        });

        ui.add_space(8.0);
        let remaining = app
            .session
            .as_ref()
            .map_or(0, |s| s.pending_clarifications.len());
        ui.label(format!("{} item(s) remaining", remaining));
    } else {
        // No more pending clarifications
        ui.label("All elements clarified.");
        ui.add_space(8.0);
        if ui.button("Continue to 3D Model").clicked() {
            app.state = AppState::ModelReady;
        }
    }
}

/// Apply a user correction: update the element type, record in CorrectionHistory,
/// remove the clarification from the pending list, and advance state if done (FR-007, FR-008).
fn apply_clarification(
    app: &mut BlueprintApp,
    element_id: uuid::Uuid,
    corrected_type: crate::blueprint::element::ElementType,
) {
    use crate::correction::history::CorrectionHistory;

    // Find original type and confidence before borrowing mutably
    let (original_type, original_confidence) = app
        .session
        .as_ref()
        .and_then(|s| s.elements.iter().find(|e| e.id == element_id))
        .map(|e| (e.element_type.clone(), e.confidence))
        .unzip();

    if let (Some(orig_type), Some(orig_conf)) = (original_type, original_confidence) {
        // Record correction in history
        let mut history = CorrectionHistory::load_or_default().unwrap_or_default();
        history.record_correction(orig_type, corrected_type.clone(), orig_conf);
        let _ = history.save(); // best-effort persist

        // Update the element type in the session
        if let Some(ref mut session) = app.session {
            if let Some(elem) = session.elements.iter_mut().find(|e| e.id == element_id) {
                elem.element_type = corrected_type;
            }
            session
                .pending_clarifications
                .retain(|p| p.element_id != element_id);
        }
    }

    advance_after_clarification(app);
}

/// Transition to ModelReady if no pending clarifications remain (FR-008).
fn advance_after_clarification(app: &mut BlueprintApp) {
    let done = app
        .session
        .as_ref()
        .map_or(true, |s| s.pending_clarifications.is_empty());
    if done {
        app.state = AppState::ModelReady;
    }
}

// ── ModelReady ───────────────────────────────────────────────────────────────

fn render_model_ready(app: &mut BlueprintApp, ui: &mut egui::Ui) {
    ui.heading("Generate 3D Model");
    ui.add_space(8.0);

    // T042: wall height input (FR-009)
    ui.group(|ui| {
        ui.label("Wall height:");
        ui.horizontal(|ui| {
            ui.add(
                egui::TextEdit::singleline(&mut app.wall_height_input)
                    .desired_width(80.0)
                    .hint_text("2.44"),
            );
            egui::ComboBox::from_id_source("wall_height_unit")
                .selected_text(if app.wall_height_use_meters {
                    "m"
                } else {
                    "ft"
                })
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut app.wall_height_use_meters, true, "Meters (m)");
                    ui.selectable_value(&mut app.wall_height_use_meters, false, "Feet (ft)");
                });
        });
        ui.label("Default: 2.44 m (8 ft)");
    });

    ui.add_space(8.0);

    // T043: format selection and generate+export trigger
    ui.group(|ui| {
        ui.label("Export format:");
        ui.horizontal(|ui| {
            ui.radio_value(
                &mut app.export_format,
                super::ExportFormat::Obj,
                "OBJ + MTL (SketchUp)",
            );
            ui.radio_value(
                &mut app.export_format,
                super::ExportFormat::Stl,
                "STL (binary)",
            );
        });
    });

    ui.add_space(12.0);

    if app.model3d.is_none() {
        let can_generate = app
            .wall_height_input
            .trim()
            .parse::<f64>()
            .is_ok_and(|h| h > 0.0);
        if ui
            .add_enabled(can_generate, egui::Button::new("Generate & Export…"))
            .clicked()
        {
            app.action_generate_model();
            if app.model3d.is_some() {
                app.action_export_file();
            }
        }
    } else {
        if ui.button("Export…").clicked() {
            app.action_export_file();
        }
        ui.add_space(4.0);
        if ui.button("Regenerate with new height").clicked() {
            app.model3d = None;
        }
    }

    if let Some(ref err) = app.error_message.clone() {
        ui.add_space(8.0);
        ui.colored_label(egui::Color32::RED, err);
    }
}

// ── Exported ─────────────────────────────────────────────────────────────────

fn render_exported(app: &mut BlueprintApp, ui: &mut egui::Ui) {
    // T044: end-of-processing summary panel (FR-015)
    ui.heading("Export complete");
    ui.add_space(4.0);

    if let Some(ref path) = app.last_export_path.clone() {
        ui.horizontal(|ui| {
            ui.label("Written to:");
            ui.label(path.to_string_lossy().as_ref());
        });
        ui.horizontal(|ui| {
            if ui.button("Show in Finder").clicked() {
                // Open parent directory
                if let Some(parent) = path.parent() {
                    let _ = std::process::Command::new("open").arg(parent).spawn();
                }
            }
        });
        ui.add_space(8.0);
    }

    // Element type counts
    if let Some(ref session) = app.session {
        use crate::blueprint::element::ElementType;
        use crate::ocr::extractor::TextAnnotationType;

        let elems = &session.elements;
        if !elems.is_empty() {
            ui.collapsing("Detected elements", |ui| {
                for et in [
                    ElementType::Wall,
                    ElementType::Door,
                    ElementType::Window,
                    ElementType::SlidingDoor,
                    ElementType::Staircase,
                    ElementType::Fireplace,
                    ElementType::Closet,
                    ElementType::Chimney,
                    ElementType::Courtyard,
                    ElementType::Unclassified,
                ] {
                    let count = elems.iter().filter(|e| e.element_type == et).count();
                    if count > 0 {
                        ui.label(format!("{:?}: {}", et, count));
                    }
                }
            });
        }

        // Unreadable OCR regions
        let unreadable: Vec<_> = session
            .text_annotations
            .iter()
            .filter(|a| matches!(a.annotation_type, TextAnnotationType::Unreadable))
            .collect();
        if !unreadable.is_empty() {
            ui.add_space(4.0);
            ui.collapsing(
                format!("{} unreadable OCR region(s)", unreadable.len()),
                |ui| {
                    for ann in &unreadable {
                        ui.label(format!(
                            "  @ ({}, {}) {}×{}  text: '{}'",
                            ann.image_bounds.x,
                            ann.image_bounds.y,
                            ann.image_bounds.width,
                            ann.image_bounds.height,
                            ann.raw_text
                        ));
                    }
                },
            );
        }

        // Unclassified segment IDs
        let unclassified: Vec<_> = elems
            .iter()
            .filter(|e| matches!(e.element_type, ElementType::Unclassified))
            .collect();
        if !unclassified.is_empty() {
            ui.add_space(4.0);
            ui.collapsing(
                format!("{} unclassified segment(s)", unclassified.len()),
                |ui| {
                    for e in &unclassified {
                        ui.label(format!("  id: {}", e.id));
                    }
                },
            );
        }
    }

    ui.add_space(12.0);
    if ui.button("Start over").clicked() {
        app.state = AppState::Welcome;
        app.session = None;
        app.model3d = None;
        app.image_texture = None;
        app.last_export_path = None;
    }
}
