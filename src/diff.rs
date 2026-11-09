//! Line diffs for reviewing AI edits, and parsing the agent's multi-file answer.

use similar::{ChangeTag, TextDiff};

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Same,
    Added,
    Removed,
}

#[derive(Clone)]
pub struct Line {
    pub kind: Kind,
    pub text: String,
    pub old_no: Option<usize>,
    pub new_no: Option<usize>,
}

pub fn lines(old: &str, new: &str) -> Vec<Line> {
    let diff = TextDiff::from_lines(old, new);
    diff.iter_all_changes()
        .map(|change| Line {
            kind: match change.tag() {
                ChangeTag::Equal => Kind::Same,
                ChangeTag::Insert => Kind::Added,
                ChangeTag::Delete => Kind::Removed,
            },
            text: change.value().trim_end_matches(['\n', '\r']).to_string(),
            old_no: change.old_index().map(|i| i + 1),
            new_no: change.new_index().map(|i| i + 1),
        })
        .collect()
}

pub fn counts(old: &str, new: &str) -> (usize, usize) {
    let mut added = 0;
    let mut removed = 0;
    for change in TextDiff::from_lines(old, new).iter_all_changes() {
        match change.tag() {
            ChangeTag::Insert => added += 1,
            ChangeTag::Delete => removed += 1,
            ChangeTag::Equal => {}
        }
    }
    (added, removed)
}

/// One file change the agent proposed.
#[derive(Clone, Debug, PartialEq)]
pub struct FileChange {
    pub rel: String,
    /// None means delete the file.
    pub content: Option<String>,
}

/// Parse `<<<FILE path` … `>>>END` and `<<<DELETE path>>>` blocks; returns the
/// remaining prose and the changes.
pub fn parse_agent_answer(answer: &str) -> (String, Vec<FileChange>) {
    let mut prose = String::new();
    let mut changes = Vec::new();
    let mut current: Option<(String, Vec<&str>)> = None;
    for line in answer.split_inclusive('\n') {
        let trimmed = line.trim();
        if let Some((rel, body)) = current.as_mut() {
            if trimmed == ">>>END" || trimmed == ">>> END" {
                let content = strip_fence(&body.concat());
                changes.push(FileChange {
                    rel: clean_rel(rel),
                    content: Some(content),
                });
                prose.push_str(&format!("[[file:{}]]\n", clean_rel(rel)));
                current = None;
            } else {
                body.push(line);
            }
            continue;
        }
        if let Some(rest) = trimmed.strip_prefix("<<<DELETE") {
            let rel = rest.trim().trim_end_matches(">>>").trim();
            if !rel.is_empty() {
                changes.push(FileChange {
                    rel: clean_rel(rel),
                    content: None,
                });
                prose.push_str(&format!("[[file:{}]]\n", clean_rel(rel)));
            }
        } else if let Some(rest) = trimmed.strip_prefix("<<<FILE") {
            let rel = rest
                .trim()
                .trim_start_matches(':')
                .trim()
                .trim_end_matches(">>>")
                .trim();
            current = Some((rel.to_string(), Vec::new()));
        } else {
            prose.push_str(line);
        }
    }
    // An unfinished block while streaming: keep it out of the prose.
    if let Some((rel, _)) = current {
        prose.push_str(&format!("[[writing:{}]]\n", clean_rel(&rel)));
    }
    (prose, changes)
}

fn clean_rel(rel: &str) -> String {
    rel.trim()
        .trim_matches('`')
        .trim_start_matches("./")
        .replace('\\', "/")
}

/// Remove a surrounding ``` fence the model sometimes adds.
pub fn strip_fence(text: &str) -> String {
    let trimmed = text.trim_matches(['\n', '\r']);
    let mut lines: Vec<&str> = trimmed.lines().collect();
    if lines
        .first()
        .map(|l| l.trim_start().starts_with("```"))
        .unwrap_or(false)
        && lines.last().map(|l| l.trim() == "```").unwrap_or(false)
        && lines.len() >= 2
    {
        lines.remove(0);
        lines.pop();
        let mut out = lines.join("\n");
        out.push('\n');
        return out;
    }
    let mut out = trimmed.to_string();
    out.push('\n');
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_blocks() {
        let answer = "I changed two files.\n<<<FILE src/a.rs\n```rust\nfn a() {}\n```\n>>>END\n<<<DELETE old.txt>>>\nDone.";
        let (prose, changes) = parse_agent_answer(answer);
        assert_eq!(changes.len(), 2);
        assert_eq!(changes[0].content.as_deref(), Some("fn a() {}\n"));
        assert_eq!(
            changes[1],
            FileChange {
                rel: "old.txt".into(),
                content: None
            }
        );
        assert!(prose.contains("[[file:src/a.rs]]"));
    }
}
