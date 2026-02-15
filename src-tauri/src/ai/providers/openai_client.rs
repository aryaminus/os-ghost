//! OpenAI Compatible Client
//!
//! Supports OpenAI API and any OpenAI-compatible API (custom endpoints, local models, etc.)

use async_trait::async_trait;
use reqwest::Client;
use serde::Deserialize;

use super::{CompletionOptions, Provider, ProviderError, ProviderInfo};

pub struct OpenAIClient {
    client: Client,
    api_key: String,
    model: String,
    base_url: String,
}

impl OpenAIClient {
    pub fn new(api_key: &str, model: &str, base_url: Option<&str>) -> Result<Self, ProviderError> {
        let base_url = base_url.unwrap_or("https://api.openai.com/v1");

        let client = Client::builder()
            .timeout(std::time::Duration::from_secs(120))
            .build()?;

        Ok(Self {
            client,
            api_key: api_key.to_string(),
            model: model.to_string(),
            base_url: base_url.to_string(),
        })
    }
}

#[async_trait]
impl Provider for OpenAIClient {
    fn name(&self) -> &str {
        "openai"
    }

    fn model(&self) -> &str {
        &self.model
    }

    async fn is_available(&self) -> bool {
        // Try a simple models list request
        let url = format!("{}/models", self.base_url);

        match self
            .client
            .get(&url)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .send()
            .await
        {
            Ok(resp) => resp.status().is_success(),
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
        let url = format!("{}/chat/completions", self.base_url);

        let body = serde_json::json!({
            "model": self.model,
            "messages": [
                {"role": "user", "content": prompt}
            ],
            "temperature": options.temperature.unwrap_or(0.7),
            "max_tokens": options.max_tokens.unwrap_or(4096),
            "top_p": options.top_p,
            "stop": options.stop,
        });

        let response = self
            .client
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.api_key))
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

        let response: OpenAIResponse = response.json().await?;

        let content = response
            .choices
            .first()
            .and_then(|c| c.message.content.clone())
            .ok_or_else(|| ProviderError::APIError("No content in response".to_string()))?;

        Ok(content)
    }

    fn supports_vision(&self) -> bool {
        // GPT-4o supports vision
        self.model.contains("gpt-4o") || self.model.contains("4o")
    }

    async fn analyze_image(
        &self,
        base64_image: &str,
        prompt: &str,
    ) -> Result<String, ProviderError> {
        if !self.supports_vision() {
            return Err(ProviderError::VisionNotSupported(self.name().to_string()));
        }

        let url = format!("{}/chat/completions", self.base_url);

        let body = serde_json::json!({
            "model": self.model,
            "messages": [
                {
                    "role": "user",
                    "content": [
                        {"type": "text", "text": prompt},
                        {
                            "type": "image_url",
                            "image_url": {
                                "url": format!("data:image/png;base64,{}", base64_image)
                            }
                        }
                    ]
                }
            ],
            "max_tokens": 4096,
        });

        let response = self
            .client
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.api_key))
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

        let response: OpenAIResponse = response.json().await?;

        let content = response
            .choices
            .first()
            .and_then(|c| c.message.content.clone())
            .ok_or_else(|| ProviderError::APIError("No content in response".to_string()))?;

        Ok(content)
    }

    fn info(&self) -> ProviderInfo {
        ProviderInfo {
            name: self.name().to_string(),
            model: self.model.clone(),
            supports_vision: self.supports_vision(),
            supports_streaming: true,
            context_window: 128000,
            max_tokens: 16384,
        }
    }
}

#[derive(Debug, Deserialize)]
struct OpenAIResponse {
    choices: Vec<Choice>,
}

#[derive(Debug, Deserialize)]
struct Choice {
    message: Message,
}

#[derive(Debug, Deserialize)]
struct Message {
    content: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_client_creation() {
        let client = OpenAIClient::new("test-key", "gpt-4o", None);
        assert!(client.is_ok());

        let client = client.unwrap();
        assert_eq!(client.name(), "openai");
        assert_eq!(client.model(), "gpt-4o");
    }

    #[test]
    fn test_custom_base_url() {
        let client = OpenAIClient::new("test-key", "gpt-4o", Some("http://localhost:8080/v1"));
        assert!(client.is_ok());

        let client = client.unwrap();
        assert!(client.base_url.contains("localhost:8080"));
    }
}
