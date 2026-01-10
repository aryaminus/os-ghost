//! Planning Workflow - Dynamic multi-step puzzle solving
//! Implements the Planning pattern with adaptive strategy selection
//!
//! This workflow implements the "Planning" pattern from Chapter 6 of Agentic Design Patterns:
//! - Analyzes puzzle complexity to determine strategy
//! - Creates and tracks sub-goals dynamically
//! - Adapts workflow based on progress and difficulty
//! - Integrates Planner, Observer, Verifier, and Narrator agents

use super::Workflow;
use crate::agents::planner::PlannerAgent;
use crate::agents::traits::{Agent, AgentContext, AgentOutput, AgentResult, NextAction, SearchStrategy};
use async_trait::async_trait;
use std::sync::Arc;

/// Planning workflow that orchestrates dynamic puzzle solving
pub struct PlanningWorkflow {
    name: String,
    /// Planner agent for goal decomposition
    planner: Arc<PlannerAgent>,
    /// Observer agent for content analysis
    observer: Arc<dyn Agent>,
    /// Verifier agent for solution validation
    verifier: Arc<dyn Agent>,
    /// Narrator agent for dialogue (optional for some workflows)
    narrator: Option<Arc<dyn Agent>>,
}

impl PlanningWorkflow {
    pub fn new(
        name: &str,
        planner: Arc<PlannerAgent>,
        observer: Arc<dyn Agent>,
        verifier: Arc<dyn Agent>,
    ) -> Self {
        Self {
            name: name.to_string(),
            planner,
            observer,
            verifier,
            narrator: None,
        }
    }

    /// Add narrator for full pipeline
    pub fn with_narrator(mut self, narrator: Arc<dyn Agent>) -> Self {
        self.narrator = Some(narrator);
        self
    }

    /// Select appropriate agents based on strategy
    fn select_agents_for_strategy(&self, strategy: &SearchStrategy) -> Vec<Arc<dyn Agent>> {
        match strategy {
            SearchStrategy::Explore => {
                // Exploration: Observer first to understand the landscape
                vec![
                    self.observer.clone(),
                    self.verifier.clone(),
                ]
            }
            SearchStrategy::Focus => {
                // Focused: Verifier is more important
                vec![
                    self.verifier.clone(),
                    self.observer.clone(),
                ]
            }
            SearchStrategy::Verify => {
                // Verification: Just verify, minimal observation
                vec![self.verifier.clone()]
            }
            SearchStrategy::Celebrate => {
                // Celebration: Narrator takes over
                if let Some(ref narrator) = self.narrator {
                    vec![narrator.clone()]
                } else {
                    vec![]
                }
            }
        }
    }
}

#[async_trait]
impl Workflow for PlanningWorkflow {
    fn name(&self) -> &str {
        &self.name
    }

    async fn execute(&self, context: &AgentContext) -> AgentResult<Vec<AgentOutput>> {
        let mut outputs = Vec::new();
        let mut current_context = context.clone();

        // Step 1: Generate or update plan
        let planning_output = self.planner.process(&current_context).await?;
        outputs.push(planning_output.clone());

        // Extract planning context from output
        if let Some(planning_json) = planning_output.data.get("planning_context") {
            if let Ok(planning) = serde_json::from_value(planning_json.clone()) {
                current_context.planning = planning;
            }
        }

        tracing::info!(
            "Planning workflow using strategy: {:?} with {} sub-goals",
            current_context.planning.strategy,
            current_context.planning.sub_goals.len()
        );

        // Step 2: Select agents based on strategy
        let agents = self.select_agents_for_strategy(&current_context.planning.strategy);

        if agents.is_empty() {
            return Ok(outputs);
        }

        // Step 3: Execute selected agents
        for agent in agents {
            if !agent.can_handle(&current_context) {
                continue;
            }

            let output = agent.process(&current_context).await?;

            // Update context with output
            if let Some(proximity) = output.data.get("proximity") {
                if let Some(p) = proximity.as_f64() {
                    current_context.proximity = p as f32;
                }
            }

            // Check for puzzle solved
            let solved = matches!(output.next_action, Some(NextAction::PuzzleSolved));

            outputs.push(output.clone());

            if solved {
                // Run narrator for celebration if available
                if let Some(ref narrator) = self.narrator {
                    current_context.planning.strategy = SearchStrategy::Celebrate;
                    let celebration = narrator.process(&current_context).await?;
                    outputs.push(celebration);
                }
                break;
            }
        }

        // Step 4: Run narrator if not already done
        if let Some(ref narrator) = self.narrator {
            if !matches!(current_context.planning.strategy, SearchStrategy::Celebrate)
                && narrator.can_handle(&current_context)
            {
                let dialogue = narrator.process(&current_context).await?;
                outputs.push(dialogue);
            }
        }

        // Step 5: Update sub-goal progress
        let updated_planning = self.planner.update_progress(&current_context);
        current_context.planning = updated_planning;

        Ok(outputs)
    }
}

/// Create an intelligent puzzle pipeline with planning
/// This is the upgraded version of `create_puzzle_pipeline`
pub fn create_intelligent_pipeline(
    planner: Arc<PlannerAgent>,
    observer: Arc<dyn Agent>,
    verifier: Arc<dyn Agent>,
    narrator: Arc<dyn Agent>,
) -> PlanningWorkflow {
    PlanningWorkflow::new("IntelligentPipeline", planner, observer, verifier)
        .with_narrator(narrator)
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_agent_selection() {
        // This would require mock agents - placeholder for future test
        // When implementing, create mock agents and verify:
        // - Explore strategy returns [observer, verifier]
        // - Focus strategy returns [verifier, observer]
        // - Verify strategy returns [verifier]
        // - Celebrate strategy returns [narrator] if available
    }
}
