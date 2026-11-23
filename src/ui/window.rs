//! Borderless window chrome: rounded frame, custom title bar, resize edges.

use eframe::egui::{
    self, Align, Align2, Button, CursorIcon, FontId, Id, Layout, PointerButton, Rect,
    ResizeDirection, RichText, Sense, Ui, UiBuilder, ViewportCommand, vec2,
};

const TITLE_BAR_HEIGHT: f32 = 34.0;
const RESIZE_MARGIN: f32 = 6.0;

/// What the user clicked in the title bar.
#[derive(Debug, Default, Clone, Copy)]
pub struct TitleBarResponse {
    /// The settings/back button was clicked.
    pub nav_clicked: bool,
}

/// Turns off winit's drop shadow for borderless windows (Windows only).
///
/// egui-winit enables it for every undecorated window. To make room for the
/// shadow, winit shifts the client area down by one pixel, which leaves an
/// opaque strip along the top edge, and the system frame/shadow it brings
/// back shows through our transparent rounded corners as a light outline.
/// See winit's `WM_NCCALCSIZE` handling and egui issue #8399.
#[cfg(windows)]
pub fn remove_undecorated_shadow(cc: &eframe::CreationContext<'_>) {
    use winit::platform::windows::WindowExtWindows as _;
    if let Some(window) = cc.winit_window() {
        window.set_undecorated_shadow(false);
    }
}

#[cfg(not(windows))]
pub fn remove_undecorated_shadow(_cc: &eframe::CreationContext<'_>) {}

/// Draws the window frame and title bar into the app's root `ui`, then
/// `add_contents` in the body.
pub fn show(
    ui: &mut Ui,
    title: &str,
    in_settings: bool,
    add_contents: impl FnOnce(&mut Ui),
) -> TitleBarResponse {
    let maximized = ui.input(|i| i.viewport().maximized.unwrap_or(false));
    let style = ui.global_style();
    let frame = egui::Frame::new()
        .fill(style.visuals.window_fill())
        .corner_radius(if maximized { 0 } else { 10 })
        .stroke(style.visuals.widgets.noninteractive.bg_stroke)
        .outer_margin(if maximized { 0 } else { 1 });

    let mut result = TitleBarResponse::default();
    frame.show(ui, |ui| {
        let app_rect = ui.max_rect();
        ui.expand_to_include_rect(app_rect); // the frame fills the whole window
        let title_rect =
            Rect::from_min_size(app_rect.min, vec2(app_rect.width(), TITLE_BAR_HEIGHT));
        result = title_bar(ui, title_rect, title, in_settings, maximized);

        let content_rect =
            Rect::from_min_max(egui::pos2(app_rect.min.x, title_rect.max.y), app_rect.max)
                .shrink2(vec2(12.0, 8.0));
        let mut content = ui.new_child(UiBuilder::new().max_rect(content_rect));
        add_contents(&mut content);

        if !maximized {
            resize_edges(ui, app_rect);
        }
    });
    result
}

fn title_bar(
    ui: &mut Ui,
    rect: Rect,
    title: &str,
    in_settings: bool,
    maximized: bool,
) -> TitleBarResponse {
    let response = ui.interact(rect, Id::unique("title_bar"), Sense::click_and_drag());
    let painter = ui.painter();
    painter.text(
        rect.center(),
        Align2::CENTER_CENTER,
        title,
        FontId::proportional(17.0),
        ui.visuals().strong_text_color(),
    );
    painter.hline(
        rect.x_range().shrink(1.0),
        rect.bottom(),
        ui.visuals().widgets.noninteractive.bg_stroke,
    );

    if response.double_clicked() {
        ui.send_viewport_cmd(ViewportCommand::Maximized(!maximized));
    } else if response.drag_started_by(PointerButton::Primary) {
        ui.send_viewport_cmd(ViewportCommand::StartDrag);
    }

    let mut out = TitleBarResponse::default();
    ui.scope_builder(
        UiBuilder::new()
            .max_rect(rect.shrink2(vec2(8.0, 0.0)))
            .layout(Layout::left_to_right(Align::Center)),
        |ui| {
            ui.visuals_mut().button_frame = false;
            let (icon, hint) = if in_settings {
                ("⬅", "Back to apps (Esc)")
            } else {
                ("⛭", "Settings (Ctrl+,)")
            };
            if ui
                .add(Button::new(RichText::new(icon).size(16.0)))
                .on_hover_text(hint)
                .clicked()
            {
                out.nav_clicked = true;
            }
        },
    );

    ui.scope_builder(
        UiBuilder::new()
            .max_rect(rect.shrink2(vec2(8.0, 0.0)))
            .layout(Layout::right_to_left(Align::Center)),
        |ui| {
            ui.visuals_mut().button_frame = false;
            ui.spacing_mut().item_spacing.x = 6.0;
            if ui
                .add(Button::new(RichText::new("❌").size(14.0)))
                .on_hover_text("Close")
                .clicked()
            {
                ui.send_viewport_cmd(ViewportCommand::Close);
            }
            let (max_icon, max_hint) = if maximized {
                ("🗗", "Restore")
            } else {
                ("🗖", "Maximize")
            };
            if ui
                .add(Button::new(RichText::new(max_icon).size(14.0)))
                .on_hover_text(max_hint)
                .clicked()
            {
                ui.send_viewport_cmd(ViewportCommand::Maximized(!maximized));
            }
            if ui
                .add(Button::new(RichText::new("🗕").size(14.0)))
                .on_hover_text("Minimize")
                .clicked()
            {
                ui.send_viewport_cmd(ViewportCommand::Minimized(true));
            }
        },
    );
    out
}

/// Invisible strips along the window border that start an OS resize.
fn resize_edges(ui: &mut Ui, rect: Rect) {
    use ResizeDirection as D;
    let m = RESIZE_MARGIN;
    let (l, r, t, b) = (rect.left(), rect.right(), rect.top(), rect.bottom());
    let zones = [
        (
            D::North,
            Rect::from_min_max([l + m, t].into(), [r - m, t + m].into()),
        ),
        (
            D::South,
            Rect::from_min_max([l + m, b - m].into(), [r - m, b].into()),
        ),
        (
            D::West,
            Rect::from_min_max([l, t + m].into(), [l + m, b - m].into()),
        ),
        (
            D::East,
            Rect::from_min_max([r - m, t + m].into(), [r, b - m].into()),
        ),
        (D::NorthWest, Rect::from_min_size([l, t].into(), vec2(m, m))),
        (
            D::NorthEast,
            Rect::from_min_size([r - m, t].into(), vec2(m, m)),
        ),
        (
            D::SouthWest,
            Rect::from_min_size([l, b - m].into(), vec2(m, m)),
        ),
        (
            D::SouthEast,
            Rect::from_min_size([r - 2.0 * m, b - 2.0 * m].into(), vec2(2.0 * m, 2.0 * m)),
        ),
    ];
    for (dir, zone) in zones {
        let response = ui.interact(zone, Id::unique(("resize", dir as u8)), Sense::drag());
        if response.hovered() || response.dragged() {
            ui.set_cursor_icon(match dir {
                D::North | D::South => CursorIcon::ResizeVertical,
                D::East | D::West => CursorIcon::ResizeHorizontal,
                D::NorthWest | D::SouthEast => CursorIcon::ResizeNwSe,
                D::NorthEast | D::SouthWest => CursorIcon::ResizeNeSw,
            });
        }
        if response.drag_started_by(PointerButton::Primary) {
            ui.send_viewport_cmd(ViewportCommand::BeginResize(dir));
        }
    }
}
