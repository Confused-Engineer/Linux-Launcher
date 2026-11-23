//! Settings page: manage apps and groups, preferences, and data files.

use super::Action;
use super::toasts::Toasts;
use crate::config::{AfterLaunch, AppEntry, Config, ConfigStore, Settings, Theme};
use crate::icons::IconCache;
use eframe::egui::{self, Grid, RichText, ScrollArea, TextEdit, Ui, Vec2};

const TARGET_COLUMN_WIDTH: f32 = 360.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SettingsTab {
    #[default]
    Apps,
    Groups,
    Appearance,
    Data,
}

impl SettingsTab {
    const ALL: [Self; 4] = [Self::Apps, Self::Groups, Self::Appearance, Self::Data];

    fn label(self) -> &'static str {
        match self {
            Self::Apps => "Apps",
            Self::Groups => "Groups",
            Self::Appearance => "Appearance & behavior",
            Self::Data => "Data & backup",
        }
    }
}

#[derive(Default)]
pub struct SettingsState {
    pub tab: SettingsTab,
    /// Apps list filter: `None` = all, `Some(None)` = ungrouped.
    filter: Option<Option<String>>,
    new_group: String,
    renaming: Option<(usize, String)>,
}

/// Mutates `config` directly for simple edits; anything needing the app
/// controller (dialogs, file I/O) is pushed to `actions`.
pub fn show(
    ui: &mut Ui,
    state: &mut SettingsState,
    config: &mut Config,
    store: &ConfigStore,
    icons: &mut IconCache,
    actions: &mut Vec<Action>,
    toasts: &mut Toasts,
) {
    ui.horizontal(|ui| {
        for tab in SettingsTab::ALL {
            ui.selectable_value(&mut state.tab, tab, tab.label());
        }
    });
    ui.separator();

    ScrollArea::vertical()
        .auto_shrink(false)
        .show(ui, |ui| match state.tab {
            SettingsTab::Apps => apps_tab(ui, state, config, icons, actions),
            SettingsTab::Groups => groups_tab(ui, state, config, toasts),
            SettingsTab::Appearance => appearance_tab(ui, &mut config.settings),
            SettingsTab::Data => data_tab(ui, store, actions),
        });
}

fn apps_tab(
    ui: &mut Ui,
    state: &mut SettingsState,
    config: &mut Config,
    icons: &mut IconCache,
    actions: &mut Vec<Action>,
) {
    ui.horizontal(|ui| {
        if ui.button("➕ Add app").clicked() {
            actions.push(Action::NewApp(AppEntry {
                group: state.filter.clone().flatten(),
                ..AppEntry::default()
            }));
        }
        ui.label("Show:");
        let label = match &state.filter {
            None => "All groups".to_owned(),
            Some(None) => "No group".to_owned(),
            Some(Some(g)) => g.clone(),
        };
        egui::ComboBox::from_id_salt("apps-filter")
            .selected_text(label)
            .show_ui(ui, |ui| {
                ui.selectable_value(&mut state.filter, None, "All groups");
                for g in &config.groups {
                    ui.selectable_value(&mut state.filter, Some(Some(g.name.clone())), &g.name);
                }
                ui.selectable_value(&mut state.filter, Some(None), "No group");
            });
    });
    ui.label(
        RichText::new(
            "Order here is the order in the launcher. You can also drag tiles on the main page.",
        )
        .weak(),
    );
    ui.add_space(6.0);

    let visible: Vec<usize> = (0..config.apps.len())
        .filter(|&i| match &state.filter {
            None => true,
            Some(group) => config.apps[i].group == *group,
        })
        .collect();

    if visible.is_empty() {
        ui.label(RichText::new("No apps here.").weak());
        return;
    }

    let mut swap: Option<(usize, usize)> = None;
    Grid::new("apps-list")
        .num_columns(6)
        .striped(true)
        .spacing([10.0, 6.0])
        .show(ui, |ui| {
            for (pos, &index) in visible.iter().enumerate() {
                let app = &config.apps[index];
                ui.horizontal(|ui| {
                    let up = pos > 0;
                    let down = pos + 1 < visible.len();
                    if ui.add_enabled(up, egui::Button::new("⏶").small()).clicked() {
                        swap = Some((index, visible[pos - 1]));
                    }
                    if ui
                        .add_enabled(down, egui::Button::new("⏷").small())
                        .clicked()
                    {
                        swap = Some((index, visible[pos + 1]));
                    }
                });
                let (rect, _) = ui.allocate_exact_size(Vec2::splat(24.0), egui::Sense::hover());
                icons.paint(ui, app, rect);
                ui.label(RichText::new(&app.name).strong());
                ui.label(RichText::new(app.group.as_deref().unwrap_or("—")).weak());
                ui.add_sized(
                    [TARGET_COLUMN_WIDTH, ui.spacing().interact_size.y],
                    egui::Label::new(RichText::new(&app.target).weak()).truncate(),
                )
                .on_hover_text(&app.target);
                ui.horizontal(|ui| {
                    if ui.small_button("✏ Edit").clicked() {
                        actions.push(Action::Edit(index));
                    }
                    if ui.small_button("🗑").on_hover_text("Remove").clicked() {
                        actions.push(Action::Remove(index));
                    }
                });
                ui.end_row();
            }
        });
    if let Some((from, to)) = swap {
        config.move_app(from, to);
    }
}

fn groups_tab(ui: &mut Ui, state: &mut SettingsState, config: &mut Config, toasts: &mut Toasts) {
    ui.label(
        RichText::new(
            "Groups appear as tabs on the main page. Removing a group keeps its apps (they become ungrouped).",
        )
        .weak(),
    );
    ui.add_space(6.0);

    ui.horizontal(|ui| {
        let response = ui.add(
            TextEdit::singleline(&mut state.new_group)
                .hint_text("New group name")
                .desired_width(220.0),
        );
        let submit = response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
        if ui.button("➕ Add group").clicked() || submit {
            match config.add_group(&state.new_group) {
                Ok(()) => state.new_group.clear(),
                Err(e) => toasts.error(e.to_string()),
            }
        }
    });
    ui.add_space(6.0);

    if config.groups.is_empty() {
        ui.label(RichText::new("No groups yet.").weak());
        return;
    }

    enum Op {
        Move(usize, usize),
        Rename(usize, String),
        Remove(usize),
    }
    let mut op = None;
    let count = config.groups.len();
    Grid::new("groups-list")
        .num_columns(4)
        .striped(true)
        .spacing([10.0, 6.0])
        .show(ui, |ui| {
            for (i, group) in config.groups.iter().enumerate() {
                ui.horizontal(|ui| {
                    if ui
                        .add_enabled(i > 0, egui::Button::new("⏶").small())
                        .clicked()
                    {
                        op = Some(Op::Move(i, i - 1));
                    }
                    if ui
                        .add_enabled(i + 1 < count, egui::Button::new("⏷").small())
                        .clicked()
                    {
                        op = Some(Op::Move(i, i + 1));
                    }
                });
                match &mut state.renaming {
                    Some((idx, text)) if *idx == i => {
                        let response = ui.add(TextEdit::singleline(text).desired_width(200.0));
                        // Check `lost_focus` before (re)focusing, or Enter is swallowed.
                        if response.lost_focus() {
                            if ui.input(|inp| inp.key_pressed(egui::Key::Escape)) {
                                state.renaming = None;
                            } else {
                                op = Some(Op::Rename(i, text.clone()));
                            }
                        } else if !response.has_focus() {
                            response.request_focus();
                        }
                    }
                    _ => {
                        ui.label(RichText::new(&group.name).strong());
                    }
                }
                let apps = config.group_count(Some(&group.name));
                ui.label(
                    RichText::new(format!("{apps} app{}", if apps == 1 { "" } else { "s" })).weak(),
                );
                ui.horizontal(|ui| {
                    if ui.small_button("✏ Rename").clicked() {
                        state.renaming = Some((i, group.name.clone()));
                    }
                    if ui.small_button("🗑 Remove").clicked() {
                        op = Some(Op::Remove(i));
                    }
                });
                ui.end_row();
            }
        });

    match op {
        Some(Op::Move(from, to)) => config.move_group(from, to),
        Some(Op::Rename(i, new)) => {
            let old = config.groups[i].name.clone();
            match config.rename_group(&old, &new) {
                Ok(()) => state.renaming = None,
                Err(e) => toasts.error(e.to_string()),
            }
        }
        Some(Op::Remove(i)) => {
            let name = config.groups[i].name.clone();
            if let Err(e) = config.remove_group(&name) {
                toasts.error(e.to_string());
            } else {
                toasts.info(format!("Removed group \"{name}\"."));
            }
        }
        None => {}
    }
}

fn appearance_tab(ui: &mut Ui, settings: &mut Settings) {
    Grid::new("prefs")
        .num_columns(2)
        .spacing([16.0, 12.0])
        .show(ui, |ui| {
            ui.label("Theme");
            ui.horizontal(|ui| {
                ui.selectable_value(&mut settings.theme, Theme::System, "System");
                ui.selectable_value(&mut settings.theme, Theme::Dark, "Dark");
                ui.selectable_value(&mut settings.theme, Theme::Light, "Light");
            });
            ui.end_row();

            ui.label("Icon size");
            ui.add(
                egui::Slider::new(
                    &mut settings.icon_size,
                    Settings::MIN_ICON_SIZE..=Settings::MAX_ICON_SIZE,
                )
                .step_by(4.0)
                .suffix(" px"),
            );
            ui.end_row();

            ui.label("After launching an app");
            ui.horizontal(|ui| {
                ui.selectable_value(
                    &mut settings.after_launch,
                    AfterLaunch::Nothing,
                    "Stay open",
                );
                ui.selectable_value(
                    &mut settings.after_launch,
                    AfterLaunch::Minimize,
                    "Minimize",
                );
                ui.selectable_value(&mut settings.after_launch, AfterLaunch::Close, "Close");
            });
            ui.end_row();

            ui.label("Tabs");
            ui.checkbox(&mut settings.show_favorites, "Show a ★ Favorites tab");
            ui.end_row();

            ui.label("\"All\" tab");
            ui.checkbox(&mut settings.group_headers, "Split into sections by group");
            ui.end_row();
        });

    ui.add_space(16.0);
    ui.label(RichText::new("Keyboard shortcuts").strong());
    Grid::new("shortcuts")
        .num_columns(2)
        .spacing([16.0, 4.0])
        .show(ui, |ui| {
            for (keys, what) in [
                ("Type", "Search"),
                ("Enter", "Launch the first search result"),
                (
                    "Ctrl+Shift+Enter / Ctrl+Shift+Click",
                    "Launch as administrator",
                ),
                ("Esc", "Clear search / go back"),
                ("Ctrl+N", "Add an app"),
                ("Ctrl+,", "Settings"),
                (
                    "Right-click a tile",
                    "Edit, favorite, move to group, remove…",
                ),
                (
                    "Drag a tile",
                    "Reorder, or drop on a tab to move it into that group",
                ),
            ] {
                ui.label(RichText::new(keys).monospace());
                ui.label(what);
                ui.end_row();
            }
        });
}

fn data_tab(ui: &mut Ui, store: &ConfigStore, actions: &mut Vec<Action>) {
    ui.label(RichText::new("Configuration file").strong());
    ui.horizontal(|ui| {
        ui.label(RichText::new(store.path().display().to_string()).monospace());
        if store.is_portable() {
            ui.label(RichText::new("(portable)").weak()).on_hover_text(
                "A config.toml or 'portable' file next to the .exe keeps settings beside the app.",
            );
        }
    });
    ui.horizontal(|ui| {
        if ui.button("📂 Open folder").clicked() {
            actions.push(Action::OpenConfigFolder);
        }
        if ui
            .button("📝 Edit file")
            .on_hover_text("Open config.toml in your editor, then click Reload")
            .clicked()
        {
            actions.push(Action::EditConfigFile);
        }
        if ui
            .button("⟳ Reload")
            .on_hover_text("Re-read config.toml from disk")
            .clicked()
        {
            actions.push(Action::Reload);
        }
        if ui.button("🖼 Refresh icons").clicked() {
            actions.push(Action::RefreshIcons);
        }
    });

    ui.add_space(14.0);
    ui.label(RichText::new("Backup & restore").strong());
    ui.horizontal(|ui| {
        if ui
            .button("Export…")
            .on_hover_text("Save a copy of your apps and settings")
            .clicked()
        {
            actions.push(Action::Export);
        }
        if ui
            .button("Import (merge)…")
            .on_hover_text("Add apps from a .toml export or an old LL.ini")
            .clicked()
        {
            actions.push(Action::Import { replace: false });
        }
        if ui
            .button("Import (replace)…")
            .on_hover_text("Replace everything with a .toml export or an old LL.ini")
            .clicked()
        {
            actions.push(Action::Import { replace: true });
        }
    });

    ui.add_space(14.0);
    ui.label(RichText::new("Danger zone").strong());
    if ui.button("Reset to defaults…").clicked() {
        actions.push(Action::ResetToDefaults);
    }

    ui.add_space(24.0);
    ui.horizontal(|ui| {
        ui.label(
            RichText::new(format!(
                "{} v{}",
                crate::APP_NAME,
                env!("CARGO_PKG_VERSION")
            ))
            .weak(),
        );
        ui.hyperlink_to("GitHub", env!("CARGO_PKG_REPOSITORY"));
    });
}
