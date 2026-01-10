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
//!
//! ## Best Practices Implemented (2024 Senior Research Audit)
//!
//! - **Fail-Safe Error Handling**: Parse failures in safety-critical agents (Critic, Guardrail)
//!   now default to rejecting content rather than approving it
//! - **Circuit Breaker**: SmartAiRouter implements circuit breaker pattern to prevent
//!   hammering failing LLM services
//! - **Rate Limiting**: RateLimiter utility for protecting against runaway API costs
//! - **Agent Metrics**: AtomicAgentMetrics for observability and performance tracking
//! - **Lifecycle Hooks**: Agent trait includes initialize(), shutdown(), health_check()
//! - **Security**: Blocked patterns in GuardrailAgent are NEVER bypassed by gaming allowlist

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
pub use traits::{
    Agent, AgentContext, AgentError, AgentMetrics, AgentMode, AgentOutput, AgentPriority,
    AgentResult, AtomicAgentMetrics, NextAction, PlanningContext, RateLimiter,
    ReflectionFeedback, SearchStrategy, SubGoal,
};
