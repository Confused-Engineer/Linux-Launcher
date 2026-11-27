//! The app tile widget: an icon with a label underneath.

use crate::config::AppEntry;
use crate::icons::IconCache;
use egui::{
    self, Color32, Rect, Response, Sense, Stroke, StrokeKind, TextStyle, TextWrapMode, Ui, Vec2,
    WidgetText, vec2,
};

const PADDING: f32 = 10.0;
const LABEL_GAP: f32 = 6.0;
pub const SPACING: f32 = 10.0;

/// Size of a tile for a given icon size.
pub fn tile_size(ui: &Ui, icon_size: f32) -> Vec2 {
    let label_height = ui.text_style_height(&TextStyle::Body);
    vec2(
        icon_size + PADDING * 2.0 + 16.0,
        icon_size + PADDING * 2.0 + LABEL_GAP + label_height,
    )
}

/// Visual state of a tile beyond hover/focus.
#[derive(Debug, Clone, Copy, Default)]
pub struct TileState {
    /// Draw as selected (e.g. the result Enter will launch).
    pub highlighted: bool,
    /// Something is being dragged over this tile and will drop here.
    pub drop_target: bool,
    /// This tile is the one being dragged.
    pub dragging: bool,
}

/// Draws an app tile. The response senses clicks and drags.
pub fn app_tile(
    ui: &mut Ui,
    app: &AppEntry,
    icons: &mut IconCache,
    icon_size: f32,
    state: TileState,
) -> Response {
    let size = tile_size(ui, icon_size);
    let (rect, response) = ui.allocate_exact_size(size, Sense::click_and_drag());
    if !ui.is_rect_visible(rect) {
        return response;
    }

    let visuals = ui.visuals();
    let rounding = 12.0;
    let painter = ui.painter();
    if state.highlighted || response.hovered() || response.has_focus() {
        let fill = if response.is_pointer_button_down_on() {
            visuals.widgets.active.weak_bg_fill
        } else {
            visuals.widgets.hovered.weak_bg_fill
        };
        painter.rect_filled(rect, rounding, fill);
    }
    if state.highlighted || response.has_focus() {
        painter.rect_stroke(rect, rounding, visuals.selection.stroke, StrokeKind::Inside);
    }
    if state.drop_target {
        painter.rect_stroke(
            rect,
            rounding,
            Stroke::new(2.0_f32, visuals.selection.bg_fill),
            StrokeKind::Inside,
        );
    }

    let icon_rect = Rect::from_center_size(
        egui::pos2(rect.center().x, rect.top() + PADDING + icon_size / 2.0),
        Vec2::splat(icon_size),
    );
    if state.dragging {
        ui.painter()
            .rect_filled(icon_rect, rounding, visuals.faint_bg_color);
    } else {
        icons.paint(ui, app, icon_rect);
    }

    if app.favorite {
        ui.painter().text(
            rect.right_top() + vec2(-PADDING, PADDING),
            egui::Align2::RIGHT_TOP,
            "★",
            egui::FontId::proportional(14.0),
            Color32::from_rgb(240, 190, 60),
        );
    }

    let text_color = visuals.text_color();
    let galley = WidgetText::from(app.name.as_str()).into_galley(
        ui,
        Some(TextWrapMode::Truncate),
        rect.width() - 8.0,
        TextStyle::Body,
    );
    let label_pos = egui::pos2(
        rect.center().x - galley.size().x / 2.0,
        icon_rect.bottom() + LABEL_GAP,
    );
    ui.painter().galley(label_pos, galley, text_color);

    response
}
