use eframe::egui::{self, ViewportCommand};
use crate::pages::{primary::page_primary, settings::page_settings};

pub const CONFIG_NAME: &str = "LL.ini";
pub const WINDOW_SIZE:  [f32; 2]  = [1000.0, 600.0];
pub struct Application {
    page: self::Page,
    config: std::rc::Rc<std::cell::RefCell<ini::Ini>>,
    search: std::rc::Rc<std::cell::RefCell<String>>,
    setting_section: std::rc::Rc<std::cell::RefCell<crate::pages::settings::Section>>,
}

impl Default for Application {
    fn default() -> Self {
        Self { 
            page: Page::Primary,
            config: std::rc::Rc::new(std::cell::RefCell::new(load_config())),
            search: std::rc::Rc::new(std::cell::RefCell::new(String::new())),
            setting_section: std::rc::Rc::new(std::cell::RefCell::new(crate::pages::settings::Section::new())),
        }
    }
}

impl Application {

    fn window_frame(&mut self, ctx: &egui::Context, title: &str, add_contents: impl FnOnce(&mut egui::Ui)) {
        use egui::{CentralPanel, UiBuilder};
    
        let panel_frame = egui::Frame::new()
            .fill(ctx.style().visuals.window_fill())
            .corner_radius(10)
            .stroke(ctx.style().visuals.widgets.noninteractive.fg_stroke)
            .outer_margin(1); // so the stroke is within the bounds
    
        CentralPanel::default().frame(panel_frame).show(ctx, |ui| {
            let app_rect = ui.max_rect();
    
            let title_bar_height = 32.0;
            let title_bar_rect = {
                let mut rect = app_rect;
                rect.max.y = rect.min.y + title_bar_height;
                rect
            };
            self.title_bar_ui(ui, title_bar_rect, title);
            
    
            // Add the contents:
            let content_rect = {
                let mut rect = app_rect;
                rect.min.y = title_bar_rect.max.y;
                rect
            }
            .shrink(5.5);
            let mut content_ui = ui.new_child(UiBuilder::new().max_rect(content_rect));
            add_contents(&mut content_ui);
        });
    }

    fn title_bar_ui(&mut self, ui: &mut egui::Ui, title_bar_rect: eframe::epaint::Rect, title: &str) {
        use egui::{vec2, Align2, FontId, Id, PointerButton, Sense, UiBuilder};
    
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
        painter.line_segment(
            [
                title_bar_rect.left_bottom() + vec2(1.0, 0.0),
                title_bar_rect.right_bottom() + vec2(-1.0, 0.0),
            ],
            ui.visuals().widgets.noninteractive.bg_stroke,
        );
    
        // Interact with the title bar (drag to move window):
            
        if title_bar_response.drag_started_by(PointerButton::Primary) {
            ui.ctx().send_viewport_cmd(ViewportCommand::StartDrag);
        }
    
        ui.allocate_new_ui(
            UiBuilder::new()
                .max_rect(title_bar_rect)
                .layout(egui::Layout::right_to_left(egui::Align::Center)),
            |ui| {
                ui.spacing_mut().item_spacing.x = 0.0;
                ui.visuals_mut().button_frame = false;
                ui.add_space(8.0);
                close_minimize(ui);
            },
        );

        ui.allocate_new_ui(
            UiBuilder::new()
                .max_rect(title_bar_rect)
                .layout(egui::Layout::left_to_right(egui::Align::Center)),
            |ui| {
                ui.spacing_mut().item_spacing.x = 0.0;
                ui.visuals_mut().button_frame = false;
                ui.add_space(8.0);
                self.page_cycle(ui);
            },
        );
    }

    fn page_cycle(&mut self, ui: &mut egui::Ui) {
        use egui::{Button, RichText};
    
        let button_height = 12.0;
    
        let settings_response = ui
            .add(Button::new(RichText::new("⛭").size(button_height)))
            .on_hover_text(match self.page {
                Page::Primary => "Open Settings",
                Page::Settings => "Return",
            });
        if settings_response.clicked() {
            match self.page {
                Page::Primary => self.page = Page::Settings,
                Page::Settings => self.page = Page::Primary,
            }
        }
    
    

    }

}


impl eframe::App for Application {
    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        egui::Rgba::TRANSPARENT.to_array() // Make sure we don't paint anything behind the rounded corners
    }

    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(WINDOW_SIZE.into()));
        let page = self.page.clone();
        let config = self.config.clone();
        let search = self.search.clone();
        let section = self.setting_section.clone();

        let title: &str = match self.page {
            Page::Primary => "Linux Launcher",
            Page::Settings => "Settings",
        };

        let entry_list = sort(self.config.clone());
        self.window_frame(ctx, title, |ui| {
            match page {
                Page::Primary => page_primary(ui, entry_list, search),
                Page::Settings => page_settings(ui, config, section),
            }
        });

        


    }
}

fn close_minimize(ui: &mut egui::Ui) {
    use egui::{Button, RichText};

    let button_height = 12.0;

    let close_response = ui
        .add(Button::new(RichText::new("❌").size(button_height)))
        .on_hover_text("Close the window");
    if close_response.clicked() {
        ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
    }

    ui.add_space(5.0);

    let minimized_response = ui
        .add(Button::new(RichText::new("🗕").size(button_height)))
        .on_hover_text("Minimize the window");
    if minimized_response.clicked() {
        ui.ctx().send_viewport_cmd(ViewportCommand::Minimized(true));
    }
}

fn load_config() -> ini::Ini
{
    if let Ok(config) = ini::Ini::load_from_file_noescape(CONFIG_NAME)
    {
        return config;
    } else {
        return ini::Ini::new();
    }
}


#[derive(PartialEq, Clone, Copy)]
enum Page {
    Primary,
    Settings
}

pub type EntryList = Vec<Entry>;
#[derive(Debug)]
pub struct Entry {
    title: String,
    icon: String,
    command: String,
    arguments: String,
    weight: i64,
}

impl Entry {
    pub fn get_title(&self) -> String
    {
        self.title.clone()
    }

    pub fn get_icon(&self) -> String
    {
        self.icon.clone()
    }

    pub fn get_command(&self) -> String
    {
        self.command.clone()
    }

    pub fn get_arguments(&self) -> String
    {
        self.arguments.clone()
    }
}



fn sort(config: std::rc::Rc<std::cell::RefCell<ini::Ini>>) -> EntryList
{
    let mut sorted_vec: Vec<Entry> = Vec::new();

    //let config_clone_1 = config.borrow().clone();
    let mut config_clone_2 = config.borrow().clone();

    //let sections = config_clone_1.sections();
    let config = config.borrow();
    let sections = config.sections();
    
    for unique_id in sections
    {
        if let Some(unique_id) = unique_id
        {
            let section = config_clone_2.with_section(Some(unique_id));
            if let (Some(title), icon, Some(command), arguments, weight) = (section.get("title"), section.get("icon"), section.get("command"), section.get("arguments"), section.get("weight"))
            {
                let weight = weight.unwrap_or("999").parse::<i64>().unwrap_or(99999);
                sorted_vec.push( 
                    Entry { 
                        title: title.to_owned(), 
                        icon: icon.unwrap_or("").to_owned(), 
                        command: command.to_owned(), 
                        arguments: arguments.unwrap_or("").to_owned(), 
                        weight: weight 
                    } 
                );
            }
        } else {
            continue;
        }
    }

    sorted_vec.sort_by_key(|x| x.weight);
    sorted_vec
}

#[cfg(test)]
mod tests {
    use super::*;



    #[test]
    fn it_works() {

        
       let config = std::rc::Rc::new(std::cell::RefCell::new(ini::Ini::load_from_str(include_str!("../assets/config.ini")).unwrap()));
        println!("{:#?}",sort(config)) ;
    }
}