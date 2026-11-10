//! Agent chat history, kept per project in the user's settings folder so the
//! Agents home can list each repository's past chats.

use std::{
    collections::hash_map::DefaultHasher,
    fs,
    hash::{Hash, Hasher},
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use serde::{Deserialize, Serialize};

#[derive(Clone, Serialize, Deserialize)]
pub struct SavedMsg {
    pub user: bool,
    pub text: String,
    pub mode: String,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct SavedChat {
    pub title: String,
    pub updated: u64,
    pub messages: Vec<SavedMsg>,
}

pub fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn file_for(root: &Path) -> PathBuf {
    let mut h = DefaultHasher::new();
    root.to_string_lossy().to_lowercase().hash(&mut h);
    crate::settings::Settings::path()
        .with_file_name("chats")
        .join(format!("{:016x}.json", h.finish()))
}

pub fn load(root: &Path) -> Vec<SavedChat> {
    let path = file_for(root);
    let legacy = path
        .parent()
        .and_then(|p| p.parent())
        .and_then(|p| p.parent())
        .map(|p| {
            p.join("OpenCode")
                .join("chats")
                .join(path.file_name().unwrap_or_default())
        });
    fs::read_to_string(&path)
        .or_else(|_| {
            legacy.map(fs::read_to_string).unwrap_or_else(|| {
                Err(std::io::Error::new(
                    std::io::ErrorKind::NotFound,
                    "No legacy chat history",
                ))
            })
        })
        .ok()
        .and_then(|t| serde_json::from_str(&t).ok())
        .unwrap_or_default()
}

pub fn save(root: &Path, chats: &[SavedChat]) {
    let path = file_for(root);
    if let Some(dir) = path.parent() {
        let _ = fs::create_dir_all(dir);
    }
    if let Ok(text) = serde_json::to_string(chats) {
        let _ = fs::write(path, text);
    }
}

/// "now", "12m", "3h", "5d", "4mo", "2y" — how long ago, as the original's list shows it.
pub fn age(updated: u64) -> String {
    let s = now().saturating_sub(updated);
    match s {
        0..=59 => "now".into(),
        60..=3599 => format!("{}m", s / 60),
        3600..=86_399 => format!("{}h", s / 3600),
        86_400..=2_591_999 => format!("{}d", s / 86_400),
        2_592_000..=31_535_999 => format!("{}mo", s / 2_592_000),
        _ => format!("{}y", s / 31_536_000),
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn ages() {
        let n = super::now();
        assert_eq!(super::age(n), "now");
        assert_eq!(super::age(n - 3 * 3600), "3h");
        assert_eq!(super::age(n - 130 * 86_400), "4mo");
    }
}
