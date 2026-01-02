//! Multi-agent system for The OS Ghost
//! Implements ADK-style agent patterns in Rust

pub mod narrator;
pub mod observer;
pub mod orchestrator;
pub mod traits;
pub mod verifier;

pub use orchestrator::AgentOrchestrator;
pub use traits::{Agent, AgentContext, AgentResult};
