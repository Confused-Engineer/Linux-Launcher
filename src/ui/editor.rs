//! Add/edit dialog for a single app.

use crate::config::{AppEntry, Group, name_from_target};
use crate::icons::IconCache;
use eframe::egui::{self, Context, Grid, Id, Key, RichText, TextEdit, Ui, Vec2};

const FIELD_WIDTH: f32 = 340.0;

const PROGRAM_EXTENSIONS: &[&str] = &["exe", "lnk", "url", "bat", "cmd", "ps1", "msc", "appref-ms"];
const ICON_EXTENSIONS: &[&str] = &[
    "png", "jpg", "jpeg", "gif", "bmp", "webp", "svg", "ico", "exe", "dll",
];

/// What happened in the dialog this frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EditorOutcome {
    Open,
    Save,
    Cancel,
    /// Launch the draft without saving it.
    Test,
}

pub struct Editor {
    /// Index of the app being edited, or `None` for a new app.
    pub index: Option<usize>,
    pub draft: AppEntry,
    tags: String,
    creating_group: bool,
    new_group: String,
    error: Option<String>,
    first_frame: bool,
}

impl Editor {
    pub fn new_app(preset: AppEntry) -> Self {
        Self::build(None, preset)
    }

    pub fn edit(index: usize, app: AppEntry) -> Self {
        Self::build(Some(index), app)
    }

    fn build(index: Option<usize>, draft: AppEntry) -> Self {
        Self {
            index,
            tags: draft.tags.join(", "),
            draft,
            creating_group: false,
            new_group: String::new(),
            error: None,
            first_frame: true,
        }
    }

    /// The draft with tags and group applied, or a validation message.
    pub fn result(&self) -> Result<AppEntry, String> {
        let mut app = self.draft.clone();
        app.tags = self.tags.split(',').map(str::to_owned).collect();
        if self.creating_group {
            app.group = Some(self.new_group.clone());
        }
        if app.name.trim().is_empty() && !app.target.trim().is_empty() {
            app.name = name_from_target(&app.target);
        }
        app.normalize();
        app.validate()?;
        Ok(app)
    }

    pub fn set_error(&mut self, error: impl Into<String>) {
        self.error = Some(error.into());
    }

    pub fn show(
        &mut self,
        ctx: &Context,
        groups: &[Group],
        icons: &mut IconCache,
    ) -> EditorOutcome {
        let mut outcome = EditorOutcome::Open;
        let modal = egui::Modal::new(Id::unique("app-editor")).show(ctx, |ui| {
            ui.set_width(FIELD_WIDTH + 140.0);
            ui.horizontal(|ui| {
                let (rect, _) = ui.allocate_exact_size(Vec2::splat(48.0), egui::Sense::hover());
                if !self.draft.target.trim().is_empty() || !self.draft.icon.trim().is_empty() {
                    icons.paint(ui, &self.draft, rect);
                }
                ui.heading(if self.index.is_some() {
                    "Edit app"
                } else {
                    "Add app"
                });
            });
            ui.add_space(8.0);

            self.form(ui, groups);

            if let Some(error) = &self.error {
                ui.add_space(4.0);
                ui.colored_label(ui.visuals().error_fg_color, error);
            }
            ui.add_space(8.0);
            ui.separator();
            ui.horizontal(|ui| {
                if ui
                    .button("Test launch")
                    .on_hover_text("Launch without saving")
                    .clicked()
                {
                    outcome = EditorOutcome::Test;
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let save = ui.add(egui::Button::new(RichText::new("Save").strong()));
                    if save.clicked() || ui.input(|i| i.modifiers.ctrl && i.key_pressed(Key::Enter))
                    {
                        outcome = EditorOutcome::Save;
                    }
                    if ui.button("Cancel").clicked() {
                        outcome = EditorOutcome::Cancel;
                    }
                });
            });
            ui.label(
                RichText::new("Ctrl+Enter to save, Esc to cancel")
                    .small()
                    .weak(),
            );
        });
        // Only Esc dismisses: a stray click on the backdrop must not discard edits.
        let escape = ctx.input(|i| i.key_pressed(Key::Escape));
        if modal.should_close() && escape && outcome == EditorOutcome::Open {
            outcome = EditorOutcome::Cancel;
        }
        self.first_frame = false;
        if outcome == EditorOutcome::Save
            && let Err(e) = self.result()
        {
            self.error = Some(e);
            return EditorOutcome::Open;
        }
        outcome
    }

    fn form(&mut self, ui: &mut Ui, groups: &[Group]) {
        Grid::new("editor-form")
            .num_columns(2)
            .spacing([12.0, 8.0])
            .show(ui, |ui| {
                ui.label("Target");
                ui.horizontal(|ui| {
                    let r = ui.add(
                        TextEdit::singleline(&mut self.draft.target)
                            .hint_text(
                                r"C:\path\app.exe, shortcut, folder, https://…, ms-settings:",
                            )
                            .desired_width(FIELD_WIDTH - 70.0),
                    );
                    if self.first_frame && self.draft.target.is_empty() {
                        r.request_focus();
                    }
                    if ui.button("📄").on_hover_text("Browse for a file").clicked() {
                        let picked = rfd::FileDialog::new()
                            .add_filter("Programs & shortcuts", PROGRAM_EXTENSIONS)
                            .add_filter("All files", &["*"])
                            .pick_file();
                        if let Some(path) = picked {
                            self.set_target(path.to_string_lossy().into_owned());
                        }
                    }
                    if ui
                        .button("📁")
                        .on_hover_text("Browse for a folder")
                        .clicked()
                        && let Some(path) = rfd::FileDialog::new().pick_folder()
                    {
                        self.set_target(path.to_string_lossy().into_owned());
                    }
                });
                ui.end_row();

                ui.label("Name");
                let r = ui.add(
                    TextEdit::singleline(&mut self.draft.name)
                        .hint_text(name_from_target(&self.draft.target))
                        .desired_width(FIELD_WIDTH),
                );
                if self.first_frame && !self.draft.target.is_empty() {
                    r.request_focus();
                }
                ui.end_row();

                ui.label("Group");
                self.group_picker(ui, groups);
                ui.end_row();

                ui.label("Arguments");
                ui.add(
                    TextEdit::singleline(&mut self.draft.args)
                        .hint_text("optional, e.g. --profile work")
                        .desired_width(FIELD_WIDTH),
                );
                ui.end_row();

                ui.label("Start in");
                ui.horizontal(|ui| {
                    ui.add(
                        TextEdit::singleline(&mut self.draft.working_dir)
                            .hint_text("optional (defaults to the target's folder)")
                            .desired_width(FIELD_WIDTH - 34.0),
                    );
                    if ui.button("📁").on_hover_text("Browse").clicked()
                        && let Some(path) = rfd::FileDialog::new().pick_folder()
                    {
                        self.draft.working_dir = path.to_string_lossy().into_owned();
                    }
                });
                ui.end_row();

                ui.label("Icon");
                ui.horizontal(|ui| {
                    ui.add(
                        TextEdit::singleline(&mut self.draft.icon)
                            .hint_text("automatic, or an image path / URL / .exe")
                            .desired_width(FIELD_WIDTH - 34.0),
                    );
                    if ui.button("📄").on_hover_text("Browse").clicked() {
                        let picked = rfd::FileDialog::new()
                            .add_filter("Images & icons", ICON_EXTENSIONS)
                            .add_filter("All files", &["*"])
                            .pick_file();
                        if let Some(path) = picked {
                            self.draft.icon = path.to_string_lossy().into_owned();
                        }
                    }
                });
                ui.end_row();

                ui.label("Tags");
                ui.add(
                    TextEdit::singleline(&mut self.tags)
                        .hint_text("extra search words, comma separated")
                        .desired_width(FIELD_WIDTH),
                );
                ui.end_row();

                ui.label("");
                ui.horizontal(|ui| {
                    ui.checkbox(&mut self.draft.favorite, "★ Favorite");
                    ui.checkbox(
                        &mut self.draft.run_as_admin,
                        "🛡 Always run as administrator",
                    );
                });
                ui.end_row();
            });
    }

    fn set_target(&mut self, target: String) {
        if self.draft.name.trim().is_empty() {
            self.draft.name = name_from_target(&target);
        }
        self.draft.target = target;
        self.error = None;
    }

    fn group_picker(&mut self, ui: &mut Ui, groups: &[Group]) {
        if self.creating_group {
            ui.horizontal(|ui| {
                ui.add(
                    TextEdit::singleline(&mut self.new_group)
                        .hint_text("New group name")
                        .desired_width(FIELD_WIDTH - 34.0),
                );
                if ui
                    .button("✖")
                    .on_hover_text("Pick an existing group")
                    .clicked()
                {
                    self.creating_group = false;
                }
            });
            return;
        }
        let current = self
            .draft
            .group
            .clone()
            .unwrap_or_else(|| "No group".into());
        egui::ComboBox::from_id_salt("editor-group")
            .selected_text(current)
            .width(FIELD_WIDTH)
            .show_ui(ui, |ui| {
                ui.selectable_value(&mut self.draft.group, None, "No group");
                for group in groups {
                    ui.selectable_value(
                        &mut self.draft.group,
                        Some(group.name.clone()),
                        &group.name,
                    );
                }
                ui.separator();
                if ui.selectable_label(false, "➕ New group…").clicked() {
                    self.creating_group = true;
                }
            });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn result_fills_name_tags_and_new_group() {
        let mut editor = Editor::new_app(AppEntry::new("", r"C:\Tools\putty.exe"));
        editor.tags = " ssh , , terminal".into();
        editor.creating_group = true;
        editor.new_group = " Remote ".into();
        let app = editor.result().unwrap();
        assert_eq!(app.name, "putty");
        assert_eq!(app.tags, ["ssh", "terminal"]);
        assert_eq!(app.group.as_deref(), Some("Remote"));
    }

    #[test]
    fn result_requires_target() {
        let editor = Editor::new_app(AppEntry::new("Name only", ""));
        assert!(editor.result().is_err());
    }
}
