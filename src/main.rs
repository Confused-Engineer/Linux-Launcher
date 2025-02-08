#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")] // hide console window on Windows in release
use eframe::egui;




fn main() -> eframe::Result {
    env_logger::init(); // Log to stderr (if you run with `RUST_LOG=debug`).
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_decorations(cfg!(debug_assertions)) // Hide the OS-specific "chrome" around the window
            .with_inner_size(linuxlauncher::framework::WINDOW_SIZE)
            //.with_min_inner_size(WINDOW_SIZE)
            //.with_resizable(true)
            .with_icon(
                // NOTE: Adding an icon is optional
                eframe::icon_data::from_png_bytes(&include_bytes!("../assets/favicon.ico")[..])
                    .unwrap(),
            )
            .with_transparent(true), // To have rounded corners we need transparency
            

        ..Default::default()
    };
    let _ = davids_standard_library::env::set_exe_dir();
    eframe::run_native(
        "Custom window frame", // unused title
        options,
        Box::new(|cc| {
            egui_extras::install_image_loaders(&cc.egui_ctx);
            Ok(Box::<linuxlauncher::framework::Application>::default())
        }),
    )
}