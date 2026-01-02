//! Sequential Workflow - Execute agents in order
//! Pipeline pattern: Agent1 → Agent2 → Agent3 → ...

use super::Workflow;
use crate::agents::traits::{Agent, AgentContext, AgentOutput, AgentResult, NextAction};
use async_trait::async_trait;
use std::sync::Arc;

/// Sequential workflow that runs agents in order
pub struct SequentialWorkflow {
    name: String,
    agents: Vec<Arc<dyn Agent>>,
    stop_on_solved: bool,
}

impl SequentialWorkflow {
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            agents: Vec::new(),
            stop_on_solved: true,
        }
    }

    /// Add an agent to the pipeline
    pub fn add_agent(mut self, agent: Arc<dyn Agent>) -> Self {
        self.agents.push(agent);
        self
    }

    /// Set whether to stop when puzzle is solved
    pub fn stop_on_solved(mut self, stop: bool) -> Self {
        self.stop_on_solved = stop;
        self
    }
}

#[async_trait]
impl Workflow for SequentialWorkflow {
    fn name(&self) -> &str {
        &self.name
    }

    async fn execute(&self, context: &AgentContext) -> AgentResult<Vec<AgentOutput>> {
        let mut outputs = Vec::new();
        let mut current_context = context.clone();

        for agent in &self.agents {
            // Check if agent can handle this context
            if !agent.can_handle(&current_context) {
                continue;
            }

            // Process with this agent
            let output = agent.process(&current_context).await?;

            // Update context with output (e.g., proximity)
            if let Some(proximity) = output.data.get("proximity") {
                if let Some(p) = proximity.as_f64() {
                    current_context.proximity = p as f32;
                }
            }

            // Check for stop conditions
            let should_stop = match &output.next_action {
                Some(NextAction::Stop) => true,
                Some(NextAction::PuzzleSolved) if self.stop_on_solved => true,
                _ => false,
            };

            outputs.push(output);

            if should_stop {
                break;
            }
        }

        Ok(outputs)
    }
}

/// Helper to create a standard clue-validation pipeline
pub fn create_puzzle_pipeline(
    observer: Arc<dyn Agent>,
    verifier: Arc<dyn Agent>,
    narrator: Arc<dyn Agent>,
) -> SequentialWorkflow {
    SequentialWorkflow::new("PuzzlePipeline")
        .add_agent(observer)
        .add_agent(verifier)
        .add_agent(narrator)
        .stop_on_solved(true)
}
