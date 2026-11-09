//! Wardrobe shelf page.
//!
//! The shelf frame, tab chips and rail button live in [`crate::shell`]. This
//! module only fills the page, using the shared theme widgets, so the chrome
//! can change without rewriting the clothing controls.

use bevy_egui::egui;
use egui::{Color32, RichText, Stroke, Vec2};

use crate::editor::Editor;
use crate::theme::{self, ACCENT, TEXT, TEXT_DIM};

const SWATCHES: &[(&str, [f32; 3])] = &[
    ("Navy", [0.16, 0.28, 0.48]),
    ("Ink", [0.10, 0.11, 0.14]),
    ("Sand", [0.62, 0.52, 0.40]),
    ("Wine", [0.38, 0.12, 0.16]),
    ("Brown", [0.12, 0.07, 0.045]),
    ("White", [0.78, 0.78, 0.80]),
];

pub(crate) fn shelf(
    ui: &mut egui::Ui,
    ed: &mut Editor,
    package_path: &mut String,
    focus: &mut Option<String>,
) {
    let items = ed.library_items();
    if focus.as_ref().is_some_and(|id| {
        !items.iter().any(|(i, ..)| i == id) && ed.worn.iter().all(|w| &w.id != id)
    }) {
        *focus = None;
    }
    if focus.is_none() {
        *focus = items.first().map(|(id, ..)| id.clone());
    }
    ui.with_layout(egui::Layout::left_to_right(egui::Align::Min), |ui| {
        ui.vertical(|ui| {
            ui.set_width(118.0);
            for (id, name, _, on) in &items {
                let label = if *on { format!("● {name}") } else { name.clone() };
                if theme::nav_item(ui, &label, focus.as_deref() == Some(id)).clicked() {
                    *focus = Some(id.clone());
                }
            }
            for w in ed.worn.iter().filter(|w| w.builtin.is_none()) {
                if theme::nav_item(ui, &w.name, focus.as_deref() == Some(w.id.as_str())).clicked() {
                    *focus = Some(w.id.clone());
                }
            }
            if items.is_empty() && ed.worn.is_empty() {
                ui.label(RichText::new("No pieces").size(11.0).color(TEXT_DIM));
            }
        });
        ui.separator();
        let id = focus.clone();
        ui.vertical(|ui| {
            let Some(id) = id else {
                ui.label(
                    RichText::new("Garments and hair fit the body and follow poses.")
                        .size(12.0)
                        .color(TEXT_DIM),
                );
                return;
            };
            let builtin = items.iter().any(|(i, ..)| i == &id);
            let worn = ed.worn.iter().any(|w| w.id == id || w.builtin.as_deref() == Some(id.as_str()));
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 6.0;
                if builtin
                    && theme::chip(ui, if worn { "Wearing" } else { "Wear" }, worn, true).clicked()
                {
                    ed.toggle_builtin(&id);
                }
                if !builtin && worn && theme::chip(ui, "Remove", false, true).clicked() {
                    ed.worn.retain(|w| w.id != id);
                    ed.dirty = true;
                    ed.clothing_dirty = true;
                    *focus = None;
                }
            });
            if let Some(i) = ed.worn.iter().position(|w| w.id == id || w.builtin.as_deref() == Some(id.as_str())) {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 6.0;
                    let method = ed.worn[i].settings.method;
                    for (label, m) in [
                        ("Fit", oxy_fit::FitMethod::Surface),
                        ("Loose", oxy_fit::FitMethod::Loose),
                        ("Hold", oxy_fit::FitMethod::Rigid),
                    ] {
                        if theme::chip(ui, label, method == m, true).clicked() && method != m {
                            ed.worn[i].settings.method = m;
                            ed.dirty = true;
                        }
                    }
                    let mut hide = ed.worn[i].settings.hide_body;
                    if theme::chip(ui, "Hide body", hide, true).clicked() {
                        hide = !hide;
                        ed.worn[i].settings.hide_body = hide;
                        ed.dirty = true;
                    }
                });
                ui.horizontal(|ui| {
                    ui.label(RichText::new("Tightness").size(11.5).color(TEXT_DIM));
                    ui.spacing_mut().slider_width = 120.0;
                    let mut tight = ed.worn[i].settings.tightness;
                    if ui.add(egui::Slider::new(&mut tight, 0.0..=1.0)).changed() {
                        ed.worn[i].settings.tightness = tight;
                        ed.dirty = true;
                    }
                });
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 8.0;
                    for (name, rgb) in SWATCHES {
                        let on = ed.worn[i].color.iter().zip(*rgb).all(|(a, b)| (a - b).abs() < 0.02);
                        if swatch(ui, name, rgb, on).clicked() {
                            ed.worn[i].color = *rgb;
                            ed.dirty = true;
                            ed.clothing_dirty = true;
                        }
                    }
                });
            } else if builtin {
                ui.label(
                    RichText::new("Not worn. Fit keeps a gap off the skin; Loose follows the body without hugging every limb; Hold only skins.")
                        .size(11.5)
                        .color(TEXT_DIM),
                );
            }
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                ui.label(RichText::new("Package").size(11.5).color(TEXT_DIM));
                ui.add(
                    egui::TextEdit::singleline(package_path)
                        .hint_text("folder with manifest.json")
                        .desired_width(220.0),
                );
                if ui
                    .add_enabled(!package_path.trim().is_empty(), egui::Button::new("Load"))
                    .clicked()
                {
                    let path = package_path.trim().to_string();
                    if let Err(e) = ed.load_wardrobe_dir(std::path::Path::new(&path)) {
                        ed.status = format!("Could not load wardrobe: {e}");
                    }
                }
            });
        });
    });
}

fn swatch(ui: &mut egui::Ui, name: &str, rgb: &[f32; 3], on: bool) -> egui::Response {
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(28.0, 36.0), egui::Sense::click());
    let c = rect.center_top() + Vec2::new(0.0, 12.0);
    ui.painter().circle_filled(
        c,
        10.0,
        Color32::from_rgb(
            (rgb[0] * 255.0) as u8,
            (rgb[1] * 255.0) as u8,
            (rgb[2] * 255.0) as u8,
        ),
    );
    if on || resp.hovered() {
        ui.painter()
            .circle_stroke(c, 11.0, Stroke::new(2.0, ACCENT));
    }
    ui.painter().text(
        rect.center_bottom(),
        egui::Align2::CENTER_BOTTOM,
        name,
        egui::FontId::proportional(9.0),
        TEXT,
    );
    resp.on_hover_text(name)
}
