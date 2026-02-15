use crate::integrations::integrations::{EmailMessage, EmailSettings};
use anyhow::{Context, Result};
use oauth2::basic::BasicClient;
use oauth2::reqwest::async_http_client;
use oauth2::{
    AuthUrl, AuthorizationCode, ClientId, ClientSecret, CsrfToken, RedirectUrl, RefreshToken,
    Scope, TokenResponse, TokenUrl,
};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::time::Duration;
use tiny_http::{Response, Server};
use url::Url;

const GMAIL_AUTH_URL: &str = "https://accounts.google.com/o/oauth2/v2/auth";
const GMAIL_TOKEN_URL: &str = "https://oauth2.googleapis.com/token";
const GMAIL_SCOPE_MODIFY: &str = "https://www.googleapis.com/auth/gmail.modify";
const RESOURCE_SECRET_PATH: &str = "resources/gmail_client_secret.json";
const DEFAULT_REDIRECT_URI: &str = "http://127.0.0.1:42813/callback";
const FALLBACK_REDIRECT_URI: &str = "http://localhost";

#[derive(Debug, Deserialize)]
struct InstalledClientSecrets {
    installed: InstalledClientFields,
}

#[derive(Debug, Deserialize)]
struct InstalledClientFields {
    client_id: String,
    client_secret: String,
    redirect_uris: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct EmailTokenStore {
    access_token: String,
    refresh_token: String,
    expires_at: Option<u64>,
}

pub enum TokenStorage {
    Keychain,
    File,
}

fn token_storage_for_platform() -> TokenStorage {
    if cfg!(target_os = "macos") {
        TokenStorage::Keychain
    } else {
        TokenStorage::File
    }
}

fn token_file_path() -> PathBuf {
    let mut path = dirs::config_dir().unwrap_or_else(|| PathBuf::from("."));
    path.push("os-ghost");
    path.push("email_tokens.json");
    path
}

fn read_client_secrets() -> Result<(String, String, String)> {
    if let (Ok(client_id), Ok(client_secret)) = (
        std::env::var("OS_GHOST_GMAIL_CLIENT_ID"),
        std::env::var("OS_GHOST_GMAIL_CLIENT_SECRET"),
    ) {
        let redirect_uri = std::env::var("OS_GHOST_GMAIL_REDIRECT_URI")
            .unwrap_or_else(|_| DEFAULT_REDIRECT_URI.to_string());
        return Ok((client_id, client_secret, redirect_uri));
    }

    let env_path = std::env::var("OS_GHOST_GMAIL_CLIENT_SECRET_JSON").ok();
    let candidates = [env_path.as_deref(), Some(RESOURCE_SECRET_PATH)];

    let mut contents = None;
    let mut last_path = String::new();
    for candidate in candidates.iter().flatten() {
        let resolved = resolve_resource_path(candidate);
        last_path = resolved.to_string_lossy().to_string();
        if resolved.exists() {
            if let Ok(data) = fs::read_to_string(&resolved) {
                contents = Some(data);
                break;
            }
        }
    }

    let contents =
        contents.with_context(|| format!("Failed to read client secrets at {}", last_path))?;
    let parsed: InstalledClientSecrets = serde_json::from_str(&contents)?;
    let redirect_uri = parsed
        .installed
        .redirect_uris
        .first()
        .cloned()
        .unwrap_or_else(|| DEFAULT_REDIRECT_URI.to_string());
    Ok((
        parsed.installed.client_id,
        parsed.installed.client_secret,
        redirect_uri,
    ))
}

fn resolve_resource_path(candidate: &str) -> PathBuf {
    let path = PathBuf::from(candidate);
    if path.is_absolute() {
        return path;
    }

    // Try relative to current directory first
    let relative_path = Path::new(candidate);
    if relative_path.exists() {
        return relative_path.to_path_buf();
    }

    // Try in config directory
    if let Some(mut config_dir) = dirs::config_dir() {
        config_dir.push("os-ghost");
        config_dir.push(candidate);
        if config_dir.exists() {
            return config_dir;
        }
    }

    Path::new(candidate).to_path_buf()
}

fn save_tokens(tokens: &EmailTokenStore) -> Result<()> {
    match token_storage_for_platform() {
        TokenStorage::Keychain => {
            let entry = keyring::Entry::new("os-ghost", "gmail_tokens")?;
            let serialized = serde_json::to_string(tokens)?;
            entry.set_password(&serialized)?;
        }
        TokenStorage::File => {
            let path = token_file_path();
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent)?;
            }
            let contents = serde_json::to_string_pretty(tokens)?;
            fs::write(path, contents)?;
        }
    }
    Ok(())
}

fn load_tokens() -> Result<Option<EmailTokenStore>> {
    match token_storage_for_platform() {
        TokenStorage::Keychain => {
            let entry = keyring::Entry::new("os-ghost", "gmail_tokens")?;
            match entry.get_password() {
                Ok(contents) => Ok(Some(serde_json::from_str(&contents)?)),
                Err(_) => Ok(None),
            }
        }
        TokenStorage::File => {
            let path = token_file_path();
            if !path.exists() {
                return Ok(None);
            }
            let contents = fs::read_to_string(path)?;
            Ok(Some(serde_json::from_str(&contents)?))
        }
    }
}

fn clear_tokens() -> Result<()> {
    match token_storage_for_platform() {
        TokenStorage::Keychain => {
            let entry = keyring::Entry::new("os-ghost", "gmail_tokens")?;
            let _ = entry.delete_password();
        }
        TokenStorage::File => {
            let path = token_file_path();
            if path.exists() {
                let _ = fs::remove_file(path);
            }
        }
    }
    Ok(())
}

fn build_oauth_client() -> Result<(BasicClient, String)> {
    let (client_id, client_secret, redirect_uri) = read_client_secrets()?;
    let client = BasicClient::new(
        ClientId::new(client_id),
        Some(ClientSecret::new(client_secret)),
        AuthUrl::new(GMAIL_AUTH_URL.to_string())?,
        Some(TokenUrl::new(GMAIL_TOKEN_URL.to_string())?),
    )
    .set_redirect_uri(RedirectUrl::new(redirect_uri.clone())?);
    Ok((client, redirect_uri))
}

fn start_oauth_listener(redirect_uri: &str) -> Result<(mpsc::Receiver<String>, u16)> {
    let url = Url::parse(redirect_uri)
        .or_else(|_| Url::parse(DEFAULT_REDIRECT_URI))
        .or_else(|_| Url::parse(FALLBACK_REDIRECT_URI))?;
    let port = url.port_or_known_default().unwrap_or(42813);
    let server = Server::http(("127.0.0.1", port))
        .map_err(|e| anyhow::anyhow!("Failed to start OAuth listener: {}", e))?;
    let (tx, rx) = mpsc::channel();

    std::thread::spawn(move || {
        if let Ok(Some(request)) = server.recv_timeout(Duration::from_secs(120)) {
            let url = format!("http://127.0.0.1:{}{}", port, request.url());
            let code = Url::parse(&url).ok().and_then(|u| {
                u.query_pairs()
                    .find(|(k, _)| k == "code")
                    .map(|(_, v)| v.to_string())
            });

            let response =
                Response::from_string("Authorization complete. You can close this window.")
                    .with_status_code(200);
            let _ = request.respond(response);

            if let Some(code) = code {
                let _ = tx.send(code);
            }
        }
    });

    Ok((rx, port))
}

async fn exchange_code(code: String, client: &BasicClient) -> Result<EmailTokenStore> {
    let token_result = client
        .exchange_code(AuthorizationCode::new(code))
        .request_async(async_http_client)
        .await?;

    let access = token_result.access_token().secret().to_string();
    let refresh = token_result
        .refresh_token()
        .map(|t| t.secret().to_string())
        .context("Missing refresh token")?;

    Ok(EmailTokenStore {
        access_token: access,
        refresh_token: refresh,
        expires_at: None,
    })
}

async fn refresh_access_token(client: &BasicClient, refresh: &str) -> Result<EmailTokenStore> {
    let token_result = client
        .exchange_refresh_token(&RefreshToken::new(refresh.to_string()))
        .request_async(async_http_client)
        .await?;

    let access = token_result.access_token().secret().to_string();
    let refresh_token = token_result
        .refresh_token()
        .map(|t| t.secret().to_string())
        .unwrap_or_else(|| refresh.to_string());

    Ok(EmailTokenStore {
        access_token: access,
        refresh_token,
        expires_at: None,
    })
}

async fn gmail_api_get(path: &str, access_token: &str) -> Result<serde_json::Value> {
    let url = format!("https://gmail.googleapis.com/gmail/v1/{}", path);
    let client = reqwest::Client::new();
    let resp = client.get(&url).bearer_auth(access_token).send().await?;

    let status = resp.status();
    let body = resp.text().await?;
    if !status.is_success() {
        return Err(anyhow::anyhow!("Gmail API error: {}", body));
    }
    Ok(serde_json::from_str(&body)?)
}

fn update_sync_timestamp() {
    let mut settings = EmailSettings::load();
    settings.last_sync_at = Some(crate::core::utils::current_timestamp());
    let _ = settings.save();
}

async fn gmail_api_post(path: &str, access_token: &str, payload: serde_json::Value) -> Result<()> {
    let url = format!("https://gmail.googleapis.com/gmail/v1/{}", path);
    let client = reqwest::Client::new();
    let resp = client
        .post(&url)
        .bearer_auth(access_token)
        .json(&payload)
        .send()
        .await?;

    let status = resp.status();
    let body = resp.text().await?;
    if !status.is_success() {
        return Err(anyhow::anyhow!("Gmail API error: {}", body));
    }
    Ok(())
}

pub async fn begin_oauth() -> Result<EmailSettings> {
    let (client, redirect_uri) = build_oauth_client()?;
    let (auth_url, _csrf_token) = client
        .authorize_url(CsrfToken::new_random)
        .add_scope(Scope::new(GMAIL_SCOPE_MODIFY.to_string()))
        .url();

    let (rx, _port) = start_oauth_listener(&redirect_uri)?;

    crate::data::events_bus::record_event(
        crate::data::events_bus::EventKind::Action,
        "Gmail OAuth flow started".to_string(),
        None,
        std::collections::HashMap::new(),
        crate::data::events_bus::EventPriority::Normal,
        Some("gmail:oauth".to_string()),
        Some(300),
        Some("email".to_string()),
    );

    let _ = tauri_plugin_opener::open_url(auth_url.as_str(), None::<String>);

    let code = rx
        .recv_timeout(Duration::from_secs(120))
        .context("OAuth timed out")?;

    let tokens = exchange_code(code, &client).await?;
    save_tokens(&tokens)?;

    let profile = gmail_api_get("users/me/profile", &tokens.access_token).await?;
    let email_address = profile
        .get("emailAddress")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    let mut settings = EmailSettings::load();
    settings.enabled = true;
    settings.provider = "gmail".to_string();
    settings.connected = true;
    settings.account_email = email_address;
    settings.last_sync_at = Some(crate::core::utils::current_timestamp());
    settings.save()?;

    Ok(settings)
}

pub async fn disconnect() -> Result<EmailSettings> {
    clear_tokens()?;
    let mut settings = EmailSettings::load();
    settings.connected = false;
    settings.account_email = None;
    settings.last_sync_at = None;
    settings.save()?;
    Ok(settings)
}

async fn get_access_token() -> Result<String> {
    let tokens = load_tokens()?.context("No stored tokens")?;
    let (client, _) = build_oauth_client()?;
    let refreshed = refresh_access_token(&client, &tokens.refresh_token).await?;
    save_tokens(&refreshed)?;
    Ok(refreshed.access_token)
}

pub async fn list_inbox(limit: usize) -> Result<Vec<EmailMessage>> {
    let access = get_access_token().await?;
    let response = gmail_api_get(
        &format!("users/me/messages?maxResults={}", limit.clamp(1, 50)),
        &access,
    )
    .await?;

    update_sync_timestamp();

    let message_ids = response
        .get("messages")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();

    let mut messages = Vec::new();
    for msg in message_ids {
        let id = msg.get("id").and_then(|v| v.as_str()).unwrap_or_default();
        if id.is_empty() {
            continue;
        }
        let detail = gmail_api_get(
            &format!("users/me/messages/{}?format=metadata", id),
            &access,
        )
        .await?;
        messages.push(parse_message(&detail));
    }

    Ok(messages)
}

fn parse_message(detail: &serde_json::Value) -> EmailMessage {
    let id = detail
        .get("id")
        .and_then(|v| v.as_str())
        .unwrap_or_default()
        .to_string();
    let thread_id = detail
        .get("threadId")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());
    let snippet = detail
        .get("snippet")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let label_ids = detail
        .get("labelIds")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(|s| s.to_string()))
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();

    let mut subject = String::new();
    let mut from = String::new();
    let mut to = Vec::new();

    if let Some(headers) = detail
        .get("payload")
        .and_then(|v| v.get("headers"))
        .and_then(|v| v.as_array())
    {
        for header in headers {
            let name = header.get("name").and_then(|v| v.as_str()).unwrap_or("");
            let value = header.get("value").and_then(|v| v.as_str()).unwrap_or("");
            match name {
                "Subject" => subject = value.to_string(),
                "From" => from = value.to_string(),
                "To" => to = value.split(',').map(|s| s.trim().to_string()).collect(),
                _ => {}
            }
        }
    }

    let internal_date = detail
        .get("internalDate")
        .and_then(|v| v.as_str())
        .and_then(|s| s.parse::<u64>().ok())
        .map(|ms| ms / 1000)
        .unwrap_or(0);
    let is_unread = label_ids.iter().any(|l| l == "UNREAD");

    EmailMessage {
        id,
        thread_id,
        subject,
        from,
        to,
        snippet,
        received_at: internal_date,
        is_unread,
        labels: label_ids,
    }
}

pub async fn triage_inbox(
    limit: usize,
    ai_router: Option<&crate::ai::ai_provider::SmartAiRouter>,
) -> Result<Vec<crate::integrations::integrations::EmailTriageDecision>> {
    let messages = list_inbox(limit).await?;
    let mut decisions = Vec::new();

    for message in &messages {
        let summary = format!("{} — {}", message.subject, message.from);
        let mut action = "review".to_string();
        let mut confidence = 0.5;
        let mut tags: Vec<String> = Vec::new();

        if let Some(router) = ai_router {
            let prompt = format!(
                "Classify this email for triage. Return JSON with action (archive|review|reply), confidence (0-1), tags (array).\nSubject: {}\nFrom: {}\nSnippet: {}",
                message.subject,
                message.from,
                message.snippet
            );
            if let Ok(response) = router.generate_text_light(&prompt).await {
                if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&response) {
                    if let Some(a) = parsed.get("action").and_then(|v| v.as_str()) {
                        action = a.to_string();
                    }
                    if let Some(c) = parsed.get("confidence").and_then(|v| v.as_f64()) {
                        confidence = c as f32;
                    }
                    if let Some(t) = parsed.get("tags").and_then(|v| v.as_array()) {
                        tags = t
                            .iter()
                            .filter_map(|v| v.as_str().map(|s| s.to_string()))
                            .collect();
                    }
                }
            }
        }

        decisions.push(crate::integrations::integrations::EmailTriageDecision {
            message_id: message.id.clone(),
            action,
            summary,
            confidence,
            tags,
        });
    }

    Ok(decisions)
}

pub async fn apply_triage(
    decisions: &[crate::integrations::integrations::EmailTriageDecision],
) -> Result<()> {
    let access = get_access_token().await?;
    for decision in decisions {
        match decision.action.as_str() {
            "archive" => {
                gmail_api_post(
                    &format!("users/me/messages/{}/modify", decision.message_id),
                    &access,
                    serde_json::json!({ "removeLabelIds": ["INBOX"] }),
                )
                .await?;
            }
            "review" => {
                gmail_api_post(
                    &format!("users/me/messages/{}/modify", decision.message_id),
                    &access,
                    serde_json::json!({ "removeLabelIds": ["UNREAD"] }),
                )
                .await?;
            }
            "reply" => {}
            _ => {}
        }
    }
    update_sync_timestamp();
    Ok(())
}
