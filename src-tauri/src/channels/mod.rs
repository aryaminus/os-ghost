//! Channel Trait for Messaging Integrations
//!
//! Defines a unified interface for messaging channels enabling multi-platform support.
//! Inspired by ZeroClaw's channel architecture: https://github.com/theonlyhennygod/zeroclaw
//!
//! Supported channels:
//! - CLI (terminal)
//! - Telegram
//! - Discord
//! - Slack
//! - Webhook

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

pub mod telegram;
pub mod discord;
pub mod slack;

pub use telegram::TelegramChannel;
pub use discord::DiscordChannel;
pub use slack::SlackChannel;

// ============================================================================
// Channel Trait
// ============================================================================

#[async_trait]
pub trait Channel: Send + Sync {
    /// Get the channel name
    fn name(&self) -> &str;
    
    /// Initialize the channel (connect, authenticate, etc.)
    async fn initialize(&self) -> Result<(), ChannelError>;
    
    /// Check if the channel is connected
    fn is_connected(&self) -> bool;
    
    /// Send a message
    async fn send(&self, message: &str, destination: &str) -> Result<(), ChannelError>;
    
    /// Receive messages (poll or stream)
    async fn receive(&self) -> Result<Vec<ChannelMessage>, ChannelError>;
    
    /// Get channel info for display
    fn info(&self) -> ChannelInfo;
}

// ============================================================================
// Channel Types
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChannelInfo {
    pub name: String,
    pub connected: bool,
    pub can_send: bool,
    pub can_receive: bool,
    pub requires_webhook: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChannelMessage {
    pub id: String,
    pub sender: String,
    pub content: String,
    pub timestamp: i64,
    pub channel: String,
    pub metadata: std::collections::HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ChannelError {
    NotConfigured(String),
    NotConnected(String),
    SendFailed(String),
    ReceiveFailed(String),
    AuthenticationFailed(String),
    RateLimited(String),
    WebhookError(String),
}

impl std::fmt::Display for ChannelError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ChannelError::NotConfigured(c) => write!(f, "Channel {} not configured", c),
            ChannelError::NotConnected(c) => write!(f, "Channel {} not connected", c),
            ChannelError::SendFailed(msg) => write!(f, "Send failed: {}", msg),
            ChannelError::ReceiveFailed(msg) => write!(f, "Receive failed: {}", msg),
            ChannelError::AuthenticationFailed(msg) => write!(f, "Authentication failed: {}", msg),
            ChannelError::RateLimited(msg) => write!(f, "Rate limited: {}", msg),
            ChannelError::WebhookError(msg) => write!(f, "Webhook error: {}", msg),
        }
    }
}

impl std::error::Error for ChannelError {}

// ============================================================================
// Channel Type Enum
// ============================================================================

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ChannelKind {
    Cli,
    Telegram,
    Discord,
    Slack,
    Webhook,
}

impl std::fmt::Display for ChannelKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ChannelKind::Cli => write!(f, "cli"),
            ChannelKind::Telegram => write!(f, "telegram"),
            ChannelKind::Discord => write!(f, "discord"),
            ChannelKind::Slack => write!(f, "slack"),
            ChannelKind::Webhook => write!(f, "webhook"),
        }
    }
}

impl ChannelKind {
    #[allow(clippy::should_implement_trait)]
    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "cli" => Some(ChannelKind::Cli),
            "telegram" => Some(ChannelKind::Telegram),
            "discord" => Some(ChannelKind::Discord),
            "slack" => Some(ChannelKind::Slack),
            "webhook" => Some(ChannelKind::Webhook),
            _ => None,
        }
    }
}

// ============================================================================
// Channel Factory
// ============================================================================

pub struct ChannelFactory;

impl ChannelFactory {
    pub fn create(
        kind: ChannelKind,
        config: ChannelConfig,
    ) -> Result<Arc<dyn Channel>, ChannelError> {
        match kind {
            ChannelKind::Cli => {
                Ok(Arc::new(CliChannel::new()))
            }
            ChannelKind::Telegram => {
                let token = config.api_token.ok_or_else(|| 
                    ChannelError::NotConfigured("telegram".to_string()))?;
                Ok(Arc::new(TelegramChannel::new(&token)?))
            }
            ChannelKind::Discord => {
                let token = config.api_token.ok_or_else(|| 
                    ChannelError::NotConfigured("discord".to_string()))?;
                Ok(Arc::new(DiscordChannel::new(&token)?))
            }
            ChannelKind::Slack => {
                let token = config.api_token.ok_or_else(|| 
                    ChannelError::NotConfigured("slack".to_string()))?;
                Ok(Arc::new(SlackChannel::new(&token)?))
            }
            ChannelKind::Webhook => {
                Ok(Arc::new(WebhookChannel::new(config.webhook_path.as_deref())))
            }
        }
    }
    
    pub fn available_channels() -> Vec<ChannelKind> {
        vec![
            ChannelKind::Cli,
            ChannelKind::Telegram,
            ChannelKind::Discord,
            ChannelKind::Slack,
            ChannelKind::Webhook,
        ]
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ChannelConfig {
    pub api_token: Option<String>,
    pub bot_token: Option<String>,
    pub webhook_path: Option<String>,
    pub allowed_senders: Vec<String>,
}

// ============================================================================
// CLI Channel Implementation
// ============================================================================

pub struct CliChannel;

impl Default for CliChannel {
    fn default() -> Self {
        Self::new()
    }
}

impl CliChannel {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl Channel for CliChannel {
    fn name(&self) -> &str {
        "cli"
    }
    
    async fn initialize(&self) -> Result<(), ChannelError> {
        Ok(())
    }
    
    fn is_connected(&self) -> bool {
        true
    }
    
    async fn send(&self, message: &str, _destination: &str) -> Result<(), ChannelError> {
        println!("{}", message);
        Ok(())
    }
    
    async fn receive(&self) -> Result<Vec<ChannelMessage>, ChannelError> {
        // CLI doesn't support receiving in this implementation
        Ok(vec![])
    }
    
    fn info(&self) -> ChannelInfo {
        ChannelInfo {
            name: self.name().to_string(),
            connected: true,
            can_send: true,
            can_receive: false,
            requires_webhook: false,
        }
    }
}

// ============================================================================
// Webhook Channel Implementation
// ============================================================================

#[allow(dead_code)]
pub struct WebhookChannel {
    webhook_path: Option<String>,
    pending_messages: std::sync::Mutex<Vec<ChannelMessage>>,
}

impl WebhookChannel {
    pub fn new(webhook_path: Option<&str>) -> Self {
        Self {
            webhook_path: webhook_path.map(|s| s.to_string()),
            pending_messages: std::sync::Mutex::new(vec![]),
        }
    }
    
    pub fn receive_message(&self, message: ChannelMessage) {
        if let Ok(mut pending) = self.pending_messages.lock() {
            pending.push(message);
        }
    }
}

#[async_trait]
impl Channel for WebhookChannel {
    fn name(&self) -> &str {
        "webhook"
    }
    
    async fn initialize(&self) -> Result<(), ChannelError> {
        Ok(())
    }
    
    fn is_connected(&self) -> bool {
        true
    }
    
    async fn send(&self, message: &str, _destination: &str) -> Result<(), ChannelError> {
        // For webhook, we'd need to POST back to the sender
        // This is a simplified implementation
        tracing::debug!("Webhook send: {}", message);
        Ok(())
    }
    
    async fn receive(&self) -> Result<Vec<ChannelMessage>, ChannelError> {
        let mut pending = vec![];
        if let Ok(mut messages) = self.pending_messages.lock() {
            pending = messages.clone();
            messages.clear();
        }
        Ok(pending)
    }
    
    fn info(&self) -> ChannelInfo {
        ChannelInfo {
            name: self.name().to_string(),
            connected: true,
            can_send: true,
            can_receive: true,
            requires_webhook: true,
        }
    }
}

// ============================================================================
// Channel Registry
// ============================================================================

use std::collections::HashMap;
use std::sync::RwLock;

lazy_static::lazy_static! {
    static ref CHANNEL_REGISTRY: RwLock<HashMap<String, Arc<dyn Channel>>> = RwLock::new(HashMap::new());
}

pub fn register_channel(name: &str, channel: Arc<dyn Channel>) {
    if let Ok(mut registry) = CHANNEL_REGISTRY.write() {
        registry.insert(name.to_string(), channel);
    }
}

pub fn get_channel(name: &str) -> Option<Arc<dyn Channel>> {
    if let Ok(registry) = CHANNEL_REGISTRY.read() {
        registry.get(name).cloned()
    } else {
        None
    }
}

pub fn list_channels() -> Vec<String> {
    if let Ok(registry) = CHANNEL_REGISTRY.read() {
        registry.keys().cloned().collect()
    } else {
        vec![]
    }
}

// ============================================================================
// Tauri Commands
// ============================================================================

#[tauri::command]
pub fn get_available_channels() -> Vec<String> {
    ChannelFactory::available_channels()
        .iter()
        .map(|c| c.to_string())
        .collect()
}

#[tauri::command]
pub fn get_registered_channels() -> Vec<String> {
    list_channels()
}

#[tauri::command]
pub fn get_channel_info(name: String) -> Option<ChannelInfo> {
    get_channel(&name).map(|c| c.info())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_channel_kind_from_str() {
        assert_eq!(ChannelKind::from_str("telegram"), Some(ChannelKind::Telegram));
        assert_eq!(ChannelKind::from_str("discord"), Some(ChannelKind::Discord));
        assert_eq!(ChannelKind::from_str("unknown"), None);
    }

    #[test]
    fn test_cli_channel() {
        let channel = CliChannel::new();
        assert_eq!(channel.name(), "cli");
        assert!(channel.is_connected());
    }
}
