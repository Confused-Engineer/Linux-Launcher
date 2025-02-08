use eframe::egui::{self, include_image};

const IMAGE_SIZE:  [f32; 2]  = [115.0,115.0];
const COLUMN_COUNT: usize = 7;
const GRID_SPACING:  [f32; 2]  = [30.0,15.0];
const CORNER_RADIUS: f32 = 10.0;

pub fn page_primary(ui: &mut egui::Ui, entries_list: crate::framework::EntryList, search: std::rc::Rc<std::cell::RefCell<String>>)
{
    
    ui.horizontal_top( |ui| {
        ui.label(egui::RichText::new("Search:").size(15.0));
        ui.text_edit_singleline(&mut *search.borrow_mut());
    });

    if !&search.borrow().is_empty()
    {
        ui.label(egui::RichText::new(format!("Showing Results For: \"{}\"", search.borrow())).size(12.0));
    } else {
        ui.add_space(17.0);
    }

    


    egui::Grid::new("app_layout").spacing(GRID_SPACING).num_columns(COLUMN_COUNT).show(ui, |ui| {
        let mut x = 1;
        for entry in entries_list
        {
            let title = entry.get_title();
            let mut icon = entry.get_icon();
            let command = entry.get_command();
            let arguments = entry.get_arguments();

            if !title.to_ascii_lowercase().contains(&search.borrow().to_ascii_lowercase())
            {
                continue;
            }

            if !icon.is_empty() && !icon.contains("https://")
            {
                icon = format!("file://{}", icon);
            }

            ui.vertical(|ui| {
                if !icon.is_empty()
                {
                    if ui.add_sized(IMAGE_SIZE, egui::widgets::ImageButton::new(icon).frame(false).corner_radius(CORNER_RADIUS)).clicked()
                    {
                        launch(&command, &arguments);
                    }
                } else {
                    if ui.add_sized(IMAGE_SIZE, egui::widgets::ImageButton::new(include_image!("../../assets/favicon.ico")).frame(false).corner_radius(CORNER_RADIUS)).clicked()
                    {
                        launch(&command, &arguments);
                    }
                }
    
                ui.vertical_centered(|ui|{
                    ui.heading(title);
                });
            });

            if (x != 0) && (x % COLUMN_COUNT == 0)
            {
                x += 1;
                ui.end_row();
            }  else {
                x += 1;
            }
            
        }
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

}