//! Slack Channel Implementation
//!
//! Provides integration with Slack Bot API

use async_trait::async_trait;
use reqwest::Client;

use super::{Channel, ChannelError, ChannelInfo, ChannelMessage};

pub struct SlackChannel {
    client: Client,
    token: String,
    connected: std::sync::atomic::AtomicBool,
}

impl SlackChannel {
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
        format!("https://slack.com/api/{}", method)
    }
}

#[async_trait]
impl Channel for SlackChannel {
    fn name(&self) -> &str {
        "slack"
    }
    
    async fn initialize(&self) -> Result<(), ChannelError> {
        // Test the bot token by calling auth.test
        let url = self.api_url("auth.test");
        
        let response = self.client.get(&url)
            .header("Authorization", format!("Bearer {}", self.token))
            .send()
            .await
            .map_err(|e| ChannelError::AuthenticationFailed(e.to_string()))?;
        
        if response.status().is_success() {
            let body: serde_json::Value = response.json().await
                .map_err(|e| ChannelError::AuthenticationFailed(e.to_string()))?;
            
            if body.get("ok").and_then(|v| v.as_bool()).unwrap_or(false) {
                self.connected.store(true, std::sync::atomic::Ordering::SeqCst);
                tracing::info!("Slack channel initialized");
                return Ok(());
            }
        }
        
        Err(ChannelError::AuthenticationFailed("Invalid bot token".to_string()))
    }
    
    fn is_connected(&self) -> bool {
        self.connected.load(std::sync::atomic::Ordering::SeqCst)
    }
    
    async fn send(&self, message: &str, channel_id: &str) -> Result<(), ChannelError> {
        let url = self.api_url("chat.postMessage");
        
        let body = serde_json::json!({
            "channel": channel_id,
            "text": message
        });
        
        let response = self.client.post(&url)
            .header("Authorization", format!("Bearer {}", self.token))
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await
            .map_err(|e| ChannelError::SendFailed(e.to_string()))?;
        
        if response.status().is_success() {
            let result: serde_json::Value = response.json().await
                .map_err(|e| ChannelError::SendFailed(e.to_string()))?;
            
            if result.get("ok").and_then(|v| v.as_bool()).unwrap_or(false) {
                return Ok(());
            }
            
            return Err(ChannelError::SendFailed(
                result.get("error").and_then(|v| v.as_str()).unwrap_or("Unknown error").to_string()
            ));
        }
        
        Err(ChannelError::SendFailed(format!("Status: {}", response.status())))
    }
    
    async fn receive(&self) -> Result<Vec<ChannelMessage>, ChannelError> {
        // For Slack, you'd typically use Event Subscriptions or RTM
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
        let channel = SlackChannel::new("TEST_TOKEN_FOR_UNIT_TESTS").unwrap();
        let url = channel.api_url("chat.postMessage");
        assert!(url.contains("slack.com"));
        assert!(url.contains("chat.postMessage"));
    }
}
