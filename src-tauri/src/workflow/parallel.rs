//! Parallel Workflow - Execute agents concurrently
//! Used for background monitoring + generation

use super::Workflow;
use crate::agents::traits::{Agent, AgentContext, AgentOutput, AgentResult};
use async_trait::async_trait;
use futures::future::join_all;
use std::sync::Arc;

/// Parallel workflow that runs agents concurrently
pub struct ParallelWorkflow {
    name: String,
    agents: Vec<Arc<dyn Agent>>,
}

impl ParallelWorkflow {
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            agents: Vec::new(),
        }
    }

    /// Add an agent to run in parallel
    pub fn add_agent(mut self, agent: Arc<dyn Agent>) -> Self {
        self.agents.push(agent);
        self
    }
}

#[async_trait]
impl Workflow for ParallelWorkflow {
    fn name(&self) -> &str {
        &self.name
    }

    async fn execute(&self, context: &AgentContext) -> AgentResult<Vec<AgentOutput>> {
        // Filter to agents that can handle this context
        let eligible_agents: Vec<_> = self
            .agents
            .iter()
            .filter(|a| a.can_handle(context))
            .collect();

        if eligible_agents.is_empty() {
            return Ok(Vec::new());
        }

        // Create futures for all agents
        let futures: Vec<_> = eligible_agents
            .iter()
            .map(|agent| agent.process(context))
            .collect();

        // Run all in parallel
        let results = join_all(futures).await;

        // Collect successful results
        let outputs: Vec<AgentOutput> = results.into_iter().filter_map(|r| r.ok()).collect();

        Ok(outputs)
    }
}

/// Create a background monitoring workflow
pub fn create_background_workflow(
    safety_monitor: Arc<dyn Agent>,
    puzzle_generator: Arc<dyn Agent>,
) -> ParallelWorkflow {
    ParallelWorkflow::new("BackgroundMonitoring")
        .add_agent(safety_monitor)
        .add_agent(puzzle_generator)
}
