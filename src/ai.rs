//! Talking to an AI model: OpenAI-compatible chat completions (OpenAI, OpenRouter,
//! Ollama, LM Studio...) and Anthropic messages, both streamed, on a background thread.

use std::{
    io::{BufRead, BufReader},
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc::Sender,
        Arc, RwLock,
    },
    time::Duration,
};

use serde_json::{json, Value};

use crate::index::Index;
use crate::settings::{Provider, Settings};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Role {
    User,
    Assistant,
}

#[derive(Clone, Debug)]
pub struct Turn {
    pub role: Role,
    pub content: String,
}

#[derive(Debug)]
pub enum AiEvent {
    Delta(String),
    Status(String),
    /// Files and lines the codebase search attached.
    Sources(Vec<String>),
    /// Keywords the model turned a question into.
    Keywords(String),
    Done,
    Error(String),
}

pub type Events = Sender<(u64, AiEvent)>;

/// What a request wants done before the model is asked.
pub struct Request {
    pub job: u64,
    pub settings: Settings,
    pub system: String,
    pub turns: Vec<Turn>,
    /// Search the codebase with this question first and add the hits to the last turn.
    pub codebase: Option<(String, Arc<RwLock<Index>>, usize)>,
    pub cancel: Arc<AtomicBool>,
}

pub fn spawn(request: Request, tx: Events, ctx: eframe::egui::Context) {
    std::thread::spawn(move || {
        let job = request.job;
        let send = |event: AiEvent| {
            let _ = tx.send((job, event));
            ctx.request_repaint();
        };
        match run(request, &send) {
            Ok(()) => send(AiEvent::Done),
            Err(error) => send(AiEvent::Error(error)),
        }
    });
}

fn run(mut request: Request, send: &dyn Fn(AiEvent)) -> Result<(), String> {
    if let Some((question, index, hits)) = request.codebase.take() {
        send(AiEvent::Status("Searching codebase…".into()));
        let keywords = keywords_for(&request.settings, &question, &request.cancel)
            .unwrap_or_else(|_| question.clone());
        send(AiEvent::Keywords(keywords.clone()));
        let query = format!("{question} {keywords}");
        let results = index
            .read()
            .map_err(|_| "index is busy".to_string())?
            .search(&query, hits);
        if !results.is_empty() {
            let mut context = String::from("\n\nRelevant code found by searching the codebase:\n");
            let mut sources = Vec::new();
            for hit in &results {
                sources.push(format!("{}:{}-{}", hit.rel, hit.start_line, hit.end_line));
                context.push_str(&format!(
                    "\n--- {} (lines {}-{}) ---\n{}\n",
                    hit.rel, hit.start_line, hit.end_line, hit.snippet
                ));
            }
            send(AiEvent::Sources(sources));
            if let Some(last) = request.turns.last_mut() {
                last.content.push_str(&context);
            }
        } else {
            send(AiEvent::Sources(Vec::new()));
        }
        send(AiEvent::Status("Thinking…".into()));
    }
    stream(
        &request.settings,
        &request.system,
        &request.turns,
        &request.cancel,
        &mut |delta| send(AiEvent::Delta(delta.to_string())),
    )
}

/// Ask the model for English code-search keywords (so Thai questions find English code).
pub fn keywords_for(
    settings: &Settings,
    question: &str,
    cancel: &AtomicBool,
) -> Result<String, String> {
    let system = "You turn a question about a codebase into search keywords. Reply with 6 to 12 \
                  English words separated by spaces: likely identifiers, function names, file names \
                  and technical terms (for example: payment checkout stripe charge invoice). \
                  No sentences, no punctuation.";
    let mut out = String::new();
    stream(
        settings,
        system,
        &[Turn {
            role: Role::User,
            content: question.to_string(),
        }],
        cancel,
        &mut |delta| out.push_str(delta),
    )?;
    Ok(out.replace(['\n', ','], " ").trim().to_string())
}

/// Stream one completion, calling `on_delta` with each piece of text.
pub fn stream(
    settings: &Settings,
    system: &str,
    turns: &[Turn],
    cancel: &AtomicBool,
    on_delta: &mut dyn FnMut(&str),
) -> Result<(), String> {
    if !settings.ready() {
        if settings.provider == Provider::ChatGpt {
            return Err("Sign in with ChatGPT in Settings (Ctrl+,) first.".into());
        }
        return Err(format!(
            "No AI model is set up yet. Open Settings (Ctrl+,) and add a {} key, or pick Ollama to run a model on this computer.",
            settings.provider.name()
        ));
    }
    if settings.provider == Provider::ChatGpt {
        return stream_chatgpt(settings, system, turns, cancel, on_delta);
    }
    let base = settings.base_url.trim().trim_end_matches('/');
    let agent = ureq::AgentBuilder::new()
        .timeout_connect(Duration::from_secs(20))
        .timeout_read(Duration::from_secs(180))
        .build();
    let anthropic = settings.provider == Provider::Anthropic;
    let response = if anthropic {
        let messages: Vec<Value> = turns
            .iter()
            .map(|t| json!({"role": role_name(t.role), "content": t.content}))
            .collect();
        agent
            .post(&format!("{base}/messages"))
            .set("x-api-key", settings.api_key.trim())
            .set("anthropic-version", "2023-06-01")
            .set("content-type", "application/json")
            .send_json(json!({
                "model": settings.model.trim(),
                "max_tokens": 8192,
                "system": system,
                "messages": messages,
                "stream": true,
            }))
    } else {
        let mut messages = vec![json!({"role": "system", "content": system})];
        messages.extend(
            turns
                .iter()
                .map(|t| json!({"role": role_name(t.role), "content": t.content})),
        );
        let mut call = agent
            .post(&format!("{base}/chat/completions"))
            .set("content-type", "application/json");
        if !settings.api_key.trim().is_empty() {
            call = call.set(
                "Authorization",
                &format!("Bearer {}", settings.api_key.trim()),
            );
        }
        if settings.provider == Provider::OpenRouter {
            call = call.set("X-Title", "OP-SE");
        }
        call.send_json(json!({
            "model": settings.model.trim(),
            "messages": messages,
            "stream": true,
        }))
    };
    let response = match response {
        Ok(response) => response,
        Err(ureq::Error::Status(code, response)) => {
            let body = response.into_string().unwrap_or_default();
            return Err(format!(
                "The AI service answered {code}: {}",
                error_text(&body)
            ));
        }
        Err(error) => return Err(format!("Could not reach the AI service: {error}")),
    };
    let reader = BufReader::new(response.into_reader());
    for line in reader.lines() {
        if cancel.load(Ordering::Relaxed) {
            return Err("Stopped.".into());
        }
        let line = line.map_err(|e| format!("Connection dropped: {e}"))?;
        let Some(data) = line.strip_prefix("data:") else {
            continue;
        };
        let data = data.trim();
        if data == "[DONE]" {
            break;
        }
        let Ok(value) = serde_json::from_str::<Value>(data) else {
            continue;
        };
        if let Some(message) = value.get("error") {
            return Err(error_text(&message.to_string()));
        }
        let piece = if anthropic {
            if value["type"] == "content_block_delta" {
                value["delta"]["text"].as_str()
            } else {
                None
            }
        } else {
            value["choices"][0]["delta"]["content"].as_str()
        };
        if let Some(piece) = piece {
            on_delta(piece);
        }
    }
    Ok(())
}

fn stream_chatgpt(
    settings: &Settings,
    system: &str,
    turns: &[Turn],
    cancel: &AtomicBool,
    on_delta: &mut dyn FnMut(&str),
) -> Result<(), String> {
    let token = crate::chatgpt::access_token()?;
    let mut input = vec![json!({"role": "developer", "content": system})];
    input.extend(
        turns
            .iter()
            .map(|t| json!({"role": role_name(t.role), "content": t.content})),
    );
    let agent = ureq::AgentBuilder::new()
        .timeout_connect(Duration::from_secs(20))
        .timeout_read(Duration::from_secs(180))
        .build();
    let response = agent
        .post("https://api.openai.com/v1/responses")
        .set("Authorization", &format!("Bearer {token}"))
        .set("Content-Type", "application/json")
        .send_json(
            json!({"model": settings.model.trim(), "input": input, "store": false, "stream": true}),
        );
    let response = match response {
        Ok(response) => response,
        Err(ureq::Error::Status(code, response)) => {
            let body = response.into_string().unwrap_or_default();
            return Err(format!(
                "ChatGPT plan answered {code}: {}",
                error_text(&body)
            ));
        }
        Err(e) => return Err(format!("Could not reach ChatGPT plan: {e}")),
    };
    let mut completed = false;
    for line in BufReader::new(response.into_reader()).lines() {
        if cancel.load(Ordering::Relaxed) {
            return Err("Stopped.".into());
        }
        let line = line.map_err(|e| format!("Connection dropped: {e}"))?;
        let Some(data) = line.strip_prefix("data:") else {
            continue;
        };
        let Ok(event) = serde_json::from_str::<Value>(data.trim()) else {
            continue;
        };
        match event["type"].as_str() {
            Some("response.output_text.delta") => {
                if let Some(delta) = event["delta"].as_str() {
                    on_delta(delta);
                }
            }
            Some("response.completed") => {
                completed = true;
                break;
            }
            Some("response.failed" | "response.incomplete" | "error") => {
                return Err(format!(
                    "ChatGPT plan response failed: {}",
                    error_text(&event.to_string())
                ));
            }
            _ => {}
        }
    }
    if !completed {
        return Err("ChatGPT plan stream ended without response.completed.".into());
    }
    Ok(())
}

fn role_name(role: Role) -> &'static str {
    match role {
        Role::User => "user",
        Role::Assistant => "assistant",
    }
}

fn error_text(body: &str) -> String {
    let parsed: Option<Value> = serde_json::from_str(body).ok();
    if let Some(value) = parsed {
        for pointer in ["/error/message", "/message", "/error"] {
            if let Some(text) = value.pointer(pointer).and_then(|v| v.as_str()) {
                return text.to_string();
            }
        }
    }
    let mut text = body.trim().to_string();
    text.truncate(400);
    if text.is_empty() {
        "no details".into()
    } else {
        text
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Write};
    use std::net::TcpListener;

    /// A one-shot local server that answers with a recorded stream.
    fn serve(body: &'static str) -> String {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        std::thread::spawn(move || {
            let (mut sock, _) = listener.accept().unwrap();
            sock.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
            let mut request = Vec::new();
            let mut buf = [0u8; 4096];
            loop {
                let n = sock.read(&mut buf).unwrap();
                if n == 0 {
                    break;
                }
                request.extend_from_slice(&buf[..n]);
                if let Some(headers_end) = request.windows(4).position(|w| w == b"\r\n\r\n") {
                    let headers = String::from_utf8_lossy(&request[..headers_end]);
                    let length = headers
                        .lines()
                        .find_map(|line| {
                            line.to_ascii_lowercase()
                                .strip_prefix("content-length:")
                                .and_then(|value| value.trim().parse::<usize>().ok())
                        })
                        .unwrap_or(0);
                    if request.len() >= headers_end + 4 + length {
                        break;
                    }
                }
            }
            let reply = format!(
                "HTTP/1.1 200 OK\r\ncontent-type: text/event-stream\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{}",
                body.len(),
                body
            );
            sock.write_all(reply.as_bytes()).unwrap();
        });
        format!("http://127.0.0.1:{port}/v1")
    }

    fn collect(settings: &Settings) -> Result<String, String> {
        let mut out = String::new();
        stream(
            settings,
            "system",
            &[Turn {
                role: Role::User,
                content: "hi".into(),
            }],
            &AtomicBool::new(false),
            &mut |d| out.push_str(d),
        )?;
        Ok(out)
    }

    #[test]
    fn streams_openai_compatible() {
        let base = serve("data: {\"choices\":[{\"delta\":{\"content\":\"สวัสดี \"}}]}\n\ndata: {\"choices\":[{\"delta\":{\"content\":\"world\"}}]}\n\ndata: [DONE]\n\n");
        let settings = Settings {
            provider: Provider::Custom,
            base_url: base,
            model: "m".into(),
            ..Default::default()
        };
        assert_eq!(collect(&settings).unwrap(), "สวัสดี world");
    }

    #[test]
    fn streams_anthropic() {
        let base = serve("event: content_block_delta\ndata: {\"type\":\"content_block_delta\",\"delta\":{\"type\":\"text_delta\",\"text\":\"Hello\"}}\n\nevent: message_stop\ndata: {\"type\":\"message_stop\"}\n\n");
        let settings = Settings {
            provider: Provider::Anthropic,
            base_url: base,
            model: "m".into(),
            api_key: "k".into(),
            ..Default::default()
        };
        assert_eq!(collect(&settings).unwrap(), "Hello");
    }

    #[test]
    fn needs_a_key() {
        let settings = Settings {
            provider: Provider::OpenAi,
            api_key: String::new(),
            ..Default::default()
        };
        assert!(collect(&settings).unwrap_err().contains("Settings"));
    }
}
