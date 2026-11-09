//! Editor state and the logic behind every feature (drawing lives in view.rs).

use std::{
    collections::{BTreeMap, HashMap, HashSet},
    fs,
    path::{Component, Path, PathBuf},
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc::{channel, Receiver, Sender},
        Arc, Mutex, RwLock,
    },
};

use eframe::egui;

use crate::ai::{self, AiEvent, Request, Role, Turn};
use crate::diff::{self, FileChange};
use crate::index::{self, Hit, Index};
use crate::settings::Settings;

/// Estimated: how much file text one chat request may carry.
pub const CONTEXT_BUDGET: usize = 120_000;
/// Estimated: largest single attached file.
pub const FILE_BUDGET: usize = 40_000;
/// Estimated: codebase hits attached to a question.
pub const CODEBASE_HITS: usize = 8;

#[allow(dead_code)]
pub struct Tab {
    pub path: PathBuf,
    pub rel: String,
    pub text: String,
    pub saved: String,
    /// Selection as character indices (start, end).
    pub sel: (usize, usize),
    pub jump_to_line: Option<usize>,
    pub exists: bool,
}

impl Tab {
    pub fn dirty(&self) -> bool {
        self.text != self.saved
    }
    pub fn name(&self) -> &str {
        self.rel.rsplit('/').next().unwrap_or(&self.rel)
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Origin {
    Inline,
    Agent {
        chat: usize,
        msg: usize,
        change: usize,
    },
}

/// An AI edit waiting for the person to accept or reject it.
pub struct Pending {
    pub rel: String,
    pub original: String,
    /// None: delete the file.
    pub proposed: Option<String>,
    pub origin: Origin,
    pub is_new: bool,
    pub scrolled: bool,
}

/// The Ctrl+K prompt bar.
pub struct Inline {
    pub rel: String,
    /// Byte range of the (whole-line) region being rewritten.
    pub start: usize,
    pub end: usize,
    pub generate: bool,
    pub line: usize,
    pub prompt: String,
    pub history: Vec<String>,
    pub job: Option<u64>,
    pub output: String,
    pub error: Option<String>,
    pub focus: bool,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Mode {
    Agent,
    Ask,
    Plan,
}

impl Mode {
    pub const ALL: [Mode; 3] = [Mode::Agent, Mode::Ask, Mode::Plan];
    pub fn name(self) -> &'static str {
        match self {
            Mode::Agent => "Agent",
            Mode::Ask => "Ask",
            Mode::Plan => "Plan",
        }
    }
    pub fn icon(self) -> &'static str {
        match self {
            Mode::Agent => "∞",
            Mode::Ask => "💬",
            Mode::Plan => "☰",
        }
    }
    pub fn description(self) -> &'static str {
        match self {
            Mode::Agent => "Plan, search, make edits across files",
            Mode::Ask => "Ask questions about your codebase",
            Mode::Plan => "Create detailed plans for accomplishing tasks",
        }
    }
    pub fn shortcut(self) -> &'static str {
        match self {
            Mode::Agent => "Ctrl+I",
            Mode::Ask => "Ctrl+L",
            Mode::Plan => "",
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ChangeState {
    Pending,
    Accepted,
    Rejected,
}

pub struct Msg {
    pub role: Role,
    pub text: String,
    pub mode: Mode,
    pub context: Vec<String>,
    pub sources: Vec<String>,
    pub keywords: String,
    pub status: String,
    pub streaming: bool,
    pub error: Option<String>,
    pub changes: Vec<FileChange>,
    pub change_state: Vec<ChangeState>,
}

impl Msg {
    fn user(text: String, mode: Mode, context: Vec<String>) -> Self {
        Self {
            role: Role::User,
            text,
            mode,
            context,
            sources: vec![],
            keywords: String::new(),
            status: String::new(),
            streaming: false,
            error: None,
            changes: vec![],
            change_state: vec![],
        }
    }
}

pub struct Selection {
    pub rel: String,
    pub start_line: usize,
    pub end_line: usize,
    pub text: String,
}

pub struct Chat {
    pub title: String,
    pub messages: Vec<Msg>,
    pub mode: Mode,
    pub selections: Vec<Selection>,
    pub updated: u64,
}

impl Chat {
    pub fn new(mode: Mode) -> Self {
        Self {
            title: "New Chat".into(),
            messages: vec![],
            mode,
            selections: vec![],
            updated: crate::history::now(),
        }
    }
}

pub enum Job {
    Inline,
    Chat { chat: usize, msg: usize },
    SearchKeywords,
    Test,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum SideView {
    Explorer,
    Search,
}

#[derive(Default)]
pub struct IndexState {
    pub running: bool,
    pub done: usize,
    pub total: usize,
}

pub struct App {
    pub settings: Settings,
    pub settings_open: bool,
    pub settings_tab: usize,
    pub persist: bool,
    pub root: Option<PathBuf>,
    pub files: Vec<String>,
    pub dirs: Vec<String>,
    pub tree: BTreeMap<String, Vec<(String, String, bool)>>,
    pub expanded: HashSet<String>,
    pub tabs: Vec<Tab>,
    pub active: Option<usize>,
    pub pending: Vec<Pending>,
    pub inline: Option<Inline>,
    pub chats: Vec<Chat>,
    pub chat: usize,
    pub chat_open: bool,
    pub input: String,
    pub focus_input: bool,
    pub mention_sel: usize,
    pub sidebar_open: bool,
    pub side: SideView,
    pub search_query: String,
    pub search_results: Vec<Hit>,
    pub search_keywords: String,
    pub search_busy: bool,
    pub search_done: bool,
    pub index: Arc<RwLock<Index>>,
    pub index_state: Arc<Mutex<IndexState>>,
    pub quick_open: Option<(String, usize)>,
    pub jobs: HashMap<u64, (Job, Arc<AtomicBool>)>,
    pub next_job: u64,
    pub tx: Sender<(u64, AiEvent)>,
    pub rx: Receiver<(u64, AiEvent)>,
    pub notice: String,
    pub test_result: String,
    pub screenshot: Option<PathBuf>,
    pub frame: u64,
    pub shot_requested: bool,
    /// The Agents home (the original's agents window) instead of the editor.
    pub agents_view: bool,
    pub agents_filter: Option<String>,
    /// Chat titles and times of every recent project, for the Agents list.
    pub history_cache: Vec<(PathBuf, Vec<(String, u64)>)>,
}

pub struct Startup {
    pub open: Option<PathBuf>,
    pub screenshot: Option<PathBuf>,
}

impl App {
    pub fn new(cc: &eframe::CreationContext<'_>, startup: Startup) -> Self {
        crate::fonts::install(&cc.egui_ctx);
        crate::theme::apply(&cc.egui_ctx);
        let (tx, rx) = channel();
        let settings = Settings::load();
        let mut app = Self {
            persist: startup.screenshot.is_none(),
            settings,
            settings_open: false,
            settings_tab: 0,
            root: None,
            files: vec![],
            dirs: vec![],
            tree: BTreeMap::new(),
            expanded: HashSet::new(),
            tabs: vec![],
            active: None,
            pending: vec![],
            inline: None,
            chats: vec![Chat::new(Mode::Agent)],
            chat: 0,
            chat_open: true,
            input: String::new(),
            focus_input: false,
            mention_sel: 0,
            sidebar_open: true,
            side: SideView::Explorer,
            search_query: String::new(),
            search_results: vec![],
            search_keywords: String::new(),
            search_busy: false,
            search_done: false,
            index: Arc::new(RwLock::new(Index::default())),
            index_state: Arc::new(Mutex::new(IndexState::default())),
            quick_open: None,
            jobs: HashMap::new(),
            next_job: 1,
            tx,
            rx,
            notice: "Ready".into(),
            test_result: String::new(),
            screenshot: startup.screenshot.clone(),
            frame: 0,
            shot_requested: false,
            agents_view: true,
            agents_filter: None,
            history_cache: vec![],
        };
        let folder = startup.open.or_else(|| {
            if startup.screenshot.is_some() {
                std::env::current_dir().ok()
            } else {
                app.settings.last_workspace.clone().filter(|p| p.is_dir())
            }
        });
        if let Some(folder) = folder {
            app.open_workspace(folder, &cc.egui_ctx);
            for candidate in [
                "src/main.rs",
                "src/lib.rs",
                "main.py",
                "src/index.ts",
                "src/main.ts",
                "index.js",
                "README.md",
            ] {
                if app.files.iter().any(|f| f == candidate) {
                    app.open_file(candidate);
                    break;
                }
            }
        }
        app.agents_view = app.settings.start_in_agents;
        app.refresh_history_cache();
        if let Ok(demo) = std::env::var("OPENCODE_DEMO") {
            app.demo(&demo);
        }
        app
    }

    /// Preset states for checking the screens without an AI service (developer aid).
    fn demo(&mut self, what: &str) {
        let Some(i) = self.active else { return };
        let text = self.tabs[i].text.clone();
        let line_start = |n: usize| -> usize {
            text.split('\n')
                .take(n)
                .map(|l| l.chars().count() + 1)
                .sum()
        };
        self.tabs[i].sel = (line_start(6), line_start(16));
        match what {
            "inline" => {
                self.agents_view = false;
                self.start_inline();
                if let Some(b) = &mut self.inline {
                    b.prompt = "เรียงลำดับ mod ตามตัวอักษร และเพิ่มคอมเมนต์อธิบาย".into();
                }
            }
            "diff" => {
                self.agents_view = false;
                self.start_inline();
                if let Some(b) = &mut self.inline {
                    b.history.push("add a comment above each module".into());
                    b.output = "// AI model client\nmod ai;\n// editor state\nmod app;".into();
                }
                self.inline_event_demo();
            }
            "agent" | "mention" => {
                let rel = self.tabs[i].rel.clone();
                self.input = if what == "mention" {
                    "Explain @src/".into()
                } else {
                    String::new()
                };
                self.focus_input = what == "mention";
                if what == "agent" {
                    self.chats[0].title = "Add a help flag".into();
                    let mut user = Msg::user(
                        "Add a --version flag and document it in the README".into(),
                        Mode::Agent,
                        vec![format!("{rel} (current file)"), "codebase".into()],
                    );
                    user.mode = Mode::Agent;
                    self.chats[0].messages.push(user);
                    let new_main = text.replacen("\"--help\" | \"-h\" => {", "\"--version\" => {\n                println!(\"opencode {}\", env!(\"CARGO_PKG_VERSION\"));\n                return Ok(());\n            }\n            \"--help\" | \"-h\" => {", 1);
                    let answer = format!(
                        "I'll add a `--version` flag to the argument parser and mention it in the README.\n\n<<<FILE {rel}\n{new_main}\n>>>END\n<<<FILE README.md\n# OpenCode\n\nRun `opencode --version` to see the version.\n>>>END\n\nBoth files are ready for review."
                    );
                    let mut reply = Msg::user(answer, Mode::Agent, vec![]);
                    reply.role = Role::Assistant;
                    reply.sources = vec![format!("{rel}:21-60"), "README.md:1-28".into()];
                    reply.keywords = "args flag version cli parse".into();
                    self.chats[0].messages.push(reply);
                    let (id, _) = self.new_job(Job::Chat { chat: 0, msg: 1 });
                    let _ = self.tx.send((id, AiEvent::Done));
                    self.poll();
                }
            }
            _ => {}
        }
    }

    fn inline_event_demo(&mut self) {
        let (id, _) = self.new_job(Job::Inline);
        if let Some(b) = &mut self.inline {
            b.job = Some(id);
        }
        let _ = self.tx.send((id, AiEvent::Done));
        self.poll();
    }

    // ---------------------------------------------------------------- workspace

    pub fn pick_workspace(&mut self, ctx: &egui::Context) {
        if let Some(folder) = rfd::FileDialog::new().pick_folder() {
            self.open_workspace(folder, ctx);
        }
    }

    pub fn open_workspace(&mut self, folder: PathBuf, ctx: &egui::Context) {
        let folder = match folder.canonicalize() {
            Ok(f) => strip_unc(f),
            Err(e) => {
                self.notice = format!("Could not open folder: {e}");
                return;
            }
        };
        self.save_history();
        self.root = Some(folder.clone());
        self.load_history();
        self.settings.recent_workspaces.retain(|p| p != &folder);
        self.settings.recent_workspaces.insert(0, folder.clone());
        self.settings.recent_workspaces.truncate(12);
        self.refresh_history_cache();
        self.tabs.clear();
        self.active = None;
        self.pending.clear();
        self.inline = None;
        self.expanded.clear();
        self.refresh_files();
        if self.persist {
            self.settings.last_workspace = Some(folder.clone());
            let _ = self.settings.save();
        }
        *self.index.write().unwrap() = Index::default();
        self.notice = format!("Opened {}", folder.display());
        if self.settings.index_on_open {
            self.start_indexing(ctx);
        }
    }

    /// Keep this project's finished chats (only when the person runs the app, not in checks).
    pub fn save_history(&self) {
        let (Some(root), true) = (&self.root, self.persist) else {
            return;
        };
        let saved: Vec<crate::history::SavedChat> = self
            .chats
            .iter()
            .filter(|c| !c.messages.is_empty())
            .map(|c| crate::history::SavedChat {
                title: c.title.clone(),
                updated: c.updated,
                messages: c
                    .messages
                    .iter()
                    .filter(|m| !m.streaming && !(m.role == Role::Assistant && m.text.is_empty()))
                    .map(|m| crate::history::SavedMsg {
                        user: m.role == Role::User,
                        text: m.text.clone(),
                        mode: m.mode.name().into(),
                    })
                    .collect(),
            })
            .collect();
        crate::history::save(root, &saved);
    }

    fn load_history(&mut self) {
        let Some(root) = self.root.clone() else {
            return;
        };
        let mut chats: Vec<Chat> = crate::history::load(&root)
            .into_iter()
            .map(|s| {
                let mut chat = Chat::new(Mode::Agent);
                chat.title = s.title;
                chat.updated = s.updated;
                for m in s.messages {
                    let mode = Mode::ALL
                        .into_iter()
                        .find(|x| x.name() == m.mode)
                        .unwrap_or(Mode::Agent);
                    let mut msg = Msg::user(m.text, mode, vec![]);
                    if !m.user {
                        msg.role = Role::Assistant;
                        if mode == Mode::Agent {
                            let (_, changes) = diff::parse_agent_answer(&msg.text);
                            // past suggestions: already decided in an earlier session
                            msg.change_state = vec![ChangeState::Rejected; changes.len()];
                            msg.changes = changes;
                        }
                    }
                    chat.mode = mode;
                    chat.messages.push(msg);
                }
                chat
            })
            .collect();
        chats.sort_by_key(|c| c.updated);
        chats.push(Chat::new(Mode::Agent));
        self.chat = chats.len() - 1;
        self.chats = chats;
    }

    pub fn refresh_history_cache(&mut self) {
        let mut cache = Vec::new();
        for root in self
            .settings
            .recent_workspaces
            .iter()
            .filter(|p| p.is_dir())
        {
            let mut list: Vec<(String, u64)> = crate::history::load(root)
                .into_iter()
                .map(|c| (c.title, c.updated))
                .collect();
            list.sort_by(|a, b| b.1.cmp(&a.1));
            cache.push((root.clone(), list));
        }
        if let Some(root) = &self.root {
            if !cache.iter().any(|(p, _)| p == root) {
                cache.insert(0, (root.clone(), vec![]));
            }
        }
        self.history_cache = cache;
    }

    pub fn refresh_files(&mut self) {
        let Some(root) = &self.root else { return };
        let (files, dirs) = index::list_paths(root);
        let mut tree: BTreeMap<String, Vec<(String, String, bool)>> = BTreeMap::new();
        for d in &dirs {
            let (parent, name) = split_parent(d);
            tree.entry(parent)
                .or_default()
                .push((name, d.clone(), true));
        }
        for f in &files {
            let (parent, name) = split_parent(f);
            tree.entry(parent)
                .or_default()
                .push((name, f.clone(), false));
        }
        for children in tree.values_mut() {
            children.sort_by(|a, b| {
                b.2.cmp(&a.2)
                    .then_with(|| a.0.to_lowercase().cmp(&b.0.to_lowercase()))
            });
        }
        self.files = files;
        self.dirs = dirs;
        self.tree = tree;
    }

    pub fn workspace_name(&self) -> String {
        self.root
            .as_ref()
            .and_then(|p| p.file_name())
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| "No folder".into())
    }

    /// A path inside the workspace, refusing anything that would leave it.
    pub fn safe_path(&self, rel: &str) -> Result<PathBuf, String> {
        let root = self.root.as_ref().ok_or("Open a folder first.")?;
        let rel_path = Path::new(rel);
        if rel_path.is_absolute()
            || rel_path.components().any(|c| {
                matches!(
                    c,
                    Component::ParentDir | Component::Prefix(_) | Component::RootDir
                )
            })
        {
            return Err(format!("Blocked: {rel} is outside the workspace."));
        }
        let path = root.join(rel_path);
        if path.exists() {
            let canonical = strip_unc(path.canonicalize().map_err(|e| e.to_string())?);
            if !canonical.starts_with(root) {
                return Err(format!("Blocked: {rel} is outside the workspace."));
            }
        }
        Ok(path)
    }

    pub fn open_file(&mut self, rel: &str) {
        if let Some(i) = self.tabs.iter().position(|t| t.rel == rel) {
            self.active = Some(i);
            return;
        }
        let path = match self.safe_path(rel) {
            Ok(p) => p,
            Err(e) => {
                self.notice = e;
                return;
            }
        };
        let (text, exists) = match fs::read(&path) {
            Ok(bytes) => {
                if bytes.iter().take(8000).any(|b| *b == 0) {
                    self.notice =
                        format!("{rel} looks like a binary file; only text files open here.");
                    return;
                }
                (String::from_utf8_lossy(&bytes).replace("\r\n", "\n"), true)
            }
            Err(_) => (String::new(), false),
        };
        self.tabs.push(Tab {
            path,
            rel: rel.to_string(),
            saved: text.clone(),
            text,
            sel: (0, 0),
            jump_to_line: None,
            exists,
        });
        self.active = Some(self.tabs.len() - 1);
    }

    pub fn open_at(&mut self, rel: &str, line: usize) {
        self.open_file(rel);
        if let Some(i) = self.active {
            if self.tabs[i].rel == rel {
                self.tabs[i].jump_to_line = Some(line);
            }
        }
    }

    pub fn close_tab(&mut self, i: usize) {
        if i >= self.tabs.len() {
            return;
        }
        self.tabs.remove(i);
        self.active = match self.active {
            Some(a) if a == i => (!self.tabs.is_empty()).then(|| i.min(self.tabs.len() - 1)),
            Some(a) if a > i => Some(a - 1),
            other => other,
        };
    }

    pub fn save_active(&mut self) {
        let Some(i) = self.active else { return };
        let rel = self.tabs[i].rel.clone();
        match self.write_file(&rel, &self.tabs[i].text.clone()) {
            Ok(()) => {
                let tab = &mut self.tabs[i];
                tab.saved = tab.text.clone();
                tab.exists = true;
                self.notice = format!("Saved {rel}");
            }
            Err(e) => self.notice = e,
        }
    }

    fn write_file(&mut self, rel: &str, text: &str) -> Result<(), String> {
        let path = self.safe_path(rel)?;
        let is_new = !path.exists();
        if let Some(dir) = path.parent() {
            fs::create_dir_all(dir).map_err(|e| format!("Could not create folder: {e}"))?;
        }
        fs::write(&path, text).map_err(|e| format!("Could not save {rel}: {e}"))?;
        if let Ok(mut idx) = self.index.write() {
            if idx.root.is_some() {
                idx.update_file(rel, text);
            }
        }
        if is_new {
            self.refresh_files();
        }
        Ok(())
    }

    // ---------------------------------------------------------------- indexing

    pub fn start_indexing(&mut self, ctx: &egui::Context) {
        let Some(root) = self.root.clone() else {
            return;
        };
        {
            let mut state = self.index_state.lock().unwrap();
            if state.running {
                return;
            }
            state.running = true;
            state.done = 0;
            state.total = 0;
        }
        let index = self.index.clone();
        let state = self.index_state.clone();
        let ctx = ctx.clone();
        std::thread::spawn(move || {
            let previous = index.read().map(|i| i.clone()).ok();
            let progress = |done: usize, total: usize| {
                if let Ok(mut s) = state.lock() {
                    s.done = done;
                    s.total = total;
                }
                ctx.request_repaint();
            };
            let built = index::build(&root, previous.as_ref(), &progress);
            if let Ok(mut slot) = index.write() {
                *slot = built;
            }
            if let Ok(mut s) = state.lock() {
                s.running = false;
            }
            ctx.request_repaint();
        });
    }

    pub fn delete_index(&mut self) {
        *self.index.write().unwrap() = Index::default();
        self.notice = "Index deleted. Sync to build it again.".into();
    }

    pub fn index_summary(&self) -> String {
        let state = self.index_state.lock().unwrap();
        if state.running {
            return if state.total > 0 {
                format!("Indexing {}/{} files…", state.done, state.total)
            } else {
                "Indexing…".into()
            };
        }
        drop(state);
        let idx = self.index.read().unwrap();
        if idx.root.is_none() {
            "Not indexed".into()
        } else if let Some(e) = &idx.error {
            e.clone()
        } else {
            format!(
                "Indexed {} files · {} chunks",
                idx.files.len(),
                idx.chunk_count
            )
        }
    }

    // ---------------------------------------------------------------- jobs

    fn new_job(&mut self, job: Job) -> (u64, Arc<AtomicBool>) {
        let id = self.next_job;
        self.next_job += 1;
        let cancel = Arc::new(AtomicBool::new(false));
        self.jobs.insert(id, (job, cancel.clone()));
        (id, cancel)
    }

    pub fn stop_job(&mut self, id: u64) {
        if let Some((_, cancel)) = self.jobs.get(&id) {
            cancel.store(true, Ordering::Relaxed);
        }
    }

    pub fn running_chat_job(&self) -> Option<u64> {
        self.jobs.iter().find_map(|(id, (job, _))| match job {
            Job::Chat { chat, .. } if *chat == self.chat => Some(*id),
            _ => None,
        })
    }

    pub fn poll(&mut self) {
        while let Ok((id, event)) = self.rx.try_recv() {
            let finished = matches!(event, AiEvent::Done | AiEvent::Error(_));
            let Some((job, _)) = self.jobs.get(&id) else {
                continue;
            };
            match job {
                Job::Inline => self.inline_event(id, event),
                Job::Chat { chat, msg } => {
                    let (chat, msg) = (*chat, *msg);
                    self.chat_event(chat, msg, event);
                }
                Job::SearchKeywords => match event {
                    AiEvent::Delta(d) => self.search_keywords.push_str(&d),
                    AiEvent::Done | AiEvent::Error(_) => self.run_search(),
                    _ => {}
                },
                Job::Test => match event {
                    AiEvent::Delta(d) => self.test_result.push_str(&d),
                    AiEvent::Done => {
                        self.notice = "The AI model answered. You're connected.".into();
                        if self.test_result.trim().is_empty() {
                            self.test_result = "(empty answer)".into();
                        }
                    }
                    AiEvent::Error(e) => self.test_result = e,
                    _ => {}
                },
            }
            if finished {
                self.jobs.remove(&id);
            }
        }
    }

    // ---------------------------------------------------------------- Ctrl+K

    pub fn start_inline(&mut self) {
        let Some(i) = self.active else {
            self.notice = "Open a file first, then press Ctrl+K.".into();
            return;
        };
        if let Some(inline) = &mut self.inline {
            inline.focus = true;
            return;
        }
        let tab = &self.tabs[i];
        if self.pending.iter().any(|p| p.rel == tab.rel) {
            self.notice =
                "Accept (Ctrl+Enter) or reject (Ctrl+Backspace) the current change first.".into();
            return;
        }
        let text = &tab.text;
        let (a, b) = tab.sel;
        let sa = char_to_byte(text, a);
        let sb = char_to_byte(text, b);
        let (start, end, generate) = if sa == sb {
            // generate: replace a blank line, or insert after the cursor's line
            let ls = text[..sa].rfind('\n').map(|p| p + 1).unwrap_or(0);
            let le = text[sa..].find('\n').map(|p| sa + p).unwrap_or(text.len());
            if text[ls..le].trim().is_empty() {
                (ls, le, true)
            } else {
                (le, le, true)
            }
        } else {
            // whole lines, like the original's prompt bar
            let ls = text[..sa].rfind('\n').map(|p| p + 1).unwrap_or(0);
            let mut eb = sb;
            if eb > sa && text[..eb].ends_with('\n') {
                eb -= 1;
            }
            let le = text[eb..].find('\n').map(|p| eb + p).unwrap_or(text.len());
            (ls, le, false)
        };
        let line = text[..start].matches('\n').count();
        self.inline = Some(Inline {
            rel: tab.rel.clone(),
            start,
            end,
            generate,
            line,
            prompt: String::new(),
            history: vec![],
            job: None,
            output: String::new(),
            error: None,
            focus: true,
        });
    }

    pub fn close_inline(&mut self) {
        if let Some(inline) = self.inline.take() {
            if let Some(job) = inline.job {
                self.stop_job(job);
            }
            self.pending
                .retain(|p| !(p.origin == Origin::Inline && p.rel == inline.rel));
        }
    }

    pub fn submit_inline(&mut self, ctx: &egui::Context) {
        let Some(inline) = &self.inline else { return };
        let prompt = inline.prompt.trim().to_string();
        if prompt.is_empty() || inline.job.is_some() {
            return;
        }
        let Some(tab) = self.tabs.iter().find(|t| t.rel == inline.rel) else {
            return;
        };
        // a follow-up refines the pending result
        let base = self
            .pending
            .iter()
            .find(|p| p.origin == Origin::Inline && p.rel == inline.rel)
            .map(|p| p.original.clone())
            .unwrap_or_else(|| tab.text.clone());
        let previous = (!inline.output.is_empty()).then(|| inline.output.clone());
        let selected = &base[inline.start..inline.end];
        let before: Vec<&str> = base[..inline.start].lines().collect();
        let after: Vec<&str> = base[inline.end..].lines().collect();
        let before = before[before.len().saturating_sub(200)..].join("\n");
        let after = after
            .iter()
            .take(200)
            .cloned()
            .collect::<Vec<_>>()
            .join("\n");
        let marked = if inline.generate {
            format!("{before}\n<<<CURSOR>>>\n{after}")
        } else {
            format!("{before}\n<<<SELECTION>>>\n{selected}\n<<<END SELECTION>>>\n{after}")
        };
        let mut content = format!(
            "File: {} ({})\n\n```\n{}\n```\n\n",
            inline.rel,
            crate::highlight::language_name(&inline.rel),
            marked
        );
        for (n, old) in inline.history.iter().enumerate() {
            content.push_str(&format!("Earlier instruction {}: {}\n", n + 1, old));
        }
        if let Some(prev) = previous {
            content.push_str(&format!(
                "\nYour previous answer was:\n```\n{prev}\n```\nNow change it as follows.\n"
            ));
        }
        content.push_str(&format!("\nInstruction: {prompt}\n\n"));
        content.push_str(if inline.generate {
            "Reply with ONLY the new code to insert at <<<CURSOR>>>."
        } else {
            "Reply with ONLY the code that replaces everything between <<<SELECTION>>> and <<<END SELECTION>>>."
        });
        let system = "You are an expert programmer editing code inside a code editor. You return code only: \
                      no explanations, no markdown fences, no markers. Keep the file's indentation and style. \
                      Comments you write follow the language of the instruction (Thai instructions may get Thai comments).";
        let (id, cancel) = self.new_job(Job::Inline);
        let inline = self.inline.as_mut().unwrap();
        inline.job = Some(id);
        inline.output.clear();
        inline.error = None;
        inline.history.push(prompt);
        inline.prompt.clear();
        ai::spawn(
            Request {
                job: id,
                settings: self.settings.clone(),
                system: system.into(),
                turns: vec![Turn {
                    role: Role::User,
                    content,
                }],
                codebase: None,
                cancel,
            },
            self.tx.clone(),
            ctx.clone(),
        );
    }

    fn inline_event(&mut self, id: u64, event: AiEvent) {
        let Some(inline) = &mut self.inline else {
            return;
        };
        if inline.job != Some(id) {
            return;
        }
        match event {
            AiEvent::Delta(d) => inline.output.push_str(&d),
            AiEvent::Error(e) => {
                inline.error = Some(e);
                inline.job = None;
            }
            AiEvent::Done => {
                inline.job = None;
                let rel = inline.rel.clone();
                let code = diff::strip_fence(&inline.output);
                let code = code.trim_end_matches('\n');
                let (start, end, generate) = (inline.start, inline.end, inline.generate);
                let base = self
                    .pending
                    .iter()
                    .find(|p| p.origin == Origin::Inline && p.rel == rel)
                    .map(|p| p.original.clone())
                    .or_else(|| {
                        self.tabs
                            .iter()
                            .find(|t| t.rel == rel)
                            .map(|t| t.text.clone())
                    });
                let Some(base) = base else { return };
                let insert =
                    if generate && start == end && start > 0 && !base[..start].ends_with('\n') {
                        format!("\n{code}")
                    } else {
                        code.to_string()
                    };
                let proposed = format!("{}{}{}", &base[..start], insert, &base[end..]);
                self.pending
                    .retain(|p| !(p.origin == Origin::Inline && p.rel == rel));
                self.pending.push(Pending {
                    rel,
                    original: base,
                    proposed: Some(proposed),
                    origin: Origin::Inline,
                    is_new: false,
                    scrolled: false,
                });
            }
            _ => {}
        }
    }

    /// Ctrl+L inside the prompt bar: carry the prompt and selection over to chat.
    pub fn inline_to_chat(&mut self) {
        let Some(inline) = self.inline.take() else {
            return;
        };
        if let Some(tab) = self.tabs.iter().find(|t| t.rel == inline.rel) {
            let text = tab.text[inline.start..inline.end.min(tab.text.len())].to_string();
            let start_line = inline.line + 1;
            let end_line = start_line + text.matches('\n').count();
            if !text.trim().is_empty() {
                self.chats[self.chat].selections.push(Selection {
                    rel: inline.rel.clone(),
                    start_line,
                    end_line,
                    text,
                });
            }
        }
        self.pending
            .retain(|p| !(p.origin == Origin::Inline && p.rel == inline.rel));
        self.chats[self.chat].mode = Mode::Agent;
        self.input = inline.prompt;
        self.chat_open = true;
        self.focus_input = true;
    }

    // ---------------------------------------------------------------- review

    pub fn pending_for_active(&self) -> Option<usize> {
        let tab = &self.tabs[self.active?];
        self.pending.iter().position(|p| p.rel == tab.rel)
    }

    pub fn accept(&mut self, i: usize) {
        if i >= self.pending.len() {
            return;
        }
        let p = self.pending.remove(i);
        let result = match p.origin {
            Origin::Inline => {
                if let Some(tab) = self.tabs.iter_mut().find(|t| t.rel == p.rel) {
                    tab.text = p.proposed.clone().unwrap_or_default();
                }
                self.inline = None;
                self.notice = format!("Edit accepted in {} (not saved yet: Ctrl+S)", p.rel);
                Ok(())
            }
            Origin::Agent { chat, msg, change } => {
                let r = match &p.proposed {
                    Some(text) => self.write_file(&p.rel, text).map(|_| {
                        if let Some(tab) = self.tabs.iter_mut().find(|t| t.rel == p.rel) {
                            tab.text = text.clone();
                            tab.saved = text.clone();
                            tab.exists = true;
                        }
                    }),
                    None => self.delete_file(&p.rel),
                };
                self.set_change_state(
                    chat,
                    msg,
                    change,
                    if r.is_ok() {
                        ChangeState::Accepted
                    } else {
                        ChangeState::Pending
                    },
                );
                if r.is_ok() {
                    self.notice = format!("Applied {}", p.rel);
                }
                r
            }
        };
        if let Err(e) = result {
            self.notice = e;
        }
    }

    pub fn reject(&mut self, i: usize) {
        if i >= self.pending.len() {
            return;
        }
        let p = self.pending.remove(i);
        match p.origin {
            Origin::Inline => {
                // keep the bar open so the person can try another instruction
                if let Some(inline) = &mut self.inline {
                    inline.output.clear();
                    inline.focus = true;
                }
            }
            Origin::Agent { chat, msg, change } => {
                self.set_change_state(chat, msg, change, ChangeState::Rejected)
            }
        }
        if p.is_new {
            if let Some(t) = self.tabs.iter().position(|t| t.rel == p.rel && !t.exists) {
                self.close_tab(t);
            }
        }
        self.notice = format!("Rejected the change to {}", p.rel);
    }

    pub fn accept_all_agent(&mut self) {
        while let Some(i) = self
            .pending
            .iter()
            .position(|p| matches!(p.origin, Origin::Agent { .. }))
        {
            self.accept(i);
        }
    }

    pub fn reject_all_agent(&mut self) {
        while let Some(i) = self
            .pending
            .iter()
            .position(|p| matches!(p.origin, Origin::Agent { .. }))
        {
            self.reject(i);
        }
    }

    fn set_change_state(&mut self, chat: usize, msg: usize, change: usize, state: ChangeState) {
        if let Some(m) = self
            .chats
            .get_mut(chat)
            .and_then(|c| c.messages.get_mut(msg))
        {
            if let Some(s) = m.change_state.get_mut(change) {
                *s = state;
            }
        }
    }

    fn delete_file(&mut self, rel: &str) -> Result<(), String> {
        let path = self.safe_path(rel)?;
        if path.exists() {
            fs::remove_file(&path).map_err(|e| format!("Could not delete {rel}: {e}"))?;
        }
        if let Some(t) = self.tabs.iter().position(|t| t.rel == rel) {
            self.close_tab(t);
        }
        if let Ok(mut idx) = self.index.write() {
            idx.remove_file(rel);
        }
        self.refresh_files();
        Ok(())
    }

    // ---------------------------------------------------------------- chat

    pub fn open_chat(&mut self, mode: Mode, with_selection: bool) {
        self.chat_open = true;
        self.focus_input = true;
        if self.chats[self.chat].messages.is_empty() || self.running_chat_job().is_none() {
            self.chats[self.chat].mode = mode;
        }
        if with_selection {
            self.add_selection_to_chat();
        }
    }

    pub fn new_chat(&mut self, mode: Mode) {
        if self.chats[self.chat].messages.is_empty() {
            self.chats[self.chat].mode = mode;
        } else {
            self.chats.push(Chat::new(mode));
            self.chat = self.chats.len() - 1;
        }
        self.input.clear();
        self.chat_open = true;
        self.focus_input = true;
    }

    pub fn add_selection_to_chat(&mut self) {
        let Some(i) = self.active else { return };
        let tab = &self.tabs[i];
        let (a, b) = tab.sel;
        if a == b {
            return;
        }
        let sa = char_to_byte(&tab.text, a);
        let sb = char_to_byte(&tab.text, b);
        let text = tab.text[sa..sb].to_string();
        let start_line = tab.text[..sa].matches('\n').count() + 1;
        let end_line = start_line + text.trim_end_matches('\n').matches('\n').count();
        let chat = &mut self.chats[self.chat];
        if !chat
            .selections
            .iter()
            .any(|s| s.rel == tab.rel && s.start_line == start_line && s.end_line == end_line)
        {
            chat.selections.push(Selection {
                rel: tab.rel.clone(),
                start_line,
                end_line,
                text,
            });
        }
    }

    /// `@` words in a message: files, folders and `codebase`.
    pub fn mentions(&self, text: &str) -> (Vec<String>, Vec<String>, bool) {
        let mut files = Vec::new();
        let mut folders = Vec::new();
        let mut codebase = false;
        for word in text.split_whitespace() {
            let Some(name) = word.strip_prefix('@') else {
                continue;
            };
            let name = name.trim_end_matches([',', '.', '?', '!', ')', ':', ';']);
            if name.eq_ignore_ascii_case("codebase") {
                codebase = true;
                continue;
            }
            let bare = name.trim_end_matches('/');
            if self.files.iter().any(|f| f == bare) {
                if !files.contains(&bare.to_string()) {
                    files.push(bare.to_string());
                }
            } else if self.dirs.iter().any(|d| d == bare) || (bare.is_empty() && name == "/") {
                if !folders.contains(&bare.to_string()) {
                    folders.push(bare.to_string());
                }
            }
        }
        (files, folders, codebase)
    }

    /// The `@query` being typed at the end of the input, if any.
    pub fn mention_query(&self) -> Option<(usize, String)> {
        let at = self.input.rfind('@')?;
        let before_ok = at == 0 || self.input[..at].ends_with(char::is_whitespace);
        let query = &self.input[at + 1..];
        (before_ok && !query.contains(char::is_whitespace)).then(|| (at, query.to_string()))
    }

    pub fn mention_candidates(&self, query: &str) -> Vec<(String, bool)> {
        let q = query.to_lowercase();
        let mut out: Vec<(String, bool, i32)> = Vec::new();
        if "codebase".starts_with(&q) {
            out.push(("codebase".into(), false, -100));
        }
        let score = |path: &str| -> Option<i32> {
            let lower = path.to_lowercase();
            let name = lower.rsplit('/').next().unwrap_or(&lower);
            if q.is_empty() {
                Some(lower.matches('/').count() as i32 * 10 + lower.len() as i32)
            } else if name.starts_with(&q) {
                Some(lower.len() as i32)
            } else if lower.contains(&q) {
                Some(1000 + lower.len() as i32)
            } else {
                None
            }
        };
        for d in &self.dirs {
            if let Some(s) = score(&format!("{d}/")) {
                out.push((format!("{d}/"), true, s));
            }
        }
        for f in &self.files {
            if let Some(s) = score(f) {
                out.push((f.clone(), false, s));
            }
        }
        out.sort_by_key(|o| o.2);
        out.into_iter().take(12).map(|(p, d, _)| (p, d)).collect()
    }

    pub fn pick_mention(&mut self, path: &str) {
        if let Some((at, _)) = self.mention_query() {
            self.input.truncate(at);
            self.input.push('@');
            self.input.push_str(path);
            self.input.push(' ');
        }
        self.mention_sel = 0;
        self.focus_input = true;
    }

    fn read_rel(&self, rel: &str) -> Option<String> {
        if let Some(tab) = self.tabs.iter().find(|t| t.rel == rel) {
            return Some(tab.text.clone());
        }
        let bytes = fs::read(self.safe_path(rel).ok()?).ok()?;
        if bytes.iter().take(8000).any(|b| *b == 0) {
            return None;
        }
        Some(String::from_utf8_lossy(&bytes).to_string())
    }

    pub fn send_chat(&mut self, ctx: &egui::Context) {
        let text = self.input.trim().to_string();
        if text.is_empty() || self.running_chat_job().is_some() {
            return;
        }
        if self.root.is_none() {
            self.notice = "Open a folder first so the AI can see your code.".into();
        }
        let mode = self.chats[self.chat].mode;
        let (files, folders, explicit_codebase) = self.mentions(&text);
        let mut labels = Vec::new();
        let mut context = String::new();
        let mut budget = CONTEXT_BUDGET;
        let attach = |labels: &mut Vec<String>,
                      label: String,
                      rel: &str,
                      body: &str,
                      context: &mut String,
                      budget: &mut usize| {
            if *budget == 0 {
                return false;
            }
            let mut body = body.to_string();
            let cap = FILE_BUDGET.min(*budget);
            if body.len() > cap {
                let mut cut = cap;
                while !body.is_char_boundary(cut) {
                    cut -= 1;
                }
                body.truncate(cut);
                body.push_str("\n… (truncated)");
            }
            *budget = budget.saturating_sub(body.len());
            context.push_str(&format!("\n--- {rel} ---\n{body}\n"));
            labels.push(label);
            true
        };
        // current file
        if self.settings.include_current_file {
            if let Some(i) = self.active {
                let tab = &self.tabs[i];
                let line = tab.text[..char_to_byte(&tab.text, tab.sel.0)]
                    .matches('\n')
                    .count()
                    + 1;
                let body = tab.text.clone();
                let rel = tab.rel.clone();
                attach(
                    &mut labels,
                    format!("{rel} (current file)"),
                    &format!("{rel} (current file, cursor on line {line})"),
                    &body,
                    &mut context,
                    &mut budget,
                );
            }
        }
        // selections
        let selections = std::mem::take(&mut self.chats[self.chat].selections);
        for s in &selections {
            attach(
                &mut labels,
                format!("{} ({}-{})", s.rel, s.start_line, s.end_line),
                &format!(
                    "selected code from {} lines {}-{}",
                    s.rel, s.start_line, s.end_line
                ),
                &s.text,
                &mut context,
                &mut budget,
            );
        }
        for rel in &files {
            if let Some(body) = self.read_rel(rel) {
                attach(
                    &mut labels,
                    rel.clone(),
                    rel,
                    &body,
                    &mut context,
                    &mut budget,
                );
            }
        }
        for folder in &folders {
            let prefix = if folder.is_empty() {
                String::new()
            } else {
                format!("{folder}/")
            };
            let mut count = 0;
            let members: Vec<String> = self
                .files
                .iter()
                .filter(|f| f.starts_with(&prefix))
                .cloned()
                .collect();
            for rel in members {
                if let Some(body) = self.read_rel(&rel) {
                    if attach(
                        &mut labels,
                        String::new(),
                        &rel,
                        &body,
                        &mut context,
                        &mut budget,
                    ) {
                        labels.pop();
                        count += 1;
                    }
                }
            }
            labels.push(format!("{}/ ({count} files)", folder));
        }
        let index_ready = self.index.read().map(|i| i.is_ready()).unwrap_or(false);
        let codebase = explicit_codebase
            || (self.settings.auto_codebase_context
                && index_ready
                && files.is_empty()
                && folders.is_empty()
                && selections.is_empty());
        if codebase {
            labels.push("codebase".into());
        }
        let mut prompt = String::new();
        if !context.is_empty() {
            prompt.push_str("Attached context:\n");
            prompt.push_str(&context);
            prompt.push_str("\n\n");
        }
        prompt.push_str(&text);

        let chat_index = self.chat;
        let chat = &mut self.chats[chat_index];
        if chat.messages.is_empty() {
            chat.title = text.chars().take(32).collect();
        }
        let mut turns: Vec<Turn> = chat
            .messages
            .iter()
            .filter(|m| m.error.is_none())
            .map(|m| Turn {
                role: m.role,
                content: m.text.clone(),
            })
            .collect();
        turns.push(Turn {
            role: Role::User,
            content: prompt,
        });
        chat.messages.push(Msg::user(text.clone(), mode, labels));
        let mut reply = Msg::user(String::new(), mode, vec![]);
        reply.role = Role::Assistant;
        reply.streaming = true;
        reply.status = "Thinking…".into();
        chat.messages.push(reply);
        let msg = chat.messages.len() - 1;
        self.input.clear();

        let system = self.system_prompt(mode);
        let (id, cancel) = self.new_job(Job::Chat {
            chat: chat_index,
            msg,
        });
        ai::spawn(
            Request {
                job: id,
                settings: self.settings.clone(),
                system,
                turns,
                codebase: codebase.then(|| (text, self.index.clone(), CODEBASE_HITS)),
                cancel,
            },
            self.tx.clone(),
            ctx.clone(),
        );
    }

    fn system_prompt(&self, mode: Mode) -> String {
        let mut s = String::from(
            "You are OpenCode, an AI pair programmer inside a code editor. The user works in the \
             workspace described below. Answer in the same language the user writes in (if they write \
             Thai, answer in Thai; keep code identifiers as they are). When you point at code, name the \
             file and line like src/app.rs:42. Use fenced code blocks with a language.\n",
        );
        match mode {
            Mode::Ask => s.push_str(
                "Mode: Ask. Explain and answer; do not rewrite whole files. Show short code snippets when useful.\n",
            ),
            Mode::Plan => s.push_str(
                "Mode: Plan. Write a clear, numbered implementation plan: which files to change or create and what \
                 to do in each. Do not write complete files.\n",
            ),
            Mode::Agent => s.push_str(
                "Mode: Agent. You can create, change and delete files in the workspace. First explain briefly what \
                 you will do. Then, for EVERY file you create or change, output its COMPLETE new content in exactly \
                 this form (no other wrapper):\n<<<FILE relative/path/from/workspace.ext\n...entire file content...\n>>>END\n\
                 To delete a file write: <<<DELETE relative/path>>>\nYou may change many files in one answer. Paths \
                 are relative to the workspace root and use '/'. Never touch files outside the workspace. The user \
                 reviews every file before it is written.\n",
            ),
        }
        if let Some(root) = &self.root {
            s.push_str(&format!("\nWorkspace folder: {}\n", self.workspace_name()));
            let _ = root;
            s.push_str(&format!(
                "Files in the workspace ({} total):\n",
                self.files.len()
            ));
            for f in self.files.iter().take(400) {
                s.push_str(f);
                s.push('\n');
            }
            if self.files.len() > 400 {
                s.push_str("…\n");
            }
        } else {
            s.push_str("\nNo workspace folder is open.\n");
        }
        s
    }

    fn chat_event(&mut self, chat: usize, msg: usize, event: AiEvent) {
        if matches!(event, AiEvent::Done | AiEvent::Error(_)) {
            if let Some(c) = self.chats.get_mut(chat) {
                c.updated = crate::history::now();
            }
        }
        let finished = matches!(event, AiEvent::Done);
        self.chat_event_inner(chat, msg, event);
        if finished {
            self.save_history();
            self.refresh_history_cache();
        }
    }

    fn chat_event_inner(&mut self, chat: usize, msg: usize, event: AiEvent) {
        let Some(m) = self
            .chats
            .get_mut(chat)
            .and_then(|c| c.messages.get_mut(msg))
        else {
            return;
        };
        match event {
            AiEvent::Delta(d) => {
                m.text.push_str(&d);
                m.status.clear();
            }
            AiEvent::Status(s) => m.status = s,
            AiEvent::Sources(s) => m.sources = s,
            AiEvent::Keywords(k) => m.keywords = k,
            AiEvent::Error(e) => {
                m.streaming = false;
                m.status.clear();
                m.error = Some(e);
            }
            AiEvent::Done => {
                m.streaming = false;
                m.status.clear();
                if m.mode == Mode::Agent {
                    let (_, changes) = diff::parse_agent_answer(&m.text);
                    m.change_state = vec![ChangeState::Pending; changes.len()];
                    m.changes = changes.clone();
                    let mut first = None;
                    for (n, change) in changes.into_iter().enumerate() {
                        let original = self.read_rel(&change.rel).unwrap_or_default();
                        let is_new = self
                            .safe_path(&change.rel)
                            .map(|p| !p.exists())
                            .unwrap_or(true);
                        if let Err(e) = self.safe_path(&change.rel) {
                            self.notice = e;
                            self.set_change_state(chat, msg, n, ChangeState::Rejected);
                            continue;
                        }
                        self.pending.retain(|p| p.rel != change.rel);
                        self.pending.push(Pending {
                            rel: change.rel.clone(),
                            original,
                            proposed: change.content,
                            origin: Origin::Agent {
                                chat,
                                msg,
                                change: n,
                            },
                            is_new,
                            scrolled: false,
                        });
                        first.get_or_insert(change.rel);
                    }
                    if let Some(rel) = first {
                        self.open_file(&rel);
                        self.notice = "Review the agent's changes: Ctrl+Enter accepts, Ctrl+Backspace rejects.".into();
                    }
                }
            }
        }
    }

    // ---------------------------------------------------------------- search

    pub fn start_search(&mut self, ctx: &egui::Context) {
        if self.search_query.trim().is_empty() {
            return;
        }
        self.search_keywords.clear();
        self.search_done = false;
        // non-English questions are turned into code words by the model first
        let needs_words =
            !self.search_query.is_ascii() || self.search_query.split_whitespace().count() > 3;
        if needs_words && self.settings.ready() {
            self.search_busy = true;
            let (id, cancel) = self.new_job(Job::SearchKeywords);
            let settings = self.settings.clone();
            let question = self.search_query.clone();
            let tx = self.tx.clone();
            let ctx = ctx.clone();
            std::thread::spawn(move || {
                let result = ai::keywords_for(&settings, &question, &cancel);
                let _ = match result {
                    Ok(words) => {
                        let _ = tx.send((id, AiEvent::Delta(words)));
                        tx.send((id, AiEvent::Done))
                    }
                    Err(e) => tx.send((id, AiEvent::Error(e))),
                };
                ctx.request_repaint();
            });
        } else {
            self.run_search();
        }
    }

    fn run_search(&mut self) {
        let query = format!("{} {}", self.search_query, self.search_keywords);
        self.search_results = self
            .index
            .read()
            .map(|i| i.search(&query, 30))
            .unwrap_or_default();
        self.search_busy = false;
        self.search_done = true;
    }

    pub fn test_connection(&mut self, ctx: &egui::Context) {
        if self.jobs.values().any(|(job, _)| matches!(job, Job::Test)) {
            return;
        }
        self.test_result.clear();
        let (id, cancel) = self.new_job(Job::Test);
        ai::spawn(
            Request {
                job: id,
                settings: self.settings.clone(),
                system: "Reply with one short sentence.".into(),
                turns: vec![Turn {
                    role: Role::User,
                    content: "Say hello and name the model you are.".into(),
                }],
                codebase: None,
                cancel,
            },
            self.tx.clone(),
            ctx.clone(),
        );
    }

    pub fn insert_at_cursor(&mut self, code: &str) {
        let Some(i) = self.active else {
            self.notice = "Open a file to insert code into.".into();
            return;
        };
        let tab = &mut self.tabs[i];
        let at = char_to_byte(&tab.text, tab.sel.1);
        tab.text.insert_str(at, code);
        self.notice = format!("Inserted into {}", tab.rel);
    }
}

pub fn char_to_byte(text: &str, chars: usize) -> usize {
    text.char_indices()
        .nth(chars)
        .map(|(b, _)| b)
        .unwrap_or(text.len())
}

fn split_parent(rel: &str) -> (String, String) {
    match rel.rfind('/') {
        Some(p) => (rel[..p].to_string(), rel[p + 1..].to_string()),
        None => (String::new(), rel.to_string()),
    }
}

/// Windows canonical paths start with \\?\ — drop it so paths compare and display normally.
pub fn strip_unc(path: PathBuf) -> PathBuf {
    let s = path.to_string_lossy();
    match s.strip_prefix(r"\\?\") {
        Some(rest) if !rest.starts_with("UNC") => PathBuf::from(rest),
        _ => path,
    }
}
