// Hide the console window on Windows in release builds.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use eframe::egui;
use linuxlauncher::{APP_ID, APP_NAME, app::LauncherApp};

fn main() -> eframe::Result {
    env_logger::init(); // Log to stderr (set `RUST_LOG=debug`).

    let mut viewport = egui::ViewportBuilder::default()
        .with_title(APP_NAME)
        .with_app_id(APP_ID)
        .with_inner_size([1000.0, 620.0])
        .with_min_inner_size([480.0, 360.0])
        .with_decorations(false) // We draw our own title bar.
        .with_transparent(true) // Needed for rounded window corners.
        .with_resizable(true);

    match eframe::icon_data::from_png_bytes(include_bytes!("../assets/favicon.ico")) {
        Ok(icon) => viewport = viewport.with_icon(icon),
        Err(err) => log::warn!("could not load window icon: {err}"),
    }

    let options = eframe::NativeOptions {
        viewport,
        persist_window: true,
        ..Default::default()
    };

    eframe::run_native(
        APP_ID,
        options,
        Box::new(|cc| Ok(Box::new(LauncherApp::new(cc)))),
    )
}
