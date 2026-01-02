//! Observer Agent - Screen analysis and vision
//! Analyzes what the user is viewing and extracts semantic meaning

use super::traits::{Agent, AgentContext, AgentError, AgentOutput, AgentResult, NextAction};
use crate::ai_client::GeminiClient;
use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Arc;

/// Observer agent for screen/page analysis
pub struct ObserverAgent {
    gemini: Arc<GeminiClient>,
}

impl ObserverAgent {
    pub fn new(gemini: Arc<GeminiClient>) -> Self {
        Self { gemini }
    }

    /// Analyze page content for relevance to puzzle
    async fn analyze_content(&self, context: &AgentContext) -> AgentResult<f32> {
        // Redact URL for privacy
        let redacted_url = crate::privacy::redact_pii(&context.current_url);

        // Use Gemini to calculate semantic similarity
        let similarity = self
            .gemini
            .calculate_url_similarity(&redacted_url, &context.target_pattern)
            .await
            .map_err(|e| AgentError::ServiceError(e.to_string()))?;

        Ok(similarity)
    }

    /// Extract key topics from page
    async fn extract_topics(&self, context: &AgentContext) -> AgentResult<Vec<String>> {
        let redacted_content = crate::privacy::redact_pii(&context.page_content);
        let prompt = format!(
            "Extract 3-5 key topics from this page content. Return as comma-separated list:\n{}",
            &redacted_content.chars().take(500).collect::<String>()
        );

        let response = self
            .gemini
            .generate_dialogue(&prompt, "analytical")
            .await
            .map_err(|e| AgentError::ServiceError(e.to_string()))?;

        let topics: Vec<String> = response
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();

        Ok(topics)
    }
}

#[async_trait]
impl Agent for ObserverAgent {
    fn name(&self) -> &str {
        "Observer"
    }

    fn description(&self) -> &str {
        "Analyzes screen content and calculates proximity to puzzle solution"
    }

    async fn process(&self, context: &AgentContext) -> AgentResult<AgentOutput> {
        // Skip if no URL
        if context.current_url.is_empty() {
            return Ok(AgentOutput {
                agent_name: self.name().to_string(),
                result: "No URL to analyze".to_string(),
                confidence: 0.0,
                data: HashMap::new(),
                next_action: Some(NextAction::Continue),
            });
        }

        // Calculate proximity
        let proximity = self.analyze_content(context).await?;

        // Extract topics (best effort, don't fail if this fails)
        let topics = self.extract_topics(context).await.unwrap_or_default();

        // Determine next action based on proximity
        let next_action = if proximity > 0.85 {
            Some(NextAction::PuzzleSolved)
        } else if proximity > 0.5 {
            Some(NextAction::Continue) // Getting warmer
        } else if context.hints_revealed < context.hints.len() && proximity < 0.2 {
            Some(NextAction::ShowHint(context.hints_revealed)) // Cold, show hint
        } else {
            Some(NextAction::Continue)
        };

        // Build data
        let mut data = HashMap::new();
        data.insert(
            "proximity".to_string(),
            serde_json::Value::Number(serde_json::Number::from_f64(proximity as f64).unwrap()),
        );
        data.insert(
            "url".to_string(),
            serde_json::Value::String(context.current_url.clone()),
        );
        data.insert(
            "topics".to_string(),
            serde_json::to_value(topics).unwrap_or(serde_json::Value::Null),
        );

        // Determine confidence message
        let result = match proximity {
            p if p > 0.85 => "The signal is overwhelming! You've found it!".to_string(),
            p if p > 0.7 => "So close... the memories are almost clear.".to_string(),
            p if p > 0.5 => "Getting warmer... I can feel fragments forming.".to_string(),
            p if p > 0.3 => "There's something here... keep searching.".to_string(),
            _ => "The signal is faint... try a different path.".to_string(),
        };

        Ok(AgentOutput {
            agent_name: self.name().to_string(),
            result,
            confidence: proximity,
            data,
            next_action,
        })
    }

    fn can_handle(&self, context: &AgentContext) -> bool {
        !context.current_url.is_empty()
    }
}
