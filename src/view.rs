//! Window layout: title bar, status bar, explorer and search side bar, the
//! editor with the Ctrl+K bar and inline diff review.

use eframe::egui::{
    self, Align, Align2, Color32, CornerRadius, Event, FontId, Frame, Id, Key, KeyboardShortcut,
    Layout, Margin, Modifiers, Order, RichText, ScrollArea, Sense, Stroke, TextEdit, Vec2,
    ViewportCommand,
};

use crate::app::{char_to_byte, App, Mode, Origin, SideView};
use crate::diff::{self, Kind};
use crate::highlight::{self, lang_for, language_name};
use crate::theme::*;

pub enum Act {
    Open(String),
    OpenAt(String, usize),
    ToggleDir(String),
    Accept(usize),
    Reject(usize),
    AcceptAll,
    RejectAll,
    Copy(String),
    Insert(String),
    Settings,
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.poll();
        self.shortcuts(ctx);
        if self.agents_view && (self.inline.is_some() || self.quick_open.is_some()) {
            // Ctrl+K and Ctrl+P work on files, so they bring the editor forward
            self.agents_view = false;
        }
        self.title_bar(ctx);
        if self.agents_view {
            self.agents_ui(ctx);
        } else {
            self.status_bar(ctx);
            if self.sidebar_open {
                self.sidebar(ctx);
            }
            if self.chat_open {
                self.chat_panel(ctx);
            }
            self.central(ctx);
        }
        self.quick_open_ui(ctx);
        self.settings_window(ctx);
        self.screenshot_step(ctx);
        let indexing = self.index_state.lock().map(|s| s.running).unwrap_or(false);
        if !self.jobs.is_empty() || indexing {
            ctx.request_repaint_after(std::time::Duration::from_millis(120));
        }
    }

    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        if self.persist {
            let _ = self.settings.save();
            self.save_history();
        }
    }
}

fn pressed(ctx: &egui::Context, modifiers: Modifiers, key: Key) -> bool {
    ctx.input_mut(|i| i.consume_shortcut(&KeyboardShortcut::new(modifiers, key)))
}

pub fn file_badge(name: &str) -> (&'static str, Color32) {
    let ext = name.rsplit('.').next().unwrap_or("").to_lowercase();
    match ext.as_str() {
        "rs" => ("RS", Color32::from_rgb(0xde, 0x8a, 0x5a)),
        "ts" | "tsx" => ("TS", Color32::from_rgb(0x4d, 0x9b, 0xe6)),
        "js" | "jsx" | "mjs" => ("JS", Color32::from_rgb(0xe8, 0xd4, 0x4d)),
        "py" => ("PY", Color32::from_rgb(0x6a, 0xa6, 0xd8)),
        "json" => ("{}", Color32::from_rgb(0xcb, 0xcb, 0x41)),
        "toml" | "yaml" | "yml" | "ini" => ("⚙", Color32::from_rgb(0x9a, 0x9a, 0x9a)),
        "md" => ("M↓", Color32::from_rgb(0x51, 0x9a, 0xba)),
        "html" => ("<>", Color32::from_rgb(0xe3, 0x7d, 0x45)),
        "css" | "scss" => ("#", Color32::from_rgb(0x56, 0x9c, 0xd6)),
        "go" => ("GO", Color32::from_rgb(0x4f, 0xc1, 0xd6)),
        _ => ("•", Color32::from_rgb(0x8a, 0x8a, 0x8a)),
    }
}

impl App {
    fn shortcuts(&mut self, ctx: &egui::Context) {
        let cs = Modifiers::COMMAND | Modifiers::SHIFT;
        if pressed(ctx, cs, Key::K) {
            if self.inline.is_some() {
                self.close_inline();
            } else {
                self.start_inline();
            }
        } else if pressed(ctx, Modifiers::COMMAND, Key::K) {
            self.start_inline();
        }
        if pressed(ctx, cs, Key::L) || pressed(ctx, cs, Key::I) {
            self.new_chat(Mode::Agent);
        }
        if pressed(ctx, Modifiers::COMMAND, Key::L) {
            let bar_focused = ctx.memory(|m| m.has_focus(Id::new("inline_prompt")));
            if bar_focused && self.inline.is_some() {
                self.inline_to_chat();
            } else {
                self.open_chat(Mode::Ask, true);
            }
        }
        if pressed(ctx, Modifiers::COMMAND, Key::I) {
            self.open_chat(Mode::Agent, true);
        }
        if !self.pending.is_empty() {
            if pressed(ctx, Modifiers::COMMAND, Key::Enter) {
                match self.pending_for_active() {
                    Some(i) => self.accept(i),
                    None => self.accept_all_agent(),
                }
            }
            if pressed(ctx, Modifiers::COMMAND, Key::Backspace) {
                match self.pending_for_active() {
                    Some(i) => self.reject(i),
                    None => self.reject_all_agent(),
                }
            }
        }
        if pressed(ctx, Modifiers::COMMAND, Key::S) {
            self.save_active();
        }
        if pressed(ctx, Modifiers::COMMAND, Key::P) {
            self.quick_open = Some((String::new(), 0));
        }
        if pressed(ctx, Modifiers::COMMAND, Key::O) {
            self.pick_workspace(ctx);
        }
        if pressed(ctx, Modifiers::COMMAND, Key::W) {
            if let Some(i) = self.active {
                self.close_tab(i);
            }
        }
        if pressed(ctx, Modifiers::COMMAND, Key::E) {
            self.agents_view = !self.agents_view;
        }
        if pressed(ctx, Modifiers::COMMAND, Key::B) {
            self.sidebar_open = !self.sidebar_open;
        }
        if pressed(ctx, cs, Key::F) {
            self.sidebar_open = true;
            self.side = SideView::Search;
        }
        if pressed(ctx, cs, Key::E) {
            self.sidebar_open = true;
            self.side = SideView::Explorer;
        }
        if pressed(ctx, Modifiers::COMMAND, Key::Comma) {
            self.settings_open = true;
        }
        if ctx.input(|i| i.key_pressed(Key::Escape)) {
            if self.quick_open.is_some() {
                self.quick_open = None;
            } else if self
                .inline
                .as_ref()
                .map(|i| i.job.is_none())
                .unwrap_or(false)
                && self.pending_for_active().is_none()
            {
                self.close_inline();
            }
        }
    }

    // ------------------------------------------------------------------ bars

    fn title_bar(&mut self, ctx: &egui::Context) {
        egui::TopBottomPanel::top("title")
            .exact_height(34.0)
            .frame(
                Frame::new()
                    .fill(SIDE)
                    .inner_margin(Margin::symmetric(10, 0))
                    .stroke(Stroke::new(1.0_f32, BORDER)),
            )
            .show(ctx, |ui| {
                ui.horizontal_centered(|ui| {
                    let (r, _) = ui.allocate_exact_size(Vec2::splat(16.0), Sense::hover());
                    ui.painter().rect_filled(r, CornerRadius::same(4), BRIGHT);
                    ui.painter().text(
                        r.center(),
                        Align2::CENTER_CENTER,
                        "</>",
                        FontId::monospace(8.0),
                        SIDE,
                    );
                    ui.add_space(6.0);
                    ui.menu_button(RichText::new("File").color(MUTED), |ui| {
                        if ui.button("Open Folder…   Ctrl+O").clicked() {
                            ui.close_menu();
                            self.pick_workspace(ctx);
                        }
                        if ui.button("Go to File…     Ctrl+P").clicked() {
                            ui.close_menu();
                            self.quick_open = Some((String::new(), 0));
                        }
                        if ui.button("Save                  Ctrl+S").clicked() {
                            ui.close_menu();
                            self.save_active();
                        }
                        if ui.button("Close Tab         Ctrl+W").clicked() {
                            ui.close_menu();
                            if let Some(i) = self.active {
                                self.close_tab(i);
                            }
                        }
                        ui.separator();
                        if ui.button("Settings            Ctrl+,").clicked() {
                            ui.close_menu();
                            self.settings_open = true;
                        }
                    });
                    ui.menu_button(RichText::new("View").color(MUTED), |ui| {
                        if ui.button("Explorer               Ctrl+Shift+E").clicked() {
                            ui.close_menu();
                            self.sidebar_open = true;
                            self.side = SideView::Explorer;
                        }
                        if ui.button("Search Codebase  Ctrl+Shift+F").clicked() {
                            ui.close_menu();
                            self.sidebar_open = true;
                            self.side = SideView::Search;
                        }
                        if ui.button("Toggle Side Bar     Ctrl+B").clicked() {
                            ui.close_menu();
                            self.sidebar_open = !self.sidebar_open;
                        }
                        if ui.button("Toggle Chat Panel").clicked() {
                            ui.close_menu();
                            self.chat_open = !self.chat_open;
                        }
                    });
                    ui.menu_button(RichText::new("AI").color(MUTED), |ui| {
                        if ui.button("Edit with AI               Ctrl+K").clicked() {
                            ui.close_menu();
                            self.start_inline();
                        }
                        if ui.button("Chat (Ask)                  Ctrl+L").clicked() {
                            ui.close_menu();
                            self.open_chat(Mode::Ask, true);
                        }
                        if ui.button("Agent (multi-file)       Ctrl+I").clicked() {
                            ui.close_menu();
                            self.open_chat(Mode::Agent, true);
                        }
                        if ui.button("New Agent Chat    Ctrl+Shift+L").clicked() {
                            ui.close_menu();
                            self.new_chat(Mode::Agent);
                        }
                        ui.separator();
                        if ui.button("Sync Codebase Index").clicked() {
                            ui.close_menu();
                            self.start_indexing(ctx);
                        }
                    });

                    let full = ui.max_rect();
                    let w = 440.0_f32.min(full.width() * 0.4);
                    let rect = egui::Rect::from_center_size(full.center(), egui::vec2(w, 22.0));
                    let resp = ui.put(
                        rect,
                        egui::Button::new(
                            RichText::new(format!("🔍  {}", self.workspace_name()))
                                .color(MUTED)
                                .size(12.5),
                        )
                        .fill(Color32::from_rgb(0x1e, 0x1e, 0x1e))
                        .stroke(Stroke::new(1.0_f32, BORDER))
                        .corner_radius(CornerRadius::same(6)),
                    );
                    if resp.on_hover_text("Go to file (Ctrl+P)").clicked() {
                        self.quick_open = Some((String::new(), 0));
                    }

                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if self.agents_view {
                            if ui
                                .add(
                                    egui::Button::new(
                                        RichText::new("Editor Window ↗").size(12.5).color(MUTED),
                                    )
                                    .frame(false),
                                )
                                .on_hover_text("Open the code editor (Ctrl+E)")
                                .clicked()
                            {
                                self.agents_view = false;
                            }
                            return;
                        }
                        if ui
                            .add(
                                egui::Button::new(RichText::new("⚙").size(15.0).color(MUTED))
                                    .frame(false),
                            )
                            .on_hover_text("Settings (Ctrl+,)")
                            .clicked()
                        {
                            self.settings_open = true;
                        }
                        let chat = if self.chat_open { BRIGHT } else { MUTED };
                        if ui
                            .add(
                                egui::Button::new(RichText::new("💬").size(14.0).color(chat))
                                    .frame(false),
                            )
                            .on_hover_text("Toggle chat panel")
                            .clicked()
                        {
                            self.chat_open = !self.chat_open;
                        }
                        let side = if self.sidebar_open { BRIGHT } else { MUTED };
                        if ui
                            .add(
                                egui::Button::new(RichText::new("🗐").size(14.0).color(side))
                                    .frame(false),
                            )
                            .on_hover_text("Toggle side bar (Ctrl+B)")
                            .clicked()
                        {
                            self.sidebar_open = !self.sidebar_open;
                        }
                        ui.add_space(6.0);
                        if ui
                            .add(
                                egui::Button::new(
                                    RichText::new("Agents Window").size(12.5).color(MUTED),
                                )
                                .frame(false),
                            )
                            .on_hover_text("Back to the agents home (Ctrl+E)")
                            .clicked()
                        {
                            self.agents_view = true;
                        }
                    });
                });
            });
    }

    fn status_bar(&mut self, ctx: &egui::Context) {
        egui::TopBottomPanel::bottom("status")
            .exact_height(24.0)
            .frame(
                Frame::new()
                    .fill(SIDE)
                    .inner_margin(Margin::symmetric(10, 0))
                    .stroke(Stroke::new(1.0_f32, BORDER)),
            )
            .show(ctx, |ui| {
                ui.horizontal_centered(|ui| {
                    let small = |t: String| RichText::new(t).size(11.5).color(MUTED);
                    ui.label(small(format!("🗀 {}", self.workspace_name())));
                    ui.add_space(8.0);
                    ui.label(small(self.notice.clone()));
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        let model = if self.settings.ready() {
                            self.settings.model.clone()
                        } else {
                            "Set up a model".into()
                        };
                        if ui
                            .add(egui::Button::new(small(model)).frame(false))
                            .clicked()
                        {
                            self.settings_open = true;
                            self.settings_tab = 0;
                        }
                        let summary = self.index_summary();
                        if ui
                            .add(egui::Button::new(small(format!("⟳ {summary}"))).frame(false))
                            .on_hover_text("Codebase index — click to sync")
                            .clicked()
                        {
                            self.start_indexing(ctx);
                        }
                        if let Some(i) = self.active {
                            let tab = &self.tabs[i];
                            let at = char_to_byte(&tab.text, tab.sel.1);
                            let line = tab.text[..at].matches('\n').count() + 1;
                            let col = tab.text[..at]
                                .rsplit('\n')
                                .next()
                                .map(|l| l.chars().count())
                                .unwrap_or(0)
                                + 1;
                            ui.label(small(language_name(&tab.rel).into()));
                            ui.label(small("UTF-8".into()));
                            ui.label(small(format!("Ln {line}, Col {col}")));
                        }
                    });
                });
            });
    }

    // ------------------------------------------------------------------ side bar

    fn sidebar(&mut self, ctx: &egui::Context) {
        let mut acts = Vec::new();
        egui::SidePanel::left("side")
            .default_width(250.0)
            .min_width(170.0)
            .resizable(true)
            .frame(
                Frame::new()
                    .fill(SIDE)
                    .inner_margin(Margin::symmetric(0, 6))
                    .stroke(Stroke::new(1.0_f32, BORDER)),
            )
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.add_space(8.0);
                    for (view, icon, tip) in [
                        (SideView::Explorer, "🗐", "Explorer (Ctrl+Shift+E)"),
                        (SideView::Search, "🔍", "Search codebase (Ctrl+Shift+F)"),
                    ] {
                        let color = if self.side == view { BRIGHT } else { FAINT };
                        if ui
                            .add(
                                egui::Button::new(RichText::new(icon).size(15.0).color(color))
                                    .frame(false),
                            )
                            .on_hover_text(tip)
                            .clicked()
                        {
                            self.side = view;
                        }
                    }
                });
                ui.add_space(4.0);
                match self.side {
                    SideView::Explorer => self.explorer(ui, ctx, &mut acts),
                    SideView::Search => self.search_view(ui, ctx, &mut acts),
                }
            });
        self.apply(acts, ctx);
    }

    fn explorer(&mut self, ui: &mut egui::Ui, ctx: &egui::Context, acts: &mut Vec<Act>) {
        ui.horizontal(|ui| {
            ui.add_space(12.0);
            ui.label(RichText::new("EXPLORER").size(11.0).color(MUTED));
        });
        if self.root.is_none() {
            ui.add_space(10.0);
            ui.horizontal(|ui| {
                ui.add_space(12.0);
                ui.vertical(|ui| {
                    ui.label(RichText::new("You have not yet opened a folder.").color(MUTED));
                    ui.add_space(6.0);
                    if ui
                        .add(
                            egui::Button::new(RichText::new("Open Folder").color(ACCENT_TEXT))
                                .fill(ACCENT),
                        )
                        .clicked()
                    {
                        self.pick_workspace(ctx);
                    }
                });
            });
            return;
        }
        ui.horizontal(|ui| {
            ui.add_space(8.0);
            ui.label(
                RichText::new(format!("⏷ {}", self.workspace_name().to_uppercase()))
                    .size(11.5)
                    .strong()
                    .color(TEXT),
            );
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                ui.add_space(8.0);
                if ui
                    .add(egui::Button::new(RichText::new("⟳").color(MUTED)).frame(false))
                    .on_hover_text("Refresh")
                    .clicked()
                {
                    self.refresh_files();
                }
                if ui
                    .add(egui::Button::new(RichText::new("⊟").color(MUTED)).frame(false))
                    .on_hover_text("Collapse folders")
                    .clicked()
                {
                    self.expanded.clear();
                }
            });
        });
        ScrollArea::vertical().auto_shrink(false).show(ui, |ui| {
            ui.spacing_mut().item_spacing.y = 0.0;
            self.tree_rows(ui, "", 0, acts);
        });
    }

    fn tree_rows(&self, ui: &mut egui::Ui, parent: &str, depth: usize, acts: &mut Vec<Act>) {
        let Some(children) = self.tree.get(parent) else {
            return;
        };
        let active = self.active.map(|i| self.tabs[i].rel.as_str());
        for (name, rel, is_dir) in children {
            let (rect, resp) =
                ui.allocate_exact_size(egui::vec2(ui.available_width(), 22.0), Sense::click());
            let selected = active == Some(rel.as_str());
            if selected {
                ui.painter().rect_filled(
                    rect,
                    CornerRadius::ZERO,
                    Color32::from_rgb(0x2a, 0x2d, 0x2e),
                );
            } else if resp.hovered() {
                ui.painter().rect_filled(
                    rect,
                    CornerRadius::ZERO,
                    Color32::from_rgb(0x1f, 0x1f, 0x1f),
                );
            }
            let x = rect.left() + 12.0 + depth as f32 * 12.0;
            let open = self.expanded.contains(rel);
            let (icon, icon_color) = if *is_dir {
                (if open { "⏷" } else { "⏵" }, MUTED)
            } else {
                file_badge(name)
            };
            ui.painter().text(
                egui::pos2(x, rect.center().y),
                Align2::LEFT_CENTER,
                icon,
                FontId::proportional(9.5),
                icon_color,
            );
            let prefix = format!("{rel}/");
            let pending = self
                .pending
                .iter()
                .find(|p| &p.rel == rel || (*is_dir && p.rel.starts_with(&prefix)));
            let color = match pending {
                Some(p) if p.proposed.is_none() => REMOVED,
                Some(p) if p.is_new => ADDED,
                Some(_) => NUMBER,
                None if selected => BRIGHT,
                None => TEXT,
            };
            ui.painter().text(
                egui::pos2(x + 18.0, rect.center().y),
                Align2::LEFT_CENTER,
                name,
                FontId::proportional(13.0),
                color,
            );
            if let (Some(p), false) = (pending, *is_dir) {
                let mark = if p.proposed.is_none() {
                    "D"
                } else if p.is_new {
                    "A"
                } else {
                    "M"
                };
                ui.painter().text(
                    egui::pos2(rect.right() - 12.0, rect.center().y),
                    Align2::RIGHT_CENTER,
                    mark,
                    FontId::proportional(11.0),
                    color,
                );
            }
            if resp.clicked() {
                acts.push(if *is_dir {
                    Act::ToggleDir(rel.clone())
                } else {
                    Act::Open(rel.clone())
                });
            }
            if *is_dir && open {
                self.tree_rows(ui, rel, depth + 1, acts);
            }
        }
    }

    fn search_view(&mut self, ui: &mut egui::Ui, ctx: &egui::Context, acts: &mut Vec<Act>) {
        Frame::new().inner_margin(Margin::symmetric(12, 2)).show(ui, |ui| {
            ui.label(RichText::new("SEARCH CODEBASE").size(11.0).color(MUTED));
            ui.add_space(4.0);
            let resp = ui.add(TextEdit::singleline(&mut self.search_query).hint_text("Where is the payment code?").desired_width(f32::INFINITY));
            if resp.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter)) {
                self.start_search(ctx);
            }
            ui.horizontal(|ui| {
                if ui.button("Search").clicked() {
                    self.start_search(ctx);
                }
                ui.label(RichText::new(self.index_summary()).size(11.0).color(FAINT));
            });
            ui.label(
                RichText::new("Ask in any language: your AI model turns the question into code words, then the local index is searched.")
                    .size(11.0)
                    .color(FAINT),
            );
            if self.search_busy {
                ui.horizontal(|ui| {
                    ui.add(egui::Spinner::new().size(12.0));
                    ui.label(RichText::new("Turning your question into code words…").size(11.5).color(MUTED));
                });
            }
            if !self.search_keywords.trim().is_empty() {
                ui.label(RichText::new(format!("Searched for: {}", self.search_keywords.trim())).size(11.0).color(MUTED));
            }
            let ready = self.index.read().map(|i| i.is_ready()).unwrap_or(false);
            if !ready && self.root.is_some() && ui.button("Index this codebase").clicked() {
                self.start_indexing(ctx);
            }
        });
        ui.separator();
        ScrollArea::vertical().auto_shrink(false).show(ui, |ui| {
            if self.search_done && self.search_results.is_empty() {
                ui.horizontal(|ui| {
                    ui.add_space(12.0);
                    ui.label(RichText::new("No results.").color(MUTED));
                });
            }
            for hit in &self.search_results {
                Frame::new()
                    .inner_margin(Margin::symmetric(12, 4))
                    .show(ui, |ui| {
                        let label = RichText::new(format!("{}  :{}", hit.rel, hit.start_line))
                            .color(ACCENT)
                            .size(12.5);
                        if ui
                            .add(egui::Label::new(label).sense(Sense::click()))
                            .on_hover_text("Open")
                            .clicked()
                        {
                            acts.push(Act::OpenAt(hit.rel.clone(), hit.start_line));
                        }
                        let preview: Vec<&str> = hit
                            .snippet
                            .lines()
                            .filter(|l| !l.trim().is_empty())
                            .take(3)
                            .collect();
                        ui.label(
                            RichText::new(preview.join("\n"))
                                .monospace()
                                .size(11.0)
                                .color(FAINT),
                        );
                    });
            }
        });
    }

    // ------------------------------------------------------------------ editor

    fn central(&mut self, ctx: &egui::Context) {
        let mut acts = Vec::new();
        egui::CentralPanel::default()
            .frame(Frame::new().fill(EDITOR))
            .show(ctx, |ui| {
                if self.tabs.is_empty() {
                    self.watermark(ui, ctx);
                    return;
                }
                self.tab_bar(ui);
                let Some(i) = self.active else { return };
                let rel = self.tabs[i].rel.clone();
                Frame::new()
                    .inner_margin(Margin::symmetric(14, 3))
                    .show(ui, |ui| {
                        ui.label(
                            RichText::new(rel.replace('/', "  ›  "))
                                .size(11.5)
                                .color(MUTED),
                        );
                    });
                if let Some(p) = self.pending_for_active() {
                    self.diff_view(ui, ctx, p, &mut acts);
                } else {
                    self.editor_view(ui, ctx, i);
                }
            });
        self.apply(acts, ctx);
    }

    fn watermark(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        ui.vertical_centered(|ui| {
            ui.add_space(ui.available_height() * 0.22);
            ui.label(
                RichText::new("OpenCode")
                    .size(42.0)
                    .strong()
                    .color(Color32::from_rgb(0x30, 0x30, 0x30)),
            );
            ui.add_space(4.0);
            ui.label(
                RichText::new("Open-source AI code editor, written in Rust")
                    .size(13.0)
                    .color(FAINT),
            );
            ui.add_space(26.0);
            let rows: [(&str, &str, u8); 7] = [
                ("Open Folder", "Ctrl+O", 0),
                ("Go to File", "Ctrl+P", 1),
                ("Edit Code with AI", "Ctrl+K", 2),
                ("Chat with your Code", "Ctrl+L", 3),
                ("Agent: Change Many Files", "Ctrl+I", 4),
                ("Search Codebase", "Ctrl+Shift+F", 5),
                ("AI Settings", "Ctrl+,", 6),
            ];
            egui::Grid::new("watermark")
                .num_columns(2)
                .spacing([28.0, 10.0])
                .show(ui, |ui| {
                    for (label, key, act) in rows {
                        let resp = ui.add(
                            egui::Label::new(RichText::new(label).color(MUTED))
                                .sense(Sense::click()),
                        );
                        ui.label(RichText::new(key).monospace().color(FAINT));
                        ui.end_row();
                        if resp.clicked() {
                            match act {
                                0 => self.pick_workspace(ctx),
                                1 => self.quick_open = Some((String::new(), 0)),
                                2 => self.start_inline(),
                                3 => self.open_chat(Mode::Ask, false),
                                4 => self.open_chat(Mode::Agent, false),
                                5 => {
                                    self.sidebar_open = true;
                                    self.side = SideView::Search;
                                }
                                _ => self.settings_open = true,
                            }
                        }
                    }
                });
        });
    }

    fn tab_bar(&mut self, ui: &mut egui::Ui) {
        let mut close = None;
        let mut select = None;
        Frame::new().fill(SIDE).show(ui, |ui| {
            ui.set_width(ui.available_width());
            ScrollArea::horizontal().id_salt("tabs").show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 0.0;
                    for (i, tab) in self.tabs.iter().enumerate() {
                        let active = self.active == Some(i);
                        let pending = self.pending.iter().any(|p| p.rel == tab.rel);
                        Frame::new()
                            .fill(if active { EDITOR } else { SIDE })
                            .inner_margin(Margin::symmetric(12, 7))
                            .stroke(Stroke::new(1.0_f32, BORDER))
                            .show(ui, |ui| {
                                ui.spacing_mut().item_spacing.x = 5.0;
                                let (badge, color) = file_badge(tab.name());
                                ui.label(RichText::new(badge).size(10.0).color(color));
                                let name_color = if pending {
                                    NUMBER
                                } else if active {
                                    BRIGHT
                                } else {
                                    MUTED
                                };
                                if ui
                                    .add(
                                        egui::Label::new(
                                            RichText::new(tab.name()).size(12.5).color(name_color),
                                        )
                                        .sense(Sense::click()),
                                    )
                                    .clicked()
                                {
                                    select = Some(i);
                                }
                                let mark = if tab.dirty() { "●" } else { "×" };
                                if ui
                                    .add(
                                        egui::Button::new(
                                            RichText::new(mark).size(11.0).color(MUTED),
                                        )
                                        .frame(false),
                                    )
                                    .clicked()
                                {
                                    close = Some(i);
                                }
                            });
                    }
                });
            });
        });
        if let Some(i) = select {
            self.active = Some(i);
        }
        if let Some(i) = close {
            self.close_tab(i);
        }
    }

    fn editor_view(&mut self, ui: &mut egui::Ui, ctx: &egui::Context, i: usize) {
        let font_size = self.settings.font_size;
        let font = FontId::monospace(font_size);
        let row_h = ui.fonts(|f| f.row_height(&font));
        let char_w = ui.fonts(|f| f.glyph_width(&font, '0'));
        let rel = self.tabs[i].rel.clone();
        let lang = lang_for(&rel);
        let id = Id::new(("editor", &rel));
        let inline_span = self
            .inline
            .as_ref()
            .filter(|b| b.rel == rel)
            .map(|b| (b.line, b.start, b.end));
        let editor_rect = ui.available_rect_before_wrap();
        let mut anchor = None;
        let mut menu: Option<u8> = None;
        ScrollArea::both()
            .id_salt(("scroll", &rel))
            .auto_shrink(false)
            .show(ui, |ui| {
                let tab = &mut self.tabs[i];
                let lines = tab.text.split('\n').count();
                let digits = lines.to_string().len().max(3);
                let gutter_w = digits as f32 * char_w + 30.0;
                ui.horizontal_top(|ui| {
                    ui.spacing_mut().item_spacing.x = 0.0;
                    let (gutter, _) = ui.allocate_exact_size(
                        egui::vec2(gutter_w, lines as f32 * row_h + 8.0),
                        Sense::hover(),
                    );
                    let mut layouter = |ui: &egui::Ui, s: &str, _w: f32| {
                        let mut job = highlight::job(ui.ctx(), s, lang, font_size);
                        job.wrap.max_width = f32::INFINITY;
                        ui.fonts(|f| f.layout_job(job))
                    };
                    let out = TextEdit::multiline(&mut tab.text)
                        .id(id)
                        .font(font.clone())
                        .code_editor()
                        .frame(false)
                        .margin(Margin::symmetric(4, 4))
                        .desired_width(f32::INFINITY)
                        .desired_rows(lines.max(40))
                        .lock_focus(true)
                        .layouter(&mut layouter)
                        .show(ui);
                    if let Some(r) = out.cursor_range {
                        let a = r.primary.ccursor.index;
                        let b = r.secondary.ccursor.index;
                        tab.sel = (a.min(b), a.max(b));
                    }
                    let top = out.galley_pos.y;
                    if let Some(line) = tab.jump_to_line.take() {
                        let y = top + line.saturating_sub(1) as f32 * row_h;
                        ui.scroll_to_rect(
                            egui::Rect::from_min_size(
                                egui::pos2(out.galley_pos.x, y - 60.0),
                                egui::vec2(10.0, 200.0),
                            ),
                            Some(Align::TOP),
                        );
                        let ci: usize = tab
                            .text
                            .split('\n')
                            .take(line.saturating_sub(1))
                            .map(|l| l.chars().count() + 1)
                            .sum();
                        let mut state = out.state.clone();
                        state
                            .cursor
                            .set_char_range(Some(egui::text::CCursorRange::one(
                                egui::text::CCursor::new(ci),
                            )));
                        state.store(ui.ctx(), id);
                        tab.sel = (ci, ci);
                        out.response.request_focus();
                    }
                    let clip = ui.clip_rect();
                    let rows = &out.galley.rows;
                    let row_y = |n: usize| -> f32 {
                        match rows.get(n) {
                            Some(r) => top + r.rect.min.y,
                            None => {
                                top + rows.last().map(|r| r.rect.max.y).unwrap_or(0.0)
                                    + (n - rows.len()) as f32 * row_h
                            }
                        }
                    };
                    let at = char_to_byte(&tab.text, tab.sel.1);
                    let cur = tab.text[..at].matches('\n').count();
                    let painter = ui.painter();
                    for n in 0..lines.min(rows.len().max(1)) {
                        let y = row_y(n);
                        if y + row_h < clip.top() || y > clip.bottom() {
                            continue;
                        }
                        painter.text(
                            egui::pos2(gutter.right() - 16.0, y),
                            Align2::RIGHT_TOP,
                            (n + 1).to_string(),
                            font.clone(),
                            if n == cur { BRIGHT } else { LINE_NO },
                        );
                    }
                    if let Some((line, start, end)) = inline_span {
                        let y = row_y(line);
                        anchor = Some(egui::pos2(gutter.right(), y));
                        let n = tab
                            .text
                            .get(start..end.min(tab.text.len()))
                            .map(|s| s.matches('\n').count() + 1)
                            .unwrap_or(1);
                        painter.rect_filled(
                            egui::Rect::from_min_max(
                                egui::pos2(gutter.right() - 6.0, y),
                                egui::pos2(gutter.right() - 3.0, row_y(line + n)),
                            ),
                            CornerRadius::same(1),
                            ACCENT,
                        );
                    }
                    out.response.context_menu(|ui| {
                        for (n, label) in [
                            "Edit with AI          Ctrl+K",
                            "Add to Chat           Ctrl+L",
                            "Add to Agent         Ctrl+I",
                            "Explain This Code",
                        ]
                        .iter()
                        .enumerate()
                        {
                            if ui.button(*label).clicked() {
                                menu = Some(n as u8);
                                ui.close_menu();
                            }
                        }
                    });
                });
            });
        match menu {
            Some(0) => self.start_inline(),
            Some(1) => self.open_chat(Mode::Ask, true),
            Some(2) => self.open_chat(Mode::Agent, true),
            Some(3) => {
                self.new_chat(Mode::Ask);
                self.add_selection_to_chat();
                self.input = "Explain how this code works, step by step.".into();
                self.send_chat(ctx);
            }
            _ => {}
        }
        if let Some(anchor) = anchor {
            let y = anchor
                .y
                .clamp(editor_rect.top() + 110.0, editor_rect.bottom() - 10.0);
            let x = (anchor.x + 8.0)
                .min(editor_rect.right() - 540.0)
                .max(editor_rect.left() + 8.0);
            egui::Area::new(Id::new("inline_bar"))
                .order(Order::Foreground)
                .fixed_pos(egui::pos2(x, y - 4.0))
                .pivot(Align2::LEFT_BOTTOM)
                .show(ctx, |ui| self.inline_bar(ui, ctx, 520.0));
        }
    }

    /// The Ctrl+K prompt bar (floating over the code, or above its diff).
    pub fn inline_bar(&mut self, ui: &mut egui::Ui, ctx: &egui::Context, width: f32) {
        let has_result = self.pending.iter().any(|p| p.origin == Origin::Inline);
        let mut submit = false;
        let mut stop = None;
        let mut close = false;
        let mut accept = false;
        let mut reject = false;
        let model = self.settings.model.clone();
        let ready = self.settings.ready();
        let Some(inline) = &mut self.inline else {
            return;
        };
        Frame::new()
            .fill(WIDGET)
            .stroke(Stroke::new(1.0_f32, Color32::from_rgb(0x3a, 0x3a, 0x3a)))
            .corner_radius(CornerRadius::same(8))
            .inner_margin(Margin::same(8))
            .shadow(egui::epaint::Shadow {
                offset: [0, 4],
                blur: 16,
                spread: 0,
                color: Color32::from_black_alpha(120),
            })
            .show(ui, |ui| {
                ui.set_width(width);
                ui.horizontal(|ui| {
                    let hint = if has_result {
                        "Add a follow-up…"
                    } else if inline.generate {
                        "Generate code… (any language, e.g. ภาษาไทย)"
                    } else {
                        "Edit selected code… (any language, e.g. ภาษาไทย)"
                    };
                    let resp = ui.add(
                        TextEdit::singleline(&mut inline.prompt)
                            .id(Id::new("inline_prompt"))
                            .hint_text(hint)
                            .frame(false)
                            .desired_width(width - 60.0),
                    );
                    if inline.focus {
                        resp.request_focus();
                        inline.focus = false;
                    }
                    if resp.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter)) {
                        submit = true;
                    }
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if ui
                            .add(egui::Button::new(RichText::new("×").color(MUTED)).frame(false))
                            .on_hover_text("Close (Esc)")
                            .clicked()
                        {
                            close = true;
                        }
                        if let Some(job) = inline.job {
                            if ui.small_button("■").on_hover_text("Stop").clicked() {
                                stop = Some(job);
                            }
                        }
                    });
                });
                if !inline.history.is_empty() {
                    ui.label(
                        RichText::new(inline.history.join("  →  "))
                            .size(11.0)
                            .color(FAINT),
                    );
                }
                if inline.job.is_some() {
                    ui.horizontal(|ui| {
                        ui.add(egui::Spinner::new().size(11.0));
                        ui.label(RichText::new("Generating…").size(11.5).color(MUTED));
                    });
                    if !inline.output.is_empty() {
                        let lines: Vec<&str> = inline.output.lines().collect();
                        let tail = lines[lines.len().saturating_sub(6)..].join("\n");
                        ui.label(RichText::new(tail).monospace().size(11.0).color(ADDED));
                    }
                }
                if let Some(e) = &inline.error {
                    ui.label(RichText::new(e).size(11.5).color(REMOVED));
                }
                ui.horizontal(|ui| {
                    if has_result {
                        if ui
                            .add(
                                egui::Button::new(
                                    RichText::new("Accept  Ctrl+Enter")
                                        .size(11.5)
                                        .color(ACCENT_TEXT),
                                )
                                .fill(ADDED),
                            )
                            .clicked()
                        {
                            accept = true;
                        }
                        if ui
                            .add(egui::Button::new(
                                RichText::new("Reject  Ctrl+Backspace").size(11.5),
                            ))
                            .clicked()
                        {
                            reject = true;
                        }
                    }
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        let text = if ready {
                            format!("{model} · Esc to close · Ctrl+L to chat")
                        } else {
                            "Set up a model in Settings (Ctrl+,)".into()
                        };
                        ui.label(RichText::new(text).size(10.5).color(FAINT));
                    });
                });
            });
        if submit {
            self.submit_inline(ctx);
        }
        if let Some(job) = stop {
            self.stop_job(job);
        }
        if accept {
            if let Some(i) = self.pending.iter().position(|p| p.origin == Origin::Inline) {
                self.accept(i);
            }
        }
        if reject {
            if let Some(i) = self.pending.iter().position(|p| p.origin == Origin::Inline) {
                self.reject(i);
            }
        }
        if close {
            self.close_inline();
        }
    }

    fn diff_view(&mut self, ui: &mut egui::Ui, ctx: &egui::Context, p: usize, acts: &mut Vec<Act>) {
        let pending = &self.pending[p];
        let origin = pending.origin;
        let rel = pending.rel.clone();
        let deleting = pending.proposed.is_none();
        let is_new = pending.is_new;
        let rows = diff::lines(&pending.original, pending.proposed.as_deref().unwrap_or(""));
        let added = rows.iter().filter(|r| r.kind == Kind::Added).count();
        let removed = rows.iter().filter(|r| r.kind == Kind::Removed).count();
        let agent_left = self
            .pending
            .iter()
            .filter(|q| matches!(q.origin, Origin::Agent { .. }))
            .count();
        Frame::new()
            .fill(WIDGET)
            .inner_margin(Margin::symmetric(12, 6))
            .stroke(Stroke::new(1.0_f32, BORDER))
            .show(ui, |ui| {
                ui.set_width(ui.available_width());
                if origin == Origin::Inline {
                    let w = (ui.available_width() - 20.0).min(720.0);
                    self.inline_bar(ui, ctx, w);
                } else {
                    ui.horizontal(|ui| {
                        let what = if deleting {
                            "Delete file"
                        } else if is_new {
                            "New file"
                        } else {
                            "Agent edit"
                        };
                        ui.label(RichText::new(format!("∞ {what}")).color(BRIGHT).strong());
                        ui.label(RichText::new(format!("+{added}")).color(ADDED));
                        ui.label(RichText::new(format!("−{removed}")).color(REMOVED));
                        if agent_left > 1 {
                            ui.label(
                                RichText::new(format!("· {agent_left} files to review"))
                                    .color(MUTED)
                                    .size(11.5),
                            );
                        }
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            if agent_left > 1 {
                                if ui.button("Reject all").clicked() {
                                    acts.push(Act::RejectAll);
                                }
                                if ui
                                    .add(
                                        egui::Button::new(
                                            RichText::new("Accept all").color(ACCENT_TEXT),
                                        )
                                        .fill(ADDED),
                                    )
                                    .clicked()
                                {
                                    acts.push(Act::AcceptAll);
                                }
                            }
                            if ui.button("Reject  Ctrl+Backspace").clicked() {
                                acts.push(Act::Reject(p));
                            }
                            if ui
                                .add(
                                    egui::Button::new(
                                        RichText::new("Accept  Ctrl+Enter").color(ACCENT_TEXT),
                                    )
                                    .fill(ADDED),
                                )
                                .clicked()
                            {
                                acts.push(Act::Accept(p));
                            }
                        });
                    });
                }
            });
        let font = FontId::monospace(self.settings.font_size);
        let row_h = ui.fonts(|f| f.row_height(&font)) + 1.0;
        let first_change = rows.iter().position(|r| r.kind != Kind::Same).unwrap_or(0);
        let mut area = ScrollArea::both()
            .id_salt(("diff", &rel))
            .auto_shrink(false);
        if let Some(pending) = self.pending.get_mut(p) {
            if !pending.scrolled {
                area = area.vertical_scroll_offset(first_change.saturating_sub(6) as f32 * row_h);
                pending.scrolled = true;
            }
        }
        area.show_rows(ui, row_h, rows.len(), |ui, range| {
            for row in &rows[range] {
                let (rect, _) = ui.allocate_exact_size(
                    egui::vec2(ui.available_width().max(900.0), row_h),
                    Sense::hover(),
                );
                let (bg, sign, color) = match row.kind {
                    Kind::Added => (ADDED_BG, "+", ADDED),
                    Kind::Removed => (REMOVED_BG, "−", REMOVED),
                    Kind::Same => (Color32::TRANSPARENT, " ", LINE_NO),
                };
                ui.painter().rect_filled(rect, CornerRadius::ZERO, bg);
                let no = row
                    .new_no
                    .or(row.old_no)
                    .map(|n| n.to_string())
                    .unwrap_or_default();
                ui.painter().text(
                    egui::pos2(rect.left() + 46.0, rect.top()),
                    Align2::RIGHT_TOP,
                    no,
                    font.clone(),
                    LINE_NO,
                );
                ui.painter().text(
                    egui::pos2(rect.left() + 56.0, rect.top()),
                    Align2::LEFT_TOP,
                    sign,
                    font.clone(),
                    color,
                );
                let text_color = if row.kind == Kind::Same { TEXT } else { BRIGHT };
                ui.painter().text(
                    egui::pos2(rect.left() + 72.0, rect.top()),
                    Align2::LEFT_TOP,
                    row.text.replace('\t', "    "),
                    font.clone(),
                    text_color,
                );
            }
        });
    }

    // ------------------------------------------------------------------ actions

    pub fn apply(&mut self, acts: Vec<Act>, ctx: &egui::Context) {
        for act in acts {
            match act {
                Act::Open(rel) => {
                    self.agents_view = false;
                    self.open_file(&rel)
                }
                Act::OpenAt(rel, line) => {
                    self.agents_view = false;
                    self.open_at(&rel, line)
                }
                Act::ToggleDir(rel) => {
                    if !self.expanded.remove(&rel) {
                        self.expanded.insert(rel);
                    }
                }
                Act::Accept(i) => self.accept(i),
                Act::Reject(i) => self.reject(i),
                Act::AcceptAll => self.accept_all_agent(),
                Act::RejectAll => self.reject_all_agent(),
                Act::Copy(text) => {
                    ctx.copy_text(text);
                    self.notice = "Copied.".into();
                }
                Act::Insert(text) => self.insert_at_cursor(&text),
                Act::Settings => {
                    self.settings_open = true;
                    self.settings_tab = 0;
                }
            }
        }
    }

    fn screenshot_step(&mut self, ctx: &egui::Context) {
        let Some(path) = self.screenshot.clone() else {
            return;
        };
        self.frame += 1;
        ctx.request_repaint();
        if self.frame >= 120 && !self.shot_requested {
            self.shot_requested = true;
            ctx.send_viewport_cmd(ViewportCommand::Screenshot(Default::default()));
        }
        let image = ctx.input(|i| {
            i.events.iter().find_map(|e| match e {
                Event::Screenshot { image, .. } => Some(image.clone()),
                _ => None,
            })
        });
        if let Some(image) = image {
            let [w, h] = image.size;
            let mut bytes = Vec::with_capacity(w * h * 4);
            for p in &image.pixels {
                bytes.extend_from_slice(&p.to_srgba_unmultiplied());
            }
            if let Some(dir) = path.parent() {
                let _ = std::fs::create_dir_all(dir);
            }
            match image::RgbaImage::from_raw(w as u32, h as u32, bytes) {
                Some(img) => {
                    if let Err(e) = img.save(&path) {
                        eprintln!("could not save screenshot: {e}");
                    }
                }
                None => eprintln!("screenshot had an unexpected size"),
            }
            ctx.send_viewport_cmd(ViewportCommand::Close);
        } else if self.frame > 1200 {
            eprintln!("no screenshot arrived");
            ctx.send_viewport_cmd(ViewportCommand::Close);
        }
    }
}
