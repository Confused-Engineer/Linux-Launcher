//! UI state remembered between runs (replaces eframe's `persistence`).
//!
//! Stored as `session.toml` next to `config.toml`. Window geometry is only
//! remembered in installed mode: a portable copy moves between PCs, and
//! another PC's window position is meaningless there.

use crate::ui::launcher::Tab;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

const FILE_NAME: &str = "session.toml";

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Session {
    pub tab: Tab,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub window: Option<WindowGeometry>,
}

/// Window placement in points (logical pixels).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct WindowGeometry {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
    #[serde(default)]
    pub maximized: bool,
}

impl Session {
    pub fn path(dir: &Path) -> PathBuf {
        dir.join(FILE_NAME)
    }

    /// Loads the session; missing or unreadable files give the defaults.
    pub fn load(dir: &Path) -> Self {
        std::fs::read_to_string(Self::path(dir))
            .ok()
            .and_then(|text| toml::from_str(&text).ok())
            .unwrap_or_default()
    }

    pub fn save(&self, dir: &Path) -> std::io::Result<()> {
        let text = toml::to_string(self).map_err(std::io::Error::other)?;
        std::fs::create_dir_all(dir)?;
        std::fs::write(Self::path(dir), text)
    }

    /// Applies the remembered size/position to a viewport builder.
    pub fn apply(&self, mut viewport: egui::ViewportBuilder) -> egui::ViewportBuilder {
        if let Some(g) = self.window
            && g.width >= 200.0
            && g.height >= 150.0
        {
            viewport = viewport
                .with_inner_size([g.width, g.height])
                .with_position([g.x, g.y])
                .with_maximized(g.maximized);
        }
        viewport
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_and_tolerates_missing_or_bad_files() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(Session::load(dir.path()), Session::default());

        let s = Session {
            tab: Tab::Group("Dev".into()),
            window: Some(WindowGeometry {
                x: 10.0,
                y: 20.0,
                width: 900.0,
                height: 600.0,
                maximized: true,
            }),
        };
        s.save(dir.path()).unwrap();
        assert_eq!(Session::load(dir.path()), s);

        std::fs::write(Session::path(dir.path()), "tab = 42").unwrap();
        assert_eq!(Session::load(dir.path()), Session::default());
    }
}
