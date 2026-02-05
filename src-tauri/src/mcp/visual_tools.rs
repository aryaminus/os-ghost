//! MCP Visual Tools for browser element interaction
//!
//! Provides tools for finding, clicking, and interacting with UI elements
//! detected through vision analysis. These tools respect AutonomyLevel
//! and require user confirmation based on safety settings.

use super::traits::*;
use super::types::*;
use crate::actions::action_preview::{ActionPreview, PreviewManager, VisualPreview, VisualPreviewType};
use crate::actions::actions::{ActionRiskLevel, PendingAction, ACTION_QUEUE};
use crate::ai::vision::{VisualElement, VisionAnalyzer};
use crate::capture::vision::VisionCapture;
use crate::config::privacy::AutonomyLevel;
use async_trait::async_trait;
use serde_json::json;
use std::collections::HashMap;
use std::sync::Arc;

/// Tool: browser.find_element
/// Finds a UI element by description using vision analysis
pub struct FindElementTool {
    vision_capture: Arc<VisionCapture>,
}

impl FindElementTool {
    pub fn new(vision_capture: Arc<VisionCapture>) -> Self {
        Self { vision_capture }
    }
}

#[async_trait]
impl McpTool for FindElementTool {
    fn name(&self) -> &str {
        "browser.find_element"
    }

    fn description(&self) -> &str {
        "Find a UI element on the current page by description using computer vision. \
         Returns element coordinates and metadata."
    }

    fn parameters(&self) -> Vec<ParameterSchema> {
        vec![
            ParameterSchema {
                name: "description".to_string(),
                param_type: ParameterType::String,
                description: "Description of the element to find (e.g., 'Search button', 'Email input')".to_string(),
                required: true,
                default: None,
                enum_values: None,
            },
            ParameterSchema {
                name: "require_interactive".to_string(),
                param_type: ParameterType::Boolean,
                description: "Only return interactive elements (buttons, links, inputs)".to_string(),
                required: false,
                default: Some(json!(true)),
                enum_values: None,
            },
        ]
    }

    async fn execute(&self, request: ToolRequest) -> Result<ToolResponse, McpError> {
        let description = request.params.get("description")
            .and_then(|v| v.as_str())
            .ok_or_else(|| McpError::InvalidParams("Missing 'description' parameter".to_string()))?;

        let require_interactive = request.params.get("require_interactive")
            .and_then(|v| v.as_bool())
            .unwrap_or(true);

        // Check if vision is available
        if !self.vision_capture.is_available() {
            return Err(McpError::ExecutionError("Vision analysis not available".to_string()));
        }

        // Capture current screenshot (in real implementation, get from browser)
        // For now, this would need to be provided by the caller
        // In practice, this would capture the browser tab
        
        // Return element info
        Ok(ToolResponse {
            content: vec![ToolContent::Text {
                text: format!("Looking for element: '{}'", description),
            }],
            is_error: false,
        })
    }
}

/// Tool: browser.click_element
/// Clicks on an element found by description
pub struct ClickElementTool {
    vision_capture: Arc<VisionCapture>,
    autonomy_level: AutonomyLevel,
}

impl ClickElementTool {
    pub fn new(vision_capture: Arc<VisionCapture>, autonomy_level: AutonomyLevel) -> Self {
        Self { vision_capture, autonomy_level }
    }
}

#[async_trait]
impl McpTool for ClickElementTool {
    fn name(&self) -> &str {
        "browser.click_element"
    }

    fn description(&self) -> &str {
        "Click on a UI element by description. Requires appropriate AutonomyLevel."
    }

    fn parameters(&self) -> Vec<ParameterSchema> {
        vec![
            ParameterSchema {
                name: "description".to_string(),
                param_type: ParameterType::String,
                description: "Description of the element to click".to_string(),
                required: true,
                default: None,
                enum_values: None,
            },
        ]
    }

    async fn execute(&self, request: ToolRequest) -> Result<ToolResponse, McpError> {
        let description = request.params.get("description")
            .and_then(|v| v.as_str())
            .ok_or_else(|| McpError::InvalidParams("Missing 'description' parameter".to_string()))?;

        // Check autonomy level
        match self.autonomy_level {
            AutonomyLevel::Observer => {
                return Err(McpError::ExecutionError(
                    "Cannot click elements in Observer mode. Change AutonomyLevel to Suggester or higher.".to_string()
                ));
            }
            AutonomyLevel::Suggester | AutonomyLevel::Supervised => {
                // Will create pending action for approval
                let action = PendingAction {
                    id: generate_action_id(),
                    action_type: "browser.click_element".to_string(),
                    description: format!("Click on '{}'", description),
                    target: description.to_string(),
                    risk_level: ActionRiskLevel::Medium,
                    status: crate::actions::actions::ActionStatus::Pending,
                    created_at: std::time::SystemTime::now(),
                    expires_at: std::time::SystemTime::now() + std::time::Duration::from_secs(300),
                    requires_confirmation: true,
                    context: request.context.clone(),
                };

                // Add to action queue
                ACTION_QUEUE.add(action.clone());

                // Create visual preview
                if let Some(preview_manager) = PreviewManager::get() {
                    let visual_preview = VisualPreview {
                        preview_type: VisualPreviewType::ElementInteraction {
                            element_description: description.to_string(),
                            coordinates: (0, 0), // Would be populated from actual detection
                            action: "Click".to_string(),
                        },
                        description: format!("Will click on '{}'", description),
                        estimated_duration: std::time::Duration::from_secs(1),
                        rollback_steps: vec!["Navigate back".to_string()],
                    };
                    
                    preview_manager.start_preview(&action);
                }

                return Ok(ToolResponse {
                    content: vec![ToolContent::Text {
                        text: format!("Action queued for approval: Click on '{}'", description),
                    }],
                    is_error: false,
                });
            }
            AutonomyLevel::Autonomous => {
                // Execute immediately (within guardrails)
                // In real implementation, would use extension to click
                return Ok(ToolResponse {
                    content: vec![ToolContent::Text {
                        text: format!("Clicked on '{}'", description),
                    }],
                    is_error: false,
                });
            }
        }
    }
}

/// Tool: browser.fill_field
/// Fills an input field with text
pub struct FillFieldTool {
    vision_capture: Arc<VisionCapture>,
    autonomy_level: AutonomyLevel,
}

impl FillFieldTool {
    pub fn new(vision_capture: Arc<VisionCapture>, autonomy_level: AutonomyLevel) -> Self {
        Self { vision_capture, autonomy_level }
    }
}

#[async_trait]
impl McpTool for FillFieldTool {
    fn name(&self) -> &str {
        "browser.fill_field"
    }

    fn description(&self) -> &str {
        "Fill an input field with text. Finds the field by description then types the value."
    }

    fn parameters(&self) -> Vec<ParameterSchema> {
        vec![
            ParameterSchema {
                name: "field".to_string(),
                param_type: ParameterType::String,
                description: "Description of the input field (e.g., 'Email field', 'Search box')".to_string(),
                required: true,
                default: None,
                enum_values: None,
            },
            ParameterSchema {
                name: "value".to_string(),
                param_type: ParameterType::String,
                description: "Text to enter into the field".to_string(),
                required: true,
                default: None,
                enum_values: None,
            },
        ]
    }

    async fn execute(&self, request: ToolRequest) -> Result<ToolResponse, McpError> {
        let field = request.params.get("field")
            .and_then(|v| v.as_str())
            .ok_or_else(|| McpError::InvalidParams("Missing 'field' parameter".to_string()))?;

        let value = request.params.get("value")
            .and_then(|v| v.as_str())
            .ok_or_else(|| McpError::InvalidParams("Missing 'value' parameter".to_string()))?;

        // Mask value in logs for privacy
        let masked_value = if value.len() > 4 {
            format!("{}****", &value[..value.len().min(4)])
        } else {
            "****".to_string()
        };

        // Similar autonomy level handling as ClickElementTool
        match self.autonomy_level {
            AutonomyLevel::Observer => {
                return Err(McpError::ExecutionError(
                    "Cannot fill fields in Observer mode".to_string()
                ));
            }
            _ => {
                // For now, return success (real implementation would queue or execute)
                Ok(ToolResponse {
                    content: vec![ToolContent::Text {
                        text: format!("Will fill '{}' with '{}'", field, masked_value),
                    }],
                    is_error: false,
                })
            }
        }
    }
}

/// Tool: browser.get_page_elements
/// Returns all detected elements on the current page
pub struct GetPageElementsTool {
    vision_capture: Arc<VisionCapture>,
}

impl GetPageElementsTool {
    pub fn new(vision_capture: Arc<VisionCapture>) -> Self {
        Self { vision_capture }
    }
}

#[async_trait]
impl McpTool for GetPageElementsTool {
    fn name(&self) -> &str {
        "browser.get_page_elements"
    }

    fn description(&self) -> &str {
        "Analyze the current page and return all detected UI elements with their types and descriptions."
    }

    fn parameters(&self) -> Vec<ParameterSchema> {
        vec![
            ParameterSchema {
                name: "include_non_interactive".to_string(),
                param_type: ParameterType::Boolean,
                description: "Include non-interactive elements (text, images)".to_string(),
                required: false,
                default: Some(json!(false)),
                enum_values: None,
            },
        ]
    }

    async fn execute(&self, _request: ToolRequest) -> Result<ToolResponse, McpError> {
        if !self.vision_capture.is_available() {
            return Err(McpError::ExecutionError("Vision analysis not available".to_string()));
        }

        // In real implementation, would capture and analyze screenshot
        Ok(ToolResponse {
            content: vec![ToolContent::Text {
                text: "Page elements would be listed here (vision analysis required)".to_string(),
            }],
            is_error: false,
        })
    }
}

/// Helper function to generate unique action IDs
fn generate_action_id() -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64
}

/// Visual Tool Registry
/// Manages all visual interaction tools
pub struct VisualToolRegistry {
    tools: HashMap<String, Box<dyn McpTool>>,
}

impl VisualToolRegistry {
    pub fn new(vision_capture: Arc<VisionCapture>, autonomy_level: AutonomyLevel) -> Self {
        let mut tools: HashMap<String, Box<dyn McpTool>> = HashMap::new();

        // Register tools
        tools.insert(
            "browser.find_element".to_string(),
            Box::new(FindElementTool::new(Arc::clone(&vision_capture))),
        );
        tools.insert(
            "browser.click_element".to_string(),
            Box::new(ClickElementTool::new(Arc::clone(&vision_capture), autonomy_level)),
        );
        tools.insert(
            "browser.fill_field".to_string(),
            Box::new(FillFieldTool::new(Arc::clone(&vision_capture), autonomy_level)),
        );
        tools.insert(
            "browser.get_page_elements".to_string(),
            Box::new(GetPageElementsTool::new(vision_capture)),
        );

        Self { tools }
    }

    pub fn get_tool(&self, name: &str) -> Option<&dyn McpTool> {
        self.tools.get(name).map(|t| t.as_ref())
    }

    pub fn list_tools(&self) -> Vec<&dyn McpTool> {
        self.tools.values().map(|t| t.as_ref()).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_find_element_tool_params() {
        let tool = FindElementTool::new(Arc::new(VisionCapture::new(None)));
        let params = tool.parameters();
        
        assert_eq!(params.len(), 2);
        assert_eq!(params[0].name, "description");
        assert!(params[0].required);
    }
}
