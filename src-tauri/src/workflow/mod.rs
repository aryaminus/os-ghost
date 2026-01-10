//! Workflow engine for agent orchestration patterns
//! Implements ADK-style workflow patterns: Sequential, Loop, Parallel, Reflection, Planning
//!
//! Enhanced with patterns from "Agentic Design Patterns":
//! - **Sequential**: Chain agents in order (Observer → Verifier → Narrator)
//! - **Parallel**: Run agents concurrently for background checks
//! - **Loop**: Repeat until condition met, with self-correction
//! - **Reflection**: Generator-Critic loop for quality control
//! - **Planning**: Dynamic goal decomposition and strategy adaptation

pub mod loop_agent;
pub mod parallel;
pub mod planning;
pub mod reflection;
pub mod sequential;

pub use loop_agent::{LoopWorkflow, create_hotcold_loop, create_adaptive_loop};
pub use parallel::ParallelWorkflow;
pub use planning::{PlanningWorkflow, create_intelligent_pipeline, create_quick_planning_workflow};
pub use reflection::{ReflectionWorkflow, ReflectionConfig, create_narrator_with_reflection};
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
