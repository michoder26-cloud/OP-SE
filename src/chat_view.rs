//! The chat / agent panel (Ctrl+L / Ctrl+I), @-mention menu, quick open and settings.

use eframe::egui::{
    self, text::LayoutJob, Align, Align2, Color32, CornerRadius, Event, FontId, Frame, Id, Key,
    Layout, Margin, Modifiers, Order, RichText, ScrollArea, Sense, Stroke, TextEdit, TextFormat,
};

use crate::ai::Role;
use crate::app::{App, ChangeState, Mode, Origin};
use crate::diff::{self, FileChange};
use crate::highlight::{self, lang_for};
use crate::index::{CHUNK_MAX, CHUNK_MIN, MAX_FILES};
use crate::settings::{Provider, Settings};
use crate::theme::*;
use crate::view::{file_badge, Act};

impl App {
    pub fn chat_panel(&mut self, ctx: &egui::Context) {
        let mut acts = Vec::new();
        egui::SidePanel::right("chat")
            .default_width(420.0)
            .min_width(320.0)
            .resizable(true)
            .frame(
                Frame::new()
                    .fill(SIDE)
                    .inner_margin(Margin::symmetric(10, 8))
                    .stroke(Stroke::new(1.0_f32, BORDER)),
            )
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    let mut select = None;
                    let start = self.chats.len().saturating_sub(3);
                    for i in start..self.chats.len() {
                        let active = i == self.chat;
                        let title: String = self.chats[i].title.chars().take(18).collect();
                        let text = RichText::new(title).size(12.0).color(if active {
                            BRIGHT
                        } else {
                            MUTED
                        });
                        let button = egui::Button::new(text)
                            .fill(if active { RAISED } else { Color32::TRANSPARENT })
                            .corner_radius(CornerRadius::same(5));
                        if ui.add(button).clicked() {
                            select = Some(i);
                        }
                    }
                    if let Some(i) = select {
                        self.chat = i;
                    }
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if ui
                            .add(egui::Button::new(RichText::new("×").color(MUTED)).frame(false))
                            .on_hover_text("Close panel")
                            .clicked()
                        {
                            self.chat_open = false;
                        }
                        ui.menu_button(RichText::new("⟲").color(MUTED), |ui| {
                            ui.label(RichText::new("Chat history").size(11.0).color(MUTED));
                            for i in (0..self.chats.len()).rev() {
                                let title = format!(
                                    "{}  ({} messages)",
                                    self.chats[i].title,
                                    self.chats[i].messages.len()
                                );
                                if ui.selectable_label(i == self.chat, title).clicked() {
                                    self.chat = i;
                                    ui.close_menu();
                                }
                            }
                        })
                        .response
                        .on_hover_text("Chat history");
                        if ui
                            .add(
                                egui::Button::new(RichText::new("+").size(15.0).color(MUTED))
                                    .frame(false),
                            )
                            .on_hover_text("New chat (Ctrl+Shift+L)")
                            .clicked()
                        {
                            self.new_chat(Mode::Agent);
                        }
                    });
                });
                ui.add_space(4.0);
                egui::TopBottomPanel::bottom("chat_input")
                    .frame(Frame::new().fill(SIDE).inner_margin(Margin {
                        left: 0,
                        right: 0,
                        top: 6,
                        bottom: 0,
                    }))
                    .show_inside(ui, |ui| self.chat_input(ui, ctx));
                egui::CentralPanel::default()
                    .frame(Frame::new().fill(SIDE))
                    .show_inside(ui, |ui| self.chat_messages(ui, &mut acts));
            });
        self.apply(acts, ctx);
    }

    pub(crate) fn chat_messages(&mut self, ui: &mut egui::Ui, acts: &mut Vec<Act>) {
        let ci = self.chat;
        if self.chats[ci].messages.is_empty() {
            let mode = self.chats[ci].mode;
            ui.add_space(30.0);
            ui.vertical_centered(|ui| {
                ui.label(RichText::new(mode.icon()).size(26.0).color(FAINT));
                ui.label(RichText::new(mode.name()).size(16.0).strong().color(BRIGHT));
                ui.label(RichText::new(mode.description()).color(MUTED));
            });
            ui.add_space(18.0);
            ui.label(RichText::new("Try").size(11.0).color(FAINT));
            let tries: &[&str] = match mode {
                Mode::Agent => &[
                    "Add input validation to the open file",
                    "Create a README that explains this project",
                    "@src/ put all error messages in one place",
                ],
                Mode::Ask => &[
                    "โค้ดส่วนต่อระบบชำระเงินอยู่ตรงไหน?",
                    "Explain how the selected function works",
                    "@codebase where are settings loaded?",
                ],
                Mode::Plan => &["Plan adding a light theme", "Plan tests for the open file"],
            };
            for t in tries {
                if ui
                    .add(
                        egui::Button::new(RichText::new(*t).size(12.5).color(TEXT))
                            .fill(WIDGET)
                            .corner_radius(CornerRadius::same(6)),
                    )
                    .clicked()
                {
                    self.input = t.to_string();
                    self.focus_input = true;
                }
            }
            ui.add_space(18.0);
            egui::Grid::new("chat_keys")
                .num_columns(2)
                .spacing([16.0, 6.0])
                .show(ui, |ui| {
                    for (k, v) in [
                        ("Ctrl+K", "Edit or generate code in the file"),
                        ("Ctrl+L", "Ask about code (adds the selection)"),
                        ("Ctrl+I", "Agent: edit many files at once"),
                        ("@", "Mention files, folders or @codebase"),
                    ] {
                        ui.label(RichText::new(k).monospace().size(11.5).color(MUTED));
                        ui.label(RichText::new(v).size(11.5).color(FAINT));
                        ui.end_row();
                    }
                });
            if !self.settings.ready() {
                ui.add_space(14.0);
                if ui
                    .add(
                        egui::Button::new(RichText::new("Set up an AI model").color(ACCENT_TEXT))
                            .fill(ACCENT),
                    )
                    .clicked()
                {
                    acts.push(Act::Settings);
                }
            }
            return;
        }
        ScrollArea::vertical()
            .auto_shrink(false)
            .stick_to_bottom(true)
            .show(ui, |ui| {
                ui.set_width(ui.available_width());
                for mi in 0..self.chats[ci].messages.len() {
                    let m = &self.chats[ci].messages[mi];
                    if m.role == Role::User {
                        Frame::new()
                            .fill(RAISED)
                            .corner_radius(CornerRadius::same(7))
                            .inner_margin(Margin::same(9))
                            .show(ui, |ui| {
                                ui.set_width(ui.available_width());
                                if !m.context.is_empty() {
                                    ui.horizontal_wrapped(|ui| {
                                        for c in &m.context {
                                            pill(ui, c);
                                        }
                                    });
                                }
                                ui.label(RichText::new(&m.text).color(BRIGHT));
                            });
                        ui.add_space(8.0);
                        continue;
                    }
                    if !m.keywords.is_empty() || !m.sources.is_empty() {
                        let title = format!("🔍 Searched codebase · {} results", m.sources.len());
                        egui::CollapsingHeader::new(RichText::new(title).size(11.5).color(MUTED))
                            .id_salt(("sources", ci, mi))
                            .show(ui, |ui| {
                                if !m.keywords.is_empty() {
                                    ui.label(
                                        RichText::new(format!("keywords: {}", m.keywords))
                                            .size(11.0)
                                            .color(FAINT),
                                    );
                                }
                                for s in &m.sources {
                                    if ui
                                        .add(
                                            egui::Label::new(
                                                RichText::new(s).size(11.5).color(ACCENT),
                                            )
                                            .sense(Sense::click()),
                                        )
                                        .clicked()
                                    {
                                        let (rel, line) = split_source(s);
                                        acts.push(Act::OpenAt(rel, line));
                                    }
                                }
                            });
                    }
                    if m.streaming {
                        ui.horizontal(|ui| {
                            ui.add(egui::Spinner::new().size(11.0));
                            let status = if m.status.is_empty() {
                                "Generating…"
                            } else {
                                m.status.as_str()
                            };
                            ui.label(RichText::new(status).size(11.5).color(MUTED));
                        });
                    }
                    let prose = if m.mode == Mode::Agent {
                        diff::parse_agent_answer(&m.text).0
                    } else {
                        m.text.clone()
                    };
                    let pending = &self.pending;
                    markdown(ui, &prose, acts, |ui, marker, acts| {
                        if let Some(rel) = marker.strip_prefix("writing:") {
                            ui.horizontal(|ui| {
                                ui.add(egui::Spinner::new().size(11.0));
                                ui.label(
                                    RichText::new(format!("Writing {rel}…"))
                                        .size(12.0)
                                        .color(MUTED),
                                );
                            });
                        } else if let Some(rel) = marker.strip_prefix("file:") {
                            if let Some(n) = m.changes.iter().position(|c| c.rel == rel) {
                                let state = m
                                    .change_state
                                    .get(n)
                                    .copied()
                                    .unwrap_or(ChangeState::Pending);
                                let index = pending.iter().position(|p| {
                                    p.origin
                                        == Origin::Agent {
                                            chat: ci,
                                            msg: mi,
                                            change: n,
                                        }
                                });
                                let original = index.map(|i| pending[i].original.as_str());
                                change_card(ui, &m.changes[n], state, index, original, acts);
                            } else {
                                ui.label(
                                    RichText::new(format!("📄 {rel}")).size(12.0).color(MUTED),
                                );
                            }
                        }
                    });
                    if let Some(e) = &m.error {
                        Frame::new()
                            .fill(Color32::from_rgb(0x2a, 0x14, 0x18))
                            .corner_radius(CornerRadius::same(6))
                            .inner_margin(Margin::same(8))
                            .show(ui, |ui| {
                                ui.set_width(ui.available_width());
                                ui.label(RichText::new(e).color(REMOVED));
                                if e.contains("Settings") || e.contains("401") || e.contains("key")
                                {
                                    if ui.button("Open AI Settings").clicked() {
                                        acts.push(Act::Settings);
                                    }
                                }
                            });
                    }
                    let open_changes = m
                        .change_state
                        .iter()
                        .filter(|s| **s == ChangeState::Pending)
                        .count();
                    if open_changes > 1 {
                        ui.horizontal(|ui| {
                            if ui
                                .add(
                                    egui::Button::new(
                                        RichText::new(format!(
                                            "Accept all {open_changes} files  Ctrl+Enter"
                                        ))
                                        .color(ACCENT_TEXT),
                                    )
                                    .fill(ADDED),
                                )
                                .clicked()
                            {
                                acts.push(Act::AcceptAll);
                            }
                            if ui.button("Reject all").clicked() {
                                acts.push(Act::RejectAll);
                            }
                        });
                    }
                    ui.add_space(12.0);
                }
            });
    }

    pub(crate) fn chat_input(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        let input_id = Id::new("chat_input");
        let has_focus = ctx.memory(|m| m.has_focus(input_id));
        let query = self.mention_query();
        let candidates = query
            .as_ref()
            .map(|(_, q)| self.mention_candidates(q))
            .unwrap_or_default();
        let popup = has_focus && query.is_some() && !candidates.is_empty();
        let mut send = false;
        let mut picked: Option<String> = None;
        if has_focus {
            if popup {
                if ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::ArrowDown)) {
                    self.mention_sel = (self.mention_sel + 1) % candidates.len();
                }
                if ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::ArrowUp)) {
                    self.mention_sel = (self.mention_sel + candidates.len() - 1) % candidates.len();
                }
                if ctx.input_mut(|i| {
                    i.consume_key(Modifiers::NONE, Key::Tab)
                        || i.consume_key(Modifiers::NONE, Key::Enter)
                }) {
                    picked = candidates
                        .get(self.mention_sel.min(candidates.len() - 1))
                        .map(|c| c.0.clone());
                }
            } else {
                send = ctx.input_mut(|i| {
                    let pos = i.events.iter().position(|e| {
                        matches!(e, Event::Key { key: Key::Enter, pressed: true, modifiers, .. } if !modifiers.shift && !modifiers.command)
                    });
                    pos.map(|p| i.events.remove(p)).is_some()
                });
            }
        }
        if let Some(p) = picked {
            self.pick_mention(&p);
        }
        let mode = self.chats[self.chat].mode;
        let running = self.running_chat_job();
        let mut remove_sel = None;
        let mut input_rect = egui::Rect::NOTHING;
        let input_frame = Frame::new()
            .fill(WIDGET)
            .stroke(Stroke::new(1.0_f32, Color32::from_rgb(0x33, 0x33, 0x33)))
            .corner_radius(CornerRadius::same(8))
            .inner_margin(Margin::same(8))
            .show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.horizontal_wrapped(|ui| {
                    if ui
                        .add(
                            egui::Button::new(RichText::new("@").size(12.0).color(MUTED))
                                .fill(RAISED),
                        )
                        .on_hover_text("Add files, folders or @codebase")
                        .clicked()
                    {
                        if !self.input.is_empty() && !self.input.ends_with(' ') {
                            self.input.push(' ');
                        }
                        self.input.push('@');
                        self.focus_input = true;
                    }
                    if let Some(i) = self.active {
                        let name = self.tabs[i].name().to_string();
                        let on = self.settings.include_current_file;
                        let text = if on {
                            format!("📄 {name}  ×")
                        } else {
                            format!("+ {name}")
                        };
                        let color = if on { TEXT } else { FAINT };
                        if ui
                            .add(
                                egui::Button::new(RichText::new(text).size(11.5).color(color))
                                    .fill(RAISED),
                            )
                            .on_hover_text("The current file is sent with your message")
                            .clicked()
                        {
                            self.settings.include_current_file = !on;
                        }
                    }
                    for (n, s) in self.chats[self.chat].selections.iter().enumerate() {
                        let name = s.rel.rsplit('/').next().unwrap_or(&s.rel);
                        if ui
                            .add(
                                egui::Button::new(
                                    RichText::new(format!(
                                        "❐ {name} ({}-{})  ×",
                                        s.start_line, s.end_line
                                    ))
                                    .size(11.5),
                                )
                                .fill(RAISED),
                            )
                            .clicked()
                        {
                            remove_sel = Some(n);
                        }
                    }
                    let (files, folders, codebase) = self.mentions(&self.input);
                    for f in files {
                        pill(ui, &format!("@{f}"));
                    }
                    for f in folders {
                        pill(ui, &format!("@{f}/"));
                    }
                    if codebase {
                        pill(ui, "@codebase");
                    }
                });
                let hint = match mode {
                    Mode::Agent => "Plan, build, @ for context",
                    Mode::Ask => "Ask about your code… (@ files, @codebase)",
                    Mode::Plan => "Describe what you want to plan…",
                };
                let resp = ui.add(
                    TextEdit::multiline(&mut self.input)
                        .id(input_id)
                        .hint_text(hint)
                        .frame(false)
                        .desired_rows(3)
                        .desired_width(f32::INFINITY),
                );
                input_rect = resp.rect;
                if self.focus_input {
                    resp.request_focus();
                    self.focus_input = false;
                }
                ui.horizontal(|ui| {
                    let mut new_mode = mode;
                    egui::ComboBox::from_id_salt("mode")
                        .selected_text(
                            RichText::new(format!("{} {}", mode.icon(), mode.name())).size(12.0),
                        )
                        .width(92.0)
                        .show_ui(ui, |ui| {
                            for m in Mode::ALL {
                                let text = format!(
                                    "{} {}   {}\n{}",
                                    m.icon(),
                                    m.name(),
                                    m.shortcut(),
                                    m.description()
                                );
                                ui.selectable_value(
                                    &mut new_mode,
                                    m,
                                    RichText::new(text).size(12.0),
                                );
                            }
                        });
                    self.chats[self.chat].mode = new_mode;
                    let model = if self.settings.ready() {
                        self.settings.model.clone()
                    } else {
                        "Set up a model".into()
                    };
                    if ui
                        .add(
                            egui::Button::new(RichText::new(model).size(11.5).color(MUTED))
                                .frame(false),
                        )
                        .clicked()
                    {
                        self.settings_open = true;
                        self.settings_tab = 0;
                    }
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if let Some(job) = running {
                            if ui
                                .add(
                                    egui::Button::new(RichText::new("■").color(BRIGHT))
                                        .corner_radius(CornerRadius::same(12)),
                                )
                                .on_hover_text("Stop")
                                .clicked()
                            {
                                self.stop_job(job);
                            }
                        } else {
                            let can = !self.input.trim().is_empty();
                            let fill = if can {
                                BRIGHT
                            } else {
                                Color32::from_rgb(0x55, 0x55, 0x55)
                            };
                            if ui
                                .add_enabled(
                                    can,
                                    egui::Button::new(RichText::new(" ↑ ").strong().color(SIDE))
                                        .fill(fill)
                                        .corner_radius(CornerRadius::same(12)),
                                )
                                .on_hover_text("Send (Enter)")
                                .clicked()
                            {
                                send = true;
                            }
                        }
                    });
                });
            });
        input_rect = input_frame.response.rect;
        if let Some(n) = remove_sel {
            self.chats[self.chat].selections.remove(n);
        }
        if popup {
            let sel = self.mention_sel.min(candidates.len() - 1);
            let mut pick = None;
            egui::Area::new(Id::new("mentions"))
                .order(Order::Foreground)
                .fixed_pos(input_rect.left_top() - egui::vec2(0.0, 4.0))
                .pivot(Align2::LEFT_BOTTOM)
                .show(ctx, |ui| {
                    Frame::new()
                        .fill(WIDGET)
                        .stroke(Stroke::new(1.0_f32, BORDER))
                        .corner_radius(CornerRadius::same(7))
                        .inner_margin(Margin::same(6))
                        .show(ui, |ui| {
                            ui.set_width(input_rect.width().max(300.0));
                            ui.label(RichText::new("Files & Folders").size(11.0).color(FAINT));
                            for (n, (path, is_dir)) in candidates.iter().enumerate() {
                                let icon = if path == "codebase" {
                                    "🔍"
                                } else if *is_dir {
                                    "🗀"
                                } else {
                                    "📄"
                                };
                                let label = if path == "codebase" {
                                    "codebase — search the whole project".to_string()
                                } else {
                                    path.clone()
                                };
                                if ui
                                    .selectable_label(
                                        n == sel,
                                        RichText::new(format!("{icon}  {label}")).size(12.0),
                                    )
                                    .clicked()
                                {
                                    pick = Some(path.clone());
                                }
                            }
                        });
                });
            if let Some(p) = pick {
                self.pick_mention(&p);
            }
        }
        if send {
            self.send_chat(ctx);
        }
    }

    // ------------------------------------------------------------------ quick open

    pub fn quick_open_ui(&mut self, ctx: &egui::Context) {
        let Some((mut query, mut sel)) = self.quick_open.take() else {
            return;
        };
        let q = query.to_lowercase();
        let mut found: Vec<(i32, &String)> = self
            .files
            .iter()
            .filter_map(|f| fuzzy(&q, f).map(|s| (s, f)))
            .collect();
        found.sort_by_key(|m| m.0);
        let found: Vec<String> = found.into_iter().take(14).map(|m| m.1.clone()).collect();
        if ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::ArrowDown)) {
            sel = (sel + 1).min(found.len().saturating_sub(1));
        }
        if ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::ArrowUp)) {
            sel = sel.saturating_sub(1);
        }
        let mut open = None;
        let mut keep = true;
        egui::Area::new(Id::new("quick_open"))
            .order(Order::Foreground)
            .fixed_pos(ctx.screen_rect().center_top() + egui::vec2(0.0, 40.0))
            .pivot(Align2::CENTER_TOP)
            .show(ctx, |ui| {
                Frame::new()
                    .fill(WIDGET)
                    .stroke(Stroke::new(1.0_f32, Color32::from_rgb(0x3a, 0x3a, 0x3a)))
                    .corner_radius(CornerRadius::same(8))
                    .inner_margin(Margin::same(6))
                    .shadow(egui::epaint::Shadow {
                        offset: [0, 6],
                        blur: 20,
                        spread: 0,
                        color: Color32::from_black_alpha(140),
                    })
                    .show(ui, |ui| {
                        ui.set_width(560.0);
                        let resp = ui.add(
                            TextEdit::singleline(&mut query)
                                .hint_text("Search files by name")
                                .desired_width(f32::INFINITY),
                        );
                        resp.request_focus();
                        if resp.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter)) {
                            open = found.get(sel).cloned();
                            keep = false;
                        }
                        if ui.input(|i| i.key_pressed(Key::Escape)) {
                            keep = false;
                        }
                        if self.files.is_empty() {
                            ui.label(RichText::new("Open a folder first (Ctrl+O).").color(MUTED));
                        }
                        for (n, f) in found.iter().enumerate() {
                            let name = f.rsplit('/').next().unwrap_or(f);
                            if ui
                                .selectable_label(
                                    n == sel,
                                    RichText::new(format!("{name}    {f}")).size(12.5),
                                )
                                .clicked()
                            {
                                open = Some(f.clone());
                            }
                        }
                    });
            });
        if let Some(f) = open {
            self.open_file(&f);
        } else if keep {
            self.quick_open = Some((query, sel));
        }
    }

    // ------------------------------------------------------------------ settings

    pub fn settings_window(&mut self, ctx: &egui::Context) {
        if !self.settings_open {
            return;
        }
        let mut open = true;
        egui::Window::new(RichText::new("OpenCode Settings").strong())
            .open(&mut open)
            .collapsible(false)
            .resizable(true)
            .default_size([680.0, 470.0])
            .pivot(Align2::CENTER_CENTER)
            .default_pos(ctx.screen_rect().center())
            .show(ctx, |ui| {
                ui.horizontal_top(|ui| {
                    ui.vertical(|ui| {
                        ui.set_width(130.0);
                        for (n, name) in ["Models", "Indexing", "Chat", "Shortcuts"]
                            .iter()
                            .enumerate()
                        {
                            if ui.selectable_label(self.settings_tab == n, *name).clicked() {
                                self.settings_tab = n;
                            }
                        }
                    });
                    ui.separator();
                    ui.vertical(|ui| match self.settings_tab {
                        0 => self.settings_models(ui, ctx),
                        1 => self.settings_indexing(ui, ctx),
                        2 => self.settings_chat(ui),
                        _ => settings_shortcuts(ui),
                    });
                });
            });
        if !open {
            self.settings_open = false;
            if self.persist {
                if let Err(e) = self.settings.save() {
                    self.notice = format!("Could not save settings: {e}");
                }
            }
        }
    }

    fn settings_models(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        ui.heading("Models");
        ui.label(
            RichText::new("Choose ChatGPT plan sign-in or a provider API key.")
                .color(MUTED)
                .size(12.0),
        );
        ui.add_space(8.0);
        egui::Grid::new("models")
            .num_columns(2)
            .spacing([12.0, 8.0])
            .show(ui, |ui| {
                ui.label("Provider");
                let mut provider = self.settings.provider;
                egui::ComboBox::from_id_salt("provider")
                    .selected_text(provider.name())
                    .width(260.0)
                    .show_ui(ui, |ui| {
                        for p in Provider::ALL {
                            ui.selectable_value(&mut provider, p, p.name());
                        }
                    });
                if provider != self.settings.provider {
                    self.settings.provider = provider;
                    self.settings.base_url = provider.default_base_url().into();
                    self.settings.model = provider.default_model().into();
                }
                ui.end_row();
                ui.label("Model");
                if self.settings.provider == Provider::ChatGpt
                    && !crate::chatgpt::models().is_empty()
                {
                    egui::ComboBox::from_id_salt("chatgpt_model")
                        .selected_text(&self.settings.model)
                        .width(300.0)
                        .show_ui(ui, |ui| {
                            for (slug, display) in crate::chatgpt::models() {
                                ui.selectable_value(&mut self.settings.model, slug, display);
                            }
                        });
                } else {
                    ui.add(TextEdit::singleline(&mut self.settings.model).desired_width(300.0));
                }
                ui.end_row();
                if self.settings.provider != Provider::ChatGpt {
                    ui.label("Address");
                    ui.add(TextEdit::singleline(&mut self.settings.base_url).desired_width(300.0));
                    ui.end_row();
                    ui.label("API key");
                    let hint = if self.settings.provider.needs_key() {
                        "Paste your key"
                    } else {
                        "Not needed for a local model"
                    };
                    ui.add(
                        TextEdit::singleline(&mut self.settings.api_key)
                            .password(true)
                            .hint_text(hint)
                            .desired_width(300.0),
                    );
                    ui.end_row();
                }
            });
        if self.settings.provider == Provider::ChatGpt {
            ui.add_space(8.0);
            let (busy, status) = crate::chatgpt::status();
            if ui
                .add_enabled(!busy, egui::Button::new("Continue with ChatGPT"))
                .clicked()
            {
                crate::chatgpt::sign_in(ctx.clone());
            }
            ui.horizontal_wrapped(|ui| {
                ui.label(RichText::new(&status).size(12.0).color(MUTED));
                if ui.small_button("Copy status").clicked() {
                    ctx.copy_text(status.clone());
                }
            });
            ui.label(RichText::new("Uses your eligible ChatGPT plan through OpenAI sign-in. Sign in again after restarting OpenCode; no OAuth tokens are saved to disk.").size(11.0).color(FAINT));
        }
        ui.add_space(8.0);
        ui.horizontal(|ui| {
            let testing = self
                .jobs
                .values()
                .any(|(job, _)| matches!(job, crate::app::Job::Test));
            if ui
                .add_enabled(
                    !testing,
                    egui::Button::new(if testing {
                        "Testing…"
                    } else {
                        "Test connection"
                    }),
                )
                .clicked()
            {
                self.test_connection(ctx);
            }
            if ui.button("Save").clicked() {
                self.notice = match self.settings.save() {
                    Ok(()) => "Settings saved.".into(),
                    Err(e) => format!("Could not save settings: {e}"),
                };
            }
        });
        if !self.test_result.is_empty() {
            ui.label(RichText::new(&self.test_result).size(12.0).color(MUTED));
        }
        ui.add_space(6.0);
        ui.label(
            RichText::new(format!("Settings file: {}", Settings::path().display()))
                .size(11.0)
                .color(FAINT),
        );
    }

    fn settings_indexing(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        ui.heading("Codebase Indexing");
        ui.label(RichText::new("OpenCode reads every text file in the open folder and builds a search index on this computer, so you can ask where things are and @codebase finds the right code. Nothing is uploaded.").color(MUTED).size(12.0));
        ui.add_space(8.0);
        let (running, done, total) = {
            let s = self.index_state.lock().unwrap();
            (s.running, s.done, s.total)
        };
        if running {
            let fraction = if total > 0 {
                done as f32 / total as f32
            } else {
                0.0
            };
            ui.add(egui::ProgressBar::new(fraction).text(format!("{done} / {total} files")));
        }
        ui.label(RichText::new(self.index_summary()).color(BRIGHT));
        if let Ok(idx) = self.index.read() {
            if idx.root.is_some() {
                ui.label(
                    RichText::new(format!(
                        "{} files reused unchanged · {} binary and {} large files skipped · built in {} ms · fingerprint {:016x}",
                        idx.reused, idx.skipped_binary, idx.skipped_large, idx.millis, idx.fingerprint
                    ))
                    .size(11.0)
                    .color(FAINT),
                );
            }
        }
        ui.horizontal(|ui| {
            if ui
                .add_enabled(!running && self.root.is_some(), egui::Button::new("⟳ Sync"))
                .clicked()
            {
                self.start_indexing(ctx);
            }
            if ui
                .add_enabled(!running, egui::Button::new("Delete Index"))
                .clicked()
            {
                self.delete_index();
            }
        });
        ui.checkbox(
            &mut self.settings.index_on_open,
            "Index new folders automatically",
        );
        ui.add_space(6.0);
        ui.label(
            RichText::new(format!(
                "Files listed in .gitignore, .opencodeignore or .cursorignore are skipped. Up to {MAX_FILES} files; code is packed whole block by whole block into pieces of {CHUNK_MIN} to {CHUNK_MAX} lines, as the original's chunker does."
            ))
            .size(11.0)
            .color(FAINT),
        );
    }

    fn settings_chat(&mut self, ui: &mut egui::Ui) {
        ui.heading("Chat");
        ui.checkbox(
            &mut self.settings.auto_codebase_context,
            "Search the codebase automatically when no files are mentioned",
        );
        ui.checkbox(
            &mut self.settings.include_current_file,
            "Send the current file with every message",
        );
        ui.add_space(8.0);
        ui.label("Editor font size");
        ui.add(egui::Slider::new(&mut self.settings.font_size, 10.0..=22.0));
    }
}

fn settings_shortcuts(ui: &mut egui::Ui) {
    ui.heading("Keyboard Shortcuts");
    egui::Grid::new("keys")
        .num_columns(2)
        .spacing([24.0, 6.0])
        .striped(true)
        .show(ui, |ui| {
            for (k, v) in [
                (
                    "Ctrl+K",
                    "Edit the selection with AI, or generate code at the cursor",
                ),
                ("Ctrl+Shift+K", "Toggle the edit bar"),
                ("Ctrl+L", "Open chat (Ask) and add the selection"),
                ("Ctrl+L in the edit bar", "Move the prompt into a chat"),
                ("Ctrl+I", "Open the agent (multi-file edits)"),
                ("Ctrl+Shift+L / I", "New agent chat"),
                ("Ctrl+Enter", "Accept the AI change"),
                ("Ctrl+Backspace", "Reject the AI change"),
                ("@", "Mention a file, folder or @codebase in chat"),
                ("Ctrl+P", "Go to file"),
                ("Ctrl+Shift+F", "Search the codebase"),
                ("Ctrl+S / Ctrl+W", "Save / close tab"),
                ("Ctrl+B", "Toggle side bar"),
                ("Ctrl+,", "Settings"),
            ] {
                ui.label(RichText::new(k).monospace().color(BRIGHT));
                ui.label(RichText::new(v).size(12.0).color(MUTED));
                ui.end_row();
            }
        });
}

fn pill(ui: &mut egui::Ui, text: &str) {
    Frame::new()
        .fill(RAISED)
        .corner_radius(CornerRadius::same(4))
        .inner_margin(Margin::symmetric(6, 2))
        .show(ui, |ui| {
            ui.label(RichText::new(text).size(11.0).color(MUTED));
        });
}

fn change_card(
    ui: &mut egui::Ui,
    change: &FileChange,
    state: ChangeState,
    pending: Option<usize>,
    original: Option<&str>,
    acts: &mut Vec<Act>,
) {
    Frame::new()
        .fill(WIDGET)
        .stroke(Stroke::new(1.0_f32, BORDER))
        .corner_radius(CornerRadius::same(6))
        .inner_margin(Margin::symmetric(8, 6))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                let (badge, color) = file_badge(&change.rel);
                ui.label(RichText::new(badge).size(10.0).color(color));
                if ui
                    .add(
                        egui::Label::new(RichText::new(&change.rel).size(12.5).color(BRIGHT))
                            .sense(Sense::click()),
                    )
                    .on_hover_text("Show the change")
                    .clicked()
                {
                    acts.push(Act::Open(change.rel.clone()));
                }
                match (&change.content, pending) {
                    (Some(text), Some(_)) => {
                        let (a, r) = diff::counts(original.unwrap_or(""), text);
                        ui.label(RichText::new(format!("+{a}")).size(11.5).color(ADDED));
                        ui.label(RichText::new(format!("−{r}")).size(11.5).color(REMOVED));
                    }
                    (None, _) => {
                        ui.label(RichText::new("delete").size(11.5).color(REMOVED));
                    }
                    _ => {}
                }
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    match (state, pending) {
                        (ChangeState::Accepted, _) => {
                            ui.label(RichText::new("✔ Applied").size(11.5).color(ADDED));
                        }
                        (ChangeState::Rejected, _) => {
                            ui.label(RichText::new("Rejected").size(11.5).color(FAINT));
                        }
                        (ChangeState::Pending, Some(i)) => {
                            if ui.small_button("Reject").clicked() {
                                acts.push(Act::Reject(i));
                            }
                            if ui
                                .add(
                                    egui::Button::new(
                                        RichText::new("Accept").size(11.5).color(ACCENT_TEXT),
                                    )
                                    .fill(ADDED),
                                )
                                .clicked()
                            {
                                acts.push(Act::Accept(i));
                            }
                        }
                        (ChangeState::Pending, None) => {
                            ui.label(RichText::new("waiting").size(11.5).color(FAINT));
                        }
                    }
                });
            });
        });
}

/// Minimal markdown: headings, bullets, **bold**, `code`, fenced code blocks with
/// Copy / Insert, and `[[marker]]` lines handed to `on_marker`.
fn markdown(
    ui: &mut egui::Ui,
    text: &str,
    acts: &mut Vec<Act>,
    mut on_marker: impl FnMut(&mut egui::Ui, &str, &mut Vec<Act>),
) {
    let width = ui.available_width();
    let mut in_code = false;
    let mut code = String::new();
    let mut lang = String::new();
    for line in text.lines() {
        let trimmed = line.trim_start();
        if trimmed.starts_with("```") {
            if in_code {
                code_block(ui, &code, &lang, acts);
                code.clear();
                in_code = false;
            } else {
                in_code = true;
                lang = trimmed.trim_start_matches('`').trim().to_string();
            }
            continue;
        }
        if in_code {
            code.push_str(line);
            code.push('\n');
            continue;
        }
        if let Some(marker) = trimmed
            .trim_end()
            .strip_prefix("[[")
            .and_then(|m| m.strip_suffix("]]"))
        {
            on_marker(ui, marker, acts);
            continue;
        }
        if trimmed.is_empty() {
            ui.add_space(4.0);
            continue;
        }
        let (size, color, body) = if let Some(h) = trimmed.strip_prefix("### ") {
            (13.5, BRIGHT, h.to_string())
        } else if let Some(h) = trimmed.strip_prefix("## ") {
            (14.5, BRIGHT, h.to_string())
        } else if let Some(h) = trimmed.strip_prefix("# ") {
            (15.5, BRIGHT, h.to_string())
        } else if let Some(b) = trimmed
            .strip_prefix("- ")
            .or_else(|| trimmed.strip_prefix("* "))
        {
            (13.0, TEXT, format!("•  {b}"))
        } else {
            (13.0, TEXT, line.to_string())
        };
        let mut job = LayoutJob::default();
        job.wrap.max_width = width;
        let mut bold = size > 13.0;
        for (n, part) in body.split('`').enumerate() {
            let inline_code = n % 2 == 1;
            for (m, piece) in part.split("**").enumerate() {
                if m > 0 && !inline_code {
                    bold = !bold;
                }
                if piece.is_empty() {
                    continue;
                }
                let format = if inline_code {
                    TextFormat {
                        font_id: FontId::monospace(12.0),
                        color: KEYWORD,
                        background: RAISED,
                        ..Default::default()
                    }
                } else {
                    TextFormat {
                        font_id: FontId::proportional(size),
                        color: if bold { BRIGHT } else { color },
                        ..Default::default()
                    }
                };
                job.append(piece, 0.0, format);
            }
        }
        ui.label(job);
    }
    if in_code && !code.is_empty() {
        code_block(ui, &code, &lang, acts);
    }
}

fn code_block(ui: &mut egui::Ui, code: &str, lang: &str, acts: &mut Vec<Act>) {
    Frame::new()
        .fill(EDITOR)
        .stroke(Stroke::new(1.0_f32, BORDER))
        .corner_radius(CornerRadius::same(6))
        .inner_margin(Margin::same(8))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new(if lang.is_empty() { "code" } else { lang })
                        .size(11.0)
                        .color(FAINT),
                );
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if ui
                        .small_button("Insert")
                        .on_hover_text("Insert at the cursor in the open file")
                        .clicked()
                    {
                        acts.push(Act::Insert(code.to_string()));
                    }
                    if ui.small_button("Copy").clicked() {
                        acts.push(Act::Copy(code.to_string()));
                    }
                });
            });
            let ext = match lang.split(':').next().unwrap_or("") {
                "rust" => "rs",
                "python" => "py",
                "javascript" => "js",
                "typescript" | "tsx" => "ts",
                "bash" | "shell" | "powershell" => "sh",
                other => other,
            };
            let mut job = highlight::job(
                ui.ctx(),
                code.trim_end_matches('\n'),
                lang_for(&format!("x.{ext}")),
                12.0,
            );
            job.wrap.max_width = ui.available_width();
            ui.label(job);
        });
}

fn split_source(s: &str) -> (String, usize) {
    match s.rsplit_once(':') {
        Some((rel, lines)) => (
            rel.to_string(),
            lines
                .split('-')
                .next()
                .and_then(|n| n.parse().ok())
                .unwrap_or(1),
        ),
        None => (s.to_string(), 1),
    }
}

fn fuzzy(q: &str, path: &str) -> Option<i32> {
    if q.is_empty() {
        return Some(path.len() as i32);
    }
    let lower = path.to_lowercase();
    let name = lower.rsplit('/').next().unwrap_or(&lower);
    if name.starts_with(q) {
        return Some(lower.len() as i32);
    }
    if lower.contains(q) {
        return Some(500 + lower.len() as i32);
    }
    let mut it = lower.chars();
    for c in q.chars() {
        it.find(|x| *x == c)?;
    }
    Some(2000 + lower.len() as i32)
}
