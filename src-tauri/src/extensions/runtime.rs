//! Minimal extension runtime with hot-reload support

use notify::{RecommendedWatcher, RecursiveMode, Watcher};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::RwLock;
use tokio::process::Command;
use tokio::time::{timeout, Duration};

const EXTENSIONS_DIR: &str = "extensions";
use std::sync::atomic::{AtomicBool, Ordering};
static WATCH_STARTED: AtomicBool = AtomicBool::new(false);
static EXTENSION_CACHE: RwLock<Vec<ExtensionStatus>> = RwLock::new(Vec::new());

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtensionManifest {
    pub id: String,
    pub name: String,
    pub version: String,
    pub entry: String,
    #[serde(default)]
    pub tools: Vec<ExtensionTool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtensionTool {
    pub name: String,
    pub description: String,
    pub command: String,
    #[serde(default)]
    pub args_schema: Option<serde_json::Value>,
    #[serde(default)]
    pub requires_approval: bool,
    #[serde(default)]
    pub approval_reason: Option<String>,
    #[serde(default)]
    pub risk_level: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtensionStatus {
    pub id: String,
    pub name: String,
    pub version: String,
    pub loaded: bool,
    pub last_error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtensionRunResult {
    pub exit_code: Option<i32>,
    pub stdout: String,
    pub stderr: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtensionToolListing {
    pub extension_id: String,
    pub tools: Vec<ExtensionTool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtensionToolRunResult {
    pub exit_code: Option<i32>,
    pub stdout: String,
    pub stderr: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtensionToolRequestResult {
    pub success: bool,
    pub action_id: Option<u64>,
    pub preview_id: Option<String>,
    pub error: Option<String>,
}

fn extensions_root() -> PathBuf {
    let mut path = dirs::config_dir().unwrap_or_else(|| PathBuf::from("."));
    path.push("os-ghost");
    path.push(EXTENSIONS_DIR);
    path
}

fn manifest_path(dir: &Path) -> PathBuf {
    dir.join("extension.json")
}

fn read_manifest(dir: &Path) -> Result<ExtensionManifest, String> {
    let path = manifest_path(dir);
    let contents = fs::read_to_string(&path).map_err(|e| e.to_string())?;
    serde_json::from_str(&contents).map_err(|e| e.to_string())
}

fn find_extension_dir(id: &str) -> Option<PathBuf> {
    let dirs = list_extension_dirs();
    for dir in dirs {
        if let Ok(manifest) = read_manifest(&dir) {
            if manifest.id == id {
                return Some(dir);
            }
        }
    }
    None
}

fn list_extension_dirs() -> Vec<PathBuf> {
    let root = extensions_root();
    let Ok(entries) = fs::read_dir(&root) else {
        return vec![];
    };
    entries
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.path())
        .filter(|path| path.is_dir())
        .collect()
}

#[tauri::command]
pub fn list_extensions() -> Vec<ExtensionStatus> {
    ensure_watcher();
    if let Ok(cache) = EXTENSION_CACHE.read() {
        return cache.clone();
    }
    vec![]
}

#[tauri::command]
pub fn reload_extensions() -> Vec<ExtensionStatus> {
    let statuses = load_extensions();
    if let Ok(mut cache) = EXTENSION_CACHE.write() {
        *cache = statuses.clone();
    }
    statuses
}

#[tauri::command]
pub fn list_extension_tools() -> Vec<ExtensionToolListing> {
    let dirs = list_extension_dirs();
    let mut listings = Vec::new();
    for dir in dirs {
        if let Ok(manifest) = read_manifest(&dir) {
            if !manifest.tools.is_empty() {
                listings.push(ExtensionToolListing {
                    extension_id: manifest.id,
                    tools: manifest.tools,
                });
            }
        }
    }
    listings
}

pub async fn execute_extension_tool_internal(
    extension_id: String,
    tool_name: String,
    args: Option<Vec<String>>,
) -> Result<ExtensionToolRunResult, String> {
    let privacy = crate::config::privacy::PrivacySettings::load();
    if privacy.read_only_mode || privacy.trust_profile != "open" {
        return Err("Extension tool execution blocked by trust profile".to_string());
    }

    let dir = find_extension_dir(&extension_id).ok_or_else(|| "Extension not found".to_string())?;
    let manifest = read_manifest(&dir)?;
    let tool = manifest
        .tools
        .iter()
        .find(|t| t.name == tool_name)
        .ok_or_else(|| "Tool not found".to_string())?;

    // Cross-platform shell execution
    let mut command = if cfg!(target_os = "windows") {
        let mut cmd = Command::new("cmd");
        cmd.arg("/C").arg(&tool.command);
        cmd
    } else {
        let mut cmd = Command::new("sh");
        cmd.arg("-c").arg(&tool.command);
        cmd
    };
    if let Some(extra) = args {
        command.args(extra);
    }

    let output = timeout(Duration::from_secs(30), command.output())
        .await
        .map_err(|_| "Extension tool timed out".to_string())?
        .map_err(|e| e.to_string())?;

    let result = ExtensionToolRunResult {
        exit_code: output.status.code(),
        stdout: String::from_utf8_lossy(&output.stdout).to_string(),
        stderr: String::from_utf8_lossy(&output.stderr).to_string(),
    };

    crate::data::events_bus::record_event(
        crate::data::events_bus::EventKind::Action,
        format!("Extension tool executed: {}", tool.name),
        None,
        std::collections::HashMap::new(),
        crate::data::events_bus::EventPriority::Normal,
        Some(format!("extension_tool:{}", tool.name)),
        Some(600),
        Some("extensions".to_string()),
    );

    Ok(result)
}

#[tauri::command]
pub async fn execute_extension_tool(
    extension_id: String,
    tool_name: String,
    args: Option<Vec<String>>,
) -> Result<ExtensionToolRunResult, String> {
    execute_extension_tool_internal(extension_id, tool_name, args).await
}

#[tauri::command]
pub fn request_extension_tool_action(
    extension_id: String,
    tool_name: String,
    args: Option<Vec<String>>,
) -> ExtensionToolRequestResult {
    let dir = match find_extension_dir(&extension_id) {
        Some(dir) => dir,
        None => {
            return ExtensionToolRequestResult {
                success: false,
                action_id: None,
                preview_id: None,
                error: Some("Extension not found".to_string()),
            }
        }
    };
    let manifest = match read_manifest(&dir) {
        Ok(manifest) => manifest,
        Err(err) => {
            return ExtensionToolRequestResult {
                success: false,
                action_id: None,
                preview_id: None,
                error: Some(err),
            }
        }
    };
    let tool = match manifest.tools.iter().find(|t| t.name == tool_name) {
        Some(tool) => tool,
        None => {
            return ExtensionToolRequestResult {
                success: false,
                action_id: None,
                preview_id: None,
                error: Some("Tool not found".to_string()),
            }
        }
    };

    let risk_level = match tool.risk_level.as_deref() {
        Some("low") => crate::actions::ActionRiskLevel::Low,
        Some("high") => crate::actions::ActionRiskLevel::High,
        _ => {
            if tool.requires_approval {
                crate::actions::ActionRiskLevel::High
            } else {
                crate::actions::ActionRiskLevel::Medium
            }
        }
    };

    let pending = crate::actions::PendingAction::new(
        "extension.tool".to_string(),
        format!("Run extension tool: {}", tool.name),
        format!("{}::{}", extension_id, tool.name),
        risk_level,
        tool.approval_reason
            .clone()
            .or_else(|| Some("Extension tool requires approval".to_string())),
        Some(serde_json::json!({
            "extension_id": extension_id,
            "tool_name": tool.name,
            "args": args.unwrap_or_default(),
            "args_schema": tool.args_schema,
        })),
    );

    let preview_id =
        if let Some(manager) = crate::actions::action_preview::get_preview_manager_mut() {
            let preview = manager.start_preview(&pending);
            manager.set_visual_preview(
                &preview.id,
                crate::actions::action_preview::VisualPreview {
                    preview_type: crate::actions::action_preview::VisualPreviewType::TextSelection,
                    content: pending.target.clone(),
                    width: None,
                    height: None,
                    alt_text: format!("Extension tool {}", pending.target),
                },
            );
            manager.update_progress(&preview.id, 1.0);
            Some(preview.id)
        } else {
            None
        };

    let action_id = crate::actions::ACTION_QUEUE.add(pending.clone());
    crate::actions::action_ledger::record_action_created(
        action_id,
        pending.action_type,
        pending.description,
        pending.target,
        "high".to_string(),
        pending.reason,
        pending.arguments,
        Some("extensions".to_string()),
    );

    ExtensionToolRequestResult {
        success: true,
        action_id: Some(action_id),
        preview_id,
        error: None,
    }
}

#[tauri::command]
pub async fn execute_extension(
    id: String,
    args: Option<Vec<String>>,
) -> Result<ExtensionRunResult, String> {
    let privacy = crate::config::privacy::PrivacySettings::load();
    if privacy.read_only_mode || privacy.trust_profile != "open" {
        return Err("Extension execution blocked by trust profile".to_string());
    }

    let dir = find_extension_dir(&id).ok_or_else(|| "Extension not found".to_string())?;
    let manifest = read_manifest(&dir)?;
    let entry_path = dir.join(&manifest.entry);
    if !entry_path.exists() {
        return Err("Extension entry not found".to_string());
    }

    let args = args.unwrap_or_default();
    let entry_str = entry_path.to_string_lossy().to_string();

    let mut command = if entry_str.ends_with(".sh") {
        if cfg!(target_os = "windows") {
            return Err("Shell scripts are not supported on Windows. Use .bat, .cmd, or .ps1 files instead.".to_string());
        }
        let mut cmd = Command::new("sh");
        cmd.arg(entry_str);
        cmd
    } else if entry_str.ends_with(".js") {
        let mut cmd = Command::new("node");
        cmd.arg(entry_str);
        cmd
    } else {
        Command::new(entry_str)
    };

    command.args(args);

    let output = timeout(Duration::from_secs(30), command.output())
        .await
        .map_err(|_| "Extension timed out".to_string())?
        .map_err(|e| e.to_string())?;

    let result = ExtensionRunResult {
        exit_code: output.status.code(),
        stdout: String::from_utf8_lossy(&output.stdout).to_string(),
        stderr: String::from_utf8_lossy(&output.stderr).to_string(),
    };

    crate::data::events_bus::record_event(
        crate::data::events_bus::EventKind::Action,
        format!("Extension executed: {}", manifest.name),
        None,
        std::collections::HashMap::new(),
        crate::data::events_bus::EventPriority::Normal,
        Some(format!("extension_run:{}", manifest.id)),
        Some(600),
        Some("extensions".to_string()),
    );

    Ok(result)
}

fn ensure_watcher() {
    if WATCH_STARTED.load(Ordering::SeqCst) {
        return;
    }
    WATCH_STARTED.store(true, Ordering::SeqCst);

    let root = extensions_root();
    let _ = fs::create_dir_all(&root);

    let _ = load_extensions();

    std::thread::spawn(move || {
        let (tx, rx) = std::sync::mpsc::channel();
        let mut watcher: RecommendedWatcher = match notify::recommended_watcher(tx) {
            Ok(w) => w,
            Err(_) => return,
        };

        let _ = watcher.watch(&root, RecursiveMode::Recursive);

        loop {
            let _ = rx.recv();
            let statuses = load_extensions();
            if let Ok(mut cache) = EXTENSION_CACHE.write() {
                *cache = statuses;
            }
        }
    });
}

fn load_extensions() -> Vec<ExtensionStatus> {
    let dirs = list_extension_dirs();
    let mut results = Vec::new();
    for dir in dirs {
        match read_manifest(&dir) {
            Ok(manifest) => {
                let entry_path = dir.join(&manifest.entry);
                if !entry_path.exists() {
                    let error = format!("Entry not found: {}", manifest.entry);
                    crate::data::events_bus::record_event(
                        crate::data::events_bus::EventKind::Guardrail,
                        format!("Extension load failed: {}", manifest.name),
                        Some(error.clone()),
                        std::collections::HashMap::new(),
                        crate::data::events_bus::EventPriority::Normal,
                        Some(format!("extension_error:{}", manifest.id)),
                        Some(600),
                        Some("extensions".to_string()),
                    );
                    results.push(ExtensionStatus {
                        id: manifest.id,
                        name: manifest.name,
                        version: manifest.version,
                        loaded: false,
                        last_error: Some(error),
                    });
                    continue;
                }
                results.push(ExtensionStatus {
                    id: manifest.id,
                    name: manifest.name,
                    version: manifest.version,
                    loaded: true,
                    last_error: None,
                });
            }
            Err(err) => {
                crate::data::events_bus::record_event(
                    crate::data::events_bus::EventKind::Guardrail,
                    "Extension manifest invalid",
                    Some(err.clone()),
                    std::collections::HashMap::new(),
                    crate::data::events_bus::EventPriority::Normal,
                    Some(format!(
                        "extension_error:{}",
                        dir.file_name().unwrap_or_default().to_string_lossy()
                    )),
                    Some(600),
                    Some("extensions".to_string()),
                );
                results.push(ExtensionStatus {
                    id: dir
                        .file_name()
                        .unwrap_or_default()
                        .to_string_lossy()
                        .to_string(),
                    name: "Invalid extension".to_string(),
                    version: "unknown".to_string(),
                    loaded: false,
                    last_error: Some(err),
                });
            }
        }
    }
    results
}
