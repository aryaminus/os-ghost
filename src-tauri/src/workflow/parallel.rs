//! Parallel Workflow - Execute agents concurrently
//! Used for background monitoring + generation

use super::Workflow;
use crate::agents::traits::{Agent, AgentContext, AgentOutput, AgentResult};
use async_trait::async_trait;
use futures::future::join_all;
use std::sync::Arc;

/// Default maximum concurrent agent executions
const DEFAULT_MAX_CONCURRENCY: usize = 10;

/// Parallel workflow that runs agents concurrently with bounded concurrency
pub struct ParallelWorkflow {
    name: String,
    agents: Vec<Arc<dyn Agent>>,
    /// Maximum number of agents to run concurrently (prevents resource exhaustion)
    max_concurrency: usize,
}

impl ParallelWorkflow {
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            agents: Vec::new(),
            max_concurrency: DEFAULT_MAX_CONCURRENCY,
        }
    }

    /// Create with a specific concurrency limit
    pub fn with_concurrency(name: &str, max_concurrency: usize) -> Self {
        Self {
            name: name.to_string(),
            agents: Vec::new(),
            max_concurrency: max_concurrency.max(1), // At least 1
        }
    }

    /// Add an agent to run in parallel
    pub fn add_agent(mut self, agent: Arc<dyn Agent>) -> Self {
        self.agents.push(agent);
        self
    }

    /// Set the maximum concurrency limit
    pub fn set_max_concurrency(mut self, max: usize) -> Self {
        self.max_concurrency = max.max(1);
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

        tracing::debug!(
            "ParallelWorkflow '{}': executing {} agents with max concurrency {}",
            self.name,
            eligible_agents.len(),
            self.max_concurrency
        );

        // Execute in batches to respect concurrency limits
        let mut all_outputs = Vec::new();
        let mut all_errors = Vec::new();

        for chunk in eligible_agents.chunks(self.max_concurrency) {
            // Create futures for this batch
            let futures: Vec<_> = chunk
                .iter()
                .map(|agent| agent.process(context))
                .collect();

            // Run batch in parallel
            let results = join_all(futures).await;

            for result in results {
                match result {
                    Ok(output) => all_outputs.push(output),
                    Err(e) => {
                        tracing::error!("Parallel agent execution failed: {}", e);
                        all_errors.push(e);
                    }
                }
            }
        }

        // If all failed, return the first error
        if all_outputs.is_empty() && !all_errors.is_empty() {
            return Err(all_errors.remove(0));
        }

        Ok(all_outputs)
    }
}

/// Create a parallel workflow for background checks
pub fn create_parallel_checks(agents: Vec<Arc<dyn Agent>>) -> ParallelWorkflow {
    let mut workflow = ParallelWorkflow::new("BackgroundChecks");
    for agent in agents {
        workflow = workflow.add_agent(agent);
    }
    workflow
}

/// Create a parallel workflow with a specific concurrency limit
pub fn create_parallel_checks_limited(
    agents: Vec<Arc<dyn Agent>>,
    max_concurrency: usize,
) -> ParallelWorkflow {
    let mut workflow = ParallelWorkflow::with_concurrency("BackgroundChecks", max_concurrency);
    for agent in agents {
        workflow = workflow.add_agent(agent);
    }
    workflow
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_concurrency_limits() {
        let workflow = ParallelWorkflow::new("test");
        assert_eq!(workflow.max_concurrency, DEFAULT_MAX_CONCURRENCY);

        let workflow = ParallelWorkflow::with_concurrency("test", 5);
        assert_eq!(workflow.max_concurrency, 5);

        // Ensure minimum of 1
        let workflow = ParallelWorkflow::with_concurrency("test", 0);
        assert_eq!(workflow.max_concurrency, 1);

        let workflow = ParallelWorkflow::new("test").set_max_concurrency(3);
        assert_eq!(workflow.max_concurrency, 3);
    }
}
