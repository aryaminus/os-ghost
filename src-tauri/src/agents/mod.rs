//! Multi-agent system for The OS Ghost
//! Implements ADK-style agent patterns in Rust
//!
//! Enhanced with patterns from "Agentic Design Patterns":
//! - **Observer**: Screen analysis and vision (existing)
//! - **Verifier**: Solution validation (existing)
//! - **Narrator**: Dialogue generation (existing)
//! - **Planner**: Dynamic goal decomposition (NEW - Chapter 6)
//! - **Critic**: Quality control and reflection (NEW - Chapter 4)
//! - **Guardrail**: Safety patterns and content filtering (NEW - Chapter 18)

pub mod critic;
pub mod guardrail;
pub mod narrator;
pub mod observer;
pub mod orchestrator;
pub mod planner;
pub mod traits;
pub mod verifier;

pub use critic::CriticAgent;
pub use guardrail::{GuardrailAgent, SafetyEvaluation, ContentType};
pub use orchestrator::AgentOrchestrator;
pub use planner::PlannerAgent;
pub use traits::{Agent, AgentContext, AgentMode, AgentResult, PlanningContext, ReflectionFeedback, SearchStrategy, SubGoal};
