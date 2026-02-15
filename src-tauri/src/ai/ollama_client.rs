//! Ollama AI client for local LLM inference
//! Communicates with Ollama server via HTTP API at localhost:11434

use crate::core::utils::{clean_json_response, runtime_config};
use anyhow::{Context, Result};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::time::Duration;

/// Ollama client for local LLM inference
/// Configuration is read dynamically from RuntimeConfig to support runtime changes
pub struct OllamaClient {
    client: Client,
}

// ============================================================================
// Request/Response Types
// ============================================================================

#[derive(Debug, Serialize)]
struct OllamaGenerateRequest {
    model: String,
    prompt: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    images: Option<Vec<String>>,
    stream: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    format: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    options: Option<OllamaOptions>,
}

#[derive(Debug, Serialize)]
struct OllamaOptions {
    temperature: f32,
    #[serde(skip_serializing_if = "Option::is_none")]
    num_predict: Option<u32>,
}

#[derive(Debug, Deserialize)]
struct OllamaGenerateResponse {
    response: String,
}

#[derive(Debug, Deserialize)]
struct OllamaTagsResponse {
    models: Vec<OllamaModel>,
}

#[derive(Debug, Deserialize)]
struct OllamaModel {
    name: String,
}

// ============================================================================
// OllamaClient Implementation
// ============================================================================

impl OllamaClient {
    /// Create a new Ollama client
    /// Configuration is read dynamically from RuntimeConfig for each request
    pub fn new() -> Self {
        let client = Client::builder()
            .timeout(Duration::from_secs(180)) // Longer timeout for local inference
            .build()
            .unwrap_or_else(|e| {
                // Extremely unlikely, but avoid panicking in production.
                tracing::error!("Failed to build HTTP client, using default client: {}", e);
                Client::new()
            });

        Self { client }
    }

    /// Get current base URL from runtime config
    fn base_url(&self) -> String {
        runtime_config().get_ollama_url()
    }

    /// Get current vision model from runtime config
    fn vision_model(&self) -> String {
        runtime_config().get_ollama_vision_model()
    }

    /// Get current text model from runtime config
    fn text_model(&self) -> String {
        runtime_config().get_ollama_text_model()
    }

    /// Check if Ollama server is running and accessible
    pub async fn is_available(&self) -> bool {
        let url = format!("{}/api/tags", self.base_url());
        match self
            .client
            .get(&url)
            .timeout(Duration::from_secs(3))
            .send()
            .await
        {
            Ok(resp) => resp.status().is_success(),
            Err(_) => false,
        }
    }

    /// Check if a specific model is available
    pub async fn has_model(&self, model_name: &str) -> bool {
        let url = format!("{}/api/tags", self.base_url());
        match self.client.get(&url).send().await {
            Ok(resp) => {
                if let Ok(tags) = resp.json::<OllamaTagsResponse>().await {
                    tags.models.iter().any(|m| m.name.starts_with(model_name))
                } else {
                    false
                }
            }
            Err(_) => false,
        }
    }

    /// Check if vision model is available
    pub async fn has_vision_model(&self) -> bool {
        self.has_model(&self.vision_model()).await
    }

    /// List available models (for UI display)
    pub async fn list_models(&self) -> Result<Vec<String>> {
        let url = format!("{}/api/tags", self.base_url());
        let resp = self
            .client
            .get(&url)
            .timeout(Duration::from_secs(5))
            .send()
            .await
            .context("Failed to connect to Ollama")?;

        let tags: OllamaTagsResponse = resp.json().await.context("Failed to parse model list")?;
        Ok(tags.models.into_iter().map(|m| m.name).collect())
    }

    /// Generate text from a prompt (text-only, uses faster model)
    pub async fn generate_text(&self, prompt: &str) -> Result<String> {
        self.generate_internal(prompt, None, Some(0.7), Some(500), false)
            .await
    }

    /// Generate text with JSON format enforcement
    pub async fn generate_json(&self, prompt: &str) -> Result<String> {
        self.generate_internal(prompt, None, Some(0.7), Some(500), true)
            .await
    }

    /// Analyze an image with a prompt (uses vision model)
    pub async fn analyze_image(&self, base64_image: &str, prompt: &str) -> Result<String> {
        self.generate_with_image(prompt, base64_image, Some(0.7), Some(500))
            .await
    }

    /// Generate with image (vision model)
    async fn generate_with_image(
        &self,
        prompt: &str,
        base64_image: &str,
        temperature: Option<f32>,
        max_tokens: Option<u32>,
    ) -> Result<String> {
        let url = format!("{}/api/generate", self.base_url());
        let vision_model = self.vision_model();

        let request = OllamaGenerateRequest {
            model: vision_model.clone(),
            prompt: prompt.to_string(),
            images: Some(vec![base64_image.to_string()]),
            stream: false,
            format: None,
            options: Some(OllamaOptions {
                temperature: temperature.unwrap_or(0.7),
                num_predict: max_tokens,
            }),
        };

        tracing::debug!(
            "Ollama vision request to {} with model {}",
            url,
            vision_model
        );

        let response = self
            .client
            .post(&url)
            .json(&request)
            .send()
            .await
            .context("Failed to connect to Ollama server")?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(anyhow::anyhow!("Ollama API error {}: {}", status, body));
        }

        let result: OllamaGenerateResponse = response
            .json()
            .await
            .context("Failed to parse Ollama response")?;

        Ok(result.response.trim().to_string())
    }

    /// Internal generate function
    async fn generate_internal(
        &self,
        prompt: &str,
        images: Option<Vec<String>>,
        temperature: Option<f32>,
        max_tokens: Option<u32>,
        json_format: bool,
    ) -> Result<String> {
        let url = format!("{}/api/generate", self.base_url());

        // Use vision model if images provided, otherwise use text model
        let model = if images.is_some() {
            self.vision_model()
        } else {
            self.text_model()
        };

        let request = OllamaGenerateRequest {
            model: model.clone(),
            prompt: prompt.to_string(),
            images,
            stream: false,
            format: if json_format {
                Some("json".to_string())
            } else {
                None
            },
            options: Some(OllamaOptions {
                temperature: temperature.unwrap_or(0.7),
                num_predict: max_tokens,
            }),
        };

        tracing::debug!("Ollama text request to {} with model {}", url, model);

        let response = self
            .client
            .post(&url)
            .json(&request)
            .send()
            .await
            .context("Failed to connect to Ollama server")?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(anyhow::anyhow!("Ollama API error {}: {}", status, body));
        }

        let result: OllamaGenerateResponse = response
            .json()
            .await
            .context("Failed to parse Ollama response")?;

        let text = result.response.trim().to_string();

        // Clean JSON response if needed
        if json_format {
            Ok(clean_json_response(&text).to_string())
        } else {
            Ok(text)
        }
    }

    /// Calculate semantic similarity between two URLs (simplified local version)
    pub async fn calculate_url_similarity(&self, url1: &str, url2: &str) -> Result<f32> {
        let prompt = format!(
            "Compare these two URLs semantically. Consider the topic, domain, and content they represent.
            Return ONLY a single number between 0.0 and 1.0 representing their similarity.
            0.0 means completely unrelated, 1.0 means identical or very closely related.
            
            URL1: {}
            URL2: {}
            
            Respond with just the number, nothing else.",
            url1, url2
        );

        let text = self
            .generate_internal(&prompt, None, Some(0.1), Some(10), false)
            .await?;

        let similarity = text.trim().parse::<f32>().unwrap_or(0.0);
        Ok(similarity.clamp(0.0, 1.0))
    }

    /// Generate dialogue based on context
    pub async fn generate_dialogue(&self, context: &str, personality: &str) -> Result<String> {
        let prompt = format!(
            "You are a desktop companion. Your personality is: {}
            
            Based on this context about what the user is viewing: {}
            
            Generate a short, helpful, or intriguing comment (max 100 characters).
            If in 'Mystery' mode, be cryptic. If in 'Companion' mode, be helpful but concise.
            Stay in character.",
            personality, context
        );

        self.generate_internal(&prompt, None, Some(0.9), Some(50), false)
            .await
    }

    /// Generate a dynamic puzzle based on page context
    pub async fn generate_dynamic_puzzle(
        &self,
        url: &str,
        page_title: &str,
        page_content: &str,
        history_context: &str,
    ) -> Result<crate::ai::gemini_client::DynamicPuzzle> {
        tracing::info!(
            "Ollama: Generating dynamic puzzle for URL: {} (title: {})",
            url,
            page_title
        );

        let prompt = format!(
            r#"Based on this webpage the user is viewing, generate a creative puzzle for a mystery game.

URL: {}
Title: {}
Content snippet: {}

Recent Browsing History:
{}

Generate a JSON object with these fields:
- "clue": A mysterious, cryptic clue that relates to this page's topic but leads to a DIFFERENT related page (max 100 chars)
- "target_description": What the player should find (a related but different topic/page)
- "target_url_pattern": A regex pattern that would match the solution URL
- "hints": An array of 3 progressive hints

Respond ONLY with valid JSON, no markdown."#,
            url,
            page_title,
            &page_content.chars().take(500).collect::<String>(),
            history_context
        );

        let text = self.generate_json(&prompt).await?;

        let puzzle: crate::ai::gemini_client::DynamicPuzzle = serde_json::from_str(&text)
            .context(format!("Failed to parse puzzle JSON. Raw: {}", text))?;

        tracing::info!("Ollama: Successfully generated puzzle: {:?}", puzzle.clue);

        Ok(puzzle)
    }

    /// Verify if a screenshot contains the solution to a puzzle
    pub async fn verify_screenshot_clue(
        &self,
        base64_image: &str,
        clue_description: &str,
    ) -> Result<crate::ai::gemini_client::VerificationResult> {
        let prompt = format!(
            r#"Analyze this screenshot. Does it contain content matching this description: '{}'?
            
Respond with a JSON object:
{{
    "found": boolean,
    "confidence": number (0.0-1.0),
    "explanation": "Short explanation of what was found or missing"
}}

Be strict and accurate. Only return true if the visual proof CLEARLY matches the specific target description."#,
            clue_description
        );

        let text = self
            .generate_with_image(&prompt, base64_image, Some(0.2), Some(200))
            .await?;
        let clean_text = clean_json_response(&text);

        let result: crate::ai::gemini_client::VerificationResult =
            serde_json::from_str(clean_text)?;
        Ok(result)
    }

    /// Generate adaptive puzzle based on user activities (simplified version for local LLM)
    pub async fn generate_adaptive_puzzle(
        &self,
        activities: &[crate::ai::gemini_client::ActivityContext],
        current_app: Option<&str>,
        _current_content: Option<&str>,
    ) -> Result<crate::ai::gemini_client::AdaptivePuzzle> {
        // Build activity summary
        let activity_summary = activities
            .iter()
            .take(5)
            .map(|a| format!("- {} ({}): {}", a.app_name, a.app_category, a.description))
            .collect::<Vec<_>>()
            .join("\n");

        let app_context = current_app.unwrap_or("desktop");

        let prompt = format!(
            r#"Based on the user's recent activities, generate a puzzle for a mystery game.

Recent Activities:
{}

Current App: {}

Generate a JSON object with these fields:
- "puzzle_type": One of "navigation", "discovery", "knowledge"
- "clue": A mysterious clue related to their activities (max 80 chars)
- "target_description": What they should find
- "hints": Array of 2 progressive hints
- "difficulty": 1-5 (based on how related to their activities)

Respond ONLY with valid JSON."#,
            activity_summary, app_context
        );

        let text = self.generate_json(&prompt).await?;

        let puzzle: crate::ai::gemini_client::AdaptivePuzzle = serde_json::from_str(&text)
            .context(format!(
                "Failed to parse adaptive puzzle JSON. Raw: {}",
                text
            ))?;

        Ok(puzzle)
    }

    /// Generate contextual dialogue based on activities
    pub async fn generate_contextual_dialogue(
        &self,
        activities: &[crate::ai::gemini_client::ActivityContext],
        current_context: &str,
        ghost_mood: &str,
    ) -> Result<String> {
        let activity_summary = activities
            .iter()
            .take(3)
            .map(|a| format!("{}: {}", a.app_name, a.description))
            .collect::<Vec<_>>()
            .join("; ");

        let prompt = format!(
            r#"You are a mysterious desktop companion ghost. Your mood is: {}.

Recent user activity: {}
Current context: {}

Generate a short, in-character comment (max 60 chars). Be cryptic if mysterious, helpful if friendly."#,
            ghost_mood,
            if activity_summary.is_empty() {
                "watching the user".to_string()
            } else {
                activity_summary
            },
            current_context
        );

        self.generate_internal(&prompt, None, Some(0.9), Some(40), false)
            .await
    }
}

impl Default for OllamaClient {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ollama_client_creation() {
        let client = OllamaClient::new();
        // Client reads from runtime config - default values should be returned
        assert!(!client.base_url().is_empty());
        assert!(!client.vision_model().is_empty());
        assert!(!client.text_model().is_empty());
    }

    #[test]
    fn test_runtime_config_defaults() {
        // Verify defaults from runtime config
        let config = runtime_config();
        assert_eq!(
            config.get_ollama_url(),
            crate::core::utils::DEFAULT_OLLAMA_URL
        );
        assert_eq!(
            config.get_ollama_vision_model(),
            crate::core::utils::DEFAULT_OLLAMA_VISION_MODEL
        );
        assert_eq!(
            config.get_ollama_text_model(),
            crate::core::utils::DEFAULT_OLLAMA_TEXT_MODEL
        );
    }
}
