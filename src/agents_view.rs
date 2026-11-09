//! The Agents home: a list of projects and their agent chats on the left, and a
//! large centred prompt box (or the open conversation) in the middle. The code
//! editor is one click away ("Editor Window").

use eframe::egui::{
    self, Align, Align2, Color32, CornerRadius, FontId, Frame, Id, Key, Layout, Margin, Modifiers,
    RichText, ScrollArea, Sense, Stroke,
};

use crate::app::{App, Mode};
use crate::theme::*;

/// Draw `add` in a column at most `max_w` wide, centred in the space left.
fn centered(ui: &mut egui::Ui, max_w: f32, add: impl FnOnce(&mut egui::Ui)) {
    let w = ui.available_width().min(max_w);
    let x = ((ui.available_width() - w) / 2.0).max(0.0);
    ui.horizontal_top(|ui| {
        ui.add_space(x);
        ui.vertical(|ui| {
            ui.set_width(w);
            add(ui);
        });
    });
}

/// One row of the left list: icon, label, optional right-hand note.
fn row(
    ui: &mut egui::Ui,
    indent: f32,
    icon: &str,
    label: &str,
    note: &str,
    selected: bool,
    strong: bool,
) -> egui::Response {
    let (rect, resp) =
        ui.allocate_exact_size(egui::vec2(ui.available_width(), 32.0), Sense::click());
    if selected {
        ui.painter().rect_filled(
            rect.shrink2(egui::vec2(4.0, 1.0)),
            CornerRadius::same(7),
            Color32::from_rgb(0x2a, 0x2a, 0x2a),
        );
    } else if resp.hovered() {
        ui.painter().rect_filled(
            rect.shrink2(egui::vec2(4.0, 1.0)),
            CornerRadius::same(7),
            Color32::from_rgb(0x21, 0x21, 0x21),
        );
    }
    let mut x = rect.left() + 14.0 + indent;
    if !icon.is_empty() {
        ui.painter().text(
            egui::pos2(x + 7.0, rect.center().y),
            Align2::CENTER_CENTER,
            icon,
            FontId::proportional(14.0),
            MUTED,
        );
        x += 24.0;
    }
    let color = if selected || strong { BRIGHT } else { TEXT };
    let max_chars = ((rect.right() - x - 50.0) / 7.0).max(6.0) as usize;
    let text: String = if label.chars().count() > max_chars {
        format!(
            "{}…",
            label
                .chars()
                .take(max_chars.saturating_sub(1))
                .collect::<String>()
        )
    } else {
        label.to_string()
    };
    ui.painter().text(
        egui::pos2(x, rect.center().y),
        Align2::LEFT_CENTER,
        text,
        FontId::proportional(14.0),
        color,
    );
    if !note.is_empty() {
        ui.painter().text(
            egui::pos2(rect.right() - 14.0, rect.center().y),
            Align2::RIGHT_CENTER,
            note,
            FontId::proportional(13.0),
            MUTED,
        );
    }
    resp
}

impl App {
    pub fn agents_ui(&mut self, ctx: &egui::Context) {
        // Shift+Tab in the prompt cycles the mode (the original's "Plan New Idea ⇧Tab").
        if ctx.memory(|m| m.has_focus(Id::new("chat_input")))
            && ctx.input_mut(|i| i.consume_key(Modifiers::SHIFT, Key::Tab))
        {
            let c = &mut self.chats[self.chat];
            c.mode = match c.mode {
                Mode::Agent => Mode::Plan,
                Mode::Plan => Mode::Ask,
                Mode::Ask => Mode::Agent,
            };
        }
        let mut acts = Vec::new();
        self.agents_sidebar(ctx);
        egui::CentralPanel::default()
            .frame(Frame::new().fill(EDITOR))
            .show(ctx, |ui| {
                if self.chats[self.chat].messages.is_empty() {
                    self.agents_home(ui, ctx);
                } else {
                    egui::TopBottomPanel::bottom("agents_input")
                        .frame(Frame::new().fill(EDITOR).inner_margin(Margin {
                            left: 0,
                            right: 0,
                            top: 8,
                            bottom: 18,
                        }))
                        .show_inside(ui, |ui| centered(ui, 780.0, |ui| self.chat_input(ui, ctx)));
                    egui::CentralPanel::default()
                        .frame(Frame::new().fill(EDITOR).inner_margin(Margin {
                            left: 0,
                            right: 0,
                            top: 18,
                            bottom: 0,
                        }))
                        .show_inside(ui, |ui| {
                            centered(ui, 780.0, |ui| self.chat_messages(ui, &mut acts))
                        });
                }
            });
        self.apply(acts, ctx);
    }

    fn agents_home(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        let full = ui.max_rect();
        ui.add_space((ui.available_height() * 0.40 - 40.0).max(20.0));
        centered(ui, 760.0, |ui| {
            ui.horizontal(|ui| {
                ui.add_space(4.0);
                let name = if self.root.is_some() {
                    self.workspace_name()
                } else {
                    "Choose a project".into()
                };
                let mut open: Option<std::path::PathBuf> = None;
                ui.menu_button(
                    RichText::new(format!("{name}  ⏷")).size(14.0).color(TEXT),
                    |ui| {
                        for p in &self.settings.recent_workspaces {
                            let label = p
                                .file_name()
                                .map(|n| n.to_string_lossy().to_string())
                                .unwrap_or_default();
                            if ui
                                .button(label)
                                .on_hover_text(p.display().to_string())
                                .clicked()
                            {
                                open = Some(p.clone());
                                ui.close_menu();
                            }
                        }
                        ui.separator();
                        if ui.button("Open Folder…").clicked() {
                            ui.close_menu();
                            if let Some(f) = rfd::FileDialog::new().pick_folder() {
                                open = Some(f);
                            }
                        }
                    },
                );
                if let Some(p) = open {
                    self.open_workspace(p, ctx);
                }
                ui.add_space(8.0);
                ui.label(RichText::new("🖥 Local").size(14.0).color(MUTED))
                    .on_hover_text("Agents run on this computer, in the folder you picked");
            });
            ui.add_space(4.0);
            self.chat_input(ui, ctx);
            ui.add_space(14.0);
            ui.horizontal(|ui| {
                let mode = self.chats[self.chat].mode;
                for (m, label, key) in [
                    (Mode::Plan, "Plan New Idea", "Shift+Tab"),
                    (Mode::Ask, "Ask About Code", "Ctrl+L"),
                    (Mode::Agent, "Build", "Ctrl+I"),
                ] {
                    let on = mode == m;
                    let mut text = egui::text::LayoutJob::default();
                    text.append(
                        label,
                        0.0,
                        egui::TextFormat {
                            font_id: FontId::proportional(14.0),
                            color: if on { BRIGHT } else { TEXT },
                            ..Default::default()
                        },
                    );
                    text.append(
                        &format!("  {key}"),
                        0.0,
                        egui::TextFormat {
                            font_id: FontId::proportional(12.0),
                            color: FAINT,
                            ..Default::default()
                        },
                    );
                    let button = egui::Button::new(text)
                        .fill(if on { RAISED } else { EDITOR })
                        .stroke(Stroke::new(1.0_f32, Color32::from_rgb(0x33, 0x33, 0x33)))
                        .corner_radius(CornerRadius::same(9));
                    if ui.add(button).clicked() {
                        self.chats[self.chat].mode = m;
                        self.focus_input = true;
                    }
                }
            });
        });
        // tip at the bottom, as in the original
        let mut tip = egui::text::LayoutJob::default();
        let fmt = |c: Color32| egui::TextFormat {
            font_id: FontId::proportional(13.5),
            color: c,
            ..Default::default()
        };
        tip.append("Use ", 0.0, fmt(MUTED));
        tip.append(
            " @codebase ",
            0.0,
            egui::TextFormat {
                background: RAISED,
                ..fmt(TEXT)
            },
        );
        tip.append(
            " to have OpenCode search your whole project before it answers",
            0.0,
            fmt(MUTED),
        );
        let galley = ui.fonts(|f| f.layout_job(tip));
        let pos = egui::pos2(
            full.center().x - galley.size().x / 2.0,
            full.bottom() - 44.0,
        );
        ui.painter().galley(pos, galley, MUTED);
    }

    fn agents_sidebar(&mut self, ctx: &egui::Context) {
        egui::SidePanel::left("agents_side")
            .default_width(275.0)
            .min_width(220.0)
            .resizable(true)
            .frame(
                Frame::new()
                    .fill(SIDE)
                    .inner_margin(Margin::symmetric(4, 10))
                    .stroke(Stroke::new(1.0_f32, BORDER)),
            )
            .show(ctx, |ui| {
                ui.spacing_mut().item_spacing.y = 2.0;
                let home = self.chats[self.chat].messages.is_empty();
                if row(ui, 0.0, "+", "New Agent", "Ctrl+Shift+L", home, false).clicked() {
                    self.new_chat(Mode::Agent);
                }
                if row(
                    ui,
                    0.0,
                    "🔍",
                    "Search",
                    "",
                    self.agents_filter.is_some(),
                    false,
                )
                .clicked()
                {
                    self.agents_filter = if self.agents_filter.is_some() {
                        None
                    } else {
                        Some(String::new())
                    };
                }
                if let Some(filter) = &mut self.agents_filter {
                    Frame::new()
                        .inner_margin(Margin::symmetric(12, 4))
                        .show(ui, |ui| {
                            let r = ui.add(
                                egui::TextEdit::singleline(filter)
                                    .hint_text("Search agents…")
                                    .desired_width(f32::INFINITY),
                            );
                            if r.gained_focus() || filter.is_empty() {
                                r.request_focus();
                            }
                        });
                }
                if row(ui, 0.0, "⚙", "Customize", "", false, false)
                    .on_hover_text("Models, indexing and chat settings")
                    .clicked()
                {
                    self.settings_open = true;
                }
                ui.add_space(18.0);
                ui.horizontal(|ui| {
                    ui.add_space(14.0);
                    ui.label(RichText::new("Repositories").size(13.0).color(MUTED));
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        ui.add_space(10.0);
                        if ui
                            .add(
                                egui::Button::new(RichText::new("🗀+").size(13.0).color(MUTED))
                                    .frame(false),
                            )
                            .on_hover_text("Open a project folder")
                            .clicked()
                        {
                            self.pick_workspace(ctx);
                        }
                    });
                });
                ui.add_space(4.0);
                let bottom_h = 60.0;
                let list_h = (ui.available_height() - bottom_h).max(60.0);
                ScrollArea::vertical()
                    .max_height(list_h)
                    .auto_shrink(false)
                    .show(ui, |ui| self.agents_repos(ui, ctx));
                // profile / settings row
                ui.with_layout(Layout::bottom_up(Align::Min), |ui| {
                    ui.add_space(4.0);
                    let (rect, resp) = ui.allocate_exact_size(
                        egui::vec2(ui.available_width(), 46.0),
                        Sense::click(),
                    );
                    if resp.hovered() {
                        ui.painter().rect_filled(
                            rect.shrink2(egui::vec2(4.0, 0.0)),
                            CornerRadius::same(7),
                            Color32::from_rgb(0x21, 0x21, 0x21),
                        );
                    }
                    let user = std::env::var("USERNAME")
                        .or_else(|_| std::env::var("USER"))
                        .unwrap_or_else(|_| "You".into());
                    let c = egui::pos2(rect.left() + 30.0, rect.center().y);
                    ui.painter().circle_filled(c, 15.0, RAISED);
                    let initial: String = user
                        .chars()
                        .next()
                        .map(|c| c.to_uppercase().collect())
                        .unwrap_or_else(|| "?".into());
                    ui.painter().text(
                        c,
                        Align2::CENTER_CENTER,
                        initial,
                        FontId::proportional(14.0),
                        BRIGHT,
                    );
                    ui.painter().text(
                        egui::pos2(rect.left() + 54.0, rect.center().y - 9.0),
                        Align2::LEFT_CENTER,
                        &user,
                        FontId::proportional(14.0),
                        BRIGHT,
                    );
                    let sub = if self.settings.ready() {
                        self.settings.model.clone()
                    } else {
                        "Open source · set up a model".into()
                    };
                    ui.painter().text(
                        egui::pos2(rect.left() + 54.0, rect.center().y + 10.0),
                        Align2::LEFT_CENTER,
                        sub,
                        FontId::proportional(12.5),
                        MUTED,
                    );
                    ui.painter().text(
                        egui::pos2(rect.right() - 22.0, rect.center().y),
                        Align2::CENTER_CENTER,
                        "⚙",
                        FontId::proportional(16.0),
                        MUTED,
                    );
                    if resp.on_hover_text("Settings (Ctrl+,)").clicked() {
                        self.settings_open = true;
                    }
                });
            });
    }

    fn agents_repos(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        let filter = self
            .agents_filter
            .clone()
            .unwrap_or_default()
            .to_lowercase();
        let matches = |t: &str| filter.is_empty() || t.to_lowercase().contains(&filter);
        let current = self.root.clone();
        let running = self.running_chat_job().is_some();
        let mut open_repo = None;
        let mut select_chat = None;
        let mut open_title: Option<(std::path::PathBuf, String)> = None;
        for (root, cached) in self.history_cache.clone() {
            let name = root
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_else(|| root.display().to_string());
            let is_current = current.as_ref() == Some(&root);
            if row(ui, 0.0, "🗀", &name, "", false, is_current)
                .on_hover_text(root.display().to_string())
                .clicked()
                && !is_current
            {
                open_repo = Some(root.clone());
            }
            let mut shown = 0;
            if is_current {
                let mut live: Vec<usize> = (0..self.chats.len())
                    .filter(|&i| !self.chats[i].messages.is_empty())
                    .collect();
                live.sort_by(|a, b| self.chats[*b].updated.cmp(&self.chats[*a].updated));
                for i in live {
                    let chat = &self.chats[i];
                    if !matches(&chat.title) {
                        continue;
                    }
                    let note = if running && i == self.chat {
                        "…".to_string()
                    } else {
                        crate::history::age(chat.updated)
                    };
                    if row(ui, 22.0, "", &chat.title, &note, i == self.chat, false).clicked() {
                        select_chat = Some(i);
                    }
                    shown += 1;
                }
            } else {
                for (title, updated) in cached.iter().take(8) {
                    if !matches(title) {
                        continue;
                    }
                    if row(
                        ui,
                        22.0,
                        "",
                        title,
                        &crate::history::age(*updated),
                        false,
                        false,
                    )
                    .clicked()
                    {
                        open_title = Some((root.clone(), title.clone()));
                    }
                    shown += 1;
                }
            }
            if shown == 0 {
                ui.horizontal(|ui| {
                    ui.add_space(38.0);
                    ui.label(RichText::new("No agents yet").size(13.5).color(MUTED));
                });
                ui.add_space(6.0);
            }
            ui.add_space(10.0);
        }
        if self.history_cache.is_empty() {
            ui.horizontal(|ui| {
                ui.add_space(14.0);
                if ui
                    .add(
                        egui::Button::new(
                            RichText::new("Open a project folder").color(ACCENT_TEXT),
                        )
                        .fill(ACCENT),
                    )
                    .clicked()
                {
                    self.pick_workspace(ctx);
                }
            });
        }
        if let Some(i) = select_chat {
            self.chat = i;
        }
        if let Some(root) = open_repo {
            self.open_workspace(root, ctx);
        }
        if let Some((root, title)) = open_title {
            self.open_workspace(root, ctx);
            if let Some(i) = self
                .chats
                .iter()
                .position(|c| c.title == title && !c.messages.is_empty())
            {
                self.chat = i;
            }
        }
    }
}
