//! Small transient notifications in the bottom-right corner.

use egui::{self, Align2, Color32, Context, Id, RichText};

const DURATION_SECS: f64 = 4.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Kind {
    Info,
    Success,
    Error,
}

struct Toast {
    kind: Kind,
    text: String,
    /// Set the first time the toast is drawn.
    expires_at: Option<f64>,
}

#[derive(Default)]
pub struct Toasts {
    items: Vec<Toast>,
}

impl Toasts {
    pub fn info(&mut self, text: impl Into<String>) {
        self.push(Kind::Info, text.into());
    }

    pub fn success(&mut self, text: impl Into<String>) {
        self.push(Kind::Success, text.into());
    }

    pub fn error(&mut self, text: impl Into<String>) {
        let text = text.into();
        log::warn!("{text}");
        self.push(Kind::Error, text);
    }

    fn push(&mut self, kind: Kind, text: String) {
        self.items.push(Toast {
            kind,
            text,
            expires_at: None,
        });
    }

    pub fn show(&mut self, ctx: &Context) {
        let now = ctx.input(|i| i.time);
        self.items.retain(|t| t.expires_at.is_none_or(|e| e > now));
        if self.items.is_empty() {
            return;
        }

        egui::Area::new(Id::unique("toasts"))
            .anchor(Align2::RIGHT_BOTTOM, [-16.0, -16.0])
            .order(egui::Order::Foreground)
            .interactable(true)
            .show(ctx, |ui| {
                ui.set_max_width(360.0);
                let mut dismissed = None;
                for (i, toast) in self.items.iter_mut().enumerate() {
                    // Errors stay twice as long.
                    let duration = match toast.kind {
                        Kind::Error => DURATION_SECS * 2.0,
                        _ => DURATION_SECS,
                    };
                    let expires = *toast.expires_at.get_or_insert(now + duration);
                    let (icon, color) = match toast.kind {
                        Kind::Info => ("ℹ", ui.visuals().hyperlink_color),
                        Kind::Success => ("✔", Color32::from_rgb(80, 180, 100)),
                        Kind::Error => ("⚠", ui.visuals().error_fg_color),
                    };
                    let frame = egui::Frame::popup(ui.style()).show(ui, |ui| {
                        ui.horizontal(|ui| {
                            ui.label(RichText::new(icon).color(color).strong());
                            ui.label(&toast.text);
                        });
                    });
                    if frame.response.interact(egui::Sense::click()).clicked() {
                        dismissed = Some(i);
                    }
                    ui.request_repaint_after_secs((expires - now).max(0.0) as f32);
                }
                if let Some(i) = dismissed {
                    self.items.remove(i);
                }
            });
    }
}
