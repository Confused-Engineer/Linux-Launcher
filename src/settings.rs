pub fn settings_page(ui: &mut eframe::egui::Ui)
{
    ui.style_mut().text_styles.insert(
        eframe::egui::TextStyle::Heading, 
        eframe::egui::FontId::new(20.0, eframe::epaint::FontFamily::Proportional),
    );
    ui.style_mut().text_styles.insert(
        eframe::egui::TextStyle::Button,
        eframe::egui::FontId::new(20.0, eframe::epaint::FontFamily::Proportional),
    );
    ui.heading("Settings");


    if ui.add_sized([200.0, 40.0], eframe::egui::widgets::Button::new("Reset Config to Default") ).clicked()
    {
        let _ = crate::config::ApplicationEntries::new().reset();
    }

    if ui.add_sized([200.0, 40.0], eframe::egui::widgets::Button::new("Open Config") ).clicked()
    {
        let _ = std::process::Command::new("notepad")
        .arg(crate::config::ApplicationEntries::current_file())
        .spawn()
        .expect("failed to execute process");
    }




}