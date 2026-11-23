//! Configuration model: apps, groups and user preferences.
//!
//! The on-disk format is TOML (see `assets/default-config.toml` for an
//! annotated example). Everything in this module is UI-free and unit tested.

mod legacy;
mod store;

pub use legacy::{LegacyError, from_legacy_ini};
pub use store::{ConfigStore, LoadOutcome, StoreError, export_file, import_file};

use serde::{Deserialize, Serialize};
use std::path::Path;

/// Current schema version written to new config files.
pub const CONFIG_VERSION: u32 = 1;

const DEFAULT_CONFIG: &str = include_str!("../../assets/default-config.toml");

/// The whole configuration file.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    pub version: u32,
    pub settings: Settings,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub groups: Vec<Group>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub apps: Vec<AppEntry>,
}

impl Default for Config {
    /// An empty configuration (no apps, no groups).
    fn default() -> Self {
        Self {
            version: CONFIG_VERSION,
            settings: Settings::default(),
            groups: Vec::new(),
            apps: Vec::new(),
        }
    }
}

/// A named group used to organise apps into tabs/sections.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Group {
    pub name: String,
}

impl Group {
    pub fn new(name: impl Into<String>) -> Self {
        Self { name: name.into() }
    }
}

/// A single launchable entry.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct AppEntry {
    /// Label shown under the icon.
    pub name: String,
    /// Executable, shortcut, document, folder, URL or URI to open.
    pub target: String,
    /// Command-line arguments passed verbatim to the target.
    #[serde(skip_serializing_if = "String::is_empty")]
    pub args: String,
    /// Start-in directory. Defaults to the target's folder when empty.
    #[serde(skip_serializing_if = "String::is_empty")]
    pub working_dir: String,
    /// Image path/URL, or a file whose shell icon should be used.
    #[serde(skip_serializing_if = "String::is_empty")]
    pub icon: String,
    /// Name of the [`Group`] this app belongs to.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub group: Option<String>,
    /// Extra search keywords.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,
    #[serde(skip_serializing_if = "is_false")]
    pub favorite: bool,
    #[serde(skip_serializing_if = "is_false")]
    pub run_as_admin: bool,
}

fn is_false(b: &bool) -> bool {
    !*b
}

impl AppEntry {
    pub fn new(name: impl Into<String>, target: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            target: target.into(),
            ..Default::default()
        }
    }

    /// Builds an entry from a file or folder path (e.g. one dropped on the window).
    pub fn from_path(path: &Path) -> Self {
        Self::new(
            name_from_target(&path.to_string_lossy()),
            path.to_string_lossy(),
        )
    }

    /// Checks that the entry has the fields required to be launched.
    pub fn validate(&self) -> Result<(), String> {
        if self.name.trim().is_empty() {
            return Err("Name is required.".into());
        }
        if self.target.trim().is_empty() {
            return Err("Target is required.".into());
        }
        Ok(())
    }

    /// Trims whitespace and drops empty optional values.
    pub fn normalize(&mut self) {
        trim_in_place(&mut self.name);
        trim_in_place(&mut self.target);
        trim_in_place(&mut self.args);
        trim_in_place(&mut self.working_dir);
        trim_in_place(&mut self.icon);
        self.group = self
            .group
            .take()
            .map(|g| g.trim().to_owned())
            .filter(|g| !g.is_empty());
        self.tags = self
            .tags
            .iter()
            .map(|t| t.trim().to_owned())
            .filter(|t| !t.is_empty())
            .collect();
    }
}

fn trim_in_place(s: &mut String) {
    let trimmed = s.trim();
    if trimmed.len() != s.len() {
        *s = trimmed.to_owned();
    }
}

/// Derives a friendly display name from a target path or URL.
///
/// `C:\Tools\putty.exe` → `putty`, `https://www.github.com/x` → `github.com`.
pub fn name_from_target(target: &str) -> String {
    let target = target.trim().trim_end_matches(['\\', '/']);
    if let Some((_, rest)) = target.split_once("://") {
        let host = rest.split(['/', '?', '#']).next().unwrap_or(rest);
        return host.trim_start_matches("www.").to_owned();
    }
    let path = Path::new(target);
    path.file_stem()
        .or_else(|| path.file_name())
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| target.to_owned())
}

/// User preferences.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    /// Edge length of app icons, in points.
    pub icon_size: f32,
    pub theme: Theme,
    pub after_launch: AfterLaunch,
    /// Show a "Favorites" tab when at least one app is a favorite.
    pub show_favorites: bool,
    /// In the "All" tab, split apps into one section per group.
    pub group_headers: bool,
}

impl Settings {
    pub const MIN_ICON_SIZE: f32 = 40.0;
    pub const MAX_ICON_SIZE: f32 = 160.0;

    pub fn icon_size(&self) -> f32 {
        self.icon_size
            .clamp(Self::MIN_ICON_SIZE, Self::MAX_ICON_SIZE)
    }
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            icon_size: 80.0,
            theme: Theme::System,
            after_launch: AfterLaunch::Nothing,
            show_favorites: true,
            group_headers: true,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Theme {
    #[default]
    System,
    Dark,
    Light,
}

/// What the launcher window does after successfully starting an app.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AfterLaunch {
    #[default]
    Nothing,
    Minimize,
    Close,
}

/// Errors from group operations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GroupError {
    EmptyName,
    Duplicate(String),
    NotFound(String),
}

impl std::fmt::Display for GroupError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::EmptyName => write!(f, "Group name cannot be empty."),
            Self::Duplicate(n) => write!(f, "A group named \"{n}\" already exists."),
            Self::NotFound(n) => write!(f, "No group named \"{n}\"."),
        }
    }
}

impl std::error::Error for GroupError {}

impl Config {
    /// The starter configuration shipped with the app.
    pub fn starter() -> Self {
        let mut config: Self =
            toml::from_str(DEFAULT_CONFIG).expect("bundled default config must be valid");
        config.normalize();
        config
    }

    /// Parses a TOML config and normalizes it.
    pub fn from_toml(text: &str) -> Result<Self, toml::de::Error> {
        let mut config: Self = toml::from_str(text)?;
        config.normalize();
        Ok(config)
    }

    pub fn to_toml(&self) -> String {
        toml::to_string_pretty(self).expect("config is always serializable")
    }

    /// Repairs inconsistencies: trims values, removes duplicate/empty groups
    /// and registers groups referenced by apps but missing from `groups`.
    pub fn normalize(&mut self) {
        self.version = CONFIG_VERSION;
        for app in &mut self.apps {
            app.normalize();
        }

        let mut seen: Vec<String> = Vec::new();
        self.groups.retain_mut(|g| {
            trim_in_place(&mut g.name);
            let key = g.name.to_lowercase();
            if g.name.is_empty() || seen.contains(&key) {
                return false;
            }
            seen.push(key);
            true
        });

        for app in &mut self.apps {
            let Some(group) = &app.group else { continue };
            // Snap to the canonical spelling if a group differs only by case.
            if let Some(existing) = self
                .groups
                .iter()
                .find(|g| g.name.eq_ignore_ascii_case(group))
            {
                app.group = Some(existing.name.clone());
            } else {
                self.groups.push(Group::new(group.clone()));
            }
        }
    }

    pub fn group_index(&self, name: &str) -> Option<usize> {
        self.groups
            .iter()
            .position(|g| g.name.eq_ignore_ascii_case(name))
    }

    /// Number of apps in each group (and ungrouped), keyed by group name.
    pub fn group_count(&self, name: Option<&str>) -> usize {
        self.apps
            .iter()
            .filter(|a| a.group.as_deref() == name)
            .count()
    }

    pub fn add_group(&mut self, name: &str) -> Result<(), GroupError> {
        let name = name.trim();
        if name.is_empty() {
            return Err(GroupError::EmptyName);
        }
        if self.group_index(name).is_some() {
            return Err(GroupError::Duplicate(name.to_owned()));
        }
        self.groups.push(Group::new(name));
        Ok(())
    }

    /// Renames a group and moves its apps along with it.
    pub fn rename_group(&mut self, old: &str, new: &str) -> Result<(), GroupError> {
        let new = new.trim();
        if new.is_empty() {
            return Err(GroupError::EmptyName);
        }
        let idx = self
            .group_index(old)
            .ok_or_else(|| GroupError::NotFound(old.to_owned()))?;
        if let Some(other) = self.group_index(new)
            && other != idx
        {
            return Err(GroupError::Duplicate(new.to_owned()));
        }
        let old_name = std::mem::replace(&mut self.groups[idx].name, new.to_owned());
        for app in &mut self.apps {
            if app.group.as_deref() == Some(old_name.as_str()) {
                app.group = Some(new.to_owned());
            }
        }
        Ok(())
    }

    /// Removes a group. Its apps are kept and become ungrouped.
    pub fn remove_group(&mut self, name: &str) -> Result<(), GroupError> {
        let idx = self
            .group_index(name)
            .ok_or_else(|| GroupError::NotFound(name.to_owned()))?;
        let removed = self.groups.remove(idx);
        for app in &mut self.apps {
            if app.group.as_deref() == Some(removed.name.as_str()) {
                app.group = None;
            }
        }
        Ok(())
    }

    pub fn move_group(&mut self, from: usize, to: usize) {
        move_item(&mut self.groups, from, to);
    }

    /// Moves an app so that it ends up at index `to`.
    pub fn move_app(&mut self, from: usize, to: usize) {
        move_item(&mut self.apps, from, to);
    }

    /// Assigns an app to a group, creating the group if necessary.
    pub fn set_app_group(&mut self, index: usize, group: Option<&str>) {
        let group = group.map(str::trim).filter(|g| !g.is_empty());
        let group = group.map(|g| match self.group_index(g) {
            Some(i) => self.groups[i].name.clone(),
            None => {
                self.groups.push(Group::new(g));
                g.to_owned()
            }
        });
        if let Some(app) = self.apps.get_mut(index) {
            app.group = group;
        }
    }

    /// Inserts a new app, registering its group if needed. Returns its index.
    pub fn add_app(&mut self, mut app: AppEntry) -> usize {
        app.normalize();
        let group = app.group.take();
        self.apps.push(app);
        let index = self.apps.len() - 1;
        self.set_app_group(index, group.as_deref());
        index
    }

    /// Appends all apps and groups from `other` (used by "Import → Merge").
    pub fn merge(&mut self, other: Config) {
        for group in other.groups {
            let _ = self.add_group(&group.name);
        }
        for app in other.apps {
            self.add_app(app);
        }
    }
}

/// Moves `v[from]` so that it ends up at index `to` (clamped), shifting the rest.
pub fn move_item<T>(v: &mut Vec<T>, from: usize, to: usize) {
    if from >= v.len() || from == to {
        return;
    }
    let item = v.remove(from);
    let to = to.min(v.len());
    v.insert(to, item);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> Config {
        let mut c = Config::default();
        c.add_app(AppEntry {
            group: Some("Tools".into()),
            ..AppEntry::new("A", "a.exe")
        });
        c.add_app(AppEntry::new("B", "b.exe"));
        c.add_app(AppEntry {
            group: Some("tools".into()),
            ..AppEntry::new("C", "c.exe")
        });
        c
    }

    #[test]
    fn starter_config_parses_and_round_trips() {
        let c = Config::starter();
        assert!(!c.apps.is_empty());
        assert!(!c.groups.is_empty());
        let again = Config::from_toml(&c.to_toml()).unwrap();
        assert_eq!(c, again);
    }

    #[test]
    fn empty_and_partial_configs_use_defaults() {
        let c = Config::from_toml("").unwrap();
        assert_eq!(c, Config::default());

        let c = Config::from_toml("[[apps]]\nname = \"X\"\ntarget = \"x.exe\"\n").unwrap();
        assert_eq!(c.apps.len(), 1);
        assert_eq!(c.settings, Settings::default());
    }

    #[test]
    fn serialization_omits_empty_fields() {
        let mut c = Config::default();
        c.add_app(AppEntry::new("X", "x.exe"));
        let text = c.to_toml();
        assert!(!text.contains("args ="));
        assert!(!text.contains("favorite ="));
        assert!(!text.contains("group ="));
    }

    #[test]
    fn add_app_registers_groups_case_insensitively() {
        let c = sample();
        assert_eq!(c.groups, vec![Group::new("Tools")]);
        assert_eq!(c.apps[2].group.as_deref(), Some("Tools"));
    }

    #[test]
    fn normalize_dedupes_groups_and_adds_missing() {
        let mut c = Config {
            groups: vec![Group::new(" Web "), Group::new("web"), Group::new("")],
            apps: vec![AppEntry {
                group: Some("Games".into()),
                ..AppEntry::new(" X ", "x")
            }],
            ..Config::default()
        };
        c.normalize();
        assert_eq!(c.groups, vec![Group::new("Web"), Group::new("Games")]);
        assert_eq!(c.apps[0].name, "X");
    }

    #[test]
    fn rename_group_moves_apps() {
        let mut c = sample();
        c.rename_group("tools", "Utilities").unwrap();
        assert_eq!(c.groups[0].name, "Utilities");
        assert_eq!(c.group_count(Some("Utilities")), 2);
        assert_eq!(c.group_count(Some("Tools")), 0);
    }

    #[test]
    fn rename_group_rejects_duplicates_but_allows_case_change() {
        let mut c = sample();
        c.add_group("Web").unwrap();
        assert_eq!(
            c.rename_group("Tools", "web"),
            Err(GroupError::Duplicate("web".into()))
        );
        c.rename_group("Tools", "TOOLS").unwrap();
        assert_eq!(c.groups[0].name, "TOOLS");
    }

    #[test]
    fn remove_group_ungroups_apps() {
        let mut c = sample();
        c.remove_group("Tools").unwrap();
        assert!(c.groups.is_empty());
        assert!(c.apps.iter().all(|a| a.group.is_none()));
        assert_eq!(c.apps.len(), 3);
    }

    #[test]
    fn add_group_validates() {
        let mut c = sample();
        assert_eq!(c.add_group("  "), Err(GroupError::EmptyName));
        assert!(matches!(
            c.add_group("tools"),
            Err(GroupError::Duplicate(_))
        ));
    }

    #[test]
    fn move_item_semantics() {
        let mut v = vec![0, 1, 2, 3];
        move_item(&mut v, 0, 2);
        assert_eq!(v, [1, 2, 0, 3]);
        move_item(&mut v, 3, 0);
        assert_eq!(v, [3, 1, 2, 0]);
        move_item(&mut v, 1, 99);
        assert_eq!(v, [3, 2, 0, 1]);
        move_item(&mut v, 99, 0);
        assert_eq!(v, [3, 2, 0, 1]);
    }

    #[test]
    fn merge_appends() {
        let mut a = sample();
        let mut b = Config::default();
        b.add_app(AppEntry {
            group: Some("Web".into()),
            ..AppEntry::new("W", "https://x")
        });
        a.merge(b);
        assert_eq!(a.apps.len(), 4);
        assert_eq!(a.groups.len(), 2);
    }

    #[test]
    fn names_from_targets() {
        assert_eq!(name_from_target(r"C:\Tools\putty.exe"), "putty");
        assert_eq!(name_from_target(r"C:\Users\me\Desktop\Notes.lnk"), "Notes");
        assert_eq!(name_from_target(r"C:\Projects\"), "Projects");
        assert_eq!(name_from_target("https://www.github.com/foo"), "github.com");
        assert_eq!(name_from_target("ms-settings:"), "ms-settings:");
    }

    #[test]
    fn validate_requires_name_and_target() {
        assert!(AppEntry::new("", "x").validate().is_err());
        assert!(AppEntry::new("x", " ").validate().is_err());
        assert!(AppEntry::new("x", "y").validate().is_ok());
    }
}
