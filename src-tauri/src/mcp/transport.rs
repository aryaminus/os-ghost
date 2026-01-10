//! MCP Transport Layer
//!
//! Defines the JSON-RPC 2.0 wire format and transport traits for MCP communication.
//! This enables the MCP server to communicate over Stdio, SSE, or HTTP.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use async_trait::async_trait;
use super::types::McpError;

/// JSON-RPC 2.0 Version Constant
pub const JSONRPC_VERSION: &str = "2.0";

/// A JSON-RPC 2.0 Request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcRequest {
    pub jsonrpc: String,
    pub method: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub params: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<Value>, // ID can be string, number, or null
}

impl JsonRpcRequest {
    /// Create a new request
    pub fn new(method: &str, params: Option<Value>, id: Option<Value>) -> Self {
        Self {
            jsonrpc: JSONRPC_VERSION.to_string(),
            method: method.to_string(),
            params,
            id,
        }
    }

    /// Create a notification (no ID)
    pub fn notification(method: &str, params: Option<Value>) -> Self {
        Self {
            jsonrpc: JSONRPC_VERSION.to_string(),
            method: method.to_string(),
            params,
            id: None,
        }
    }
}

/// A JSON-RPC 2.0 Response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcResponse {
    pub jsonrpc: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<JsonRpcError>,
    pub id: Option<Value>,
}

impl JsonRpcResponse {
    /// Create a success response
    pub fn success(id: Value, result: Value) -> Self {
        Self {
            jsonrpc: JSONRPC_VERSION.to_string(),
            result: Some(result),
            error: None,
            id: Some(id),
        }
    }

    /// Create an error response
    pub fn error(id: Option<Value>, code: i32, message: &str, data: Option<Value>) -> Self {
        Self {
            jsonrpc: JSONRPC_VERSION.to_string(),
            result: None,
            error: Some(JsonRpcError {
                code,
                message: message.to_string(),
                data,
            }),
            id,
        }
    }
}

/// A JSON-RPC 2.0 Error Object
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcError {
    pub code: i32,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<Value>,
}

/// Standard JSON-RPC Error Codes
pub mod error_codes {
    pub const PARSE_ERROR: i32 = -32700;
    pub const INVALID_REQUEST: i32 = -32600;
    pub const METHOD_NOT_FOUND: i32 = -32601;
    pub const INVALID_PARAMS: i32 = -32602;
    pub const INTERNAL_ERROR: i32 = -32603;
    pub const SERVER_ERROR_START: i32 = -32000;
    pub const SERVER_ERROR_END: i32 = -32099;
}

/// Trait for MCP Transport (Stdio, SSE, etc.)
#[async_trait]
pub trait McpTransport: Send + Sync {
    /// Send a JSON-RPC message
    async fn send(&self, message: JsonRpcRequest) -> Result<(), McpError>;

    /// Receive a JSON-RPC message (blocking/awaiting next message)
    async fn receive(&self) -> Result<Option<JsonRpcResponse>, McpError>;

    /// Check if transport is connected
    fn is_connected(&self) -> bool;
}

/// Convert McpError to JSON-RPC Error Code
impl From<&McpError> for i32 {
    fn from(err: &McpError) -> Self {
        match err {
            McpError::ToolNotFound(_) => error_codes::METHOD_NOT_FOUND,
            McpError::ResourceNotFound(_) => error_codes::INVALID_PARAMS,
            McpError::InvalidArguments(_) => error_codes::INVALID_PARAMS,
            McpError::ExecutionFailed(_) => error_codes::INTERNAL_ERROR,
            McpError::ConnectionError(_) => error_codes::INTERNAL_ERROR,
            McpError::Timeout(_) => error_codes::SERVER_ERROR_START, // Custom error range
            McpError::PermissionDenied(_) => -32001, // Access denied
        }
    }
}
