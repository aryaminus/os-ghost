//! Server State Management
//!
//! Manages shared state for the headless server including:
//! - Active agents and their status
//! - Pending actions and their approval state
//! - Workflow recordings and executions
//! - Memory entries and statistics
//! - Connection status

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Server runtime state
#[derive(Clone, Debug)]
pub struct ServerState {
    /// Whether the server is connected to AI providers
    pub connected: bool,
    /// Map of active agent IDs to their status
    pub active_agents: HashMap<String, AgentStatus>,
    /// List of pending actions awaiting approval
    pub pending_actions: Vec<PendingAction>,
    /// List of recorded workflows
    pub workflows: Vec<WorkflowInfo>,
    /// Total memory entries count
    pub memory_entries: usize,
    /// Server start time
    pub start_time: chrono::DateTime<chrono::Utc>,
    /// API configuration
    pub api_config: ApiConfig,
}

impl ServerState {
    /// Create new empty server state
    pub fn new() -> Self {
        Self {
            connected: false,
            active_agents: HashMap::new(),
            pending_actions: Vec::new(),
            workflows: Vec::new(),
            memory_entries: 0,
            start_time: chrono::Utc::now(),
            api_config: ApiConfig::default(),
        }
    }

    /// Get uptime in seconds
    pub fn uptime_secs(&self) -> i64 {
        let now = chrono::Utc::now();
        now.signed_duration_since(self.start_time).num_seconds()
    }

    /// Add an agent to active agents
    pub fn add_agent(&mut self, id: String, name: String, agent_type: String) {
        self.active_agents.insert(
            id.clone(),
            AgentStatus {
                id,
                name,
                agent_type,
                status: "idle".to_string(),
                last_activity: chrono::Utc::now(),
            },
        );
    }

    /// Remove an agent from active agents
    pub fn remove_agent(&mut self, id: &str) {
        self.active_agents.remove(id);
    }

    /// Update agent status
    pub fn update_agent_status(&mut self, id: &str, status: &str) {
        if let Some(agent) = self.active_agents.get_mut(id) {
            agent.status = status.to_string();
            agent.last_activity = chrono::Utc::now();
        }
    }

    /// Add a pending action
    pub fn add_pending_action(&mut self, action: PendingAction) {
        self.pending_actions.push(action);
    }

    /// Remove a pending action by ID
    pub fn remove_pending_action(&mut self, id: &str) -> Option<PendingAction> {
        if let Some(pos) = self.pending_actions.iter().position(|a| a.id == id) {
            Some(self.pending_actions.remove(pos))
        } else {
            None
        }
    }

    /// Add a workflow
    pub fn add_workflow(&mut self, workflow: WorkflowInfo) {
        self.workflows.push(workflow);
    }

    /// Remove a workflow by ID
    pub fn remove_workflow(&mut self, id: &str) {
        self.workflows.retain(|w| w.id != id);
    }

    /// Get a workflow by ID
    pub fn get_workflow(&self, id: &str) -> Option<&WorkflowInfo> {
        self.workflows.iter().find(|w| w.id == id)
    }

    /// Update memory entries count
    pub fn update_memory_count(&mut self, count: usize) {
        self.memory_entries = count;
    }
}

impl Default for ServerState {
    fn default() -> Self {
        Self::new()
    }
}

/// Agent status information
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AgentStatus {
    pub id: String,
    pub name: String,
    pub agent_type: String,
    pub status: String,
    pub last_activity: chrono::DateTime<chrono::Utc>,
}

/// Pending action information
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PendingAction {
    pub id: String,
    pub action_type: String,
    pub description: String,
    pub risk_level: String,
    pub requested_at: chrono::DateTime<chrono::Utc>,
    pub requires_approval: bool,
}

/// Workflow information
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct WorkflowInfo {
    pub id: String,
    pub name: String,
    pub description: String,
    pub step_count: usize,
    pub execution_count: u32,
    pub success_rate: f32,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub enabled: bool,
}

/// API configuration
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ApiConfig {
    pub version: String,
    pub max_concurrent_requests: usize,
    pub request_timeout_secs: u64,
    pub enable_audit_log: bool,
}

impl Default for ApiConfig {
    fn default() -> Self {
        Self {
            version: "v1".to_string(),
            max_concurrent_requests: 10,
            request_timeout_secs: 60,
            enable_audit_log: true,
        }
    }
}

/// Task execution request
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ExecuteRequest {
    pub task: String,
    pub autonomy_level: Option<String>,
    pub context: Option<serde_json::Value>,
}

/// Task execution response
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ExecuteResponse {
    pub task_id: String,
    pub status: String,
    pub result: Option<serde_json::Value>,
    pub error: Option<String>,
    pub actions_created: Vec<String>,
}

/// Recording start request
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RecordingRequest {
    pub name: String,
    pub description: String,
    pub start_url: String,
}

/// Recording status response
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RecordingResponse {
    pub recording_id: String,
    pub status: String,
    pub steps_recorded: usize,
    pub started_at: chrono::DateTime<chrono::Utc>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_server_state_new() {
        let state = ServerState::new();
        assert!(!state.connected);
        assert_eq!(state.active_agents.len(), 0);
        assert_eq!(state.pending_actions.len(), 0);
        assert_eq!(state.workflows.len(), 0);
        assert_eq!(state.memory_entries, 0);
    }

    #[test]
    fn test_add_and_remove_agent() {
        let mut state = ServerState::new();

        state.add_agent(
            "agent-1".to_string(),
            "Test Agent".to_string(),
            "operator".to_string(),
        );
        assert_eq!(state.active_agents.len(), 1);

        state.remove_agent("agent-1");
        assert_eq!(state.active_agents.len(), 0);
    }

    #[test]
    fn test_pending_actions() {
        let mut state = ServerState::new();

        let action = PendingAction {
            id: "action-1".to_string(),
            action_type: "click".to_string(),
            description: "Click button".to_string(),
            risk_level: "low".to_string(),
            requested_at: chrono::Utc::now(),
            requires_approval: true,
        };

        state.add_pending_action(action);
        assert_eq!(state.pending_actions.len(), 1);

        let removed = state.remove_pending_action("action-1");
        assert!(removed.is_some());
        assert_eq!(state.pending_actions.len(), 0);
    }
}
