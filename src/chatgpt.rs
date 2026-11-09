//! Sign in with ChatGPT for the open-source desktop app.
//! OAuth credentials stay in memory. Only the non-secret client and host IDs
//! are saved, so a new browser sign-in is needed after restarting OpenCode.

use std::{
    fs,
    io::{Read, Write},
    net::TcpListener,
    sync::{Mutex, OnceLock},
    time::{Duration, Instant},
};

use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use eframe::egui;
use jsonwebtoken::{decode, decode_header, jwk::JwkSet, Algorithm, DecodingKey, Validation};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use url::Url;

use crate::settings::Settings;

const AUTH_URL: &str = "https://auth.openai.com/api/accounts/authorize";
const TOKEN_URL: &str = "https://auth.openai.com/api/accounts/oauth/token";
const RESOURCE: &str = "https://api.openai.com/v1";
const SCOPES: &str =
    "openid profile email offline_access resource.invoke chatgpt.tokens.use.direct";

#[derive(Default)]
struct State {
    busy: bool,
    status: String,
    session: Option<Session>,
    models: Vec<(String, String)>,
}

struct Session {
    client_id: String,
    access_token: String,
    refresh_token: String,
    expires_at: Instant,
}

#[derive(Default, Serialize, Deserialize)]
struct Client {
    host_id: String,
    client_id: Option<String>,
    subject: Option<String>,
}

static STATE: OnceLock<Mutex<State>> = OnceLock::new();

fn state() -> &'static Mutex<State> {
    STATE.get_or_init(|| Mutex::new(State::default()))
}

pub fn connected() -> bool {
    state()
        .try_lock()
        .map(|s| s.session.is_some())
        .unwrap_or(true)
}

pub fn status() -> (bool, String) {
    match state().try_lock() {
        Ok(s) => (
            s.busy,
            if s.status.is_empty() {
                "Sign in to use your ChatGPT plan.".into()
            } else {
                s.status.clone()
            },
        ),
        Err(_) => (true, "Refreshing ChatGPT session…".into()),
    }
}

pub fn models() -> Vec<(String, String)> {
    state()
        .try_lock()
        .map(|s| s.models.clone())
        .unwrap_or_default()
}

pub fn sign_in(ctx: egui::Context) {
    {
        let Ok(mut s) = state().lock() else { return };
        if s.busy {
            return;
        }
        s.busy = true;
        s.status = "Opening ChatGPT sign-in in your browser…".into();
    }
    std::thread::spawn(move || {
        let result = sign_in_inner();
        if let Ok(mut s) = state().lock() {
            s.busy = false;
            match result {
                Ok((session, models)) => {
                    s.session = Some(session);
                    s.models = models;
                    s.status = "Connected to ChatGPT plan for this app session.".into();
                }
                Err(error) => s.status = format!("ChatGPT sign-in failed: {error}"),
            }
        }
        ctx.request_repaint();
    });
}

fn client_path() -> std::path::PathBuf {
    Settings::path().with_file_name("chatgpt-client.json")
}

fn load_client() -> Result<Client, String> {
    let path = client_path();
    let mut client: Client = fs::read_to_string(&path)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default();
    if client.host_id.is_empty() {
        let mut b = [0u8; 16];
        getrandom::fill(&mut b).map_err(|e| e.to_string())?;
        b[6] = (b[6] & 0x0f) | 0x40;
        b[8] = (b[8] & 0x3f) | 0x80;
        client.host_id = format!("urn:uuid:{:02x}{:02x}{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}",
            b[0],b[1],b[2],b[3],b[4],b[5],b[6],b[7],b[8],b[9],b[10],b[11],b[12],b[13],b[14],b[15]);
        save_client(&client)?;
    }
    Ok(client)
}

fn save_client(client: &Client) -> Result<(), String> {
    let path = client_path();
    fs::create_dir_all(path.parent().ok_or("No settings folder")?).map_err(|e| e.to_string())?;
    let tmp = path.with_extension("tmp");
    fs::write(
        &tmp,
        serde_json::to_vec_pretty(client).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    fs::rename(tmp, path).map_err(|e| e.to_string())
}

fn random_urlsafe() -> Result<String, String> {
    let mut bytes = [0u8; 32];
    getrandom::fill(&mut bytes).map_err(|e| e.to_string())?;
    Ok(URL_SAFE_NO_PAD.encode(bytes))
}

fn sign_in_inner() -> Result<(Session, Vec<(String, String)>), String> {
    let mut client = load_client()?;
    let listener = TcpListener::bind("127.0.0.1:0").map_err(|e| e.to_string())?;
    listener.set_nonblocking(true).map_err(|e| e.to_string())?;
    let redirect = format!(
        "http://127.0.0.1:{}/auth/callback",
        listener.local_addr().map_err(|e| e.to_string())?.port()
    );
    let verifier = random_urlsafe()?;
    let challenge = URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()));
    let state_value = random_urlsafe()?;
    let nonce = random_urlsafe()?;
    let first_registration = client.client_id.is_none();
    let requested_client = client
        .client_id
        .as_deref()
        .unwrap_or("dynamic_agent_client");
    let mut authorize = Url::parse(AUTH_URL).map_err(|e| e.to_string())?;
    {
        let mut q = authorize.query_pairs_mut();
        q.append_pair("client_id", requested_client)
            .append_pair("response_type", "code")
            .append_pair("redirect_uri", &redirect)
            .append_pair("scope", SCOPES)
            .append_pair("resource", RESOURCE)
            .append_pair("state", &state_value)
            .append_pair("nonce", &nonce)
            .append_pair("code_challenge_method", "S256")
            .append_pair("code_challenge", &challenge)
            .append_pair("ext_agent_host_id", &client.host_id);
        if first_registration {
            q.append_pair("agent_name_hint", "OpenCode");
        }
    }
    webbrowser::open(authorize.as_str()).map_err(|e| format!("Could not open browser: {e}"))?;
    let start = Instant::now();
    let callback = loop {
        if start.elapsed() > Duration::from_secs(300) {
            return Err("Sign-in timed out after five minutes.".into());
        }
        match listener.accept() {
            Ok((mut socket, _)) => {
                socket.set_read_timeout(Some(Duration::from_secs(5))).ok();
                let mut bytes = [0u8; 8192];
                let n = socket.read(&mut bytes).map_err(|e| e.to_string())?;
                let request = std::str::from_utf8(&bytes[..n]).map_err(|e| e.to_string())?;
                let target = request
                    .lines()
                    .next()
                    .and_then(|l| l.split_whitespace().nth(1))
                    .ok_or("Invalid callback")?;
                let url =
                    Url::parse(&format!("http://127.0.0.1{target}")).map_err(|e| e.to_string())?;
                if url.path() != "/auth/callback" {
                    continue;
                }
                let pairs: std::collections::HashMap<String, String> =
                    url.query_pairs().into_owned().collect();
                let valid = pairs
                    .get("state")
                    .is_some_and(|value| value == &state_value);
                let body = if valid {
                    "You can return to OpenCode."
                } else {
                    "Invalid sign-in response. Return to OpenCode and try again."
                };
                let reply = format!("HTTP/1.1 200 OK\r\nContent-Type: text/plain; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}", body.len(), body);
                let _ = socket.write_all(reply.as_bytes());
                if !valid {
                    return Err("OAuth state did not match.".into());
                }
                break pairs;
            }
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                std::thread::sleep(Duration::from_millis(100))
            }
            Err(e) => return Err(e.to_string()),
        }
    };
    if let Some(error) = callback.get("error") {
        return Err(format!("Authorization declined: {error}"));
    }
    let code = callback
        .get("code")
        .ok_or("No authorization code returned")?;
    let issued_client = if first_registration {
        callback
            .get("client_id")
            .filter(|id| !id.is_empty() && *id != "dynamic_agent_client")
            .ok_or("No issued client ID returned")?
            .clone()
    } else {
        let known = client.client_id.as_ref().ok_or("Missing saved client ID")?;
        if callback.get("client_id").is_some_and(|id| id != known) {
            return Err("Client ID changed during sign-in.".into());
        }
        known.clone()
    };
    let agent = ureq::AgentBuilder::new()
        .timeout(Duration::from_secs(30))
        .build();
    let response = agent
        .post(TOKEN_URL)
        .send_form(&[
            ("grant_type", "authorization_code"),
            ("client_id", &issued_client),
            ("code", code),
            ("code_verifier", &verifier),
            ("redirect_uri", &redirect),
            ("resource", RESOURCE),
        ])
        .map_err(http_error)?;
    let tokens: Value = response.into_json().map_err(|e| e.to_string())?;
    let id_token = field(&tokens, "id_token")?;
    let subject = validate_id_token(id_token, &issued_client, &nonce)?;
    if client
        .subject
        .as_deref()
        .is_some_and(|known| known != subject)
    {
        return Err(
            "A different ChatGPT account answered this registration. Start a new sign-in.".into(),
        );
    }
    let scopes = field(&tokens, "scope")?;
    if !scopes
        .split_whitespace()
        .any(|s| s == "chatgpt.tokens.use.direct")
    {
        return Err("ChatGPT plan permission was not granted.".into());
    }
    let session = Session {
        client_id: issued_client.clone(),
        access_token: field(&tokens, "access_token")?.to_owned(),
        refresh_token: field(&tokens, "refresh_token")?.to_owned(),
        expires_at: Instant::now()
            + Duration::from_secs(tokens["expires_in"].as_u64().unwrap_or(3600)),
    };
    client.client_id = Some(issued_client);
    client.subject = Some(subject);
    save_client(&client)?;
    let models = list_models(&session.access_token).unwrap_or_default();
    Ok((session, models))
}

fn validate_id_token(token: &str, client_id: &str, nonce: &str) -> Result<String, String> {
    let header = decode_header(token).map_err(|e| e.to_string())?;
    if header.alg != Algorithm::RS256 {
        return Err("Unexpected ID token signing algorithm.".into());
    }
    let kid = header.kid.ok_or("ID token has no key ID")?;
    let agent = ureq::AgentBuilder::new()
        .timeout(Duration::from_secs(20))
        .build();
    let discovery: Value = agent
        .get("https://auth.openai.com/.well-known/openid-configuration")
        .call()
        .map_err(http_error)?
        .into_json()
        .map_err(|e| e.to_string())?;
    let jwks_uri = field(&discovery, "jwks_uri")?;
    if !jwks_uri.starts_with("https://auth.openai.com/") {
        return Err("Unexpected OpenAI key endpoint.".into());
    }
    let keys: JwkSet = agent
        .get(jwks_uri)
        .call()
        .map_err(http_error)?
        .into_json()
        .map_err(|e| e.to_string())?;
    let jwk = keys.find(&kid).ok_or("ID token signing key not found")?;
    let key = DecodingKey::from_jwk(jwk).map_err(|e| e.to_string())?;
    let mut validation = Validation::new(Algorithm::RS256);
    validation.set_issuer(&["https://auth.openai.com"]);
    validation.set_audience(&[client_id]);
    let claims = decode::<Value>(token, &key, &validation)
        .map_err(|e| e.to_string())?
        .claims;
    if claims["nonce"].as_str() != Some(nonce) {
        return Err("ID token nonce did not match.".into());
    }
    claims["sub"]
        .as_str()
        .filter(|s| !s.is_empty())
        .map(str::to_owned)
        .ok_or("ID token has no account subject.".into())
}

fn field<'a>(value: &'a Value, name: &str) -> Result<&'a str, String> {
    value[name]
        .as_str()
        .filter(|s| !s.is_empty())
        .ok_or_else(|| format!("Missing {name} in OpenAI response"))
}

fn http_error(error: ureq::Error) -> String {
    match error {
        ureq::Error::Status(code, response) => {
            let mut body = response.into_string().unwrap_or_default();
            body.truncate(300);
            format!("HTTP {code}: {body}")
        }
        other => other.to_string(),
    }
}

fn list_models(token: &str) -> Result<Vec<(String, String)>, String> {
    let agent = ureq::AgentBuilder::new()
        .timeout(Duration::from_secs(20))
        .build();
    let response: Value = agent
        .get("https://api.openai.com/v1/models")
        .set("Authorization", &format!("Bearer {token}"))
        .call()
        .map_err(http_error)?
        .into_json()
        .map_err(|e| e.to_string())?;
    Ok(response["models"]
        .as_array()
        .map(|array| {
            array
                .iter()
                .filter_map(|m| {
                    if m["visibility"] != "list" {
                        return None;
                    }
                    Some((
                        m["slug"].as_str()?.to_string(),
                        m["display_name"].as_str()?.to_string(),
                    ))
                })
                .collect()
        })
        .unwrap_or_default())
}

pub fn access_token() -> Result<String, String> {
    let mut s = state().lock().map_err(|e| e.to_string())?;
    let session = s
        .session
        .as_mut()
        .ok_or("Sign in with ChatGPT in Settings first.")?;
    if Instant::now() + Duration::from_secs(60) >= session.expires_at {
        let agent = ureq::AgentBuilder::new()
            .timeout(Duration::from_secs(30))
            .build();
        let response = agent
            .post(TOKEN_URL)
            .send_form(&[
                ("grant_type", "refresh_token"),
                ("client_id", &session.client_id),
                ("refresh_token", &session.refresh_token),
                ("resource", RESOURCE),
            ])
            .map_err(http_error)?;
        let tokens: Value = response.into_json().map_err(|e| e.to_string())?;
        if !field(&tokens, "scope")?
            .split_whitespace()
            .any(|x| x == "chatgpt.tokens.use.direct")
        {
            return Err("ChatGPT plan permission expired.".into());
        }
        session.access_token = field(&tokens, "access_token")?.to_owned();
        session.refresh_token = field(&tokens, "refresh_token")?.to_owned();
        session.expires_at =
            Instant::now() + Duration::from_secs(tokens["expires_in"].as_u64().unwrap_or(3600));
    }
    Ok(session.access_token.clone())
}
