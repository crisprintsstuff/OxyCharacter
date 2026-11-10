//! Feature panels for the editor.
//!
//! The window chrome (bars, rail, shelf frame, inspector stack) lives in
//! [`crate::shell`] and calls into this module. Add a shelf page by extending
//! [`LeftTab`] and [`draw_shelf`]. Add a material section via [`MaterialSection`].
//! Dialogs for projects and the OxyBrush bridge stay here so file-dialog work
//! can land without rewriting the chrome.

use bevy::prelude::*;
use bevy_egui::egui;
use egui::{Color32, RichText, Sense, Stroke, Vec2};

use crate::document::{After, PathDialog};
use crate::editor::{self, split_group, Editor, Preset, PRESETS, SKIN_TONES};
use crate::file_dialog;
use crate::theme::{self, Icon, ACCENT, TEXT, TEXT_DIM, WIDGET};

#[derive(Resource, Default)]
pub struct UiState {
    pub left_tab: LeftTab,
    pub preset_filter: Option<&'static str>,
    pub pointer_busy: bool,
    pub styled: bool,
    /// Set when the user confirms an export path; the export system consumes it.
    pub export_path: Option<std::path::PathBuf>,
    pub dialogs: file_dialog::FileDialogState,
    pub right_mode: RightMode,
    /// Window menu hides the shelf. Rail buttons for presets, skin and
    /// accessories show it again. Default (false) keeps the shelf open.
    pub shelf_collapsed: bool,
    pub material_section: MaterialSection,
    pub help_open: bool,
    pub about_open: bool,
    /// Folder typed into the wardrobe shelf (not the shared file dialog).
    pub package_path: String,
    /// Which wardrobe item the shelf is editing.
    pub wardrobe_focus: Option<String>,
    /// Which accessory card the shelf is editing.
    pub accessory_focus: Option<usize>,
    pub plugins: crate::plugins::PluginState,
}

#[derive(Default, PartialEq, Eq, Clone, Copy, Debug)]
pub enum RightMode {
    #[default]
    Morph,
    Pose,
    Material,
    Export,
}

impl RightMode {
    /// Order of the centered mode switch.
    pub const ALL: [Self; 4] = [Self::Morph, Self::Material, Self::Pose, Self::Export];

    pub fn label(self) -> &'static str {
        match self {
            Self::Morph => "Morph",
            Self::Material => "Material",
            Self::Pose => "Pose",
            Self::Export => "Export",
        }
    }
}

#[derive(Default, PartialEq, Clone, Copy)]
pub enum LeftTab {
    #[default]
    Presets,
    Skin,
    Accessories,
    Wardrobe,
}

/// Sub-sections of the Material mode inspector.
#[derive(Default, PartialEq, Eq, Clone, Copy, Debug)]
pub enum MaterialSection {
    #[default]
    Skin,
    Eyes,
    Bake,
}

/// Shelf body for [`LeftTab`]. The shelf frame and tab strip are in `shell`.
pub(crate) fn draw_shelf(
    ui: &mut egui::Ui,
    ed: &mut Editor,
    tab: LeftTab,
    filter: &mut Option<&'static str>,
    package_path: &mut String,
    wardrobe_focus: &mut Option<String>,
    accessory_focus: &mut Option<usize>,
) {
    match tab {
        LeftTab::Presets => presets_shelf(ui, ed, filter),
        LeftTab::Skin => skin_shelf(ui, ed),
        LeftTab::Accessories => accessories_shelf(ui, ed, accessory_focus),
        LeftTab::Wardrobe => crate::wardrobe_ui::shelf(ui, ed, package_path, wardrobe_focus),
    }
}

fn presets_shelf(ui: &mut egui::Ui, ed: &mut Editor, filter: &mut Option<&'static str>) {
    let mut kinds = Vec::new();
    for p in PRESETS {
        if !kinds.contains(&p.kind) {
            kinds.push(p.kind);
        }
    }
    ui.with_layout(egui::Layout::left_to_right(egui::Align::Min), |ui| {
        ui.vertical(|ui| {
            ui.set_width(118.0);
            if theme::nav_item(ui, "All", filter.is_none()).clicked() {
                *filter = None;
            }
            for kind in kinds {
                if theme::nav_item(ui, kind, *filter == Some(kind)).clicked() {
                    *filter = Some(kind);
                }
            }
        });
        ui.separator();
        let presets: Vec<&Preset> = PRESETS
            .iter()
            .filter(|p| filter.is_none_or(|f| f == p.kind))
            .collect();
        egui::ScrollArea::horizontal()
            .id_salt("preset-cards")
            .max_height(116.0)
            .auto_shrink([false, false])
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 12.0;
                    ui.add_space(4.0);
                    for p in presets {
                        preset_card(ui, ed, p);
                    }
                });
            });
    });
}

fn preset_card(ui: &mut egui::Ui, ed: &mut Editor, p: &Preset) {
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(84.0, 108.0), Sense::click());
    let circle = rect.center_top() + Vec2::new(0.0, 36.0);
    let fill = if resp.hovered() {
        theme::WIDGET_HOVER
    } else {
        WIDGET
    };
    ui.painter().circle_filled(circle, 30.0, fill);
    if resp.hovered() {
        ui.painter()
            .circle_stroke(circle, 30.0, Stroke::new(1.5, ACCENT));
    }
    let fig = egui::Rect::from_center_size(circle + Vec2::new(0.0, 4.0), Vec2::new(34.0, 46.0));
    draw_figure(ui.painter(), fig, p.values);
    ui.painter().text(
        rect.center_bottom(),
        egui::Align2::CENTER_BOTTOM,
        p.name,
        egui::FontId::proportional(11.0),
        TEXT,
    );
    if resp.on_hover_text(format!("{} preset", p.kind)).clicked() {
        ed.apply_preset(p);
    }
}

/// Procedural silhouette thumbnail from a preset's macro values.
fn draw_figure(painter: &egui::Painter, area: egui::Rect, values: &[(&str, f32)]) {
    let get = |k: &str, d: f32| values.iter().find(|(id, _)| *id == k).map_or(d, |v| v.1);
    let (gender, age, muscle, weight, height) = (
        get("gender", 0.5),
        get("age", 0.5),
        get("muscle", 0.5),
        get("weight", 0.5),
        get("height", 0.0),
    );
    let growth = (editor::age_years(age) / 25.0).clamp(0.5, 1.0);
    let h = area.height() * growth * (0.9 + 0.1 * height);
    let cx = area.center().x;
    let feet = area.bottom();
    let top = feet - h;
    let head_r = h * (0.068 + 0.05 * (1.0 - growth));
    let sh = h * (0.10 + 0.045 * gender + 0.03 * muscle + 0.02 * weight);
    let hip = h * (0.085 + 0.035 * (1.0 - gender) + 0.04 * weight);
    let waist = h * (0.065 + 0.05 * weight + 0.01 * gender);
    let limb = h * (0.035 + 0.025 * weight + 0.015 * muscle);
    let col = ACCENT.linear_multiply(0.9);
    let y = |f: f32| top + h * f;
    let p = egui::pos2;
    painter.circle_filled(p(cx, top + head_r), head_r, col);
    let neck_y = top + head_r * 2.0 + h * 0.02;
    painter.add(egui::Shape::convex_polygon(
        vec![
            p(cx - sh, neck_y + h * 0.02),
            p(cx + sh, neck_y + h * 0.02),
            p(cx + waist, y(0.42)),
            p(cx + hip, y(0.52)),
            p(cx - hip, y(0.52)),
            p(cx - waist, y(0.42)),
        ],
        col,
        Stroke::NONE,
    ));
    let stroke = Stroke::new(limb * 2.0, col);
    for s in [-1.0f32, 1.0] {
        painter.line_segment(
            [
                p(cx + s * hip * 0.5, y(0.5)),
                p(cx + s * hip * 0.42, feet - limb * 0.5),
            ],
            stroke,
        );
        painter.line_segment(
            [
                p(cx + s * sh * 0.92, neck_y + h * 0.04),
                p(cx + s * (sh + h * 0.07), y(0.5)),
            ],
            Stroke::new(limb * 1.6, col),
        );
    }
}

fn skin_shelf(ui: &mut egui::Ui, ed: &mut Editor) {
    ui.with_layout(egui::Layout::left_to_right(egui::Align::Min), |ui| {
        ui.vertical(|ui| {
            ui.set_width(118.0);
            let _ = theme::nav_item(ui, "Tones", true);
        });
        ui.separator();
        egui::ScrollArea::horizontal()
            .id_salt("skin-cards")
            .max_height(116.0)
            .auto_shrink([false, false])
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 10.0;
                    ui.add_space(4.0);
                    for (name, rgb) in SKIN_TONES {
                        if tone_circle(ui, name, rgb, ed.skin == *rgb, 52.0).clicked() {
                            ed.skin = *rgb;
                        }
                    }
                    custom_tone(ui, ed);
                });
            });
    });
}

fn tone_circle(
    ui: &mut egui::Ui,
    name: &str,
    rgb: &[f32; 3],
    selected: bool,
    diameter: f32,
) -> egui::Response {
    let (rect, resp) =
        ui.allocate_exact_size(Vec2::new(diameter + 8.0, diameter + 20.0), Sense::click());
    let c = rect.center_top() + Vec2::new(0.0, diameter * 0.5 + 2.0);
    ui.painter().circle_filled(
        c,
        diameter * 0.5 - 1.0,
        egui::Rgba::from_rgb(rgb[0], rgb[1], rgb[2]),
    );
    if selected || resp.hovered() {
        ui.painter()
            .circle_stroke(c, diameter * 0.5, Stroke::new(2.0, ACCENT));
    }
    ui.painter().text(
        rect.center_bottom(),
        egui::Align2::CENTER_BOTTOM,
        name,
        egui::FontId::proportional(10.5),
        TEXT_DIM,
    );
    resp.on_hover_text(name)
}

fn custom_tone(ui: &mut egui::Ui, ed: &mut Editor) {
    ui.vertical(|ui| {
        ui.add_space(16.0);
        ui.label(RichText::new("Custom").size(11.0).color(TEXT_DIM));
        let _ = color_button(ui, &mut ed.skin);
    });
}

fn accessories_shelf(ui: &mut egui::Ui, ed: &mut Editor, focus: &mut Option<usize>) {
    if focus.is_some_and(|i| i >= ed.accessories.len()) {
        *focus = None;
    }
    ui.with_layout(egui::Layout::left_to_right(egui::Align::Min), |ui| {
        ui.vertical(|ui| {
            ui.set_width(118.0);
            let _ = theme::nav_item(ui, "Imported", focus.is_none());
            if let Some(i) = *focus {
                if i < ed.accessories.len() {
                    ui.add_space(4.0);
                    let method = ed.accessories[i].settings.method;
                    for (label, m) in [
                        ("Hold", oxy_fit::FitMethod::Rigid),
                        ("Fit", oxy_fit::FitMethod::Surface),
                        ("Loose", oxy_fit::FitMethod::Loose),
                    ] {
                        if theme::nav_item(ui, label, method == m).clicked() && method != m {
                            ed.accessories[i].settings.method = m;
                            ed.accessories[i].binding = None;
                            ed.accessories[i].influences = None;
                            ed.accessories[i].fitted = None;
                            if m != oxy_fit::FitMethod::Rigid {
                                ed.accessories[i].settings.hide_body = true;
                            }
                            ed.dirty = true;
                        }
                    }
                }
            }
        });
        ui.separator();
        if ed.accessories.is_empty() {
            ui.vertical(|ui| {
                ui.add_space(8.0);
                ui.label(RichText::new("No accessories yet.").color(TEXT_DIM));
                ui.label(
                    RichText::new(
                        "Pieces saved from OxyBrush show up here. One that sits on the body is fitted; one held away only follows the skeleton.",
                    )
                    .size(11.5)
                    .color(TEXT_DIM),
                );
            });
            return;
        }
        let mut remove = None;
        let count = ed.accessories.len();
        egui::ScrollArea::horizontal()
            .id_salt("acc-cards")
            .max_height(116.0)
            .auto_shrink([false, false])
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 12.0;
                    for i in 0..count {
                        let name = ed.accessories[i].acc.name.clone();
                        let tris = ed.accessories[i].acc.triangles.len();
                        let visible = ed.accessories[i].visible;
                        let (rect, card) = ui.allocate_exact_size(Vec2::new(96.0, 112.0), Sense::click());
                        if card.clicked() {
                            *focus = Some(i);
                        }
                        if *focus == Some(i) {
                            ui.painter().rect_stroke(
                                rect,
                                egui::CornerRadius::same(6),
                                Stroke::new(1.5, ACCENT),
                                egui::StrokeKind::Inside,
                            );
                        }
                        let circle = rect.center_top() + Vec2::new(0.0, 32.0);
                        ui.painter().circle_filled(
                            circle,
                            26.0,
                            if visible { WIDGET } else { theme::CANVAS },
                        );
                        let letter: String = name.chars().take(1).collect();
                        ui.painter().text(
                            circle,
                            egui::Align2::CENTER_CENTER,
                            letter,
                            egui::FontId::proportional(16.0),
                            if visible { TEXT } else { TEXT_DIM },
                        );
                        ui.painter().text(
                            rect.center_top() + Vec2::new(0.0, 68.0),
                            egui::Align2::CENTER_CENTER,
                            short_name(&name),
                            egui::FontId::proportional(11.0),
                            TEXT,
                        );
                        let eye_rect = egui::Rect::from_center_size(
                            rect.left_bottom() + Vec2::new(24.0, -14.0),
                            Vec2::splat(22.0),
                        );
                        let trash_rect = egui::Rect::from_center_size(
                            rect.right_bottom() + Vec2::new(-24.0, -14.0),
                            Vec2::splat(22.0),
                        );
                        let mut eye_ui = ui.new_child(
                            egui::UiBuilder::new()
                                .max_rect(eye_rect)
                                .layout(egui::Layout::left_to_right(egui::Align::Center)),
                        );
                        if theme::icon_button_sized(
                            &mut eye_ui,
                            if visible { Icon::Eye } else { Icon::EyeOff },
                            "Show / hide",
                            false,
                            true,
                            22.0,
                        )
                        .clicked()
                        {
                            ed.accessories[i].visible = !visible;
                            ed.accessories_dirty = true;
                        }
                        let mut trash_ui = ui.new_child(
                            egui::UiBuilder::new()
                                .max_rect(trash_rect)
                                .layout(egui::Layout::left_to_right(egui::Align::Center)),
                        );
                        if theme::icon_button_sized(&mut trash_ui, Icon::Trash, "Remove", false, true, 22.0)
                            .on_hover_text(format!("{name} · {tris} triangles"))
                            .clicked()
                        {
                            remove = Some(i);
                        }
                    }
                });
            });
        if let Some(i) = remove {
            let a = ed.accessories.remove(i);
            ed.status = format!("Removed accessory “{}”", a.acc.name);
            ed.accessories_dirty = true;
        }
    });
}

fn short_name(name: &str) -> String {
    let count = name.chars().count();
    if count > 12 {
        let mut s: String = name.chars().take(11).collect();
        s.push('…');
        s
    } else {
        name.to_string()
    }
}

/// Category rows under the selected top-level tab (or search hits).
pub(crate) fn morph_sub_list(ui: &mut egui::Ui, ed: &mut Editor) {
    if !ed.search.trim().is_empty() {
        let hits: Vec<(String, String)> = ed
            .search_results()
            .into_iter()
            .map(|p| {
                let (_top, sub) = split_group(p);
                (p.id.clone(), format!("{sub} · {}", p.label))
            })
            .collect();
        if hits.is_empty() {
            ui.label(RichText::new("No matching morphs.").color(TEXT_DIM));
            return;
        }
        for (id, label) in hits {
            let resp = theme::selectable_row(ui, &id, &label, "", false);
            if resp.hovered() {
                ed.hovered = Some(id);
            }
        }
        return;
    }
    let top = ed.selected.0.clone();
    let subs: Vec<String> = ed
        .categories
        .iter()
        .find(|c| c.top == top)
        .map(|c| c.subs.clone())
        .unwrap_or_default();
    if subs.is_empty() {
        ui.label(RichText::new("Nothing in this category.").color(TEXT_DIM));
        return;
    }
    for sub in subs {
        let n = ed.modified_count(&top, Some(&sub));
        let trailing = if n > 0 { n.to_string() } else { String::new() };
        let sel = ed.selected.1 == sub;
        if theme::selectable_row(ui, (&top, &sub), &sub, &trailing, sel).clicked() {
            ed.selected = (top.clone(), sub);
        }
    }
}

/// Sliders for the selected group, or for the current search.
pub(crate) fn morph_sliders(ui: &mut egui::Ui, ed: &mut Editor) {
    let searching = !ed.search.trim().is_empty();
    let ids: Vec<String> = if searching {
        ed.search_results()
            .into_iter()
            .map(|p| p.id.clone())
            .collect()
    } else {
        let (t, s) = ed.selected.clone();
        ed.params_in(&t, &s).map(|p| p.id.clone()).collect()
    };
    if ids.is_empty() {
        ui.label(
            RichText::new(if searching {
                "No matching morphs."
            } else {
                "No morphs in this group."
            })
            .color(TEXT_DIM),
        );
        return;
    }
    let mut shown = false;
    for id in ids {
        let Some(p) = ed.asset.catalog.param(&id).cloned() else {
            continue;
        };
        if ed.modified_only && !ed.is_modified(&p) {
            continue;
        }
        shown = true;
        slider_row(ui, ed, &p, searching);
    }
    if !shown {
        ui.label(RichText::new("No modified morphs in this group.").color(TEXT_DIM));
    }
}

/// One morph on the tool-options bar (the parameter under the pointer).
pub(crate) fn compact_param(ui: &mut egui::Ui, ed: &mut Editor, id: &str) {
    let Some(p) = ed.asset.catalog.param(id).cloned() else {
        return;
    };
    ui.label(RichText::new(&p.label).size(12.0).color(TEXT));
    let mut v = ed.value(&p.id);
    ui.spacing_mut().slider_width = 110.0;
    let resp = ui.add(
        egui::Slider::new(&mut v, p.min..=p.max)
            .show_value(false)
            .trailing_fill(p.min >= 0.0),
    );
    if resp.changed() {
        ed.set(&p.id, v);
    }
    if resp.double_clicked() {
        ed.set(&p.id, p.default);
    }
    if resp.dragged() || resp.hovered() {
        ed.hovered = Some(p.id.clone());
    }
    ui.label(
        RichText::new(ed.display_value(&p))
            .monospace()
            .color(if ed.is_modified(&p) { ACCENT } else { TEXT_DIM }),
    );
}

fn slider_row(ui: &mut egui::Ui, ed: &mut Editor, p: &oxy_morph::ParamDef, show_group: bool) {
    let modified = ed.is_modified(p);
    let label = if show_group {
        format!("{} · {}", split_group(p).1, p.label)
    } else {
        p.label.clone()
    };
    let row = ui.vertical(|ui| {
        ui.horizontal(|ui| {
            let txt = RichText::new(label).color(if modified {
                Color32::WHITE
            } else {
                Color32::from_gray(205)
            });
            ui.label(txt);
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if p.id.starts_with(crate::bridge::CUSTOM_PREFIX)
                    && ui
                        .add(egui::Button::new("🗑").small())
                        .on_hover_text("Delete this sculpted morph")
                        .clicked()
                {
                    ed.delete_custom_morph(&p.id);
                }
                let reset = ui
                    .add_enabled(modified, egui::Button::new("↺").small())
                    .on_hover_text("Reset");
                if reset.clicked() {
                    ed.set(&p.id, p.default);
                }
                ui.label(
                    RichText::new(ed.display_value(p))
                        .monospace()
                        .color(if modified { ACCENT } else { TEXT_DIM }),
                );
            });
        });
        let mut v = ed.value(&p.id);
        ui.spacing_mut().slider_width = ui.available_width();
        let resp = ui.add(
            egui::Slider::new(&mut v, p.min..=p.max)
                .show_value(false)
                .trailing_fill(p.min >= 0.0),
        );
        if resp.changed() {
            ed.set(&p.id, v);
        }
        if resp.double_clicked() {
            ed.set(&p.id, p.default);
        }
        if let Some([lo, hi]) = &p.ends {
            ui.horizontal(|ui| {
                ui.label(RichText::new(lo).size(10.0).color(TEXT_DIM));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(RichText::new(hi).size(10.0).color(TEXT_DIM));
                });
            });
        }
        resp.hovered() || resp.dragged()
    });
    ui.add_space(3.0);
    if row.inner || row.response.contains_pointer() {
        ed.hovered = Some(p.id.clone());
    }
}

pub(crate) fn bridge_windows(ctx: &egui::Context, ed: &mut Editor) {
    let Some(pending) = ed.bridge.pending.as_mut() else {
        return;
    };
    let mut apply = false;
    let mut cancel = false;
    egui::Window::new("Import from OxyBrush")
        .collapsible(false)
        .resizable(false)
        .anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO)
        .show(ctx, |ui| {
            ui.label(
                RichText::new(pending.path.display().to_string())
                    .size(11.0)
                    .color(TEXT_DIM),
            );
            ui.add_space(4.0);
            egui::Grid::new("pending")
                .num_columns(3)
                .spacing([10.0, 6.0])
                .show(ui, |ui| {
                    for it in &mut pending.items {
                        ui.checkbox(&mut it.include, "");
                        let (kind, detail) = match &it.item {
                            oxy_io::oxybrush::Imported::Morph {
                                indices,
                                max_displacement,
                                ..
                            } => (
                                RichText::new("Morph").color(ACCENT).strong(),
                                format!(
                                    "{} vertices moved, up to {:.1} mm",
                                    indices.len(),
                                    max_displacement * 1000.0
                                ),
                            ),
                            oxy_io::oxybrush::Imported::Accessory(a) => (
                                RichText::new("Accessory")
                                    .color(Color32::from_rgb(220, 170, 80))
                                    .strong(),
                                format!("{} triangles", a.triangles.len()),
                            ),
                            oxy_io::oxybrush::Imported::Unchanged { .. } => {
                                (RichText::new("Body"), "unchanged".into())
                            }
                        };
                        ui.label(kind);
                        ui.vertical(|ui| {
                            ui.add(egui::TextEdit::singleline(&mut it.name).desired_width(240.0));
                            ui.label(RichText::new(detail).size(11.0).color(TEXT_DIM));
                        });
                        ui.end_row();
                    }
                });
            for n in &pending.notes {
                ui.add_space(2.0);
                ui.label(RichText::new(format!("ⓘ {n}")).size(11.0).color(TEXT_DIM));
            }
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                apply = ui
                    .add_enabled(
                        pending.items.iter().any(|i| i.include),
                        egui::Button::new("Import"),
                    )
                    .clicked();
                cancel = ui.button("Cancel").clicked();
            });
            ui.label(
                RichText::new("Morphs become sliders under Custom › Sculpted.")
                    .size(11.0)
                    .color(TEXT_DIM),
            );
        });
    if apply {
        ed.apply_pending();
    } else if cancel {
        ed.bridge.pending = None;
        ed.status = "Import cancelled".into();
    }
}

pub(crate) fn pose_list(ui: &mut egui::Ui, ed: &mut Editor) {
    if ed.asset.rig.is_none() {
        ui.label(
            RichText::new(
                "This character has no skeleton. Rebuild the bundle with tools/fetch-assets.sh.",
            )
            .color(TEXT_DIM),
        );
        return;
    }
    for (i, p) in oxy_rig::pose::PRESETS.iter().enumerate() {
        if theme::selectable_row(ui, i, p.name, "", ed.pose == i).clicked() {
            ed.set_pose(i);
        }
    }
}

pub(crate) fn pose_properties(ui: &mut egui::Ui, ed: &mut Editor) {
    let Some(rig) = &ed.asset.rig else {
        ui.label(
            RichText::new(
                "This character has no skeleton. Rebuild the bundle with tools/fetch-assets.sh.",
            )
            .color(TEXT_DIM),
        );
        return;
    };
    ui.label(
        RichText::new(format!("{} · {} bones", rig.name, rig.bones.len()))
            .size(11.5)
            .color(TEXT_DIM),
    );
    ui.add_space(6.0);
    ui.label(
        RichText::new(
            "The skeleton refits to every morph. Poses only affect the viewport unless you export that pose as the bind pose.",
        )
        .size(11.0)
        .color(TEXT_DIM),
    );
    ui.add_space(6.0);
    ui.checkbox(&mut ed.show_skeleton, "Show skeleton");
}

pub(crate) fn export_summary(ui: &mut egui::Ui, ed: &Editor) {
    use oxy_io::export::BindPose;
    let e = &ed.export;
    let bind = match e.bind_pose {
        BindPose::Rest => "Rest (A-pose)",
        BindPose::TPose => "T-pose",
    };
    let on = |v: bool| if v { "on" } else { "off" };
    let accessories = ed.accessories.iter().filter(|a| a.visible).count();
    for line in [
        format!("Skeleton and weights: {}", on(e.skeleton)),
        format!("Bind pose: {bind}"),
        format!("Sculpts as blendshapes: {}", on(e.blendshapes)),
        format!("Baked textures: {}", on(e.textures)),
        format!("Texture size: {}", e.texture_size),
        format!("Eyes: {}", on(e.eyes)),
        format!("Visible accessories: {accessories}"),
        format!(
            "Worn garments: {}",
            ed.worn.iter().filter(|w| w.settings.visible).count()
        ),
    ] {
        ui.label(RichText::new(line).size(12.5));
        ui.add_space(2.0);
    }
    ui.add_space(4.0);
    ui.label(
        RichText::new("Export asks for a path. The suggestion is exports/<name>.glb.")
            .size(11.0)
            .color(TEXT_DIM),
    );
}

pub(crate) fn export_controls(ui: &mut egui::Ui, ed: &mut Editor) {
    use oxy_io::export::BindPose;
    let export = &mut ed.export;
    ui.checkbox(&mut export.skeleton, "Skeleton and skin weights");
    ui.add_enabled_ui(export.skeleton, |ui| {
        ui.horizontal(|ui| {
            ui.label("Bind pose");
            ui.selectable_value(&mut export.bind_pose, BindPose::Rest, "Rest (A-pose)");
            ui.selectable_value(&mut export.bind_pose, BindPose::TPose, "T-pose");
        });
    });
    ui.checkbox(&mut export.blendshapes, "Sculpted morphs as blendshapes").on_hover_text(
        "Off: sculpts are baked into the mesh. On: each sculpt becomes a named morph target at its current weight.",
    );
    ui.checkbox(&mut export.textures, "Bake and embed textures");
    ui.add_enabled_ui(export.textures, |ui| {
        ui.horizontal(|ui| {
            ui.label("Size");
            for size in [1024u32, 2048, 4096] {
                ui.selectable_value(&mut export.texture_size, size, size.to_string());
            }
        });
    });
    ui.checkbox(&mut export.eyes, "Include eyes");
    let accessories = ed.accessories.iter().filter(|a| a.visible).count();
    ui.label(
        RichText::new(format!(
            "Includes the body{}{}.",
            {
                let clothes = ed.worn.iter().filter(|w| w.settings.visible).count();
                if clothes > 0 {
                    format!(", {clothes} fitted garment(s)")
                } else {
                    String::new()
                }
            },
            if accessories > 0 {
                format!(" and {accessories} accessory mesh(es)")
            } else {
                String::new()
            }
        ))
        .size(11.0)
        .color(TEXT_DIM),
    );
    ui.add_space(6.0);
    if ui
        .add(egui::Button::new("Export .glb…").min_size(Vec2::new(140.0, 28.0)))
        .clicked()
    {
        request_dialog(ed, PathDialog::Export);
    }
    ui.label(
        RichText::new("Asks for a path. Defaults to exports/<project>.glb.")
            .size(11.0)
            .color(TEXT_DIM),
    );
}

/// Menu, shortcut, or button asked for a file. The shell turns this into a
/// native dialog or the typed fallback.
pub(crate) fn request_dialog(ed: &mut Editor, kind: PathDialog) {
    ed.session.as_mut().expect("session").dialog_request = Some(kind);
}

/// Drive native and typed file dialogs. Call once per frame from the chrome.
pub(crate) fn service_dialogs(ctx: &egui::Context, ed: &mut Editor, state: &mut UiState) {
    file_dialog::take_request(ed, &mut state.dialogs);
    file_dialog::poll_native(ctx, ed, &mut state.dialogs, &mut state.export_path);
    file_dialog::draw_typed(ctx, ed, &mut state.export_path);
}

pub(crate) fn shortcuts(ctx: &egui::Context, ed: &mut Editor) {
    use egui::{Key, KeyboardShortcut, Modifiers};
    if ctx.egui_wants_keyboard_input() {
        return; // typing in a text field
    }
    let hit =
        |m: Modifiers, k: Key| ctx.input_mut(|i| i.consume_shortcut(&KeyboardShortcut::new(m, k)));
    let shift = Modifiers::COMMAND | Modifiers::SHIFT;
    // Longer chords first, so Ctrl+Shift+Z isn't taken as Ctrl+Z.
    if hit(shift, Key::Z) || hit(Modifiers::COMMAND, Key::Y) {
        ed.redo();
    } else if hit(Modifiers::COMMAND, Key::Z) {
        ed.undo();
    }
    if hit(shift, Key::S) {
        request_dialog(ed, PathDialog::SaveAs);
    } else if hit(Modifiers::COMMAND, Key::S) {
        ed.save_or_ask();
    }
    if hit(Modifiers::COMMAND, Key::O) {
        request_dialog(ed, PathDialog::Open);
    }
    if hit(Modifiers::COMMAND, Key::N) {
        ed.request(After::New);
    }
}

pub(crate) fn document_windows(ctx: &egui::Context, ed: &mut Editor) {
    // Open / Save as / Export / Import live in `file_dialog` (native or typed).

    // Unsaved changes.
    if let Some(after) = ed.session().confirm.clone() {
        let what = match after {
            After::New => "starting a new character",
            After::Open(_) => "opening another project",
            After::Quit => "quitting",
        };
        let mut answer: Option<Option<bool>> = None;
        egui::Window::new("Unsaved changes")
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO)
            .show(ctx, |ui| {
                ui.label(format!(
                    "Save changes to {} before {what}?",
                    ed.session()
                        .path
                        .as_ref()
                        .and_then(|p| p.file_name())
                        .map_or("Untitled".into(), |n| n.to_string_lossy().into_owned())
                ));
                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    if ui.button("Save").clicked() {
                        answer = Some(Some(true));
                    }
                    if ui.button("Don't save").clicked() {
                        answer = Some(Some(false));
                    }
                    if ui.button("Cancel").clicked() {
                        answer = Some(None);
                    }
                });
            });
        if let Some(a) = answer {
            ed.resolve_confirm(a);
        }
    }

    // Crash recovery.
    if ed.session().recover.is_some() {
        let mut answer = None;
        egui::Window::new("Recover unsaved work?").collapsible(false).resizable(false).anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO).show(ctx, |ui| {
            ui.label("OxyCharacter didn't close normally last time. An autosave of your work is available.");
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                if ui.button("Recover").clicked() {
                    answer = Some(true);
                }
                if ui.button("Discard").clicked() {
                    answer = Some(false);
                }
            });
        });
        if let Some(a) = answer {
            ed.resolve_recover(a);
        }
    }

    // Notes from opening a project.
    if !ed.session().notes.is_empty() {
        let mut close = false;
        egui::Window::new("Opened with changes")
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO)
            .show(ctx, |ui| {
                for n in &ed.session().notes {
                    ui.label(format!("ⓘ {n}"));
                }
                close = ui.button("OK").clicked();
            });
        if close {
            ed.session.as_mut().expect("session").notes.clear();
        }
    }
}

/// Linear-RGB colour button.
fn color_button(ui: &mut egui::Ui, c: &mut [f32; 3]) -> bool {
    let mut rgba = egui::Rgba::from_rgb(c[0], c[1], c[2]);
    let changed = egui::color_picker::color_edit_button_rgba(
        ui,
        &mut rgba,
        egui::color_picker::Alpha::Opaque,
    )
    .changed();
    if changed {
        *c = [rgba.r(), rgba.g(), rgba.b()];
    }
    changed
}

fn amount_row(ui: &mut egui::Ui, label: &str, v: &mut f32, color: Option<&mut [f32; 3]>) {
    ui.horizontal(|ui| {
        ui.add_sized([86.0, 18.0], egui::Label::new(label));
        if let Some(c) = color {
            color_button(ui, c);
        }
        ui.spacing_mut().slider_width = (ui.available_width() - 50.0).max(60.0);
        ui.add(
            egui::Slider::new(v, 0.0..=1.0)
                .show_value(true)
                .fixed_decimals(2),
        );
    });
}

/// Iris colour presets (linear RGB).
pub const IRIS_PRESETS: &[(&str, [f32; 3])] = &[
    ("Brown", [0.20, 0.11, 0.05]),
    ("Hazel", [0.32, 0.22, 0.08]),
    ("Amber", [0.55, 0.30, 0.05]),
    ("Green", [0.12, 0.28, 0.12]),
    ("Blue", [0.12, 0.25, 0.48]),
    ("Grey", [0.30, 0.33, 0.36]),
];

/// Swatches for the upper material card.
pub(crate) fn material_browser(ui: &mut egui::Ui, ed: &mut Editor, section: MaterialSection) {
    if ed.asset.render.is_none() {
        ui.label(
            RichText::new(
                "This bundle has no UVs; rebuild it with tools/fetch-assets.sh for textured skin.",
            )
            .color(TEXT_DIM),
        );
        ui.add_space(6.0);
    }
    match section {
        MaterialSection::Skin => {
            ui.horizontal_wrapped(|ui| {
                for (name, rgb) in SKIN_TONES {
                    let (rect, resp) = ui.allocate_exact_size(Vec2::splat(28.0), Sense::click());
                    ui.painter().circle_filled(
                        rect.center(),
                        12.0,
                        egui::Rgba::from_rgb(rgb[0], rgb[1], rgb[2]),
                    );
                    if ed.skin == *rgb || resp.hovered() {
                        ui.painter()
                            .circle_stroke(rect.center(), 13.0, Stroke::new(2.0, ACCENT));
                    }
                    if resp.on_hover_text(*name).clicked() {
                        ed.skin = *rgb;
                    }
                }
                let _ = color_button(ui, &mut ed.skin);
            });
        }
        MaterialSection::Eyes => {
            let e = &mut ed.material.eyes;
            ui.horizontal_wrapped(|ui| {
                for (name, rgb) in IRIS_PRESETS {
                    let (rect, resp) = ui.allocate_exact_size(Vec2::splat(28.0), Sense::click());
                    ui.painter().circle_filled(
                        rect.center(),
                        12.0,
                        egui::Rgba::from_rgb(rgb[0], rgb[1], rgb[2]),
                    );
                    if e.iris == *rgb || resp.hovered() {
                        ui.painter()
                            .circle_stroke(rect.center(), 13.0, Stroke::new(2.0, ACCENT));
                    }
                    if resp.on_hover_text(*name).clicked() {
                        e.iris = *rgb;
                    }
                }
                let _ = color_button(ui, &mut e.iris);
            });
        }
        MaterialSection::Bake => {
            ui.label(
                RichText::new(
                    "Skin textures are generated from these settings (no image files), so they follow any body shape.",
                )
                .size(11.5)
                .color(TEXT_DIM),
            );
            ui.add_space(6.0);
            let e = &ed.export;
            ui.label(format!("Bake: {}", if e.textures { "on" } else { "off" }));
            ui.label(format!("Size: {}", e.texture_size));
            ui.label(format!(
                "Eyes: {}",
                if e.eyes { "included" } else { "omitted" }
            ));
        }
    }
}

/// Sliders and export toggles for the material properties card.
pub(crate) fn material_properties(ui: &mut egui::Ui, ed: &mut Editor, section: MaterialSection) {
    match section {
        MaterialSection::Skin => {
            let s = &mut ed.material.skin;
            amount_row(ui, "Variation", &mut s.variation, None);
            amount_row(ui, "Detail", &mut s.detail, None);
            ui.horizontal(|ui| {
                ui.add_sized([86.0, 18.0], egui::Label::new("Roughness"));
                ui.spacing_mut().slider_width = (ui.available_width() - 50.0).max(60.0);
                ui.add(egui::Slider::new(&mut s.roughness, 0.25..=0.85).fixed_decimals(2));
            });
            amount_row(ui, "Freckles", &mut s.freckles, Some(&mut s.freckle_color));
            amount_row(ui, "Blush", &mut s.blush, Some(&mut s.blush_color));
            amount_row(
                ui,
                "Eye shadow",
                &mut s.eye_shadow,
                Some(&mut s.eye_shadow_color),
            );
            amount_row(ui, "Lips", &mut s.lips, Some(&mut s.lip_color));
            if ui
                .button("New pattern")
                .on_hover_text("Re-seed the procedural variation")
                .clicked()
            {
                s.seed = s.seed.wrapping_add(1);
            }
        }
        MaterialSection::Eyes => {
            let e = &mut ed.material.eyes;
            let old_size = e.size;
            ui.add(egui::Slider::new(&mut e.iris_angle, 0.45..=0.65).text("Iris size"));
            ui.add(egui::Slider::new(&mut e.pupil, 0.2..=0.6).text("Pupil"));
            ui.add(egui::Slider::new(&mut e.size, 0.16..=0.22).text("Eyeball size"));
            if e.size != old_size {
                ed.dirty = true;
            }
        }
        MaterialSection::Bake => {
            ui.checkbox(&mut ed.export.textures, "Bake and embed textures");
            ui.add_enabled_ui(ed.export.textures, |ui| {
                ui.horizontal(|ui| {
                    ui.label("Size");
                    for size in [1024u32, 2048, 4096] {
                        ui.selectable_value(&mut ed.export.texture_size, size, size.to_string());
                    }
                });
            });
            ui.checkbox(&mut ed.export.eyes, "Include eyes");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mode_switch_lists_the_existing_editors() {
        let labels: Vec<_> = RightMode::ALL
            .iter()
            .copied()
            .map(RightMode::label)
            .collect();
        assert_eq!(labels, ["Morph", "Material", "Pose", "Export"]);
        assert_eq!(RightMode::default(), RightMode::Morph);
        assert!(!UiState::default().shelf_collapsed);
    }
}
