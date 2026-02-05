//! Gemini AI client for screen analysis and semantic similarity
//! Uses Google's Gemini API for vision and text understanding

use crate::core::utils::clean_json_response;
use anyhow::{Context, Result};
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
        Self {
            client: Client::new(),
            api_key,
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
            .post(self.get_api_url())
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
            .and_then(|c| c.content.parts.first().map(|p| p.text.clone()))
            .ok_or_else(|| anyhow::anyhow!("No text in response"))?;

        Ok(text)
    }

    /// Generate raw text from a prompt
    pub async fn generate_text(&self, prompt: &str) -> Result<String> {
        if self.api_key.is_empty() {
            return Ok(String::new());
        }


        let request = GeminiRequest {
            contents: vec![Content {
                parts: vec![Part::Text {
                    text: prompt.to_string(),
                }],
            }],
            generation_config: Some(GenerationConfig {
                temperature: 0.7,
                max_output_tokens: 500,
            }),
            tools: None,
        };

        let response = self
            .client
            .post(self.get_api_url())
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
            .and_then(|c| c.content.parts.first().map(|p| p.text.clone()))
            .ok_or_else(|| anyhow::anyhow!("No text in response"))?;

        Ok(text.trim().to_string())
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

        // We can use the internal logic here or just keep as is to avoid breaking changes if specific config needed
        // For now, keeping as is but improved structure
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
            .post(self.get_api_url())
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
            .and_then(|c| c.content.parts.first().map(|p| p.text.clone()))
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
            "You are a desktop companion. Your personality is: {}
            
            Based on this context about what the user is viewing: {}
            
            Generate a short, helpful, or intriguing comment (max 100 characters).
            If in 'Mystery' mode, be cryptic. If in 'Companion' mode, be helpful but concise.
            Stay in character.",
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
            .post(self.get_api_url())
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
            .and_then(|c| c.content.parts.first().map(|p| p.text.clone()))
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
            .post(self.get_api_url())
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
            .and_then(|c| c.content.parts.first().map(|p| p.text.clone()))
            .ok_or_else(|| anyhow::anyhow!("No text in response"))?;

        tracing::debug!("Raw AI response text: {}", text);

        // Clean up markdown code blocks if present
        let clean_text = clean_json_response(&text);

        // Parse JSON response
        let puzzle: DynamicPuzzle = serde_json::from_str(clean_text)
            .context(format!("Failed to parse puzzle JSON. Raw: {}", text))?;

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
            .post(self.get_api_url())
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
            .and_then(|c| c.content.parts.first().map(|p| p.text.clone()))
            .ok_or_else(|| anyhow::anyhow!("No text in response"))?;

        let clean_text = clean_json_response(&text);

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

/// An adaptive puzzle generated from observed user activity
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdaptivePuzzle {
    /// The puzzle clue
    pub clue: String,
    /// What the player should find
    pub target_description: String,
    /// Regex pattern for matching solution
    pub target_url_pattern: String,
    /// Progressive hints
    pub hints: Vec<String>,
    /// The activity context that inspired this puzzle
    pub inspired_by: String,
    /// Puzzle difficulty (1-5)
    pub difficulty: u8,
    /// Theme/category of the puzzle
    pub theme: String,
}

// ============================================================================
// Adaptive Behavior Methods
// ============================================================================

impl GeminiClient {
    /// Generate an adaptive puzzle based on user's observed activity patterns
    /// Uses activity history to create contextually relevant puzzles
    pub async fn generate_adaptive_puzzle(
        &self,
        activities: &[ActivityContext],
        current_app: Option<&str>,
        current_content: Option<&str>,
    ) -> Result<AdaptivePuzzle> {
        if self.api_key.is_empty() {
            return Err(anyhow::anyhow!("No API Key configured"));
        }


        // Build activity context string
        let activity_summary = activities
            .iter()
            .map(|a| format!("- {} ({}): {}", a.app_category, a.app_name, a.description))
            .collect::<Vec<_>>()
            .join("\n");

        let current_context = match (current_app, current_content) {
            (Some(app), Some(content)) => format!("Currently using {} viewing {}", app, content),
            (Some(app), None) => format!("Currently using {}", app),
            _ => "No current context".to_string(),
        };

        let prompt = format!(
            r#"You are creating an educational puzzle for a desktop companion game. 
The user's recent activity shows their interests. Create a puzzle that connects to what they've been doing.

Recent User Activity:
{}

Current Context: {}

Create a fun, educational puzzle that:
1. Relates to topics from their activity
2. Leads them to discover something new but related
3. Has a clear, verifiable answer (a webpage they can find)

Respond with ONLY valid JSON (no markdown):
{{
    "clue": "A mysterious, engaging clue (max 100 chars)",
    "target_description": "What they should find",
    "target_url_pattern": "regex pattern for solution URL",
    "hints": ["hint 1", "hint 2", "hint 3"],
    "inspired_by": "which activity inspired this",
    "difficulty": 2,
    "theme": "category (history, science, tech, culture, etc.)"
}}"#,
            activity_summary, current_context
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
                max_output_tokens: 400,
            }),
        };

        let response = self
            .client
            .post(self.get_api_url())
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
            .and_then(|c| c.content.parts.first().map(|p| p.text.clone()))
            .ok_or_else(|| anyhow::anyhow!("No text in response"))?;

        let clean_text = clean_json_response(&text);

        let puzzle: AdaptivePuzzle = serde_json::from_str(clean_text).context(format!(
            "Failed to parse adaptive puzzle JSON. Raw: {}",
            clean_text
        ))?;

        tracing::info!(
            "Generated adaptive puzzle inspired by '{}': {}",
            puzzle.inspired_by,
            puzzle.clue
        );

        Ok(puzzle)
    }

    /// Generate contextual dialogue based on observation history
    pub async fn generate_contextual_dialogue(
        &self,
        recent_activities: &[ActivityContext],
        current_context: &str,
        ghost_mood: &str,
    ) -> Result<String> {
        if self.api_key.is_empty() {
            return Ok("...".to_string());
        }



        let activity_summary = recent_activities
            .iter()
            .take(5)
            .map(|a| format!("{} ({})", a.description, a.app_name))
            .collect::<Vec<_>>()
            .join(", ");

        let prompt = format!(
            r#"You are a mysterious AI companion ghost. Generate a short, contextual comment.

Ghost mood: {}
Recent user activities: {}
Current context: {}

The ghost should:
- Reference what the user has been doing naturally
- Be helpful yet mysterious
- Keep it under 80 characters
- Stay in character

Respond with ONLY the dialogue text, no quotes or formatting."#,
            ghost_mood, activity_summary, current_context
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
            .post(self.get_api_url())
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
            .and_then(|c| c.content.parts.first().map(|p| p.text.clone()))
            .ok_or_else(|| anyhow::anyhow!("No text"))?;

        Ok(text.trim().replace('"', ""))
    }
}

/// Context about a user activity for adaptive puzzle generation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActivityContext {
    pub app_name: String,
    pub app_category: String,
    pub description: String,
    pub content_context: Option<String>,
}
