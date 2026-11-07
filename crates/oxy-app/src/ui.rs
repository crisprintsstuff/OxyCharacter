//! egui interface: menu + toolbar, content browser (left), morph editor
//! (right), viewport overlay, status bar.

use bevy::prelude::*;
use bevy_egui::{egui, EguiContexts};
use egui::{Color32, CornerRadius, Frame, Margin, RichText, Sense, Stroke, StrokeKind, Vec2};

use crate::editor::{self, split_group, Editor, Preset, PRESETS, SKIN_TONES};
use crate::scene::{CharacterHeight, Orbit, ViewportRect};

pub const ACCENT: Color32 = Color32::from_rgb(38, 140, 230);
const BG: Color32 = Color32::from_rgb(30, 31, 34);
const PANEL: Color32 = Color32::from_rgb(39, 40, 44);
const WIDGET: Color32 = Color32::from_rgb(52, 54, 59);
const TEXT_DIM: Color32 = Color32::from_rgb(150, 154, 160);

#[derive(Resource, Default)]
pub struct UiState {
    pub left_tab: LeftTab,
    pub preset_filter: Option<&'static str>,
    pub pointer_busy: bool,
    pub styled: bool,
    pub export_requested: bool,
    pub export: oxy_io::export::ExportOptions,
    pub right_mode: RightMode,
}

#[derive(Default, PartialEq, Eq, Clone, Copy, Debug)]
pub enum RightMode {
    #[default]
    Morph,
    Pose,
}

#[derive(Default, PartialEq, Clone, Copy)]
pub enum LeftTab {
    #[default]
    Presets,
    Skin,
    Accessories,
}

fn style(ctx: &egui::Context) {
    let mut v = egui::Visuals::dark();
    v.panel_fill = PANEL;
    v.window_fill = PANEL;
    v.extreme_bg_color = BG;
    v.faint_bg_color = Color32::from_rgb(44, 45, 50);
    v.selection.bg_fill = ACCENT.linear_multiply(0.55);
    v.selection.stroke = Stroke::new(1.0, ACCENT);
    v.widgets.inactive.bg_fill = WIDGET;
    v.widgets.inactive.weak_bg_fill = WIDGET;
    v.widgets.hovered.bg_fill = Color32::from_rgb(64, 67, 74);
    v.widgets.hovered.weak_bg_fill = Color32::from_rgb(64, 67, 74);
    v.widgets.active.bg_fill = ACCENT;
    v.slider_trailing_fill = true;
    ctx.set_visuals(v);
    ctx.global_style_mut(|s| {
        s.spacing.item_spacing = Vec2::new(6.0, 5.0);
        s.spacing.slider_width = 150.0;
    });
}

fn panel_header(ui: &mut egui::Ui, title: &str) {
    Frame::NONE
        .fill(BG)
        .inner_margin(Margin::symmetric(8, 5))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.label(RichText::new(title).strong().size(14.0));
        });
}

#[allow(clippy::too_many_arguments)]
pub fn ui_system(
    mut contexts: EguiContexts,
    mut editor: ResMut<Editor>,
    mut state: ResMut<UiState>,
    mut rect_res: ResMut<ViewportRect>,
    mut orbit: ResMut<Orbit>,
    height: Res<CharacterHeight>,
    diagnostics: Res<Time>,
    mut exit: MessageWriter<AppExit>,
) -> Result {
    let ctx = contexts.ctx_mut()?.clone();
    if !state.styled {
        style(&ctx);
        state.styled = true;
    }
    let mut root = egui::Ui::new(
        ctx.clone(),
        "root".into(),
        egui::UiBuilder::new()
            .layer_id(egui::LayerId::background())
            .max_rect(ctx.viewport_rect()),
    );
    let ed = &mut *editor;
    let previous_hover = ed.hovered.take();

    // ── Menu + toolbar ────────────────────────────────────────────────
    egui::Panel::top("menu")
        .frame(Frame::NONE.fill(BG).inner_margin(Margin::symmetric(6, 2)))
        .show(&mut root, |ui| {
            egui::MenuBar::new().ui(ui, |ui| {
                ui.label(RichText::new("OxyCharacter").strong().color(ACCENT));
                ui.separator();
                egui::containers::menu::MenuButton::new("File").ui(ui, |ui| {
                    if ui.button("Send to OxyBrush").clicked() {
                        ed.send_to_oxybrush();
                    }
                    if ui.button("Import OxyBrush file (.oxb)…").clicked() {
                        ed.bridge.open_dialog = true;
                    }
                    ui.separator();
                    if ui.button("Export glTF (.glb)").clicked() {
                        state.export_requested = true;
                    }
                    ui.separator();
                    if ui.button("Quit").clicked() {
                        exit.write(AppExit::Success);
                    }
                });
                egui::containers::menu::MenuButton::new("Edit").ui(ui, |ui| {
                    if ui.button("Reset all morphs").clicked() {
                        ed.reset_all();
                    }
                    if ui.button("Randomize").clicked() {
                        ed.randomize();
                    }
                    ui.checkbox(&mut ed.symmetry, "Symmetric editing");
                });
                egui::containers::menu::MenuButton::new("View").ui(ui, |ui| {
                    if ui.button("Frame full body").clicked() {
                        *orbit = Orbit::full_body(height.0);
                    }
                    if ui.button("Frame head").clicked() {
                        *orbit = Orbit::head(height.0);
                    }
                });
            });
        });
    egui::Panel::top("toolbar")
        .frame(
            Frame::NONE
                .fill(PANEL)
                .inner_margin(Margin::symmetric(8, 5)),
        )
        .show(&mut root, |ui| {
            ui.horizontal(|ui| {
                let tool = |ui: &mut egui::Ui, label: &str, tip: &str| {
                    ui.add(egui::Button::new(label).min_size(Vec2::new(30.0, 26.0)))
                        .on_hover_text(tip)
                };
                if tool(ui, "⟲ Reset", "Reset all morphs to neutral").clicked() {
                    ed.reset_all();
                }
                if tool(ui, "🎲 Random", "Generate a random character").clicked() {
                    ed.randomize();
                }
                ui.separator();
                if tool(ui, "Body", "Frame the full body").clicked() {
                    *orbit = Orbit::full_body(height.0);
                }
                if tool(ui, "Head", "Frame the head").clicked() {
                    *orbit = Orbit::head(height.0);
                }
                ui.separator();
                ui.toggle_value(&mut ed.symmetry, "Symmetry")
                    .on_hover_text("Edit left and right together");
                ui.separator();
                if tool(ui, "⬇ Export", "Export the current character as .glb").clicked() {
                    state.export_requested = true;
                }
                ui.separator();
                if tool(
                    ui,
                    "Send to OxyBrush",
                    "Send the character to OxyBrush. Save there (Ctrl+S): a reshaped body comes back as a morph slider, new pieces as accessories.",
                )
                .clicked()
                {
                    ed.send_to_oxybrush();
                }
                if tool(ui, "Import .oxb", "Import an OxyBrush project (or drop an .oxb file on the window)").clicked() {
                    ed.bridge.open_dialog = true;
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    for (label, mode) in [("Pose", Some(RightMode::Pose)), ("Material", None), ("Morph", Some(RightMode::Morph))] {
                        let selected = mode == Some(state.right_mode);
                        let b = egui::Button::selectable(selected, label).min_size(Vec2::new(70.0, 24.0));
                        let r = ui.add_enabled(mode.is_some(), b).on_disabled_hover_text("Planned for a later milestone");
                        if let (true, Some(m)) = (r.clicked(), mode) {
                            state.right_mode = m;
                        }
                    }
                });
            });
        });
    egui::Panel::bottom("status")
        .frame(Frame::NONE.fill(BG).inner_margin(Margin::symmetric(8, 3)))
        .show(&mut root, |ui| {
            // Right-aligned license first, so the status message truncates into the remaining space.
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label(
                    RichText::new(format!(
                        "Base mesh: {} · {}",
                        ed.asset.meta.name, ed.asset.meta.license
                    ))
                    .color(TEXT_DIM),
                );
                ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                    if let Some((p, _)) = &ed.bridge.watched {
                        ui.label(RichText::new("Linked to OxyBrush").color(ACCENT))
                            .on_hover_text(format!("Watching {} for OxyBrush saves", p.display()));
                    }
                    ui.add(egui::Label::new(RichText::new(&ed.status).color(TEXT_DIM)).truncate());
                });
            });
        });

    // ── Left: content browser ─────────────────────────────────────────
    egui::Panel::left("content")
        .default_size(250.0)
        .resizable(true)
        .frame(Frame::NONE.fill(PANEL))
        .show(&mut root, |ui| {
            panel_header(ui, "Content");
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                ui.add_space(6.0);
                ui.selectable_value(&mut state.left_tab, LeftTab::Presets, "Body Presets");
                ui.selectable_value(&mut state.left_tab, LeftTab::Skin, "Skin");
                let n = ed.accessories.len();
                ui.selectable_value(
                    &mut state.left_tab,
                    LeftTab::Accessories,
                    if n > 0 {
                        format!("Accessories ({n})")
                    } else {
                        "Accessories".into()
                    },
                );
            });
            ui.separator();
            egui::ScrollArea::vertical()
                .auto_shrink(false)
                .show(ui, |ui| {
                    ui.add_space(4.0);
                    match state.left_tab {
                        LeftTab::Presets => presets_tab(ui, ed, &mut state.preset_filter),
                        LeftTab::Skin => skin_tab(ui, ed),
                        LeftTab::Accessories => accessories_tab(ui, ed),
                    }
                });
        });

    // ── Right: morph editor ───────────────────────────────────────────
    egui::Panel::right("morph")
        .default_size(330.0)
        .resizable(true)
        .frame(Frame::NONE.fill(PANEL))
        .show(&mut root, |ui| match state.right_mode {
            RightMode::Morph => {
                panel_header(ui, "Morph");
                morph_panel(ui, ed);
            }
            RightMode::Pose => {
                panel_header(ui, "Pose & Export");
                let st = &mut *state;
                pose_panel(ui, ed, &mut st.export, &mut st.export_requested);
            }
        });

    // ── Center: viewport overlay ──────────────────────────────────────
    let fps = 1.0 / diagnostics.delta_secs().max(1e-4);
    let central = egui::CentralPanel::default()
        .frame(Frame::NONE)
        .show(&mut root, |ui| {
            let r = ui.max_rect();
            let text = format!(
            "Vertices: {}\nTriangles: {}\nActive morph targets: {}\nHeight: {:.2} m\nFPS: {:.0}",
            ed.asset.mesh.base.vertex_count(),
            ed.asset.mesh.base.indices.len() / 3,
            ed.active_targets,
            height.0,
            fps
        );
            ui.painter().text(
                r.min + Vec2::new(10.0, 8.0),
                egui::Align2::LEFT_TOP,
                text,
                egui::FontId::monospace(11.5),
                Color32::from_rgb(230, 196, 90),
            );
            ui.painter().text(
                r.left_bottom() + Vec2::new(10.0, -8.0),
                egui::Align2::LEFT_BOTTOM,
                "LMB orbit · RMB/MMB pan · wheel zoom",
                egui::FontId::proportional(11.0),
                TEXT_DIM,
            );
        });
    let r = central.response.rect;
    rect_res.min = bevy::math::Vec2::new(r.min.x, r.min.y);
    rect_res.max = bevy::math::Vec2::new(r.max.x, r.max.y);

    bridge_windows(&ctx, ed);

    state.pointer_busy = ctx.egui_is_using_pointer();
    if ed.hovered.is_none() {
        ed.hovered = ed.pinned_hover.clone();
    }
    if ed.hovered != previous_hover {
        ed.highlight_dirty = true;
    }
    Ok(())
}

fn presets_tab(ui: &mut egui::Ui, ed: &mut Editor, filter: &mut Option<&'static str>) {
    ui.horizontal_wrapped(|ui| {
        ui.add_space(6.0);
        if ui.selectable_label(filter.is_none(), "All").clicked() {
            *filter = None;
        }
        for kind in ["Adult", "Young", "Elder"] {
            if ui.selectable_label(*filter == Some(kind), kind).clicked() {
                *filter = Some(kind);
            }
        }
    });
    ui.add_space(6.0);
    let gap = 6.0;
    let cols = ((ui.available_width() - gap) / (96.0 + gap))
        .floor()
        .max(1.0) as usize;
    let w = (ui.available_width() - gap * (cols as f32 + 1.0)) / cols as f32;
    let card = Vec2::new(w, w * 1.25);
    let presets: Vec<&Preset> = PRESETS
        .iter()
        .filter(|p| filter.is_none_or(|f| f == p.kind))
        .collect();
    for row in presets.chunks(cols) {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = gap;
            ui.add_space(gap);
            for p in row {
                let (rect, resp) = ui.allocate_exact_size(card, Sense::click());
                let hovered = resp.hovered();
                let painter = ui.painter_at(rect);
                painter.rect_filled(
                    rect,
                    CornerRadius::same(6),
                    if hovered { WIDGET } else { BG },
                );
                if hovered {
                    painter.rect_stroke(
                        rect,
                        CornerRadius::same(6),
                        Stroke::new(1.5, ACCENT),
                        StrokeKind::Inside,
                    );
                }
                let fig = egui::Rect::from_min_max(
                    rect.min + Vec2::new(8.0, 10.0),
                    rect.max - Vec2::new(8.0, 26.0),
                );
                draw_figure(&painter, fig, p.values);
                painter.text(
                    rect.center_bottom() - Vec2::new(0.0, 8.0),
                    egui::Align2::CENTER_BOTTOM,
                    p.name,
                    egui::FontId::proportional(12.0),
                    Color32::WHITE,
                );
                if resp.on_hover_text(format!("{} preset", p.kind)).clicked() {
                    ed.apply_preset(p);
                }
            }
        });
        ui.add_space(gap - 5.0);
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

fn skin_tab(ui: &mut egui::Ui, ed: &mut Editor) {
    ui.horizontal(|ui| {
        ui.add_space(6.0);
        ui.label(RichText::new("Skin tone").color(TEXT_DIM));
    });
    ui.horizontal_wrapped(|ui| {
        ui.add_space(6.0);
        for (name, rgb) in SKIN_TONES {
            let (rect, resp) = ui.allocate_exact_size(Vec2::splat(52.0), Sense::click());
            let c = egui::Rgba::from_rgb(rgb[0], rgb[1], rgb[2]);
            ui.painter().rect_filled(rect, CornerRadius::same(5), c);
            if ed.skin == *rgb || resp.hovered() {
                ui.painter().rect_stroke(
                    rect,
                    CornerRadius::same(5),
                    Stroke::new(2.0, ACCENT),
                    StrokeKind::Outside,
                );
            }
            if resp.on_hover_text(*name).clicked() {
                ed.skin = *rgb;
            }
        }
    });
    ui.add_space(8.0);
    ui.horizontal(|ui| {
        ui.add_space(6.0);
        ui.label("Custom");
        let mut c = egui::Rgba::from_rgb(ed.skin[0], ed.skin[1], ed.skin[2]);
        if egui::color_picker::color_edit_button_rgba(ui, &mut c, egui::color_picker::Alpha::Opaque)
            .changed()
        {
            ed.skin = [c.r(), c.g(), c.b()];
        }
    });
    ui.add_space(8.0);
    ui.horizontal(|ui| {
        ui.add_space(6.0);
        ui.label(
            RichText::new("Layered skin materials arrive in milestone M4.")
                .italics()
                .color(TEXT_DIM),
        );
    });
}

fn morph_panel(ui: &mut egui::Ui, ed: &mut Editor) {
    let pad = Margin::symmetric(8, 6);
    Frame::NONE.inner_margin(pad).show(ui, |ui| {
        ui.horizontal(|ui| {
            ui.label("🔍");
            ui.add(
                egui::TextEdit::singleline(&mut ed.search)
                    .hint_text("Search morphs…")
                    .desired_width(f32::INFINITY),
            );
        });
    });

    // Bottom options first so the scroll areas get the remaining height.
    egui::Panel::bottom("morph_options")
        .frame(Frame::NONE.inner_margin(pad))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.checkbox(&mut ed.modified_only, "Show modified only");
                ui.checkbox(&mut ed.symmetry, "Symmetry");
            });
        });

    let searching = !ed.search.trim().is_empty();
    if !searching {
        // Category tree.
        let tree_h = (ui.available_height() * 0.38).clamp(140.0, 300.0);
        Frame::NONE
            .fill(BG)
            .inner_margin(Margin::symmetric(6, 4))
            .show(ui, |ui| {
                egui::ScrollArea::vertical()
                    .id_salt("tree")
                    .max_height(tree_h)
                    .auto_shrink([false, true])
                    .show(ui, |ui| {
                        ui.set_width(ui.available_width());
                        let cats = ed.categories.clone();
                        for cat in &cats {
                            let n = ed.modified_count(&cat.top, None);
                            let title = if n > 0 {
                                format!("{}  ({n})", cat.top)
                            } else {
                                cat.top.clone()
                            };
                            egui::CollapsingHeader::new(RichText::new(title).strong())
                                .id_salt(&cat.top)
                                .default_open(cat.top == "Body" || cat.top == "Head")
                                .show(ui, |ui| {
                                    for sub in &cat.subs {
                                        let m = ed.modified_count(&cat.top, Some(sub));
                                        let label = if m > 0 {
                                            format!("{sub}  ({m})")
                                        } else {
                                            sub.clone()
                                        };
                                        let sel = ed.selected.0 == cat.top && &ed.selected.1 == sub;
                                        if ui.selectable_label(sel, label).clicked() {
                                            ed.selected = (cat.top.clone(), sub.clone());
                                        }
                                    }
                                });
                        }
                    });
            });
        ui.add_space(4.0);
    }

    // Slider list.
    let ids: Vec<String> = if searching {
        ed.search_results()
            .into_iter()
            .map(|p| p.id.clone())
            .collect()
    } else {
        let (t, s) = ed.selected.clone();
        ed.params_in(&t, &s).map(|p| p.id.clone()).collect()
    };
    Frame::NONE
        .inner_margin(Margin::symmetric(8, 2))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                let title = if searching {
                    format!("{} results", ids.len())
                } else {
                    format!("{} › {}", ed.selected.0, ed.selected.1)
                };
                ui.label(RichText::new(title).strong());
                if !searching {
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.small_button("Reset group").clicked() {
                            let (t, s) = ed.selected.clone();
                            ed.reset_group(&t, &s);
                        }
                    });
                }
            });
        });
    egui::ScrollArea::vertical()
        .id_salt("sliders")
        .auto_shrink(false)
        .show(ui, |ui| {
            Frame::NONE
                .inner_margin(Margin::symmetric(8, 2))
                .show(ui, |ui| {
                    for id in ids {
                        let Some(p) = ed.asset.catalog.param(&id).cloned() else {
                            continue;
                        };
                        if ed.modified_only && !ed.is_modified(&p) {
                            continue;
                        }
                        slider_row(ui, ed, &p, searching);
                    }
                });
        });
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

fn accessories_tab(ui: &mut egui::Ui, ed: &mut Editor) {
    Frame::NONE.inner_margin(Margin::symmetric(8, 2)).show(ui, |ui| {
        if ed.accessories.is_empty() {
            ui.label(RichText::new("No accessories yet.").color(TEXT_DIM));
            ui.add_space(4.0);
            ui.label(
                RichText::new("Model props, hair or clothing in OxyBrush as new pieces, then save: they arrive here. They don't follow body changes yet (fitting comes in M5).")
                    .color(TEXT_DIM)
                    .size(11.5),
            );
            return;
        }
        let mut remove = None;
        for (i, a) in ed.accessories.iter_mut().enumerate() {
            Frame::NONE.fill(BG).corner_radius(CornerRadius::same(5)).inner_margin(Margin::same(6)).show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.horizontal(|ui| {
                    if ui.checkbox(&mut a.visible, "").on_hover_text("Show / hide").changed() {
                        ed.accessories_dirty = true;
                    }
                    ui.label(RichText::new(&a.acc.name).strong());
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.small_button("🗑").on_hover_text("Remove").clicked() {
                            remove = Some(i);
                        }
                    });
                });
                ui.label(RichText::new(format!("{} triangles{}", a.acc.triangles.len(), if a.acc.color.is_empty() { "" } else { " · painted" })).size(11.0).color(TEXT_DIM));
            });
            ui.add_space(4.0);
        }
        if let Some(i) = remove {
            let a = ed.accessories.remove(i);
            ed.status = format!("Removed accessory “{}”", a.acc.name);
            ed.accessories_dirty = true;
        }
    });
}

fn bridge_windows(ctx: &egui::Context, ed: &mut Editor) {
    if ed.bridge.open_dialog {
        let mut open = true;
        let mut go = false;
        egui::Window::new("Import OxyBrush file")
            .collapsible(false)
            .resizable(false)
            .open(&mut open)
            .show(ctx, |ui| {
                ui.label("Path to an OxyBrush project (.oxb):");
                let r = ui.add(
                    egui::TextEdit::singleline(&mut ed.bridge.path_input)
                        .desired_width(420.0)
                        .hint_text("/path/to/sculpt.oxb"),
                );
                go = r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
                ui.label(
                    RichText::new("Tip: you can also drop an .oxb file onto the window.")
                        .size(11.0)
                        .color(TEXT_DIM),
                );
                ui.horizontal(|ui| {
                    go |= ui
                        .add_enabled(
                            !ed.bridge.path_input.trim().is_empty(),
                            egui::Button::new("Open"),
                        )
                        .clicked();
                });
            });
        if go {
            let p = std::path::PathBuf::from(ed.bridge.path_input.trim());
            ed.open_oxb(&p);
            open = false;
        }
        ed.bridge.open_dialog = open;
    }

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

fn pose_panel(
    ui: &mut egui::Ui,
    ed: &mut Editor,
    export: &mut oxy_io::export::ExportOptions,
    export_requested: &mut bool,
) {
    use oxy_io::export::BindPose;
    Frame::NONE.inner_margin(Margin::symmetric(8, 6)).show(ui, |ui| {
        let Some(rig) = &ed.asset.rig else {
            ui.label(RichText::new("This character has no skeleton. Rebuild the bundle with tools/fetch-assets.sh.").color(TEXT_DIM));
            return;
        };
        ui.label(RichText::new(format!("{} · {} bones", rig.name, rig.bones.len())).size(11.5).color(TEXT_DIM));
        ui.add_space(6.0);
        ui.label(RichText::new("Preview pose").strong());
        ui.label(RichText::new("The skeleton refits to every morph; poses only affect the viewport unless exported as the bind pose.").size(11.0).color(TEXT_DIM));
        ui.add_space(4.0);
        let mut chosen = None;
        ui.horizontal_wrapped(|ui| {
            for (i, p) in oxy_rig::pose::PRESETS.iter().enumerate() {
                if ui.add(egui::Button::selectable(ed.pose == i, p.name).min_size(Vec2::new(96.0, 26.0))).clicked() {
                    chosen = Some(i);
                }
            }
        });
        if let Some(i) = chosen {
            ed.set_pose(i);
        }
        ui.add_space(4.0);
        ui.checkbox(&mut ed.show_skeleton, "Show skeleton");

        ui.add_space(12.0);
        ui.separator();
        ui.label(RichText::new("Export (.glb)").strong());
        ui.add_space(2.0);
        ui.checkbox(&mut export.skeleton, "Skeleton and skin weights");
        ui.add_enabled_ui(export.skeleton, |ui| {
            ui.horizontal(|ui| {
                ui.label("Bind pose");
                ui.selectable_value(&mut export.bind_pose, BindPose::Rest, "Rest (A-pose)");
                ui.selectable_value(&mut export.bind_pose, BindPose::TPose, "T-pose");
            });
        });
        ui.checkbox(&mut export.blendshapes, "Sculpted morphs as blendshapes")
            .on_hover_text("Off: sculpts are baked into the mesh. On: each sculpt becomes a named morph target at its current weight.");
        let accessories = ed.accessories.iter().filter(|a| a.visible).count();
        ui.label(
            RichText::new(format!("Includes the body{}.", if accessories > 0 { format!(" and {accessories} accessory mesh(es), skinned to the nearest body vertex") } else { String::new() }))
                .size(11.0)
                .color(TEXT_DIM),
        );
        ui.add_space(6.0);
        if ui.add(egui::Button::new("⬇ Export .glb").min_size(Vec2::new(140.0, 28.0))).clicked() {
            *export_requested = true;
        }
    });
}
