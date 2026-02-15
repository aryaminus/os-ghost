//! Workspace Context Files - Moltis-inspired context file support
//!
//! Supports loading and injecting context from workspace files:
//! - TOOLS.md: Tool notes and policies injected into system prompt
//! - AGENTS.md: Workspace-level agent instructions
//! - BOOT.md: Tasks to execute on startup
//!
//! These files allow users to customize agent behavior without code changes.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::RwLock;

lazy_static::lazy_static! {
    static ref WORKSPACE_CONTEXT: RwLock<WorkspaceContext> = RwLock::new(WorkspaceContext::default());
}

const TOOLS_MD: &str = "TOOLS.md";
const AGENTS_MD: &str = "AGENTS.md";
const BOOT_MD: &str = "BOOT.md";

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct WorkspaceContext {
    pub tools_md: Option<String>,
    pub agents_md: Option<String>,
    pub boot_md: Option<String>,
}

impl WorkspaceContext {
    pub fn load(data_dir: &PathBuf) -> Self {
        let mut context = WorkspaceContext::default();

        // Load TOOLS.md
        let tools_path = data_dir.join(TOOLS_MD);
        if tools_path.exists() {
            if let Ok(content) = std::fs::read_to_string(&tools_path) {
                if !content.trim().is_empty() {
                    context.tools_md = Some(content);
                }
            }
        }

        // Load AGENTS.md
        let agents_path = data_dir.join(AGENTS_MD);
        if agents_path.exists() {
            if let Ok(content) = std::fs::read_to_string(&agents_path) {
                if !content.trim().is_empty() {
                    context.agents_md = Some(content);
                }
            }
        }

        // Load BOOT.md
        let boot_path = data_dir.join(BOOT_MD);
        if boot_path.exists() {
            if let Ok(content) = std::fs::read_to_string(&boot_path) {
                if !content.trim().is_empty() {
                    context.boot_md = Some(content);
                }
            }
        }

        tracing::info!(
            "Loaded workspace context: tools={}, agents={}, boot={}",
            context.tools_md.is_some(),
            context.agents_md.is_some(),
            context.boot_md.is_some()
        );

        context
    }

    pub fn get_tools_for_prompt(&self) -> String {
        self.tools_md
            .as_ref()
            .map(|c| format!("\n\n## Workspace Tools\n{}\n", c))
            .unwrap_or_default()
    }

    pub fn get_agents_for_prompt(&self) -> String {
        self.agents_md
            .as_ref()
            .map(|c| format!("\n\n## Agent Instructions\n{}\n", c))
            .unwrap_or_default()
    }
}

fn get_data_dir() -> PathBuf {
    dirs::data_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("os-ghost")
}

pub fn init_workspace_context() {
    let data_dir = get_data_dir();
    let context = WorkspaceContext::load(&data_dir);
    if let Ok(mut ctx) = WORKSPACE_CONTEXT.write() {
        *ctx = context;
    }
}

pub fn reload_workspace_context() {
    init_workspace_context();
}

pub fn get_workspace_context() -> WorkspaceContext {
    WORKSPACE_CONTEXT
        .read()
        .map(|c| c.clone())
        .unwrap_or_default()
}

pub fn get_tools_context() -> String {
    get_workspace_context().get_tools_for_prompt()
}

pub fn get_agents_context() -> String {
    get_workspace_context().get_agents_for_prompt()
}

pub fn get_boot_tasks() -> Option<String> {
    get_workspace_context().boot_md
}

pub fn inject_into_system_prompt(prompt: &str) -> String {
    let context = get_workspace_context();
    let mut result = prompt.to_string();

    if let Some(tools) = &context.tools_md {
        result.push_str("\n\n## Workspace Tools\n");
        result.push_str(tools);
    }

    if let Some(agents) = &context.agents_md {
        result.push_str("\n\n## Agent Instructions\n");
        result.push_str(agents);
    }

    result
}

// ============================================================================
// Tauri Commands
// ============================================================================

#[tauri::command]
pub fn get_workspace_context_files() -> WorkspaceContext {
    get_workspace_context()
}

#[tauri::command]
pub fn reload_workspace_context_cmd() {
    reload_workspace_context();
}

#[tauri::command]
pub fn get_tools_md_path() -> String {
    get_data_dir().join(TOOLS_MD).to_string_lossy().to_string()
}

#[tauri::command]
pub fn get_agents_md_path() -> String {
    get_data_dir().join(AGENTS_MD).to_string_lossy().to_string()
}

#[tauri::command]
pub fn get_boot_md_path() -> String {
    get_data_dir().join(BOOT_MD).to_string_lossy().to_string()
}
