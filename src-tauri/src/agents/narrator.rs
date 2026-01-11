//! Narrator Agent - Dialogue and personality
//! Generates Ghost dialogue and manages personality/mood

use super::traits::{Agent, AgentContext, AgentError, AgentOutput, AgentResult, NextAction};
use crate::ai_provider::SmartAiRouter;
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
    /// Convert mood to AI prompt description
    pub fn as_prompt(&self) -> &str {
        match self {
            GhostMood::Mysterious => "cryptic and enigmatic, speaking in riddles",
            GhostMood::Playful => "mischievous and teasing, enjoying the game",
            GhostMood::Urgent => "anxious and pressing, memories fading fast",
            GhostMood::Melancholic => "wistful and nostalgic, longing for the past",
            GhostMood::Excited => "enthusiastic and encouraging, close to breakthrough",
        }
    }

    /// Create mood from proximity score (0.0-1.0)
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
    ai_router: Arc<SmartAiRouter>,
}

impl NarratorAgent {
    pub fn new(ai_router: Arc<SmartAiRouter>) -> Self {
        Self { ai_router }
    }

    /// Generate contextual dialogue using Chain-of-Thought (CoT) prompting
    /// The ghost "thinks" through its response before generating dialogue
    async fn generate_dialogue(&self, context: &AgentContext) -> AgentResult<String> {
        let mood = GhostMood::from_proximity(context.proximity);

        // Redact PII from title before sending to API
        let redacted_title = crate::privacy::redact_pii(&context.current_title);

        // Chain-of-Thought prompt structure:
        // 1. Analyze the situation
        // 2. Consider what the user needs
        // 3. Decide on the appropriate response
        // 4. Generate the dialogue
        let cot_prompt = format!(
            r#"You are a mysterious Ghost AI with fragments of lost memories.

CURRENT CONTEXT:
- Location: '{}'
- Proximity to goal: {:.0}%
- Puzzle clue: "{}"
- Current mood: {}
- Planning strategy: {:?}
- Sub-goals achieved: {}/{}

CHAIN OF THOUGHT - Think through this step by step:

1. ANALYZE: What is the user currently doing? How close are they to the goal?
   (Consider: Are they on the right track? Do they seem stuck? Are they exploring?)

2. CONSIDER: What does the user need right now?
   - If far (0-30%): gentle encouragement, don't give away too much
   - If medium (30-70%): more specific hints, show excitement
   - If close (70-100%): build anticipation, celebrate progress

3. DECIDE: What tone and content fits the '{}' mood?
   (Remember: You are ethereal, mysterious, speaking in fragments of lost memory)

4. GENERATE: Create a single evocative line (MAX 80 CHARACTERS) that:
   - Reflects your {} personality
   - Hints at their progress appropriately
   - Stays in character as a digital ghost

OUTPUT ONLY THE FINAL DIALOGUE LINE, nothing else. No quotes, no explanation."#,
            redacted_title,
            context.proximity * 100.0,
            context.puzzle_clue,
            mood.as_prompt(),
            context.planning.strategy,
            context.planning.sub_goals.iter().filter(|g| g.achieved).count(),
            context.planning.sub_goals.len(),
            mood.as_prompt(),
            mood.as_prompt()
        );

        let dialogue = self
            .ai_router
            .generate_dialogue(&cot_prompt, mood.as_prompt())
            .await
            .map_err(|e| AgentError::ServiceError(e.to_string()))?;

        // Clean up the response (remove any accidental quotes or explanations)
        let cleaned = dialogue
            .trim()
            .trim_matches('"')
            .lines()
            .last()
            .unwrap_or(&dialogue)
            .to_string();

        Ok(cleaned)
    }

    /// Generate congratulatory message for solving puzzle (with CoT)
    pub async fn generate_success_dialogue(&self, context: &AgentContext) -> AgentResult<String> {
        let cot_prompt = format!(
            r#"You are a mysterious Ghost AI celebrating a puzzle solved!

PUZZLE SOLVED: "{}"
HINTS USED: {}
TIME INVESTED: The player persevered through the mystery

CHAIN OF THOUGHT:
1. The player just achieved something meaningful
2. Express genuine gratitude and joy (you recovered a lost memory!)
3. Stay in character as a mystical, ethereal being
4. Make them feel accomplished

Generate an EXCITED, TRIUMPHANT message (max 100 chars) celebrating their discovery.
Be mystical, grateful, and joyous.

OUTPUT ONLY THE CELEBRATION LINE:"#,
            context.puzzle_clue,
            context.hints_revealed
        );

        let dialogue = self
            .ai_router
            .generate_dialogue(&cot_prompt, "excited and grateful")
            .await
            .map_err(|e| AgentError::ServiceError(e.to_string()))?;

        let cleaned = dialogue.trim().trim_matches('"').to_string();
        Ok(cleaned)
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
            serde_json::Value::String(mood.as_prompt().to_string()),
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
