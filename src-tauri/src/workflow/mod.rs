//! Workflow engine for agent orchestration patterns
//! Implements ADK-style workflow patterns: Sequential, Loop, Parallel, Reflection, Planning
//!
//! Enhanced with patterns from "Agentic Design Patterns":
//! - **Sequential**: Chain agents in order (Observer → Verifier → Narrator)
//! - **Parallel**: Run agents concurrently for background checks
//! - **Loop**: Repeat until condition met, with self-correction
//! - **Reflection**: Generator-Critic loop for quality control
//! - **Planning**: Dynamic goal decomposition and strategy adaptation
//!
//! ## Cancellation Support
//! Workflows support graceful cancellation via `CancellationToken`:
//! ```ignore
//! let token = CancellationToken::new();
//! let cancel_handle = token.clone();
//! 
//! // Start workflow in a task
//! let handle = tokio::spawn(async move {
//!     workflow.execute_cancellable(&context, token).await
//! });
//!
//! // Cancel from another task
//! cancel_handle.cancel();
//! ```

pub mod loop_agent;
pub mod parallel;
pub mod planning;
pub mod recording;
pub mod reflection;
pub mod replay;
pub mod sequential;

pub use loop_agent::{LoopWorkflow, create_adaptive_loop};
pub use parallel::ParallelWorkflow;
pub use planning::{PlanningWorkflow, create_intelligent_pipeline};
pub use recording::{Workflow as RecordedWorkflow, WorkflowStep, WorkflowActionType, WorkflowRecorder, WorkflowStore, RecordingProgress};
pub use reflection::{ReflectionWorkflow, ReflectionConfig, create_narrator_with_reflection};
pub use replay::{WorkflowReplayer, ReplayResult, ReplayProgress, StepResult, VerificationResult};
pub use sequential::SequentialWorkflow;

use crate::agents::traits::{AgentContext, AgentError, AgentOutput, AgentResult};
use async_trait::async_trait;
use tokio_util::sync::CancellationToken;

/// Workflow trait - defines a reusable execution pattern
#[async_trait]
pub trait Workflow: Send + Sync {
    /// Get workflow name
    fn name(&self) -> &str;

    /// Execute the workflow with given context
    async fn execute(&self, context: &AgentContext) -> AgentResult<Vec<AgentOutput>>;

    /// Execute the workflow with cancellation support
    /// Default implementation wraps `execute()` with cancellation check
    async fn execute_cancellable(
        &self,
        context: &AgentContext,
        cancel_token: CancellationToken,
    ) -> AgentResult<Vec<AgentOutput>> {
        tokio::select! {
            result = self.execute(context) => result,
            _ = cancel_token.cancelled() => {
                tracing::info!("Workflow '{}' was cancelled", self.name());
                Err(AgentError::Cancelled)
            }
        }
    }

    /// Check if workflow is complete
    fn is_complete(&self) -> bool {
        false
    }
}
