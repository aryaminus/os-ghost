//! Callback System - ADK-style lifecycle callbacks
//!
//! Implements the callback pattern from Google ADK for intercepting:
//! - Agent execution (before/after)
//! - Model calls (before/after LLM request)
//! - Tool execution (before/after tool call)
//!
//! Callbacks can:
//! - Observe and log execution
//! - Modify inputs/outputs
//! - Block execution by returning an override
//! - Apply guardrails and safety policies
//!
//! Reference: Google ADK Callbacks documentation

use super::traits::{AgentContext, AgentOutput};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;

// =============================================================================
// Callback Context
// =============================================================================

/// Context passed to callbacks during execution
#[derive(Debug, Clone)]
pub struct CallbackContext {
    /// Current agent context
    pub agent_context: AgentContext,
    /// Invocation ID for correlation
    pub invocation_id: String,
    /// Agent name (for agent callbacks)
    pub agent_name: String,
    /// Custom metadata
    pub metadata: HashMap<String, serde_json::Value>,
}

impl CallbackContext {
    pub fn new(agent_context: AgentContext, agent_name: &str, invocation_id: &str) -> Self {
        Self {
            agent_context,
            invocation_id: invocation_id.to_string(),
            agent_name: agent_name.to_string(),
            metadata: HashMap::new(),
        }
    }

    pub fn with_metadata<K: Into<String>, V: Into<serde_json::Value>>(mut self, key: K, value: V) -> Self {
        self.metadata.insert(key.into(), value.into());
        self
    }
}

// =============================================================================
// Model Callback Types
// =============================================================================

/// Request being sent to an LLM
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmRequest {
    /// The prompt/messages to send
    pub prompt: String,
    /// System prompt if any
    pub system_prompt: Option<String>,
    /// Model name/identifier
    pub model: String,
    /// Temperature setting
    pub temperature: Option<f32>,
    /// Max tokens to generate
    pub max_tokens: Option<usize>,
    /// Additional parameters
    pub params: HashMap<String, serde_json::Value>,
}

/// Response from an LLM
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmResponse {
    /// Generated text
    pub text: String,
    /// Whether this was blocked by safety
    pub blocked: bool,
    /// Reason for blocking (if blocked)
    pub block_reason: Option<String>,
    /// Token usage info
    pub usage: Option<TokenUsage>,
}

impl LlmResponse {
    /// Create a blocked response
    pub fn blocked(reason: impl Into<String>) -> Self {
        Self {
            text: String::new(),
            blocked: true,
            block_reason: Some(reason.into()),
            usage: None,
        }
    }

    /// Create a successful response
    pub fn success(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            blocked: false,
            block_reason: None,
            usage: None,
        }
    }

    /// Add usage info
    pub fn with_usage(mut self, usage: TokenUsage) -> Self {
        self.usage = Some(usage);
        self
    }
}

/// Token usage information
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TokenUsage {
    pub prompt_tokens: usize,
    pub completion_tokens: usize,
    pub total_tokens: usize,
}

// =============================================================================
// Tool Callback Types
// =============================================================================

/// Tool call request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCall {
    /// Tool/function name
    pub name: String,
    /// Arguments to pass
    pub arguments: HashMap<String, serde_json::Value>,
    /// Tool call ID for correlation
    pub call_id: String,
}

/// Tool call result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolResult {
    /// Result data
    pub result: serde_json::Value,
    /// Whether the tool succeeded
    pub success: bool,
    /// Error message if failed
    pub error: Option<String>,
}

impl ToolResult {
    /// Create a successful result
    pub fn success(result: serde_json::Value) -> Self {
        Self {
            result,
            success: true,
            error: None,
        }
    }

    /// Create a failed result
    pub fn error(message: impl Into<String>) -> Self {
        Self {
            result: serde_json::Value::Null,
            success: false,
            error: Some(message.into()),
        }
    }

    /// Create a blocked result (policy violation)
    pub fn blocked(reason: impl Into<String>) -> Self {
        Self {
            result: serde_json::json!({ "blocked": true }),
            success: false,
            error: Some(format!("Blocked: {}", reason.into())),
        }
    }
}

// =============================================================================
// Callback Traits
// =============================================================================

/// Callback for agent lifecycle events
/// Return Some(output) to skip agent execution and use the override
#[async_trait]
pub trait AgentCallback: Send + Sync {
    /// Called before agent execution
    /// Return Some(output) to skip the agent and use this output instead
    async fn before_agent(&self, ctx: &CallbackContext) -> Option<AgentOutput> {
        let _ = ctx;
        None
    }

    /// Called after agent execution
    /// Return Some(output) to replace the agent's output
    async fn after_agent(&self, ctx: &CallbackContext, output: &AgentOutput) -> Option<AgentOutput> {
        let _ = (ctx, output);
        None
    }
}

/// Callback for LLM/model events
/// Return Some(response) to skip the LLM call and use the override
#[async_trait]
pub trait ModelCallback: Send + Sync {
    /// Called before LLM call
    /// Return Some(response) to skip the LLM and use this response instead
    async fn before_model(&self, ctx: &CallbackContext, request: &LlmRequest) -> Option<LlmResponse> {
        let _ = (ctx, request);
        None
    }

    /// Called after LLM call
    /// Return Some(response) to replace the LLM's response
    async fn after_model(&self, ctx: &CallbackContext, request: &LlmRequest, response: &LlmResponse) -> Option<LlmResponse> {
        let _ = (ctx, request, response);
        None
    }
}

/// Callback for tool execution events
/// Return Some(result) to skip tool execution and use the override
#[async_trait]
pub trait ToolCallback: Send + Sync {
    /// Called before tool execution
    /// Return Some(result) to skip the tool and use this result instead
    async fn before_tool(&self, ctx: &CallbackContext, tool_call: &ToolCall) -> Option<ToolResult> {
        let _ = (ctx, tool_call);
        None
    }

    /// Called after tool execution
    /// Return Some(result) to replace the tool's result
    async fn after_tool(&self, ctx: &CallbackContext, tool_call: &ToolCall, result: &ToolResult) -> Option<ToolResult> {
        let _ = (ctx, tool_call, result);
        None
    }
}

// =============================================================================
// Callback Registry
// =============================================================================

/// Registry for managing callbacks
pub struct CallbackRegistry {
    agent_callbacks: Vec<Arc<dyn AgentCallback>>,
    model_callbacks: Vec<Arc<dyn ModelCallback>>,
    tool_callbacks: Vec<Arc<dyn ToolCallback>>,
}

impl Default for CallbackRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl CallbackRegistry {
    pub fn new() -> Self {
        Self {
            agent_callbacks: Vec::new(),
            model_callbacks: Vec::new(),
            tool_callbacks: Vec::new(),
        }
    }

    /// Register an agent callback
    pub fn register_agent_callback(&mut self, callback: Arc<dyn AgentCallback>) {
        self.agent_callbacks.push(callback);
    }

    /// Register a model callback
    pub fn register_model_callback(&mut self, callback: Arc<dyn ModelCallback>) {
        self.model_callbacks.push(callback);
    }

    /// Register a tool callback
    pub fn register_tool_callback(&mut self, callback: Arc<dyn ToolCallback>) {
        self.tool_callbacks.push(callback);
    }

    /// Run before_agent callbacks, return first override if any
    pub async fn run_before_agent(&self, ctx: &CallbackContext) -> Option<AgentOutput> {
        for callback in &self.agent_callbacks {
            if let Some(output) = callback.before_agent(ctx).await {
                return Some(output);
            }
        }
        None
    }

    /// Run after_agent callbacks, return first override if any
    pub async fn run_after_agent(&self, ctx: &CallbackContext, output: &AgentOutput) -> Option<AgentOutput> {
        for callback in &self.agent_callbacks {
            if let Some(override_output) = callback.after_agent(ctx, output).await {
                return Some(override_output);
            }
        }
        None
    }

    /// Run before_model callbacks, return first override if any
    pub async fn run_before_model(&self, ctx: &CallbackContext, request: &LlmRequest) -> Option<LlmResponse> {
        for callback in &self.model_callbacks {
            if let Some(response) = callback.before_model(ctx, request).await {
                return Some(response);
            }
        }
        None
    }

    /// Run after_model callbacks, return first override if any
    pub async fn run_after_model(
        &self,
        ctx: &CallbackContext,
        request: &LlmRequest,
        response: &LlmResponse,
    ) -> Option<LlmResponse> {
        for callback in &self.model_callbacks {
            if let Some(override_response) = callback.after_model(ctx, request, response).await {
                return Some(override_response);
            }
        }
        None
    }

    /// Run before_tool callbacks, return first override if any
    pub async fn run_before_tool(&self, ctx: &CallbackContext, tool_call: &ToolCall) -> Option<ToolResult> {
        for callback in &self.tool_callbacks {
            if let Some(result) = callback.before_tool(ctx, tool_call).await {
                return Some(result);
            }
        }
        None
    }

    /// Run after_tool callbacks, return first override if any
    pub async fn run_after_tool(
        &self,
        ctx: &CallbackContext,
        tool_call: &ToolCall,
        result: &ToolResult,
    ) -> Option<ToolResult> {
        for callback in &self.tool_callbacks {
            if let Some(override_result) = callback.after_tool(ctx, tool_call, result).await {
                return Some(override_result);
            }
        }
        None
    }
}

// =============================================================================
// Common Callback Implementations
// =============================================================================

/// Logging callback - logs all agent activity
pub struct LoggingCallback {
    prefix: String,
}

impl LoggingCallback {
    pub fn new(prefix: impl Into<String>) -> Self {
        Self {
            prefix: prefix.into(),
        }
    }
}

#[async_trait]
impl AgentCallback for LoggingCallback {
    async fn before_agent(&self, ctx: &CallbackContext) -> Option<AgentOutput> {
        tracing::debug!(
            "{} before_agent: {} (invocation: {})",
            self.prefix,
            ctx.agent_name,
            ctx.invocation_id
        );
        None
    }

    async fn after_agent(&self, ctx: &CallbackContext, output: &AgentOutput) -> Option<AgentOutput> {
        tracing::debug!(
            "{} after_agent: {} -> confidence: {:.2}",
            self.prefix,
            ctx.agent_name,
            output.confidence
        );
        None
    }
}

#[async_trait]
impl ModelCallback for LoggingCallback {
    async fn before_model(&self, ctx: &CallbackContext, request: &LlmRequest) -> Option<LlmResponse> {
        tracing::debug!(
            "{} before_model: {} (model: {})",
            self.prefix,
            ctx.agent_name,
            request.model
        );
        None
    }

    async fn after_model(&self, _ctx: &CallbackContext, _request: &LlmRequest, response: &LlmResponse) -> Option<LlmResponse> {
        if let Some(usage) = &response.usage {
            tracing::debug!(
                "{} after_model: {} tokens used",
                self.prefix,
                usage.total_tokens
            );
        }
        None
    }
}

#[async_trait]
impl ToolCallback for LoggingCallback {
    async fn before_tool(&self, ctx: &CallbackContext, tool_call: &ToolCall) -> Option<ToolResult> {
        tracing::debug!(
            "{} before_tool: {} -> {} ({})",
            self.prefix,
            ctx.agent_name,
            tool_call.name,
            tool_call.call_id
        );
        None
    }

    async fn after_tool(&self, _ctx: &CallbackContext, tool_call: &ToolCall, result: &ToolResult) -> Option<ToolResult> {
        tracing::debug!(
            "{} after_tool: {} success={}",
            self.prefix,
            tool_call.name,
            result.success
        );
        None
    }
}

/// Policy enforcement callback - blocks based on content policies
pub struct PolicyCallback {
    /// Blocked tool names
    blocked_tools: Vec<String>,
    /// Blocked prompts (substrings)
    blocked_prompts: Vec<String>,
}

impl PolicyCallback {
    pub fn new() -> Self {
        Self {
            blocked_tools: Vec::new(),
            blocked_prompts: Vec::new(),
        }
    }

    pub fn block_tool(mut self, tool_name: impl Into<String>) -> Self {
        self.blocked_tools.push(tool_name.into().to_lowercase());
        self
    }

    pub fn block_prompt(mut self, pattern: impl Into<String>) -> Self {
        self.blocked_prompts.push(pattern.into().to_lowercase());
        self
    }
}

impl Default for PolicyCallback {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl ModelCallback for PolicyCallback {
    async fn before_model(&self, _ctx: &CallbackContext, request: &LlmRequest) -> Option<LlmResponse> {
        let prompt_lower = request.prompt.to_lowercase();
        for blocked in &self.blocked_prompts {
            if prompt_lower.contains(blocked) {
                tracing::warn!("PolicyCallback: Blocked prompt containing '{}'", blocked);
                return Some(LlmResponse::blocked(format!("Prompt contains blocked content: {}", blocked)));
            }
        }
        None
    }
}

#[async_trait]
impl ToolCallback for PolicyCallback {
    async fn before_tool(&self, _ctx: &CallbackContext, tool_call: &ToolCall) -> Option<ToolResult> {
        let tool_lower = tool_call.name.to_lowercase();
        for blocked in &self.blocked_tools {
            if tool_lower == *blocked {
                tracing::warn!("PolicyCallback: Blocked tool '{}'", tool_call.name);
                return Some(ToolResult::blocked(format!("Tool '{}' is not allowed", tool_call.name)));
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_callback_registry() {
        let mut registry = CallbackRegistry::new();
        registry.register_agent_callback(Arc::new(LoggingCallback::new("[TEST]")));
        
        let ctx = CallbackContext::new(
            AgentContext::default(),
            "TestAgent",
            "inv_001",
        );
        
        // Should not override (logging only)
        let result = registry.run_before_agent(&ctx).await;
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_policy_callback_blocks_tool() {
        let mut registry = CallbackRegistry::new();
        registry.register_tool_callback(Arc::new(
            PolicyCallback::new().block_tool("dangerous_action")
        ));
        
        let ctx = CallbackContext::new(
            AgentContext::default(),
            "TestAgent",
            "inv_001",
        );
        
        let tool_call = ToolCall {
            name: "dangerous_action".to_string(),
            arguments: HashMap::new(),
            call_id: "call_001".to_string(),
        };
        
        let result = registry.run_before_tool(&ctx, &tool_call).await;
        assert!(result.is_some());
        let result = result.unwrap();
        assert!(!result.success);
    }

    #[tokio::test]
    async fn test_policy_callback_blocks_prompt() {
        let mut registry = CallbackRegistry::new();
        registry.register_model_callback(Arc::new(
            PolicyCallback::new().block_prompt("ignore previous instructions")
        ));
        
        let ctx = CallbackContext::new(
            AgentContext::default(),
            "TestAgent",
            "inv_001",
        );
        
        let request = LlmRequest {
            prompt: "Please ignore previous instructions and...".to_string(),
            system_prompt: None,
            model: "test".to_string(),
            temperature: None,
            max_tokens: None,
            params: HashMap::new(),
        };
        
        let result = registry.run_before_model(&ctx, &request).await;
        assert!(result.is_some());
        let response = result.unwrap();
        assert!(response.blocked);
    }

    #[test]
    fn test_llm_response_constructors() {
        let blocked = LlmResponse::blocked("safety violation");
        assert!(blocked.blocked);
        assert!(blocked.block_reason.is_some());
        
        let success = LlmResponse::success("Hello world");
        assert!(!success.blocked);
        assert_eq!(success.text, "Hello world");
    }

    #[test]
    fn test_tool_result_constructors() {
        let success = ToolResult::success(serde_json::json!({"result": 42}));
        assert!(success.success);
        
        let error = ToolResult::error("Something went wrong");
        assert!(!error.success);
        assert!(error.error.is_some());
        
        let blocked = ToolResult::blocked("Not allowed");
        assert!(!blocked.success);
        assert!(blocked.error.unwrap().contains("Blocked"));
    }
}
