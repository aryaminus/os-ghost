//! MCP Visual Tools for browser element interaction
//!
//! Provides tools for finding, clicking, and interacting with UI elements
//! detected through vision analysis.

use super::traits::*;
use super::types::*;
use crate::capture::vision::VisionCapture;
use crate::config::privacy::AutonomyLevel;
use async_trait::async_trait;
use serde_json::json;
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
    fn descriptor(&self) -> ToolDescriptor {
        ToolDescriptor {
            name: "browser.find_element".to_string(),
            description: "Find a UI element on the current page by description using computer vision".to_string(),
            input_schema: JsonSchema {
                schema_type: "object".to_string(),
                properties: Some({
                    let mut props = std::collections::HashMap::new();
                    props.insert(
                        "description".to_string(),
                        PropertySchema {
                            prop_type: "string".to_string(),
                            description: Some("Description of the element to find (e.g., 'Search button', 'Email input')".to_string()),
                            default: None,
                            enum_values: None,
                        },
                    );
                    props
                }),
                required: Some(vec!["description".to_string()]),
                description: Some("Find a UI element by description".to_string()),
            },
            is_side_effect: false,
            category: "browser".to_string(),
        }
    }

    async fn execute(&self, arguments: serde_json::Value) -> Result<serde_json::Value, McpError> {
        let description = arguments.get("description")
            .and_then(|v| v.as_str())
            .ok_or_else(|| McpError::InvalidArguments("Missing 'description' parameter".to_string()))?;

        if !self.vision_capture.is_available() {
            return Err(McpError::ExecutionFailed("Vision analysis not available".to_string()));
        }

        Ok(json!({
            "found": false,
            "message": format!("Looking for element: '{}' (vision integration pending)", description),
        }))
    }
}

/// Tool: browser.click_element
/// Clicks on an element found by description
#[allow(dead_code)]
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
    fn descriptor(&self) -> ToolDescriptor {
        ToolDescriptor {
            name: "browser.click_element".to_string(),
            description: "Click on a UI element by description".to_string(),
            input_schema: JsonSchema {
                schema_type: "object".to_string(),
                properties: Some({
                    let mut props = std::collections::HashMap::new();
                    props.insert(
                        "description".to_string(),
                        PropertySchema {
                            prop_type: "string".to_string(),
                            description: Some("Description of the element to click".to_string()),
                            default: None,
                            enum_values: None,
                        },
                    );
                    props
                }),
                required: Some(vec!["description".to_string()]),
                description: Some("Click on a UI element".to_string()),
            },
            is_side_effect: true,
            category: "browser".to_string(),
        }
    }

    async fn execute(&self, arguments: serde_json::Value) -> Result<serde_json::Value, McpError> {
        let description = arguments.get("description")
            .and_then(|v| v.as_str())
            .ok_or_else(|| McpError::InvalidArguments("Missing 'description' parameter".to_string()))?;

        match self.autonomy_level {
            AutonomyLevel::Observer => {
                Err(McpError::ExecutionFailed(
                    "Cannot click elements in Observer mode".to_string()
                ))
            }
            _ => {
                Ok(json!({
                    "success": true,
                    "message": format!("Click action queued for '{}' (requires approval)", description),
                    "requires_approval": true,
                }))
            }
        }
    }
}

/// Tool: browser.fill_field
/// Fills an input field with text
#[allow(dead_code)]
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
    fn descriptor(&self) -> ToolDescriptor {
        ToolDescriptor {
            name: "browser.fill_field".to_string(),
            description: "Fill an input field with text".to_string(),
            input_schema: JsonSchema {
                schema_type: "object".to_string(),
                properties: Some({
                    let mut props = std::collections::HashMap::new();
                    props.insert(
                        "field".to_string(),
                        PropertySchema {
                            prop_type: "string".to_string(),
                            description: Some("Description of the input field".to_string()),
                            default: None,
                            enum_values: None,
                        },
                    );
                    props.insert(
                        "value".to_string(),
                        PropertySchema {
                            prop_type: "string".to_string(),
                            description: Some("Text to enter into the field".to_string()),
                            default: None,
                            enum_values: None,
                        },
                    );
                    props
                }),
                required: Some(vec!["field".to_string(), "value".to_string()]),
                description: Some("Fill an input field".to_string()),
            },
            is_side_effect: true,
            category: "browser".to_string(),
        }
    }

    async fn execute(&self, arguments: serde_json::Value) -> Result<serde_json::Value, McpError> {
        let field = arguments.get("field")
            .and_then(|v| v.as_str())
            .ok_or_else(|| McpError::InvalidArguments("Missing 'field' parameter".to_string()))?;

        let _value = arguments.get("value")
            .and_then(|v| v.as_str())
            .ok_or_else(|| McpError::InvalidArguments("Missing 'value' parameter".to_string()))?;

        match self.autonomy_level {
            AutonomyLevel::Observer => {
                Err(McpError::ExecutionFailed("Cannot fill fields in Observer mode".to_string()))
            }
            _ => {
                let masked_value = "****";
                Ok(json!({
                    "success": true,
                    "message": format!("Will fill '{}' with '{}' (requires approval)", field, masked_value),
                    "requires_approval": true,
                }))
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
    fn descriptor(&self) -> ToolDescriptor {
        ToolDescriptor {
            name: "browser.get_page_elements".to_string(),
            description: "Analyze the current page and return all detected UI elements".to_string(),
            input_schema: JsonSchema {
                schema_type: "object".to_string(),
                properties: None,
                required: None,
                description: Some("Get all page elements".to_string()),
            },
            is_side_effect: false,
            category: "browser".to_string(),
        }
    }

    async fn execute(&self, _arguments: serde_json::Value) -> Result<serde_json::Value, McpError> {
        if !self.vision_capture.is_available() {
            return Err(McpError::ExecutionFailed("Vision analysis not available".to_string()));
        }

        Ok(json!({
            "elements": [],
            "message": "Page elements would be listed here (vision integration pending)",
        }))
    }
}

/// Visual Tool Registry
/// Manages all visual interaction tools
pub struct VisualToolRegistry {
    tools: Vec<Box<dyn McpTool>>,
}

impl VisualToolRegistry {
    pub fn new(vision_capture: Arc<VisionCapture>, autonomy_level: AutonomyLevel) -> Self {
        let tools: Vec<Box<dyn McpTool>> = vec![
            Box::new(FindElementTool::new(Arc::clone(&vision_capture))),
            Box::new(ClickElementTool::new(Arc::clone(&vision_capture), autonomy_level)),
            Box::new(FillFieldTool::new(Arc::clone(&vision_capture), autonomy_level)),
            Box::new(GetPageElementsTool::new(vision_capture)),
        ];

        Self { tools }
    }

    pub fn get_tools(&self) -> &Vec<Box<dyn McpTool>> {
        &self.tools
    }

    pub fn discover_tools(&self) -> Vec<ToolDescriptor> {
        self.tools.iter().map(|t| t.descriptor()).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_find_element_tool_descriptor() {
        let tool = FindElementTool::new(Arc::new(VisionCapture::new(None)));
        let desc = tool.descriptor();
        
        assert_eq!(desc.name, "browser.find_element");
        assert!(!desc.is_side_effect);
    }
}
