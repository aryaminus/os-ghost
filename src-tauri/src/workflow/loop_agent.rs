//! Loop Workflow - Repeat until condition met
//! Used for hot/cold feedback loops with self-correction capabilities
//!
//! Enhanced to support the "Self-Correction" aspect of the Reflection pattern:
//! - Modifies context based on previous outputs
//! - Tracks failed approaches for plan revision
//! - Enables adaptive behavior through context mutation

use super::Workflow;
use crate::agents::traits::{Agent, AgentContext, AgentOutput, AgentResult, NextAction};
use async_trait::async_trait;
use std::sync::Arc;

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
}

#[async_trait]
impl Workflow for LoopWorkflow {
    fn name(&self) -> &str {
        &self.name
    }

    async fn execute(&self, context: &AgentContext) -> AgentResult<Vec<AgentOutput>> {
        let mut outputs = Vec::new();
        let mut current_context = context.clone();
        let mut last_proximity: f32 = 0.0;
        let mut stagnation_count: usize = 0;

        for iteration in 0..self.max_iterations {
            // Process with agent
            let output = self.agent.process(&current_context).await?;

            // Extract proximity for progress tracking
            let new_proximity = output
                .data
                .get("proximity")
                .and_then(|p| p.as_f64())
                .map(|p| p as f32)
                .unwrap_or(output.confidence);

            // Update context with output
            current_context.proximity = new_proximity;

            // Track stagnation (no meaningful progress)
            if (new_proximity - last_proximity).abs() < 0.05 {
                stagnation_count += 1;
            } else {
                stagnation_count = 0;
            }
            last_proximity = new_proximity;

            // Check termination condition
            let should_stop = (self.condition)(&output);

            // Check for explicit stop actions
            let action_stop = matches!(
                output.next_action,
                Some(NextAction::Stop) | Some(NextAction::PuzzleSolved)
            );

            // Add iteration metadata to output
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

            // Self-correction: Apply context modifier if provided
            if let Some(ref modifier) = self.context_modifier {
                current_context = modifier(&current_context, &output);
            }

            // Adaptive behavior: Record stagnation as failed approach
            if stagnation_count >= self.stagnation_threshold {
                tracing::debug!(
                    "Loop '{}' detected stagnation ({} iterations without progress)",
                    self.name,
                    stagnation_count
                );

                // Record the current approach as "failed" for self-correction
                let failed_approach = format!(
                    "Stagnated at proximity {:.0}% after {} checks",
                    new_proximity * 100.0,
                    stagnation_count
                );
                current_context
                    .planning
                    .failed_approaches
                    .push(failed_approach);

                // Reset stagnation counter
                stagnation_count = 0;
            }

            // Store output for reflection/self-correction
            current_context.previous_outputs.push(output.result.clone());

            // Delay before next iteration
            if self.delay_ms > 0 {
                tokio::time::sleep(tokio::time::Duration::from_millis(self.delay_ms)).await;
            }
        }

        Ok(outputs)
    }
}

/// Create a hot/cold feedback loop for autonomous monitoring
/// Enhanced with self-correction: tracks progress and adapts when stagnating
pub fn create_hotcold_loop(observer: Arc<dyn Agent>, max_iter: usize, delay: u64) -> LoopWorkflow {
    LoopWorkflow::new(
        "HotColdLoop",
        observer,
        Box::new(|output| {
            // Stop when puzzle is solved or very close
            matches!(output.next_action, Some(NextAction::PuzzleSolved)) || output.confidence > 0.8
        }),
    )
    .max_iterations(max_iter)
    .delay_ms(delay)
    .stagnation_threshold(3)
    .with_context_modifier(Box::new(|ctx, output| {
        // Self-correction: Modify context based on previous output
        let mut new_ctx = ctx.clone();
        
        // Update proximity from output
        if let Some(proximity) = output.data.get("proximity") {
            if let Some(p) = proximity.as_f64() {
                new_ctx.proximity = p as f32;
            }
        }
        
        // Track this output for self-reflection
        new_ctx.previous_outputs.push(output.result.clone());
        
        // If confidence is very low, note the failed approach
        if output.confidence < 0.2 {
            let failed = format!(
                "Low confidence ({:.0}%) at: {}",
                output.confidence * 100.0,
                crate::privacy::redact_pii(&ctx.current_url)
            );
            if !new_ctx.planning.failed_approaches.contains(&failed) {
                new_ctx.planning.failed_approaches.push(failed);
            }
        }
        
        new_ctx
    }))
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
