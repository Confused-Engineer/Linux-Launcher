use eframe::egui::{self, include_image};

pub fn primary_page(ui: &mut eframe::egui::Ui)
{
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

    let mut config_cmd = config_file.return_config();
    let mut config_icon = config_file.return_config();
    let mut config_title = config_file.return_config();
    let mut config_args = config_file.return_config();

    let config = config_file.return_config();
    let sections = config.sections().enumerate();
    
    egui::ScrollArea::vertical().show(ui, |ui|{

    egui::Grid::new("issource").spacing([30.0,15.0]).show(ui, |ui|{

        for (x, section) in sections
        {
            if section.is_none()
            {
                continue;
            }

            let section_cmd = config_cmd.with_section(section);
            let section_icon = config_icon.with_section(section);
            let section_title = config_title.with_section(section);
            let section_args = config_args.with_section(section);
            

            let icon = section_icon.get("icon");
            let command = section_cmd.get("command");
            let title = section_title.get("title");
            let args = section_args.get("arguments");

            if command.is_none() || title.is_none()
            {
                continue;
            }

            ui.vertical(|ui|{

                
                
                let command = command.unwrap();
                let icon = icon.unwrap_or("");
                if icon.is_empty()
                {
                    if ui.add_sized([130.0,130.0], egui::widgets::ImageButton::new(include_image!("../assets/favicon.ico")).frame(false).rounding(10.0)).clicked()
                    {
                        launch(command, args.unwrap_or(""));
                        
                    }
                } else if icon.contains("https://") {
                    if ui.add_sized([130.0,130.0], egui::widgets::ImageButton::new(icon).frame(false).rounding(10.0)).clicked()
                    {
                        launch(command, args.unwrap_or(""));
                        
                    }
                } else {
                    if ui.add_sized([130.0,130.0], egui::widgets::ImageButton::new(format!("file://{}", icon)).frame(false).rounding(10.0)).clicked()
                    {
                        launch(command, args.unwrap_or(""));

                    }
                }
                
                







                ui.vertical_centered(|ui|{
                    ui.heading(title.unwrap());
                });
                

            });

            if (x != 0) && (x % 6 == 0)
            {
                ui.end_row();
            }  
        }
    });

});
    
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

    println!("i got here");


    
    

}

