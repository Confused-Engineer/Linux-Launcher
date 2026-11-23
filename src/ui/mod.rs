//! egui front end. Pages render from `&Config` and report what the user did
//! as [`Action`]s, which [`crate::app::LauncherApp`] applies afterwards. This
//! keeps borrowing simple and all mutations in one place.

pub mod editor;
pub mod launcher;
pub mod settings;
pub mod tile;
pub mod toasts;
pub mod window;

use crate::config::AppEntry;
use std::path::PathBuf;

/// Something the user asked for. Indices refer to `Config::apps`.
#[derive(Debug, Clone)]
pub enum Action {
    Launch {
        index: usize,
        elevated: bool,
    },
    /// Open the editor for a new app, pre-filled with the given values.
    NewApp(AppEntry),
    Edit(usize),
    Duplicate(usize),
    /// Ask for confirmation, then remove.
    Remove(usize),
    SetFavorite(usize, bool),
    SetGroup {
        index: usize,
        group: Option<String>,
    },
    /// Move an app so it ends up at `to`. If `group` is `Some`, the app is also
    /// moved into that group (`Some(None)` = ungrouped).
    Reorder {
        from: usize,
        to: usize,
        group: Option<Option<String>>,
    },
    Reveal(usize),
    /// Files/folders dropped onto the window.
    AddPaths(Vec<PathBuf>),
    AddGroup(String),
    OpenSettings(settings::SettingsTab),
    // Data management
    OpenConfigFolder,
    EditConfigFile,
    Reload,
    Export,
    Import {
        replace: bool,
    },
    ResetToDefaults,
    RefreshIcons,
}
