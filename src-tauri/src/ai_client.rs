//! Gemini AI client for screen analysis and semantic similarity
//! Uses Google's Gemini API for vision and text understanding

use anyhow::Result;
use reqwest::Client;
use serde::{Deserialize, Serialize};

pub struct GeminiClient {
    client: Client,
    api_key: String,
}

#[derive(Debug, Serialize)]
struct GeminiRequest {
    contents: Vec<Content>,
    #[serde(rename = "generationConfig", skip_serializing_if = "Option::is_none")]
    generation_config: Option<GenerationConfig>,
}

#[derive(Debug, Serialize)]
struct Content {
    parts: Vec<Part>,
}

#[derive(Debug, Serialize)]
#[serde(untagged)]
enum Part {
    Text { text: String },
    Image { inline_data: InlineData },
}

#[derive(Debug, Serialize)]
struct InlineData {
    mime_type: String,
    data: String,
}

#[derive(Debug, Serialize)]
struct GenerationConfig {
    temperature: f32,
    #[serde(rename = "maxOutputTokens")]
    max_output_tokens: u32,
}

#[derive(Debug, Deserialize)]
struct GeminiResponse {
    candidates: Option<Vec<Candidate>>,
    error: Option<GeminiError>,
}

#[derive(Debug, Deserialize)]
struct Candidate {
    content: ContentResponse,
}

#[derive(Debug, Deserialize)]
struct ContentResponse {
    parts: Vec<PartResponse>,
}

#[derive(Debug, Deserialize)]
struct PartResponse {
    text: String,
}

#[derive(Debug, Deserialize)]
struct GeminiError {
    message: String,
}

impl GeminiClient {
    pub fn new(api_key: String) -> Self {
        Self {
            client: Client::new(),
            api_key,
        }
    }

    fn get_api_url(&self) -> String {
        format!(
            "https://generativelanguage.googleapis.com/v1beta/models/gemini-1.5-flash:generateContent?key={}",
            self.api_key
        )
    }

    /// Analyze screenshot with Gemini Vision
    pub async fn analyze_image(&self, base64_image: &str, prompt: &str) -> Result<String> {
        if self.api_key.is_empty() {
            return Ok("AI Analysis unavailable (No API Key)".to_string());
        }

        let request = GeminiRequest {
            contents: vec![Content {
                parts: vec![
                    Part::Text {
                        text: prompt.to_string(),
                    },
                    Part::Image {
                        inline_data: InlineData {
                            mime_type: "image/png".to_string(),
                            data: base64_image.to_string(),
                        },
                    },
                ],
            }],
            generation_config: Some(GenerationConfig {
                temperature: 0.7,
                max_output_tokens: 500,
            }),
        };

        let response = self
            .client
            .post(&self.get_api_url())
            .json(&request)
            .send()
            .await?
            .json::<GeminiResponse>()
            .await?;

        if let Some(error) = response.error {
            return Err(anyhow::anyhow!("Gemini API error: {}", error.message));
        }

        let candidates = response
            .candidates
            .ok_or_else(|| anyhow::anyhow!("No candidates in response"))?;

        let text = candidates
            .first()
            .map(|c| c.content.parts.first().map(|p| p.text.clone()))
            .flatten()
            .ok_or_else(|| anyhow::anyhow!("No text in response"))?;

        Ok(text)
    }

    /// Calculate semantic similarity between two URLs (returns 0.0-1.0)
    pub async fn calculate_url_similarity(&self, url1: &str, url2: &str) -> Result<f32> {
        if self.api_key.is_empty() {
            return Ok(0.0);
        }

        let prompt = format!(
            "Compare these two URLs semantically. Consider the topic, domain, and content they represent.
            Return ONLY a single number between 0.0 and 1.0 representing their similarity.
            0.0 means completely unrelated, 1.0 means identical or very closely related.
            
            URL1: {}
            URL2: {}
            
            Respond with just the number, nothing else.",
            url1, url2
        );

        let request = GeminiRequest {
            contents: vec![Content {
                parts: vec![Part::Text { text: prompt }],
            }],
            generation_config: Some(GenerationConfig {
                temperature: 0.1,
                max_output_tokens: 10,
            }),
        };

        let response = self
            .client
            .post(&self.get_api_url())
            .json(&request)
            .send()
            .await?
            .json::<GeminiResponse>()
            .await?;

        if let Some(error) = response.error {
            return Err(anyhow::anyhow!("Gemini API error: {}", error.message));
        }

        let candidates = response
            .candidates
            .ok_or_else(|| anyhow::anyhow!("No candidates"))?;

        let text = candidates
            .first()
            .map(|c| c.content.parts.first().map(|p| p.text.clone()))
            .flatten()
            .ok_or_else(|| anyhow::anyhow!("No text in response"))?;

        let similarity = text.trim().parse::<f32>().unwrap_or(0.0);

        Ok(similarity.clamp(0.0, 1.0))
    }

    /// Generate Ghost dialogue based on context
    pub async fn generate_dialogue(&self, context: &str, personality: &str) -> Result<String> {
        if self.api_key.is_empty() {
            return Ok("...".to_string());
        }

        let prompt = format!(
            "You are a mysterious AI ghost character with this personality: {}
            
            Based on this context about what the user is viewing: {}
            
            Generate a short, cryptic, but helpful hint or observation (max 100 characters).
            Stay in character. Be intriguing but not annoying.",
            personality, context
        );

        let request = GeminiRequest {
            contents: vec![Content {
                parts: vec![Part::Text { text: prompt }],
            }],
            generation_config: Some(GenerationConfig {
                temperature: 0.9,
                max_output_tokens: 50,
            }),
        };

        let response = self
            .client
            .post(&self.get_api_url())
            .json(&request)
            .send()
            .await?
            .json::<GeminiResponse>()
            .await?;

        if let Some(error) = response.error {
            return Err(anyhow::anyhow!("Gemini API error: {}", error.message));
        }

        let candidates = response
            .candidates
            .ok_or_else(|| anyhow::anyhow!("No candidates"))?;

        let text = candidates
            .first()
            .map(|c| c.content.parts.first().map(|p| p.text.clone()))
            .flatten()
            .ok_or_else(|| anyhow::anyhow!("No text in response"))?;

        Ok(text.trim().to_string())
    }

    /// Generate a dynamic puzzle based on current page context
    /// Creates unique puzzles based on what the user is currently viewing
    pub async fn generate_dynamic_puzzle(
        &self,
        url: &str,
        page_title: &str,
        page_content: &str,
    ) -> Result<DynamicPuzzle> {
        let prompt = format!(
            r#"Based on this webpage the user is viewing, generate a creative puzzle for a mystery game.

URL: {}
Title: {}
Content snippet: {}

Generate a JSON object with these fields:
- "clue": A mysterious, cryptic clue that relates to this page's topic but leads to a DIFFERENT related page (max 100 chars)
- "target_description": What the player should find (a related but different topic/page)
- "target_url_pattern": A regex pattern that would match the solution URL
- "hints": An array of 3 progressive hints

Example response format (respond ONLY with valid JSON, no markdown):
{{"clue": "The cipher machine's nemesis worked at a park...", "target_description": "Alan Turing Bletchley Park", "target_url_pattern": "(turing|bletchley)", "hints": ["Think about who cracked the code...", "A British mathematician...", "Search for Alan Turing"]}}

Make the puzzle interesting and educational. The target should be related but not the same page."#,
            url,
            page_title,
            &page_content.chars().take(500).collect::<String>()
        );

        let request = GeminiRequest {
            contents: vec![Content {
                parts: vec![Part::Text { text: prompt }],
            }],
            generation_config: Some(GenerationConfig {
                temperature: 0.8,
                max_output_tokens: 300,
            }),
        };

        let response = self
            .client
            .post(&self.get_api_url())
            .json(&request)
            .send()
            .await?
            .json::<GeminiResponse>()
            .await?;

        if let Some(error) = response.error {
            return Err(anyhow::anyhow!("Gemini API error: {}", error.message));
        }

        let candidates = response
            .candidates
            .ok_or_else(|| anyhow::anyhow!("No candidates"))?;

        let text = candidates
            .first()
            .map(|c| c.content.parts.first().map(|p| p.text.clone()))
            .flatten()
            .ok_or_else(|| anyhow::anyhow!("No text in response"))?;

        // Parse JSON response
        let puzzle: DynamicPuzzle = serde_json::from_str(text.trim())
            .map_err(|e| anyhow::anyhow!("Failed to parse puzzle JSON: {} - Raw: {}", e, text))?;

        Ok(puzzle)
    }
}

/// A dynamically generated puzzle based on screen context
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DynamicPuzzle {
    pub clue: String,
    pub target_description: String,
    pub target_url_pattern: String,
    pub hints: Vec<String>,
}
