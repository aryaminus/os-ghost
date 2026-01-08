//! Shared utility functions
//! Common helpers used across the codebase

use std::sync::OnceLock;
use std::time::{SystemTime, UNIX_EPOCH};

/// Thread-safe runtime configuration store
/// Used instead of env::set_var which is not thread-safe
pub struct RuntimeConfig {
    gemini_api_key: std::sync::RwLock<Option<String>>,
}

impl RuntimeConfig {
    const fn new() -> Self {
        Self {
            gemini_api_key: std::sync::RwLock::new(None),
        }
    }

    /// Set the Gemini API key at runtime
    pub fn set_api_key(&self, key: String) {
        if let Ok(mut guard) = self.gemini_api_key.write() {
            *guard = Some(key);
        }
    }

    /// Get the Gemini API key
    pub fn get_api_key(&self) -> Option<String> {
        // First check runtime config
        if let Ok(guard) = self.gemini_api_key.read() {
            if let Some(ref key) = *guard {
                return Some(key.clone());
            }
        }
        // Fall back to environment variable
        std::env::var("GEMINI_API_KEY").ok()
    }

    /// Check if API key is configured
    pub fn has_api_key(&self) -> bool {
        self.get_api_key().map(|k| !k.is_empty()).unwrap_or(false)
    }
}

/// Global runtime configuration instance
static RUNTIME_CONFIG: OnceLock<RuntimeConfig> = OnceLock::new();

/// Get the global runtime configuration
pub fn runtime_config() -> &'static RuntimeConfig {
    RUNTIME_CONFIG.get_or_init(RuntimeConfig::new)
}

/// Get current Unix timestamp in seconds
/// Consistent implementation used throughout the codebase
#[inline]
#[must_use]
pub fn current_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

/// Get current Unix timestamp in milliseconds
#[inline]
#[must_use]
pub fn current_timestamp_millis() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
}

/// Clean markdown code blocks from AI responses
/// Handles ```json and ``` wrappers commonly returned by LLMs
#[inline]
#[must_use]
pub fn clean_json_response(text: &str) -> &str {
    text.trim()
        .trim_start_matches("```json")
        .trim_start_matches("```")
        .trim_end_matches("```")
        .trim()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_clean_json_response() {
        assert_eq!(clean_json_response("```json\n{}\n```"), "{}");
        assert_eq!(clean_json_response("```\n{}\n```"), "{}");
        assert_eq!(clean_json_response("{}"), "{}");
        assert_eq!(clean_json_response("  {}  "), "{}");
    }

    #[test]
    fn test_timestamp() {
        let ts = current_timestamp();
        assert!(ts > 0);
        // Should be after 2024
        assert!(ts > 1704067200);
    }
}
