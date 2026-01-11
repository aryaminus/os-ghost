//! Shared utility functions
//! Common helpers used across the codebase

use std::sync::OnceLock;
use std::time::{SystemTime, UNIX_EPOCH};

/// Default Ollama configuration values
pub const DEFAULT_OLLAMA_URL: &str = "http://localhost:11434";
pub const DEFAULT_OLLAMA_VISION_MODEL: &str = "llama3.2-vision";
pub const DEFAULT_OLLAMA_TEXT_MODEL: &str = "llama3.2";

/// Thread-safe runtime configuration store
/// Used instead of env::set_var which is not thread-safe
pub struct RuntimeConfig {
    gemini_api_key: std::sync::RwLock<Option<String>>,
    ollama_url: std::sync::RwLock<Option<String>>,
    ollama_vision_model: std::sync::RwLock<Option<String>>,
    ollama_text_model: std::sync::RwLock<Option<String>>,
}

impl RuntimeConfig {
    const fn new() -> Self {
        Self {
            gemini_api_key: std::sync::RwLock::new(None),
            ollama_url: std::sync::RwLock::new(None),
            ollama_vision_model: std::sync::RwLock::new(None),
            ollama_text_model: std::sync::RwLock::new(None),
        }
    }

    /// Set the Gemini API key at runtime
    pub fn set_api_key(&self, key: String) {
        if let Ok(mut guard) = self.gemini_api_key.write() {
            *guard = Some(key);
        }
    }

    /// Clear the runtime Gemini API key (reverting to env var if present)
    pub fn clear_api_key(&self) {
        if let Ok(mut guard) = self.gemini_api_key.write() {
            *guard = None;
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

    /// Check if using user-provided runtime key
    pub fn is_using_user_key(&self) -> bool {
        if let Ok(guard) = self.gemini_api_key.read() {
            return guard.is_some();
        }
        false
    }

    /// Check if API key is configured
    pub fn has_api_key(&self) -> bool {
        self.get_api_key().map(|k| !k.is_empty()).unwrap_or(false)
    }

    // ========================================================================
    // Ollama Configuration
    // ========================================================================

    /// Set Ollama URL
    pub fn set_ollama_url(&self, url: String) {
        if let Ok(mut guard) = self.ollama_url.write() {
            *guard = Some(url);
        }
    }

    /// Get Ollama URL (defaults to localhost:11434)
    pub fn get_ollama_url(&self) -> String {
        if let Ok(guard) = self.ollama_url.read() {
            if let Some(ref url) = *guard {
                return url.clone();
            }
        }
        std::env::var("OLLAMA_URL").unwrap_or_else(|_| DEFAULT_OLLAMA_URL.to_string())
    }

    /// Set Ollama vision model
    pub fn set_ollama_vision_model(&self, model: String) {
        if let Ok(mut guard) = self.ollama_vision_model.write() {
            *guard = Some(model);
        }
    }

    /// Get Ollama vision model (defaults to llama3.2-vision)
    pub fn get_ollama_vision_model(&self) -> String {
        if let Ok(guard) = self.ollama_vision_model.read() {
            if let Some(ref model) = *guard {
                return model.clone();
            }
        }
        std::env::var("OLLAMA_VISION_MODEL")
            .unwrap_or_else(|_| DEFAULT_OLLAMA_VISION_MODEL.to_string())
    }

    /// Set Ollama text model
    pub fn set_ollama_text_model(&self, model: String) {
        if let Ok(mut guard) = self.ollama_text_model.write() {
            *guard = Some(model);
        }
    }

    /// Get Ollama text model (defaults to llama3.2)
    pub fn get_ollama_text_model(&self) -> String {
        if let Ok(guard) = self.ollama_text_model.read() {
            if let Some(ref model) = *guard {
                return model.clone();
            }
        }
        std::env::var("OLLAMA_TEXT_MODEL").unwrap_or_else(|_| DEFAULT_OLLAMA_TEXT_MODEL.to_string())
    }

    /// Reset all Ollama settings to defaults
    pub fn reset_ollama_to_defaults(&self) {
        if let Ok(mut guard) = self.ollama_url.write() {
            *guard = None;
        }
        if let Ok(mut guard) = self.ollama_vision_model.write() {
            *guard = None;
        }
        if let Ok(mut guard) = self.ollama_text_model.write() {
            *guard = None;
        }
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
