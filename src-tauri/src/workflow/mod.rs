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

pub use loop_agent::{create_adaptive_loop, LoopWorkflow};
pub use parallel::ParallelWorkflow;
pub use planning::{create_intelligent_pipeline, PlanningWorkflow};
pub use recording::{
    RecordingProgress, Workflow as RecordedWorkflow, WorkflowActionType, WorkflowRecorder,
    WorkflowStep, WorkflowStore,
};
pub use reflection::{create_narrator_with_reflection, ReflectionConfig, ReflectionWorkflow};
pub use replay::{ReplayProgress, ReplayResult, StepResult, VerificationResult, WorkflowReplayer};
pub use sequential::SequentialWorkflow;

/// Export workflow to DroidClaw-style JSON format
/// This allows workflows to be shared and reused across platforms
pub fn export_workflow_to_json(workflow: &RecordedWorkflow) -> String {
    use crate::memory::advanced::{ExportedWorkflow, WorkflowStep as ExportStep};
    
    let steps: Vec<ExportStep> = workflow.steps.iter().map(|step| {
        // Convert action to human-readable goal
        let goal = format!("{}: {}", step.step_number, step.description);
        
        ExportStep {
            app: None,
            goal,
            form_data: None,
        }
    }).collect();
    
    let exported = ExportedWorkflow {
        name: workflow.name.clone(),
        steps,
    };
    
    exported.to_json()
}

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
