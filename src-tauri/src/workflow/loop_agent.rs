//! Loop Workflow - Repeat until condition met
//! Used for hot/cold feedback loops with self-correction capabilities
//!
//! Enhanced to support the "Self-Correction" aspect of the Reflection pattern:
//! - Modifies context based on previous outputs
//! - Tracks failed approaches for plan revision
//! - Enables adaptive behavior through context mutation
//! - Supports graceful cancellation via CancellationToken

use super::Workflow;
use crate::agents::traits::{Agent, AgentContext, AgentError, AgentOutput, AgentResult, NextAction};
use async_trait::async_trait;
use std::sync::Arc;
use tokio_util::sync::CancellationToken;

/// Condition to check for loop termination
pub type LoopCondition = Box<dyn Fn(&AgentOutput) -> bool + Send + Sync>;

/// Context modifier function - allows self-correction by modifying context between iterations
/// Takes the current context and previous output, returns modified context
pub type ContextModifier = Box<dyn Fn(&AgentContext, &AgentOutput) -> AgentContext + Send + Sync>;

/// Loop workflow that repeats until condition is met
/// Enhanced with self-correction capabilities
pub struct LoopWorkflow {
    name: String,
    agent: Arc<dyn Agent>,
    condition: LoopCondition,
    max_iterations: usize,
    delay_ms: u64,
    /// Optional context modifier for self-correction
    context_modifier: Option<ContextModifier>,
    /// Track stagnation (no progress) for adaptive behavior
    stagnation_threshold: usize,
}

impl LoopWorkflow {
    pub fn new(name: &str, agent: Arc<dyn Agent>, condition: LoopCondition) -> Self {
        Self {
            name: name.to_string(),
            agent,
            condition,
            max_iterations: 100,
            delay_ms: 2000, // 2 second default poll interval
            context_modifier: None,
            stagnation_threshold: 3,
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

    /// Set context modifier for self-correction
    /// This function is called between iterations to modify the context
    /// based on the previous output, enabling adaptive behavior
    pub fn with_context_modifier(mut self, modifier: ContextModifier) -> Self {
        self.context_modifier = Some(modifier);
        self
    }

    /// Set stagnation threshold (number of iterations without progress before adapting)
    pub fn stagnation_threshold(mut self, threshold: usize) -> Self {
        self.stagnation_threshold = threshold;
        self
    }

    /// Execute the loop with cancellation support
    /// Checks the cancellation token before each iteration
    pub async fn execute_with_cancellation(
        &self,
        context: &AgentContext,
        cancel_token: CancellationToken,
    ) -> AgentResult<Vec<AgentOutput>> {
        let mut outputs = Vec::new();
        let mut current_context = context.clone();
        let mut last_proximity: f32 = 0.0;
        let mut stagnation_count: usize = 0;

        for iteration in 0..self.max_iterations {
            // Check for cancellation before each iteration
            if cancel_token.is_cancelled() {
                tracing::info!(
                    "Loop '{}' cancelled after {} iterations",
                    self.name,
                    iteration
                );
                return Err(AgentError::Cancelled);
            }

            // Process with agent (with cancellation race)
            let output_result = tokio::select! {
                result = self.agent.process(&current_context) => result,
                _ = cancel_token.cancelled() => {
                    tracing::info!("Loop '{}' cancelled during agent processing", self.name);
                    return Err(AgentError::Cancelled);
                }
            };

            let output = match output_result {
                Ok(out) => out,
                Err(AgentError::CircuitOpen(msg)) => {
                    tracing::warn!("Circuit breaker open in loop '{}': {}. Pausing loop.", self.name, msg);
                    // Wait with cancellation support
                    tokio::select! {
                        _ = tokio::time::sleep(tokio::time::Duration::from_secs(30)) => {},
                        _ = cancel_token.cancelled() => {
                            return Err(AgentError::Cancelled);
                        }
                    }
                    continue;
                }
                Err(e) => return Err(e),
            };

            // Extract proximity for progress tracking
            let new_proximity = output
                .data
                .get("proximity")
                .and_then(|p| p.as_f64())
                .map(|p| p as f32)
                .unwrap_or(output.confidence);

            current_context.proximity = new_proximity;

            // Track stagnation
            if (new_proximity - last_proximity).abs() < 0.05 {
                stagnation_count += 1;
            } else {
                stagnation_count = 0;
            }
            last_proximity = new_proximity;

            // Check termination condition
            let should_stop = (self.condition)(&output);
            let action_stop = matches!(
                output.next_action,
                Some(NextAction::Stop) | Some(NextAction::PuzzleSolved)
            );

            // Add iteration metadata
            let mut enriched_output = output.clone();
            enriched_output.data.insert(
                "loop_iteration".to_string(),
                serde_json::Value::Number((iteration + 1).into()),
            );
            enriched_output.data.insert(
                "stagnation_count".to_string(),
                serde_json::Value::Number(stagnation_count.into()),
            );

            outputs.push(enriched_output);

            if should_stop || action_stop {
                tracing::info!(
                    "Loop '{}' terminated after {} iterations (proximity: {:.2})",
                    self.name,
                    iteration + 1,
                    new_proximity
                );
                break;
            }

            // Self-correction
            if let Some(ref modifier) = self.context_modifier {
                current_context = modifier(&current_context, &output);
            }

            // Adaptive behavior
            if stagnation_count >= self.stagnation_threshold {
                let failed_approach = format!(
                    "Stagnated at proximity {:.0}% after {} checks",
                    new_proximity * 100.0,
                    stagnation_count
                );
                current_context.planning.failed_approaches.push(failed_approach);
                stagnation_count = 0;
            }

            current_context.previous_outputs.push(output.result.clone());

            // Delay with cancellation support
            if self.delay_ms > 0 {
                tokio::select! {
                    _ = tokio::time::sleep(tokio::time::Duration::from_millis(self.delay_ms)) => {},
                    _ = cancel_token.cancelled() => {
                        tracing::info!("Loop '{}' cancelled during delay", self.name);
                        return Err(AgentError::Cancelled);
                    }
                }
            }
        }

        Ok(outputs)
    }
}

#[async_trait]
impl Workflow for LoopWorkflow {
    fn name(&self) -> &str {
        &self.name
    }

    async fn execute(&self, context: &AgentContext) -> AgentResult<Vec<AgentOutput>> {
        self.execute_with_cancellation(context, CancellationToken::new())
            .await
    }

    async fn execute_cancellable(
        &self,
        context: &AgentContext,
        cancel_token: CancellationToken,
    ) -> AgentResult<Vec<AgentOutput>> {
        self.execute_with_cancellation(context, cancel_token).await
    }
}

/// Create an adaptive loop with planner integration
/// This loop can revise its search strategy based on stagnation
pub fn create_adaptive_loop(
    observer: Arc<dyn Agent>,
    max_iter: usize,
    delay: u64,
) -> LoopWorkflow {
    LoopWorkflow::new(
        "AdaptiveLoop",
        observer,
        Box::new(|output| {
            matches!(output.next_action, Some(NextAction::PuzzleSolved)) || output.confidence > 0.85
        }),
    )
    .max_iterations(max_iter)
    .delay_ms(delay)
    .stagnation_threshold(2) // Lower threshold for faster adaptation
    .with_context_modifier(Box::new(|ctx, output| {
        let mut new_ctx = ctx.clone();
        
        // Update proximity
        new_ctx.proximity = output.confidence;
        
        // Determine new strategy based on progress
        let strategy = if output.confidence > 0.7 {
            crate::agents::traits::SearchStrategy::Verify
        } else if output.confidence > 0.4 {
            crate::agents::traits::SearchStrategy::Focus
        } else {
            crate::agents::traits::SearchStrategy::Explore
        };
        
        new_ctx.planning.strategy = strategy;
        new_ctx.previous_outputs.push(output.result.clone());
        
        new_ctx
    }))
}
