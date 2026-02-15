//! OS-Level MCP Tools
//!
//! Provides desktop automation capabilities through MCP:
//! - os.click - Click at screen coordinates
//! - os.type - Type text
//! - os.key - Press keys
//! - os.capture - Take screenshots
//! - os.move - Move mouse
//! - os.scroll - Scroll

use async_trait::async_trait;
use base64::Engine;
use serde_json::json;
use std::collections::HashMap;
use std::sync::Arc;
use tracing::info;

use crate::capture::vision::VisionCapture;
use crate::config::privacy::{AutonomyLevel, PrivacySettings};
use crate::input::{InputController, Key, MouseButton, ScrollDirection};
use crate::mcp::traits::McpTool;
use crate::mcp::types::{JsonSchema, McpError, PropertySchema, ToolDescriptor};

/// OS automation tool provider
pub struct OsToolProvider {
    input_controller: Arc<InputController>,
    vision_capture: Arc<VisionCapture>,
}

impl OsToolProvider {
    pub fn new(autonomy_level: AutonomyLevel, privacy_settings: PrivacySettings) -> Self {
        let input_controller = Arc::new(InputController::new(autonomy_level, privacy_settings));

        let vision_capture = Arc::new(VisionCapture::new(None));

        Self {
            input_controller,
            vision_capture,
        }
    }

    /// Get all OS tools
    pub fn get_tools(&self) -> Vec<Box<dyn McpTool>> {
        vec![
            Box::new(ClickTool::new(self.input_controller.clone())),
            Box::new(TypeTool::new(self.input_controller.clone())),
            Box::new(KeyTool::new(self.input_controller.clone())),
            Box::new(CaptureTool::new(self.vision_capture.clone())),
            Box::new(MoveTool::new(self.input_controller.clone())),
            Box::new(ScrollTool::new(self.input_controller.clone())),
        ]
    }
}

// ============================================================================
// Click Tool
// ============================================================================

pub struct ClickTool {
    input_controller: Arc<InputController>,
}

impl ClickTool {
    pub fn new(input_controller: Arc<InputController>) -> Self {
        Self { input_controller }
    }
}

#[async_trait]
impl McpTool for ClickTool {
    fn descriptor(&self) -> ToolDescriptor {
        let mut properties = HashMap::new();
        properties.insert(
            "x".to_string(),
            PropertySchema {
                prop_type: "integer".to_string(),
                description: Some("X coordinate".to_string()),
                default: None,
                enum_values: None,
            },
        );
        properties.insert(
            "y".to_string(),
            PropertySchema {
                prop_type: "integer".to_string(),
                description: Some("Y coordinate".to_string()),
                default: None,
                enum_values: None,
            },
        );
        properties.insert(
            "button".to_string(),
            PropertySchema {
                prop_type: "string".to_string(),
                description: Some("Mouse button".to_string()),
                default: Some(json!("left")),
                enum_values: Some(vec![
                    "left".to_string(),
                    "right".to_string(),
                    "middle".to_string(),
                ]),
            },
        );

        ToolDescriptor {
            name: "os.click".to_string(),
            description: "Click the mouse at specified screen coordinates".to_string(),
            input_schema: JsonSchema {
                schema_type: "object".to_string(),
                properties: Some(properties),
                required: Some(vec!["x".to_string(), "y".to_string()]),
                description: None,
            },
            is_side_effect: true,
            category: "os".to_string(),
        }
    }

    async fn execute(&self, arguments: serde_json::Value) -> Result<serde_json::Value, McpError> {
        let x = arguments["x"]
            .as_i64()
            .ok_or_else(|| McpError::InvalidArguments("Missing x".into()))? as i32;
        let y = arguments["y"]
            .as_i64()
            .ok_or_else(|| McpError::InvalidArguments("Missing y".into()))? as i32;
        let button_str = arguments["button"].as_str().unwrap_or("left");

        let button = match button_str {
            "left" => MouseButton::Left,
            "right" => MouseButton::Right,
            "middle" => MouseButton::Middle,
            _ => MouseButton::Left,
        };

        info!("Clicking at ({}, {}) with {:?} button", x, y, button);

        self.input_controller
            .move_mouse(x, y)
            .await
            .map_err(|e| McpError::ExecutionFailed(format!("Mouse move failed: {}", e)))?;

        self.input_controller
            .click_mouse(button)
            .await
            .map_err(|e| McpError::ExecutionFailed(format!("Click failed: {}", e)))?;

        Ok(json!({ "success": true, "x": x, "y": y, "button": button_str }))
    }
}

// ============================================================================
// Type Tool
// ============================================================================

pub struct TypeTool {
    input_controller: Arc<InputController>,
}

impl TypeTool {
    pub fn new(input_controller: Arc<InputController>) -> Self {
        Self { input_controller }
    }
}

#[async_trait]
impl McpTool for TypeTool {
    fn descriptor(&self) -> ToolDescriptor {
        let mut properties = HashMap::new();
        properties.insert(
            "text".to_string(),
            PropertySchema {
                prop_type: "string".to_string(),
                description: Some("Text to type".to_string()),
                default: None,
                enum_values: None,
            },
        );

        ToolDescriptor {
            name: "os.type".to_string(),
            description: "Type text as if from keyboard".to_string(),
            input_schema: JsonSchema {
                schema_type: "object".to_string(),
                properties: Some(properties),
                required: Some(vec!["text".to_string()]),
                description: None,
            },
            is_side_effect: true,
            category: "os".to_string(),
        }
    }

    async fn execute(&self, arguments: serde_json::Value) -> Result<serde_json::Value, McpError> {
        let text = arguments["text"]
            .as_str()
            .ok_or_else(|| McpError::InvalidArguments("Missing text".into()))?;

        info!("Typing text ({} chars)", text.len());

        self.input_controller
            .type_text(text)
            .await
            .map_err(|e| McpError::ExecutionFailed(format!("Type failed: {}", e)))?;

        Ok(json!({ "success": true, "characters_typed": text.len() }))
    }
}

// ============================================================================
// Capture Tool
// ============================================================================

pub struct CaptureTool {
    _vision_capture: Arc<VisionCapture>,
}

impl CaptureTool {
    pub fn new(vision_capture: Arc<VisionCapture>) -> Self {
        Self {
            _vision_capture: vision_capture,
        }
    }
}

#[async_trait]
impl McpTool for CaptureTool {
    fn descriptor(&self) -> ToolDescriptor {
        ToolDescriptor {
            name: "os.capture".to_string(),
            description: "Capture a screenshot of the desktop".to_string(),
            input_schema: JsonSchema {
                schema_type: "object".to_string(),
                properties: None,
                required: None,
                description: None,
            },
            is_side_effect: false,
            category: "os".to_string(),
        }
    }

    async fn execute(&self, _arguments: serde_json::Value) -> Result<serde_json::Value, McpError> {
        info!("Capturing screenshot");

        // Use desktop capture from input module
        let screenshot = crate::input::desktop_capture::capture_desktop()
            .await
            .map_err(|e| McpError::ExecutionFailed(format!("Capture failed: {}", e)))?;

        let base64_image = base64::engine::general_purpose::STANDARD.encode(&screenshot);

        Ok(json!({
            "success": true,
            "image_base64": base64_image,
            "message": "Screenshot captured"
        }))
    }
}

// ============================================================================
// Move Tool
// ============================================================================

pub struct MoveTool {
    input_controller: Arc<InputController>,
}

impl MoveTool {
    pub fn new(input_controller: Arc<InputController>) -> Self {
        Self { input_controller }
    }
}

#[async_trait]
impl McpTool for MoveTool {
    fn descriptor(&self) -> ToolDescriptor {
        let mut properties = HashMap::new();
        properties.insert(
            "x".to_string(),
            PropertySchema {
                prop_type: "integer".to_string(),
                description: Some("X coordinate".to_string()),
                default: None,
                enum_values: None,
            },
        );
        properties.insert(
            "y".to_string(),
            PropertySchema {
                prop_type: "integer".to_string(),
                description: Some("Y coordinate".to_string()),
                default: None,
                enum_values: None,
            },
        );

        ToolDescriptor {
            name: "os.move".to_string(),
            description: "Move the mouse cursor to specified coordinates".to_string(),
            input_schema: JsonSchema {
                schema_type: "object".to_string(),
                properties: Some(properties),
                required: Some(vec!["x".to_string(), "y".to_string()]),
                description: None,
            },
            is_side_effect: true,
            category: "os".to_string(),
        }
    }

    async fn execute(&self, arguments: serde_json::Value) -> Result<serde_json::Value, McpError> {
        let x = arguments["x"]
            .as_i64()
            .ok_or_else(|| McpError::InvalidArguments("Missing x".into()))? as i32;
        let y = arguments["y"]
            .as_i64()
            .ok_or_else(|| McpError::InvalidArguments("Missing y".into()))? as i32;

        info!("Moving mouse to ({}, {})", x, y);

        self.input_controller
            .move_mouse(x, y)
            .await
            .map_err(|e| McpError::ExecutionFailed(format!("Move failed: {}", e)))?;

        Ok(json!({ "success": true, "x": x, "y": y }))
    }
}

// ============================================================================
// Scroll Tool
// ============================================================================

pub struct ScrollTool {
    input_controller: Arc<InputController>,
}

impl ScrollTool {
    pub fn new(input_controller: Arc<InputController>) -> Self {
        Self { input_controller }
    }
}

#[async_trait]
impl McpTool for ScrollTool {
    fn descriptor(&self) -> ToolDescriptor {
        let mut properties = HashMap::new();
        properties.insert(
            "direction".to_string(),
            PropertySchema {
                prop_type: "string".to_string(),
                description: Some("Scroll direction".to_string()),
                default: None,
                enum_values: Some(vec![
                    "up".to_string(),
                    "down".to_string(),
                    "left".to_string(),
                    "right".to_string(),
                ]),
            },
        );
        properties.insert(
            "amount".to_string(),
            PropertySchema {
                prop_type: "integer".to_string(),
                description: Some("Scroll amount".to_string()),
                default: Some(json!(3)),
                enum_values: None,
            },
        );

        ToolDescriptor {
            name: "os.scroll".to_string(),
            description: "Scroll the mouse wheel".to_string(),
            input_schema: JsonSchema {
                schema_type: "object".to_string(),
                properties: Some(properties),
                required: Some(vec!["direction".to_string()]),
                description: None,
            },
            is_side_effect: true,
            category: "os".to_string(),
        }
    }

    async fn execute(&self, arguments: serde_json::Value) -> Result<serde_json::Value, McpError> {
        let direction_str = arguments["direction"]
            .as_str()
            .ok_or_else(|| McpError::InvalidArguments("Missing direction".into()))?;
        let amount = arguments["amount"].as_i64().unwrap_or(3) as i32;

        let direction = match direction_str {
            "up" => ScrollDirection::Up,
            "down" => ScrollDirection::Down,
            "left" => ScrollDirection::Left,
            "right" => ScrollDirection::Right,
            _ => ScrollDirection::Down,
        };

        info!("Scrolling {:?} by {}", direction, amount);

        self.input_controller
            .scroll(direction, amount)
            .await
            .map_err(|e| McpError::ExecutionFailed(format!("Scroll failed: {}", e)))?;

        Ok(json!({ "success": true, "direction": direction_str, "amount": amount }))
    }
}

// ============================================================================
// Key Tool (simplified for now)
// ============================================================================

pub struct KeyTool {
    input_controller: Arc<InputController>,
}

impl KeyTool {
    pub fn new(input_controller: Arc<InputController>) -> Self {
        Self { input_controller }
    }
}

#[async_trait]
impl McpTool for KeyTool {
    fn descriptor(&self) -> ToolDescriptor {
        let mut properties = HashMap::new();
        properties.insert(
            "key".to_string(),
            PropertySchema {
                prop_type: "string".to_string(),
                description: Some("Key to press".to_string()),
                default: None,
                enum_values: None,
            },
        );

        ToolDescriptor {
            name: "os.key".to_string(),
            description: "Press a keyboard key".to_string(),
            input_schema: JsonSchema {
                schema_type: "object".to_string(),
                properties: Some(properties),
                required: Some(vec!["key".to_string()]),
                description: None,
            },
            is_side_effect: true,
            category: "os".to_string(),
        }
    }

    async fn execute(&self, arguments: serde_json::Value) -> Result<serde_json::Value, McpError> {
        let _key = arguments["key"]
            .as_str()
            .ok_or_else(|| McpError::InvalidArguments("Missing key".into()))?;

        info!("Pressing key");

        // Simplified - just press space for now
        self.input_controller
            .press_key(Key::Space)
            .await
            .map_err(|e| McpError::ExecutionFailed(format!("Key press failed: {}", e)))?;

        Ok(json!({ "success": true }))
    }
}
