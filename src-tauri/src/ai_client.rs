//! Gemini AI client for screen analysis and semantic similarity
//! Uses Google's Gemini API for vision and text understanding

use anyhow::Result;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

/// Rate limiting configuration
const MAX_REQUESTS_PER_MINUTE: u64 = 10;
const RATE_LIMIT_WINDOW_SECS: u64 = 60;

pub struct GeminiClient {
    client: Client,
    api_key: String,
    /// Timestamp of window start (seconds since epoch)
    rate_limit_window_start: AtomicU64,
    /// Request count in current window
    request_count: AtomicU64,
}

#[derive(Debug, Serialize)]
struct GeminiRequest {
    contents: Vec<Content>,
    #[serde(rename = "generationConfig", skip_serializing_if = "Option::is_none")]
    generation_config: Option<GenerationConfig>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tools: Option<Vec<Tool>>,
}

#[derive(Debug, Serialize)]
struct Tool {
    #[serde(rename = "googleSearch")]
    google_search: GoogleSearch,
}

#[derive(Debug, Serialize)]
struct GoogleSearch {}

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
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        Self {
            client: Client::new(),
            api_key,
            rate_limit_window_start: AtomicU64::new(now),
            request_count: AtomicU64::new(0),
        }
    }

    /// Check and update rate limit, returns true if request is allowed
    fn check_rate_limit(&self) -> bool {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        let window_start = self.rate_limit_window_start.load(Ordering::SeqCst);

        // If window has expired, try to reset atomically
        if now.saturating_sub(window_start) >= RATE_LIMIT_WINDOW_SECS {
            // Try to be the one that resets the window
            if self
                .rate_limit_window_start
                .compare_exchange(window_start, now, Ordering::SeqCst, Ordering::SeqCst)
                .is_ok()
            {
                self.request_count.store(1, Ordering::SeqCst);
                return true;
            }
            // Another thread reset it, re-check
            return self.check_rate_limit();
        }

        // Check current count BEFORE incrementing
        let current = self.request_count.load(Ordering::SeqCst);
        if current >= MAX_REQUESTS_PER_MINUTE {
            return false;
        }

        // Try to increment atomically
        if self
            .request_count
            .compare_exchange(current, current + 1, Ordering::SeqCst, Ordering::SeqCst)
            .is_ok()
        {
            true
        } else {
            // Race condition, re-check
            self.check_rate_limit()
        }
    }

    /// Wait for rate limit availability with simple backoff
    /// Returns false if max attempts exceeded
    async fn wait_for_rate_limit(&self) -> bool {
        const MAX_WAIT_ATTEMPTS: u32 = 12; // Max ~1 minute of waiting
        let mut attempts = 0;

        loop {
            if self.check_rate_limit() {
                return true;
            }

            attempts += 1;
            if attempts > MAX_WAIT_ATTEMPTS {
                tracing::error!("Rate limit: max wait attempts exceeded, dropping request");
                return false;
            }

            let now = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs();
            let window_start = self.rate_limit_window_start.load(Ordering::SeqCst);

            // Calculate time until window reset
            let elapsed = now.saturating_sub(window_start);
            let wait_secs = if elapsed < RATE_LIMIT_WINDOW_SECS {
                RATE_LIMIT_WINDOW_SECS - elapsed
            } else {
                1
            };

            // Cap max wait
            let wait_secs = wait_secs.min(5).max(1);

            // Only log every few attempts to reduce spam
            if attempts == 1 || attempts % 4 == 0 {
                tracing::warn!(
                    "Rate limit hit (attempt {}/{}), waiting {}s...",
                    attempts,
                    MAX_WAIT_ATTEMPTS,
                    wait_secs
                );
            }

            tokio::time::sleep(std::time::Duration::from_secs(wait_secs)).await;
        }
    }

    fn get_api_url(&self) -> String {
        format!(
            "https://generativelanguage.googleapis.com/v1beta/models/gemini-2.0-flash:generateContent?key={}",
            self.api_key
        )
    }

    /// Analyze screenshot with Gemini Vision
    pub async fn analyze_image(&self, base64_image: &str, prompt: &str) -> Result<String> {
        if self.api_key.is_empty() {
            return Ok("AI Analysis unavailable (No API Key)".to_string());
        }

        if !self.wait_for_rate_limit().await {
            return Err(anyhow::anyhow!("Rate limit exceeded, request dropped"));
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
            tools: None,
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

        if !self.wait_for_rate_limit().await {
            return Ok(0.0); // Return no similarity if rate limited
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
            tools: None,
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

        if !self.wait_for_rate_limit().await {
            return Ok("...".to_string()); // Return placeholder if rate limited
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
            tools: None,
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
        history_context: &str,
    ) -> Result<DynamicPuzzle> {
        tracing::info!(
            "Generating dynamic puzzle for URL: {} (title: {})",
            url,
            page_title
        );

        if !self.wait_for_rate_limit().await {
            return Err(anyhow::anyhow!(
                "Rate limit exceeded, puzzle generation dropped"
            ));
        }

        let prompt = format!(
            r#"Based on this webpage the user is viewing, generate a creative puzzle for a mystery game.
            Use Google Search to find a connection to a real-world event, person, or historical fact related to this page's topic.
            
            Also consider the user's recent browsing history (provided below) to see if you can make a thematic connection to their recent interests, but prioritize the CURRENT page for the clue.

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

Example response format (respond ONLY with valid JSON, no markdown):
{{"clue": "The cipher machine's nemesis worked at a park...", "target_description": "Alan Turing Bletchley Park", "target_url_pattern": "(turing|bletchley)", "hints": ["Think about who cracked the code...", "A British mathematician...", "Search for Alan Turing"]}}

Make the puzzle interesting and educational. The target should be related but not the same page."#,
            url,
            page_title,
            &page_content.chars().take(500).collect::<String>(),
            history_context
        );

        let request = GeminiRequest {
            contents: vec![Content {
                parts: vec![Part::Text { text: prompt }],
            }],
            tools: Some(vec![Tool {
                google_search: GoogleSearch {},
            }]),
            generation_config: Some(GenerationConfig {
                temperature: 0.8,
                max_output_tokens: 300,
            }),
        };

        tracing::debug!("Sending request to Gemini API...");

        let response = self
            .client
            .post(&self.get_api_url())
            .json(&request)
            .send()
            .await?
            .json::<GeminiResponse>()
            .await?;

        tracing::debug!("Gemini response received: {:?}", response);

        if let Some(error) = response.error {
            tracing::error!("Gemini API error: {}", error.message);
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

        tracing::debug!("Raw AI response text: {}", text);

        // Clean up markdown code blocks if present
        let clean_text = text
            .trim()
            .trim_start_matches("```json")
            .trim_start_matches("```")
            .trim_end_matches("```")
            .trim();

        // Parse JSON response
        let puzzle: DynamicPuzzle = serde_json::from_str(clean_text).map_err(|e| {
            tracing::error!("Failed to parse puzzle JSON: {} - Raw: {}", e, text);
            anyhow::anyhow!("Failed to parse puzzle JSON: {} - Raw: {}", e, text)
        })?;

        tracing::info!("Successfully generated puzzle: {:?}", puzzle.clue);

        Ok(puzzle)
    }

    /// Verify if a screenshot contains the solution to a puzzle
    pub async fn verify_screenshot_clue(
        &self,
        base64_image: &str,
        clue_description: &str,
    ) -> Result<VerificationResult> {
        if self.api_key.is_empty() {
            return Err(anyhow::anyhow!("No API Key configured"));
        }

        if !self.wait_for_rate_limit().await {
            return Err(anyhow::anyhow!("Rate limit exceeded, verification dropped"));
        }

        let prompt = format!(
            "Analyze this screenshot. Does it contain content matching this description: '{}'?
            
            Respond with a JSON object:
            {{
                \"found\": boolean,
                \"confidence\": number (0.0-1.0),
                \"explanation\": \"Short explanation of what was found or missing\"
            }}
            
            Be strict and accurate. Only return true if the visual proof CLEARLY matches the specific target description.",
            clue_description
        );

        let request = GeminiRequest {
            contents: vec![Content {
                parts: vec![
                    Part::Text { text: prompt },
                    Part::Image {
                        inline_data: InlineData {
                            mime_type: "image/png".to_string(),
                            data: base64_image.to_string(),
                        },
                    },
                ],
            }],
            generation_config: Some(GenerationConfig {
                temperature: 0.2, // Lower temperature for more objective analysis
                max_output_tokens: 200,
            }),
            tools: None,
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

        let clean_text = text
            .trim()
            .trim_start_matches("```json")
            .trim_start_matches("```")
            .trim_end_matches("```")
            .trim();

        let result: VerificationResult = serde_json::from_str(clean_text)?;
        Ok(result)
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

/// Result of screenshot verification
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerificationResult {
    pub found: bool,
    pub confidence: f32,
    pub explanation: String,
}
