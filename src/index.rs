//! Local codebase indexing: walk the workspace, chunk every text file, and rank
//! chunks for a question with BM25. Unchanged files are reused on re-sync
//! (each file keeps a content hash; the workspace fingerprint hashes them all).

use std::{
    collections::{hash_map::DefaultHasher, HashMap, HashSet},
    fs,
    hash::{Hash, Hasher},
    path::{Path, PathBuf},
    time::{Instant, SystemTime},
};

/// From the original (absoluteMaxNumberFiles:1e5).
pub const MAX_FILES: usize = 100_000;
/// Estimated.
pub const MAX_FILE_BYTES: u64 = 1_048_576;
/// Lower limit of a piece: from the original's `chunk(40, 80)` call
/// (cursor-agent-exec), passed unchanged by tree-chunk-napi 0x1800ae330.
pub const CHUNK_MIN: usize = 40;
/// Upper limit of a piece: same call; the greedy splitter at 0x1800ad290
/// only adds a block while `current + block < CHUNK_MAX`.
pub const CHUNK_MAX: usize = 80;
/// Estimated BM25 parameters.
const K1: f32 = 1.2;
const B: f32 = 0.75;
/// Estimated: words in a file's path count this much.
const PATH_WEIGHT: f32 = 2.0;

pub const SKIP_DIRS: &[&str] = &[
    ".git",
    "node_modules",
    "target",
    "dist",
    "build",
    "out",
    ".next",
    ".venv",
    "venv",
    "__pycache__",
    ".idea",
    ".vscode",
    ".gradle",
    "bin",
    "obj",
    ".cache",
    "vendor",
];

const IGNORE_FILES: &[&str] = &[
    ".gitignore",
    ".opencodeignore",
    ".cursorignore",
    ".cursorindexingignore",
];

#[derive(Clone)]
pub struct ChunkData {
    pub start_line: usize,
    pub end_line: usize,
    pub len: f32,
    pub tf: HashMap<String, f32>,
}

#[derive(Clone)]
pub struct FileData {
    pub rel: String,
    pub size: u64,
    pub modified: Option<SystemTime>,
    pub hash: u64,
    pub chunks: Vec<ChunkData>,
}

#[derive(Default, Clone)]
pub struct Index {
    pub root: Option<PathBuf>,
    pub files: Vec<FileData>,
    pub df: HashMap<String, u32>,
    pub chunk_count: usize,
    pub avg_len: f32,
    pub fingerprint: u64,
    pub skipped_large: usize,
    pub skipped_binary: usize,
    pub reused: usize,
    pub millis: u128,
    pub error: Option<String>,
}

#[derive(Clone, Debug)]
pub struct Hit {
    pub rel: String,
    pub start_line: usize,
    pub end_line: usize,
    #[allow(dead_code)]
    pub score: f32,
    pub snippet: String,
}

/// Ignore rules from .gitignore / .opencodeignore / .cursorignore at the workspace root.
#[derive(Clone, Default)]
pub struct Ignore {
    patterns: Vec<(String, bool, bool)>, // (pattern, anchored, dir_only)
}

impl Ignore {
    pub fn load(root: &Path) -> Self {
        let mut patterns = Vec::new();
        for name in IGNORE_FILES {
            let Ok(text) = fs::read_to_string(root.join(name)) else {
                continue;
            };
            for line in text.lines() {
                let line = line.trim();
                if line.is_empty() || line.starts_with('#') || line.starts_with('!') {
                    continue;
                }
                let dir_only = line.ends_with('/');
                let mut p = line.trim_end_matches('/').to_string();
                let anchored = p.starts_with('/') || p.contains('/');
                p = p.trim_start_matches('/').to_string();
                if !p.is_empty() {
                    patterns.push((p, anchored, dir_only));
                }
            }
        }
        Self { patterns }
    }

    pub fn ignored(&self, rel: &str, is_dir: bool) -> bool {
        let name = rel.rsplit('/').next().unwrap_or(rel);
        if is_dir && SKIP_DIRS.contains(&name) {
            return true;
        }
        self.patterns.iter().any(|(p, anchored, dir_only)| {
            if *dir_only && !is_dir {
                return false;
            }
            if *anchored {
                glob(p, rel) || glob(&format!("{p}/**"), rel)
            } else {
                glob(p, name)
            }
        })
    }
}

/// `*` and `?` stay inside one path part; `**` crosses parts.
fn glob(pattern: &str, text: &str) -> bool {
    let p: Vec<char> = pattern.chars().collect();
    let t: Vec<char> = text.chars().collect();
    fn go(p: &[char], t: &[char]) -> bool {
        if p.is_empty() {
            return t.is_empty();
        }
        if p.len() >= 2 && p[0] == '*' && p[1] == '*' {
            let rest = if p.len() > 2 && p[2] == '/' {
                &p[3..]
            } else {
                &p[2..]
            };
            return (0..=t.len()).any(|i| go(rest, &t[i..]));
        }
        match p[0] {
            '*' => (0..=t.len())
                .take_while(|&i| i == 0 || t[i - 1] != '/')
                .any(|i| go(&p[1..], &t[i..])),
            '?' => !t.is_empty() && t[0] != '/' && go(&p[1..], &t[1..]),
            c => !t.is_empty() && t[0].eq_ignore_ascii_case(&c) && go(&p[1..], &t[1..]),
        }
    }
    go(&p, &t)
}

/// Every file and folder in the workspace that isn't ignored (relative, `/`-separated).
pub fn list_paths(root: &Path) -> (Vec<String>, Vec<String>) {
    let ignore = Ignore::load(root);
    let mut files = Vec::new();
    let mut dirs = Vec::new();
    let walker = walkdir::WalkDir::new(root).follow_links(false).into_iter();
    for entry in walker.filter_entry(|e| {
        let rel = rel_of(root, e.path());
        rel.is_empty() || !ignore.ignored(&rel, e.file_type().is_dir())
    }) {
        let Ok(entry) = entry else { continue };
        let rel = rel_of(root, entry.path());
        if rel.is_empty() {
            continue;
        }
        if entry.file_type().is_dir() {
            dirs.push(rel);
        } else if entry.file_type().is_file() {
            files.push(rel);
            if files.len() >= MAX_FILES {
                break;
            }
        }
    }
    files.sort();
    dirs.sort();
    (files, dirs)
}

pub fn rel_of(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}

/// Build (or re-sync, reusing unchanged files from `previous`) the index.
pub fn build(root: &Path, previous: Option<&Index>, progress: &dyn Fn(usize, usize)) -> Index {
    let started = Instant::now();
    let (files, _) = list_paths(root);
    let mut index = Index {
        root: Some(root.to_path_buf()),
        ..Default::default()
    };
    if files.len() >= MAX_FILES {
        index.error = Some(format!("Too many files to index (limit {MAX_FILES})."));
    }
    let old: HashMap<&str, &FileData> = previous
        .filter(|p| p.root.as_deref() == Some(root))
        .map(|p| p.files.iter().map(|f| (f.rel.as_str(), f)).collect())
        .unwrap_or_default();
    let total = files.len();
    for (n, rel) in files.iter().enumerate() {
        if n % 50 == 0 {
            progress(n, total);
        }
        let path = root.join(rel);
        let Ok(meta) = fs::metadata(&path) else {
            continue;
        };
        if meta.len() > MAX_FILE_BYTES {
            index.skipped_large += 1;
            continue;
        }
        let modified = meta.modified().ok();
        if let Some(prev) = old.get(rel.as_str()) {
            if prev.size == meta.len() && prev.modified == modified && modified.is_some() {
                index.files.push((*prev).clone());
                index.reused += 1;
                continue;
            }
        }
        let Ok(bytes) = fs::read(&path) else { continue };
        if bytes.iter().take(8000).any(|b| *b == 0) {
            index.skipped_binary += 1;
            continue;
        }
        let text = String::from_utf8_lossy(&bytes);
        index
            .files
            .push(file_data(rel, &text, meta.len(), modified));
    }
    index.finish();
    index.millis = started.elapsed().as_millis();
    progress(total, total);
    index
}

pub fn file_data(rel: &str, text: &str, size: u64, modified: Option<SystemTime>) -> FileData {
    let mut hasher = DefaultHasher::new();
    text.hash(&mut hasher);
    let lines: Vec<&str> = text.lines().collect();
    let path_terms = tokens(rel);
    let mut chunks = Vec::new();
    for (start, end) in greedy_chunks(&lines, CHUNK_MIN, CHUNK_MAX) {
        let mut tf: HashMap<String, f32> = HashMap::new();
        let mut len = 0.0;
        for line in &lines[start..end] {
            for t in tokens(line) {
                *tf.entry(t).or_default() += 1.0;
                len += 1.0;
            }
        }
        for t in &path_terms {
            *tf.entry(t.clone()).or_default() += PATH_WEIGHT;
            len += PATH_WEIGHT;
        }
        chunks.push(ChunkData {
            start_line: start + 1,
            end_line: end.max(start + 1),
            len,
            tf,
        });
    }
    FileData {
        rel: rel.to_string(),
        size,
        modified,
        hash: hasher.finish(),
        chunks,
    }
}

/// Top-level blocks of a file (functions, classes, items): a block starts at a
/// non-blank line at column 0 while no bracket is open. A rough stand-in for the
/// syntax tree the original walks.
fn blocks(lines: &[&str]) -> Vec<(usize, usize)> {
    let mut out = Vec::new();
    let mut start = 0;
    let mut depth: i32 = 0;
    for (i, line) in lines.iter().enumerate() {
        let starts_block = i > start
            && depth <= 0
            && !line.trim().is_empty()
            && !line.starts_with(char::is_whitespace)
            && !line.trim_start().starts_with(['}', ')', ']']);
        if starts_block {
            out.push((start, i));
            start = i;
        }
        for c in line.chars() {
            match c {
                '{' | '(' | '[' => depth += 1,
                '}' | ')' | ']' => depth -= 1,
                _ => {}
            }
        }
    }
    if start < lines.len() {
        out.push((start, lines.len()));
    }
    out
}

/// The original's greedy packing (tree-chunk-napi 0x1800ad290), in lines:
/// a block joins the current piece while `current + block < max`; when it
/// doesn't fit, the piece is closed if it is already longer than `min`, and a
/// block that still doesn't fit is split into its parts (here: its lines)
/// which go back to the front of the queue; a single line that is too big is
/// taken whole. Pieces don't overlap.
pub fn greedy_chunks(lines: &[&str], min: usize, max: usize) -> Vec<(usize, usize)> {
    let mut queue: std::collections::VecDeque<(usize, usize)> = blocks(lines).into();
    let mut out = Vec::new();
    let mut cur: Option<(usize, usize)> = None;
    let size = |c: &Option<(usize, usize)>| c.map(|(s, e)| e - s).unwrap_or(0);
    while let Some((s, e)) = queue.pop_front() {
        let n = e - s;
        if size(&cur) + n < max {
            cur = Some(cur.map(|(cs, _)| (cs, e)).unwrap_or((s, e)));
            continue;
        }
        if size(&cur) > min {
            out.extend(cur.take());
        }
        if n > 1 && size(&cur) + n > max {
            for i in (s..e).rev() {
                queue.push_front((i, i + 1));
            }
        } else {
            cur = Some(cur.map(|(cs, _)| (cs, e)).unwrap_or((s, e)));
        }
    }
    out.extend(cur);
    if out.is_empty() {
        out.push((0, lines.len()));
    }
    out
}

impl Index {
    pub fn finish(&mut self) {
        self.df.clear();
        let mut total_len = 0.0;
        let mut count = 0;
        let mut fp = DefaultHasher::new();
        self.files.sort_by(|a, b| a.rel.cmp(&b.rel));
        for file in &self.files {
            (&file.rel, file.hash).hash(&mut fp);
            for chunk in &file.chunks {
                count += 1;
                total_len += chunk.len;
                for term in chunk.tf.keys() {
                    *self.df.entry(term.clone()).or_default() += 1;
                }
            }
        }
        self.chunk_count = count;
        self.avg_len = if count > 0 {
            total_len / count as f32
        } else {
            1.0
        };
        self.fingerprint = fp.finish();
    }

    /// Replace one file's data after it was saved or edited.
    pub fn update_file(&mut self, rel: &str, text: &str) {
        self.files.retain(|f| f.rel != rel);
        self.files
            .push(file_data(rel, text, text.len() as u64, None));
        self.finish();
    }

    pub fn remove_file(&mut self, rel: &str) {
        self.files.retain(|f| f.rel != rel);
        self.finish();
    }

    pub fn is_ready(&self) -> bool {
        self.root.is_some() && !self.files.is_empty()
    }

    pub fn search(&self, query: &str, limit: usize) -> Vec<Hit> {
        let terms: HashSet<String> = tokens(query).into_iter().collect();
        if terms.is_empty() || self.chunk_count == 0 {
            return Vec::new();
        }
        let n = self.chunk_count as f32;
        let idf: Vec<(&String, f32)> = terms
            .iter()
            .filter_map(|t| {
                let df = *self.df.get(t)? as f32;
                Some((t, ((n - df + 0.5) / (df + 0.5) + 1.0).ln()))
            })
            .collect();
        if idf.is_empty() {
            return Vec::new();
        }
        let mut scored: Vec<(f32, usize, usize)> = Vec::new();
        for (fi, file) in self.files.iter().enumerate() {
            for (ci, chunk) in file.chunks.iter().enumerate() {
                let mut score = 0.0;
                for (term, w) in &idf {
                    if let Some(tf) = chunk.tf.get(*term) {
                        score += w * (tf * (K1 + 1.0))
                            / (tf + K1 * (1.0 - B + B * chunk.len / self.avg_len));
                    }
                }
                if score > 0.0 {
                    scored.push((score, fi, ci));
                }
            }
        }
        scored.sort_by(|a, b| b.0.total_cmp(&a.0));
        let mut hits = Vec::new();
        let mut per_file: HashMap<usize, usize> = HashMap::new();
        let root = self.root.clone().unwrap_or_default();
        for (score, fi, ci) in scored {
            let used = per_file.entry(fi).or_default();
            if *used >= 2 {
                continue;
            }
            *used += 1;
            let file = &self.files[fi];
            let chunk = &file.chunks[ci];
            let snippet = fs::read_to_string(root.join(&file.rel))
                .map(|text| {
                    text.lines()
                        .skip(chunk.start_line - 1)
                        .take(chunk.end_line + 1 - chunk.start_line)
                        .collect::<Vec<_>>()
                        .join("\n")
                })
                .unwrap_or_default();
            hits.push(Hit {
                rel: file.rel.clone(),
                start_line: chunk.start_line,
                end_line: chunk.end_line,
                score,
                snippet,
            });
            if hits.len() >= limit {
                break;
            }
        }
        hits
    }
}

const STOP: &[&str] = &[
    "the", "a", "an", "is", "are", "of", "to", "in", "and", "or", "for", "on", "it", "this",
    "that", "be", "with", "as", "at", "by", "from", "where", "what", "how", "does", "do", "which",
    "let", "var", "const", "fn", "pub", "if", "else", "return", "self", "new", "get", "set",
];

/// Lower-case search words: whole identifiers plus their snake/kebab/camelCase parts.
pub fn tokens(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    for word in text.split(|c: char| !(c.is_alphanumeric() || c == '_')) {
        if word.is_empty() {
            continue;
        }
        let whole = stem(&word.to_lowercase());
        let mut parts = Vec::new();
        for piece in word.split('_') {
            let chars: Vec<char> = piece.chars().collect();
            let mut current = String::new();
            for (i, c) in chars.iter().enumerate() {
                let boundary = i > 0
                    && c.is_uppercase()
                    && (chars[i - 1].is_lowercase()
                        || chars.get(i + 1).map(|n| n.is_lowercase()).unwrap_or(false));
                if boundary && !current.is_empty() {
                    parts.push(std::mem::take(&mut current));
                }
                current.push(*c);
            }
            if !current.is_empty() {
                parts.push(current);
            }
        }
        if whole.chars().count() >= 2 && !STOP.contains(&whole.as_str()) {
            out.push(whole.clone());
        }
        if parts.len() > 1 {
            for part in parts {
                let part = stem(&part.to_lowercase());
                if part.chars().count() >= 2 && part != whole && !STOP.contains(&part.as_str()) {
                    out.push(part);
                }
            }
        }
    }
    out
}

fn stem(word: &str) -> String {
    if word.len() > 4 && word.ends_with("ies") {
        format!("{}y", &word[..word.len() - 3])
    } else if word.len() > 3 && word.ends_with('s') && !word.ends_with("ss") && word.is_ascii() {
        word[..word.len() - 1].to_string()
    } else {
        word.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splits_identifiers() {
        let t = tokens("processPaymentIntent stripe_checkout");
        assert!(t.contains(&"payment".to_string()));
        assert!(t.contains(&"checkout".to_string()));
        assert!(t.contains(&"processpaymentintent".to_string()));
    }

    #[test]
    fn globs() {
        assert!(glob("*.log", "debug.log"));
        assert!(glob("src/**/*.rs", "src/a/b/c.rs"));
        assert!(!glob("*.rs", "src/a.rs"));
    }

    #[test]
    fn greedy_keeps_blocks_whole() {
        // ten 15-line functions: pieces never cut a function, never overlap,
        // and stay under 80 lines once past 40
        let mut text = String::new();
        for f in 0..10 {
            text.push_str(&format!("fn f{f}() {{\n"));
            for _ in 0..13 {
                text.push_str("    work();\n");
            }
            text.push_str("}\n");
        }
        let lines: Vec<&str> = text.lines().collect();
        let pieces = greedy_chunks(&lines, CHUNK_MIN, CHUNK_MAX);
        let mut next = 0;
        for (s, e) in &pieces {
            assert_eq!(*s, next, "no gaps or overlap");
            assert_eq!(s % 15, 0, "starts on a function");
            assert!(e - s < CHUNK_MAX);
            next = *e;
        }
        assert_eq!(next, 150);
        assert_eq!(pieces.len(), 2); // 75 + 75 lines

        // one 200-line function is split into line runs under the limit
        let long: Vec<String> = std::iter::once("fn big() {".to_string())
            .chain((0..198).map(|_| "    x();".to_string()))
            .chain(std::iter::once("}".to_string()))
            .collect();
        let long: Vec<&str> = long.iter().map(|s| s.as_str()).collect();
        for (s, e) in greedy_chunks(&long, CHUNK_MIN, CHUNK_MAX) {
            assert!(e - s <= CHUNK_MAX);
        }
    }

    #[test]
    fn finds_payment_code() {
        let mut index = Index {
            root: Some(PathBuf::from(".")),
            ..Default::default()
        };
        index.files.push(file_data(
            "src/billing/stripe.rs",
            "fn charge_card(amount: u64) { stripe_payment(amount) }",
            10,
            None,
        ));
        index.files.push(file_data(
            "src/ui/menu.rs",
            "fn draw_menu() { button(\"Open\") }",
            10,
            None,
        ));
        index.finish();
        let hits = index.search("where is the payment code", 3);
        assert_eq!(hits[0].rel, "src/billing/stripe.rs");
    }
}
