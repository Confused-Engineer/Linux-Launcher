#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")] // hide console window on Windows in release

use eframe::egui;
use linuxlauncher::*;

fn main() -> eframe::Result {
    env_logger::init(); // Log to stderr (if you run with `RUST_LOG=debug`).

    let _options = eframe::NativeOptions {
        run_and_return: true,
        viewport: egui::ViewportBuilder::default().with_inner_size([320.0, 240.0]),
        ..Default::default()
    };

    let size = [1006.0, 600.0];

    let fancyoptions = eframe::NativeOptions {
        run_and_return: true,
        viewport: egui::ViewportBuilder::default()
        .with_decorations(false) 
        .with_inner_size(size)
        .with_min_inner_size(size)
        .with_max_inner_size(size)
        .with_close_button(true)
        .with_icon(
            // NOTE: Adding an icon is optional
            eframe::icon_data::from_png_bytes(&include_bytes!("../assets/favicon.ico")[..])
                .unwrap(),
        )
        .with_transparent(true),
        ..Default::default()
        
    };


    eframe::run_native(
        "main",
        fancyoptions,
        Box::new(|cc| 
            {
                egui_extras::install_image_loaders(&cc.egui_ctx);
                Ok(Box::new(MyApp::new(cc)))
            }),
    )

}


