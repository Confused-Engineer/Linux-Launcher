use eframe::egui::{self, include_image};

struct Dimension
{
    column_count: usize,
    image_size: [f32; 2] 
}



pub fn primary_page(ui: &mut eframe::egui::Ui, search: &mut String)
{
    let dimensions = Dimension {
        image_size: [115.0,115.0],
        column_count: 7
    };


    let mut config_file = crate::config::ApplicationEntries::new();

    if config_file.is_empty()
    {
        ui.heading("Config File Not Found or is in Error");
        if ui.button("Generate New Config File").clicked()
        {
            config_file.reset();
        }
        return;
    }



    let config = config_file.return_config();

    ui.with_layout(egui::Layout::left_to_right(egui::Align::TOP), |ui|{
        ui.add_space(4.0);
        ui.heading("Search");
        //ui.text_edit_singleline(search);
        ui.add_sized([600.0,20.0], egui::widgets::TextEdit::singleline(search));
    });

    if !search.clone().is_empty()
    {
        ui.label(format!("Showing results for \"{}\"", search.clone()));
        
        
    }

    ui.add_space(10.0);
    egui::ScrollArea::vertical().show(ui, |ui|{
        egui::Grid::new("issource").spacing([30.0,15.0]).show(ui, |ui|{
            layout(ui, dimensions, config, search.clone());
        });
    });
    
}



fn layout(ui: &mut eframe::egui::Ui, dimensions: Dimension, config: ini::Ini, search: String)
{

    

    let sections = config.sections();
    let mut x = 1;
    for section in sections
            {
                if section.is_none()
                {
                    continue;
                }
                let mut config_options = config.clone();
                let config_section = config_options.with_section(section);

                let icon = config_section.get("icon");
                let command = config_section.get("command");
                let title = config_section.get("title");
                let args = config_section.get("arguments");

                if command.is_none() || title.is_none()
                {
                    continue;
                }

                let title = title.unwrap();
                if !search.is_empty() && !title.to_lowercase().contains(&search.to_lowercase())
                {
                    continue;
                }

                ui.vertical(|ui|{

                    
                    
                    let command = command.unwrap();
                    let icon = icon.unwrap_or("");
                    if icon.is_empty()
                    {
                        if ui.add_sized(dimensions.image_size, egui::widgets::ImageButton::new(include_image!("../assets/favicon.ico")).frame(false).rounding(10.0)).clicked()
                        {
                            launch(command, args.unwrap_or(""));
                            
                        }
                    } else if icon.contains("https://") {
                        if ui.add_sized(dimensions.image_size, egui::widgets::ImageButton::new(icon).frame(false).rounding(10.0)).clicked()
                        {
                            launch(command, args.unwrap_or(""));
                            
                        }
                    } else {
                        if ui.add_sized(dimensions.image_size, egui::widgets::ImageButton::new(format!("file://{}", icon)).frame(false).rounding(10.0)).clicked()
                        {
                            launch(command, args.unwrap_or(""));

                        }
                    }

                    ui.vertical_centered(|ui|{
                        ui.heading(title);
                    });
                    

                });

                if (x != 0) && (x % dimensions.column_count == 0)
                {
                    x += 1;
                    ui.end_row();
                }  else {
                    x += 1;
                }
                
            }
    
}



fn launch(app: &str, args: &str) {
    use std::{os::windows::process::CommandExt, process::Command};

    if args.len() == 0
    {
        let _ = Command::new("cmd")
        .args(["/C", app])
        .creation_flags(0x08000000)
        .spawn()
        .expect("failed to execute process");
        return;
    }

    let arg_vec: Vec<&str> = args.split(" ").collect();

    let mut cmd = Command::new("cmd");
    cmd.args(["/C", app]);
    for argument in arg_vec
    {
        cmd.arg(argument);
    }

    cmd.creation_flags(0x08000000)
    .spawn()
    .expect("failed to execute process");

}

