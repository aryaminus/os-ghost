//! Hook System - Moltis-inspired lifecycle hooks
//!
//! Provides sophisticated event hooks that allow external scripts to observe,
//! modify, or block agent behavior at key lifecycle points.
//!
//! Hook Types:
//! - Modifying events (sequential): BeforeToolCall, AfterToolCall, BeforeLLMCall, AfterLLMCall, BeforeCompaction
//! - Read-only events (parallel): SessionStart, SessionEnd, GatewayStart, GatewayStop, Command
//!
//! Shell Protocol:
//! - Input: JSON payload on stdin
//! - Exit 0 + empty = continue
//! - Exit 0 + {"action":"modify","data":{...}} = modify payload
//! - Exit 1 = block (stderr = reason)

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::RwLock;
use std::time::{Duration, Instant};

lazy_static::lazy_static! {
    static ref HOOK_REGISTRY: RwLock<HookRegistry> = RwLock::new(HookRegistry::default());
    static ref HOOK_STATE: RwLock<HookState> = RwLock::new(HookState::default());
}

// ============================================================================
// Hook Types
// ============================================================================

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HookEvent {
    BeforeAgentStart,
    BeforeLLMCall,
    AfterLLMCall,
    BeforeToolCall,
    AfterToolCall,
    BeforeCompaction,
    AfterCompaction,
    MessageReceived,
    MessageSending,
    MessageSent,
    SessionStart,
    SessionEnd,
    GatewayStart,
    GatewayStop,
    Command,
}

impl HookEvent {
    pub fn is_modifying(&self) -> bool {
        matches!(
            self,
            HookEvent::BeforeAgentStart
                | HookEvent::BeforeLLMCall
                | HookEvent::AfterLLMCall
                | HookEvent::BeforeToolCall
                | HookEvent::BeforeCompaction
                | HookEvent::MessageSending
        )
    }

    pub fn is_parallel(&self) -> bool {
        !self.is_modifying()
    }
}

// ============================================================================
// Hook Definition
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HookDefinition {
    pub name: String,
    pub description: String,
    pub command: String,
    pub args: Vec<String>,
    pub events: Vec<HookEvent>,
    pub timeout_secs: u64,
    pub priority: i32,
    pub enabled: bool,
    pub requires: HookRequirements,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct HookRequirements {
    pub os: Vec<String>,
    pub bins: Vec<String>,
    pub env: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HookConfig {
    pub name: String,
    pub command: String,
    pub args: Vec<String>,
    pub events: Vec<String>,
    pub timeout: Option<u64>,
    pub priority: Option<i32>,
    pub enabled: Option<bool>,
    pub requires: Option<HookRequirements>,
}

impl Default for HookConfig {
    fn default() -> Self {
        Self {
            name: String::new(),
            command: String::new(),
            args: Vec::new(),
            events: Vec::new(),
            timeout: Some(5),
            priority: Some(100),
            enabled: Some(true),
            requires: None,
        }
    }
}

// ============================================================================
// Hook Registry
// ============================================================================

#[derive(Debug, Default)]
pub struct HookRegistry {
    pub hooks: Vec<HookDefinition>,
}

impl HookRegistry {
    pub fn add_hook(&mut self, hook: HookDefinition) {
        self.hooks.push(hook);
        self.hooks.sort_by(|a, b| b.priority.cmp(&a.priority));
    }

    pub fn get_hooks_for_event(&self, event: HookEvent) -> Vec<&HookDefinition> {
        self.hooks
            .iter()
            .filter(|h| h.enabled && h.events.contains(&event))
            .collect()
    }
}

// ============================================================================
// Hook State (for circuit breaker)
// ============================================================================

#[derive(Debug, Default)]
pub struct HookState {
    pub failure_count: HashMap<String, u32>,
    pub last_failure: HashMap<String, Instant>,
    pub disabled_hooks: Vec<String>,
}

impl HookState {
    pub fn record_failure(&mut self, hook_name: &str) {
        let count = self.failure_count.entry(hook_name.to_string()).or_insert(0);
        *count += 1;
        self.last_failure
            .insert(hook_name.to_string(), Instant::now());

        if *count >= 5 {
            self.disabled_hooks.push(hook_name.to_string());
            tracing::warn!("Hook '{}' disabled after 5 consecutive failures", hook_name);
        }
    }

    pub fn record_success(&mut self, hook_name: &str) {
        self.failure_count.remove(hook_name);
        self.last_failure.remove(hook_name);
    }

    pub fn is_disabled(&self, hook_name: &str) -> bool {
        self.disabled_hooks.contains(&hook_name.to_string())
    }

    pub fn should_reenable(&self, hook_name: &str) -> bool {
        if let Some(last) = self.last_failure.get(hook_name) {
            last.elapsed() > Duration::from_secs(60)
        } else {
            false
        }
    }

    pub fn reenable(&mut self, hook_name: &str) {
        self.disabled_hooks.retain(|h| h != hook_name);
        self.failure_count.remove(hook_name);
        self.last_failure.remove(hook_name);
        tracing::info!("Hook '{}' re-enabled after cooldown", hook_name);
    }
}

// ============================================================================
// Hook Payload
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HookPayload {
    pub event: String,
    pub session_id: Option<String>,
    pub timestamp: String,
    #[serde(flatten)]
    pub data: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HookResult {
    pub action: HookAction,
    pub data: Option<serde_json::Value>,
    pub reason: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HookAction {
    Continue,
    Modify,
    Block,
}

// ============================================================================
// Hook Execution
// ============================================================================

pub fn execute_hook(hook: &HookDefinition, payload: &HookPayload) -> HookResult {
    if !hook.enabled {
        return HookResult {
            action: HookAction::Continue,
            data: None,
            reason: None,
        };
    }

    // Check eligibility
    if !is_hook_eligible(hook) {
        return HookResult {
            action: HookAction::Continue,
            data: None,
            reason: Some("Hook eligibility requirements not met".to_string()),
        };
    }

    // Check circuit breaker
    {
        let state = HOOK_STATE.read().unwrap();
        if state.is_disabled(&hook.name) {
            return HookResult {
                action: HookAction::Continue,
                data: None,
                reason: Some("Hook disabled due to repeated failures".to_string()),
            };
        }
    }

    // Serialize payload to JSON
    let _payload_json = match serde_json::to_string(payload) {
        Ok(j) => j,
        Err(e) => {
            tracing::error!("Failed to serialize hook payload: {}", e);
            return HookResult {
                action: HookAction::Continue,
                data: None,
                reason: Some(format!("Serialization error: {}", e)),
            };
        }
    };

    // Execute command synchronously
    let output = std::process::Command::new(&hook.command)
        .args(&hook.args)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .env("HOOK_EVENT", &payload.event)
        .env(
            "HOOK_SESSION_ID",
            payload.session_id.as_deref().unwrap_or(""),
        )
        .output();

    match output {
        Ok(output) => {
            let stdout = String::from_utf8_lossy(&output.stdout).to_string();
            let stderr = String::from_utf8_lossy(&output.stderr).to_string();

            if output.status.success() {
                if stdout.trim().is_empty() {
                    // Record success for circuit breaker
                    if let Ok(mut state) = HOOK_STATE.write() {
                        state.record_success(&hook.name);
                    }
                    HookResult {
                        action: HookAction::Continue,
                        data: None,
                        reason: None,
                    }
                } else {
                    // Try to parse as modification
                    if let Ok(result) = serde_json::from_str::<serde_json::Value>(&stdout) {
                        if result.get("action").and_then(|v| v.as_str()) == Some("modify") {
                            if let Some(data) = result.get("data") {
                                // Record success for circuit breaker
                                if let Ok(mut state) = HOOK_STATE.write() {
                                    state.record_success(&hook.name);
                                }
                                return HookResult {
                                    action: HookAction::Modify,
                                    data: Some(data.clone()),
                                    reason: None,
                                };
                            }
                        }
                    }
                    // Record success for circuit breaker
                    if let Ok(mut state) = HOOK_STATE.write() {
                        state.record_success(&hook.name);
                    }
                    HookResult {
                        action: HookAction::Continue,
                        data: None,
                        reason: None,
                    }
                }
            } else {
                // Failure - record for circuit breaker
                if let Ok(mut state) = HOOK_STATE.write() {
                    state.record_failure(&hook.name);
                }
                tracing::warn!("Hook '{}' failed: {}", hook.name, stderr);
                HookResult {
                    action: HookAction::Block,
                    data: None,
                    reason: Some(stderr),
                }
            }
        }
        Err(e) => {
            // Failure - record for circuit breaker
            if let Ok(mut state) = HOOK_STATE.write() {
                state.record_failure(&hook.name);
            }
            tracing::warn!("Hook '{}' error: {}", hook.name, e);
            HookResult {
                action: HookAction::Block,
                data: None,
                reason: Some(format!("Execution error: {}", e)),
            }
        }
    }
}

fn is_hook_eligible(hook: &HookDefinition) -> bool {
    let reqs = &hook.requires;

    // Check OS
    if !reqs.os.is_empty() {
        #[cfg(target_os = "macos")]
        if !reqs.os.contains(&"darwin".to_string()) {
            return false;
        }
        #[cfg(target_os = "linux")]
        if !reqs.os.contains(&"linux".to_string()) {
            return false;
        }
        #[cfg(target_os = "windows")]
        if !reqs.os.contains(&"windows".to_string()) {
            return false;
        }
    }

    // Check binaries
    for bin in &reqs.bins {
        if which::which(bin).is_err() {
            return false;
        }
    }

    // Check env vars
    for env_var in &reqs.env {
        if std::env::var(env_var).is_err() {
            return false;
        }
    }

    true
}

// ============================================================================
// Hook Discovery
// ============================================================================

fn get_hooks_dirs() -> Vec<PathBuf> {
    let mut dirs = Vec::new();

    // User global: ~/.os-ghost/hooks/
    if let Some(config_dir) = dirs::config_dir() {
        dirs.push(config_dir.join("os-ghost").join("hooks"));
    }

    // Workspace: ./os-ghost/hooks/
    if let Ok(cwd) = std::env::current_dir() {
        dirs.push(cwd.join(".os-ghost").join("hooks"));
    }

    dirs
}

pub fn discover_hooks() -> Vec<HookDefinition> {
    let mut hooks = Vec::new();

    for dir in get_hooks_dirs() {
        if !dir.exists() {
            continue;
        }

        if let Ok(entries) = std::fs::read_dir(&dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    let hook_md = path.join("HOOK.md");
                    if hook_md.exists() {
                        if let Ok(hook) = parse_hook_md(&path) {
                            hooks.push(hook);
                        }
                    }
                }
            }
        }
    }

    tracing::info!("Discovered {} hooks", hooks.len());
    hooks
}

fn parse_hook_md(dir: &Path) -> Result<HookDefinition, String> {
    let hook_md_path = dir.join("HOOK.md");
    let content = std::fs::read_to_string(&hook_md_path)
        .map_err(|e| format!("Failed to read HOOK.md: {}", e))?;

    // Parse TOML frontmatter
    let mut in_frontmatter = false;
    let mut frontmatter = String::new();

    for line in content.lines() {
        if line.trim() == "+++" {
            if in_frontmatter {
                break;
            } else {
                in_frontmatter = true;
                continue;
            }
        }
        if in_frontmatter {
            frontmatter.push_str(line);
            frontmatter.push('\n');
        }
    }

    // Parse TOML
    let config: HookConfig =
        toml::from_str(&frontmatter).map_err(|e| format!("Failed to parse HOOK.md: {}", e))?;

    let events: Vec<HookEvent> = config
        .events
        .iter()
        .filter_map(|s| serde_json::from_str(&format!("\"{}\"", s)).ok())
        .collect();

    let _name = dir
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("unnamed")
        .to_string();

    Ok(HookDefinition {
        name: config.name,
        description: String::new(),
        command: config.command,
        args: config.args,
        events,
        timeout_secs: config.timeout.unwrap_or(5),
        priority: config.priority.unwrap_or(100),
        enabled: config.enabled.unwrap_or(true),
        requires: config.requires.unwrap_or_default(),
    })
}

// ============================================================================
// Public API
// ============================================================================

pub fn init_hooks() {
    let hooks = discover_hooks();
    if let Ok(mut registry) = HOOK_REGISTRY.write() {
        *registry = HookRegistry { hooks };
    }
}

pub fn fire_event(
    event: HookEvent,
    session_id: Option<String>,
    data: serde_json::Value,
) -> (HookAction, Option<serde_json::Value>) {
    let event_name = match event {
        HookEvent::BeforeAgentStart => "before_agent_start",
        HookEvent::BeforeLLMCall => "before_llm_call",
        HookEvent::AfterLLMCall => "after_llm_call",
        HookEvent::BeforeToolCall => "before_tool_call",
        HookEvent::AfterToolCall => "after_tool_call",
        HookEvent::BeforeCompaction => "before_compaction",
        HookEvent::AfterCompaction => "after_compaction",
        HookEvent::MessageReceived => "message_received",
        HookEvent::MessageSending => "message_sending",
        HookEvent::MessageSent => "message_sent",
        HookEvent::SessionStart => "session_start",
        HookEvent::SessionEnd => "session_end",
        HookEvent::GatewayStart => "gateway_start",
        HookEvent::GatewayStop => "gateway_stop",
        HookEvent::Command => "command",
    };

    let payload = HookPayload {
        event: event_name.to_string(),
        session_id,
        timestamp: chrono::Utc::now().to_rfc3339(),
        data,
    };

    let hooks: Vec<HookDefinition> = {
        let registry = HOOK_REGISTRY.read().unwrap();
        registry
            .get_hooks_for_event(event)
            .iter()
            .map(|h| (*h).clone())
            .collect()
    };

    if event.is_modifying() {
        // Sequential execution for modifying events
        let mut current_data = payload.data.clone();

        for hook in hooks.clone() {
            let hook_payload = HookPayload {
                event: payload.event.clone(),
                session_id: payload.session_id.clone(),
                timestamp: payload.timestamp.clone(),
                data: current_data.clone(),
            };

            let result = execute_hook(&hook, &hook_payload);

            match result.action {
                HookAction::Block => {
                    tracing::info!("Hook '{}' blocked event: {:?}", hook.name, result.reason);
                    return (HookAction::Block, None);
                }
                HookAction::Modify => {
                    current_data = result.data.unwrap_or(current_data);
                }
                HookAction::Continue => {}
            }
        }

        (HookAction::Continue, Some(current_data))
    } else {
        // Parallel execution for read-only events
        for hook in hooks.clone() {
            let hook_payload = payload.clone();
            let _hook_name = hook.name.clone();

            // Fire and forget
            std::thread::spawn(move || {
                let _ = execute_hook(&hook, &hook_payload);
            });
        }

        (HookAction::Continue, None)
    }
}

pub fn list_hooks() -> Vec<HookDefinition> {
    HOOK_REGISTRY.read().unwrap().hooks.clone()
}

pub fn reload_hooks() {
    init_hooks();
}

pub fn enable_hook(name: &str) -> bool {
    if let Ok(mut registry) = HOOK_REGISTRY.write() {
        if let Some(hook) = registry.hooks.iter_mut().find(|h| h.name == name) {
            hook.enabled = true;
            return true;
        }
    }
    false
}

pub fn disable_hook(name: &str) -> bool {
    if let Ok(mut registry) = HOOK_REGISTRY.write() {
        if let Some(hook) = registry.hooks.iter_mut().find(|h| h.name == name) {
            hook.enabled = false;
            return true;
        }
    }
    false
}

// ============================================================================
// Tauri Commands
// ============================================================================

#[tauri::command]
pub fn get_hooks() -> Vec<HookDefinition> {
    list_hooks()
}

#[tauri::command]
pub fn reload_hooks_cmd() {
    reload_hooks();
}

#[tauri::command]
pub fn enable_hook_cmd(name: String) -> bool {
    enable_hook(&name)
}

#[tauri::command]
pub fn disable_hook_cmd(name: String) -> bool {
    disable_hook(&name)
}

#[tauri::command]
pub fn get_hook_state() -> HashMap<String, bool> {
    HOOK_STATE
        .read()
        .unwrap()
        .failure_count
        .iter()
        .map(|(k, v)| (k.clone(), *v >= 5))
        .collect()
}
