//! MCP Trait Definitions
//!
//! Defines the core traits that MCP servers and clients must implement.

use super::types::*;
use async_trait::async_trait;

/// Trait for MCP Resources - data sources that agents can read from
#[async_trait]
pub trait McpResource: Send + Sync {
    /// Get the resource descriptor for discovery
    fn descriptor(&self) -> ResourceDescriptor;

    /// Read the resource content
    async fn read(
        &self,
        query: Option<std::collections::HashMap<String, String>>,
    ) -> Result<serde_json::Value, McpError>;

    /// Check if resource content has changed since last read
    fn has_changed(&self) -> bool {
        true // Default: assume always changed (conservative)
    }

    /// Subscribe to resource changes (returns stream of updates)
    /// Default implementation returns None (no subscription support)
    fn subscribe(&self) -> Option<tokio::sync::broadcast::Receiver<serde_json::Value>> {
        None
    }
}

/// Trait for MCP Tools - executable functions that perform actions
#[async_trait]
pub trait McpTool: Send + Sync {
    /// Get the tool descriptor for discovery
    fn descriptor(&self) -> ToolDescriptor;

    /// Execute the tool with given arguments
    async fn execute(&self, arguments: serde_json::Value) -> Result<serde_json::Value, McpError>;

    /// Validate arguments before execution (optional pre-check)
    fn validate_arguments(&self, arguments: &serde_json::Value) -> Result<(), McpError> {
        // Default: accept any arguments
        let _ = arguments;
        Ok(())
    }

    /// Check if tool can be executed (e.g., connection available)
    fn is_available(&self) -> bool {
        true
    }
}

/// Trait for MCP Prompts - templates that guide LLM interaction
pub trait McpPrompt: Send + Sync {
    /// Get the prompt descriptor for discovery
    fn descriptor(&self) -> PromptDescriptor;

    /// Render the prompt with given parameters
    fn render(
        &self,
        parameters: std::collections::HashMap<String, String>,
    ) -> Result<String, McpError>;
}

/// Trait for MCP Servers - aggregates resources, tools, and prompts
#[async_trait]
pub trait McpServer: Send + Sync {
    /// Get the server manifest (all available capabilities)
    fn manifest(&self) -> McpManifest;

    /// Get current connection state
    fn connection_state(&self) -> McpConnectionState;

    /// Discover available tools (optionally filtered by category)
    fn discover_tools(&self, category: Option<&str>) -> Vec<ToolDescriptor>;

    /// Discover available resources
    fn discover_resources(&self) -> Vec<ResourceDescriptor>;

    /// Discover available prompts
    fn discover_prompts(&self) -> Vec<PromptDescriptor>;

    /// Invoke a tool by name
    async fn invoke_tool(&self, request: ToolRequest) -> ToolResponse;

    /// Read a resource by URI
    async fn read_resource(&self, request: ResourceRequest) -> ResourceResponse;

    /// Render a prompt by name
    fn render_prompt(
        &self,
        name: &str,
        parameters: std::collections::HashMap<String, String>,
    ) -> Result<String, McpError>;
}
