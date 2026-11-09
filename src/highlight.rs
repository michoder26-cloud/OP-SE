//! A small, fast syntax colouring for the editor (keywords, types, strings,
//! numbers, comments), cached per text so typing stays smooth.

use eframe::egui::{
    self,
    text::{LayoutJob, TextFormat},
    util::cache::{ComputerMut, FrameCache},
    Color32, FontId,
};

use crate::theme;

#[derive(Clone, Copy, Hash, PartialEq, Eq)]
pub enum Lang {
    CLike,
    Rust,
    Hash,
    Sql,
    Plain,
}

pub fn lang_for(path: &str) -> Lang {
    let ext = path.rsplit('.').next().unwrap_or("").to_lowercase();
    match ext.as_str() {
        "rs" => Lang::Rust,
        "c" | "h" | "cpp" | "hpp" | "cc" | "js" | "jsx" | "ts" | "tsx" | "mjs" | "go" | "java"
        | "kt" | "cs" | "swift" | "dart" | "php" | "scala" | "css" | "scss" | "json" | "jsonc"
        | "zig" => Lang::CLike,
        "py" | "sh" | "bash" | "toml" | "yaml" | "yml" | "rb" | "ps1" | "r" | "conf" | "ini"
        | "dockerfile" | "makefile" | "cmake" | "env" => Lang::Hash,
        "sql" | "lua" | "hs" => Lang::Sql,
        _ => Lang::Plain,
    }
}

pub fn language_name(path: &str) -> &'static str {
    let ext = path.rsplit('.').next().unwrap_or("").to_lowercase();
    match ext.as_str() {
        "rs" => "Rust",
        "js" | "mjs" => "JavaScript",
        "jsx" => "JavaScript React",
        "ts" => "TypeScript",
        "tsx" => "TypeScript React",
        "py" => "Python",
        "go" => "Go",
        "java" => "Java",
        "c" | "h" => "C",
        "cpp" | "hpp" | "cc" => "C++",
        "cs" => "C#",
        "json" => "JSON",
        "toml" => "TOML",
        "yaml" | "yml" => "YAML",
        "md" => "Markdown",
        "html" => "HTML",
        "css" => "CSS",
        "sql" => "SQL",
        "sh" => "Shell",
        _ => "Plain Text",
    }
}

const KEYWORDS: &[&str] = &[
    "as",
    "async",
    "await",
    "break",
    "case",
    "catch",
    "class",
    "const",
    "continue",
    "crate",
    "def",
    "default",
    "defer",
    "del",
    "do",
    "elif",
    "else",
    "enum",
    "except",
    "export",
    "extends",
    "extern",
    "false",
    "finally",
    "fn",
    "for",
    "from",
    "func",
    "function",
    "go",
    "if",
    "impl",
    "import",
    "in",
    "interface",
    "is",
    "lambda",
    "let",
    "loop",
    "match",
    "mod",
    "move",
    "mut",
    "new",
    "nil",
    "None",
    "not",
    "null",
    "or",
    "and",
    "package",
    "pass",
    "private",
    "protected",
    "pub",
    "public",
    "raise",
    "ref",
    "return",
    "self",
    "Self",
    "static",
    "struct",
    "super",
    "switch",
    "this",
    "throw",
    "trait",
    "True",
    "False",
    "true",
    "try",
    "type",
    "typeof",
    "undefined",
    "unsafe",
    "use",
    "using",
    "var",
    "void",
    "where",
    "while",
    "with",
    "yield",
    "select",
    "insert",
    "update",
    "delete",
    "create",
    "table",
    "local",
    "then",
    "end",
    "dyn",
    "readonly",
    "override",
];

#[derive(Default)]
struct Highlighter;

impl ComputerMut<(&str, Lang, u32), LayoutJob> for Highlighter {
    fn compute(&mut self, (code, lang, size): (&str, Lang, u32)) -> LayoutJob {
        build(code, lang, size as f32 / 10.0)
    }
}

type Cache = FrameCache<LayoutJob, Highlighter>;

pub fn job(ctx: &egui::Context, code: &str, lang: Lang, font_size: f32) -> LayoutJob {
    ctx.memory_mut(|mem| {
        mem.caches
            .cache::<Cache>()
            .get((code, lang, (font_size * 10.0) as u32))
    })
}

fn build(code: &str, lang: Lang, size: f32) -> LayoutJob {
    let font = FontId::monospace(size);
    let mut job = LayoutJob::default();
    let fmt = |c: Color32| TextFormat {
        font_id: font.clone(),
        color: c,
        ..Default::default()
    };
    let bytes = code.as_bytes();
    let mut i = 0;
    let mut plain_start = 0;
    let flush = |job: &mut LayoutJob, from: usize, to: usize| {
        if to > from {
            job.append(&code[from..to], 0.0, fmt(theme::TEXT));
        }
    };
    while i < bytes.len() {
        let c = bytes[i];
        let rest = &code[i..];
        // comments
        let line_comment = match lang {
            Lang::CLike | Lang::Rust => rest.starts_with("//"),
            Lang::Hash => c == b'#',
            Lang::Sql => rest.starts_with("--"),
            Lang::Plain => false,
        };
        if line_comment {
            flush(&mut job, plain_start, i);
            let end = rest.find('\n').map(|e| i + e).unwrap_or(bytes.len());
            job.append(&code[i..end], 0.0, fmt(theme::COMMENT));
            i = end;
            plain_start = i;
            continue;
        }
        if matches!(lang, Lang::CLike | Lang::Rust) && rest.starts_with("/*") {
            flush(&mut job, plain_start, i);
            let end = rest[2..]
                .find("*/")
                .map(|e| i + e + 4)
                .unwrap_or(bytes.len());
            job.append(&code[i..end], 0.0, fmt(theme::COMMENT));
            i = end;
            plain_start = i;
            continue;
        }
        // strings
        if lang != Lang::Plain
            && (c == b'"'
                || c == b'`'
                || (c == b'\'' && (lang != Lang::Rust || is_char_literal(rest))))
        {
            flush(&mut job, plain_start, i);
            let mut j = i + 1;
            while j < bytes.len() && bytes[j] != c {
                if bytes[j] == b'\\' {
                    j += 1;
                }
                if bytes.get(j) == Some(&b'\n') && c != b'`' {
                    break;
                }
                j += 1;
            }
            let end = (j + 1).min(bytes.len());
            job.append(&code[i..end], 0.0, fmt(theme::STRING));
            i = end;
            plain_start = i;
            continue;
        }
        // words and numbers
        if c.is_ascii_alphanumeric() || c == b'_' {
            let prev_ok = i == 0 || !(bytes[i - 1].is_ascii_alphanumeric() || bytes[i - 1] == b'_');
            let mut j = i;
            while j < bytes.len() && (bytes[j].is_ascii_alphanumeric() || bytes[j] == b'_') {
                j += 1;
            }
            if prev_ok && lang != Lang::Plain {
                let word = &code[i..j];
                let color = if c.is_ascii_digit() {
                    Some(theme::NUMBER)
                } else if KEYWORDS.contains(&word) {
                    Some(theme::KEYWORD)
                } else if c.is_ascii_uppercase() && word.len() > 1 {
                    Some(theme::TYPE)
                } else if bytes.get(j) == Some(&b'(') || bytes.get(j) == Some(&b'!') {
                    Some(theme::FUNCTION)
                } else {
                    None
                };
                if let Some(color) = color {
                    flush(&mut job, plain_start, i);
                    job.append(word, 0.0, fmt(color));
                    plain_start = j;
                }
            }
            i = j;
            continue;
        }
        // step over one UTF-8 character
        i += utf8_len(c);
    }
    flush(&mut job, plain_start, bytes.len());
    job
}

fn is_char_literal(rest: &str) -> bool {
    let b = rest.as_bytes();
    (b.len() >= 3 && b[2] == b'\'' && b[1] != b'\\')
        || (b.len() >= 4 && b[1] == b'\\' && b[3] == b'\'')
}

fn utf8_len(first: u8) -> usize {
    match first {
        0x00..=0x7F => 1,
        0xC0..=0xDF => 2,
        0xE0..=0xEF => 3,
        0xF0..=0xFF => 4,
        _ => 1,
    }
}
