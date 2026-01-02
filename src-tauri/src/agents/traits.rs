//! Agent trait definitions and shared types
//! Core abstractions for the multi-agent system

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Result type for agent operations
pub type AgentResult<T> = Result<T, AgentError>;

/// Agent error types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AgentError {
    /// Failed to process input
    ProcessingError(String),
    /// External service failed (e.g., AI API)
    ServiceError(String),
    /// Invalid state or configuration
    ConfigError(String),
    /// Agent timed out
    Timeout,
    /// Agent was cancelled
    Cancelled,
}

impl std::fmt::Display for AgentError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AgentError::ProcessingError(msg) => write!(f, "Processing error: {}", msg),
            AgentError::ServiceError(msg) => write!(f, "Service error: {}", msg),
            AgentError::ConfigError(msg) => write!(f, "Config error: {}", msg),
            AgentError::Timeout => write!(f, "Agent timed out"),
            AgentError::Cancelled => write!(f, "Agent was cancelled"),
        }
    }
}

impl std::error::Error for AgentError {}

/// Context passed to agents during execution
#[derive(Debug, Clone, Default)]
pub struct AgentContext {
    /// Current URL being viewed
    pub current_url: String,
    /// Current page title
    pub current_title: String,
    /// Current page content (text)
    pub page_content: String,
    /// Current puzzle ID
    pub puzzle_id: String,
    /// Current puzzle clue
    pub puzzle_clue: String,
    /// Target URL pattern for current puzzle
    pub target_pattern: String,
    /// Current proximity score
    pub proximity: f32,
    /// Ghost personality/mood
    pub ghost_mood: String,
    /// Hints already revealed
    pub hints_revealed: usize,
    /// Available hints
    pub hints: Vec<String>,
    /// Custom metadata
    pub metadata: HashMap<String, String>,
}

/// Output from an agent
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentOutput {
    /// Agent name that produced this output
    pub agent_name: String,
    /// Main result/response
    pub result: String,
    /// Confidence score (0.0 - 1.0)
    pub confidence: f32,
    /// Additional data
    pub data: HashMap<String, serde_json::Value>,
    /// Suggested next action
    pub next_action: Option<NextAction>,
}

/// Suggested next action from an agent
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum NextAction {
    /// Continue to next agent
    Continue,
    /// Repeat current operation
    Retry,
    /// Puzzle was solved
    PuzzleSolved,
    /// Show specific hint
    ShowHint(usize),
    /// Generate new puzzle
    GeneratePuzzle,
    /// Stop processing
    Stop,
}

/// Core agent trait - all agents implement this
#[async_trait]
pub trait Agent: Send + Sync {
    /// Get agent name
    fn name(&self) -> &str;

    /// Get agent description
    fn description(&self) -> &str;

    /// Process context and produce output
    async fn process(&self, context: &AgentContext) -> AgentResult<AgentOutput>;

    /// Check if agent can handle this context
    fn can_handle(&self, context: &AgentContext) -> bool {
        // Default: can handle any context
        let _ = context;
        true
    }

    /// Reset agent state
    fn reset(&mut self) {
        // Default: no state to reset
    }
}

/// Agent priority for ordering
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum AgentPriority {
    Critical = 0,
    High = 1,
    Normal = 2,
    Low = 3,
    Background = 4,
}
