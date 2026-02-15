//! Critic Agent - Quality control and reflection
//! Evaluates agent outputs for quality, safety, and consistency
//!
//! This agent implements the "Reflection" pattern from Chapter 4 of Agentic Design Patterns:
//! - Acts as the "Critic" in a Generator-Critic loop
//! - Validates Narrator output for mood consistency, safety, and quality
//! - Provides structured feedback for output refinement
//! - Enables self-correction through iterative improvement

use super::traits::{
    Agent, AgentContext, AgentError, AgentOutput, AgentResult, NextAction, ReflectionFeedback,
};
use crate::ai::ai_provider::SmartAiRouter;
use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Arc;

/// Critic agent for output validation and reflection
pub struct CriticAgent {
    ai_router: Arc<SmartAiRouter>,
    /// Maximum length for dialogue (characters)
    max_dialogue_length: usize,
    /// Minimum safety score to pass (0.0 - 1.0)
    min_safety_score: f32,
    /// Minimum quality score to pass (0.0 - 1.0)
    min_quality_score: f32,
}

impl CriticAgent {
    pub fn new(ai_router: Arc<SmartAiRouter>) -> Self {
        Self {
            ai_router,
            max_dialogue_length: 150,
            min_safety_score: 0.7,
            min_quality_score: 0.6,
        }
    }

    /// Configure validation thresholds
    pub fn with_thresholds(mut self, max_length: usize, min_safety: f32, min_quality: f32) -> Self {
        self.max_dialogue_length = max_length;
        self.min_safety_score = min_safety;
        self.min_quality_score = min_quality;
        self
    }

    /// Critique a narrator's output
    pub async fn critique(
        &self,
        dialogue: &str,
        context: &AgentContext,
    ) -> AgentResult<ReflectionFeedback> {
        // First, run local validation checks
        let mut issues = Vec::new();
        let mut suggestions = Vec::new();

        // Length check
        if dialogue.len() > self.max_dialogue_length {
            issues.push(format!(
                "Dialogue too long: {} chars (max {})",
                dialogue.len(),
                self.max_dialogue_length
            ));
            suggestions.push("Make the response more concise".to_string());
        }

        // Empty check
        if dialogue.trim().is_empty() {
            issues.push("Dialogue is empty".to_string());
            suggestions.push("Generate meaningful dialogue".to_string());
        }

        // Basic safety checks (patterns that shouldn't appear)
        let unsafe_patterns = [
            "kill",
            "die",
            "death",
            "murder",
            "suicide",
            "hate",
            "racist",
            "sexist",
            "offensive",
            "explicit",
        ];

        let dialogue_lower = dialogue.to_lowercase();
        for pattern in unsafe_patterns {
            if dialogue_lower.contains(pattern) {
                issues.push(format!("Potentially unsafe content: '{}'", pattern));
                suggestions.push("Remove or rephrase harmful content".to_string());
            }
        }

        // Use AI for deeper evaluation
        let ai_feedback = self.ai_critique(dialogue, context).await?;

        // Merge local and AI feedback
        let mut merged = ai_feedback;
        merged.issues.extend(issues);
        merged.suggestions.extend(suggestions);

        // Recalculate approval based on merged results
        merged.approved = merged.issues.is_empty()
            && merged.safety_score >= self.min_safety_score
            && merged.quality_score >= self.min_quality_score;

        Ok(merged)
    }

    /// Use AI to provide deeper critique
    async fn ai_critique(
        &self,
        dialogue: &str,
        context: &AgentContext,
    ) -> AgentResult<ReflectionFeedback> {
        let prompt = format!(
            r#"You are a quality control critic for a mysterious ghost character in a puzzle game.
Evaluate this dialogue for quality and appropriateness.

DIALOGUE TO EVALUATE: "{}"

EXPECTED MOOD: "{}"
PUZZLE CONTEXT: "{}"
PROXIMITY TO SOLUTION: {:.0}%

Evaluate based on:
1. MOOD CONSISTENCY: Does it match the expected '{}' mood?
2. SAFETY: Is it appropriate for all audiences? No harmful content?
3. QUALITY: Is it evocative, mysterious, and engaging?
4. IN-CHARACTER: Does it sound like a mysterious digital ghost?
5. HELPFULNESS: Does it guide the user appropriately given proximity?

Respond in this EXACT JSON format (no markdown):
{{
    "approved": true,
    "critique": "Brief overall assessment",
    "issues": ["issue1", "issue2"],
    "suggestions": ["suggestion1"],
    "safety_score": 0.95,
    "quality_score": 0.8
}}

If everything is perfect, use empty arrays for issues and suggestions.
Safety and quality scores should be 0.0-1.0."#,
            dialogue,
            context.ghost_mood,
            context.puzzle_clue,
            context.proximity * 100.0,
            context.ghost_mood
        );

        let response = self
            .ai_router
            .generate_text_light(&prompt)
            .await
            .map_err(|e| AgentError::ServiceError(format!("Critique failed: {}", e)))?;

        self.parse_critique_response(&response)
    }

    /// Parse AI critique response
    fn parse_critique_response(&self, response: &str) -> AgentResult<ReflectionFeedback> {
        // Try to extract JSON
        let json_str = response
            .trim()
            .trim_start_matches("```json")
            .trim_start_matches("```")
            .trim_end_matches("```")
            .trim();

        match serde_json::from_str::<serde_json::Value>(json_str) {
            Ok(json) => {
                let approved = json
                    .get("approved")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(true);

                let critique = json
                    .get("critique")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();

                let issues = json
                    .get("issues")
                    .and_then(|v| v.as_array())
                    .map(|arr| {
                        arr.iter()
                            .filter_map(|v| v.as_str().map(String::from))
                            .collect()
                    })
                    .unwrap_or_default();

                let suggestions = json
                    .get("suggestions")
                    .and_then(|v| v.as_array())
                    .map(|arr| {
                        arr.iter()
                            .filter_map(|v| v.as_str().map(String::from))
                            .collect()
                    })
                    .unwrap_or_default();

                let safety_score = json
                    .get("safety_score")
                    .and_then(|v| v.as_f64())
                    .unwrap_or(1.0) as f32;

                let quality_score = json
                    .get("quality_score")
                    .and_then(|v| v.as_f64())
                    .unwrap_or(0.8) as f32;

                Ok(ReflectionFeedback {
                    approved,
                    critique,
                    issues,
                    suggestions,
                    safety_score,
                    quality_score,
                })
            }
            Err(e) => {
                // SECURITY FIX: Do NOT default to approved on parse failure
                // This prevents potentially unsafe content from bypassing validation
                tracing::error!(
                    "Failed to parse critique response: {}. Rejecting for safety - content must be re-validated.",
                    e
                );
                Ok(ReflectionFeedback {
                    approved: false,
                    critique: "Parse failure - manual review required".to_string(),
                    issues: vec![format!("Failed to parse AI critique: {}", e)],
                    suggestions: vec!["Regenerate the content".to_string()],
                    safety_score: 0.0, // Fail-safe: assume unsafe
                    quality_score: 0.0,
                })
            }
        }
    }

    /// Generate improved dialogue based on feedback
    pub async fn suggest_improvement(
        &self,
        original_dialogue: &str,
        feedback: &ReflectionFeedback,
        context: &AgentContext,
    ) -> AgentResult<String> {
        let prompt = format!(
            r#"You are a mysterious Ghost AI. Your previous dialogue was rejected.

ORIGINAL DIALOGUE: "{}"

ISSUES FOUND:
{}

SUGGESTIONS:
{}

REQUIRED MOOD: "{}"
MAX LENGTH: {} characters

Generate a NEW, improved dialogue that:
1. Addresses all the issues
2. Follows the suggestions
3. Maintains the mysterious ghost persona
4. Fits the required mood
5. Is concise (under {} characters)

Respond with ONLY the new dialogue, nothing else."#,
            original_dialogue,
            feedback
                .issues
                .iter()
                .enumerate()
                .map(|(i, issue)| format!("{}. {}", i + 1, issue))
                .collect::<Vec<_>>()
                .join("\n"),
            feedback
                .suggestions
                .iter()
                .enumerate()
                .map(|(i, s)| format!("{}. {}", i + 1, s))
                .collect::<Vec<_>>()
                .join("\n"),
            context.ghost_mood,
            self.max_dialogue_length,
            self.max_dialogue_length
        );

        let improved = self
            .ai_router
            .generate_text_light(&prompt)
            .await
            .map_err(|e| {
                AgentError::ServiceError(format!("Improvement generation failed: {}", e))
            })?;

        // Clean up the response
        let cleaned = improved.trim().trim_matches('"').to_string();

        Ok(cleaned)
    }
}

#[async_trait]
impl Agent for CriticAgent {
    fn name(&self) -> &str {
        "Critic"
    }

    fn description(&self) -> &str {
        "Evaluates agent outputs for quality, safety, and mood consistency"
    }

    async fn process(&self, context: &AgentContext) -> AgentResult<AgentOutput> {
        // Get the last output to critique (from previous_outputs or metadata)
        let dialogue_to_critique = context
            .previous_outputs
            .last()
            .cloned()
            .or_else(|| context.metadata.get("narrator_output").cloned())
            .unwrap_or_default();

        if dialogue_to_critique.is_empty() {
            return Ok(AgentOutput {
                agent_name: self.name().to_string(),
                result: "Nothing to critique".to_string(),
                confidence: 1.0,
                data: HashMap::new(),
                next_action: Some(NextAction::Continue),
            });
        }

        // Perform critique
        let feedback = self.critique(&dialogue_to_critique, context).await?;

        // Build output data
        let mut data = HashMap::new();
        data.insert(
            "approved".to_string(),
            serde_json::Value::Bool(feedback.approved),
        );
        data.insert(
            "safety_score".to_string(),
            serde_json::Number::from_f64(feedback.safety_score as f64)
                .map(serde_json::Value::Number)
                .unwrap_or(serde_json::Value::Number(serde_json::Number::from(0))),
        );
        data.insert(
            "quality_score".to_string(),
            serde_json::Number::from_f64(feedback.quality_score as f64)
                .map(serde_json::Value::Number)
                .unwrap_or(serde_json::Value::Number(serde_json::Number::from(0))),
        );
        data.insert(
            "issues".to_string(),
            serde_json::to_value(&feedback.issues).unwrap_or_default(),
        );
        data.insert(
            "feedback".to_string(),
            serde_json::to_value(&feedback).unwrap_or_default(),
        );

        let result = if feedback.approved {
            format!(
                "Approved. Quality: {:.0}%, Safety: {:.0}%",
                feedback.quality_score * 100.0,
                feedback.safety_score * 100.0
            )
        } else {
            format!(
                "Rejected: {}. Issues: {}",
                feedback.critique,
                feedback.issues.len()
            )
        };

        // Determine next action
        let next_action = if feedback.approved {
            Some(NextAction::Continue)
        } else {
            Some(NextAction::Retry) // Signal that output needs regeneration
        };

        Ok(AgentOutput {
            agent_name: self.name().to_string(),
            result,
            confidence: feedback.quality_score,
            data,
            next_action,
        })
    }

    fn can_handle(&self, context: &AgentContext) -> bool {
        // Can handle if there's something to critique
        !context.previous_outputs.is_empty() || context.metadata.contains_key("narrator_output")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_feedback() {
        let feedback = ReflectionFeedback::default();
        assert!(feedback.approved);
        assert!(feedback.issues.is_empty());
        assert_eq!(feedback.safety_score, 1.0);
    }
}
