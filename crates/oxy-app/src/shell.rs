//! Editor chrome for the Oxy family layout.
//!
//! Feature widgets stay in [`crate::ui`]: shelf pages ([`crate::ui::draw_shelf`]),
//! morph lists and sliders, material sections, pose and export controls, and
//! the file / OxyBrush dialogs. This file only places them.
//!
//! To add a shelf page (clothing, for example): extend [`crate::ui::LeftTab`]
//! and the match in [`crate::ui::draw_shelf`], then add a rail button in
//! [`rail`]. To add a mode, extend [`crate::ui::RightMode`] and the matches
//! in [`mode_switch`], [`tool_options`] and [`right_stack`].

use bevy::prelude::*;
use bevy_egui::{egui, EguiContexts};
use egui::{Color32, Margin, RichText, Stroke};

use crate::document::{After, PathDialog};
use crate::editor::Editor;
use crate::scene::{CharacterHeight, Orbit, ViewportRect};
use crate::theme::{self, Icon, ACCENT, BAR, BORDER, CANVAS, RAIL, TEXT, TEXT_DIM, TOOL_BAR};
use crate::ui::{self, LeftTab, MaterialSection, RightMode, UiState};

#[allow(clippy::too_many_arguments)]
pub fn ui_system(
    mut contexts: EguiContexts,
    mut editor: ResMut<Editor>,
    mut state: ResMut<UiState>,
    mut rect_res: ResMut<ViewportRect>,
    mut orbit: ResMut<Orbit>,
    height: Res<CharacterHeight>,
    time: Res<Time>,
) -> Result {
    let ctx = contexts.ctx_mut()?.clone();
    if !state.styled {
        theme::install(&ctx);
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
    ui::shortcuts(&ctx, ed);
    let dirty = ed.has_unsaved_changes();

    top_bar(&mut root, ed, &mut state, &mut orbit, height.0, dirty);
    tool_options(
        &mut root,
        ed,
        &mut state,
        &mut orbit,
        height.0,
        previous_hover.as_deref(),
    );
    status_bar(&mut root, ed, &state, height.0, &time);
    if !state.shelf_collapsed {
        shelf(&mut root, ed, &mut state);
    }
    rail(&mut root, ed, &mut state, &mut orbit, height.0);
    right_stack(&mut root, ed, &mut state);
    let viewport = viewport(&mut root, &orbit);
    rect_res.min = bevy::math::Vec2::new(viewport.min.x, viewport.min.y);
    rect_res.max = bevy::math::Vec2::new(viewport.max.x, viewport.max.y);

    ui::bridge_windows(&ctx, ed);
    ui::document_windows(&ctx, ed);
    ui::service_dialogs(&ctx, ed, &mut state);
    help_window(&ctx, &mut state.help_open);
    about_window(&ctx, ed, &mut state.about_open);

    state.pointer_busy = ctx.egui_is_using_pointer();
    if ed.hovered.is_none() {
        ed.hovered = ed.pinned_hover.clone();
    }
    if ed.hovered != previous_hover {
        ed.highlight_dirty = true;
    }
    Ok(())
}

fn top_bar(
    root: &mut egui::Ui,
    ed: &mut Editor,
    state: &mut UiState,
    orbit: &mut Orbit,
    height: f32,
    dirty: bool,
) {
    egui::Panel::top("menu")
        .show_separator_line(false)
        .frame(theme::bar_frame(BAR))
        .show(root, |ui| {
            let bar_h = 32.0;
            let (full, _) = ui.allocate_exact_size(
                egui::vec2(ui.available_width(), bar_h),
                egui::Sense::empty(),
            );
            let center_w = 86.0 * RightMode::ALL.len() as f32;
            let center = egui::Rect::from_center_size(full.center(), egui::vec2(center_w, bar_h));
            let left =
                egui::Rect::from_min_max(full.min, egui::pos2(center.min.x - 8.0, full.max.y));
            let right =
                egui::Rect::from_min_max(egui::pos2(center.max.x + 8.0, full.min.y), full.max);
            let mut left_ui = ui.new_child(
                egui::UiBuilder::new()
                    .max_rect(left)
                    .layout(egui::Layout::left_to_right(egui::Align::Center)),
            );
            brand(&mut left_ui, dirty);
            left_ui.separator();
            menus(&mut left_ui, ed, state, orbit, height);
            let mut mid = ui.new_child(
                egui::UiBuilder::new()
                    .max_rect(center)
                    .layout(egui::Layout::left_to_right(egui::Align::Center)),
            );
            mode_switch(&mut mid, state);
            trailing_tools(ui, right, ed, state);
            hairline(ui, BORDER);
        });
}

/// Brand cluster: mark, wordmark, unsaved dot.
fn brand(ui: &mut egui::Ui, dirty: bool) {
    let (rect, _) = ui.allocate_exact_size(egui::vec2(22.0, 22.0), egui::Sense::hover());
    theme::brand_mark(ui.painter(), rect);
    ui.label(
        RichText::new("OxyCharacter")
            .strong()
            .size(14.0)
            .color(TEXT),
    );
    if dirty {
        let (dot, resp) = ui.allocate_exact_size(egui::vec2(10.0, 16.0), egui::Sense::hover());
        ui.painter().circle_filled(dot.center(), 3.0, ACCENT);
        resp.on_hover_text("Unsaved changes");
    }
}

fn trailing_tools(ui: &mut egui::Ui, rect: egui::Rect, ed: &mut Editor, state: &mut UiState) {
    let mut tools = ui.new_child(
        egui::UiBuilder::new()
            .max_rect(rect)
            .layout(egui::Layout::right_to_left(egui::Align::Center)),
    );
    // right_to_left: the first widget is the rightmost.
    if avatar(&mut tools).clicked() {
        state.about_open = true;
    }
    if theme::icon_button(&mut tools, Icon::Send, "Send to OxyBrush", false, true).clicked() {
        ed.send_to_oxybrush();
    }
    if theme::icon_button(
        &mut tools,
        Icon::Import,
        "Import an OxyBrush project (.oxb)",
        false,
        true,
    )
    .clicked()
    {
        ui::request_dialog(ed, PathDialog::ImportOxb);
    }
    let can_redo = ed.session().history.redo_label().is_some();
    let can_undo = ed.session().history.undo_label().is_some();
    let redo_tip = ed
        .session()
        .history
        .redo_label()
        .map_or_else(|| "Redo".to_string(), |l| format!("Redo {l}"));
    let undo_tip = ed
        .session()
        .history
        .undo_label()
        .map_or_else(|| "Undo".to_string(), |l| format!("Undo {l}"));
    if theme::icon_button(&mut tools, Icon::Redo, redo_tip, false, can_redo).clicked() {
        ed.redo();
    }
    if theme::icon_button(&mut tools, Icon::Undo, undo_tip, false, can_undo).clicked() {
        ed.undo();
    }
    let before = ed.search.clone();
    let _ = theme::search_field(&mut tools, &mut ed.search, "Search morphs…", 220.0);
    if ed.search != before && !ed.search.trim().is_empty() {
        state.right_mode = RightMode::Morph;
    }
}

/// Round mark in the avatar slot. It opens About — there is no account system.
fn avatar(ui: &mut egui::Ui) -> egui::Response {
    let (rect, resp) = ui.allocate_exact_size(egui::vec2(28.0, 28.0), egui::Sense::click());
    ui.painter().circle_filled(rect.center(), 13.0, ACCENT);
    theme::paint_icon(ui.painter(), rect.shrink(7.0), Icon::Head, Color32::WHITE);
    resp.on_hover_text("About OxyCharacter")
}

fn menus(ui: &mut egui::Ui, ed: &mut Editor, state: &mut UiState, orbit: &mut Orbit, height: f32) {
    egui::MenuBar::new().ui(ui, |ui| {
        file_menu(ui, ed);
        edit_menu(ui, ed);
        mode_menu(ui, state);
        window_menu(ui, state);
        viewport_menu(ui, ed, orbit, height);
        help_menu(ui, state);
    });
}

fn file_menu(ui: &mut egui::Ui, ed: &mut Editor) {
    egui::containers::menu::MenuButton::new("File").ui(ui, |ui| {
        if ui
            .add(egui::Button::new("New").shortcut_text("Ctrl+N"))
            .clicked()
        {
            ed.request(After::New);
        }
        if ui
            .add(egui::Button::new("Open…").shortcut_text("Ctrl+O"))
            .clicked()
        {
            ui::request_dialog(ed, PathDialog::Open);
        }
        let recent = ed.session().recent.clone();
        ui.add_enabled_ui(!recent.is_empty(), |ui| {
            egui::containers::menu::MenuButton::new("Open recent").ui(ui, |ui| {
                for p in recent {
                    if ui.button(p.display().to_string()).clicked() {
                        ed.request(After::Open(p));
                    }
                }
            });
        });
        if ui
            .add(egui::Button::new("Save").shortcut_text("Ctrl+S"))
            .clicked()
        {
            ed.save_or_ask();
        }
        if ui
            .add(egui::Button::new("Save as…").shortcut_text("Ctrl+Shift+S"))
            .clicked()
        {
            ui::request_dialog(ed, PathDialog::SaveAs);
        }
        ui.separator();
        if ui.button("Send to OxyBrush").clicked() {
            ed.send_to_oxybrush();
        }
        if ui.button("Import OxyBrush file (.oxb)…").clicked() {
            ui::request_dialog(ed, PathDialog::ImportOxb);
        }
        ui.separator();
        if ui.button("Export glTF (.glb)…").clicked() {
            ui::request_dialog(ed, PathDialog::Export);
        }
        ui.separator();
        if ui.button("Quit").clicked() {
            ed.request(After::Quit);
        }
    });
}

fn edit_menu(ui: &mut egui::Ui, ed: &mut Editor) {
    egui::containers::menu::MenuButton::new("Edit").ui(ui, |ui| {
        let undo = ed
            .session()
            .history
            .undo_label()
            .map(|l| format!("Undo {l}"));
        let redo = ed
            .session()
            .history
            .redo_label()
            .map(|l| format!("Redo {l}"));
        if ui
            .add_enabled(
                undo.is_some(),
                egui::Button::new(undo.unwrap_or_else(|| "Undo".into())).shortcut_text("Ctrl+Z"),
            )
            .clicked()
        {
            ed.undo();
        }
        if ui
            .add_enabled(
                redo.is_some(),
                egui::Button::new(redo.unwrap_or_else(|| "Redo".into()))
                    .shortcut_text("Ctrl+Shift+Z"),
            )
            .clicked()
        {
            ed.redo();
        }
        ui.separator();
        if ui.button("Reset all morphs").clicked() {
            ed.reset_all();
        }
        if ui.button("Randomize").clicked() {
            ed.randomize();
        }
        ui.checkbox(&mut ed.symmetry, "Symmetric editing");
        ui.checkbox(&mut ed.modified_only, "Show modified only");
    });
}

fn mode_menu(ui: &mut egui::Ui, state: &mut UiState) {
    egui::containers::menu::MenuButton::new("Mode").ui(ui, |ui| {
        for mode in RightMode::ALL {
            if ui
                .selectable_label(state.right_mode == mode, mode.label())
                .clicked()
            {
                state.right_mode = mode;
            }
        }
    });
}

fn window_menu(ui: &mut egui::Ui, state: &mut UiState) {
    egui::containers::menu::MenuButton::new("Window").ui(ui, |ui| {
        let mut shown = !state.shelf_collapsed;
        if ui.checkbox(&mut shown, "Shelf").changed() {
            state.shelf_collapsed = !shown;
        }
    });
}

fn viewport_menu(ui: &mut egui::Ui, ed: &mut Editor, orbit: &mut Orbit, height: f32) {
    egui::containers::menu::MenuButton::new("Viewport").ui(ui, |ui| {
        if ui.button("Frame full body").clicked() {
            *orbit = Orbit::full_body(height);
        }
        if ui.button("Frame head").clicked() {
            *orbit = Orbit::head(height);
        }
        ui.add_enabled_ui(ed.asset.rig.is_some(), |ui| {
            ui.checkbox(&mut ed.show_skeleton, "Show skeleton");
        });
        ui.separator();
        ui.label(RichText::new("LMB orbit").size(11.0).color(TEXT_DIM));
        ui.label(RichText::new("RMB or MMB pan").size(11.0).color(TEXT_DIM));
        ui.label(RichText::new("Wheel zoom").size(11.0).color(TEXT_DIM));
    });
}

fn help_menu(ui: &mut egui::Ui, state: &mut UiState) {
    egui::containers::menu::MenuButton::new("Help").ui(ui, |ui| {
        if ui.button("Keyboard shortcuts").clicked() {
            state.help_open = true;
        }
        if ui.button("About OxyCharacter").clicked() {
            state.about_open = true;
        }
    });
}

fn mode_switch(ui: &mut egui::Ui, state: &mut UiState) {
    let labels: Vec<&str> = RightMode::ALL.iter().map(|m| m.label()).collect();
    let current = RightMode::ALL
        .iter()
        .position(|m| *m == state.right_mode)
        .unwrap_or(0);
    if let Some(i) = theme::segmented(ui, "mode", &labels, current) {
        state.right_mode = RightMode::ALL[i];
    }
}

fn tool_options(
    root: &mut egui::Ui,
    ed: &mut Editor,
    state: &mut UiState,
    orbit: &mut Orbit,
    height: f32,
    focus: Option<&str>,
) {
    egui::Panel::top("tools")
        .show_separator_line(false)
        .frame(theme::bar_frame(TOOL_BAR))
        .show(root, |ui| {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 6.0;
                match state.right_mode {
                    RightMode::Morph => morph_tools(ui, ed, focus),
                    RightMode::Material => material_tools(ui, ed),
                    RightMode::Pose => pose_tools(ui, ed, orbit, height),
                    RightMode::Export => export_tools(ui, ed),
                }
            });
            hairline(ui, BORDER);
        });
}

fn morph_tools(ui: &mut egui::Ui, ed: &mut Editor, focus: Option<&str>) {
    ui.label(RichText::new("Mirror").size(11.5).color(TEXT_DIM));
    if theme::chip(ui, "X", ed.symmetry, true).clicked() {
        ed.symmetry = !ed.symmetry;
    }
    theme::chip(ui, "Y", false, false).on_hover_text("Only the left/right axis (X) is mirrored.");
    theme::chip(ui, "Z", false, false).on_hover_text("Only the left/right axis (X) is mirrored.");
    ui.separator();
    if theme::chip(ui, "Modified", ed.modified_only, true).clicked() {
        ed.modified_only = !ed.modified_only;
    }
    ui.separator();
    ui.label(
        RichText::new(format!("{}  ·  {}", ed.selected.0, ed.selected.1))
            .size(12.0)
            .color(TEXT),
    );
    if let Some(id) = focus {
        ui.separator();
        ui::compact_param(ui, ed, id);
    }
    ui.separator();
    if ui.button("Reset group").clicked() {
        let (t, s) = ed.selected.clone();
        ed.reset_group(&t, &s);
    }
}

fn material_tools(ui: &mut egui::Ui, ed: &mut Editor) {
    labeled_unit(ui, "Variation", &mut ed.material.skin.variation, 0.0..=1.0);
    labeled_unit(ui, "Detail", &mut ed.material.skin.detail, 0.0..=1.0);
    labeled_unit(
        ui,
        "Roughness",
        &mut ed.material.skin.roughness,
        0.25..=0.85,
    );
    if ui
        .button("New pattern")
        .on_hover_text("Re-seed the procedural variation")
        .clicked()
    {
        let seed = ed.material.skin.seed.wrapping_add(1);
        ed.material.skin.seed = seed;
    }
}

fn pose_tools(ui: &mut egui::Ui, ed: &mut Editor, orbit: &mut Orbit, height: f32) {
    let names: Vec<&str> = oxy_rig::pose::PRESETS.iter().map(|p| p.name).collect();
    let current = names.get(ed.pose).copied().unwrap_or("Pose");
    egui::ComboBox::from_id_salt("pose-quick")
        .selected_text(current)
        .width(140.0)
        .show_ui(ui, |ui| {
            for (i, name) in names.iter().enumerate() {
                if ui.selectable_label(ed.pose == i, *name).clicked() {
                    ed.set_pose(i);
                }
            }
        });
    ui.checkbox(&mut ed.show_skeleton, "Skeleton");
    ui.separator();
    if ui.button("Frame body").clicked() {
        *orbit = Orbit::full_body(height);
    }
    if ui.button("Frame head").clicked() {
        *orbit = Orbit::head(height);
    }
}

fn export_tools(ui: &mut egui::Ui, ed: &mut Editor) {
    use oxy_io::export::BindPose;
    ui.label(RichText::new("Bind").size(11.5).color(TEXT_DIM));
    let rest = ed.export.bind_pose == BindPose::Rest;
    if theme::chip(ui, "Rest", rest, ed.export.skeleton).clicked() {
        ed.export.bind_pose = BindPose::Rest;
    }
    if theme::chip(ui, "T-pose", !rest, ed.export.skeleton).clicked() {
        ed.export.bind_pose = BindPose::TPose;
    }
    ui.separator();
    ui.label(RichText::new("Textures").size(11.5).color(TEXT_DIM));
    egui::ComboBox::from_id_salt("tex-size")
        .selected_text(format!("{}", ed.export.texture_size))
        .width(72.0)
        .show_ui(ui, |ui| {
            for size in [1024u32, 2048, 4096] {
                ui.selectable_value(&mut ed.export.texture_size, size, size.to_string());
            }
        });
    if ui.button("Export .glb…").clicked() {
        ui::request_dialog(ed, PathDialog::Export);
    }
}

fn labeled_unit(
    ui: &mut egui::Ui,
    label: &str,
    value: &mut f32,
    range: std::ops::RangeInclusive<f32>,
) {
    ui.label(RichText::new(label).size(11.5).color(TEXT_DIM));
    ui.spacing_mut().slider_width = 88.0;
    ui.add(
        egui::Slider::new(value, range.clone())
            .show_value(false)
            .trailing_fill(true),
    );
    ui.add(
        egui::DragValue::new(value)
            .range(range)
            .speed(0.005)
            .fixed_decimals(2),
    );
}

fn status_bar(root: &mut egui::Ui, ed: &Editor, state: &UiState, height: f32, time: &Time) {
    let fps = 1.0 / time.delta_secs().max(1e-4);
    let verts = ed.asset.mesh.base.vertex_count();
    let tris = ed.asset.mesh.base.indices.len() / 3;
    let stats = format!(
        "Body · {} verts · {} tris · {:.2} m · {} morphs · {:.0} FPS",
        compact_count(verts),
        compact_count(tris),
        height,
        ed.active_targets,
        fps
    );
    let linked = ed.bridge.watched.is_some();
    let link_tip = if let Some((p, _)) = &ed.bridge.watched {
        format!("Watching {} for OxyBrush saves", p.display())
    } else {
        "Not linked. File → Send to OxyBrush writes a project and watches it.".into()
    };
    egui::Panel::bottom("status")
        .show_separator_line(false)
        .frame(theme::bar_frame(BAR).inner_margin(Margin::symmetric(10, 3)))
        .show(root, |ui| {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 10.0;
                ui.label(RichText::new(stats).size(11.0).color(TEXT_DIM))
                    .on_hover_text(format!(
                        "LMB orbit · RMB or MMB pan · wheel zoom\nBase mesh: {} · {}",
                        ed.asset.meta.name, ed.asset.meta.license
                    ));
                let reserve = 210.0;
                let msg_w = (ui.available_width() - reserve).max(16.0);
                ui.add_sized(
                    [msg_w, 16.0],
                    egui::Label::new(RichText::new(&ed.status).size(11.0).color(TEXT_DIM))
                        .truncate(),
                );
                theme::status_dot(ui, linked);
                ui.label(
                    RichText::new(if linked { "Connected" } else { "Idle" })
                        .size(11.0)
                        .color(if linked { TEXT } else { TEXT_DIM }),
                )
                .on_hover_text(link_tip);
                ui.label(
                    RichText::new(state.right_mode.label())
                        .size(11.0)
                        .color(TEXT),
                );
            });
        });
}

fn compact_count(n: usize) -> String {
    if n >= 10_000 {
        format!("{:.1}k", n as f32 / 1000.0)
    } else {
        n.to_string()
    }
}

fn shelf(root: &mut egui::Ui, ed: &mut Editor, state: &mut UiState) {
    egui::Panel::bottom("shelf")
        .resizable(true)
        .default_size(theme::SHELF_HEIGHT)
        .size_range(150.0..=320.0)
        .show_separator_line(false)
        .frame(
            egui::Frame::NONE
                .fill(CANVAS)
                .inner_margin(Margin::symmetric(8, 6)),
        )
        .show(root, |ui| {
            ui.horizontal(|ui| {
                ui.label(RichText::new("Shelf").strong().size(12.0).color(TEXT));
                ui.add_space(8.0);
                for (label, tab) in [
                    ("Presets", LeftTab::Presets),
                    ("Skin", LeftTab::Skin),
                    ("Wardrobe", LeftTab::Wardrobe),
                    ("Accessories", LeftTab::Accessories),
                ] {
                    let n = match tab {
                        LeftTab::Accessories => ed.accessories.len(),
                        LeftTab::Wardrobe => ed.worn.iter().filter(|w| w.settings.visible).count(),
                        _ => 0,
                    };
                    let text = if n > 0 {
                        format!("{label} ({n})")
                    } else {
                        label.to_string()
                    };
                    let mut resp = theme::chip(ui, &text, state.left_tab == tab, true);
                    if tab == LeftTab::Skin {
                        resp = resp.on_hover_text(
                            "Layered skin (freckles, blush, lips, eyes) is edited in Material mode.",
                        );
                    }
                    if resp.clicked() {
                        state.left_tab = tab;
                    }
                }
            });
            ui.add_space(4.0);
            ui::draw_shelf(
                ui,
                ed,
                state.left_tab,
                &mut state.preset_filter,
                &mut state.package_path,
                &mut state.wardrobe_focus,
                &mut state.accessory_focus,
            );
        });
}

fn rail(root: &mut egui::Ui, ed: &mut Editor, state: &mut UiState, orbit: &mut Orbit, height: f32) {
    egui::Panel::left("rail")
        .exact_size(theme::RAIL_WIDTH)
        .resizable(false)
        .show_separator_line(false)
        .frame(
            egui::Frame::NONE
                .fill(RAIL)
                .inner_margin(Margin::symmetric(0, 8)),
        )
        .show(root, |ui| {
            ui.with_layout(egui::Layout::top_down(egui::Align::Center), |ui| {
                ui.spacing_mut().item_spacing.y = 4.0;
                if theme::icon_button(
                    ui,
                    Icon::Preset,
                    "Body presets",
                    state.left_tab == LeftTab::Presets,
                    true,
                )
                .clicked()
                {
                    state.left_tab = LeftTab::Presets;
                    state.shelf_collapsed = false;
                }
                if theme::icon_button(
                    ui,
                    Icon::Skin,
                    "Skin tones",
                    state.left_tab == LeftTab::Skin,
                    true,
                )
                .clicked()
                {
                    state.left_tab = LeftTab::Skin;
                    state.shelf_collapsed = false;
                }
                let worn = ed.worn.iter().filter(|w| w.settings.visible).count();
                let wardrobe_tip = if worn == 0 {
                    "Wardrobe".to_string()
                } else {
                    format!("Wardrobe ({worn})")
                };
                if theme::icon_button(
                    ui,
                    Icon::Wardrobe,
                    wardrobe_tip,
                    state.left_tab == LeftTab::Wardrobe,
                    true,
                )
                .clicked()
                {
                    state.left_tab = LeftTab::Wardrobe;
                    state.shelf_collapsed = false;
                }
                let tip = if ed.accessories.is_empty() {
                    "Accessories".to_string()
                } else {
                    format!("Accessories ({})", ed.accessories.len())
                };
                if theme::icon_button(
                    ui,
                    Icon::Accessory,
                    tip,
                    state.left_tab == LeftTab::Accessories,
                    true,
                )
                .clicked()
                {
                    state.left_tab = LeftTab::Accessories;
                    state.shelf_collapsed = false;
                }
                ui.separator();
                if theme::icon_button(ui, Icon::Frame, "Frame the full body", false, true).clicked()
                {
                    *orbit = Orbit::full_body(height);
                }
                if theme::icon_button(ui, Icon::Head, "Frame the head", false, true).clicked() {
                    *orbit = Orbit::head(height);
                }
                if theme::icon_button(
                    ui,
                    Icon::Bone,
                    "Show skeleton",
                    ed.show_skeleton,
                    ed.asset.rig.is_some(),
                )
                .clicked()
                {
                    ed.show_skeleton = !ed.show_skeleton;
                }
                if theme::icon_button(ui, Icon::Symmetry, "Symmetric editing", ed.symmetry, true)
                    .clicked()
                {
                    ed.symmetry = !ed.symmetry;
                }
                ui.separator();
                if theme::icon_button(ui, Icon::Reset, "Reset all morphs", false, true).clicked() {
                    ed.reset_all();
                }
                if theme::icon_button(ui, Icon::Random, "Randomize the character", false, true)
                    .clicked()
                {
                    ed.randomize();
                }
            });
            let rect = ui.max_rect();
            ui.painter().line_segment(
                [rect.right_top(), rect.right_bottom()],
                Stroke::new(1.0, BORDER),
            );
        });
}

fn right_stack(root: &mut egui::Ui, ed: &mut Editor, state: &mut UiState) {
    egui::Panel::right("inspector")
        .resizable(true)
        .default_size(360.0)
        .size_range(300.0..=520.0)
        .show_separator_line(false)
        .frame(egui::Frame::NONE.fill(CANVAS).inner_margin(Margin::same(8)))
        .show(root, |ui| {
            let gap = theme::GAP;
            let avail = ui.available_height();
            let top_h = ((avail - gap) * 0.42).clamp(120.0, (avail - gap - 140.0).max(120.0));
            theme::card(ui, top_h, |ui| upper_card(ui, ed, state));
            ui.add_space(gap);
            let rest = ui.available_height();
            theme::card(ui, rest, |ui| lower_card(ui, ed, state));
        });
}

fn upper_card(ui: &mut egui::Ui, ed: &mut Editor, state: &mut UiState) {
    match state.right_mode {
        RightMode::Morph => {
            morph_tabs(ui, ed);
            ui.add_space(4.0);
            egui::ScrollArea::vertical()
                .id_salt("morph-subs")
                .auto_shrink([false, false])
                .show(ui, |ui| ui::morph_sub_list(ui, ed));
        }
        RightMode::Material => {
            material_tabs(ui, state);
            ui.add_space(4.0);
            egui::ScrollArea::vertical()
                .id_salt("mat-browse")
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    ui::material_browser(ui, ed, state.material_section)
                });
        }
        RightMode::Pose => {
            ui.label(RichText::new("Poses").strong().size(12.5));
            ui.add_space(4.0);
            egui::ScrollArea::vertical()
                .id_salt("poses")
                .auto_shrink([false, false])
                .show(ui, |ui| ui::pose_list(ui, ed));
        }
        RightMode::Export => {
            ui.label(RichText::new("Export").strong().size(12.5));
            ui.add_space(4.0);
            egui::ScrollArea::vertical()
                .id_salt("export-sum")
                .auto_shrink([false, false])
                .show(ui, |ui| ui::export_summary(ui, ed));
        }
    }
}

fn lower_card(ui: &mut egui::Ui, ed: &mut Editor, state: &mut UiState) {
    ui.horizontal(|ui| {
        ui.label(RichText::new("Properties").strong().size(12.5));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.label(
                RichText::new(state.right_mode.label())
                    .size(11.0)
                    .color(TEXT_DIM),
            );
        });
    });
    ui.add_space(2.0);
    match state.right_mode {
        RightMode::Morph => {
            let searching = !ed.search.trim().is_empty();
            ui.horizontal(|ui| {
                let title = if searching {
                    "Search results".to_string()
                } else {
                    format!("{}  ·  {}", ed.selected.0, ed.selected.1)
                };
                ui.label(RichText::new(title).size(11.5).color(TEXT_DIM));
                if !searching {
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.small_button("Reset group").clicked() {
                            let (t, s) = ed.selected.clone();
                            ed.reset_group(&t, &s);
                        }
                    });
                }
            });
            egui::ScrollArea::vertical()
                .id_salt("morph-sliders")
                .auto_shrink([false, false])
                .show(ui, |ui| ui::morph_sliders(ui, ed));
        }
        RightMode::Material => {
            egui::ScrollArea::vertical()
                .id_salt("mat-props")
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    ui::material_properties(ui, ed, state.material_section)
                });
        }
        RightMode::Pose => {
            egui::ScrollArea::vertical()
                .id_salt("pose-props")
                .auto_shrink([false, false])
                .show(ui, |ui| ui::pose_properties(ui, ed));
        }
        RightMode::Export => {
            egui::ScrollArea::vertical()
                .id_salt("export-props")
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    ui::export_controls(ui, ed);
                });
        }
    }
}

fn morph_tabs(ui: &mut egui::Ui, ed: &mut Editor) {
    if !ed.search.trim().is_empty() {
        ui.label(RichText::new("Search").strong().size(12.5));
        return;
    }
    let tops: Vec<String> = ed.categories.iter().map(|c| c.top.clone()).collect();
    if tops.is_empty() {
        ui.label(RichText::new("No morph categories").color(TEXT_DIM));
        return;
    }
    let labels: Vec<&str> = tops.iter().map(String::as_str).collect();
    let current = tops.iter().position(|t| t == &ed.selected.0).unwrap_or(0);
    if let Some(i) = theme::text_tabs(ui, "morph-top", &labels, current) {
        let top = tops[i].clone();
        let sub = ed
            .categories
            .iter()
            .find(|c| c.top == top)
            .and_then(|c| {
                if ed.selected.0 == top && c.subs.iter().any(|s| s == &ed.selected.1) {
                    Some(ed.selected.1.clone())
                } else {
                    c.subs.first().cloned()
                }
            })
            .unwrap_or_default();
        ed.selected = (top, sub);
    }
}

fn material_tabs(ui: &mut egui::Ui, state: &mut UiState) {
    let labels = ["Skin", "Eyes", "Bake"];
    let current = match state.material_section {
        MaterialSection::Skin => 0,
        MaterialSection::Eyes => 1,
        MaterialSection::Bake => 2,
    };
    if let Some(i) = theme::text_tabs(ui, "mat-sec", &labels, current) {
        state.material_section = match i {
            0 => MaterialSection::Skin,
            1 => MaterialSection::Eyes,
            _ => MaterialSection::Bake,
        };
    }
}

fn viewport(root: &mut egui::Ui, orbit: &Orbit) -> egui::Rect {
    let central = egui::CentralPanel::default()
        .frame(egui::Frame::NONE)
        .show(root, |ui| {
            let rect = ui.max_rect();
            paint_vignette(ui.painter(), rect);
            paint_gizmo(ui.painter(), orbit, rect);
            rect
        });
    central.inner
}

fn paint_vignette(painter: &egui::Painter, rect: egui::Rect) {
    let depth = 110.0;
    let edge = Color32::from_black_alpha(165);
    let clear = Color32::TRANSPARENT;
    // Top
    shade(
        painter,
        egui::Rect::from_min_max(
            rect.left_top(),
            egui::pos2(rect.right(), rect.top() + depth),
        ),
        edge,
        edge,
        clear,
        clear,
    );
    // Bottom
    shade(
        painter,
        egui::Rect::from_min_max(
            egui::pos2(rect.left(), rect.bottom() - depth),
            rect.right_bottom(),
        ),
        clear,
        clear,
        edge,
        edge,
    );
    // Left
    shade(
        painter,
        egui::Rect::from_min_max(
            rect.left_top(),
            egui::pos2(rect.left() + depth, rect.bottom()),
        ),
        edge,
        clear,
        edge,
        clear,
    );
    // Right
    shade(
        painter,
        egui::Rect::from_min_max(
            egui::pos2(rect.right() - depth, rect.top()),
            rect.right_bottom(),
        ),
        clear,
        edge,
        clear,
        edge,
    );
}

fn shade(
    painter: &egui::Painter,
    rect: egui::Rect,
    tl: Color32,
    tr: Color32,
    bl: Color32,
    br: Color32,
) {
    if rect.width() <= 1.0 || rect.height() <= 1.0 {
        return;
    }
    let uv = egui::Pos2::ZERO;
    let mut mesh = egui::Mesh::default();
    mesh.vertices.extend([
        egui::epaint::Vertex {
            pos: rect.left_top(),
            uv,
            color: tl,
        },
        egui::epaint::Vertex {
            pos: rect.right_top(),
            uv,
            color: tr,
        },
        egui::epaint::Vertex {
            pos: rect.right_bottom(),
            uv,
            color: br,
        },
        egui::epaint::Vertex {
            pos: rect.left_bottom(),
            uv,
            color: bl,
        },
    ]);
    mesh.indices.extend_from_slice(&[0, 1, 2, 0, 2, 3]);
    painter.add(egui::Shape::mesh(mesh));
}

fn paint_gizmo(painter: &egui::Painter, orbit: &Orbit, rect: egui::Rect) {
    let center = rect.right_top() + egui::vec2(-46.0, 46.0);
    painter.circle_filled(center, 26.0, Color32::from_black_alpha(90));
    painter.circle_stroke(center, 26.0, Stroke::new(1.0, BORDER));
    let rot = Quat::from_euler(EulerRot::YXZ, orbit.yaw, -orbit.pitch, 0.0);
    let right = rot * Vec3::X;
    let up = rot * Vec3::Y;
    let axes = [
        (Vec3::X, Color32::from_rgb(226, 84, 74), "X"),
        (Vec3::Y, Color32::from_rgb(88, 196, 108), "Y"),
        (Vec3::Z, Color32::from_rgb(90, 150, 240), "Z"),
    ];
    let mut order = [0, 1, 2];
    let depth = |v: Vec3| v.dot(rot * Vec3::Z);
    order.sort_by(|a, b| {
        depth(axes[*a].0)
            .partial_cmp(&depth(axes[*b].0))
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    for i in order {
        let (dir, color, label) = axes[i];
        let end = center + egui::vec2(dir.dot(right), -dir.dot(up)) * 18.0;
        painter.line_segment([center, end], Stroke::new(2.0, color));
        painter.circle_filled(end, 2.4, color);
        let delta = end - center;
        let nudge = if delta.length_sq() > 4.0 {
            delta.normalized() * 10.0
        } else {
            egui::vec2(0.0, -12.0)
        };
        painter.text(
            end + nudge,
            egui::Align2::CENTER_CENTER,
            label,
            egui::FontId::proportional(10.0),
            color,
        );
    }
}

fn hairline(ui: &egui::Ui, color: Color32) {
    let r = ui.max_rect();
    ui.painter()
        .line_segment([r.left_bottom(), r.right_bottom()], Stroke::new(1.0, color));
}

fn help_window(ctx: &egui::Context, open: &mut bool) {
    if !*open {
        return;
    }
    egui::Window::new("Shortcuts")
        .collapsible(false)
        .resizable(false)
        .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
        .open(open)
        .show(ctx, |ui| {
            egui::Grid::new("shortcuts")
                .num_columns(2)
                .spacing([16.0, 6.0])
                .show(ui, |ui| {
                    for (key, action) in [
                        ("Ctrl+Z", "Undo"),
                        ("Ctrl+Shift+Z", "Redo"),
                        ("Ctrl+Y", "Redo"),
                        ("Ctrl+S", "Save"),
                        ("Ctrl+Shift+S", "Save as"),
                        ("Ctrl+O", "Open"),
                        ("Ctrl+N", "New"),
                    ] {
                        ui.label(RichText::new(key).color(ACCENT));
                        ui.label(action);
                        ui.end_row();
                    }
                });
            ui.add_space(6.0);
            ui.label(
                RichText::new("Double-click a slider to reset it.")
                    .size(11.0)
                    .color(TEXT_DIM),
            );
            ui.label(
                RichText::new("LMB orbit · RMB or MMB pan · wheel zoom")
                    .size(11.0)
                    .color(TEXT_DIM),
            );
        });
}

fn about_window(ctx: &egui::Context, ed: &Editor, open: &mut bool) {
    if !*open {
        return;
    }
    egui::Window::new("About OxyCharacter")
        .collapsible(false)
        .resizable(false)
        .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
        .open(open)
        .show(ctx, |ui| {
            ui.label(RichText::new("OxyCharacter").strong().size(16.0));
            ui.label(
                RichText::new(format!("Version {}", env!("CARGO_PKG_VERSION"))).color(TEXT_DIM),
            );
            ui.add_space(6.0);
            ui.label("Parametric character editor. Shape, pose, skin and export.");
            ui.add_space(6.0);
            ui.label(RichText::new("Base mesh").strong());
            ui.label(ed.asset.meta.name.as_str());
            ui.label(RichText::new(ed.asset.meta.license.as_str()).color(TEXT_DIM));
            ui.add_space(6.0);
            ui.label(RichText::new("Interface").strong());
            ui.label(
                RichText::new("Typeface: Roboto (Apache-2.0).")
                    .size(11.5)
                    .color(TEXT_DIM),
            );
            ui.label(
                RichText::new("Icons: original geometry in the theme module.")
                    .size(11.5)
                    .color(TEXT_DIM),
            );
            ui.label(
                RichText::new("Colors and spacing live in crates/oxy-app/src/theme.rs.")
                    .size(11.5)
                    .color(TEXT_DIM),
            );
        });
}
