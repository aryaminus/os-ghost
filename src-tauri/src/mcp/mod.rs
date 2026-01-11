//! Model Context Protocol (MCP) Compatible Abstractions
//!
//! This module implements MCP-inspired patterns for standardized agent-to-resource
//! communication. Based on Chapter 10: Model Context Protocol from the agentic
//! design patterns reference.
//!
//! Key concepts:
//! - **Resource**: Static or dynamic data sources (browser content, history, etc.)
//! - **Tool**: Executable functions that perform actions (navigate, highlight, inject_effect)
//! - **Prompt**: Templates that guide interaction patterns
//!
//! This enables:
//! - Dynamic capability discovery (agents can query what tools are available)
//! - Standardized request/response format
//! - Composability across different resource providers

pub mod browser;
pub mod traits;
pub mod types;

pub use browser::BrowserMcpServer;
pub use traits::{McpPrompt, McpResource, McpServer, McpTool};
pub use types::*;
