//! Encrypted Secrets Storage
//!
//! Provides secure storage for API keys and sensitive credentials using the system keychain.
//! Uses the keyring crate for cross-platform secure storage (macOS Keychain, Windows Credential Manager, Linux Secret Service).
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
