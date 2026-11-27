// Hide the console window on Windows in release builds.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::process::ExitCode;

use linuxlauncher::{
    APP_ID, APP_NAME, app::LauncherApp, config::ConfigStore, cpu, session::Session,
};

fn viewport(session: &Session) -> egui::ViewportBuilder {
    let mut viewport = egui::ViewportBuilder::default()
        .with_title(APP_NAME)
        .with_app_id(APP_ID)
        .with_inner_size([1000.0, 620.0])
        .with_min_inner_size([480.0, 360.0])
        .with_decorations(false) // We draw our own title bar.
        .with_resizable(true);
    // Title bar / taskbar icon; the exe icon comes from build.rs.
    match image::load_from_memory(include_bytes!("../assets/favicon.ico")) {
        Ok(img) => {
            let rgba = img.to_rgba8();
            viewport = viewport.with_icon(egui::IconData {
                width: rgba.width(),
                height: rgba.height(),
                rgba: rgba.into_raw(),
            });
        }
        Err(err) => log::warn!("could not load window icon: {err}"),
    }
    session.apply(viewport)
}

fn main() -> ExitCode {
    env_logger::init(); // Log to stderr (set `RUST_LOG=debug`).

    let store = ConfigStore::locate();
    let session = Session::load(store.dir());
    let viewport = viewport(&session);
    match cpu::run(
        viewport,
        Box::new(move |ctx| LauncherApp::new(ctx, store, session)),
    ) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            log::error!("{e}");
            rfd::MessageDialog::new()
                .set_level(rfd::MessageLevel::Error)
                .set_title(APP_NAME)
                .set_description(format!("The window could not be created.\n\n{e}"))
                .show();
            ExitCode::FAILURE
        }
    }
}
