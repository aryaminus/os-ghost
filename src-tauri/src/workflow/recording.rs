//! Workflow Recording System
//!
//! Records user actions as reusable workflows. Captures clicks, form fills,
//! navigation, and other interactions for later replay.

use crate::ai::vision::VisualElement;
use crate::capture::vision::VisionCapture;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

/// A recorded workflow
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Workflow {
    /// Unique workflow ID
    pub id: String,
    /// Human-readable name
    pub name: String,
    /// Description of what this workflow does
    pub description: String,
    /// Steps in the workflow
    pub steps: Vec<WorkflowStep>,
    /// Starting URL
    pub start_url: String,
    /// When workflow was created
    pub created_at: u64,
    /// Last modified timestamp
    pub modified_at: u64,
    /// How many times this workflow has been executed
    pub execution_count: u32,
    /// Success rate (0.0 - 1.0)
    pub success_rate: f32,
    /// Average execution time
    pub avg_execution_time_secs: f64,
    /// Tags for organization
    pub tags: Vec<String>,
    /// Whether workflow is enabled
    pub enabled: bool,
    /// Trigger conditions (when to auto-run)
    pub triggers: Vec<WorkflowTrigger>,
}

/// A single step in a workflow
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowStep {
    /// Step number (0-indexed)
    pub step_number: u32,
    /// Type of action
    pub action_type: WorkflowActionType,
    /// Description of what this step does
    pub description: String,
    /// Visual context (screenshot + detected elements)
    pub visual_context: Option<VisualContext>,
    /// Expected outcome
    pub expected_outcome: String,
    /// Timeout for this step (seconds)
    pub timeout_secs: u32,
    /// Whether to continue on error
    pub continue_on_error: bool,
    /// Custom parameters
    pub parameters: HashMap<String, serde_json::Value>,
}

/// Types of workflow actions
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowActionType {
    /// Navigate to URL
    Navigate { url: String },
    /// Click on an element
    Click {
        element_description: String,
        coordinates: Option<(f32, f32)>, // Normalized coordinates
    },
    /// Fill an input field
    Fill {
        field_description: String,
        value: String,
    },
    /// Select from dropdown
    Select {
        dropdown_description: String,
        option: String,
    },
    /// Scroll the page
    Scroll {
        direction: ScrollDirection,
        amount: u32,
    },
    /// Wait for element/condition
    Wait {
        condition: WaitCondition,
        timeout_secs: u32,
    },
    /// Press keyboard key
    KeyPress { key: String, modifiers: Vec<String> },
    /// Hover over element
    Hover { element_description: String },
    /// Take screenshot
    Screenshot,
    /// Verify element exists
    Verify {
        element_description: String,
        should_exist: bool,
    },
    /// Conditional branch
    If {
        condition: String,
        then_steps: Vec<WorkflowStep>,
        else_steps: Option<Vec<WorkflowStep>>,
    },
    /// Loop actions
    Loop {
        condition: String,
        max_iterations: u32,
        steps: Vec<WorkflowStep>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ScrollDirection {
    Up,
    Down,
    Left,
    Right,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WaitCondition {
    ElementVisible { description: String },
    ElementHidden { description: String },
    Duration { seconds: u32 },
    PageLoaded,
    Custom { script: String },
}

/// Visual context for a step
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VisualContext {
    /// Screenshot bytes (base64 encoded)
    pub screenshot_base64: String,
    /// Detected elements at time of recording
    pub detected_elements: Vec<VisualElement>,
    /// Page URL
    pub page_url: String,
    /// Page title
    pub page_title: String,
    /// Timestamp
    pub timestamp: u64,
}

/// Triggers for automatic workflow execution
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowTrigger {
    /// When visiting specific URL pattern
    UrlPattern { pattern: String },
    /// When intent matches
    IntentMatch { intent: String },
    /// At specific time (cron expression)
    Scheduled { cron: String },
    /// When idle for duration
    OnIdle { duration_secs: u64 },
    /// Manual trigger only
    Manual,
}

/// Workflow recorder
#[allow(dead_code)]
pub struct WorkflowRecorder {
    /// Currently recording workflow
    current_workflow: Arc<Mutex<Option<Workflow>>>,
    /// Recording state
    is_recording: Arc<Mutex<bool>>,
    /// Vision capture for screenshots
    vision_capture: Arc<VisionCapture>,
    /// Recording start time
    recording_start: Arc<Mutex<Option<Instant>>>,
    /// Step counter
    step_counter: Arc<Mutex<u32>>,
}

impl WorkflowRecorder {
    /// Create a new workflow recorder
    pub fn new(vision_capture: Arc<VisionCapture>) -> Self {
        Self {
            current_workflow: Arc::new(Mutex::new(None)),
            is_recording: Arc::new(Mutex::new(false)),
            vision_capture,
            recording_start: Arc::new(Mutex::new(None)),
            step_counter: Arc::new(Mutex::new(0)),
        }
    }

    /// Start recording a new workflow
    pub fn start_recording(
        &self,
        name: String,
        description: String,
        start_url: String,
    ) -> Result<String, String> {
        let mut is_recording = self.is_recording.lock().unwrap();

        if *is_recording {
            return Err("Already recording a workflow".to_string());
        }

        let workflow_name = name.clone();

        let workflow = Workflow {
            id: generate_workflow_id(),
            name,
            description,
            steps: Vec::new(),
            start_url,
            created_at: current_timestamp_secs(),
            modified_at: current_timestamp_secs(),
            execution_count: 0,
            success_rate: 0.0,
            avg_execution_time_secs: 0.0,
            tags: Vec::new(),
            enabled: true,
            triggers: vec![WorkflowTrigger::Manual],
        };

        *self.current_workflow.lock().unwrap() = Some(workflow);
        *is_recording = true;
        *self.recording_start.lock().unwrap() = Some(Instant::now());
        *self.step_counter.lock().unwrap() = 0;

        tracing::info!("Started recording workflow: {}", workflow_name);

        Ok(self
            .current_workflow
            .lock()
            .unwrap()
            .as_ref()
            .unwrap()
            .id
            .clone())
    }

    /// Stop recording and return the workflow
    pub fn stop_recording(&self) -> Result<Workflow, String> {
        let mut is_recording = self.is_recording.lock().unwrap();

        if !*is_recording {
            return Err("Not currently recording".to_string());
        }

        let mut workflow = self.current_workflow.lock().unwrap();

        if let Some(ref mut wf) = *workflow {
            wf.modified_at = current_timestamp_secs();

            let duration = self
                .recording_start
                .lock()
                .unwrap()
                .map(|start| start.elapsed())
                .unwrap_or(Duration::from_secs(0));

            tracing::info!(
                "Stopped recording workflow '{}' with {} steps (took {:?})",
                wf.name,
                wf.steps.len(),
                duration
            );

            *is_recording = false;
            return Ok(wf.clone());
        }

        Err("No workflow to stop".to_string())
    }

    /// Record a navigation action
    pub fn record_navigation(&self, url: String) -> Result<(), String> {
        self.add_step(
            WorkflowActionType::Navigate { url },
            "Navigate to URL".to_string(),
        )
    }

    /// Record a click action
    pub fn record_click(
        &self,
        element_description: String,
        coordinates: Option<(f32, f32)>,
    ) -> Result<(), String> {
        let desc = format!("Click on '{}'", element_description);
        self.add_step(
            WorkflowActionType::Click {
                element_description,
                coordinates,
            },
            desc,
        )
    }

    /// Record a form fill action
    pub fn record_fill(&self, field_description: String, value: String) -> Result<(), String> {
        // Mask value in description for privacy
        let masked_value = if value.len() > 3 {
            format!("{}***", &value[..3])
        } else {
            "***".to_string()
        };

        let desc = format!("Fill '{}' with '{}'", field_description, masked_value);
        self.add_step(
            WorkflowActionType::Fill {
                field_description,
                value,
            },
            desc,
        )
    }

    /// Record a scroll action
    pub fn record_scroll(&self, direction: ScrollDirection, amount: u32) -> Result<(), String> {
        let desc = format!("Scroll {:?} by {}", direction, amount);
        self.add_step(WorkflowActionType::Scroll { direction, amount }, desc)
    }

    /// Record a wait action
    pub fn record_wait(&self, condition: WaitCondition, timeout_secs: u32) -> Result<(), String> {
        let desc = format!("Wait for {:?} (timeout: {}s)", condition, timeout_secs);
        self.add_step(
            WorkflowActionType::Wait {
                condition,
                timeout_secs,
            },
            desc,
        )
    }

    /// Record a key press action
    pub fn record_key_press(&self, key: String, modifiers: Vec<String>) -> Result<(), String> {
        let desc = if modifiers.is_empty() {
            format!("Press '{}'", key)
        } else {
            format!("Press {}+'{}'", modifiers.join("+"), key)
        };
        self.add_step(WorkflowActionType::KeyPress { key, modifiers }, desc)
    }

    /// Record a screenshot action
    pub fn record_screenshot(&self) -> Result<(), String> {
        self.add_step(
            WorkflowActionType::Screenshot,
            "Take screenshot".to_string(),
        )
    }

    /// Add a step to the current workflow
    fn add_step(&self, action_type: WorkflowActionType, description: String) -> Result<(), String> {
        if !*self.is_recording.lock().unwrap() {
            return Err("Not recording".to_string());
        }

        let mut counter = self.step_counter.lock().unwrap();
        let step_number = *counter;
        *counter += 1;

        // Capture visual context if vision is available
        let visual_context = None; // Would capture screenshot here

        let step = WorkflowStep {
            step_number,
            action_type,
            description,
            visual_context,
            expected_outcome: "Step completed successfully".to_string(),
            timeout_secs: 30,
            continue_on_error: false,
            parameters: HashMap::new(),
        };

        if let Some(ref mut workflow) = *self.current_workflow.lock().unwrap() {
            workflow.steps.push(step);
            workflow.modified_at = current_timestamp_secs();

            tracing::debug!(
                "Recorded step {} for workflow '{}'",
                step_number,
                workflow.name
            );
            Ok(())
        } else {
            Err("No active workflow".to_string())
        }
    }

    /// Check if currently recording
    pub fn is_recording(&self) -> bool {
        *self.is_recording.lock().unwrap()
    }

    /// Get current recording progress
    pub fn get_progress(&self) -> Option<RecordingProgress> {
        if !self.is_recording() {
            return None;
        }

        let workflow = self.current_workflow.lock().unwrap();
        let start = self.recording_start.lock().unwrap();

        workflow.as_ref().map(|wf| RecordingProgress {
            workflow_name: wf.name.clone(),
            steps_recorded: wf.steps.len() as u32,
            duration_secs: start.map(|s| s.elapsed().as_secs()).unwrap_or(0),
        })
    }

    /// Cancel current recording
    pub fn cancel_recording(&self) {
        *self.current_workflow.lock().unwrap() = None;
        *self.is_recording.lock().unwrap() = false;
        *self.recording_start.lock().unwrap() = None;
        *self.step_counter.lock().unwrap() = 0;

        tracing::info!("Cancelled workflow recording");
    }
}

/// Recording progress information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecordingProgress {
    pub workflow_name: String,
    pub steps_recorded: u32,
    pub duration_secs: u64,
}

/// Workflow storage/management
#[derive(Clone)]
pub struct WorkflowStore {
    workflows: Arc<Mutex<HashMap<String, Workflow>>>,
}

impl WorkflowStore {
    /// Create a new workflow store
    pub fn new() -> Self {
        Self {
            workflows: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Save a workflow
    pub fn save(&self, workflow: Workflow) {
        let workflow_id = workflow.id.clone();
        let mut workflows = self.workflows.lock().unwrap();
        workflows.insert(workflow_id.clone(), workflow);
        tracing::info!("Saved workflow '{}'", workflow_id);
    }

    /// Get a workflow by ID
    pub fn get(&self, id: &str) -> Option<Workflow> {
        self.workflows.lock().unwrap().get(id).cloned()
    }

    /// Get all workflows
    pub fn get_all(&self) -> Vec<Workflow> {
        self.workflows.lock().unwrap().values().cloned().collect()
    }

    /// Get workflows by tag
    pub fn get_by_tag(&self, tag: &str) -> Vec<Workflow> {
        self.workflows
            .lock()
            .unwrap()
            .values()
            .filter(|w| w.tags.contains(&tag.to_string()))
            .cloned()
            .collect()
    }

    /// Delete a workflow
    pub fn delete(&self, id: &str) -> bool {
        let mut workflows = self.workflows.lock().unwrap();
        workflows.remove(id).is_some()
    }

    /// Update workflow execution stats
    pub fn record_execution(&self, id: &str, success: bool, duration_secs: f64) {
        let mut workflows = self.workflows.lock().unwrap();

        if let Some(workflow) = workflows.get_mut(id) {
            workflow.execution_count += 1;

            // Update success rate
            let total = workflow.execution_count as f32;
            let prev_success = workflow.success_rate * (total - 1.0);
            workflow.success_rate = (prev_success + if success { 1.0 } else { 0.0 }) / total;

            // Update average execution time
            let prev_total_time = workflow.avg_execution_time_secs * (total as f64 - 1.0);
            workflow.avg_execution_time_secs = (prev_total_time + duration_secs) / total as f64;

            workflow.modified_at = current_timestamp_secs();
        }
    }
}

impl Default for WorkflowStore {
    fn default() -> Self {
        Self::new()
    }
}

/// Generate a unique workflow ID
fn generate_workflow_id() -> String {
    format!("wf_{}", current_timestamp_secs())
}

/// Get current timestamp in seconds
fn current_timestamp_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_workflow_creation() {
        let workflow = Workflow {
            id: "test_123".to_string(),
            name: "Test Workflow".to_string(),
            description: "A test workflow".to_string(),
            steps: Vec::new(),
            start_url: "https://example.com".to_string(),
            created_at: 1234567890,
            modified_at: 1234567890,
            execution_count: 0,
            success_rate: 0.0,
            avg_execution_time_secs: 0.0,
            tags: vec!["test".to_string()],
            enabled: true,
            triggers: vec![WorkflowTrigger::Manual],
        };

        assert_eq!(workflow.name, "Test Workflow");
        assert!(workflow.enabled);
    }

    #[test]
    fn test_workflow_step_creation() {
        let step = WorkflowStep {
            step_number: 0,
            action_type: WorkflowActionType::Navigate {
                url: "https://example.com".to_string(),
            },
            description: "Navigate to example.com".to_string(),
            visual_context: None,
            expected_outcome: "Page loaded".to_string(),
            timeout_secs: 30,
            continue_on_error: false,
            parameters: HashMap::new(),
        };

        assert_eq!(step.step_number, 0);
    }

    #[test]
    fn test_workflow_store() {
        let store = WorkflowStore::new();

        let workflow = Workflow {
            id: "test_123".to_string(),
            name: "Test".to_string(),
            description: "Test".to_string(),
            steps: Vec::new(),
            start_url: "https://example.com".to_string(),
            created_at: 0,
            modified_at: 0,
            execution_count: 0,
            success_rate: 0.0,
            avg_execution_time_secs: 0.0,
            tags: Vec::new(),
            enabled: true,
            triggers: Vec::new(),
        };

        store.save(workflow.clone());
        assert!(store.get("test_123").is_some());

        store.record_execution("test_123", true, 5.0);
        let updated = store.get("test_123").unwrap();
        assert_eq!(updated.execution_count, 1);
    }
}
