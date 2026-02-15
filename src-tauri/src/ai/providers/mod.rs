//! AI Provider Trait and Factory
//!
//! Defines a unified interface for AI providers enabling runtime-swappable backends.
//! Inspired by ZeroClaw's provider architecture: https://github.com/theonlyhennygod/zeroclaw
//!
//! Supported providers:
//! - Google Gemini
//! - Ollama (local)
//! - OpenAI (GPT-4, GPT-4o)
//! - Anthropic (Claude)
//! - OpenAI-compatible APIs

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

pub mod anthropic_client;
pub mod openai_client;

pub use anthropic_client::AnthropicClient;
pub use openai_client::OpenAIClient;

// ============================================================================
// Provider Trait
// ============================================================================

#[async_trait]
pub trait Provider: Send + Sync {
    /// Get the provider name
    fn name(&self) -> &str;

    /// Get the model name
    fn model(&self) -> &str;

    /// Check if the provider is available
    async fn is_available(&self) -> bool;

    /// Generate text completion
    async fn complete(&self, prompt: &str) -> Result<String, ProviderError>;

    /// Generate text with options
    async fn complete_with_options(
        &self,
        prompt: &str,
        options: CompletionOptions,
    ) -> Result<String, ProviderError>;

    /// Check if this provider supports vision
    fn supports_vision(&self) -> bool;

    /// Analyze an image (if vision supported)
    async fn analyze_image(
        &self,
        _base64_image: &str,
        _prompt: &str,
    ) -> Result<String, ProviderError> {
        Err(ProviderError::VisionNotSupported(self.name().to_string()))
    }

    /// Get provider info for display
    fn info(&self) -> ProviderInfo;
}

// ============================================================================
// Provider Types
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderInfo {
    pub name: String,
    pub model: String,
    pub supports_vision: bool,
    pub supports_streaming: bool,
    pub context_window: usize,
    pub max_tokens: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompletionOptions {
    pub temperature: Option<f64>,
    pub max_tokens: Option<usize>,
    pub top_p: Option<f64>,
    pub stop: Option<Vec<String>>,
}

impl Default for CompletionOptions {
    fn default() -> Self {
        Self {
            temperature: Some(0.7),
            max_tokens: Some(4096),
            top_p: None,
            stop: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ProviderError {
    NotConfigured(String),
    NotAvailable(String),
    RateLimited(String),
    InvalidRequest(String),
    APIError(String),
    VisionNotSupported(String),
    NetworkError(String),
    Timeout,
}

impl std::fmt::Display for ProviderError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ProviderError::NotConfigured(p) => write!(f, "Provider {} not configured", p),
            ProviderError::NotAvailable(p) => write!(f, "Provider {} not available", p),
            ProviderError::RateLimited(msg) => write!(f, "Rate limited: {}", msg),
            ProviderError::InvalidRequest(msg) => write!(f, "Invalid request: {}", msg),
            ProviderError::APIError(msg) => write!(f, "API error: {}", msg),
            ProviderError::VisionNotSupported(p) => {
                write!(f, "Provider {} does not support vision", p)
            }
            ProviderError::NetworkError(msg) => write!(f, "Network error: {}", msg),
            ProviderError::Timeout => write!(f, "Request timed out"),
        }
    }
}

impl std::error::Error for ProviderError {}

impl From<reqwest::Error> for ProviderError {
    fn from(e: reqwest::Error) -> Self {
        if e.is_timeout() {
            ProviderError::Timeout
        } else {
            ProviderError::NetworkError(e.to_string())
        }
    }
}

impl From<serde_json::Error> for ProviderError {
    fn from(e: serde_json::Error) -> Self {
        ProviderError::InvalidRequest(e.to_string())
    }
}

// ============================================================================
// Provider Type Enum
// ============================================================================

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ProviderKind {
    Gemini,
    Ollama,
    OpenAI,
    Anthropic,
    Custom,
}

impl std::fmt::Display for ProviderKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ProviderKind::Gemini => write!(f, "gemini"),
            ProviderKind::Ollama => write!(f, "ollama"),
            ProviderKind::OpenAI => write!(f, "openai"),
            ProviderKind::Anthropic => write!(f, "anthropic"),
            ProviderKind::Custom => write!(f, "custom"),
        }
    }
}

impl ProviderKind {
    #[allow(clippy::should_implement_trait)]
    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "gemini" => Some(ProviderKind::Gemini),
            "ollama" => Some(ProviderKind::Ollama),
            "openai" => Some(ProviderKind::OpenAI),
            "anthropic" => Some(ProviderKind::Anthropic),
            "custom" => Some(ProviderKind::Custom),
            _ => None,
        }
    }
}

// ============================================================================
// Provider Factory
// ============================================================================

pub struct ProviderFactory;

impl ProviderFactory {
    /// Create a provider from configuration
    pub fn create(
        kind: ProviderKind,
        api_key: Option<&str>,
        model: Option<&str>,
        base_url: Option<&str>,
    ) -> Result<Arc<dyn Provider>, ProviderError> {
        match kind {
            ProviderKind::Gemini => {
                let _api_key =
                    api_key.ok_or_else(|| ProviderError::NotConfigured("gemini".to_string()))?;
                // Use the existing Gemini client wrapper
                // For now, return an error - the existing SmartAiRouter handles Gemini
                Err(ProviderError::NotConfigured(
                    "Use SmartAiRouter for Gemini".to_string(),
                ))
            }
            ProviderKind::Ollama => {
                // Use the existing Ollama client wrapper
                Err(ProviderError::NotConfigured(
                    "Use SmartAiRouter for Ollama".to_string(),
                ))
            }
            ProviderKind::OpenAI => {
                let api_key =
                    api_key.ok_or_else(|| ProviderError::NotConfigured("openai".to_string()))?;
                let model = model.unwrap_or("gpt-4o");
                Ok(Arc::new(OpenAIClient::new(api_key, model, base_url)?))
            }
            ProviderKind::Anthropic => {
                let api_key =
                    api_key.ok_or_else(|| ProviderError::NotConfigured("anthropic".to_string()))?;
                let model = model.unwrap_or("claude-sonnet-4-20250514");
                Ok(Arc::new(AnthropicClient::new(api_key, model)?))
            }
            ProviderKind::Custom => {
                let api_key =
                    api_key.ok_or_else(|| ProviderError::NotConfigured("custom".to_string()))?;
                let model = model.unwrap_or("gpt-4o");
                let base_url =
                    base_url.ok_or_else(|| ProviderError::NotConfigured("custom".to_string()))?;
                Ok(Arc::new(OpenAIClient::new(api_key, model, Some(base_url))?))
            }
        }
    }

    /// List available providers
    pub fn available_providers() -> Vec<ProviderKind> {
        vec![
            ProviderKind::Gemini,
            ProviderKind::Ollama,
            ProviderKind::OpenAI,
            ProviderKind::Anthropic,
        ]
    }
}

// ============================================================================
// Provider Registry
// ============================================================================

use std::collections::HashMap;
use std::sync::RwLock;

lazy_static::lazy_static! {
    static ref PROVIDER_REGISTRY: RwLock<HashMap<String, Arc<dyn Provider>>> = RwLock::new(HashMap::new());
}

pub fn register_provider(name: &str, provider: Arc<dyn Provider>) {
    if let Ok(mut registry) = PROVIDER_REGISTRY.write() {
        registry.insert(name.to_string(), provider);
    }
}

pub fn get_provider(name: &str) -> Option<Arc<dyn Provider>> {
    if let Ok(registry) = PROVIDER_REGISTRY.read() {
        registry.get(name).cloned()
    } else {
        None
    }
}

pub fn list_providers() -> Vec<String> {
    if let Ok(registry) = PROVIDER_REGISTRY.read() {
        registry.keys().cloned().collect()
    } else {
        vec![]
    }
}

// ============================================================================
// Tauri Commands
// ============================================================================

#[tauri::command]
pub fn get_available_providers() -> Vec<String> {
    ProviderFactory::available_providers()
        .iter()
        .map(|p| p.to_string())
        .collect()
}

#[tauri::command]
pub fn get_registered_providers() -> Vec<String> {
    list_providers()
}

#[tauri::command]
pub fn get_provider_info(name: String) -> Option<ProviderInfo> {
    get_provider(&name).map(|p| p.info())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_provider_kind_from_str() {
        assert_eq!(ProviderKind::from_str("gemini"), Some(ProviderKind::Gemini));
        assert_eq!(ProviderKind::from_str("openai"), Some(ProviderKind::OpenAI));
        assert_eq!(ProviderKind::from_str("unknown"), None);
    }

    #[test]
    fn test_completion_options_default() {
        let options = CompletionOptions::default();
        assert_eq!(options.temperature, Some(0.7));
        assert_eq!(options.max_tokens, Some(4096));
    }
}
