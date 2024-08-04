use eframe::egui;

use crate::primary::primary_page;




#[derive(serde::Deserialize, serde::Serialize)]
#[serde(default)] 
pub struct MyApp {
    strign: String,
    dm: bool,
    #[serde(skip)]
    settings: bool,

}

impl Default for MyApp {
    fn default() -> Self {
        Self {
            dm: true,
            strign: String::new(),
            settings: false
        }
    }
}

impl eframe::App for MyApp {
    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        eframe::set_value(storage, eframe::APP_KEY, self);
    }

    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        egui::Rgba::TRANSPARENT.to_array() // Make sure we don't paint anything behind the rounded corners
    }


    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        //if self.dm { ctx.set_visuals(Visuals::dark()); } else { ctx.set_visuals(Visuals::light()); };
        let settings = self.settings.clone();

        custom_window_frame(ctx, "Linux Launcher", |ui| {
            //ui.add_space(10.0);
            if settings { 
                crate::settings::settings_page(ui);
                
            }

            if !settings {
                primary_page(ui);
            }
            

        }, &mut self.settings);
    }
}

impl MyApp {

    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        // This is also where you can customize the look and feel of egui using
        // `cc.egui_ctx.set_visuals` and `cc.egui_ctx.set_fonts`.
        
        // Load previous app state (if any).
        // Note that you must enable the `persistence` feature for this to work.
        
        if let Some(storage) = cc.storage {
            return eframe::get_value(storage, eframe::APP_KEY).unwrap_or_default();
        }

        Default::default()
    }
}

fn custom_window_frame(ctx: &egui::Context, title: &str, add_contents: impl FnOnce(&mut egui::Ui), show_settings: &mut bool) {
    use egui::*;

    let panel_frame = egui::Frame {
        fill: ctx.style().visuals.window_fill(),
        rounding: 10.0.into(),
        stroke: ctx.style().visuals.widgets.noninteractive.fg_stroke,
        outer_margin: 0.5.into(), // so the stroke is within the bounds
        ..Default::default()
    };

    CentralPanel::default().frame(panel_frame).show(ctx, |ui| {
        let app_rect = ui.max_rect();

        let title_bar_height = 32.0;
        let title_bar_rect = {
            let mut rect = app_rect;
            rect.max.y = rect.min.y + title_bar_height;
            rect
        };
        title_bar_ui(ui, title_bar_rect, title, show_settings);

        // Add the contents:
        let content_rect = {
            let mut rect = app_rect;
            rect.min.y = title_bar_rect.max.y;
            rect
        }
        .shrink(10.0);
        let mut content_ui = ui.child_ui(content_rect, *ui.layout(), None);
        add_contents(&mut content_ui);
    });
}

fn title_bar_ui(ui: &mut egui::Ui, title_bar_rect: eframe::epaint::Rect, title: &str, show_settings: &mut bool) {
    use egui::*;

    let painter = ui.painter();

    let title_bar_response = ui.interact(
        title_bar_rect,
        Id::new("title_bar"),
        Sense::click_and_drag(),
    );

    // Paint the title:
    painter.text(
        title_bar_rect.center(),
        Align2::CENTER_CENTER,
        title,
        FontId::proportional(20.0),
        ui.style().visuals.text_color(),
    );

    // Paint the line under the title:
    /* 
    painter.line_segment(
        [
            title_bar_rect.left_bottom() + vec2(1.0, 0.0),
            title_bar_rect.right_bottom() + vec2(-1.0, 0.0),
        ],
        ui.visuals().widgets.noninteractive.bg_stroke,
    );
    */

    if title_bar_response.drag_started_by(PointerButton::Primary) {
        ui.ctx().send_viewport_cmd(ViewportCommand::StartDrag);
    }

    ui.allocate_ui_at_rect(title_bar_rect, |ui| {
        ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
            ui.spacing_mut().item_spacing.x = 0.0;
            ui.visuals_mut().button_frame = false;
            ui.add_space(8.0);
            settings(ui, show_settings);
            
            
        });


    });


    ui.allocate_ui_at_rect(title_bar_rect, |ui| {
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.spacing_mut().item_spacing.x = 0.0;
            ui.visuals_mut().button_frame = false;
            ui.add_space(8.0);
            close(ui);
        });
    });


}





/// Show some close/maximize/minimize buttons for the native window.
fn close(ui: &mut egui::Ui) {
    use egui::{Button, RichText};

    let button_height = 14.0;

    let close_response = ui
        .add(Button::new(RichText::new("❌").size(button_height)))
        .on_hover_text("Close the window");
    if close_response.clicked() {
        ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
    }
    ui.add_space(6.0);
    let minimized_response = ui
    .add(Button::new(RichText::new("🗕").size(button_height)))
    .on_hover_text("Minimize the window");
    if minimized_response.clicked() {
        ui.ctx().send_viewport_cmd(egui::ViewportCommand::Minimized(true));
    }
}


fn settings(ui: &mut egui::Ui, show_settings: &mut bool) {
    use egui::{Button, RichText};

    let button_height = 14.0;
    
    let minimized_response = ui
        .add(Button::new(RichText::new("⛭").size(button_height)))
        .on_hover_text("Show Settings");
    if minimized_response.clicked() {
        *show_settings = !*show_settings
    }

}



