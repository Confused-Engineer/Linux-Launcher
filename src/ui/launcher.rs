//! The main page: search box, group tabs and the app grid.

use super::Action;
use super::settings::SettingsTab;
use super::tile::{self, TileState};
use crate::config::{AppEntry, Config};
use crate::icons::IconCache;
use crate::launch;
use crate::search;
use egui::{
    self, Align, Align2, Id, Key, Layout, PopupCloseBehavior, RichText, ScrollArea, Sense, Stroke,
    StrokeKind, TextEdit, Ui,
    containers::menu::{MenuButton, MenuConfig},
};
use serde::{Deserialize, Serialize};

/// Which subset of apps is shown.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum Tab {
    #[default]
    All,
    Favorites,
    /// Apps that are not in any group.
    Ungrouped,
    Group(String),
}

impl Tab {
    /// The group new apps should land in when added from this tab.
    fn group(&self) -> Option<String> {
        match self {
            Self::Group(g) => Some(g.clone()),
            _ => None,
        }
    }
}

#[derive(Default)]
pub struct LauncherState {
    pub tab: Tab,
    pub search: String,
    pub new_group: String,
    /// Whether the "new group" menu was open last frame (to focus its field once).
    new_group_menu_open: bool,
}

/// Payload carried while a tile is being dragged.
#[derive(Debug, Clone, Copy)]
struct DraggedApp(usize);

/// How dropping a tile onto another tile affects grouping.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DropMode {
    /// Reordering is not meaningful (search results).
    Disabled,
    /// Reorder only; keep the dragged app's group (Favorites tab).
    KeepGroup,
    /// Reorder and adopt the target app's group.
    AdoptGroup,
}

struct Section<'a> {
    title: Option<String>,
    apps: Vec<usize>,
    /// Group assigned when an app is dropped on the section header.
    group: Option<Option<String>>,
    empty_hint: &'a str,
}

/// Draws the launcher page. `keyboard` is false while a dialog is open.
pub fn show(
    ui: &mut Ui,
    state: &mut LauncherState,
    config: &Config,
    icons: &mut IconCache,
    actions: &mut Vec<Action>,
    keyboard: bool,
) {
    ensure_valid_tab(state, config);

    let searching = !state.search.trim().is_empty();
    let results = if searching {
        search::search(&config.apps, &state.search)
    } else {
        Vec::new()
    };

    search_bar(ui, state, &results, actions, keyboard);
    ui.add_space(6.0);
    if !searching {
        tab_bar(ui, state, config, actions);
    }
    ui.separator();

    let sections = if searching {
        vec![Section {
            title: None,
            apps: results,
            group: None,
            empty_hint: "Try a different word. Search covers names, groups, tags and paths.",
        }]
    } else {
        sections_for_tab(&state.tab, config)
    };
    let drop_mode = match (&state.tab, searching) {
        (_, true) => DropMode::Disabled,
        (Tab::Favorites, _) => DropMode::KeepGroup,
        _ => DropMode::AdoptGroup,
    };

    ScrollArea::vertical()
        .auto_shrink(false)
        .show(ui, |ui| {
            if config.apps.is_empty() {
                empty_state(
                    ui,
                    "No apps yet",
                    "Drag programs, shortcuts, files or folders onto this window,\nor click \"➕ Add app\".",
                );
                return;
            }
            let multiple = sections.len() > 1;
            for (i, section) in sections.iter().enumerate() {
                if let Some(title) = &section.title {
                    if i > 0 {
                        ui.add_space(8.0);
                    }
                    section_header(ui, title, section, actions);
                }
                if section.apps.is_empty() {
                    if !multiple {
                        let title = if searching { "No results" } else { "Nothing here yet" };
                        empty_state(ui, title, section.empty_hint);
                    }
                    continue;
                }
                grid(ui, config, &section.apps, icons, actions, drop_mode, searching);
            }
        });

    drag_ghost(ui, config);
}

fn ensure_valid_tab(state: &mut LauncherState, config: &Config) {
    let valid = match &state.tab {
        Tab::All => true,
        Tab::Favorites => config.settings.show_favorites,
        Tab::Ungrouped => !config.groups.is_empty(),
        Tab::Group(g) => config.group_index(g).is_some(),
    };
    if !valid {
        state.tab = Tab::All;
    }
}

fn search_bar(
    ui: &mut Ui,
    state: &mut LauncherState,
    results: &[usize],
    actions: &mut Vec<Action>,
    keyboard: bool,
) {
    const ROW_HEIGHT: f32 = 30.0;
    let row = egui::vec2(ui.available_width(), ROW_HEIGHT);
    ui.allocate_ui_with_layout(row, Layout::right_to_left(Align::Center), |ui| {
        // Right-to-left: the button claims its space first, search fills the rest.
        let add = ui
            .add_sized([100.0, ROW_HEIGHT], egui::Button::new("➕ Add app"))
            .on_hover_text(
                "Add a program, shortcut, folder or website (Ctrl+N)\n\
                 Tip: you can also drag files onto the window.",
            );
        if add.clicked() {
            actions.push(Action::NewApp(AppEntry {
                group: state.tab.group(),
                ..AppEntry::default()
            }));
        }

        let response = ui.add_sized(
            [ui.available_width(), ROW_HEIGHT],
            TextEdit::singleline(&mut state.search)
                .id(Id::unique("search"))
                .hint_text("🔍  Type to search… (Enter launches the first result)")
                .align(Align2::LEFT_CENTER),
        );

        if keyboard {
            let (enter, escape) =
                ui.input(|i| (i.key_pressed(Key::Enter), i.key_pressed(Key::Escape)));
            if response.lost_focus()
                && enter
                && let Some(&index) = results.first()
            {
                actions.push(Action::Launch {
                    index,
                    elevated: ui.input(|i| i.modifiers.ctrl && i.modifiers.shift),
                });
            }
            if escape {
                state.search.clear();
            }
            // Typing anywhere goes to the search box.
            if ui.memory(|m| m.focused().is_none()) {
                response.request_focus();
            }
        }
    });
}

fn tab_bar(ui: &mut Ui, state: &mut LauncherState, config: &Config, actions: &mut Vec<Action>) {
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing.x = 4.0;
        tab_button(ui, state, Tab::All, "All", config.apps.len(), actions);

        let favorites = config.apps.iter().filter(|a| a.favorite).count();
        if config.settings.show_favorites {
            tab_button(ui, state, Tab::Favorites, "★ Favorites", favorites, actions);
        }
        for group in &config.groups {
            let count = config.group_count(Some(&group.name));
            tab_button(
                ui,
                state,
                Tab::Group(group.name.clone()),
                &group.name,
                count,
                actions,
            );
        }
        if !config.groups.is_empty() {
            let ungrouped = config.group_count(None);
            if ungrouped > 0 || state.tab == Tab::Ungrouped {
                tab_button(ui, state, Tab::Ungrouped, "Other", ungrouped, actions);
            }
        }

        // Contains a text field, so it must stay open while clicking inside it
        // (menus close on any click by default).
        let new_group_menu = MenuButton::new("➕")
            .config(MenuConfig::new().close_behavior(PopupCloseBehavior::CloseOnClickOutside));
        let (response, menu) = new_group_menu.ui(ui, |ui| {
            ui.label("New group");
            let response = ui.add(
                TextEdit::singleline(&mut state.new_group)
                    .hint_text("Name")
                    .desired_width(160.0),
            );
            // Take focus from the search box once, when the menu opens.
            if !state.new_group_menu_open {
                response.request_focus();
            }
            let submit = response.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter));
            if (ui.button("Create").clicked() || submit) && !state.new_group.trim().is_empty() {
                let name = std::mem::take(&mut state.new_group).trim().to_owned();
                actions.push(Action::AddGroup(name.clone()));
                state.tab = Tab::Group(name);
                ui.close();
            }
        });
        state.new_group_menu_open = menu.is_some();
        response.on_hover_text("Create a group");
    });
}

fn tab_button(
    ui: &mut Ui,
    state: &mut LauncherState,
    tab: Tab,
    label: &str,
    count: usize,
    actions: &mut Vec<Action>,
) {
    let selected = state.tab == tab;
    let text = RichText::new(format!("{label}  {count}"));
    let response = ui.selectable_label(selected, if selected { text.strong() } else { text });
    if response.clicked() {
        state.tab = tab.clone();
    }

    // Dropping a tile on a tab moves it into that group / favorites.
    let accepts_drop = !matches!(tab, Tab::All);
    if accepts_drop && response.dnd_hover_payload::<DraggedApp>().is_some() {
        ui.painter().rect_stroke(
            response.rect,
            4.0,
            Stroke::new(2.0_f32, ui.visuals().selection.bg_fill),
            StrokeKind::Outside,
        );
    }
    if let Some(dragged) = response.dnd_release_payload::<DraggedApp>() {
        let index = dragged.0;
        match &tab {
            Tab::All => {}
            Tab::Favorites => actions.push(Action::SetFavorite(index, true)),
            Tab::Ungrouped => actions.push(Action::SetGroup { index, group: None }),
            Tab::Group(g) => actions.push(Action::SetGroup {
                index,
                group: Some(g.clone()),
            }),
        }
    }

    if let Tab::Group(name) = &tab {
        response.context_menu(|ui| {
            if ui.button("➕ Add app to this group…").clicked() {
                actions.push(Action::NewApp(AppEntry {
                    group: Some(name.clone()),
                    ..AppEntry::default()
                }));
            }
            if ui.button("✏ Manage groups…").clicked() {
                actions.push(Action::OpenSettings(SettingsTab::Groups));
            }
        });
    }
}

fn sections_for_tab<'a>(tab: &Tab, config: &'a Config) -> Vec<Section<'a>> {
    let indices = |pred: &dyn Fn(&AppEntry) -> bool| -> Vec<usize> {
        config
            .apps
            .iter()
            .enumerate()
            .filter(|(_, a)| pred(a))
            .map(|(i, _)| i)
            .collect()
    };
    match tab {
        Tab::All if config.settings.group_headers && !config.groups.is_empty() => {
            let mut sections: Vec<Section> = config
                .groups
                .iter()
                .map(|g| Section {
                    title: Some(g.name.clone()),
                    apps: indices(&|a| a.group.as_deref() == Some(g.name.as_str())),
                    group: Some(Some(g.name.clone())),
                    empty_hint: "",
                })
                .filter(|s| !s.apps.is_empty())
                .collect();
            let ungrouped = indices(&|a| a.group.is_none());
            if !ungrouped.is_empty() {
                sections.push(Section {
                    title: Some("Other".into()),
                    apps: ungrouped,
                    group: Some(None),
                    empty_hint: "",
                });
            }
            sections
        }
        Tab::All => vec![Section {
            title: None,
            apps: indices(&|_| true),
            group: None,
            empty_hint: "",
        }],
        Tab::Favorites => vec![Section {
            title: None,
            apps: indices(&|a| a.favorite),
            group: None,
            empty_hint: "Right-click an app and choose \"Add to favorites\",\nor drag it onto the ★ Favorites tab.",
        }],
        Tab::Ungrouped => vec![Section {
            title: None,
            apps: indices(&|a| a.group.is_none()),
            group: Some(None),
            empty_hint: "Apps that aren't in a group show up here.",
        }],
        Tab::Group(name) => vec![Section {
            title: None,
            apps: indices(&|a| a.group.as_deref() == Some(name.as_str())),
            group: Some(Some(name.clone())),
            empty_hint: "Drag apps onto this group's tab, drop files here,\nor click \"➕ Add app\".",
        }],
    }
}

fn section_header(ui: &mut Ui, title: &str, section: &Section, actions: &mut Vec<Action>) {
    let response = ui
        .horizontal(|ui| {
            ui.label(RichText::new(title).strong().size(15.0));
            ui.label(RichText::new(section.apps.len().to_string()).weak());
            ui.add(egui::Separator::default().horizontal());
        })
        .response
        .interact(Sense::hover());
    if section.group.is_some() && response.dnd_hover_payload::<DraggedApp>().is_some() {
        ui.painter().rect_stroke(
            response.rect,
            4.0,
            Stroke::new(2.0_f32, ui.visuals().selection.bg_fill),
            StrokeKind::Outside,
        );
    }
    if let (Some(group), Some(dragged)) =
        (&section.group, response.dnd_release_payload::<DraggedApp>())
    {
        actions.push(Action::SetGroup {
            index: dragged.0,
            group: group.clone(),
        });
    }
}

fn grid(
    ui: &mut Ui,
    config: &Config,
    apps: &[usize],
    icons: &mut IconCache,
    actions: &mut Vec<Action>,
    drop_mode: DropMode,
    highlight_first: bool,
) {
    let icon_size = config.settings.icon_size();
    let tile_width = tile::tile_size(ui, icon_size).x;
    let columns = ((ui.available_width() + tile::SPACING) / (tile_width + tile::SPACING))
        .floor()
        .max(1.0) as usize;
    let dragging = egui::DragAndDrop::payload::<DraggedApp>(ui.ctx()).map(|p| p.0);

    for (row_index, row) in apps.chunks(columns).enumerate() {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = tile::SPACING;
            for (col, &index) in row.iter().enumerate() {
                let app = &config.apps[index];
                let hovering_payload =
                    drop_mode != DropMode::Disabled && dragging.is_some_and(|d| d != index);
                let state = TileState {
                    highlighted: highlight_first && row_index == 0 && col == 0,
                    drop_target: false,
                    dragging: dragging == Some(index),
                };
                let response = tile::app_tile(ui, app, icons, icon_size, state);

                if hovering_payload && response.dnd_hover_payload::<DraggedApp>().is_some() {
                    ui.painter().rect_stroke(
                        response.rect,
                        12.0,
                        Stroke::new(2.0_f32, ui.visuals().selection.bg_fill),
                        StrokeKind::Inside,
                    );
                }
                if response.clicked() {
                    actions.push(Action::Launch {
                        index,
                        elevated: ui.input(|i| i.modifiers.ctrl && i.modifiers.shift),
                    });
                }
                if drop_mode != DropMode::Disabled {
                    response.dnd_set_drag_payload(DraggedApp(index));
                }
                if let Some(dragged) = response.dnd_release_payload::<DraggedApp>()
                    && drop_mode != DropMode::Disabled
                    && dragged.0 != index
                {
                    actions.push(Action::Reorder {
                        from: dragged.0,
                        to: index,
                        group: (drop_mode == DropMode::AdoptGroup).then(|| app.group.clone()),
                    });
                }

                let response = response.on_hover_ui_at_pointer(|ui| {
                    ui.strong(&app.name);
                    ui.label(RichText::new(&app.target).weak());
                    if !app.args.is_empty() {
                        ui.label(RichText::new(format!("Args: {}", app.args)).weak());
                    }
                });
                response.context_menu(|ui| tile_menu(ui, config, index, actions));
            }
        });
        ui.add_space(tile::SPACING);
    }
}

fn tile_menu(ui: &mut Ui, config: &Config, index: usize, actions: &mut Vec<Action>) {
    let app = &config.apps[index];
    ui.set_min_width(200.0);
    // Menus close themselves when an item is clicked.
    let item = |ui: &mut Ui, actions: &mut Vec<Action>, label: &str, action: Action| {
        if ui.button(label).clicked() {
            actions.push(action);
        }
    };
    item(
        ui,
        actions,
        "▶ Open",
        Action::Launch {
            index,
            elevated: false,
        },
    );
    if !crate::icons::is_uri(app.target.trim()) {
        item(
            ui,
            actions,
            "🛡 Run as administrator",
            Action::Launch {
                index,
                elevated: true,
            },
        );
    }
    if launch::local_path(&app.target).is_some() {
        item(ui, actions, "📂 Open file location", Action::Reveal(index));
    }
    ui.separator();
    item(ui, actions, "✏ Edit…", Action::Edit(index));
    item(ui, actions, "⎘ Duplicate", Action::Duplicate(index));
    if app.favorite {
        item(
            ui,
            actions,
            "☆ Remove from favorites",
            Action::SetFavorite(index, false),
        );
    } else {
        item(
            ui,
            actions,
            "★ Add to favorites",
            Action::SetFavorite(index, true),
        );
    }
    ui.menu_button("🗀 Move to group", |ui| {
        for group in &config.groups {
            let current = app.group.as_deref() == Some(group.name.as_str());
            if ui.radio(current, &group.name).clicked() {
                actions.push(Action::SetGroup {
                    index,
                    group: Some(group.name.clone()),
                });
            }
        }
        if !config.groups.is_empty() {
            ui.separator();
        }
        if ui.radio(app.group.is_none(), "No group").clicked() {
            actions.push(Action::SetGroup { index, group: None });
        }
        if ui.button("Manage groups…").clicked() {
            actions.push(Action::OpenSettings(SettingsTab::Groups));
        }
    });
    ui.separator();
    item(ui, actions, "🗑 Remove…", Action::Remove(index));
}

fn empty_state(ui: &mut Ui, title: &str, hint: &str) {
    ui.add_space(60.0);
    ui.vertical_centered(|ui| {
        ui.label(RichText::new(title).size(20.0).strong());
        ui.add_space(6.0);
        ui.label(RichText::new(hint).weak());
    });
}

/// Shows the dragged app's name next to the pointer.
fn drag_ghost(ui: &Ui, config: &Config) {
    let ctx = ui.ctx();
    let Some(dragged) = egui::DragAndDrop::payload::<DraggedApp>(ctx) else {
        return;
    };
    let (Some(app), Some(pos)) = (config.apps.get(dragged.0), ctx.pointer_interact_pos()) else {
        return;
    };
    ctx.set_cursor_icon(egui::CursorIcon::Grabbing);
    egui::Area::new(Id::unique("drag-ghost"))
        .fixed_pos(pos + egui::vec2(14.0, 10.0))
        .order(egui::Order::Tooltip)
        .interactable(false)
        .show(ctx, |ui| {
            egui::Frame::popup(ui.style()).show(ui, |ui| {
                ui.label(&app.name);
            });
        });
}
