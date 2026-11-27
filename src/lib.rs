//! Linux Launcher: a Linux-inspired, grid-style application launcher for Windows.
//!
//! The crate is split into a UI-free core (configuration, search, launching,
//! icon resolution) and an egui front end under [`ui`], wired together by
//! [`app::LauncherApp`].

pub mod app;
pub mod config;
pub mod cpu;
pub mod icons;
pub mod launch;
pub mod search;
pub mod session;
pub mod ui;

/// Human-readable application name.
pub const APP_NAME: &str = "Linux Launcher";
/// Identifier used for config/storage folders and the window class.
pub const APP_ID: &str = "LinuxLauncher";
