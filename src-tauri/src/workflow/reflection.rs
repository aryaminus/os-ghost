//! Reflection Workflow - Generator-Critic loop pattern
//! Implements iterative refinement through self-correction
//!
//! This workflow implements the "Reflection" pattern from Chapter 4 of Agentic Design Patterns:
//! - Generator (e.g., Narrator) produces initial output
//! - Critic evaluates the output against criteria
//! - If rejected, Generator refines based on feedback
//! - Loop continues until approved or max iterations reached

use super::Workflow;
use crate::agents::critic::CriticAgent;
use crate::agents::traits::{Agent, AgentContext, AgentOutput, AgentResult, NextAction};
use async_trait::async_trait;
use std::sync::Arc;

/// Configuration for reflection workflow
#[derive(Clone)]
pub struct ReflectionConfig {
    /// Maximum number of refinement iterations
    pub max_iterations: usize,
    /// Delay between iterations (ms)
    pub delay_ms: u64,
    /// Whether to include all iterations in output
    pub include_history: bool,
}

impl Default for ReflectionConfig {
    fn default() -> Self {
        Self {
            max_iterations: 3,
            delay_ms: 100,
            include_history: false,
        }
    }
}

/// Reflection workflow implementing Generator-Critic loop
pub struct ReflectionWorkflow {
    name: String,
    /// The generator agent (e.g., Narrator)
    generator: Arc<dyn Agent>,
    /// The critic agent for evaluation
    critic: Arc<CriticAgent>,
    /// Configuration
    config: ReflectionConfig,
}

impl ReflectionWorkflow {
    pub fn new(
        name: &str,
        generator: Arc<dyn Agent>,
        critic: Arc<CriticAgent>,
    ) -> Self {
        Self {
            name: name.to_string(),
            generator,
            critic,
            config: ReflectionConfig::default(),
        }
    }

    /// Set configuration
    pub fn with_config(mut self, config: ReflectionConfig) -> Self {
        self.config = config;
        self
    }

    /// Set max iterations
    pub fn max_iterations(mut self, max: usize) -> Self {
        self.config.max_iterations = max;
        self
    }
}

#[async_trait]
impl Workflow for ReflectionWorkflow {
    fn name(&self) -> &str {
        &self.name
    }

    async fn execute(&self, context: &AgentContext) -> AgentResult<Vec<AgentOutput>> {
        let mut outputs = Vec::new();
        let mut current_context = context.clone();
        let mut approved = false;

        for iteration in 0..self.config.max_iterations {
            tracing::debug!(
                "Reflection iteration {} of {}",
                iteration + 1,
                self.config.max_iterations
            );

            // Step 1: Generate output
            let generator_output = self.generator.process(&current_context).await?;
            let generated_text = generator_output.result.clone();

            if self.config.include_history || iteration == 0 {
                outputs.push(generator_output.clone());
            }

            // Step 2: Critique the output
            // Add the generated text to context for the critic
            current_context.previous_outputs.push(generated_text.clone());
            current_context.metadata.insert(
                "narrator_output".to_string(),
                generated_text.clone(),
            );

            let feedback = self.critic.critique(&generated_text, &current_context).await?;

            // Step 3: Check if approved
            if feedback.approved {
                tracing::info!(
                    "Reflection approved after {} iterations. Quality: {:.0}%",
                    iteration + 1,
                    feedback.quality_score * 100.0
                );
                approved = true;

                // Update the last output with approval status
                if let Some(last) = outputs.last_mut() {
                    last.data.insert(
                        "reflection_approved".to_string(),
                        serde_json::Value::Bool(true),
                    );
                    last.data.insert(
                        "reflection_iterations".to_string(),
                        serde_json::Value::Number((iteration + 1).into()),
                    );
                    last.data.insert(
                        "quality_score".to_string(),
                        serde_json::Value::Number(
                            serde_json::Number::from_f64(feedback.quality_score as f64).unwrap(),
                        ),
                    );
                }

                break;
            }

            // Step 4: Prepare for refinement
            tracing::debug!(
                "Reflection rejected: {}. Refining...",
                feedback.critique
            );

            // Update context with feedback for next iteration
            current_context.last_reflection = Some(feedback.clone());
            current_context.reflection_iterations = iteration + 1;

            // If not the last iteration, try to improve
            if iteration < self.config.max_iterations - 1 {
                // Generate improved output based on feedback
                let improved = self
                    .critic
                    .suggest_improvement(&generated_text, &feedback, &current_context)
                    .await?;

                // Replace the last output with the improved version
                if !outputs.is_empty() && !self.config.include_history {
                    outputs.pop();
                }

                let improved_output = AgentOutput {
                    agent_name: self.generator.name().to_string(),
                    result: improved,
                    confidence: feedback.quality_score,
                    data: {
                        let mut data = std::collections::HashMap::new();
                        data.insert(
                            "reflection_iteration".to_string(),
                            serde_json::Value::Number((iteration + 1).into()),
                        );
                        data.insert(
                            "refined".to_string(),
                            serde_json::Value::Bool(true),
                        );
                        data
                    },
                    next_action: Some(NextAction::Continue),
                };

                outputs.push(improved_output);

                // Delay before next iteration
                if self.config.delay_ms > 0 {
                    tokio::time::sleep(tokio::time::Duration::from_millis(self.config.delay_ms))
                        .await;
                }
            }
        }

        // If not approved after all iterations, mark the final output
        if !approved {
            tracing::warn!(
                "Reflection exhausted {} iterations without full approval",
                self.config.max_iterations
            );
            if let Some(last) = outputs.last_mut() {
                last.data.insert(
                    "reflection_approved".to_string(),
                    serde_json::Value::Bool(false),
                );
                last.data.insert(
                    "reflection_exhausted".to_string(),
                    serde_json::Value::Bool(true),
                );
            }
        }

        Ok(outputs)
    }
}

/// Create a narrator with reflection (Generator-Critic loop)
pub fn create_narrator_with_reflection(
    narrator: Arc<dyn Agent>,
    critic: Arc<CriticAgent>,
    max_iterations: usize,
) -> ReflectionWorkflow {
    ReflectionWorkflow::new("NarratorReflection", narrator, critic)
        .max_iterations(max_iterations)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_reflection_config_default() {
        let config = ReflectionConfig::default();
        assert_eq!(config.max_iterations, 3);
        assert!(!config.include_history);
    }
}
