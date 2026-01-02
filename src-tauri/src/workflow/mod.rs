//! Workflow engine for agent orchestration patterns
//! Implements ADK-style workflow patterns: Sequential, Loop, Parallel

pub mod loop_agent;
pub mod parallel;
pub mod sequential;

pub use loop_agent::LoopWorkflow;
pub use parallel::ParallelWorkflow;
pub use sequential::SequentialWorkflow;

use crate::agents::traits::{AgentContext, AgentOutput, AgentResult};
use async_trait::async_trait;

/// Workflow trait - defines a reusable execution pattern
#[async_trait]
pub trait Workflow: Send + Sync {
    /// Get workflow name
    fn name(&self) -> &str;

    /// Execute the workflow with given context
    async fn execute(&self, context: &AgentContext) -> AgentResult<Vec<AgentOutput>>;

    /// Check if workflow is complete
    fn is_complete(&self) -> bool {
        false
    }
}
