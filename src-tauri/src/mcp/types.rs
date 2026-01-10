//! MCP Core Types
//!
//! Defines the fundamental data structures for MCP communication.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Unique identifier for MCP entities
pub type McpId = String;

/// JSON Schema representation for tool parameters
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonSchema {
    #[serde(rename = "type")]
    pub schema_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub properties: Option<HashMap<String, PropertySchema>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub required: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

/// Schema for individual properties in a JSON Schema
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PropertySchema {
    #[serde(rename = "type")]
    pub prop_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(rename = "enum")]
    pub enum_values: Option<Vec<String>>,
}

/// Tool descriptor for capability discovery
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolDescriptor {
    /// Unique tool name (e.g., "browser.navigate", "browser.get_content")
    pub name: String,
    /// Human-readable description for LLM context
    pub description: String,
    /// JSON Schema for input parameters
    pub input_schema: JsonSchema,
    /// Whether tool execution has side effects
    pub is_side_effect: bool,
    /// Category for grouping related tools
    pub category: String,
}

/// Resource descriptor for data source discovery
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceDescriptor {
    /// Resource URI (e.g., "browser://current-page", "browser://history")
    pub uri: String,
    /// Human-readable name
    pub name: String,
    /// Description for LLM context
    pub description: String,
    /// MIME type of the resource content
    pub mime_type: String,
    /// Whether resource content can change
    pub is_dynamic: bool,
}

/// Prompt template descriptor
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PromptDescriptor {
    /// Unique prompt name
    pub name: String,
    /// Description of when to use this prompt
    pub description: String,
    /// Parameter placeholders in the template
    pub parameters: Vec<String>,
    /// The actual prompt template with {{placeholders}}
    pub template: String,
}

/// Request to invoke an MCP tool
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolRequest {
    /// Name of the tool to invoke
    pub tool_name: String,
    /// Arguments as JSON object
    pub arguments: serde_json::Value,
    /// Request correlation ID
    pub request_id: McpId,
}

/// Response from MCP tool execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolResponse {
    /// Request correlation ID
    pub request_id: McpId,
    /// Whether execution was successful
    pub success: bool,
    /// Result data (tool-specific)
    pub data: serde_json::Value,
    /// Error message if failed
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    /// Execution time in milliseconds
    pub execution_time_ms: u64,
}

/// Request to read an MCP resource
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceRequest {
    /// Resource URI
    pub uri: String,
    /// Request correlation ID
    pub request_id: McpId,
    /// Optional query parameters
    #[serde(skip_serializing_if = "Option::is_none")]
    pub query: Option<HashMap<String, String>>,
}

/// Response from MCP resource read
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceResponse {
    /// Request correlation ID
    pub request_id: McpId,
    /// Whether read was successful
    pub success: bool,
    /// Resource content
    pub content: serde_json::Value,
    /// Content MIME type
    pub mime_type: String,
    /// Error message if failed
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// Server manifest describing all available capabilities
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpManifest {
    /// Server name
    pub name: String,
    /// Server version
    pub version: String,
    /// Available tools
    pub tools: Vec<ToolDescriptor>,
    /// Available resources
    pub resources: Vec<ResourceDescriptor>,
    /// Available prompts
    pub prompts: Vec<PromptDescriptor>,
}

/// Connection state for MCP server
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum McpConnectionState {
    #[default]
    Disconnected,
    Connecting,
    Connected,
    Error,
}

/// Error types for MCP operations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum McpError {
    /// Tool not found in manifest
    ToolNotFound(String),
    /// Resource not found
    ResourceNotFound(String),
    /// Invalid arguments provided
    InvalidArguments(String),
    /// Execution failed
    ExecutionFailed(String),
    /// Connection error
    ConnectionError(String),
    /// Timeout waiting for response
    Timeout(String),
    /// Permission denied
    PermissionDenied(String),
}

impl std::fmt::Display for McpError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            McpError::ToolNotFound(name) => write!(f, "Tool not found: {}", name),
            McpError::ResourceNotFound(uri) => write!(f, "Resource not found: {}", uri),
            McpError::InvalidArguments(msg) => write!(f, "Invalid arguments: {}", msg),
            McpError::ExecutionFailed(msg) => write!(f, "Execution failed: {}", msg),
            McpError::ConnectionError(msg) => write!(f, "Connection error: {}", msg),
            McpError::Timeout(msg) => write!(f, "Timeout: {}", msg),
            McpError::PermissionDenied(msg) => write!(f, "Permission denied: {}", msg),
        }
    }
}

impl std::error::Error for McpError {}
