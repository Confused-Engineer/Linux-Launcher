use eframe::egui;

use crate::framework::CONFIG_NAME;

const BUTTON_SIZE:  [f32; 2]  = [100.0,30.0];

pub fn page_settings(ui: &mut egui::Ui, config: std::rc::Rc<std::cell::RefCell<ini::Ini>>, section: std::rc::Rc<std::cell::RefCell<Section>>)
{

    ui.heading("Edit Entries");
    
    

    ui.horizontal_top(|ui| {
        egui::Grid::new("userinput").num_columns(2).striped(false).show(ui, |ui| {
            ui.label("Unique ID:");
            ui.text_edit_singleline(&mut section.borrow_mut().unique_id).on_hover_text("Unique ID, entering a non-unique ID will overwrite/update the entry on that ID");
            ui.end_row();
            ui.label("Title:");
            ui.text_edit_singleline(&mut section.borrow_mut().title).on_hover_text("A friendly name, no requirements");
            ui.end_row();
            ui.label("Icon:");
            ui.text_edit_singleline(&mut section.borrow_mut().icon).on_hover_text("This can be an https link to png/jpg, a file path to an image, or empty for the default icon");
            ui.end_row();
            ui.label("Command:");
            ui.text_edit_singleline(&mut section.borrow_mut().command).on_hover_text("This will be the path to an exe or shortcut. If you want the entry to open to a webpage, put the word 'start'");
            ui.end_row();
            ui.label("Arguments:");
            ui.text_edit_singleline(&mut section.borrow_mut().arguments).on_hover_text("This will be the list of arguments the exe needs if any, or the webpage, or blank");
            ui.end_row();
            ui.label("Weight");
            ui.text_edit_singleline(&mut section.borrow_mut().weight).on_hover_text("This decides the order of entries, from least to greatest, so 1 is 1st");
        });



        
    });
    


    ui.horizontal(|ui| {
        if ui.button("Add/Update Entry").clicked()
        {
            let _ = section.borrow_mut().update_config(&mut config.borrow_mut());
        }

        if ui.button("Clear Fields").clicked()
        {
            section.borrow_mut().clear();
        }
    
        egui::menu::menu_button(ui, "Select Entry", |ui| {
            let entries = &config.borrow();
            ui.set_min_width(150.0);
            for entry in entries.sections()
            {
                if let Some(name) = entry
                {
                    if ui.button(name).clicked()
                    {
                        let mut config_clone = config.borrow().clone();
                        section.borrow_mut().unique_id = name.to_owned();
                        
                        if let Some(title) = config_clone.with_section(Some(name)).get("title")
                        {
                            section.borrow_mut().title = title.to_owned();
                        }

                        if let Some(icon) = config_clone.with_section(Some(name)).get("icon")
                        {
                            section.borrow_mut().icon = icon.to_owned();
                        }

                        if let Some(command) = config_clone.with_section(Some(name)).get("command")
                        {
                            section.borrow_mut().command = command.to_owned();
                        }

                        if let Some(arguments) = config_clone.with_section(Some(name)).get("arguments")
                        {
                            section.borrow_mut().arguments = arguments.to_owned();
                        }

                        if let Some(weight) = config_clone.with_section(Some(name)).get("weight")
                        {
                            section.borrow_mut().weight = weight.to_owned();
                        }
                        
                        ui.close_menu();
                        
                    }
                }
            }
        });

        

        egui::menu::menu_button(ui, "Delete Entry", |ui| {
            let entries = config.borrow().clone();
            ui.set_min_width(150.0);
            for entry in entries.sections()
            {
                
                if let Some(name) = entry
                {
                    if ui.button(name).clicked()
                    {
                        config.borrow_mut().delete(Some(name));
                    }
                }
            }
        });

        
    });

    ui.separator();
    ui.heading("Examples");
    ui.horizontal_top(|ui| {
        egui::Grid::new("example1").num_columns(2).striped(true).show(ui, |ui| {
            ui.label("Unique ID:");
            ui.label("example1");
            ui.end_row();
            ui.label("Title:");
            ui.label("anything");
            ui.end_row();
            ui.label("Icon:");
            ui.label("https://really.cool.png");
            ui.end_row();
            ui.label("Command:");
            ui.label("start");
            ui.end_row();
            ui.label("Arguments:");
            ui.label("https://google.com");
            ui.end_row();
            ui.label("Weight");
            ui.label("2");
        });

        egui::Grid::new("example2").num_columns(2).striped(true).show(ui, |ui| {
            ui.label("Unique ID:");
            ui.label("example2");
            ui.end_row();
            ui.label("Title:");
            ui.label("anything");
            ui.end_row();
            ui.label("Icon:");
            ui.label(r"C:\path\to\icon.jpg");
            ui.end_row();
            ui.label("Command:");
            ui.label(r"C:\path\to\exe");
            ui.end_row();
            ui.label("Arguments:");
            ui.label("exe arguments");
            ui.end_row();
            ui.label("Weight");
            ui.label("1");
        });
    });
    
    ui.separator();
    
    ui.heading("Config Options");

    ui.horizontal(|ui| {

        ui.style_mut().text_styles.insert(
            eframe::egui::TextStyle::Heading, 
            eframe::egui::FontId::new(20.0, eframe::epaint::FontFamily::Proportional),
        );
        ui.style_mut().text_styles.insert(
            eframe::egui::TextStyle::Button,
            eframe::egui::FontId::new(20.0, eframe::epaint::FontFamily::Proportional),
        );
        
        if ui.add_sized(BUTTON_SIZE, egui::widgets::Button::new("Reset")).clicked()
        {
            if let Ok(ini) = ini::Ini::load_from_str_noescape(include_str!("../../assets/config.ini"))
            {
                *config.borrow_mut() = ini;
                let _ = config.borrow_mut().write_to_file_policy(crate::framework::CONFIG_NAME, ini::EscapePolicy::Nothing);
            }
        }
        


        if ui.add_sized(BUTTON_SIZE, egui::widgets::Button::new("Backup")).clicked()
        {
            if let Ok(home_dir) = davids_standard_library::env::get_home()
            {
                let file_path = format!("{}/.linuxlauncher/{}", home_dir, CONFIG_NAME);
                let path = std::path::Path::new(&file_path);
                if let Some(prefix) = path.parent()
                {
                    let _ = std::fs::create_dir_all(prefix);
                }
                let _ = config.borrow().write_to_file_policy(file_path, ini::EscapePolicy::Nothing);
            }

            
        }

        if ui.add_sized(BUTTON_SIZE, egui::widgets::Button::new("Restore")).clicked()
        {
            if let Ok(home_dir) = davids_standard_library::env::get_home()
            {
                let file_path = format!("{}/.linuxlauncher/{}", home_dir, CONFIG_NAME);
                if let Ok(backup_config) = ini::Ini::load_from_file_noescape(file_path)
                {
                    *config.borrow_mut() = backup_config;
                    let _ = config.borrow().write_to_file_policy(CONFIG_NAME, ini::EscapePolicy::Nothing);

                }
            }

            
        }
    });

    

    
}


pub struct Section {
    unique_id: String,
    title: String,
    icon: String,
    command: String,
    arguments: String,
    weight: String
}

impl Section {
    pub fn new() -> Self
    {
        Self { unique_id: String::new(), 
            title: String::new(), 
            icon: String::new(), 
            command: String::new(), 
            arguments: String::new(),
            weight: String::new()

        }
    }

    fn update_config(&mut self, config: &mut ini::Ini) -> Result<(), std::io::Error>
    {
        if self.unique_id.is_empty() || self.command.is_empty()
        {
            return Err(std::io::Error::new(std::io::ErrorKind::InvalidInput, "error"));
        }
        config.with_section(Some(&self.unique_id))
        .set("title", &self.title)
        .set("icon", &self.icon)
        .set("command", &self.command)
        .set("arguments", &self.arguments)
        .set("weight", &self.weight);
    

        config.write_to_file_policy(crate::framework::CONFIG_NAME, ini::EscapePolicy::Nothing)
    }

    fn clear(&mut self)
    {
        self.unique_id = String::new();
        self.title = String::new();
        self.icon = String::new();
        self.command = String::new();
        self.arguments = String::new();
        self.weight = String::new();
    }
}