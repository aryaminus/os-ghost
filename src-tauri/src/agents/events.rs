//! Event System - ADK-style event flow with actions
//!
//! Implements the Event pattern from Google ADK:
//! - Events carry content, metadata, and action deltas
//! - EventActions control workflow behavior (escalate, transfer, state delta)
//! - Event stream enables observability and debugging
//!
//! Reference: Chapter 11 (Goal Setting and Monitoring) and ADK Sessions

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

/// Atomic counter for unique event IDs
static EVENT_COUNTER: AtomicU64 = AtomicU64::new(0);

/// Generate a unique event ID
fn generate_event_id() -> String {
    let counter = EVENT_COUNTER.fetch_add(1, Ordering::SeqCst);
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0);
    format!("evt_{}_{}", timestamp, counter)
}

/// Event author type
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub enum EventAuthor {
    /// User-initiated event
    User,
    /// Agent-generated event
    Agent(String),
    /// System/orchestrator event
    #[default]
    System,
}

/// Event content types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum EventContent {
    /// Plain text message
    Text(String),
    /// Function/tool call request
    FunctionCall {
        name: String,
        arguments: HashMap<String, serde_json::Value>,
    },
    /// Function/tool response
    FunctionResponse {
        name: String,
        result: serde_json::Value,
    },
    /// Error message
    Error(String),
    /// Partial/streaming content
    Partial(String),
}

impl Default for EventContent {
    fn default() -> Self {
        EventContent::Text(String::new())
    }
}

/// Actions that can be triggered by an event
/// These are side effects that the runner/orchestrator should apply
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct EventActions {
    /// State updates to apply (key -> value)
    /// Keys can have scope prefixes: temp:, user:, app:, session: (default)
    pub state_delta: HashMap<String, serde_json::Value>,

    /// Artifact updates (artifact_name -> version increment)
    pub artifact_delta: HashMap<String, u32>,

    /// Transfer control to another agent
    pub transfer_to_agent: Option<String>,

    /// Escalate to human/supervisor
    /// When true, the workflow should pause and wait for human input
    pub escalate: bool,

    /// Skip LLM summarization of tool results
    /// Used when tool output should be passed directly to user
    pub skip_summarization: bool,

    /// Request workflow termination
    pub terminate: bool,

    /// Priority level for this event (higher = more important)
    pub priority: EventPriority,
}

impl EventActions {
    /// Create new EventActions with a state delta
    pub fn with_state<K: Into<String>, V: Into<serde_json::Value>>(key: K, value: V) -> Self {
        let mut actions = Self::default();
        actions.state_delta.insert(key.into(), value.into());
        actions
    }

    /// Add a state delta entry
    pub fn add_state<K: Into<String>, V: Into<serde_json::Value>>(
        mut self,
        key: K,
        value: V,
    ) -> Self {
        self.state_delta.insert(key.into(), value.into());
        self
    }

    /// Request escalation to human
    pub fn escalate(mut self) -> Self {
        self.escalate = true;
        self
    }

    /// Request transfer to another agent
    pub fn transfer_to<S: Into<String>>(mut self, agent_name: S) -> Self {
        self.transfer_to_agent = Some(agent_name.into());
        self
    }

    /// Mark for termination
    pub fn terminate(mut self) -> Self {
        self.terminate = true;
        self
    }
}

/// Event priority levels
#[derive(Debug, Clone, Copy, Serialize, Deserialize, Default, PartialEq, Eq, PartialOrd, Ord)]
pub enum EventPriority {
    /// Background/low priority events
    Low = 0,
    /// Normal priority (default)
    #[default]
    Normal = 1,
    /// High priority - should be processed quickly
    High = 2,
    /// Critical - requires immediate attention
    Critical = 3,
}

/// An event in the agent execution flow
/// Represents a discrete action, message, or state change
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentEvent {
    /// Unique event identifier
    pub id: String,

    /// Who generated this event
    pub author: EventAuthor,

    /// Invocation/request ID this event belongs to
    pub invocation_id: String,

    /// Unix timestamp (milliseconds)
    pub timestamp: u64,

    /// Event content
    pub content: EventContent,

    /// Actions/side effects to apply
    pub actions: EventActions,

    /// Whether this is a partial/streaming event
    pub partial: bool,

    /// Whether this is a final response
    pub is_final: bool,

    /// Custom metadata
    pub metadata: HashMap<String, serde_json::Value>,
}

impl AgentEvent {
    /// Create a new text event from an agent
    pub fn text(agent_name: &str, invocation_id: &str, text: impl Into<String>) -> Self {
        Self {
            id: generate_event_id(),
            author: EventAuthor::Agent(agent_name.to_string()),
            invocation_id: invocation_id.to_string(),
            timestamp: current_timestamp_ms(),
            content: EventContent::Text(text.into()),
            actions: EventActions::default(),
            partial: false,
            is_final: false,
            metadata: HashMap::new(),
        }
    }

    /// Create a system event
    pub fn system(invocation_id: &str, text: impl Into<String>) -> Self {
        Self {
            id: generate_event_id(),
            author: EventAuthor::System,
            invocation_id: invocation_id.to_string(),
            timestamp: current_timestamp_ms(),
            content: EventContent::Text(text.into()),
            actions: EventActions::default(),
            partial: false,
            is_final: false,
            metadata: HashMap::new(),
        }
    }

    /// Create a user event
    pub fn user(invocation_id: &str, text: impl Into<String>) -> Self {
        Self {
            id: generate_event_id(),
            author: EventAuthor::User,
            invocation_id: invocation_id.to_string(),
            timestamp: current_timestamp_ms(),
            content: EventContent::Text(text.into()),
            actions: EventActions::default(),
            partial: false,
            is_final: false,
            metadata: HashMap::new(),
        }
    }

    /// Create a function call event
    pub fn function_call(
        agent_name: &str,
        invocation_id: &str,
        function_name: &str,
        arguments: HashMap<String, serde_json::Value>,
    ) -> Self {
        Self {
            id: generate_event_id(),
            author: EventAuthor::Agent(agent_name.to_string()),
            invocation_id: invocation_id.to_string(),
            timestamp: current_timestamp_ms(),
            content: EventContent::FunctionCall {
                name: function_name.to_string(),
                arguments,
            },
            actions: EventActions::default(),
            partial: false,
            is_final: false,
            metadata: HashMap::new(),
        }
    }

    /// Create a function response event
    pub fn function_response(
        agent_name: &str,
        invocation_id: &str,
        function_name: &str,
        result: serde_json::Value,
    ) -> Self {
        Self {
            id: generate_event_id(),
            author: EventAuthor::Agent(agent_name.to_string()),
            invocation_id: invocation_id.to_string(),
            timestamp: current_timestamp_ms(),
            content: EventContent::FunctionResponse {
                name: function_name.to_string(),
                result,
            },
            actions: EventActions::default(),
            partial: false,
            is_final: false,
            metadata: HashMap::new(),
        }
    }

    /// Create an error event
    pub fn error(agent_name: &str, invocation_id: &str, error: impl Into<String>) -> Self {
        Self {
            id: generate_event_id(),
            author: EventAuthor::Agent(agent_name.to_string()),
            invocation_id: invocation_id.to_string(),
            timestamp: current_timestamp_ms(),
            content: EventContent::Error(error.into()),
            actions: EventActions::default(),
            partial: false,
            is_final: false,
            metadata: HashMap::new(),
        }
    }

    /// Add actions to this event
    pub fn with_actions(mut self, actions: EventActions) -> Self {
        self.actions = actions;
        self
    }

    /// Mark as final response
    pub fn as_final(mut self) -> Self {
        self.is_final = true;
        self
    }

    /// Mark as partial/streaming
    pub fn as_partial(mut self) -> Self {
        self.partial = true;
        self
    }

    /// Add metadata
    pub fn with_metadata<K: Into<String>, V: Into<serde_json::Value>>(
        mut self,
        key: K,
        value: V,
    ) -> Self {
        self.metadata.insert(key.into(), value.into());
        self
    }

    /// Check if this is a final response (ADK pattern)
    pub fn is_final_response(&self) -> bool {
        if self.is_final {
            return true;
        }
        if self.partial {
            return false;
        }
        // Check if there are no pending function calls
        !matches!(self.content, EventContent::FunctionCall { .. })
    }
}

/// Get current timestamp in milliseconds
fn current_timestamp_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

/// Event stream - collects events during an invocation
#[derive(Debug, Default)]
pub struct EventStream {
    events: Vec<AgentEvent>,
    #[allow(dead_code)]
    invocation_id: String,
}

impl EventStream {
    /// Create a new event stream for an invocation
    pub fn new(invocation_id: impl Into<String>) -> Self {
        Self {
            events: Vec::new(),
            invocation_id: invocation_id.into(),
        }
    }

    /// Push an event to the stream
    pub fn push(&mut self, event: AgentEvent) {
        self.events.push(event);
    }

    /// Get all events
    pub fn events(&self) -> &[AgentEvent] {
        &self.events
    }

    /// Get the last event
    pub fn last(&self) -> Option<&AgentEvent> {
        self.events.last()
    }

    /// Get events by author
    pub fn by_author(&self, author: &EventAuthor) -> Vec<&AgentEvent> {
        self.events.iter().filter(|e| &e.author == author).collect()
    }

    /// Get the final response event if present
    pub fn final_response(&self) -> Option<&AgentEvent> {
        self.events.iter().rev().find(|e| e.is_final_response())
    }

    /// Collect all state deltas from events
    pub fn collect_state_deltas(&self) -> HashMap<String, serde_json::Value> {
        let mut state = HashMap::new();
        for event in &self.events {
            for (key, value) in &event.actions.state_delta {
                state.insert(key.clone(), value.clone());
            }
        }
        state
    }

    /// Check if any event requested escalation
    pub fn has_escalation(&self) -> bool {
        self.events.iter().any(|e| e.actions.escalate)
    }

    /// Get the transfer target if any event requested transfer
    pub fn transfer_target(&self) -> Option<&str> {
        self.events
            .iter()
            .rev()
            .find_map(|e| e.actions.transfer_to_agent.as_deref())
    }

    /// Check if any event requested termination
    pub fn should_terminate(&self) -> bool {
        self.events.iter().any(|e| e.actions.terminate)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_event_creation() {
        let event = AgentEvent::text("Observer", "inv_123", "Hello world");
        assert!(event.id.starts_with("evt_"));
        assert_eq!(event.invocation_id, "inv_123");
        assert!(!event.partial);
    }

    #[test]
    fn test_event_actions() {
        let actions = EventActions::with_state("temp:result", "success")
            .add_state("user:preference", "dark")
            .escalate();

        assert!(actions.escalate);
        assert_eq!(actions.state_delta.len(), 2);
        assert!(actions.state_delta.contains_key("temp:result"));
    }

    #[test]
    fn test_event_stream() {
        let mut stream = EventStream::new("inv_001");

        stream.push(AgentEvent::text("Observer", "inv_001", "Analyzing..."));
        stream.push(
            AgentEvent::text("Narrator", "inv_001", "Found a clue!")
                .with_actions(EventActions::with_state("proximity", 0.7))
                .as_final(),
        );

        assert_eq!(stream.events().len(), 2);
        assert!(stream.final_response().is_some());

        let deltas = stream.collect_state_deltas();
        assert!(deltas.contains_key("proximity"));
    }

    #[test]
    fn test_scoped_state_keys() {
        let actions = EventActions::with_state("temp:scratch", "value")
            .add_state("user:name", "Ghost")
            .add_state("app:version", "1.0")
            .add_state("session_count", 5); // No prefix = session scope

        assert!(actions.state_delta.contains_key("temp:scratch"));
        assert!(actions.state_delta.contains_key("user:name"));
        assert!(actions.state_delta.contains_key("app:version"));
        assert!(actions.state_delta.contains_key("session_count"));
    }
}
