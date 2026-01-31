//! Browser MCP Server Implementation
//!
//! Implements the browser extension as an MCP-compatible server, exposing
//! browser capabilities as discoverable Resources and Tools.
//!
//! Resources:
//! - browser://current-page - Current page URL, title, and content
//! - browser://history - Recent browsing history
//! - browser://top-sites - Most visited sites
//!
//! Tools:
//! - browser.navigate - Navigate to a URL
//! - browser.get_content - Get page content
//! - browser.inject_effect - Apply visual effect
//! - browser.highlight_text - Highlight specific text

use super::traits::*;
use super::types::*;
use crate::action_preview::{VisualPreview, VisualPreviewType};
use crate::actions::{ActionRiskLevel, PendingAction, ACTION_QUEUE};
use crate::privacy::PrivacySettings;
use async_trait::async_trait;
use serde_json::json;
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::sync::{broadcast, RwLock};

/// Current page state cached from browser extension
#[derive(Debug, Clone, Default)]
pub struct PageState {
    pub url: String,
    pub title: String,
    pub body_text: String,
    pub timestamp: i64,
}

/// Browsing context from extension
#[derive(Debug, Clone, Default)]
pub struct BrowsingContext {
    pub recent_history: Vec<serde_json::Value>,
    pub top_sites: Vec<serde_json::Value>,
    pub last_updated: i64,
}

/// Shared browser state managed by the MCP server
#[derive(Debug)]
pub struct BrowserState {
    pub current_page: RwLock<PageState>,
    pub context: RwLock<BrowsingContext>,
    pub is_connected: AtomicBool,
    /// Channel for broadcasting page changes to subscribers
    page_updates: broadcast::Sender<serde_json::Value>,
}

impl Default for BrowserState {
    fn default() -> Self {
        let (tx, _) = broadcast::channel(16);
        Self {
            current_page: RwLock::new(PageState::default()),
            context: RwLock::new(BrowsingContext::default()),
            is_connected: AtomicBool::new(false),
            page_updates: tx,
        }
    }
}

impl BrowserState {
    pub fn new() -> Self {
        Self::default()
    }

    /// Update current page state (called when extension sends page_load/page_content)
    pub async fn update_page(&self, url: String, title: String, body_text: String, timestamp: i64) {
        {
            let mut page = self.current_page.write().await;
            page.url = url.clone();
            page.title = title.clone();
            page.body_text = body_text.clone();
            page.timestamp = timestamp;
        }

        if let Some(rollback) = crate::rollback::get_rollback_manager() {
            let title_opt = if title.is_empty() { None } else { Some(title.as_str()) };
            rollback.update_page_state(&url, title_opt);
        }

        // Notify subscribers
        let _ = self.page_updates.send(json!({
            "url": url,
            "title": title,
            "timestamp": timestamp
        }));
    }

    /// Update browsing context (called when extension sends browsing_context)
    pub async fn update_context(
        &self,
        history: Vec<serde_json::Value>,
        top_sites: Vec<serde_json::Value>,
    ) {
        let mut ctx = self.context.write().await;
        ctx.recent_history = history;
        ctx.top_sites = top_sites;
        ctx.last_updated = chrono::Utc::now().timestamp();
    }

    /// Subscribe to page updates
    pub fn subscribe_page_updates(&self) -> broadcast::Receiver<serde_json::Value> {
        self.page_updates.subscribe()
    }
}

// ============================================================================
// Browser Resources
// ============================================================================

/// Resource: Current page content
pub struct CurrentPageResource {
    state: Arc<BrowserState>,
}

impl CurrentPageResource {
    pub fn new(state: Arc<BrowserState>) -> Self {
        Self { state }
    }
}

#[async_trait]
impl McpResource for CurrentPageResource {
    fn descriptor(&self) -> ResourceDescriptor {
        ResourceDescriptor {
            uri: "browser://current-page".to_string(),
            name: "Current Page".to_string(),
            description: "The currently active browser tab's URL, title, and text content"
                .to_string(),
            mime_type: "application/json".to_string(),
            is_dynamic: true,
        }
    }

    async fn read(
        &self,
        _query: Option<HashMap<String, String>>,
    ) -> Result<serde_json::Value, McpError> {
        let page = self.state.current_page.read().await;
        Ok(json!({
            "url": page.url,
            "title": page.title,
            "body_text": page.body_text,
            "timestamp": page.timestamp
        }))
    }

    fn subscribe(&self) -> Option<broadcast::Receiver<serde_json::Value>> {
        Some(self.state.subscribe_page_updates())
    }
}

/// Resource: Browsing history
pub struct HistoryResource {
    state: Arc<BrowserState>,
}

impl HistoryResource {
    pub fn new(state: Arc<BrowserState>) -> Self {
        Self { state }
    }
}

#[async_trait]
impl McpResource for HistoryResource {
    fn descriptor(&self) -> ResourceDescriptor {
        ResourceDescriptor {
            uri: "browser://history".to_string(),
            name: "Browsing History".to_string(),
            description: "Recent browsing history (last 7 days, up to 50 items)".to_string(),
            mime_type: "application/json".to_string(),
            is_dynamic: true,
        }
    }

    async fn read(
        &self,
        query: Option<HashMap<String, String>>,
    ) -> Result<serde_json::Value, McpError> {
        let ctx = self.state.context.read().await;
        let limit = query
            .and_then(|q| q.get("limit").and_then(|l| l.parse().ok()))
            .unwrap_or(50);

        let history: Vec<_> = ctx.recent_history.iter().take(limit).cloned().collect();
        Ok(json!({
            "items": history,
            "count": history.len(),
            "last_updated": ctx.last_updated
        }))
    }
}

/// Resource: Top sites
pub struct TopSitesResource {
    state: Arc<BrowserState>,
}

impl TopSitesResource {
    pub fn new(state: Arc<BrowserState>) -> Self {
        Self { state }
    }
}

#[async_trait]
impl McpResource for TopSitesResource {
    fn descriptor(&self) -> ResourceDescriptor {
        ResourceDescriptor {
            uri: "browser://top-sites".to_string(),
            name: "Top Sites".to_string(),
            description: "Most frequently visited sites (top 10)".to_string(),
            mime_type: "application/json".to_string(),
            is_dynamic: true,
        }
    }

    async fn read(
        &self,
        _query: Option<HashMap<String, String>>,
    ) -> Result<serde_json::Value, McpError> {
        let ctx = self.state.context.read().await;
        Ok(json!({
            "sites": ctx.top_sites,
            "count": ctx.top_sites.len()
        }))
    }
}

// ============================================================================
// Browser Tools
// ============================================================================

/// Channel for sending commands back to the extension
pub type EffectSender = tokio::sync::mpsc::Sender<serde_json::Value>;

/// Tool: Navigate to URL
pub struct NavigateTool {
    effect_sender: EffectSender,
}

impl NavigateTool {
    pub fn new(sender: EffectSender) -> Self {
        Self {
            effect_sender: sender,
        }
    }
}

#[async_trait]
impl McpTool for NavigateTool {
    fn descriptor(&self) -> ToolDescriptor {
        let mut props = HashMap::new();
        props.insert(
            "url".to_string(),
            PropertySchema {
                prop_type: "string".to_string(),
                description: Some("The URL to navigate to".to_string()),
                default: None,
                enum_values: None,
            },
        );

        ToolDescriptor {
            name: "browser.navigate".to_string(),
            description: "Navigate the active browser tab to a specific URL".to_string(),
            input_schema: JsonSchema {
                schema_type: "object".to_string(),
                properties: Some(props),
                required: Some(vec!["url".to_string()]),
                description: None,
            },
            is_side_effect: true,
            category: "navigation".to_string(),
        }
    }

    fn validate_arguments(&self, arguments: &serde_json::Value) -> Result<(), McpError> {
        let url = arguments
            .get("url")
            .and_then(|v| v.as_str())
            .ok_or_else(|| McpError::InvalidArguments("Missing 'url' parameter".to_string()))?;

        if !url.starts_with("http://") && !url.starts_with("https://") {
            return Err(McpError::InvalidArguments(
                "URL must start with http:// or https://".to_string(),
            ));
        }

        Ok(())
    }

    async fn execute(&self, arguments: serde_json::Value) -> Result<serde_json::Value, McpError> {
        self.validate_arguments(&arguments)?;

        let url = arguments
            .get("url")
            .and_then(|v| v.as_str())
            .ok_or_else(|| McpError::InvalidArguments("Missing 'url' parameter".to_string()))?;

        self.effect_sender
            .send(json!({
                "action": "navigate",
                "url": url
            }))
            .await
            .map_err(|e| McpError::ExecutionFailed(format!("Failed to send command: {}", e)))?;

        Ok(json!({
            "navigated_to": url
        }))
    }
}

/// Tool: Inject visual effect
pub struct InjectEffectTool {
    effect_sender: EffectSender,
}

impl InjectEffectTool {
    pub fn new(sender: EffectSender) -> Self {
        Self {
            effect_sender: sender,
        }
    }
}

#[async_trait]
impl McpTool for InjectEffectTool {
    fn descriptor(&self) -> ToolDescriptor {
        let mut props = HashMap::new();
        props.insert(
            "effect".to_string(),
            PropertySchema {
                prop_type: "string".to_string(),
                description: Some("The visual effect to apply".to_string()),
                default: None,
                enum_values: Some(vec![
                    "glitch".to_string(),
                    "scanlines".to_string(),
                    "static".to_string(),
                    "flicker".to_string(),
                    "vignette".to_string(),
                ]),
            },
        );
        props.insert(
            "duration".to_string(),
            PropertySchema {
                prop_type: "integer".to_string(),
                description: Some("Effect duration in milliseconds".to_string()),
                default: Some(json!(1000)),
                enum_values: None,
            },
        );

        ToolDescriptor {
            name: "browser.inject_effect".to_string(),
            description: "Apply a visual glitch effect to the current page".to_string(),
            input_schema: JsonSchema {
                schema_type: "object".to_string(),
                properties: Some(props),
                required: Some(vec!["effect".to_string()]),
                description: None,
            },
            is_side_effect: true,
            category: "effects".to_string(),
        }
    }

    async fn execute(&self, arguments: serde_json::Value) -> Result<serde_json::Value, McpError> {
        let effect = arguments
            .get("effect")
            .and_then(|v| v.as_str())
            .ok_or_else(|| McpError::InvalidArguments("Missing 'effect' parameter".to_string()))?;

        let duration = arguments
            .get("duration")
            .and_then(|v| v.as_i64())
            .unwrap_or(1000);

        self.effect_sender
            .send(json!({
                "action": "inject_effect",
                "effect": effect,
                "duration": duration
            }))
            .await
            .map_err(|e| McpError::ExecutionFailed(format!("Failed to send command: {}", e)))?;

        Ok(json!({
            "effect_applied": effect,
            "duration_ms": duration
        }))
    }
}

/// Tool: Highlight text on page
pub struct HighlightTextTool {
    effect_sender: EffectSender,
}

impl HighlightTextTool {
    pub fn new(sender: EffectSender) -> Self {
        Self {
            effect_sender: sender,
        }
    }
}

#[async_trait]
impl McpTool for HighlightTextTool {
    fn descriptor(&self) -> ToolDescriptor {
        let mut props = HashMap::new();
        props.insert(
            "text".to_string(),
            PropertySchema {
                prop_type: "string".to_string(),
                description: Some("The text to highlight on the page".to_string()),
                default: None,
                enum_values: None,
            },
        );

        ToolDescriptor {
            name: "browser.highlight_text".to_string(),
            description: "Highlight specific text on the current page to draw user attention"
                .to_string(),
            input_schema: JsonSchema {
                schema_type: "object".to_string(),
                properties: Some(props),
                required: Some(vec!["text".to_string()]),
                description: None,
            },
            is_side_effect: true,
            category: "effects".to_string(),
        }
    }

    async fn execute(&self, arguments: serde_json::Value) -> Result<serde_json::Value, McpError> {
        let text = arguments
            .get("text")
            .and_then(|v| v.as_str())
            .ok_or_else(|| McpError::InvalidArguments("Missing 'text' parameter".to_string()))?;

        self.effect_sender
            .send(json!({
                "action": "highlight_text",
                "text": text
            }))
            .await
            .map_err(|e| McpError::ExecutionFailed(format!("Failed to send command: {}", e)))?;

        Ok(json!({
            "highlighted": text
        }))
    }
}

/// Tool: Get page content (request fresh content from extension)
pub struct GetContentTool {
    effect_sender: EffectSender,
    state: Arc<BrowserState>,
}

impl GetContentTool {
    pub fn new(sender: EffectSender, state: Arc<BrowserState>) -> Self {
        Self {
            effect_sender: sender,
            state,
        }
    }
}

#[async_trait]
impl McpTool for GetContentTool {
    fn descriptor(&self) -> ToolDescriptor {
        ToolDescriptor {
            name: "browser.get_content".to_string(),
            description: "Request fresh page content from the browser extension".to_string(),
            input_schema: JsonSchema {
                schema_type: "object".to_string(),
                properties: None,
                required: None,
                description: Some("No parameters required".to_string()),
            },
            is_side_effect: false,
            category: "content".to_string(),
        }
    }

    async fn execute(&self, _arguments: serde_json::Value) -> Result<serde_json::Value, McpError> {
        // Request fresh content from extension
        self.effect_sender
            .send(json!({
                "action": "get_page_content"
            }))
            .await
            .map_err(|e| McpError::ExecutionFailed(format!("Failed to send command: {}", e)))?;

        // Return current cached content (extension will update async)
        let page = self.state.current_page.read().await;
        Ok(json!({
            "url": page.url,
            "title": page.title,
            "body_text": page.body_text,
            "timestamp": page.timestamp,
            "note": "Fresh content requested; cached content returned"
        }))
    }
}

// ============================================================================
// Browser Prompts
// ============================================================================

/// Prompt: Analyze page for puzzle clues
pub struct AnalyzePagePrompt;

impl McpPrompt for AnalyzePagePrompt {
    fn descriptor(&self) -> PromptDescriptor {
        PromptDescriptor {
            name: "browser.analyze_page".to_string(),
            description: "Analyze the current page content for puzzle-relevant clues".to_string(),
            parameters: vec![
                "puzzle_context".to_string(),
                "keywords".to_string(),
                "page_content".to_string(),
            ],
            template: r#"
Analyze the following page content in the context of the current puzzle.

PUZZLE CONTEXT:
{{puzzle_context}}

KEYWORDS TO LOOK FOR:
{{keywords}}

PAGE CONTENT:
{{page_content}}

Extract any relevant clues, patterns, or information that could help solve the puzzle.
Focus on:
1. Direct keyword matches
2. Semantic relationships to the puzzle theme
3. Hidden patterns or unusual elements
4. Links or references that might lead to solutions
"#
            .to_string(),
        }
    }

    fn render(&self, parameters: HashMap<String, String>) -> Result<String, McpError> {
        let mut result = self.descriptor().template;
        for (key, value) in parameters {
            result = result.replace(&format!("{{{{{}}}}}", key), &value);
        }
        Ok(result)
    }
}

// ============================================================================
// Browser MCP Server
// ============================================================================

/// Complete Browser MCP Server implementation
pub struct BrowserMcpServer {
    state: Arc<BrowserState>,
    resources: Vec<Box<dyn McpResource>>,
    tools: Vec<Box<dyn McpTool>>,
    prompts: Vec<Box<dyn McpPrompt>>,
}

impl BrowserMcpServer {
    /// Create a new browser MCP server
    pub fn new(state: Arc<BrowserState>, effect_sender: EffectSender) -> Self {
        // Initialize resources
        let resources: Vec<Box<dyn McpResource>> = vec![
            Box::new(CurrentPageResource::new(state.clone())),
            Box::new(HistoryResource::new(state.clone())),
            Box::new(TopSitesResource::new(state.clone())),
        ];

        // Initialize tools
        let tools: Vec<Box<dyn McpTool>> = vec![
            Box::new(NavigateTool::new(effect_sender.clone())),
            Box::new(InjectEffectTool::new(effect_sender.clone())),
            Box::new(HighlightTextTool::new(effect_sender.clone())),
            Box::new(GetContentTool::new(effect_sender.clone(), state.clone())),
        ];

        // Initialize prompts
        let prompts: Vec<Box<dyn McpPrompt>> = vec![Box::new(AnalyzePagePrompt)];

        Self {
            state,
            resources,
            tools,
            prompts,
        }
    }

    /// Get the shared browser state
    pub fn state(&self) -> Arc<BrowserState> {
        self.state.clone()
    }

    /// Find a tool by name
    fn find_tool(&self, name: &str) -> Option<&dyn McpTool> {
        self.tools
            .iter()
            .find(|t| t.descriptor().name == name)
            .map(|b| b.as_ref())
    }

    /// Find a resource by URI
    fn find_resource(&self, uri: &str) -> Option<&dyn McpResource> {
        self.resources
            .iter()
            .find(|r| r.descriptor().uri == uri)
            .map(|b| b.as_ref())
    }

    /// Find a prompt by name
    fn find_prompt(&self, name: &str) -> Option<&dyn McpPrompt> {
        self.prompts
            .iter()
            .find(|p| p.descriptor().name == name)
            .map(|b| b.as_ref())
    }
}

#[async_trait]
impl McpServer for BrowserMcpServer {
    fn manifest(&self) -> McpManifest {
        McpManifest {
            name: "OS Ghost Browser Server".to_string(),
            version: "1.0.0".to_string(),
            tools: self.tools.iter().map(|t| t.descriptor()).collect(),
            resources: self.resources.iter().map(|r| r.descriptor()).collect(),
            prompts: self.prompts.iter().map(|p| p.descriptor()).collect(),
        }
    }

    fn connection_state(&self) -> McpConnectionState {
        if self.state.is_connected.load(Ordering::SeqCst) {
            McpConnectionState::Connected
        } else {
            McpConnectionState::Disconnected
        }
    }

    fn discover_tools(&self, category: Option<&str>) -> Vec<ToolDescriptor> {
        self.tools
            .iter()
            .map(|t| t.descriptor())
            .filter(|d| category.is_none_or(|c| d.category == c))
            .collect()
    }

    fn discover_resources(&self) -> Vec<ResourceDescriptor> {
        self.resources.iter().map(|r| r.descriptor()).collect()
    }

    fn discover_prompts(&self) -> Vec<PromptDescriptor> {
        self.prompts.iter().map(|p| p.descriptor()).collect()
    }

    async fn invoke_tool(&self, request: ToolRequest) -> ToolResponse {
        let start = std::time::Instant::now();
        let arguments = request.arguments.clone();

        match self.find_tool(&request.tool_name) {
            Some(tool) => {
                let descriptor = tool.descriptor();
                
                // Check privacy and autonomy settings for side-effect tools
                if descriptor.is_side_effect {
                    let privacy = PrivacySettings::load();
                    
                    // Read-only mode blocks all side effects
                    if privacy.read_only_mode {
                        return ToolResponse {
                            request_id: request.request_id,
                            success: false,
                            data: json!(null),
                            error: Some(
                                "Read-only mode enabled; side-effect tools are disabled"
                                    .to_string(),
                            ),
                            execution_time_ms: start.elapsed().as_millis() as u64,
                        };
                    }
                    
                    // Check autonomy level
                    if !privacy.autonomy_level.allows_actions() {
                        return ToolResponse {
                            request_id: request.request_id,
                            success: false,
                            data: json!(null),
                            error: Some(
                                "Observer mode: actions are disabled. Change autonomy level to enable."
                                    .to_string(),
                            ),
                            execution_time_ms: start.elapsed().as_millis() as u64,
                        };
                    }
                    
                    // Determine risk level based on action type
                    let risk_level = match descriptor.name.as_str() {
                        "browser.navigate" => ActionRiskLevel::High,
                        "browser.inject_effect" | "browser.highlight_text" => ActionRiskLevel::Low,
                        _ => ActionRiskLevel::Medium,
                    };

                    // Preview policy control
                    let preview_policy = privacy.preview_policy;
                    let should_preview = match preview_policy {
                        crate::privacy::PreviewPolicy::Always => true,
                        crate::privacy::PreviewPolicy::HighRisk => risk_level.is_high_risk(),
                        crate::privacy::PreviewPolicy::Off => false,
                    };

                    // Check if confirmation is required
                    if privacy.autonomy_level.requires_confirmation(risk_level.is_high_risk())
                        || matches!(preview_policy, crate::privacy::PreviewPolicy::Always)
                    {
                        // Get target description for the action
                        let target = match descriptor.name.as_str() {
                            "browser.navigate" => request.arguments
                                .get("url")
                                .and_then(|v| v.as_str())
                                .unwrap_or("unknown URL")
                                .to_string(),
                            "browser.inject_effect" => request.arguments
                                .get("effect")
                                .and_then(|v| v.as_str())
                                .unwrap_or("effect")
                                .to_string(),
                            "browser.highlight_text" => request.arguments
                                .get("text")
                                .and_then(|v| v.as_str())
                                .unwrap_or("text")
                                .to_string(),
                            _ => "action".to_string(),
                        };
                        
                        let description = match descriptor.name.as_str() {
                            "browser.navigate" => format!("Navigate browser to: {}", target),
                            "browser.inject_effect" => format!("Apply visual effect: {}", target),
                            "browser.highlight_text" => format!("Highlight text: {}", target),
                            _ => format!("Execute: {}", descriptor.name),
                        };
                        
                        // Create pending action for user confirmation
                        let preview_target = target.clone();
                        let pending = PendingAction::new(
                            descriptor.name.clone(),
                            description,
                            target.clone(),
                            risk_level,
                            None, // reason could come from agent context
                            Some(request.arguments.clone()),
                        );

                        // Create an action preview for richer UX (if manager available)
                        let preview_id = if should_preview {
                            if let Some(manager) = crate::action_preview::get_preview_manager_mut() {
                                let preview = manager.start_preview(&pending);

                            match descriptor.name.as_str() {
                                "browser.navigate" => {
                                    manager.set_visual_preview(
                                        &preview.id,
                                        VisualPreview {
                                            preview_type: VisualPreviewType::UrlCard,
                                            content: preview_target.clone(),
                                            width: None,
                                            height: None,
                                            alt_text: format!("Navigate to {}", preview_target),
                                        },
                                    );
                                }
                                "browser.highlight_text" => {
                                    manager.set_visual_preview(
                                        &preview.id,
                                        VisualPreview {
                                            preview_type: VisualPreviewType::TextSelection,
                                            content: preview_target.clone(),
                                            width: None,
                                            height: None,
                                            alt_text: format!("Highlight '{}'", preview_target),
                                        },
                                    );
                                }
                                _ => {}
                            }

                            manager.update_progress(&preview.id, 1.0);
                                Some(preview.id)
                            } else {
                                None
                            }
                        } else {
                            None
                        };

                        let action_id = ACTION_QUEUE.add(pending);
                        
                        return ToolResponse {
                            request_id: request.request_id,
                            success: false,
                            data: json!({
                                "status": "pending_confirmation",
                                "action_id": action_id,
                                "preview_id": preview_id,
                                "message": "Action requires user confirmation"
                            }),
                            error: None,
                            execution_time_ms: start.elapsed().as_millis() as u64,
                        };
                    }
                }

                match tool.execute(arguments.clone()).await {
                    Ok(data) => {
                        if descriptor.is_side_effect {
                            if let Some(rollback) = crate::rollback::get_rollback_manager() {
                                match descriptor.name.as_str() {
                                    "browser.navigate" => {
                                        if let Some(url) = arguments.get("url").and_then(|v| v.as_str()) {
                                            rollback.record_navigation(&request.request_id, url, None);
                                        }
                                    }
                                    "browser.inject_effect" => {
                                        if let Some(effect) = arguments.get("effect").and_then(|v| v.as_str()) {
                                            let duration = arguments
                                                .get("duration")
                                                .and_then(|v| v.as_u64())
                                                .unwrap_or(1000);
                                            rollback.record_effect(&request.request_id, effect, duration);
                                        }
                                    }
                                    "browser.highlight_text" => {
                                        if let Some(text) = arguments.get("text").and_then(|v| v.as_str()) {
                                            rollback.record_highlight(&request.request_id, text);
                                        }
                                    }
                                    _ => {}
                                }
                            }
                        }

                        ToolResponse {
                            request_id: request.request_id,
                            success: true,
                            data,
                            error: None,
                            execution_time_ms: start.elapsed().as_millis() as u64,
                        }
                    }
                    Err(e) => ToolResponse {
                        request_id: request.request_id,
                        success: false,
                        data: json!(null),
                        error: Some(e.to_string()),
                        execution_time_ms: start.elapsed().as_millis() as u64,
                    },
                }
            }
            None => ToolResponse {
                request_id: request.request_id,
                success: false,
                data: json!(null),
                error: Some(format!("Tool not found: {}", request.tool_name)),
                execution_time_ms: start.elapsed().as_millis() as u64,
            },
        }
    }

    async fn read_resource(&self, request: ResourceRequest) -> ResourceResponse {
        match self.find_resource(&request.uri) {
            Some(resource) => match resource.read(request.query).await {
                Ok(content) => ResourceResponse {
                    request_id: request.request_id,
                    success: true,
                    content,
                    mime_type: resource.descriptor().mime_type,
                    error: None,
                },
                Err(e) => ResourceResponse {
                    request_id: request.request_id,
                    success: false,
                    content: json!(null),
                    mime_type: "".to_string(),
                    error: Some(e.to_string()),
                },
            },
            None => ResourceResponse {
                request_id: request.request_id,
                success: false,
                content: json!(null),
                mime_type: "".to_string(),
                error: Some(format!("Resource not found: {}", request.uri)),
            },
        }
    }

    fn render_prompt(
        &self,
        name: &str,
        parameters: HashMap<String, String>,
    ) -> Result<String, McpError> {
        self.find_prompt(name)
            .ok_or_else(|| McpError::ResourceNotFound(format!("Prompt not found: {}", name)))?
            .render(parameters)
    }
}
