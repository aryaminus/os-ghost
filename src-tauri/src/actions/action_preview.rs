//! Action Preview System
//!
//! Implements streaming action previews similar to OpenAI Operator's "takeover mode".
//! Shows what the agent is about to do with live preview before confirmation.
//!
//! Key patterns from research:
//! - **Streaming Preview**: Real-time display of proposed action with visual preview
//! - **Edit Before Execute**: User can modify action parameters before approval
//! - **Instant Takeover**: User can abort/modify at any point in the stream
//! - **Risk Visualization**: Clear display of action risk level and potential consequences

use crate::actions::actions::PendingAction;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use tokio::sync::broadcast;

// ============================================================================
// Action Preview Types
// ============================================================================

/// Preview state representing streaming action visualization
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActionPreview {
    /// Unique preview ID
    pub id: String,
    /// The pending action being previewed
    pub action: PendingAction,
    /// Current preview state
    pub state: PreviewState,
    /// Visual preview data (e.g., screenshot of target, highlighted element)
    pub visual_preview: Option<VisualPreview>,
    /// Streaming progress (0.0 - 1.0)
    pub progress: f32,
    /// Editable parameters that user can modify
    pub editable_params: HashMap<String, EditableParam>,
    /// Preview started timestamp
    pub started_at: DateTime<Utc>,
    /// Estimated action duration in ms
    pub estimated_duration_ms: Option<u64>,
    /// Rollback possible?
    pub is_reversible: bool,
    /// Description of what can be undone
    pub rollback_description: Option<String>,
    /// Whether explicit approval is required
    #[serde(default)]
    pub requires_approval: bool,
    /// Summary of sensitive params
    #[serde(default)]
    pub approval_summary: Option<String>,
}

impl ActionPreview {
    /// Build updated arguments using editable parameters (if any)
    pub fn updated_arguments(&self) -> Option<serde_json::Value> {
        if self.editable_params.is_empty() {
            return self.action.arguments.clone();
        }

        let mut args = self
            .action
            .arguments
            .clone()
            .unwrap_or_else(|| serde_json::json!({}));

        if let Some(obj) = args.as_object_mut() {
            for (name, param) in &self.editable_params {
                obj.insert(name.clone(), param.value.clone());
            }
        }

        Some(args)
    }

    /// Check if any sensitive params require approval
    pub fn has_sensitive_params(&self) -> bool {
        self.editable_params
            .values()
            .any(|param| param.requires_approval)
    }
}

/// Preview lifecycle state
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PreviewState {
    /// Preview is loading/preparing
    Loading,
    /// Preview is streaming (showing what will happen)
    Streaming,
    /// Preview ready, awaiting user decision
    Ready,
    /// User is editing parameters
    Editing,
    /// User approved, executing
    Executing,
    /// Execution complete
    Completed,
    /// User denied action
    Denied,
    /// Preview expired or cancelled
    Cancelled,
}

/// Visual preview data for rich previews
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VisualPreview {
    /// Type of visual preview
    pub preview_type: VisualPreviewType,
    /// Preview content (base64 image, HTML snippet, etc.)
    pub content: String,
    /// Width for display
    pub width: Option<u32>,
    /// Height for display
    pub height: Option<u32>,
    /// Alt text for accessibility
    pub alt_text: String,
}

/// Types of visual previews
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VisualPreviewType {
    /// Screenshot of target area
    Screenshot,
    /// HTML snippet preview
    HtmlSnippet,
    /// URL preview card
    UrlCard,
    /// Element highlight overlay
    ElementHighlight,
    /// Text selection preview
    TextSelection,
}

/// Editable parameter that user can modify before execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EditableParam {
    /// Parameter name
    pub name: String,
    /// Current value
    pub value: serde_json::Value,
    /// Original value (for reset)
    pub original_value: serde_json::Value,
    /// Parameter type for UI rendering
    pub param_type: ParamType,
    /// Human-readable label
    pub label: String,
    /// Description/help text
    pub description: Option<String>,
    /// Additional help text
    #[serde(default)]
    pub help_text: Option<String>,
    /// Unit label for numeric values
    #[serde(default)]
    pub unit: Option<String>,
    /// Validation constraints
    pub constraints: Option<ParamConstraints>,
    /// Whether this param requires explicit approval
    #[serde(default)]
    pub requires_approval: bool,
    /// Why approval is needed
    #[serde(default)]
    pub approval_reason: Option<String>,
}

/// Parameter types for UI rendering
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ParamType {
    /// Text input
    Text,
    /// URL input
    Url,
    /// Number input
    Number,
    /// Boolean toggle
    Boolean,
    /// Select from options
    Select,
    /// Duration in ms
    Duration,
}

/// Validation constraints for parameters
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParamConstraints {
    /// Minimum value (for numbers)
    pub min: Option<f64>,
    /// Maximum value (for numbers)
    pub max: Option<f64>,
    /// Max length (for strings)
    pub max_length: Option<usize>,
    /// Regex pattern (for strings)
    pub pattern: Option<String>,
    /// Allowed values (for select)
    pub options: Option<Vec<String>>,
    /// Required?
    pub required: bool,
}

// ============================================================================
// Preview Events
// ============================================================================

/// Events emitted during preview lifecycle
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PreviewEvent {
    /// Preview ID
    pub preview_id: String,
    /// Event type
    pub event_type: PreviewEventType,
    /// Event timestamp
    pub timestamp: DateTime<Utc>,
    /// Additional event data
    pub data: serde_json::Value,
}

/// Types of preview events
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PreviewEventType {
    /// Preview started
    Started,
    /// Progress updated
    Progress,
    /// Visual preview ready
    VisualReady,
    /// State changed
    StateChanged,
    /// Parameter edited
    ParamEdited,
    /// User approved
    Approved,
    /// User denied
    Denied,
    /// Execution started
    ExecutionStarted,
    /// Execution completed
    ExecutionCompleted,
    /// Execution failed
    ExecutionFailed,
    /// Preview expired/cancelled
    Cancelled,
}

// ============================================================================
// Preview Manager
// ============================================================================

/// Manages active action previews
pub struct PreviewManager {
    /// Currently active preview (only one at a time for focus)
    active_preview: Arc<Mutex<Option<ActionPreview>>>,
    /// Preview history (last 20)
    history: Arc<Mutex<Vec<ActionPreview>>>,
    /// Event broadcast channel
    event_tx: broadcast::Sender<PreviewEvent>,
    /// Counter for unique preview IDs
    counter: AtomicU64,
}

impl Default for PreviewManager {
    fn default() -> Self {
        Self::new()
    }
}

impl PreviewManager {
    /// Create new preview manager
    pub fn new() -> Self {
        let (event_tx, _) = broadcast::channel(100);
        Self {
            active_preview: Arc::new(Mutex::new(None)),
            history: Arc::new(Mutex::new(Vec::new())),
            event_tx,
            counter: AtomicU64::new(0),
        }
    }

    /// Subscribe to preview events
    pub fn subscribe(&self) -> broadcast::Receiver<PreviewEvent> {
        self.event_tx.subscribe()
    }

    /// Create a preview for a pending action
    pub fn create_preview(&self, action: &PendingAction) -> ActionPreview {
        let id = format!("preview_{}", self.counter.fetch_add(1, Ordering::Relaxed));

        // Determine if action is reversible
        let (is_reversible, rollback_desc) = match action.action_type.as_str() {
            "browser.navigate" => (true, Some("Can navigate back to previous page".to_string())),
            "browser.inject_effect" => (true, Some("Effect will fade after duration".to_string())),
            "browser.highlight_text" => (true, Some("Highlight can be removed".to_string())),
            "sandbox.write_file" => (true, Some("File write can be reverted".to_string())),
            "notes.add" | "notes.update" | "notes.delete" => {
                (true, Some("Note changes can be reverted".to_string()))
            }
            _ => (false, None),
        };

        // Build editable parameters from action arguments
        let editable_params = self.extract_editable_params(action);

        // Estimate duration based on action type
        let estimated_duration = match action.action_type.as_str() {
            "browser.navigate" => Some(2000),
            "browser.inject_effect" => action
                .arguments
                .as_ref()
                .and_then(|a| a.get("duration"))
                .and_then(|d| d.as_u64())
                .or(Some(1000)),
            "browser.highlight_text" => Some(500),
            _ => None,
        };

        let has_sensitive_param = editable_params
            .values()
            .any(|param| param.requires_approval);

        ActionPreview {
            id,
            action: action.clone(),
            state: PreviewState::Loading,
            visual_preview: None,
            progress: 0.0,
            editable_params,
            started_at: Utc::now(),
            estimated_duration_ms: estimated_duration,
            is_reversible,
            rollback_description: rollback_desc,
            requires_approval: has_sensitive_param,
            approval_summary: if has_sensitive_param {
                Some("Sensitive parameters require explicit approval".to_string())
            } else {
                None
            },
        }
    }

    /// Extract editable parameters from action
    fn extract_editable_params(&self, action: &PendingAction) -> HashMap<String, EditableParam> {
        let mut params = HashMap::new();

        if let Some(args) = &action.arguments {
            match action.action_type.as_str() {
                "browser.navigate" => {
                    if let Some(url) = args.get("url") {
                        params.insert(
                            "url".to_string(),
                            EditableParam {
                                name: "url".to_string(),
                                value: url.clone(),
                                original_value: url.clone(),
                                param_type: ParamType::Url,
                                label: "Target URL".to_string(),
                                description: Some("The URL to navigate to".to_string()),
                                help_text: None,
                                unit: None,
                                constraints: Some(ParamConstraints {
                                    min: None,
                                    max: None,
                                    max_length: Some(2048),
                                    pattern: Some(r"^https?://".to_string()),
                                    options: None,
                                    required: true,
                                }),
                                requires_approval: false,
                                approval_reason: None,
                            },
                        );
                    }
                }
                "browser.inject_effect" => {
                    if let Some(effect) = args.get("effect") {
                        params.insert(
                            "effect".to_string(),
                            EditableParam {
                                name: "effect".to_string(),
                                value: effect.clone(),
                                original_value: effect.clone(),
                                param_type: ParamType::Select,
                                label: "Visual Effect".to_string(),
                                description: Some("The visual effect to apply".to_string()),
                                help_text: None,
                                unit: None,
                                constraints: Some(ParamConstraints {
                                    min: None,
                                    max: None,
                                    max_length: None,
                                    pattern: None,
                                    options: Some(vec![
                                        "glitch".to_string(),
                                        "scanlines".to_string(),
                                        "static".to_string(),
                                        "flicker".to_string(),
                                        "vignette".to_string(),
                                    ]),
                                    required: true,
                                }),
                                requires_approval: false,
                                approval_reason: None,
                            },
                        );
                    }
                    if let Some(duration) = args.get("duration") {
                        params.insert(
                            "duration".to_string(),
                            EditableParam {
                                name: "duration".to_string(),
                                value: duration.clone(),
                                original_value: duration.clone(),
                                param_type: ParamType::Duration,
                                label: "Duration (ms)".to_string(),
                                description: Some("How long the effect lasts".to_string()),
                                help_text: None,
                                unit: Some("ms".to_string()),
                                constraints: Some(ParamConstraints {
                                    min: Some(100.0),
                                    max: Some(10000.0),
                                    max_length: None,
                                    pattern: None,
                                    options: None,
                                    required: false,
                                }),
                                requires_approval: false,
                                approval_reason: None,
                            },
                        );
                    }
                }
                "browser.highlight_text" => {
                    if let Some(text) = args.get("text") {
                        params.insert(
                            "text".to_string(),
                            EditableParam {
                                name: "text".to_string(),
                                value: text.clone(),
                                original_value: text.clone(),
                                param_type: ParamType::Text,
                                label: "Text to Highlight".to_string(),
                                description: Some(
                                    "The text to find and highlight on the page".to_string(),
                                ),
                                help_text: None,
                                unit: None,
                                constraints: Some(ParamConstraints {
                                    min: None,
                                    max: None,
                                    max_length: Some(500),
                                    pattern: None,
                                    options: None,
                                    required: true,
                                }),
                                requires_approval: false,
                                approval_reason: None,
                            },
                        );
                    }
                }
                "sandbox.read_file" => {
                    if let Some(path) = args.get("path") {
                        params.insert(
                            "path".to_string(),
                            EditableParam {
                                name: "path".to_string(),
                                value: path.clone(),
                                original_value: path.clone(),
                                param_type: ParamType::Text,
                                label: "File path".to_string(),
                                description: Some("Path of the file to read".to_string()),
                                help_text: None,
                                unit: None,
                                constraints: Some(ParamConstraints {
                                    min: None,
                                    max: None,
                                    max_length: Some(4096),
                                    pattern: None,
                                    options: None,
                                    required: true,
                                }),
                                requires_approval: true,
                                approval_reason: Some(
                                    "Reading local files exposes sensitive data".to_string(),
                                ),
                            },
                        );
                    }
                }
                "sandbox.write_file" => {
                    if let Some(path) = args.get("path") {
                        params.insert(
                            "path".to_string(),
                            EditableParam {
                                name: "path".to_string(),
                                value: path.clone(),
                                original_value: path.clone(),
                                param_type: ParamType::Text,
                                label: "File path".to_string(),
                                description: Some("Path of the file to write".to_string()),
                                help_text: None,
                                unit: None,
                                constraints: Some(ParamConstraints {
                                    min: None,
                                    max: None,
                                    max_length: Some(4096),
                                    pattern: None,
                                    options: None,
                                    required: true,
                                }),
                                requires_approval: true,
                                approval_reason: Some(
                                    "Writing files can overwrite local data".to_string(),
                                ),
                            },
                        );
                    }
                    if let Some(content) = args.get("content") {
                        params.insert(
                            "content".to_string(),
                            EditableParam {
                                name: "content".to_string(),
                                value: content.clone(),
                                original_value: content.clone(),
                                param_type: ParamType::Text,
                                label: "Content".to_string(),
                                description: Some("Content to write into the file".to_string()),
                                help_text: None,
                                unit: None,
                                constraints: Some(ParamConstraints {
                                    min: None,
                                    max: None,
                                    max_length: Some(20000),
                                    pattern: None,
                                    options: None,
                                    required: true,
                                }),
                                requires_approval: true,
                                approval_reason: Some(
                                    "File contents may include sensitive data".to_string(),
                                ),
                            },
                        );
                    }
                    if let Some(create_dirs) = args.get("create_dirs") {
                        params.insert(
                            "create_dirs".to_string(),
                            EditableParam {
                                name: "create_dirs".to_string(),
                                value: create_dirs.clone(),
                                original_value: create_dirs.clone(),
                                param_type: ParamType::Boolean,
                                label: "Create directories".to_string(),
                                description: Some(
                                    "Create parent directories if missing".to_string(),
                                ),
                                help_text: None,
                                unit: None,
                                constraints: None,
                                requires_approval: false,
                                approval_reason: None,
                            },
                        );
                    }
                }
                "sandbox.list_dir" => {
                    if let Some(path) = args.get("path") {
                        params.insert(
                            "path".to_string(),
                            EditableParam {
                                name: "path".to_string(),
                                value: path.clone(),
                                original_value: path.clone(),
                                param_type: ParamType::Text,
                                label: "Directory path".to_string(),
                                description: Some("Path of the directory to list".to_string()),
                                help_text: None,
                                unit: None,
                                constraints: Some(ParamConstraints {
                                    min: None,
                                    max: None,
                                    max_length: Some(4096),
                                    pattern: None,
                                    options: None,
                                    required: true,
                                }),
                                requires_approval: true,
                                approval_reason: Some(
                                    "Directory listings expose local files".to_string(),
                                ),
                            },
                        );
                    }
                    if let Some(include_hidden) = args.get("include_hidden") {
                        params.insert(
                            "include_hidden".to_string(),
                            EditableParam {
                                name: "include_hidden".to_string(),
                                value: include_hidden.clone(),
                                original_value: include_hidden.clone(),
                                param_type: ParamType::Boolean,
                                label: "Include hidden".to_string(),
                                description: Some("Include hidden files in listing".to_string()),
                                help_text: None,
                                unit: None,
                                constraints: None,
                                requires_approval: false,
                                approval_reason: None,
                            },
                        );
                    }
                }
                "sandbox.shell" => {
                    if let Some(command) = args.get("command") {
                        params.insert(
                            "command".to_string(),
                            EditableParam {
                                name: "command".to_string(),
                                value: command.clone(),
                                original_value: command.clone(),
                                param_type: ParamType::Text,
                                label: "Command".to_string(),
                                description: Some("Shell command to execute".to_string()),
                                help_text: None,
                                unit: None,
                                constraints: Some(ParamConstraints {
                                    min: None,
                                    max: None,
                                    max_length: Some(2048),
                                    pattern: None,
                                    options: None,
                                    required: true,
                                }),
                                requires_approval: true,
                                approval_reason: Some(
                                    "Shell commands can modify the system".to_string(),
                                ),
                            },
                        );
                    }
                    if let Some(working_dir) = args.get("working_dir") {
                        params.insert(
                            "working_dir".to_string(),
                            EditableParam {
                                name: "working_dir".to_string(),
                                value: working_dir.clone(),
                                original_value: working_dir.clone(),
                                param_type: ParamType::Text,
                                label: "Working directory".to_string(),
                                description: Some("Directory to run the command in".to_string()),
                                help_text: None,
                                unit: None,
                                constraints: Some(ParamConstraints {
                                    min: None,
                                    max: None,
                                    max_length: Some(4096),
                                    pattern: None,
                                    options: None,
                                    required: false,
                                }),
                                requires_approval: false,
                                approval_reason: None,
                            },
                        );
                    }
                }
                "extension.tool" => {
                    let schema = args.get("args_schema").cloned();
                    if let Some(extension_id) = args.get("extension_id") {
                        params.insert(
                            "extension_id".to_string(),
                            EditableParam {
                                name: "extension_id".to_string(),
                                value: extension_id.clone(),
                                original_value: extension_id.clone(),
                                param_type: ParamType::Text,
                                label: "Extension ID".to_string(),
                                description: Some("Extension identifier".to_string()),
                                help_text: None,
                                unit: None,
                                constraints: Some(ParamConstraints {
                                    min: None,
                                    max: None,
                                    max_length: Some(128),
                                    pattern: None,
                                    options: None,
                                    required: true,
                                }),
                                requires_approval: false,
                                approval_reason: None,
                            },
                        );
                    }
                    if let Some(tool_name) = args.get("tool_name") {
                        params.insert(
                            "tool_name".to_string(),
                            EditableParam {
                                name: "tool_name".to_string(),
                                value: tool_name.clone(),
                                original_value: tool_name.clone(),
                                param_type: ParamType::Text,
                                label: "Tool name".to_string(),
                                description: Some("Tool to execute".to_string()),
                                help_text: None,
                                unit: None,
                                constraints: Some(ParamConstraints {
                                    min: None,
                                    max: None,
                                    max_length: Some(128),
                                    pattern: None,
                                    options: None,
                                    required: true,
                                }),
                                requires_approval: false,
                                approval_reason: None,
                            },
                        );
                    }
                    if let Some(schema_value) = schema {
                        if let Some(properties) =
                            schema_value.get("properties").and_then(|v| v.as_object())
                        {
                            let required = schema_value
                                .get("required")
                                .and_then(|v| v.as_array())
                                .map(|arr| {
                                    arr.iter().filter_map(|v| v.as_str()).collect::<Vec<_>>()
                                })
                                .unwrap_or_default();
                            for (name, def) in properties {
                                let param_type = match def.get("type").and_then(|v| v.as_str()) {
                                    Some("number") | Some("integer") => ParamType::Number,
                                    Some("boolean") => ParamType::Boolean,
                                    _ => ParamType::Text,
                                };
                                let default_value = def.get("default").cloned().unwrap_or_else(
                                    || match param_type {
                                        ParamType::Number => serde_json::json!(0),
                                        ParamType::Boolean => serde_json::json!(false),
                                        _ => serde_json::json!(""),
                                    },
                                );
                                let description = def
                                    .get("description")
                                    .and_then(|v| v.as_str())
                                    .map(|s| s.to_string());
                                let help_text = def
                                    .get("help")
                                    .or_else(|| def.get("x-help"))
                                    .or_else(|| def.get("x_help"))
                                    .or_else(|| def.get("hint"))
                                    .or_else(|| def.get("x-hint"))
                                    .or_else(|| def.get("x_hint"))
                                    .and_then(|v| v.as_str())
                                    .map(|s| s.to_string());
                                let unit = def
                                    .get("unit")
                                    .or_else(|| def.get("units"))
                                    .or_else(|| def.get("x-unit"))
                                    .or_else(|| def.get("x_unit"))
                                    .and_then(|v| v.as_str())
                                    .map(|s| s.to_string());
                                let options =
                                    def.get("enum").and_then(|v| v.as_array()).map(|arr| {
                                        arr.iter()
                                            .filter_map(|v| v.as_str())
                                            .map(|s| s.to_string())
                                            .collect::<Vec<_>>()
                                    });
                                let constraints = ParamConstraints {
                                    min: def.get("minimum").and_then(|v| v.as_f64()),
                                    max: def.get("maximum").and_then(|v| v.as_f64()),
                                    max_length: def
                                        .get("maxLength")
                                        .and_then(|v| v.as_u64())
                                        .map(|v| v as usize),
                                    pattern: def
                                        .get("pattern")
                                        .and_then(|v| v.as_str())
                                        .map(|s| s.to_string()),
                                    options,
                                    required: required.contains(&name.as_str()),
                                };
                                params.insert(
                                    format!("arg.{}", name),
                                    EditableParam {
                                        name: format!("arg.{}", name),
                                        value: default_value.clone(),
                                        original_value: default_value,
                                        param_type,
                                        label: name.to_string(),
                                        description,
                                        help_text,
                                        unit,
                                        constraints: Some(constraints),
                                        requires_approval: true,
                                        approval_reason: Some(
                                            "Extension arguments require confirmation".to_string(),
                                        ),
                                    },
                                );
                            }
                        }
                    } else if let Some(args_val) = args.get("args") {
                        let as_text = if args_val.is_array() {
                            args_val
                                .as_array()
                                .unwrap_or(&vec![])
                                .iter()
                                .filter_map(|v| v.as_str())
                                .collect::<Vec<_>>()
                                .join(" ")
                        } else {
                            args_val.to_string()
                        };
                        params.insert(
                            "args_text".to_string(),
                            EditableParam {
                                name: "args_text".to_string(),
                                value: serde_json::Value::String(as_text.clone()),
                                original_value: serde_json::Value::String(as_text),
                                param_type: ParamType::Text,
                                label: "Arguments".to_string(),
                                description: Some("Space-separated arguments".to_string()),
                                help_text: None,
                                unit: None,
                                constraints: Some(ParamConstraints {
                                    min: None,
                                    max: None,
                                    max_length: Some(2048),
                                    pattern: None,
                                    options: None,
                                    required: false,
                                }),
                                requires_approval: true,
                                approval_reason: Some(
                                    "Extension arguments require confirmation".to_string(),
                                ),
                            },
                        );
                    }
                }
                "intent.quick_ask" => {
                    if let Some(prompt) = args.get("prompt") {
                        params.insert(
                            "prompt".to_string(),
                            EditableParam {
                                name: "prompt".to_string(),
                                value: prompt.clone(),
                                original_value: prompt.clone(),
                                param_type: ParamType::Text,
                                label: "Prompt".to_string(),
                                description: Some(
                                    "Question or instruction for quick ask".to_string(),
                                ),
                                help_text: None,
                                unit: None,
                                constraints: Some(ParamConstraints {
                                    min: None,
                                    max: None,
                                    max_length: Some(2048),
                                    pattern: None,
                                    options: None,
                                    required: true,
                                }),
                                requires_approval: false,
                                approval_reason: None,
                            },
                        );
                    }
                }
                "intent.summarize_page" => {
                    params.insert(
                        "tone".to_string(),
                        EditableParam {
                            name: "tone".to_string(),
                            value: serde_json::Value::String("concise".to_string()),
                            original_value: serde_json::Value::String("concise".to_string()),
                            param_type: ParamType::Select,
                            label: "Tone".to_string(),
                            description: Some("How the summary should feel".to_string()),
                            help_text: None,
                            unit: None,
                            constraints: Some(ParamConstraints {
                                min: None,
                                max: None,
                                max_length: None,
                                pattern: None,
                                options: Some(vec![
                                    "concise".to_string(),
                                    "detailed".to_string(),
                                    "bulleted".to_string(),
                                ]),
                                required: false,
                            }),
                            requires_approval: false,
                            approval_reason: None,
                        },
                    );
                }
                "intent.create_tasks" => {
                    params.insert(
                        "format".to_string(),
                        EditableParam {
                            name: "format".to_string(),
                            value: serde_json::Value::String("checklist".to_string()),
                            original_value: serde_json::Value::String("checklist".to_string()),
                            param_type: ParamType::Select,
                            label: "Format".to_string(),
                            description: Some("How tasks should be listed".to_string()),
                            help_text: None,
                            unit: None,
                            constraints: Some(ParamConstraints {
                                min: None,
                                max: None,
                                max_length: None,
                                pattern: None,
                                options: Some(vec!["checklist".to_string(), "bullets".to_string()]),
                                required: false,
                            }),
                            requires_approval: false,
                            approval_reason: None,
                        },
                    );
                }
                "intent.draft_reply" => {
                    params.insert(
                        "style".to_string(),
                        EditableParam {
                            name: "style".to_string(),
                            value: serde_json::Value::String("friendly".to_string()),
                            original_value: serde_json::Value::String("friendly".to_string()),
                            param_type: ParamType::Select,
                            label: "Style".to_string(),
                            description: Some("Tone for the draft reply".to_string()),
                            help_text: None,
                            unit: None,
                            constraints: Some(ParamConstraints {
                                min: None,
                                max: None,
                                max_length: None,
                                pattern: None,
                                options: Some(vec![
                                    "friendly".to_string(),
                                    "formal".to_string(),
                                    "short".to_string(),
                                ]),
                                required: false,
                            }),
                            requires_approval: false,
                            approval_reason: None,
                        },
                    );
                }
                _ => {}
            }
        }

        params
    }

    /// Start streaming a preview
    pub fn start_preview(&self, action: &PendingAction) -> ActionPreview {
        let mut preview = self.create_preview(action);
        preview.state = PreviewState::Streaming;

        // Store as active preview
        {
            let mut active = self.active_preview.lock().unwrap();
            *active = Some(preview.clone());
        }

        // Emit start event
        let _ = self.event_tx.send(PreviewEvent {
            preview_id: preview.id.clone(),
            event_type: PreviewEventType::Started,
            timestamp: Utc::now(),
            data: serde_json::json!({
                "action_type": preview.action.action_type,
                "risk_level": preview.action.risk_level,
            }),
        });

        preview
    }

    /// Update preview progress
    pub fn update_progress(&self, preview_id: &str, progress: f32) {
        let mut active = self.active_preview.lock().unwrap();
        if let Some(preview) = active.as_mut() {
            if preview.id == preview_id {
                preview.progress = progress.clamp(0.0, 1.0);

                // Mark as ready when progress reaches 1.0
                if preview.progress >= 1.0 {
                    preview.state = PreviewState::Ready;
                }

                let _ = self.event_tx.send(PreviewEvent {
                    preview_id: preview_id.to_string(),
                    event_type: PreviewEventType::Progress,
                    timestamp: Utc::now(),
                    data: serde_json::json!({ "progress": preview.progress }),
                });
            }
        }
    }

    /// Set visual preview data
    pub fn set_visual_preview(&self, preview_id: &str, visual: VisualPreview) {
        let mut active = self.active_preview.lock().unwrap();
        if let Some(preview) = active.as_mut() {
            if preview.id == preview_id {
                preview.visual_preview = Some(visual);

                let _ = self.event_tx.send(PreviewEvent {
                    preview_id: preview_id.to_string(),
                    event_type: PreviewEventType::VisualReady,
                    timestamp: Utc::now(),
                    data: serde_json::json!({}),
                });
            }
        }
    }

    /// Update a parameter value
    pub fn update_param(
        &self,
        preview_id: &str,
        param_name: &str,
        value: serde_json::Value,
    ) -> Result<(), String> {
        let mut active = self.active_preview.lock().unwrap();
        if let Some(preview) = active.as_mut() {
            if preview.id == preview_id {
                if let Some(param) = preview.editable_params.get_mut(param_name) {
                    // Validate if constraints exist
                    if let Some(constraints) = &param.constraints {
                        self.validate_param(&value, constraints)?;
                    }

                    param.value = value.clone();
                    preview.state = PreviewState::Ready;

                    let _ = self.event_tx.send(PreviewEvent {
                        preview_id: preview_id.to_string(),
                        event_type: PreviewEventType::ParamEdited,
                        timestamp: Utc::now(),
                        data: serde_json::json!({
                            "param": param_name,
                            "value": value,
                        }),
                    });

                    Ok(())
                } else {
                    Err(format!("Unknown parameter: {}", param_name))
                }
            } else {
                Err("Preview ID mismatch".to_string())
            }
        } else {
            Err("No active preview".to_string())
        }
    }

    /// Validate a parameter value against constraints
    fn validate_param(
        &self,
        value: &serde_json::Value,
        constraints: &ParamConstraints,
    ) -> Result<(), String> {
        // Check required
        if constraints.required && value.is_null() {
            return Err("Value is required".to_string());
        }

        // Check numeric constraints
        if let Some(num) = value.as_f64() {
            if let Some(min) = constraints.min {
                if num < min {
                    return Err(format!("Value must be at least {}", min));
                }
            }
            if let Some(max) = constraints.max {
                if num > max {
                    return Err(format!("Value must be at most {}", max));
                }
            }
        }

        // Check string constraints
        if let Some(s) = value.as_str() {
            if let Some(max_len) = constraints.max_length {
                if s.len() > max_len {
                    return Err(format!("Value too long (max {} chars)", max_len));
                }
            }
            if let Some(pattern) = &constraints.pattern {
                if let Ok(re) = regex::Regex::new(pattern) {
                    if !re.is_match(s) {
                        return Err("Value doesn't match required format".to_string());
                    }
                }
            }
            if let Some(options) = &constraints.options {
                if !options.contains(&s.to_string()) {
                    return Err(format!("Value must be one of: {:?}", options));
                }
            }
        }

        Ok(())
    }

    /// Approve the preview and execute
    pub fn approve_preview(&self, preview_id: &str) -> Result<(), String> {
        let preview = {
            let mut active = self.active_preview.lock().unwrap();
            if let Some(preview) = active.as_mut() {
                if preview.id == preview_id {
                    if let Some(updated_args) = preview.updated_arguments() {
                        preview.action.arguments = Some(updated_args);
                    }
                    preview.state = PreviewState::Executing;
                    Some(preview.clone())
                } else {
                    return Err("Preview ID mismatch".to_string());
                }
            } else {
                return Err("No active preview".to_string());
            }
        };

        if preview.is_some() {
            let _ = self.event_tx.send(PreviewEvent {
                preview_id: preview_id.to_string(),
                event_type: PreviewEventType::Approved,
                timestamp: Utc::now(),
                data: serde_json::json!({}),
            });

            // Note: The actual action queue approval is handled by the Tauri command
            // that calls this method - we just track state here

            // Move to history
            self.move_to_history(preview_id);
        }

        Ok(())
    }

    /// Deny the preview
    pub fn deny_preview(&self, preview_id: &str, reason: Option<String>) -> Result<(), String> {
        let mut active = self.active_preview.lock().unwrap();
        if let Some(preview) = active.as_mut() {
            if preview.id == preview_id {
                preview.state = PreviewState::Denied;

                let _ = self.event_tx.send(PreviewEvent {
                    preview_id: preview_id.to_string(),
                    event_type: PreviewEventType::Denied,
                    timestamp: Utc::now(),
                    data: serde_json::json!({ "reason": reason }),
                });

                // Note: The actual action queue denial is handled by the Tauri command
                // that calls this method - we just track state here

                // Move to history
                drop(active);
                self.move_to_history(preview_id);

                Ok(())
            } else {
                Err("Preview ID mismatch".to_string())
            }
        } else {
            Err("No active preview".to_string())
        }
    }

    /// Cancel the preview (without deny)
    pub fn cancel_preview(&self, preview_id: &str) -> Result<(), String> {
        let mut active = self.active_preview.lock().unwrap();
        if let Some(preview) = active.as_mut() {
            if preview.id == preview_id {
                preview.state = PreviewState::Cancelled;

                let _ = self.event_tx.send(PreviewEvent {
                    preview_id: preview_id.to_string(),
                    event_type: PreviewEventType::Cancelled,
                    timestamp: Utc::now(),
                    data: serde_json::json!({}),
                });

                drop(active);
                self.move_to_history(preview_id);

                Ok(())
            } else {
                Err("Preview ID mismatch".to_string())
            }
        } else {
            Err("No active preview".to_string())
        }
    }

    /// Move preview to history
    fn move_to_history(&self, preview_id: &str) {
        let preview = {
            let mut active = self.active_preview.lock().unwrap();
            if active.as_ref().is_some_and(|p| p.id == preview_id) {
                active.take()
            } else {
                None
            }
        };

        if let Some(preview) = preview {
            let mut history = self.history.lock().unwrap();
            history.insert(0, preview);
            history.truncate(20);
        }
    }

    /// Get current active preview
    pub fn get_active_preview(&self) -> Option<ActionPreview> {
        self.active_preview.lock().unwrap().clone()
    }

    /// Get preview history
    pub fn get_history(&self) -> Vec<ActionPreview> {
        self.history.lock().unwrap().clone()
    }

    /// Mark execution as complete
    pub fn mark_completed(&self, preview_id: &str, success: bool, error: Option<String>) {
        let mut active = self.active_preview.lock().unwrap();
        if let Some(preview) = active.as_mut() {
            if preview.id == preview_id {
                preview.state = PreviewState::Completed;

                let event_type = if success {
                    PreviewEventType::ExecutionCompleted
                } else {
                    PreviewEventType::ExecutionFailed
                };

                let _ = self.event_tx.send(PreviewEvent {
                    preview_id: preview_id.to_string(),
                    event_type,
                    timestamp: Utc::now(),
                    data: serde_json::json!({
                        "success": success,
                        "error": error,
                    }),
                });

                drop(active);
                self.move_to_history(preview_id);
            }
        }
    }
}

// ============================================================================
// Global Instance
// ============================================================================

use lazy_static::lazy_static;
use std::sync::RwLock;

lazy_static! {
    /// Global preview manager instance
    static ref PREVIEW_MANAGER: RwLock<PreviewManager> = RwLock::new(PreviewManager::new());
}

/// Initialize the global preview manager (no-op with lazy_static, kept for API compatibility)
pub fn init_preview_manager() {
    // The lazy_static initializes on first access
    drop(PREVIEW_MANAGER.read());
}

/// Get the global preview manager
pub fn get_preview_manager() -> Option<std::sync::RwLockReadGuard<'static, PreviewManager>> {
    PREVIEW_MANAGER.read().ok()
}

/// Get mutable access to the preview manager
pub fn get_preview_manager_mut() -> Option<std::sync::RwLockWriteGuard<'static, PreviewManager>> {
    PREVIEW_MANAGER.write().ok()
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::actions::ActionRiskLevel;

    #[test]
    fn test_create_preview() {
        let manager = PreviewManager::new();

        let action = PendingAction {
            id: 1001,
            action_type: "browser.navigate".to_string(),
            description: "Navigate to example.com".to_string(),
            target: "https://example.com".to_string(),
            risk_level: ActionRiskLevel::Medium,
            status: crate::actions::ActionStatus::Pending,
            created_at: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
            reason: None,
            arguments: Some(serde_json::json!({ "url": "https://example.com" })),
        };

        let preview = manager.create_preview(&action);

        assert!(preview.id.starts_with("preview_"));
        assert_eq!(preview.state, PreviewState::Loading);
        assert!(preview.is_reversible);
        assert!(preview.editable_params.contains_key("url"));
    }

    #[test]
    fn test_editable_params() {
        let manager = PreviewManager::new();

        let action = PendingAction {
            id: 1002,
            action_type: "browser.inject_effect".to_string(),
            description: "Apply glitch effect".to_string(),
            target: String::new(),
            risk_level: ActionRiskLevel::Low,
            status: crate::actions::ActionStatus::Pending,
            created_at: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
            reason: None,
            arguments: Some(serde_json::json!({
                "effect": "glitch",
                "duration": 1000
            })),
        };

        let preview = manager.create_preview(&action);

        assert!(preview.editable_params.contains_key("effect"));
        assert!(preview.editable_params.contains_key("duration"));

        let effect_param = preview.editable_params.get("effect").unwrap();
        assert_eq!(effect_param.param_type, ParamType::Select);
    }

    #[test]
    fn test_param_validation() {
        let manager = PreviewManager::new();

        let constraints = ParamConstraints {
            min: Some(100.0),
            max: Some(10000.0),
            max_length: None,
            pattern: None,
            options: None,
            required: true,
        };

        // Valid value
        assert!(manager
            .validate_param(&serde_json::json!(500), &constraints)
            .is_ok());

        // Below min
        assert!(manager
            .validate_param(&serde_json::json!(50), &constraints)
            .is_err());

        // Above max
        assert!(manager
            .validate_param(&serde_json::json!(20000), &constraints)
            .is_err());
    }
}
