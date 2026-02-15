//! Planner Agent - Dynamic planning and goal decomposition
//! Analyzes puzzles and generates sub-goals, keywords, and search strategies at runtime
//!
//! This agent implements the "Planning" pattern from Chapter 6 of Agentic Design Patterns:
//! - Decomposes high-level puzzle objectives into actionable sub-goals
//! - Generates primary and secondary keywords for the Verifier
//! - Determines optimal search strategy based on puzzle complexity
//! - Enables the Ghost to guide users proactively rather than reactively

use super::traits::{
    Agent, AgentContext, AgentError, AgentOutput, AgentResult, NextAction, PlanningContext,
    SearchStrategy, SubGoal,
};
use crate::ai::ai_provider::SmartAiRouter;
use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Arc;

/// Planner agent for dynamic puzzle analysis and goal decomposition
pub struct PlannerAgent {
    ai_router: Arc<SmartAiRouter>,
}

impl PlannerAgent {
    pub fn new(ai_router: Arc<SmartAiRouter>) -> Self {
        Self { ai_router }
    }

    /// Analyze a puzzle and generate a planning context
    pub async fn analyze_puzzle(&self, context: &AgentContext) -> AgentResult<PlanningContext> {
        // Build the planning prompt
        let prompt = format!(
            r#"You are a strategic puzzle planner. Analyze this puzzle and create a search plan.

PUZZLE CLUE: "{}"
TARGET PATTERN: "{}"
CURRENT URL: "{}"
CURRENT PROXIMITY: {:.0}%

Your task is to decompose this puzzle into actionable steps. Think about:
1. What is the user ultimately looking for?
2. What intermediate steps would lead them there?
3. What keywords should they look for on web pages?

Respond in this EXACT JSON format (no markdown, just raw JSON):
{{
    "sub_goals": [
        {{"step": 1, "description": "Brief action description", "keywords": ["keyword1", "keyword2"]}},
        {{"step": 2, "description": "Next action", "keywords": ["keyword3"]}}
    ],
    "primary_keywords": ["most", "important", "keywords"],
    "secondary_keywords": ["related", "alternative", "terms"],
    "difficulty": 0.5,
    "strategy": "explore"
}}

For "strategy", use one of: "explore" (far from goal), "focus" (getting closer), "verify" (very close), "celebrate" (solved).
For "difficulty", use 0.0-1.0 where 0.0 is trivial and 1.0 is very hard.
Generate 2-5 sub_goals depending on complexity."#,
            context.puzzle_clue,
            context.target_pattern,
            crate::config::privacy::redact_with_settings(&context.current_url),
            context.proximity * 100.0
        );

        let response = self
            .ai_router
            .generate_text_light(&prompt)
            .await
            .map_err(|e| AgentError::ServiceError(format!("Planning failed: {}", e)))?;

        // Parse the JSON response
        self.parse_planning_response(&response, context)
    }

    /// Parse AI response into PlanningContext
    fn parse_planning_response(
        &self,
        response: &str,
        context: &AgentContext,
    ) -> AgentResult<PlanningContext> {
        // Try to extract JSON from response (handle markdown code blocks)
        let json_str = response
            .trim()
            .trim_start_matches("```json")
            .trim_start_matches("```")
            .trim_end_matches("```")
            .trim();

        match serde_json::from_str::<serde_json::Value>(json_str) {
            Ok(json) => {
                // Parse sub_goals
                let sub_goals = json
                    .get("sub_goals")
                    .and_then(|sg| sg.as_array())
                    .map(|arr| {
                        arr.iter()
                            .filter_map(|g| {
                                Some(SubGoal {
                                    step: g.get("step")?.as_u64()? as usize,
                                    description: g.get("description")?.as_str()?.to_string(),
                                    keywords: g
                                        .get("keywords")
                                        .and_then(|k| k.as_array())
                                        .map(|a| {
                                            a.iter()
                                                .filter_map(|v| v.as_str().map(String::from))
                                                .collect()
                                        })
                                        .unwrap_or_default(),
                                    achieved: false,
                                    confidence: 0.0,
                                })
                            })
                            .collect()
                    })
                    .unwrap_or_default();

                // Parse keywords
                let primary_keywords = json
                    .get("primary_keywords")
                    .and_then(|k| k.as_array())
                    .map(|a| {
                        a.iter()
                            .filter_map(|v| v.as_str().map(String::from))
                            .collect()
                    })
                    .unwrap_or_default();

                let secondary_keywords = json
                    .get("secondary_keywords")
                    .and_then(|k| k.as_array())
                    .map(|a| {
                        a.iter()
                            .filter_map(|v| v.as_str().map(String::from))
                            .collect()
                    })
                    .unwrap_or_default();

                // Parse strategy
                let strategy = match json
                    .get("strategy")
                    .and_then(|s| s.as_str())
                    .unwrap_or("explore")
                {
                    "focus" => SearchStrategy::Focus,
                    "verify" => SearchStrategy::Verify,
                    "celebrate" => SearchStrategy::Celebrate,
                    _ => SearchStrategy::Explore,
                };

                // Parse difficulty
                let difficulty = json
                    .get("difficulty")
                    .and_then(|d| d.as_f64())
                    .unwrap_or(0.5) as f32;

                Ok(PlanningContext {
                    sub_goals,
                    primary_keywords,
                    secondary_keywords,
                    strategy,
                    difficulty,
                    revision_count: context.planning.revision_count,
                    failed_approaches: context.planning.failed_approaches.clone(),
                })
            }
            Err(e) => {
                tracing::warn!("Failed to parse planning response: {}. Using fallback.", e);
                // Fallback: extract keywords from puzzle clue
                Ok(self.create_fallback_plan(context))
            }
        }
    }

    /// Create a basic fallback plan when AI parsing fails
    fn create_fallback_plan(&self, context: &AgentContext) -> PlanningContext {
        // Extract keywords from puzzle clue (simple heuristic)
        let keywords: Vec<String> = context
            .puzzle_clue
            .split_whitespace()
            .filter(|w| w.len() > 4 && !is_stop_word(w))
            .map(|w| {
                w.to_lowercase()
                    .trim_matches(|c: char| !c.is_alphanumeric())
                    .to_string()
            })
            .filter(|w| !w.is_empty())
            .collect();

        let strategy = if context.proximity > 0.8 {
            SearchStrategy::Verify
        } else if context.proximity > 0.5 {
            SearchStrategy::Focus
        } else {
            SearchStrategy::Explore
        };

        PlanningContext {
            sub_goals: vec![SubGoal {
                step: 1,
                description: "Search for puzzle-related content".to_string(),
                keywords: keywords.clone(),
                achieved: false,
                confidence: 0.0,
            }],
            primary_keywords: keywords.clone(),
            secondary_keywords: Vec::new(),
            strategy,
            difficulty: 0.5,
            revision_count: 0,
            failed_approaches: Vec::new(),
        }
    }

    /// Revise the plan based on failed approaches (self-correction)
    pub async fn revise_plan(
        &self,
        context: &AgentContext,
        failed_reason: &str,
    ) -> AgentResult<PlanningContext> {
        let mut new_context = context.clone();
        new_context
            .planning
            .failed_approaches
            .push(failed_reason.to_string());

        let prompt = format!(
            r#"You are revising a puzzle search plan because the previous approach failed.

PUZZLE CLUE: "{}"
PREVIOUS FAILED APPROACHES:
{}

What went wrong and what alternative approach should we try?

Respond in the same JSON format as before, but with NEW keywords and sub_goals that avoid the failed approaches.
Generate a revised plan that takes a different angle on the puzzle."#,
            context.puzzle_clue,
            new_context
                .planning
                .failed_approaches
                .iter()
                .enumerate()
                .map(|(i, f)| format!("{}. {}", i + 1, f))
                .collect::<Vec<_>>()
                .join("\n")
        );

        let response = self
            .ai_router
            .generate_text_light(&prompt)
            .await
            .map_err(|e| AgentError::ServiceError(format!("Plan revision failed: {}", e)))?;

        let mut plan = self.parse_planning_response(&response, &new_context)?;
        plan.revision_count = context.planning.revision_count + 1;
        plan.failed_approaches = new_context.planning.failed_approaches;

        Ok(plan)
    }

    /// Update sub-goal progress based on current context
    pub fn update_progress(&self, context: &AgentContext) -> PlanningContext {
        let mut planning = context.planning.clone();
        let content_lower = context.page_content.to_lowercase();
        let url_lower = context.current_url.to_lowercase();

        // Check each sub-goal's keywords against current page
        for sub_goal in &mut planning.sub_goals {
            if sub_goal.achieved {
                continue;
            }

            let matches: usize = sub_goal
                .keywords
                .iter()
                .filter(|kw| {
                    content_lower.contains(&kw.to_lowercase())
                        || url_lower.contains(&kw.to_lowercase())
                })
                .count();

            if !sub_goal.keywords.is_empty() {
                sub_goal.confidence = matches as f32 / sub_goal.keywords.len() as f32;
                if sub_goal.confidence > 0.6 {
                    sub_goal.achieved = true;
                }
            }
        }

        // Update strategy based on progress
        let achieved_count = planning.sub_goals.iter().filter(|g| g.achieved).count();
        let total_goals = planning.sub_goals.len();

        if total_goals > 0 {
            let progress = achieved_count as f32 / total_goals as f32;
            planning.strategy = if context.proximity > 0.85 || progress > 0.9 {
                SearchStrategy::Celebrate
            } else if context.proximity > 0.7 || progress > 0.7 {
                SearchStrategy::Verify
            } else if context.proximity > 0.4 || progress > 0.4 {
                SearchStrategy::Focus
            } else {
                SearchStrategy::Explore
            };
        }

        planning
    }
}

/// Simple stop word filter for keyword extraction
#[inline]
fn is_stop_word(word: &str) -> bool {
    const STOP_WORDS: &[&str] = &[
        "the", "a", "an", "is", "are", "was", "were", "be", "been", "being", "have", "has", "had",
        "do", "does", "did", "will", "would", "could", "should", "may", "might", "must", "shall",
        "can", "need", "dare", "ought", "used", "to", "of", "in", "for", "on", "with", "at", "by",
        "from", "as", "into", "through", "during", "before", "after", "above", "below", "between",
        "under", "again", "further", "then", "once", "here", "there", "when", "where", "why",
        "how", "all", "each", "few", "more", "most", "other", "some", "such", "no", "nor", "not",
        "only", "own", "same", "so", "than", "too", "very", "just", "and", "but", "if", "or",
        "because", "until", "while", "this", "that", "these", "those", "find", "first", "about",
        "which", "what", "their", "they", "them", "your",
    ];
    STOP_WORDS.contains(&word.to_lowercase().as_str())
}

#[async_trait]
impl Agent for PlannerAgent {
    fn name(&self) -> &str {
        "Planner"
    }

    fn description(&self) -> &str {
        "Analyzes puzzles and generates dynamic sub-goals, keywords, and search strategies"
    }

    async fn process(&self, context: &AgentContext) -> AgentResult<AgentOutput> {
        // Check if we need to create a new plan or update existing
        let planning = if context.planning.sub_goals.is_empty() {
            // No plan exists - create one
            tracing::info!("Creating new puzzle plan for: {}", context.puzzle_id);
            self.analyze_puzzle(context).await?
        } else {
            // Update progress on existing plan
            self.update_progress(context)
        };

        // Build output data
        let mut data = HashMap::new();
        data.insert(
            "sub_goals_count".to_string(),
            serde_json::Value::Number(planning.sub_goals.len().into()),
        );
        data.insert(
            "primary_keywords".to_string(),
            serde_json::to_value(&planning.primary_keywords).unwrap_or_default(),
        );
        data.insert(
            "strategy".to_string(),
            serde_json::Value::String(format!("{:?}", planning.strategy)),
        );
        data.insert(
            "difficulty".to_string(),
            // Handle NaN/Infinity safely (serde_json::Number rejects non-finite)
            serde_json::Number::from_f64(planning.difficulty as f64)
                .map(serde_json::Value::Number)
                .unwrap_or_else(|| {
                    serde_json::Number::from_f64(0.5)
                        .map(serde_json::Value::Number)
                        .unwrap_or_else(|| serde_json::Value::Number(serde_json::Number::from(0)))
                }),
        );
        data.insert(
            "planning_context".to_string(),
            serde_json::to_value(&planning).unwrap_or_default(),
        );

        // Determine achieved sub-goals
        let achieved: Vec<_> = planning
            .sub_goals
            .iter()
            .filter(|g| g.achieved)
            .map(|g| g.description.clone())
            .collect();

        let result = if achieved.is_empty() {
            format!(
                "Plan created with {} sub-goals. Strategy: {:?}",
                planning.sub_goals.len(),
                planning.strategy
            )
        } else {
            format!(
                "Progress: {}/{} goals achieved. Current: {:?}",
                achieved.len(),
                planning.sub_goals.len(),
                planning.strategy
            )
        };

        Ok(AgentOutput {
            agent_name: self.name().to_string(),
            result,
            confidence: planning.difficulty,
            data,
            next_action: Some(NextAction::Continue),
        })
    }

    fn can_handle(&self, context: &AgentContext) -> bool {
        // Can handle any context with a puzzle clue
        !context.puzzle_clue.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_stop_word_filter() {
        assert!(is_stop_word("the"));
        assert!(is_stop_word("The"));
        assert!(!is_stop_word("manifesto"));
        assert!(!is_stop_word("unabomber"));
    }
}
