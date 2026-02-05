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
//! - **Sandbox**: Sandboxed file system and shell access with trust levels
//!
//! This enables:
//! - Dynamic capability discovery (agents can query what tools are available)
//! - Standardized request/response format
//! - Composability across different resource providers
//! - Safe system access with progressive trust

pub mod browser;
pub mod sandbox;
pub mod traits;
pub mod types;
pub mod visual_tools;
pub mod os_tools;

pub use browser::BrowserMcpServer;
pub use os_tools::OsToolProvider;
pub use sandbox::{
    get_sandbox_config, update_sandbox_config, categorize_command, is_command_blocked,
    SandboxConfig, SandboxError, ShellCategory, TrustLevel,
};
pub use traits::{McpPrompt, McpResource, McpServer, McpTool};
pub use types::*;
pub use visual_tools::{FindElementTool, ClickElementTool, FillFieldTool, GetPageElementsTool, VisualToolRegistry};
