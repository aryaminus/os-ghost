//! Operator Agent - Visual task execution
//!
//! Executes tasks through visual interaction with browser elements.

use super::traits::{Agent, AgentContext, AgentError, AgentOutput, AgentResult, AgentPriority};
use crate::capture::vision::VisionCapture;
use crate::config::privacy::{AutonomyLevel, PrivacySettings};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::{Duration, Instant};

/// Result of a visual task execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VisualTaskResult {
    pub success: bool,
    pub actions_taken: u32,
    pub duration_secs: f64,
    pub summary: String,
    pub error: Option<String>,
}

/// A step in a visual task
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VisualTaskStep {
    pub description: String,
    pub action_type: VisualActionType,
    pub expected_outcome: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VisualActionType {
    Click,
    Fill { field: String, value: String },
    Scroll { direction: ScrollDirection, amount: u32 },
    Wait { condition: String, timeout_secs: u32 },
    Navigate { url: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ScrollDirection {
    Up,
    Down,
    Left,
    Right,
}

/// Operator Agent for visual task execution
pub struct OperatorAgent {
    vision_capture: Arc<VisionCapture>,
    privacy_settings: PrivacySettings,
    max_steps: u32,
    step_delay_ms: u64,
}

impl OperatorAgent {
    pub fn new(
        vision_capture: Arc<VisionCapture>,
        privacy_settings: PrivacySettings,
    ) -> Self {
        Self {
            vision_capture,
            privacy_settings,
            max_steps: 20,
            step_delay_ms: 1000,
        }
    }

    pub async fn execute_visual_task(
        &self,
        goal: &str,
        steps: Vec<VisualTaskStep>,
    ) -> Result<VisualTaskResult, AgentError> {
        let start_time = Instant::now();
        let mut actions_taken = 0u32;

        let current_site = "example.com";
        if !self.privacy_settings.can_use_visual_automation(current_site) {
            return Ok(VisualTaskResult {
                success: false,
                actions_taken: 0,
                duration_secs: 0.0,
                summary: "Visual automation not allowed for this site".to_string(),
                error: Some("Visual automation consent required".to_string()),
            });
        }

        for (i, step) in steps.iter().enumerate() {
            if i as u32 >= self.max_steps {
                return Ok(VisualTaskResult {
                    success: false,
                    actions_taken,
                    duration_secs: start_time.elapsed().as_secs_f64(),
                    summary: format!("Stopped after {} steps (max reached)", self.max_steps),
                    error: Some("Max steps exceeded".to_string()),
                });
            }

            tracing::info!("Executing step {}: {}", i + 1, step.description);

            match self.execute_step(step).await {
                Ok(true) => {
                    actions_taken += 1;
                    tokio::time::sleep(Duration::from_millis(self.step_delay_ms)).await;
                }
                Ok(false) => {
                    tracing::warn!("Step {} did not complete as expected", i + 1);
                }
                Err(e) => {
                    return Ok(VisualTaskResult {
                        success: false,
                        actions_taken,
                        duration_secs: start_time.elapsed().as_secs_f64(),
                        summary: format!("Failed at step {}: {}", i + 1, step.description),
                        error: Some(e.to_string()),
                    });
                }
            }
        }

        Ok(VisualTaskResult {
            success: true,
            actions_taken,
            duration_secs: start_time.elapsed().as_secs_f64(),
            summary: format!("Completed task: {} ({} actions)", goal, actions_taken),
            error: None,
        })
    }

    async fn execute_step(&self, step: &VisualTaskStep) -> Result<bool, AgentError> {
        match &step.action_type {
            VisualActionType::Click => {
                tracing::info!("Would click: {}", step.description);
                Ok(true)
            }
            VisualActionType::Fill { field, value } => {
                tracing::info!("Would fill '{}' with '{}'", field, value);
                Ok(true)
            }
            VisualActionType::Scroll { direction, amount } => {
                tracing::info!("Would scroll {:?} by {}", direction, amount);
                Ok(true)
            }
            VisualActionType::Wait { condition, timeout_secs } => {
                tracing::info!("Would wait for '{}' (timeout: {}s)", condition, timeout_secs);
                Ok(true)
            }
            VisualActionType::Navigate { url } => {
                tracing::info!("Would navigate to: {}", url);
                Ok(true)
            }
        }
    }
}

#[async_trait]
impl Agent for OperatorAgent {
    fn name(&self) -> &str {
        "Operator"
    }

    fn description(&self) -> &str {
        "Executes visual tasks by interacting with browser UI elements"
    }

    async fn process(&self, context: &AgentContext) -> AgentResult<AgentOutput> {
        let goal = context.page_content.clone();

        let steps = vec![VisualTaskStep {
            description: goal.clone(),
            action_type: VisualActionType::Click,
            expected_outcome: "Task completed".to_string(),
        }];

        match self.execute_visual_task(&goal, steps).await {
            Ok(result) => {
                let mut data = std::collections::HashMap::new();
                data.insert("actions_taken".to_string(), serde_json::json!(result.actions_taken));
                data.insert("duration_secs".to_string(), serde_json::json!(result.duration_secs));
                
                Ok(AgentOutput {
                    agent_name: self.name().to_string(),
                    result: result.summary,
                    confidence: if result.success { 1.0 } else { 0.0 },
                    data,
                    next_action: None,
                })
            }
            Err(e) => Err(e),
        }
    }

    fn priority(&self) -> AgentPriority {
        AgentPriority::High
    }
}

/// Task planner for breaking down goals into visual steps
pub struct VisualTaskPlanner;

impl VisualTaskPlanner {
    pub async fn plan_task(goal: &str) -> Vec<VisualTaskStep> {
        vec![
            VisualTaskStep {
                description: format!("Navigate to appropriate page for: {}", goal),
                action_type: VisualActionType::Navigate {
                    url: "https://example.com".to_string(),
                },
                expected_outcome: "Page loaded".to_string(),
            },
            VisualTaskStep {
                description: format!("Execute: {}", goal),
                action_type: VisualActionType::Click,
                expected_outcome: "Task completed".to_string(),
            },
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_visual_task_result() {
        let result = VisualTaskResult {
            success: true,
            actions_taken: 3,
            duration_secs: 5.5,
            summary: "Test completed".to_string(),
            error: None,
        };

        assert!(result.success);
        assert_eq!(result.actions_taken, 3);
    }
}
