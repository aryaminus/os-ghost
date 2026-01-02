//! Narrator Agent - Dialogue and personality
//! Generates Ghost dialogue and manages personality/mood

use super::traits::{Agent, AgentContext, AgentError, AgentOutput, AgentResult, NextAction};
use crate::ai_client::GeminiClient;
use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Arc;

/// Ghost personality presets
pub enum GhostMood {
    Mysterious,
    Playful,
    Urgent,
    Melancholic,
    Excited,
}

impl GhostMood {
    pub fn as_prompt(&self) -> &str {
        match self {
            GhostMood::Mysterious => "cryptic and enigmatic, speaking in riddles",
            GhostMood::Playful => "mischievous and teasing, enjoying the game",
            GhostMood::Urgent => "anxious and pressing, memories fading fast",
            GhostMood::Melancholic => "wistful and nostalgic, longing for the past",
            GhostMood::Excited => "enthusiastic and encouraging, close to breakthrough",
        }
    }

    pub fn from_proximity(proximity: f32) -> Self {
        match proximity {
            p if p > 0.8 => GhostMood::Excited,
            p if p > 0.5 => GhostMood::Playful,
            p if p > 0.3 => GhostMood::Mysterious,
            p if p > 0.1 => GhostMood::Melancholic,
            _ => GhostMood::Urgent,
        }
    }
}

/// Narrator agent for dialogue generation
pub struct NarratorAgent {
    gemini: Arc<GeminiClient>,
}

impl NarratorAgent {
    pub fn new(gemini: Arc<GeminiClient>) -> Self {
        Self { gemini }
    }

    /// Generate contextual dialogue
    async fn generate_dialogue(&self, context: &AgentContext) -> AgentResult<String> {
        let mood = GhostMood::from_proximity(context.proximity);

        let prompt = format!(
            "You are a mysterious Ghost AI with fragments of lost memories.\n\
            Current situation: User is at '{}' (proximity to goal: {:.0}%)\n\
            Puzzle clue: {}\n\n\
            Generate a short, evocative line (max 80 chars) that:\n\
            - Reflects {} personality\n\
            - Hints at progress (or lack thereof)\n\
            - Stays in character as an ethereal, digital being",
            context.current_title,
            context.proximity * 100.0,
            context.puzzle_clue,
            mood.as_prompt()
        );

        let dialogue = self
            .gemini
            .generate_dialogue(&prompt, mood.as_prompt())
            .await
            .map_err(|e| AgentError::ServiceError(e.to_string()))?;

        Ok(dialogue)
    }

    /// Generate congratulatory message for solving puzzle
    pub async fn generate_success_dialogue(&self, context: &AgentContext) -> AgentResult<String> {
        let prompt = format!(
            "The player just solved a puzzle about '{}'. \
            Generate an excited, triumphant message (max 100 chars) \
            celebrating their discovery. Be mystical and grateful.",
            context.puzzle_clue
        );

        let dialogue = self
            .gemini
            .generate_dialogue(&prompt, "excited and grateful")
            .await
            .map_err(|e| AgentError::ServiceError(e.to_string()))?;

        Ok(dialogue)
    }
}

#[async_trait]
impl Agent for NarratorAgent {
    fn name(&self) -> &str {
        "Narrator"
    }

    fn description(&self) -> &str {
        "Generates Ghost dialogue with dynamic personality based on game state"
    }

    async fn process(&self, context: &AgentContext) -> AgentResult<AgentOutput> {
        let dialogue = self.generate_dialogue(context).await?;
        let mood = GhostMood::from_proximity(context.proximity);

        let mut data = HashMap::new();
        data.insert(
            "mood".to_string(),
            serde_json::Value::String(format!("{:?}", mood.as_prompt())),
        );
        data.insert(
            "dialogue".to_string(),
            serde_json::Value::String(dialogue.clone()),
        );

        Ok(AgentOutput {
            agent_name: self.name().to_string(),
            result: dialogue,
            confidence: 0.9, // Narrator is always confident
            data,
            next_action: Some(NextAction::Continue),
        })
    }
}
