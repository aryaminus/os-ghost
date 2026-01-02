//! Loop Workflow - Repeat until condition met
//! Used for hot/cold feedback loops

use super::Workflow;
use crate::agents::traits::{Agent, AgentContext, AgentOutput, AgentResult, NextAction};
use async_trait::async_trait;
use std::sync::Arc;

/// Condition to check for loop termination
pub type LoopCondition = Box<dyn Fn(&AgentOutput) -> bool + Send + Sync>;

/// Loop workflow that repeats until condition is met
pub struct LoopWorkflow {
    name: String,
    agent: Arc<dyn Agent>,
    condition: LoopCondition,
    max_iterations: usize,
    delay_ms: u64,
}

impl LoopWorkflow {
    pub fn new(name: &str, agent: Arc<dyn Agent>, condition: LoopCondition) -> Self {
        Self {
            name: name.to_string(),
            agent,
            condition,
            max_iterations: 100,
            delay_ms: 2000, // 2 second default poll interval
        }
    }

    /// Set maximum iterations
    pub fn max_iterations(mut self, max: usize) -> Self {
        self.max_iterations = max;
        self
    }

    /// Set delay between iterations (ms)
    pub fn delay_ms(mut self, delay: u64) -> Self {
        self.delay_ms = delay;
        self
    }
}

#[async_trait]
impl Workflow for LoopWorkflow {
    fn name(&self) -> &str {
        &self.name
    }

    async fn execute(&self, context: &AgentContext) -> AgentResult<Vec<AgentOutput>> {
        let mut outputs = Vec::new();
        let mut current_context = context.clone();

        for iteration in 0..self.max_iterations {
            // Process with agent
            let output = self.agent.process(&current_context).await?;

            // Update context with output
            if let Some(proximity) = output.data.get("proximity") {
                if let Some(p) = proximity.as_f64() {
                    current_context.proximity = p as f32;
                }
            }

            // Check termination condition
            let should_stop = (self.condition)(&output);

            // Check for explicit stop actions
            let action_stop = matches!(
                output.next_action,
                Some(NextAction::Stop) | Some(NextAction::PuzzleSolved)
            );

            outputs.push(output);

            if should_stop || action_stop {
                tracing::info!(
                    "Loop '{}' terminated after {} iterations",
                    self.name,
                    iteration + 1
                );
                break;
            }

            // Delay before next iteration
            if self.delay_ms > 0 {
                tokio::time::sleep(tokio::time::Duration::from_millis(self.delay_ms)).await;
            }
        }

        Ok(outputs)
    }
}

/// Create a hot/cold feedback loop
pub fn create_hotcold_loop(observer: Arc<dyn Agent>) -> LoopWorkflow {
    LoopWorkflow::new(
        "HotColdLoop",
        observer,
        Box::new(|output| {
            // Stop when puzzle is solved or very close
            matches!(output.next_action, Some(NextAction::PuzzleSolved)) || output.confidence > 0.9
        }),
    )
    .max_iterations(100)
    .delay_ms(2000) // Poll every 2 seconds
}
