//! Encrypted Secrets Storage
//!
//! Provides secure storage for API keys and sensitive credentials using the system keychain.
//! Uses the keyring crate for cross-platform secure storage (macOS Keychain, Windows Credential Manager, Linux Secret Service).
//!
//! Security Features (IronClaw-inspired):
//! - Encrypted storage in system keychain
//! - Host-boundary secret injection (never exposed to tools)
//! - Per-tool secret authorization
//! - Secret leak detection integration
//!
//! ZeroClaw reference: https://github.com/theonlyhennygod/zeroclaw

use keyring::Entry;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::RwLock;

const SERVICE_NAME: &str = "os-ghost";

lazy_static::lazy_static! {
    static ref SECRET_CACHE: RwLock<HashMap<String, String>> = RwLock::new(HashMap::new());
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecretError {
    pub message: String,
}

impl std::fmt::Display for SecretError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Secret error: {}", self.message)
    }
}

impl std::error::Error for SecretError {}

impl From<keyring::Error> for SecretError {
    fn from(e: keyring::Error) -> Self {
        SecretError {
            message: e.to_string(),
        }
    }
}

pub fn get_service_name() -> String {
    SERVICE_NAME.to_string()
}

pub fn store_secret(key: &str, value: &str) -> Result<(), SecretError> {
    let entry = Entry::new(SERVICE_NAME, key)?;
    entry.set_password(value)?;

    // Update cache
    if let Ok(mut cache) = SECRET_CACHE.write() {
        cache.insert(key.to_string(), value.to_string());
    }

    tracing::debug!("Stored secret: {}", key);
    Ok(())
}

pub fn get_secret(key: &str) -> Result<String, SecretError> {
    // Check cache first
    if let Ok(cache) = SECRET_CACHE.read() {
        if let Some(value) = cache.get(key) {
            return Ok(value.clone());
        }
    }

    let entry = Entry::new(SERVICE_NAME, key)?;
    let password = entry.get_password()?;

    // Update cache
    if let Ok(mut cache) = SECRET_CACHE.write() {
        cache.insert(key.to_string(), password.clone());
    }

    Ok(password)
}

pub fn delete_secret(key: &str) -> Result<(), SecretError> {
    let entry = Entry::new(SERVICE_NAME, key)?;
    entry.delete_password().map_err(|e| SecretError {
        message: e.to_string(),
    })?;

    // Remove from cache
    if let Ok(mut cache) = SECRET_CACHE.write() {
        cache.remove(key);
    }

    tracing::debug!("Deleted secret: {}", key);
    Ok(())
}

pub fn has_secret(key: &str) -> bool {
    get_secret(key).is_ok()
}

pub fn clear_cache() {
    if let Ok(mut cache) = SECRET_CACHE.write() {
        cache.clear();
    }
}

pub fn list_secrets() -> Vec<String> {
    // Note: keyring doesn't provide a list operation, so we return known keys
    // This could be enhanced by storing a manifest
    if let Ok(cache) = SECRET_CACHE.read() {
        cache.keys().cloned().collect()
    } else {
        vec![]
    }
}

// ============================================================================
// Convenience functions for OS Ghost specific secrets
// ============================================================================

pub fn store_api_key(provider: &str, api_key: &str) -> Result<(), SecretError> {
    store_secret(&format!("api_key:{}", provider), api_key)
}

pub fn get_api_key(provider: &str) -> Result<String, SecretError> {
    get_secret(&format!("api_key:{}", provider))
}

pub fn delete_api_key(provider: &str) -> Result<(), SecretError> {
    delete_secret(&format!("api_key:{}", provider))
}

pub fn has_api_key(provider: &str) -> bool {
    has_secret(&format!("api_key:{}", provider))
}

// Known provider keys
pub fn get_known_providers() -> Vec<&'static str> {
    vec!["gemini", "openai", "anthropic", "ollama"]
}

// ============================================================================
// Host-Boundary Secret Injection (IronClaw-inspired)
// ============================================================================
//
// Secrets are NEVER exposed to tool code. They are injected at the host boundary
// only when making authenticated requests, and stripped from responses before
// the tool sees them.

use crate::security::leak_detector;

/// A secret injection context - tracks which secrets have been authorized for which tools
#[derive(Debug, Clone, Default)]
pub struct SecretInjectionContext {
    /// Map of tool_name -> authorized secrets for that tool
    authorized_secrets: HashMap<String, Vec<String>>,
}

lazy_static::lazy_static! {
    static ref INJECTION_CONTEXT: RwLock<SecretInjectionContext> = RwLock::new(SecretInjectionContext::default());
}

/// Authorize a secret for use by a specific tool
pub fn authorize_secret_for_tool(tool_name: &str, secret_key: &str) {
    if let Ok(mut ctx) = INJECTION_CONTEXT.write() {
        ctx.authorized_secrets
            .entry(tool_name.to_string())
            .or_insert_with(Vec::new)
            .push(secret_key.to_string());
    }
}

/// Revoke all secrets for a specific tool
pub fn revoke_tool_secrets(tool_name: &str) {
    if let Ok(mut ctx) = INJECTION_CONTEXT.write() {
        ctx.authorized_secrets.remove(tool_name);
    }
}

/// Check if a tool is authorized to use a specific secret
pub fn is_tool_authorized(tool_name: &str, secret_key: &str) -> bool {
    if let Ok(ctx) = INJECTION_CONTEXT.read() {
        ctx.authorized_secrets
            .get(tool_name)
            .map(|secrets| secrets.contains(&secret_key.to_string()))
            .unwrap_or(false)
    } else {
        false
    }
}

/// Inject secrets into HTTP headers at the host boundary
/// Only injects secrets that are authorized for the calling tool
pub fn inject_secrets_for_request(
    tool_name: &str,
    url: &str,
    mut headers: HashMap<String, String>,
) -> Result<HashMap<String, String>, String> {
    // First check if URL is allowed
    let allowed = crate::security::http_allowlist::check_url_allowed(url);
    if !allowed.allowed {
        return Err(format!("URL not allowed: {}", allowed.reason));
    }

    // Check for leak in URL before any secret injection
    let leak_result = leak_detector::scan_for_leaks(url);
    if leak_result.blocked {
        return Err(format!(
            "Potential leak detected in URL: {:?}",
            leak_result.matches
        ));
    }

    // Only inject authorized secrets
    if let Ok(ctx) = INJECTION_CONTEXT.read() {
        if let Some(secrets) = ctx.authorized_secrets.get(tool_name) {
            for secret_key in secrets {
                if let Ok(secret) = get_secret(secret_key) {
                    // Determine header name from secret key
                    let header_name = match secret_key.as_str() {
                        k if k.contains("openai") => "Authorization",
                        k if k.contains("anthropic") => "x-api-key",
                        k if k.contains("gemini") => "x-goog-api-key",
                        _ => "Authorization",
                    };

                    // Format header value
                    let header_value = if header_name == "Authorization" {
                        format!("Bearer {}", secret)
                    } else {
                        secret.clone()
                    };

                    headers.insert(header_name.to_string(), header_value);
                }
            }
        }
    }

    // Scan headers for leaks before sending
    let headers_str = headers
        .iter()
        .map(|(k, v)| format!("{}: {}", k, v))
        .collect::<Vec<_>>()
        .join("\n");

    let leak_result = leak_detector::scan_for_leaks(&headers_str);
    if leak_result.blocked {
        return Err(format!(
            "Potential leak detected in headers: {:?}",
            leak_result.matches
        ));
    }

    Ok(headers)
}

/// Scan response for leaked secrets and sanitize if needed
pub fn sanitize_response(
    tool_name: &str,
    status: u16,
    headers: Option<&HashMap<String, String>>,
    body: Option<&str>,
) -> Result<String, String> {
    // Scan response for leaks
    let leak_result = leak_detector::scan_response(status, headers, body);

    if leak_result.blocked {
        tracing::warn!(
            "Potential credential leak detected in response to tool '{}': {:?}",
            tool_name,
            leak_result.matches
        );

        // Return sanitized content
        if let Some(sanitized) = leak_result.sanitized_content {
            return Ok(sanitized);
        }
    }

    // Return original content
    Ok(body.unwrap_or("").to_string())
}

// ============================================================================
// Tauri Commands
// ============================================================================

#[tauri::command]
pub fn secrets_store(key: String, value: String) -> Result<(), String> {
    store_secret(&key, &value).map_err(|e| e.message)
}

#[tauri::command]
pub fn secrets_get(key: String) -> Result<String, String> {
    get_secret(&key).map_err(|e| e.message)
}

#[tauri::command]
pub fn secrets_delete(key: String) -> Result<(), String> {
    delete_secret(&key).map_err(|e| e.message)
}

#[tauri::command]
pub fn secrets_has(key: String) -> bool {
    has_secret(&key)
}

#[tauri::command]
pub fn secrets_list() -> Vec<String> {
    list_secrets()
}

#[tauri::command]
pub fn secrets_clear_cache() {
    clear_cache();
}

// ============================================================================
// API Key specific commands
// ============================================================================

#[tauri::command]
pub fn store_provider_api_key(provider: String, api_key: String) -> Result<(), String> {
    store_api_key(&provider, &api_key).map_err(|e| e.message)
}

#[tauri::command]
pub fn get_provider_api_key(provider: String) -> Result<String, String> {
    get_api_key(&provider).map_err(|e| e.message)
}

#[tauri::command]
pub fn delete_provider_api_key(provider: String) -> Result<(), String> {
    delete_api_key(&provider).map_err(|e| e.message)
}

#[tauri::command]
pub fn has_provider_api_key(provider: String) -> bool {
    has_api_key(&provider)
}

#[tauri::command]
pub fn get_configured_providers() -> Vec<String> {
    get_known_providers()
        .into_iter()
        .filter(|p| has_api_key(p))
        .map(|p| p.to_string())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_service_name() {
        assert_eq!(get_service_name(), "os-ghost");
    }

    #[test]
    fn test_provider_key_format() {
        let key = format!("api_key:{}", "gemini");
        assert_eq!(key, "api_key:gemini");
    }
}
