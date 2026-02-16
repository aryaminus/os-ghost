//! Discord Channel Implementation
//!
//! Provides integration with Discord Bot API

use async_trait::async_trait;
use reqwest::Client;

use super::{Channel, ChannelError, ChannelInfo, ChannelMessage};

pub struct DiscordChannel {
    client: Client,
    token: String,
    connected: std::sync::atomic::AtomicBool,
}

impl DiscordChannel {
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
    
    fn api_url(&self, endpoint: &str) -> String {
        format!("https://discord.com/api/v10{}", endpoint)
    }
}

#[async_trait]
impl Channel for DiscordChannel {
    fn name(&self) -> &str {
        "discord"
    }
    
    async fn initialize(&self) -> Result<(), ChannelError> {
        // Test the bot token by getting current user
        let url = self.api_url("/users/@me");
        
        let response = self.client.get(&url)
            .header("Authorization", format!("Bot {}", self.token))
            .send()
            .await
            .map_err(|e| ChannelError::AuthenticationFailed(e.to_string()))?;
        
        if response.status().is_success() {
            self.connected.store(true, std::sync::atomic::Ordering::SeqCst);
            tracing::info!("Discord channel initialized");
            Ok(())
        } else {
            Err(ChannelError::AuthenticationFailed("Invalid bot token".to_string()))
        }
    }
    
    fn is_connected(&self) -> bool {
        self.connected.load(std::sync::atomic::Ordering::SeqCst)
    }
    
    async fn send(&self, message: &str, channel_id: &str) -> Result<(), ChannelError> {
        let url = self.api_url(&format!("/channels/{}/messages", channel_id));
        
        let body = serde_json::json!({
            "content": message
        });
        
        let response = self.client.post(&url)
            .header("Authorization", format!("Bot {}", self.token))
            .header("Content-Type", "application/json")
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
        // For Discord, you'd typically use a gateway connection or webhooks
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_api_url() {
        let channel = DiscordChannel::new("TEST_TOKEN_FOR_UNIT_TESTS").unwrap();
        let url = channel.api_url("/channels/123/messages");
        assert!(url.contains("discord.com"));
        assert!(url.contains("channels/123"));
    }
}
