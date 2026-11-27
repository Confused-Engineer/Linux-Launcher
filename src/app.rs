//! The application: owns state, renders pages and applies [`Action`]s.
//! Driven by the CPU runner in [`crate::cpu`].

use crate::config::{
    AfterLaunch, AppEntry, Config, ConfigStore, LoadOutcome, Theme, export_file, import_file,
};
use crate::icons::IconCache;
use crate::launch::{self, LaunchError};
use crate::session::{Session, WindowGeometry};
use crate::ui::editor::{Editor, EditorOutcome};
use crate::ui::launcher::{self, LauncherState, Tab};
use crate::ui::settings::{self, SettingsState, SettingsTab};
use crate::ui::toasts::Toasts;
use crate::ui::{Action, window};
use egui::{self, Context, Id, Key, KeyboardShortcut, Modifiers, RichText};
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Page {
    Launcher,
    Settings,
}

/// A pending yes/no question.
enum Confirm {
    RemoveApp(usize),
    Reset,
    ImportReplace(Box<Config>, PathBuf),
}

pub struct LauncherApp {
    store: ConfigStore,
    config: Config,
    /// Snapshot of what is on disk; any difference triggers a save.
    saved: Config,
    /// Set when the config file exists but could not be read. Saving is
    /// suspended so the user's file is never clobbered.
    load_error: Option<String>,
    page: Page,
    launcher: LauncherState,
    settings: SettingsState,
    icons: IconCache,
    toasts: Toasts,
    editor: Option<Editor>,
    confirm: Option<Confirm>,
    applied_theme: Option<Theme>,
    /// Last normal (not maximized/minimized) window placement, for `session.toml`.
    geometry: Option<WindowGeometry>,
}

impl LauncherApp {
    /// Builds the app around a config location and the previous session.
    pub fn new(ctx: &Context, store: ConfigStore, session: Session) -> Self {
        egui_extras::install_image_loaders(ctx);

        log::info!("config file: {}", store.path().display());
        let mut toasts = Toasts::default();
        let mut load_error = None;
        let config = match store.load() {
            LoadOutcome::Loaded(c) => c,
            LoadOutcome::Created(c) => {
                toasts.info("Welcome! Drag apps onto the window or click ➕ Add app.");
                c
            }
            LoadOutcome::Migrated { config, from } => {
                toasts.success(format!(
                    "Imported {} apps from {}",
                    config.apps.len(),
                    from.display()
                ));
                config
            }
            LoadOutcome::Invalid(e) => {
                load_error = Some(e.to_string());
                Config::default()
            }
        };

        let mut launcher = LauncherState::default();
        launcher.tab = session.tab;

        Self {
            store,
            saved: config.clone(),
            config,
            load_error,
            page: Page::Launcher,
            launcher,
            settings: SettingsState::default(),
            icons: IconCache::new(ctx),
            toasts,
            editor: None,
            confirm: None,
            applied_theme: None,
            geometry: session.window,
        }
    }

    fn modal_open(&self) -> bool {
        self.editor.is_some() || self.confirm.is_some()
    }

    fn apply_theme(&mut self, ctx: &Context) {
        let theme = self.config.settings.theme;
        if self.applied_theme != Some(theme) {
            ctx.set_theme(match theme {
                Theme::System => egui::ThemePreference::System,
                Theme::Dark => egui::ThemePreference::Dark,
                Theme::Light => egui::ThemePreference::Light,
            });
            self.applied_theme = Some(theme);
        }
    }

    fn global_shortcuts(&mut self, ctx: &Context, actions: &mut Vec<Action>) {
        if self.modal_open() {
            return;
        }
        let new_app = KeyboardShortcut::new(Modifiers::COMMAND, Key::N);
        let settings = KeyboardShortcut::new(Modifiers::COMMAND, Key::Comma);
        ctx.input_mut(|i| {
            if i.consume_shortcut(&new_app) {
                actions.push(Action::NewApp(AppEntry {
                    group: match &self.launcher.tab {
                        Tab::Group(g) => Some(g.clone()),
                        _ => None,
                    },
                    ..AppEntry::default()
                }));
            }
            if i.consume_shortcut(&settings) {
                self.page = match self.page {
                    Page::Launcher => Page::Settings,
                    Page::Settings => Page::Launcher,
                };
            }
            if self.page == Page::Settings && i.consume_key(Modifiers::NONE, Key::Escape) {
                self.page = Page::Launcher;
            }
        });
    }

    /// Files dragged from Explorer onto the window become apps.
    fn handle_file_drop(&mut self, ctx: &Context, actions: &mut Vec<Action>) {
        let (hovering, dropped) = ctx.input(|i| {
            let dropped: Vec<PathBuf> = i
                .raw
                .dropped_files
                .iter()
                .map(|f| f.path().to_path_buf())
                .filter(|p| !p.as_os_str().is_empty())
                .collect();
            (!i.raw.hovered_files.is_empty(), dropped)
        });
        if !dropped.is_empty() {
            actions.push(Action::AddPaths(dropped));
        }
        if hovering {
            let painter = ctx.layer_painter(egui::LayerId::new(
                egui::Order::Foreground,
                Id::unique("file-drop"),
            ));
            let rect = ctx.content_rect().shrink(4.0);
            painter.rect_filled(rect, 10.0, egui::Color32::from_black_alpha(160));
            painter.text(
                rect.center(),
                egui::Align2::CENTER_CENTER,
                "Drop to add to the launcher",
                egui::FontId::proportional(24.0),
                egui::Color32::WHITE,
            );
        }
    }

    fn current_group(&self) -> Option<String> {
        match &self.launcher.tab {
            Tab::Group(g) if self.page == Page::Launcher => Some(g.clone()),
            _ => None,
        }
    }

    fn apply(&mut self, ctx: &Context, action: Action) {
        match action {
            Action::Launch { index, elevated } => self.launch(ctx, index, elevated),
            Action::NewApp(preset) => self.editor = Some(Editor::new_app(preset)),
            Action::Edit(index) => {
                if let Some(app) = self.config.apps.get(index) {
                    self.editor = Some(Editor::edit(index, app.clone()));
                }
            }
            Action::Duplicate(index) => {
                if let Some(app) = self.config.apps.get(index) {
                    let mut copy = app.clone();
                    copy.name = format!("{} (copy)", copy.name);
                    let new = self.config.add_app(copy);
                    self.config.move_app(new, index + 1);
                }
            }
            Action::Remove(index) => self.confirm = Some(Confirm::RemoveApp(index)),
            Action::SetFavorite(index, favorite) => {
                if let Some(app) = self.config.apps.get_mut(index) {
                    app.favorite = favorite;
                }
            }
            Action::SetGroup { index, group } => self.config.set_app_group(index, group.as_deref()),
            Action::Reorder { from, to, group } => {
                self.config.move_app(from, to);
                if let Some(group) = group {
                    let to = to.min(self.config.apps.len().saturating_sub(1));
                    self.config.set_app_group(to, group.as_deref());
                }
            }
            Action::Reveal(index) => {
                if let Some(app) = self.config.apps.get(index)
                    && let Err(e) = launch::reveal(&app.target)
                {
                    self.toasts.error(format!("Couldn't open location: {e}"));
                }
            }
            Action::AddPaths(paths) => self.add_paths(paths),
            Action::AddGroup(name) => {
                if let Err(e) = self.config.add_group(&name) {
                    self.toasts.error(e.to_string());
                }
            }
            Action::OpenSettings(tab) => {
                self.settings.tab = tab;
                self.page = Page::Settings;
            }
            Action::OpenConfigFolder => self.open(self.store.dir().to_path_buf()),
            Action::EditConfigFile => self.open(self.store.path().to_path_buf()),
            Action::Reload => self.reload(),
            Action::Export => self.export(),
            Action::Import { replace } => self.import(replace),
            Action::ResetToDefaults => self.confirm = Some(Confirm::Reset),
            Action::RefreshIcons => {
                self.icons.clear(ctx);
                self.toasts.info("Icons refreshed.");
            }
        }
    }

    fn launch(&mut self, ctx: &Context, index: usize, elevated: bool) {
        let Some(app) = self.config.apps.get(index) else {
            return;
        };
        match launch::launch(app, elevated) {
            Ok(()) => {
                self.launcher.search.clear();
                match self.config.settings.after_launch {
                    AfterLaunch::Nothing => {}
                    AfterLaunch::Minimize => {
                        ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(true))
                    }
                    AfterLaunch::Close => ctx.send_viewport_cmd(egui::ViewportCommand::Close),
                }
            }
            Err(LaunchError::Cancelled) => {}
            Err(e) => self
                .toasts
                .error(format!("Couldn't open \"{}\": {e}", app.name)),
        }
    }

    fn add_paths(&mut self, paths: Vec<PathBuf>) {
        let group = self.current_group();
        if let [path] = paths.as_slice() {
            // A single item: let the user review it first.
            self.editor = Some(Editor::new_app(AppEntry {
                group,
                ..AppEntry::from_path(path)
            }));
            return;
        }
        for path in &paths {
            self.config.add_app(AppEntry {
                group: group.clone(),
                ..AppEntry::from_path(path)
            });
        }
        self.toasts.success(format!("Added {} apps.", paths.len()));
    }

    fn open(&mut self, path: PathBuf) {
        if let Err(e) = launch::open_path(&path) {
            self.toasts
                .error(format!("Couldn't open {}: {e}", path.display()));
        }
    }

    fn reload(&mut self) {
        match self.store.load() {
            LoadOutcome::Loaded(c)
            | LoadOutcome::Created(c)
            | LoadOutcome::Migrated { config: c, .. } => {
                self.saved = c.clone();
                self.config = c;
                self.load_error = None;
                self.toasts.success("Configuration reloaded.");
            }
            LoadOutcome::Invalid(e) => {
                self.toasts
                    .error(format!("Still can't read the config: {e}"));
                self.load_error = Some(e.to_string());
            }
        }
    }

    fn export(&mut self) {
        let Some(path) = rfd::FileDialog::new()
            .set_file_name("linux-launcher-backup.toml")
            .add_filter("Linux Launcher config", &["toml"])
            .save_file()
        else {
            return;
        };
        match export_file(&self.config, &path) {
            Ok(()) => self
                .toasts
                .success(format!("Exported to {}", path.display())),
            Err(e) => self.toasts.error(format!("Export failed: {e}")),
        }
    }

    fn import(&mut self, replace: bool) {
        let Some(path) = rfd::FileDialog::new()
            .add_filter("Linux Launcher config", &["toml", "ini"])
            .pick_file()
        else {
            return;
        };
        match import_file(&path) {
            Ok(imported) if replace => {
                self.confirm = Some(Confirm::ImportReplace(Box::new(imported), path))
            }
            Ok(imported) => {
                let n = imported.apps.len();
                self.config.merge(imported);
                self.toasts.success(format!("Imported {n} apps."));
            }
            Err(e) => self.toasts.error(format!("Import failed: {e}")),
        }
    }

    fn reset(&mut self) {
        if self.load_error.is_some() {
            match self.store.quarantine() {
                Ok(moved) => self
                    .toasts
                    .info(format!("Old config kept as {}", moved.display())),
                Err(e) => {
                    self.toasts
                        .error(format!("Couldn't move the broken config aside: {e}"));
                    return;
                }
            }
            self.load_error = None;
        }
        self.config = Config::starter();
        self.saved = Config::default(); // force a save
        self.toasts.success("Reset to defaults.");
    }

    fn show_editor(&mut self, ctx: &Context) {
        let Some(editor) = &mut self.editor else {
            return;
        };
        match editor.show(ctx, &self.config.groups, &mut self.icons) {
            EditorOutcome::Open => {}
            EditorOutcome::Cancel => self.editor = None,
            EditorOutcome::Test => match editor.result() {
                Ok(app) => match launch::launch(&app, false) {
                    Ok(()) | Err(LaunchError::Cancelled) => {}
                    Err(e) => editor.set_error(format!("Launch failed: {e}")),
                },
                Err(e) => editor.set_error(e),
            },
            EditorOutcome::Save => {
                let Ok(app) = editor.result() else { return };
                let name = app.name.clone();
                match editor.index {
                    Some(index) if index < self.config.apps.len() => {
                        let group = app.group.clone();
                        self.config.apps[index] = app;
                        self.config.set_app_group(index, group.as_deref());
                    }
                    _ => {
                        self.config.add_app(app);
                        self.toasts.success(format!("Added \"{name}\"."));
                    }
                }
                self.editor = None;
            }
        }
    }

    fn show_confirm(&mut self, ctx: &Context) {
        let Some(confirm) = &self.confirm else {
            return;
        };
        let (title, body, ok) = match confirm {
            Confirm::RemoveApp(i) => (
                "Remove app?",
                format!(
                    "Remove \"{}\" from the launcher? The program itself is not affected.",
                    self.config.apps.get(*i).map_or("", |a| a.name.as_str())
                ),
                "Remove",
            ),
            Confirm::Reset => (
                "Reset to defaults?",
                "This replaces all your apps, groups and settings with the starter configuration. Consider exporting a backup first.".to_owned(),
                "Reset",
            ),
            Confirm::ImportReplace(c, path) => (
                "Replace configuration?",
                format!(
                    "Replace everything with the {} apps in {}?",
                    c.apps.len(),
                    path.display()
                ),
                "Replace",
            ),
        };

        let mut decision = None;
        let modal = egui::Modal::new(Id::unique("confirm")).show(ctx, |ui| {
            ui.set_max_width(380.0);
            ui.heading(title);
            ui.add_space(6.0);
            ui.label(body);
            ui.add_space(10.0);
            ui.horizontal(|ui| {
                if ui
                    .button(RichText::new(ok).color(ui.visuals().error_fg_color))
                    .clicked()
                {
                    decision = Some(true);
                }
                if ui.button("Cancel").clicked() {
                    decision = Some(false);
                }
            });
        });
        if modal.should_close() && decision.is_none() {
            decision = Some(false);
        }

        if decision == Some(true) {
            match self.confirm.take() {
                Some(Confirm::RemoveApp(i)) if i < self.config.apps.len() => {
                    let app = self.config.apps.remove(i);
                    self.toasts.info(format!("Removed \"{}\".", app.name));
                }
                Some(Confirm::Reset) => self.reset(),
                Some(Confirm::ImportReplace(c, _)) => {
                    self.config = *c;
                    self.toasts.success("Configuration replaced.");
                }
                _ => {}
            }
        } else if decision == Some(false) {
            self.confirm = None;
        }
    }

    fn load_error_banner(&mut self, ui: &mut egui::Ui, actions: &mut Vec<Action>) {
        let Some(error) = &self.load_error else {
            return;
        };
        egui::Frame::new()
            .fill(ui.visuals().error_fg_color.gamma_multiply(0.15))
            .corner_radius(6)
            .inner_margin(8)
            .show(ui, |ui| {
                ui.colored_label(
                    ui.visuals().error_fg_color,
                    "⚠ Your config file couldn't be read, so changes won't be saved until it's fixed.",
                );
                ui.label(RichText::new(error).monospace().small());
                ui.horizontal(|ui| {
                    if ui.button("📝 Edit file").clicked() {
                        actions.push(Action::EditConfigFile);
                    }
                    if ui.button("⟳ Reload").clicked() {
                        actions.push(Action::Reload);
                    }
                    if ui.button("Reset to defaults…").clicked() {
                        actions.push(Action::ResetToDefaults);
                    }
                });
            });
        ui.add_space(6.0);
    }

    fn autosave(&mut self) {
        if self.load_error.is_some() || self.config == self.saved {
            return;
        }
        self.config.normalize();
        if let Err(e) = self.store.save(&self.config) {
            self.toasts.error(format!("Couldn't save settings: {e}"));
        }
        // Record the attempt either way so a failing disk doesn't spam errors;
        // the next change retries.
        self.saved = self.config.clone();
    }
}

/// Entry points called by the runner each frame / at exit.
impl LauncherApp {
    /// Non-UI work run before [`Self::ui`] each frame.
    pub fn logic(&mut self, ctx: &Context) {
        self.icons.poll(ctx);
        self.apply_theme(ctx);
        self.track_geometry(ctx);
    }

    pub fn ui(&mut self, ui: &mut egui::Ui) {
        // A cheap handle (Arc) so dialogs can use the context while `ui` is borrowed.
        let ctx = ui.ctx().clone();
        let ctx = &ctx;

        let mut actions = Vec::new();
        self.global_shortcuts(ctx, &mut actions);
        self.handle_file_drop(ctx, &mut actions);

        let title = match self.page {
            Page::Launcher => crate::APP_NAME,
            Page::Settings => "Settings",
        };
        let keyboard = !self.modal_open();
        let chrome = window::show(ui, title, self.page == Page::Settings, |ui| {
            self.load_error_banner(ui, &mut actions);
            match self.page {
                Page::Launcher => launcher::show(
                    ui,
                    &mut self.launcher,
                    &self.config,
                    &mut self.icons,
                    &mut actions,
                    keyboard,
                ),
                Page::Settings => settings::show(
                    ui,
                    &mut self.settings,
                    &mut self.config,
                    &self.store,
                    &mut self.icons,
                    &mut actions,
                    &mut self.toasts,
                ),
            }
        });
        if chrome.nav_clicked {
            self.page = match self.page {
                Page::Launcher => {
                    self.settings.tab = SettingsTab::default();
                    Page::Settings
                }
                Page::Settings => Page::Launcher,
            };
        }

        for action in actions {
            self.apply(ctx, action);
        }
        self.show_editor(ctx);
        self.show_confirm(ctx);
        self.toasts.show(ctx);
        self.autosave();
    }

    /// Saves the session (tab, window placement) when the window closes.
    pub fn on_exit(&mut self) {
        let session = Session {
            tab: self.launcher.tab.clone(),
            window: (!self.store.is_portable())
                .then_some(self.geometry)
                .flatten(),
        };
        if let Err(e) = session.save(self.store.dir()) {
            log::warn!("could not save session: {e}");
        }
    }

    /// Remembers where the window is. The normal-size placement is kept while
    /// maximized, so restoring after a maximized exit returns to that size.
    fn track_geometry(&mut self, ctx: &Context) {
        let (outer, inner, maximized, minimized) = ctx.input(|i| {
            let v = i.viewport();
            (
                v.outer_rect,
                v.inner_rect,
                v.maximized.unwrap_or(false),
                v.minimized.unwrap_or(false),
            )
        });
        if minimized {
            return;
        }
        match (outer, inner) {
            (Some(outer), Some(inner)) if !maximized => {
                self.geometry = Some(WindowGeometry {
                    x: outer.min.x,
                    y: outer.min.y,
                    width: inner.width(),
                    height: inner.height(),
                    maximized: false,
                });
            }
            _ => {
                if let Some(g) = &mut self.geometry {
                    g.maximized = maximized;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cpu::Offscreen;

    /// Renders both pages through the same CPU rasterizer the window uses.
    /// Set `LL_SNAPSHOT_DIR` to also write the frames as PNGs for review.
    #[test]
    fn renders_launcher_and_settings_on_the_cpu() {
        let dir = tempfile::tempdir().unwrap();
        let store = ConfigStore::at(dir.path().join("config.toml"));
        let mut off = Offscreen::new(1000, 620);
        let mut app = LauncherApp::new(&off.ctx, store, Session::default());

        for (page, name) in [(Page::Launcher, "launcher"), (Page::Settings, "settings")] {
            app.page = page;
            let mut img = off.frame(&mut app);
            for _ in 0..30 {
                // let background icon extraction finish
                std::thread::sleep(std::time::Duration::from_millis(30));
                img = off.frame(&mut app);
            }
            // Every pixel painted (opaque frame), and real content on top of
            // the window fill: count pixels that differ from the corner colour.
            assert!(
                img.pixels().all(|p| p[3] == 255),
                "{name}: transparent pixels"
            );
            let bg = *img.get_pixel(500, 600);
            let content = img.pixels().filter(|p| **p != bg).count();
            assert!(content > 20_000, "{name}: only {content} content pixels");
            if let Some(out) = std::env::var_os("LL_SNAPSHOT_DIR") {
                img.save(std::path::Path::new(&out).join(format!("{name}.png")))
                    .unwrap();
            }
        }
        app.on_exit();
        assert!(Session::path(dir.path()).exists());
    }
}
