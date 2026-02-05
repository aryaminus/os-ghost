//! Safe Workflow Replay System
//!
//! Replays recorded workflows with safety verification. Checks context,
//! verifies outcomes, and allows user intervention.

use crate::actions::action_preview::VisualPreview;
use crate::config::privacy::{AutonomyLevel, PrivacySettings};
use crate::capture::vision::VisionCapture;
use super::recording::{Workflow, WorkflowStep, WorkflowActionType, WaitCondition};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::{Duration, Instant};

/// Result of workflow replay
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReplayResult {
    /// Whether replay succeeded
    pub success: bool,
    /// Number of steps completed
    pub steps_completed: u32,
    /// Total steps in workflow
    pub total_steps: u32,
    /// Duration of replay
    pub duration_secs: f64,
    /// Step-by-step results
    pub step_results: Vec<StepResult>,
    /// Error message (if failed)
    pub error: Option<String>,
    /// Verification results
    pub verifications: Vec<VerificationResult>,
}

/// Result of a single step replay
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StepResult {
    /// Step number
    pub step_number: u32,
    /// Whether step succeeded
    pub success: bool,
    /// Duration of step
    pub duration_secs: f64,
    /// Error message (if failed)
    pub error: Option<String>,
    /// Whether user intervened
    pub user_intervened: bool,
    /// Verification result
    pub verification: Option<VerificationResult>,
}

/// Verification result for a step
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerificationResult {
    /// What was checked
    pub check_type: VerificationType,
    /// Whether check passed
    pub passed: bool,
    /// Expected value
    pub expected: String,
    /// Actual value
    pub actual: String,
    /// Confidence score
    pub confidence: f32,
}

/// Types of verification checks
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VerificationType {
    /// Check URL matches expected
    UrlMatch,
    /// Check page title
    PageTitle,
    /// Check element exists
    ElementExists,
    /// Check element contains text
    ElementText,
    /// Visual comparison (screenshot)
    VisualMatch,
    /// Custom script verification
    Custom,
}

/// Workflow replayer with safety controls
pub struct WorkflowReplayer {
    /// Privacy settings
    privacy_settings: PrivacySettings,
    /// Autonomy level
    autonomy_level: AutonomyLevel,
    /// Vision capture for verification
    vision_capture: Option<Arc<VisionCapture>>,
    /// Whether replay is running
    is_running: bool,
    /// Current step
    current_step: u32,
    /// Whether user requested pause
    should_pause: bool,
    /// User override (skip/cancel)
    user_override: Option<UserOverride>,
}

/// User override action
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum UserOverride {
    /// Skip current step
    Skip,
    /// Cancel entire replay
    Cancel,
    /// Modify step parameters
    Modify { parameters: serde_json::Value },
}

impl WorkflowReplayer {
    /// Create a new workflow replayer
    pub fn new(
        privacy_settings: PrivacySettings,
        autonomy_level: AutonomyLevel,
        vision_capture: Option<Arc<VisionCapture>>,
    ) -> Self {
        Self {
            privacy_settings,
            autonomy_level,
            vision_capture,
            is_running: false,
            current_step: 0,
            should_pause: false,
            user_override: None,
        }
    }

    /// Replay a workflow
    pub async fn replay(&mut self, workflow: &Workflow) -> Result<ReplayResult, String> {
        // Check if workflow is enabled
        if !workflow.enabled {
            return Err("Workflow is disabled".to_string());
        }

        // Check permissions
        if !self.can_replay(workflow) {
            return Err("Insufficient permissions to replay workflow".to_string());
        }

        self.is_running = true;
        self.current_step = 0;
        
        let start_time = Instant::now();
        let mut step_results = Vec::new();
        let mut verifications = Vec::new();
        let mut overall_success = true;

        tracing::info!("Starting workflow replay: '{}' ({} steps)", workflow.name, workflow.steps.len());

        // Navigate to start URL if needed
        if !workflow.start_url.is_empty() {
            tracing::info!("Navigating to start URL: {}", workflow.start_url);
            // Would execute navigation via browser extension
        }

        // Execute each step
        for step in &workflow.steps {
            if !self.is_running {
                tracing::info!("Workflow replay cancelled by user");
                break;
            }

            self.current_step = step.step_number;

            // Check if we should pause
            if self.should_pause || self.should_pause_for_step(step) {
                tracing::info!("Pausing at step {} for user confirmation", step.step_number);
                // Wait for user to resume or override
                tokio::time::sleep(Duration::from_millis(100)).await;
                continue;
            }

            // Execute step
            let step_start = Instant::now();
            
            let (step_success, step_error) = match self.execute_step(step).await {
                Ok(()) => {
                    tracing::info!("Step {} completed successfully", step.step_number);
                    (true, None)
                }
                Err(e) => {
                    tracing::error!("Step {} failed: {}", step.step_number, e);
                    
                    if step.continue_on_error {
                        tracing::warn!("Continuing despite error (continue_on_error=true)");
                        (false, Some(e))
                    } else {
                        overall_success = false;
                        (false, Some(e))
                    }
                }
            };

            let step_duration = step_start.elapsed().as_secs_f64();

            // Verify outcome
            let verification = self.verify_step(step).await;
            if let Some(ref v) = verification {
                verifications.push(v.clone());
            }

            // Record result
            step_results.push(StepResult {
                step_number: step.step_number,
                success: step_success && verification.as_ref().map(|v| v.passed).unwrap_or(true),
                duration_secs: step_duration,
                error: step_error,
                user_intervened: self.user_override.is_some(),
                verification,
            });

            // Check if user cancelled
            if let Some(UserOverride::Cancel) = self.user_override.take() {
                tracing::info!("Workflow replay cancelled by user at step {}", step.step_number);
                overall_success = false;
                break;
            }

            // Delay between steps
            tokio::time::sleep(Duration::from_millis(500)).await;
        }

        self.is_running = false;

        let total_duration = start_time.elapsed().as_secs_f64();
        let completed_steps = step_results.iter().filter(|r| r.success).count() as u32;

        tracing::info!(
            "Workflow replay completed: {}/{} steps succeeded in {:.2}s",
            completed_steps,
            workflow.steps.len(),
            total_duration
        );

        Ok(ReplayResult {
            success: overall_success && completed_steps == workflow.steps.len() as u32,
            steps_completed: completed_steps,
            total_steps: workflow.steps.len() as u32,
            duration_secs: total_duration,
            step_results,
            error: if overall_success { None } else { Some("One or more steps failed".to_string()) },
            verifications,
        })
    }

    /// Execute a single step
    async fn execute_step(&self, step: &WorkflowStep) -> Result<(), String> {
        tracing::debug!("Executing step {}: {:?}", step.step_number, step.action_type);

        match &step.action_type {
            WorkflowActionType::Navigate { url } => {
                // Would: Send navigation command to browser extension
                tracing::info!("Would navigate to: {}", url);
                Ok(())
            }
            WorkflowActionType::Click { element_description, coordinates } => {
                // Would: Find element and click via visual automation
                if let Some(coords) = coordinates {
                    tracing::info!("Would click '{}' at ({:.2}, {:.2})", element_description, coords.0, coords.1);
                } else {
                    tracing::info!("Would find and click '{}'", element_description);
                }
                Ok(())
            }
            WorkflowActionType::Fill { field_description, value } => {
                // Would: Find field and fill via visual automation
                let masked = if value.len() > 3 {
                    format!("{}***", &value[..3])
                } else {
                    "***".to_string()
                };
                tracing::info!("Would fill '{}' with '{}'", field_description, masked);
                Ok(())
            }
            WorkflowActionType::Select { dropdown_description, option } => {
                tracing::info!("Would select '{}' from '{}'", option, dropdown_description);
                Ok(())
            }
            WorkflowActionType::Scroll { direction, amount } => {
                tracing::info!("Would scroll {:?} by {}", direction, amount);
                Ok(())
            }
            WorkflowActionType::Wait { condition, timeout_secs } => {
                tracing::info!("Waiting for {:?} (timeout: {}s)", condition, timeout_secs);
                // Actually wait
                tokio::time::sleep(Duration::from_secs(*timeout_secs as u64)).await;
                Ok(())
            }
            WorkflowActionType::KeyPress { key, modifiers } => {
                let mod_str = if modifiers.is_empty() {
                    key.clone()
                } else {
                    format!("{}+{}", modifiers.join("+"), key)
                };
                tracing::info!("Would press keys: {}", mod_str);
                Ok(())
            }
            WorkflowActionType::Hover { element_description } => {
                tracing::info!("Would hover over '{}'", element_description);
                Ok(())
            }
            WorkflowActionType::Screenshot => {
                tracing::info!("Would take screenshot");
                Ok(())
            }
            WorkflowActionType::Verify { element_description, should_exist } => {
                tracing::info!("Would verify '{}' exists={}", element_description, should_exist);
                // Would: Check if element exists
                Ok(())
            }
            WorkflowActionType::If { condition, .. } => {
                tracing::info!("Conditional branch: {}", condition);
                // Would: Evaluate condition and execute appropriate branch
                Ok(())
            }
            WorkflowActionType::Loop { condition, max_iterations, .. } => {
                tracing::info!("Loop: {} (max {} iterations)", condition, max_iterations);
                // Would: Execute loop
                Ok(())
            }
        }
    }

    /// Verify step outcome
    async fn verify_step(&self, step: &WorkflowStep) -> Option<VerificationResult> {
        // In real implementation, would:
        // 1. Take screenshot
        // 2. Compare with expected visual context
        // 3. Check if element exists
        // 4. Verify page state
        
        // Mock verification
        Some(VerificationResult {
            check_type: VerificationType::VisualMatch,
            passed: true,
            expected: "Expected state".to_string(),
            actual: "Actual state".to_string(),
            confidence: 0.85,
        })
    }

    /// Check if we should pause for this step
    fn should_pause_for_step(&self, step: &WorkflowStep) -> bool {
        match self.autonomy_level {
            AutonomyLevel::Observer => true, // Always pause
            AutonomyLevel::Suggester => true, // Always pause
            AutonomyLevel::Supervised => {
                // Pause for potentially destructive actions
                matches!(
                    step.action_type,
                    WorkflowActionType::Fill { .. } |
                    WorkflowActionType::KeyPress { .. } |
                    WorkflowActionType::If { .. } |
                    WorkflowActionType::Loop { .. }
                )
            }
            AutonomyLevel::Autonomous => {
                // Only pause if explicitly requested
                false
            }
        }
    }

    /// Check if we can replay this workflow
    fn can_replay(&self, workflow: &Workflow) -> bool {
        // Check autonomy level
        if matches!(self.autonomy_level, AutonomyLevel::Observer) {
            return false;
        }

        // Check privacy settings
        if !self.privacy_settings.visual_automation_consent {
            return false;
        }

        // Check site allowlist (if any)
        if !self.privacy_settings.visual_automation_allowlist.is_empty() {
            let site_allowed = self.privacy_settings.visual_automation_allowlist.iter().any(|allowed| {
                workflow.start_url.to_lowercase().contains(&allowed.to_lowercase())
            });
            if !site_allowed {
                return false;
            }
        }

        // Check site blocklist
        let site_blocked = self.privacy_settings.visual_automation_blocklist.iter().any(|blocked| {
            workflow.start_url.to_lowercase().contains(&blocked.to_lowercase())
        });
        if site_blocked {
            return false;
        }

        true
    }

    /// Pause replay
    pub fn pause(&mut self) {
        self.should_pause = true;
        tracing::info!("Workflow replay paused");
    }

    /// Resume replay
    pub fn resume(&mut self) {
        self.should_pause = false;
        tracing::info!("Workflow replay resumed");
    }

    /// Cancel replay
    pub fn cancel(&mut self) {
        self.is_running = false;
        self.user_override = Some(UserOverride::Cancel);
        tracing::info!("Workflow replay cancelled");
    }

    /// Skip current step
    pub fn skip_step(&mut self) {
        self.user_override = Some(UserOverride::Skip);
        tracing::info!("Current step skipped");
    }

    /// Get current replay progress
    pub fn get_progress(&self, workflow: &Workflow) -> ReplayProgress {
        ReplayProgress {
            is_running: self.is_running,
            current_step: self.current_step,
            total_steps: workflow.steps.len() as u32,
            percent_complete: if workflow.steps.is_empty() {
                0.0
            } else {
                (self.current_step as f32 / workflow.steps.len() as f32) * 100.0
            },
            is_paused: self.should_pause,
        }
    }

    /// Get current step being executed
    pub fn get_current_step(&self) -> u32 {
        self.current_step
    }
}

/// Replay progress information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReplayProgress {
    /// Whether replay is running
    pub is_running: bool,
    /// Current step number
    pub current_step: u32,
    /// Total steps
    pub total_steps: u32,
    /// Percentage complete
    pub percent_complete: f32,
    /// Whether replay is paused
    pub is_paused: bool,
}

/// Workflow execution statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowStats {
    /// Total executions
    pub total_executions: u32,
    /// Success rate
    pub success_rate: f32,
    /// Average execution time
    pub avg_execution_time_secs: f64,
    /// Last execution result
    pub last_result: Option<ReplayResult>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_replay_result_creation() {
        let result = ReplayResult {
            success: true,
            steps_completed: 5,
            total_steps: 5,
            duration_secs: 10.5,
            step_results: vec![],
            error: None,
            verifications: vec![],
        };

        assert!(result.success);
        assert_eq!(result.steps_completed, 5);
    }

    #[test]
    fn test_step_result_creation() {
        let result = StepResult {
            step_number: 0,
            success: true,
            duration_secs: 2.5,
            error: None,
            user_intervened: false,
            verification: Some(VerificationResult {
                check_type: VerificationType::UrlMatch,
                passed: true,
                expected: "https://example.com".to_string(),
                actual: "https://example.com".to_string(),
                confidence: 1.0,
            }),
        };

        assert!(result.success);
        assert!(result.verification.as_ref().unwrap().passed);
    }

    #[test]
    fn test_verification_result_creation() {
        let verification = VerificationResult {
            check_type: VerificationType::ElementExists,
            passed: true,
            expected: "Button should exist".to_string(),
            actual: "Button found".to_string(),
            confidence: 0.95,
        };

        assert!(verification.passed);
        assert!(verification.confidence > 0.9);
    }
}
