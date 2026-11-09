//! Oxy family theme.
//!
//! Colors, spacing, corner radii and widget painters shared by the editor
//! chrome. The module depends only on egui — no Bevy, no document state — so
//! the same tokens can be copied into OxyBrush later (or extracted into a
//! tiny crate) without dragging the character document along.
//!
//! Icons are original geometry drawn in [`paint_icon`]. The proportional
//! typeface is Roboto (Apache-2.0), bundled under `assets/fonts/`. egui's
//! built-in monospace stays as the numeric face and as a glyph fallback.
//! See `assets/LICENSES.md`.

use std::sync::Arc;

use egui::{
    epaint::Shadow, Color32, CornerRadius, FontData, FontDefinitions, FontFamily, FontId, Frame,
    Margin, Pos2, Rect, Sense, Stroke, StrokeKind, TextStyle, Ui, Vec2, WidgetText,
};

/// Gap between floating cards, in logical pixels.
pub const GAP: f32 = 8.0;
/// Corner radius of panels and cards.
pub const RADIUS: u8 = 8;
/// Corner radius of buttons, fields and rows.
pub const RADIUS_SM: u8 = 6;
/// Width of the vertical tool rail.
pub const RAIL_WIDTH: f32 = 52.0;
/// Default height of the bottom shelf.
pub const SHELF_HEIGHT: f32 = 188.0;

// Measured from the Oxy family mockup: near-black chrome, neutral panels,
// a saturated blue for selection and slider fills.
const CANVAS_RGB: [u8; 3] = [14, 14, 16];
const BAR_RGB: [u8; 3] = [26, 26, 28];
const TOOL_RGB: [u8; 3] = [18, 18, 20];
const RAIL_RGB: [u8; 3] = [32, 34, 40];
const PANEL_RGB: [u8; 3] = [30, 30, 32];
const WIDGET_RGB: [u8; 3] = [42, 43, 48];
const WIDGET_HOVER_RGB: [u8; 3] = [54, 56, 64];
const BORDER_RGB: [u8; 3] = [54, 56, 62];
const TEXT_RGB: [u8; 3] = [236, 237, 240];
const TEXT_DIM_RGB: [u8; 3] = [150, 152, 158];
const TEXT_FAINT_RGB: [u8; 3] = [110, 112, 118];
const ACCENT_RGB: [u8; 3] = [20, 114, 226];
const GOOD_RGB: [u8; 3] = [46, 170, 78];
const ROW_RGB: [u8; 3] = [32, 58, 96];

/// Window background behind the panels (the gutters).
pub const CANVAS: Color32 = Color32::from_rgb(CANVAS_RGB[0], CANVAS_RGB[1], CANVAS_RGB[2]);
/// Top bar and status bar.
pub const BAR: Color32 = Color32::from_rgb(BAR_RGB[0], BAR_RGB[1], BAR_RGB[2]);
/// Contextual tool-options bar (a step darker than the menu bar).
pub const TOOL_BAR: Color32 = Color32::from_rgb(TOOL_RGB[0], TOOL_RGB[1], TOOL_RGB[2]);
/// Vertical icon rail.
pub const RAIL: Color32 = Color32::from_rgb(RAIL_RGB[0], RAIL_RGB[1], RAIL_RGB[2]);
/// Cards and dialogs.
pub const PANEL: Color32 = Color32::from_rgb(PANEL_RGB[0], PANEL_RGB[1], PANEL_RGB[2]);
/// Inputs, chips, thumbnail wells.
pub const WIDGET: Color32 = Color32::from_rgb(WIDGET_RGB[0], WIDGET_RGB[1], WIDGET_RGB[2]);
/// Hover fill for wells and rows.
pub const WIDGET_HOVER: Color32 = Color32::from_rgb(
    WIDGET_HOVER_RGB[0],
    WIDGET_HOVER_RGB[1],
    WIDGET_HOVER_RGB[2],
);
/// Hairline borders.
pub const BORDER: Color32 = Color32::from_rgb(BORDER_RGB[0], BORDER_RGB[1], BORDER_RGB[2]);
/// Primary text.
pub const TEXT: Color32 = Color32::from_rgb(TEXT_RGB[0], TEXT_RGB[1], TEXT_RGB[2]);
/// Secondary text.
pub const TEXT_DIM: Color32 = Color32::from_rgb(TEXT_DIM_RGB[0], TEXT_DIM_RGB[1], TEXT_DIM_RGB[2]);
/// Disabled and hint text.
pub const TEXT_FAINT: Color32 =
    Color32::from_rgb(TEXT_FAINT_RGB[0], TEXT_FAINT_RGB[1], TEXT_FAINT_RGB[2]);
/// Selection, slider fill, active pill.
pub const ACCENT: Color32 = Color32::from_rgb(ACCENT_RGB[0], ACCENT_RGB[1], ACCENT_RGB[2]);
/// "Connected" status.
pub const GOOD: Color32 = Color32::from_rgb(GOOD_RGB[0], GOOD_RGB[1], GOOD_RGB[2]);
/// Selected list row (muted accent, not a solid pill).
pub const ROW_SELECTED: Color32 = Color32::from_rgb(ROW_RGB[0], ROW_RGB[1], ROW_RGB[2]);

/// Clear color of the 3D viewport, sRGB 0–1. The egui vignette darkens the edges.
pub const VIEWPORT_BG_SRGB: [f32; 3] = [32.0 / 255.0, 40.0 / 255.0, 54.0 / 255.0];

const ROBOTO: &[u8] = include_bytes!("../../../assets/fonts/Roboto-Regular.ttf");

/// Install Roboto and the dark visuals. Call once, not every frame.
pub fn install(ctx: &egui::Context) {
    let mut fonts = FontDefinitions::default();
    fonts
        .font_data
        .insert("roboto".to_owned(), Arc::new(FontData::from_static(ROBOTO)));
    fonts
        .families
        .get_mut(&FontFamily::Proportional)
        .expect("proportional family")
        .insert(0, "roboto".to_owned());
    // Monospace stays egui's built-in face; Roboto covers glyphs it lacks.
    fonts
        .families
        .get_mut(&FontFamily::Monospace)
        .expect("monospace family")
        .push("roboto".to_owned());
    ctx.set_fonts(fonts);

    ctx.global_style_mut(|s| {
        s.text_styles.insert(
            TextStyle::Small,
            FontId::new(11.0, FontFamily::Proportional),
        );
        s.text_styles
            .insert(TextStyle::Body, FontId::new(13.0, FontFamily::Proportional));
        s.text_styles.insert(
            TextStyle::Button,
            FontId::new(13.0, FontFamily::Proportional),
        );
        s.text_styles.insert(
            TextStyle::Heading,
            FontId::new(15.0, FontFamily::Proportional),
        );
        s.text_styles.insert(
            TextStyle::Monospace,
            FontId::new(12.0, FontFamily::Monospace),
        );
        s.spacing.item_spacing = Vec2::new(8.0, 6.0);
        s.spacing.button_padding = Vec2::new(8.0, 3.0);
        s.spacing.slider_width = 140.0;
        s.spacing.interact_size = Vec2::new(36.0, 18.0);
        s.spacing.menu_margin = Margin::same(6);
        s.spacing.window_margin = Margin::same(12);
        // Thin rails match the mockup's sliders. Older egui builds lack the field;
        // set it only when the struct has it — it does on egui 0.36.
        s.spacing.slider_rail_height = 3.0;
    });
    ctx.set_visuals(visuals());
}

fn visuals() -> egui::Visuals {
    let mut v = egui::Visuals::dark();
    v.panel_fill = PANEL;
    v.window_fill = PANEL;
    v.extreme_bg_color = CANVAS;
    v.faint_bg_color = Color32::from_rgb(36, 37, 42);
    v.code_bg_color = CANVAS;
    v.widgets.noninteractive.bg_fill = PANEL;
    v.widgets.noninteractive.weak_bg_fill = Color32::TRANSPARENT;
    v.widgets.noninteractive.bg_stroke = Stroke::new(1.0, BORDER);
    v.widgets.noninteractive.fg_stroke = Stroke::new(1.0, TEXT);
    round(&mut v.widgets.noninteractive, RADIUS_SM);

    v.widgets.inactive.bg_fill = WIDGET;
    v.widgets.inactive.weak_bg_fill = Color32::TRANSPARENT;
    v.widgets.inactive.bg_stroke = Stroke::new(1.0, BORDER);
    v.widgets.inactive.fg_stroke = Stroke::new(1.0, TEXT);
    round(&mut v.widgets.inactive, RADIUS_SM);

    v.widgets.hovered.bg_fill = WIDGET_HOVER;
    v.widgets.hovered.weak_bg_fill = WIDGET_HOVER;
    v.widgets.hovered.bg_stroke = Stroke::new(1.0, ACCENT.linear_multiply(0.65));
    v.widgets.hovered.fg_stroke = Stroke::new(1.0, Color32::WHITE);
    round(&mut v.widgets.hovered, RADIUS_SM);

    v.widgets.active.bg_fill = ACCENT;
    v.widgets.active.weak_bg_fill = ACCENT;
    v.widgets.active.bg_stroke = Stroke::new(1.0, ACCENT);
    v.widgets.active.fg_stroke = Stroke::new(1.0, Color32::WHITE);
    round(&mut v.widgets.active, RADIUS_SM);

    v.widgets.open.bg_fill = WIDGET;
    v.widgets.open.weak_bg_fill = WIDGET;
    v.widgets.open.bg_stroke = Stroke::new(1.0, ACCENT);
    v.widgets.open.fg_stroke = Stroke::new(1.0, TEXT);
    round(&mut v.widgets.open, RADIUS_SM);

    v.selection.bg_fill = ACCENT;
    v.selection.stroke = Stroke::new(1.0, ACCENT);
    v.hyperlink_color = ACCENT;
    v.window_stroke = Stroke::new(1.0, BORDER);
    v.window_corner_radius = CornerRadius::same(RADIUS);
    v.menu_corner_radius = CornerRadius::same(RADIUS_SM);
    v.window_shadow = Shadow {
        offset: [0, 6],
        blur: 18,
        spread: 0,
        color: Color32::from_black_alpha(90),
    };
    v.popup_shadow = v.window_shadow;
    v.slider_trailing_fill = true;
    v
}

fn round(w: &mut egui::style::WidgetVisuals, radius: u8) {
    w.corner_radius = CornerRadius::same(radius);
}

/// Full-width bar used by the menu, tool options and status strips.
pub fn bar_frame(fill: Color32) -> Frame {
    Frame::NONE
        .fill(fill)
        .inner_margin(Margin::symmetric(10, 5))
}

/// Rounded inspector card. `height` is the outer size, including the border.
pub fn card<R>(ui: &mut Ui, height: f32, add: impl FnOnce(&mut Ui) -> R) -> R {
    let width = ui.available_width();
    let (rect, _) = ui.allocate_exact_size(Vec2::new(width, height), Sense::empty());
    ui.painter()
        .rect_filled(rect, CornerRadius::same(RADIUS), PANEL);
    ui.painter().rect_stroke(
        rect,
        CornerRadius::same(RADIUS),
        Stroke::new(1.0, BORDER),
        StrokeKind::Inside,
    );
    let inner = rect.shrink(8.0);
    let mut child = ui.new_child(
        egui::UiBuilder::new()
            .max_rect(inner)
            .layout(egui::Layout::top_down(egui::Align::Min)),
    );
    child.set_clip_rect(inner);
    add(&mut child)
}

/// Blue rounded-square mark with an original figure glyph (not a lettermark
/// taken from another product).
pub fn brand_mark(painter: &egui::Painter, rect: Rect) {
    painter.rect_filled(rect, CornerRadius::same(5), ACCENT);
    let color = Color32::WHITE;
    let c = rect.center();
    let s = rect.width().min(rect.height());
    painter.circle_filled(c + Vec2::new(0.0, -s * 0.16), s * 0.16, color);
    painter.add(egui::Shape::convex_polygon(
        vec![
            c + Vec2::new(-s * 0.28, s * 0.08),
            c + Vec2::new(s * 0.28, s * 0.08),
            c + Vec2::new(s * 0.16, s * 0.34),
            c + Vec2::new(-s * 0.16, s * 0.34),
        ],
        color,
        Stroke::NONE,
    ));
}

/// Original geometric icons. Drawn to fit `rect`; no third-party artwork.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Icon {
    Preset,
    Skin,
    Accessory,
    Reset,
    Random,
    Symmetry,
    Bone,
    Send,
    Import,
    Undo,
    Redo,
    Search,
    Eye,
    EyeOff,
    Trash,
    /// Four corner marks: frame the current view.
    Frame,
    /// Head, used to frame the head.
    Head,
    /// Shirt: wardrobe / clothing shelf.
    Wardrobe,
}

pub fn paint_icon(painter: &egui::Painter, rect: Rect, icon: Icon, color: Color32) {
    let stroke = Stroke::new(1.6, color);
    let c = rect.center();
    let s = rect.width().min(rect.height());
    match icon {
        Icon::Preset => {
            painter.circle_stroke(c + Vec2::new(0.0, -s * 0.22), s * 0.18, stroke);
            painter.line_segment(
                [
                    c + Vec2::new(-s * 0.28, s * 0.34),
                    c + Vec2::new(0.0, s * 0.02),
                ],
                stroke,
            );
            painter.line_segment(
                [
                    c + Vec2::new(s * 0.28, s * 0.34),
                    c + Vec2::new(0.0, s * 0.02),
                ],
                stroke,
            );
        }
        Icon::Skin => {
            painter.circle_filled(c, s * 0.32, color);
            painter.circle_filled(c + Vec2::new(-s * 0.08, -s * 0.08), s * 0.08, PANEL);
        }
        Icon::Accessory => {
            let d = s * 0.34;
            painter.add(egui::Shape::closed_line(
                vec![
                    c + Vec2::new(0.0, -d),
                    c + Vec2::new(d, 0.0),
                    c + Vec2::new(0.0, d),
                    c + Vec2::new(-d, 0.0),
                ],
                stroke,
            ));
        }
        Icon::Reset => {
            arc(painter, c, s * 0.32, 0.6, 5.3, stroke);
            let tip = c + Vec2::new(s * 0.32 * 0.6_f32.cos(), s * 0.32 * 0.6_f32.sin());
            painter.line_segment([tip, tip + Vec2::new(-s * 0.16, -s * 0.02)], stroke);
            painter.line_segment([tip, tip + Vec2::new(0.02, s * 0.16)], stroke);
        }
        Icon::Random => {
            let r = Rect::from_center_size(c, Vec2::splat(s * 0.62));
            painter.rect_stroke(r, CornerRadius::same(2), stroke, StrokeKind::Inside);
            for (dx, dy) in [
                (-0.16, -0.16),
                (0.16, -0.16),
                (0.0, 0.0),
                (-0.16, 0.16),
                (0.16, 0.16),
            ] {
                painter.circle_filled(c + Vec2::new(dx * s, dy * s), s * 0.06, color);
            }
        }
        Icon::Symmetry => {
            painter.line_segment(
                [c + Vec2::new(0.0, -s * 0.34), c + Vec2::new(0.0, s * 0.34)],
                Stroke::new(1.0, color),
            );
            painter.add(egui::Shape::closed_line(
                vec![
                    c + Vec2::new(-s * 0.08, -s * 0.22),
                    c + Vec2::new(-s * 0.32, 0.0),
                    c + Vec2::new(-s * 0.08, s * 0.22),
                ],
                stroke,
            ));
            painter.add(egui::Shape::closed_line(
                vec![
                    c + Vec2::new(s * 0.08, -s * 0.22),
                    c + Vec2::new(s * 0.32, 0.0),
                    c + Vec2::new(s * 0.08, s * 0.22),
                ],
                stroke,
            ));
        }
        Icon::Bone => {
            painter.line_segment(
                [
                    c + Vec2::new(-s * 0.16, s * 0.16),
                    c + Vec2::new(s * 0.16, -s * 0.16),
                ],
                stroke,
            );
            painter.circle_stroke(c + Vec2::new(-s * 0.2, s * 0.2), s * 0.12, stroke);
            painter.circle_stroke(c + Vec2::new(s * 0.2, -s * 0.2), s * 0.12, stroke);
        }
        Icon::Send => {
            let box_r = Rect::from_min_max(
                c + Vec2::new(-s * 0.34, -s * 0.16),
                c + Vec2::new(0.0, s * 0.28),
            );
            painter.rect_stroke(box_r, CornerRadius::same(1), stroke, StrokeKind::Inside);
            painter.line_segment(
                [c + Vec2::new(-s * 0.02, 0.0), c + Vec2::new(s * 0.32, 0.0)],
                stroke,
            );
            painter.line_segment(
                [
                    c + Vec2::new(s * 0.16, -s * 0.14),
                    c + Vec2::new(s * 0.32, 0.0),
                ],
                stroke,
            );
            painter.line_segment(
                [
                    c + Vec2::new(s * 0.16, s * 0.14),
                    c + Vec2::new(s * 0.32, 0.0),
                ],
                stroke,
            );
        }
        Icon::Import => {
            painter.line_segment(
                [c + Vec2::new(0.0, -s * 0.32), c + Vec2::new(0.0, s * 0.08)],
                stroke,
            );
            painter.line_segment(
                [
                    c + Vec2::new(-s * 0.14, -s * 0.06),
                    c + Vec2::new(0.0, s * 0.08),
                ],
                stroke,
            );
            painter.line_segment(
                [
                    c + Vec2::new(s * 0.14, -s * 0.06),
                    c + Vec2::new(0.0, s * 0.08),
                ],
                stroke,
            );
            painter.line_segment(
                [
                    c + Vec2::new(-s * 0.28, s * 0.16),
                    c + Vec2::new(-s * 0.28, s * 0.32),
                ],
                stroke,
            );
            painter.line_segment(
                [
                    c + Vec2::new(s * 0.28, s * 0.16),
                    c + Vec2::new(s * 0.28, s * 0.32),
                ],
                stroke,
            );
            painter.line_segment(
                [
                    c + Vec2::new(-s * 0.28, s * 0.32),
                    c + Vec2::new(s * 0.28, s * 0.32),
                ],
                stroke,
            );
        }
        Icon::Undo => arrow(painter, c, s, -1.0, stroke),
        Icon::Redo => arrow(painter, c, s, 1.0, stroke),
        Icon::Search => {
            painter.circle_stroke(c + Vec2::new(-s * 0.06, -s * 0.06), s * 0.22, stroke);
            painter.line_segment(
                [
                    c + Vec2::new(s * 0.1, s * 0.1),
                    c + Vec2::new(s * 0.3, s * 0.3),
                ],
                stroke,
            );
        }
        Icon::Eye => {
            painter.circle_stroke(c, s * 0.22, stroke);
            painter.circle_filled(c, s * 0.08, color);
        }
        Icon::EyeOff => {
            painter.circle_stroke(c, s * 0.22, stroke);
            painter.line_segment(
                [
                    c + Vec2::new(-s * 0.28, s * 0.28),
                    c + Vec2::new(s * 0.28, -s * 0.28),
                ],
                stroke,
            );
        }
        Icon::Frame => {
            let d = s * 0.28;
            let a = s * 0.14;
            for (sx, sy) in [(-1.0, -1.0), (1.0, -1.0), (-1.0, 1.0), (1.0, 1.0)] {
                let o = Vec2::new(sx * d, sy * d);
                painter.line_segment([c + o, c + o + Vec2::new(-sx * a, 0.0)], stroke);
                painter.line_segment([c + o, c + o + Vec2::new(0.0, sy * -a)], stroke);
            }
        }
        Icon::Head => {
            painter.circle_stroke(c + Vec2::new(0.0, -s * 0.06), s * 0.22, stroke);
            painter.line_segment(
                [
                    c + Vec2::new(-s * 0.22, s * 0.28),
                    c + Vec2::new(s * 0.22, s * 0.28),
                ],
                stroke,
            );
        }
        Icon::Trash => {
            painter.line_segment(
                [
                    c + Vec2::new(-s * 0.22, -s * 0.16),
                    c + Vec2::new(s * 0.22, -s * 0.16),
                ],
                stroke,
            );
            painter.line_segment(
                [
                    c + Vec2::new(-s * 0.08, -s * 0.16),
                    c + Vec2::new(-s * 0.08, -s * 0.28),
                ],
                stroke,
            );
            painter.line_segment(
                [
                    c + Vec2::new(s * 0.08, -s * 0.16),
                    c + Vec2::new(s * 0.08, -s * 0.28),
                ],
                stroke,
            );
            painter.line_segment(
                [
                    c + Vec2::new(-s * 0.08, -s * 0.28),
                    c + Vec2::new(s * 0.08, -s * 0.28),
                ],
                stroke,
            );
            let bin = Rect::from_min_max(
                c + Vec2::new(-s * 0.2, -s * 0.12),
                c + Vec2::new(s * 0.2, s * 0.32),
            );
            painter.rect_stroke(bin, CornerRadius::same(1), stroke, StrokeKind::Inside);
        }
        Icon::Wardrobe => {
            // Torso, neck opening and two short sleeves. Original geometry.
            let torso =
                Rect::from_center_size(c + Vec2::new(0.0, s * 0.08), Vec2::new(s * 0.4, s * 0.58));
            painter.rect_stroke(torso, CornerRadius::same(2), stroke, StrokeKind::Inside);
            painter.line_segment(
                [
                    torso.left_top() + Vec2::new(0.0, s * 0.1),
                    c + Vec2::new(-s * 0.4, -s * 0.02),
                ],
                stroke,
            );
            painter.line_segment(
                [
                    torso.right_top() + Vec2::new(0.0, s * 0.1),
                    c + Vec2::new(s * 0.4, -s * 0.02),
                ],
                stroke,
            );
            painter.line_segment(
                [
                    torso.center_top() + Vec2::new(-s * 0.08, 0.0),
                    torso.center_top() + Vec2::new(0.0, s * 0.12),
                ],
                stroke,
            );
            painter.line_segment(
                [
                    torso.center_top() + Vec2::new(s * 0.08, 0.0),
                    torso.center_top() + Vec2::new(0.0, s * 0.12),
                ],
                stroke,
            );
        }
    }
}

fn arrow(painter: &egui::Painter, c: Pos2, s: f32, dir: f32, stroke: Stroke) {
    painter.line_segment(
        [
            c + Vec2::new(-dir * s * 0.28, 0.0),
            c + Vec2::new(dir * s * 0.28, 0.0),
        ],
        stroke,
    );
    let tip = c + Vec2::new(dir * s * 0.28, 0.0);
    painter.line_segment([tip, tip + Vec2::new(-dir * s * 0.16, -s * 0.14)], stroke);
    painter.line_segment([tip, tip + Vec2::new(-dir * s * 0.16, s * 0.14)], stroke);
}

fn arc(painter: &egui::Painter, center: Pos2, radius: f32, start: f32, end: f32, stroke: Stroke) {
    let steps = 14;
    let mut pts = Vec::with_capacity(steps + 1);
    for i in 0..=steps {
        let t = start + (end - start) * (i as f32 / steps as f32);
        pts.push(center + Vec2::new(radius * t.cos(), radius * t.sin()));
    }
    painter.add(egui::Shape::line(pts, stroke));
}

/// Square icon button. `selected` paints the accent fill used by the rail.
pub fn icon_button(
    ui: &mut Ui,
    icon: Icon,
    tip: impl Into<WidgetText>,
    selected: bool,
    enabled: bool,
) -> egui::Response {
    icon_button_sized(ui, icon, tip, selected, enabled, 32.0)
}

pub fn icon_button_sized(
    ui: &mut Ui,
    icon: Icon,
    tip: impl Into<WidgetText>,
    selected: bool,
    enabled: bool,
    size: f32,
) -> egui::Response {
    let sense = if enabled {
        Sense::click()
    } else {
        Sense::hover()
    };
    let (rect, resp) = ui.allocate_exact_size(Vec2::splat(size), sense);
    let bg = if selected && enabled {
        ACCENT
    } else if resp.hovered() && enabled {
        WIDGET_HOVER
    } else {
        Color32::TRANSPARENT
    };
    if bg != Color32::TRANSPARENT {
        ui.painter()
            .rect_filled(rect, CornerRadius::same(RADIUS_SM), bg);
    }
    let color = if !enabled {
        TEXT_FAINT
    } else if selected {
        Color32::WHITE
    } else {
        TEXT
    };
    let pad = size * 0.22;
    paint_icon(ui.painter(), rect.shrink(pad), icon, color);
    resp.on_hover_text(tip)
}

/// Pill segmented control. Returns the index that was clicked.
pub fn segmented(ui: &mut Ui, salt: &str, labels: &[&str], selected: usize) -> Option<usize> {
    let h = 28.0;
    let w = 86.0;
    let total = Vec2::new(w * labels.len() as f32, h);
    let (rect, _) = ui.allocate_exact_size(total, Sense::empty());
    ui.painter()
        .rect_filled(rect, CornerRadius::same(RADIUS), WIDGET);
    ui.painter().rect_stroke(
        rect,
        CornerRadius::same(RADIUS),
        Stroke::new(1.0, BORDER),
        StrokeKind::Inside,
    );
    let mut clicked = None;
    for (i, label) in labels.iter().enumerate() {
        let r = Rect::from_min_size(
            rect.min + Vec2::new(i as f32 * w + 2.0, 2.0),
            Vec2::new(w - 4.0, h - 4.0),
        );
        let resp = ui.interact(r, ui.id().with((salt, i)), Sense::click());
        if i == selected {
            ui.painter()
                .rect_filled(r, CornerRadius::same(RADIUS_SM), ACCENT);
        } else if resp.hovered() {
            ui.painter()
                .rect_filled(r, CornerRadius::same(RADIUS_SM), WIDGET_HOVER);
        }
        let color = if i == selected {
            Color32::WHITE
        } else {
            TEXT_DIM
        };
        ui.painter().text(
            r.center(),
            egui::Align2::CENTER_CENTER,
            *label,
            FontId::proportional(12.5),
            color,
        );
        if resp.clicked() {
            clicked = Some(i);
        }
    }
    clicked
}

/// Text tabs with an accent underline on the active one.
pub fn text_tabs(ui: &mut Ui, salt: &str, labels: &[&str], selected: usize) -> Option<usize> {
    let mut clicked = None;
    egui::ScrollArea::horizontal()
        .id_salt(salt)
        .max_height(26.0)
        .auto_shrink([false, true])
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 2.0;
                for (i, label) in labels.iter().enumerate() {
                    let w = (label.len() as f32).mul_add(6.6, 18.0).max(44.0);
                    let (rect, resp) = ui.allocate_exact_size(Vec2::new(w, 24.0), Sense::click());
                    let color = if i == selected { TEXT } else { TEXT_DIM };
                    ui.painter().text(
                        rect.center(),
                        egui::Align2::CENTER_CENTER,
                        *label,
                        FontId::proportional(12.0),
                        color,
                    );
                    if i == selected {
                        ui.painter().line_segment(
                            [rect.left_bottom(), rect.right_bottom()],
                            Stroke::new(2.0, ACCENT),
                        );
                    }
                    if resp.clicked() {
                        clicked = Some(i);
                    }
                }
            });
        });
    clicked
}

/// Toggle chip. `enabled == false` draws it inert (used for axes we don't mirror).
pub fn chip(ui: &mut Ui, label: &str, on: bool, enabled: bool) -> egui::Response {
    let w = (label.len() as f32).mul_add(7.0, 16.0).max(28.0);
    let sense = if enabled {
        Sense::click()
    } else {
        Sense::hover()
    };
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(w, 24.0), sense);
    let bg = if on && enabled {
        ACCENT
    } else if resp.hovered() && enabled {
        WIDGET_HOVER
    } else {
        WIDGET
    };
    ui.painter().rect_filled(rect, CornerRadius::same(5), bg);
    ui.painter().rect_stroke(
        rect,
        CornerRadius::same(5),
        Stroke::new(1.0, if on && enabled { ACCENT } else { BORDER }),
        StrokeKind::Inside,
    );
    let color = if !enabled {
        TEXT_FAINT
    } else if on {
        Color32::WHITE
    } else {
        TEXT
    };
    ui.painter().text(
        rect.center(),
        egui::Align2::CENTER_CENTER,
        label,
        FontId::proportional(12.0),
        color,
    );
    resp
}

/// Shelf category / filter row.
pub fn nav_item(ui: &mut Ui, label: &str, selected: bool) -> egui::Response {
    let (rect, resp) =
        ui.allocate_exact_size(Vec2::new(ui.available_width(), 26.0), Sense::click());
    if selected {
        ui.painter()
            .rect_filled(rect, CornerRadius::same(RADIUS_SM), ACCENT);
    } else if resp.hovered() {
        ui.painter()
            .rect_filled(rect, CornerRadius::same(RADIUS_SM), WIDGET_HOVER);
    }
    ui.painter().text(
        rect.left_center() + Vec2::new(10.0, 0.0),
        egui::Align2::LEFT_CENTER,
        label,
        FontId::proportional(12.5),
        if selected { Color32::WHITE } else { TEXT },
    );
    resp
}

/// Inspector list row: label on the left, optional trailing text on the right.
pub fn selectable_row(
    ui: &mut Ui,
    id: impl std::hash::Hash + std::fmt::Debug,
    label: &str,
    trailing: &str,
    selected: bool,
) -> egui::Response {
    ui.push_id(id, |ui| {
        let (rect, resp) =
            ui.allocate_exact_size(Vec2::new(ui.available_width(), 28.0), Sense::click());
        paint_row(ui, rect, &resp, label, trailing, selected);
        resp
    })
    .inner
}

fn paint_row(
    ui: &Ui,
    rect: Rect,
    resp: &egui::Response,
    label: &str,
    trailing: &str,
    selected: bool,
) {
    if selected {
        ui.painter()
            .rect_filled(rect, CornerRadius::same(5), ROW_SELECTED);
    } else if resp.hovered() {
        ui.painter()
            .rect_filled(rect, CornerRadius::same(5), WIDGET_HOVER);
    }
    ui.painter().text(
        rect.left_center() + Vec2::new(8.0, 0.0),
        egui::Align2::LEFT_CENTER,
        label,
        FontId::proportional(12.5),
        if selected { Color32::WHITE } else { TEXT },
    );
    if !trailing.is_empty() {
        ui.painter().text(
            rect.right_center() + Vec2::new(-8.0, 0.0),
            egui::Align2::RIGHT_CENTER,
            trailing,
            FontId::proportional(11.0),
            if selected { Color32::WHITE } else { ACCENT },
        );
    }
}

/// Rounded search field with a magnifier. Edits `text`.
pub fn search_field(ui: &mut Ui, text: &mut String, hint: &str, width: f32) -> egui::Response {
    let h = 28.0;
    let (rect, _) = ui.allocate_exact_size(Vec2::new(width, h), Sense::empty());
    ui.painter()
        .rect_filled(rect, CornerRadius::same(7), WIDGET);
    ui.painter().rect_stroke(
        rect,
        CornerRadius::same(7),
        Stroke::new(1.0, BORDER),
        StrokeKind::Inside,
    );
    let icon = Rect::from_min_size(rect.min + Vec2::new(6.0, 5.0), Vec2::splat(18.0));
    paint_icon(ui.painter(), icon, Icon::Search, TEXT_DIM);
    let edit = Rect::from_min_max(
        rect.min + Vec2::new(28.0, 2.0),
        rect.max - Vec2::new(8.0, 2.0),
    );
    let mut child = ui.new_child(
        egui::UiBuilder::new()
            .max_rect(edit)
            .layout(egui::Layout::left_to_right(egui::Align::Center)),
    );
    child.add(
        egui::TextEdit::singleline(text)
            .hint_text(hint)
            .frame(Frame::NONE)
            .desired_width(edit.width()),
    )
}

/// Filled status dot (green when `on`).
pub fn status_dot(ui: &mut Ui, on: bool) {
    let (rect, _) = ui.allocate_exact_size(Vec2::new(12.0, 14.0), Sense::hover());
    ui.painter()
        .circle_filled(rect.center(), 4.0, if on { GOOD } else { TEXT_FAINT });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accent_is_the_family_blue() {
        assert!(ACCENT.b() > 180, "accent should read as blue");
        assert!(ACCENT.b() > ACCENT.r() + 80);
        assert!(ACCENT.b() > ACCENT.g());
        // Text must stay clearly lighter than the panel it sits on.
        let text = 0.2126 * f32::from(TEXT.r())
            + 0.7152 * f32::from(TEXT.g())
            + 0.0722 * f32::from(TEXT.b());
        let panel = 0.2126 * f32::from(PANEL.r())
            + 0.7152 * f32::from(PANEL.g())
            + 0.0722 * f32::from(PANEL.b());
        assert!(text - panel > 140.0);
    }

    #[test]
    fn roboto_file_is_a_font() {
        assert!(ROBOTO.len() > 10_000);
        let tag = &ROBOTO[..4];
        assert!(
            tag == [0x00, 0x01, 0x00, 0x00] || tag == b"true" || tag == b"OTTO",
            "expected a TrueType or OpenType file"
        );
    }

    #[test]
    fn widgets_paint_without_panicking() {
        let ctx = egui::Context::default();
        install(&ctx);
        let output = ctx.run_ui(egui::RawInput::default(), |ui| {
            assert!(segmented(ui, "modes", &["Morph", "Material", "Pose", "Export"], 0).is_none());
            assert!(text_tabs(ui, "cats", &["Body", "Head"], 0).is_none());
            assert!(!chip(ui, "X", true, true).clicked());
            assert!(!nav_item(ui, "All", true).clicked());
            assert!(!selectable_row(ui, "nose", "Nose", "2", true).clicked());
            let mut query = String::new();
            let _ = search_field(ui, &mut query, "Search morphs…", 180.0);
            let (rect, _) = ui.allocate_exact_size(Vec2::splat(24.0), Sense::hover());
            for icon in [
                Icon::Preset,
                Icon::Skin,
                Icon::Accessory,
                Icon::Reset,
                Icon::Random,
                Icon::Symmetry,
                Icon::Bone,
                Icon::Send,
                Icon::Import,
                Icon::Undo,
                Icon::Redo,
                Icon::Search,
                Icon::Eye,
                Icon::EyeOff,
                Icon::Trash,
                Icon::Frame,
                Icon::Head,
                Icon::Wardrobe,
            ] {
                paint_icon(ui.painter(), rect, icon, TEXT);
            }
            brand_mark(ui.painter(), rect);
        });
        output.drop_without_applying_deltas();
    }
}
