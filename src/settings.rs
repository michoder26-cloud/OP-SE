//! User settings, stored in the user's own config folder (never in the workspace).

use std::{fs, path::PathBuf};

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Debug)]
pub enum Provider {
    ChatGpt,
    OpenAi,
    Anthropic,
    OpenRouter,
    Ollama,
    Custom,
}

impl Provider {
    pub const ALL: [Provider; 6] = [
        Provider::ChatGpt,
        Provider::OpenAi,
        Provider::Anthropic,
        Provider::OpenRouter,
        Provider::Ollama,
        Provider::Custom,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Self::ChatGpt => "ChatGPT subscription",
            Self::OpenAi => "OpenAI",
            Self::Anthropic => "Anthropic",
            Self::OpenRouter => "OpenRouter",
            Self::Ollama => "Ollama (local)",
            Self::Custom => "Custom (OpenAI-compatible)",
        }
    }

    pub fn default_base_url(self) -> &'static str {
        match self {
            Self::ChatGpt => "https://api.openai.com/v1",
            Self::OpenAi => "https://api.openai.com/v1",
            Self::Anthropic => "https://api.anthropic.com/v1",
            Self::OpenRouter => "https://openrouter.ai/api/v1",
            Self::Ollama => "http://localhost:11434/v1",
            Self::Custom => "http://localhost:1234/v1",
        }
    }

    pub fn default_model(self) -> &'static str {
        match self {
            Self::ChatGpt => "gpt-6.1-sol",
            Self::OpenAi => "gpt-4.1-mini",
            Self::Anthropic => "claude-sonnet-5-5",
            Self::OpenRouter => "openai/gpt-4.1-mini",
            Self::Ollama => "qwen2.5-coder:7b",
            Self::Custom => "local-model",
        }
    }

    pub fn needs_key(self) -> bool {
        !matches!(self, Self::ChatGpt | Self::Ollama | Self::Custom)
    }
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub provider: Provider,
    pub base_url: String,
    pub model: String,
    pub api_key: String,
    pub last_workspace: Option<PathBuf>,
    pub index_on_open: bool,
    pub auto_codebase_context: bool,
    pub include_current_file: bool,
    pub font_size: f32,
    pub recent_workspaces: Vec<PathBuf>,
    pub start_in_agents: bool,
}

impl Default for Settings {
    fn default() -> Self {
        let provider = Provider::OpenAi;
        Self {
            provider,
            base_url: provider.default_base_url().into(),
            model: provider.default_model().into(),
            api_key: String::new(),
            last_workspace: None,
            index_on_open: true,
            auto_codebase_context: true,
            include_current_file: true,
            font_size: 13.5,
            recent_workspaces: Vec::new(),
            start_in_agents: true,
        }
    }
}

impl Settings {
    pub fn path() -> PathBuf {
        let base = std::env::var_os("APPDATA")
            .map(PathBuf::from)
            .or_else(|| std::env::var_os("XDG_CONFIG_HOME").map(PathBuf::from))
            .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))
            .unwrap_or_else(|| PathBuf::from("."));
        base.join("OP-SE").join("settings.json")
    }

    pub fn load() -> Self {
        let path = Self::path();
        let legacy = path
            .parent()
            .and_then(|p| p.parent())
            .map(|p| p.join("OpenCode").join("settings.json"));
        fs::read_to_string(&path)
            .or_else(|_| {
                legacy.map(fs::read_to_string).unwrap_or_else(|| {
                    Err(std::io::Error::new(
                        std::io::ErrorKind::NotFound,
                        "No legacy settings",
                    ))
                })
            })
            .ok()
            .and_then(|text| serde_json::from_str(&text).ok())
            .unwrap_or_default()
    }

    pub fn save(&self) -> Result<(), String> {
        let path = Self::path();
        if let Some(dir) = path.parent() {
            fs::create_dir_all(dir).map_err(|e| e.to_string())?;
        }
        let text = serde_json::to_string_pretty(self).map_err(|e| e.to_string())?;
        fs::write(path, text).map_err(|e| e.to_string())
    }

    pub fn ready(&self) -> bool {
        !self.model.trim().is_empty()
            && !self.base_url.trim().is_empty()
            && (self.provider != Provider::ChatGpt || crate::chatgpt::connected())
            && (!self.provider.needs_key() || !self.api_key.trim().is_empty())
    }
}
