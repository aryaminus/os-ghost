//! Telegram Channel Implementation
//!
//! Provides integration with Telegram Bot API

use async_trait::async_trait;
use reqwest::Client;
use serde::Deserialize;

use super::{Channel, ChannelError, ChannelInfo, ChannelMessage};

pub struct TelegramChannel {
    client: Client,
    token: String,
    connected: std::sync::atomic::AtomicBool,
}

impl TelegramChannel {
    pub fn new(token: &str) -> Result<Self, ChannelError> {
        let client = Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .map_err(|e| ChannelError::NotConfigured(e.to_string()))?;
        
        Ok(Self {
            client,
            token: token.to_string(),
            connected: std::sync::atomic::AtomicBool::new(false),
        })
    }
    
    fn api_url(&self, method: &str) -> String {
        format!("https://api.telegram.org/bot{}/{}", self.token, method)
    }
}

#[async_trait]
impl Channel for TelegramChannel {
    fn name(&self) -> &str {
        "telegram"
    }
    
    async fn initialize(&self) -> Result<(), ChannelError> {
        // Test the bot token by getting bot info
        let url = self.api_url("getMe");
        
        let response = self.client.get(&url)
            .send()
            .await
            .map_err(|e| ChannelError::AuthenticationFailed(e.to_string()))?;
        
        if response.status().is_success() {
            self.connected.store(true, std::sync::atomic::Ordering::SeqCst);
            tracing::info!("Telegram channel initialized");
            Ok(())
        } else {
            Err(ChannelError::AuthenticationFailed("Invalid bot token".to_string()))
        }
    }
    
    fn is_connected(&self) -> bool {
        self.connected.load(std::sync::atomic::Ordering::SeqCst)
    }
    
    async fn send(&self, message: &str, chat_id: &str) -> Result<(), ChannelError> {
        let url = self.api_url("sendMessage");
        
        let body = serde_json::json!({
            "chat_id": chat_id,
            "text": message,
            "parse_mode": "Markdown"
        });
        
        let response = self.client.post(&url)
            .json(&body)
            .send()
            .await
            .map_err(|e| ChannelError::SendFailed(e.to_string()))?;
        
        if response.status().is_success() {
            Ok(())
        } else {
            Err(ChannelError::SendFailed(format!("Status: {}", response.status())))
        }
    }
    
    async fn receive(&self) -> Result<Vec<ChannelMessage>, ChannelError> {
        // For Telegram, you'd typically set up a webhook or poll getUpdates
        // This is a simplified implementation
        Ok(vec![])
    }
    
    fn info(&self) -> ChannelInfo {
        ChannelInfo {
            name: self.name().to_string(),
            connected: self.is_connected(),
            can_send: true,
            can_receive: true,
            requires_webhook: false,
        }
    }
}

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
struct TelegramUpdate {
    update_id: u64,
    message: Option<TelegramMessage>,
}

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
struct TelegramMessage {
    message_id: u64,
    from: Option<TelegramUser>,
    chat: TelegramChat,
    text: Option<String>,
    date: u64,
}

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
struct TelegramUser {
    id: u64,
    username: Option<String>,
    first_name: Option<String>,
}

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
struct TelegramChat {
    id: i64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_api_url() {
        let channel = TelegramChannel::new("123456DEF123:ABC-4ghIkl-zyx57W2v1u123ew11").unwrap();
        let url = channel.api_url("sendMessage");
        assert!(url.contains("api.telegram.org"));
        assert!(url.contains("sendMessage"));
    }
}
