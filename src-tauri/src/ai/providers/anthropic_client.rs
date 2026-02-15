//! Anthropic Claude Client
//!
//! Supports Claude API (Anthropic)

use async_trait::async_trait;
use reqwest::Client;
use serde::Deserialize;

use super::{CompletionOptions, Provider, ProviderError, ProviderInfo};

pub struct AnthropicClient {
    client: Client,
    api_key: String,
    model: String,
}

impl AnthropicClient {
    pub fn new(api_key: &str, model: &str) -> Result<Self, ProviderError> {
        let client = Client::builder()
            .timeout(std::time::Duration::from_secs(120))
            .build()?;

        Ok(Self {
            client,
            api_key: api_key.to_string(),
            model: model.to_string(),
        })
    }

    fn max_tokens_for_model(&self) -> usize {
        // All Claude models support 200k context
        if self.model.contains("claude-") {
            200000
        } else {
            4096
        }
    }
}

#[async_trait]
impl Provider for AnthropicClient {
    fn name(&self) -> &str {
        "anthropic"
    }

    fn model(&self) -> &str {
        &self.model
    }

    async fn is_available(&self) -> bool {
        // Try a simple models list request
        let url = "https://api.anthropic.com/v1/messages";

        let body = serde_json::json!({
            "model": self.model,
            "max_tokens": 1,
            "messages": [{"role": "user", "content": "hi"}]
        });

        match self
            .client
            .post(url)
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", "2023-06-01")
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await
        {
            Ok(resp) => resp.status().is_success() || resp.status().as_u16() == 400, // 400 means auth works but maybe other issue
            Err(_) => false,
        }
    }

    async fn complete(&self, prompt: &str) -> Result<String, ProviderError> {
        self.complete_with_options(prompt, CompletionOptions::default())
            .await
    }

    async fn complete_with_options(
        &self,
        prompt: &str,
        options: CompletionOptions,
    ) -> Result<String, ProviderError> {
        let url = "https://api.anthropic.com/v1/messages";

        let max_tokens = options.max_tokens.unwrap_or(self.max_tokens_for_model());

        let mut body = serde_json::json!({
            "model": self.model,
            "messages": [
                {"role": "user", "content": prompt}
            ],
            "max_tokens": max_tokens,
        });

        // Add optional parameters
        if let Some(temp) = options.temperature {
            body["temperature"] = serde_json::json!(temp);
        }
        if let Some(top_p) = options.top_p {
            body["top_p"] = serde_json::json!(top_p);
        }
        if let Some(stop) = options.stop {
            body["stop_sequences"] = serde_json::json!(stop);
        }

        let response = self
            .client
            .post(url)
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", "2023-06-01")
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status();
            let text = response.text().await.unwrap_or_default();
            return Err(ProviderError::APIError(format!(
                "Status {}: {}",
                status, text
            )));
        }

        let response: AnthropicResponse = response.json().await?;

        let content = response
            .content
            .first()
            .map(|c| c.text.clone())
            .ok_or_else(|| ProviderError::APIError("No content in response".to_string()))?;

        Ok(content)
    }

    fn supports_vision(&self) -> bool {
        // Claude 3 Sonnet and Opus support vision
        self.model.contains("claude-3")
            || self.model.contains("sonnet")
            || self.model.contains("opus")
    }

    async fn analyze_image(
        &self,
        base64_image: &str,
        prompt: &str,
    ) -> Result<String, ProviderError> {
        if !self.supports_vision() {
            return Err(ProviderError::VisionNotSupported(self.name().to_string()));
        }

        let url = "https://api.anthropic.com/v1/messages";

        let image_media_type = "image/png";

        let body = serde_json::json!({
            "model": self.model,
            "max_tokens": 4096,
            "messages": [{
                "role": "user",
                "content": [
                    {
                        "type": "image",
                        "source": {
                            "type": "base64",
                            "media_type": image_media_type,
                            "data": base64_image
                        }
                    },
                    {
                        "type": "text",
                        "text": prompt
                    }
                ]
            }]
        });

        let response = self
            .client
            .post(url)
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", "2023-06-01")
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status();
            let text = response.text().await.unwrap_or_default();
            return Err(ProviderError::APIError(format!(
                "Status {}: {}",
                status, text
            )));
        }

        let response: AnthropicResponse = response.json().await?;

        let content = response
            .content
            .first()
            .map(|c| c.text.clone())
            .ok_or_else(|| ProviderError::APIError("No content in response".to_string()))?;

        Ok(content)
    }

    fn info(&self) -> ProviderInfo {
        let context_window = if self.model.contains("claude-") {
            200000
        } else {
            100000
        };

        ProviderInfo {
            name: self.name().to_string(),
            model: self.model.clone(),
            supports_vision: self.supports_vision(),
            supports_streaming: true,
            context_window,
            max_tokens: 4096,
        }
    }
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct AnthropicResponse {
    content: Vec<ContentBlock>,
    #[serde(rename = "stop_reason")]
    stop_reason: Option<String>,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct ContentBlock {
    #[serde(rename = "type")]
    block_type: String,
    text: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_client_creation() {
        let client = AnthropicClient::new("test-key", "claude-sonnet-4-20250514");
        assert!(client.is_ok());

        let client = client.unwrap();
        assert_eq!(client.name(), "anthropic");
        assert_eq!(client.model(), "claude-sonnet-4-20250514");
    }

    #[test]
    fn test_max_tokens() {
        let client = AnthropicClient::new("test-key", "claude-opus-4-20251114").unwrap();
        assert!(client.max_tokens_for_model() >= 4096);

        let client = AnthropicClient::new("test-key", "claude-haiku-20240307").unwrap();
        assert!(client.max_tokens_for_model() >= 4096);
    }
}
