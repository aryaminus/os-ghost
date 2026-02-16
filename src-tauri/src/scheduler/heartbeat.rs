//! Heartbeat/Scheduler System
//! 
//! Reference: ZeroClaw heartbeat engine
//! 
//! Provides periodic task execution for autonomous agents

use serde::{Deserialize, Serialize};
use std::sync::RwLock;
use tokio::time::{interval, Duration};
use tokio::sync::mpsc;

lazy_static::lazy_static! {
    static ref HEARTBEAT_STATE: RwLock<HeartbeatState> = RwLock::new(HeartbeatState::default());
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct HeartbeatState {
    pub running: bool,
    pub tasks_executed: u64,
    pub last_task_time: Option<u64>,
    pub errors: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HeartbeatTask {
    pub id: String,
    pub command: String,
    pub description: Option<String>,
    pub enabled: bool,
}

impl HeartbeatTask {
    pub fn from_line(line: &str) -> Option<Self> {
        let trimmed = line.trim();
        if trimmed.is_empty() || !trimmed.starts_with('-') {
            return None;
        }
        
        let content = trimmed.strip_prefix("- ").unwrap_or(trimmed);
        
        Some(Self {
            id: uuid::Uuid::new_v4().to_string(),
            command: content.to_string(),
            description: None,
            enabled: true,
        })
    }
}

pub struct HeartbeatEngine {
    interval_mins: u32,
    workspace_dir: Option<String>,
}

impl HeartbeatEngine {
    pub fn new(interval_mins: u32) -> Self {
        Self {
            interval_mins,
            workspace_dir: None,
        }
    }
    
    pub fn with_workspace(mut self, dir: &str) -> Self {
        self.workspace_dir = Some(dir.to_string());
        self
    }
    
    pub async fn run(&self, tx: mpsc::Sender<HeartbeatEvent>) {
        let mut interval = interval(Duration::from_secs(u64::from(self.interval_mins) * 60));
        
        tracing::info!("Heartbeat engine started with {} minute interval", self.interval_mins);
        
        {
            if let Ok(mut state) = HEARTBEAT_STATE.write() {
                state.running = true;
            }
        }
        
        loop {
            interval.tick().await;
            
            if let Err(e) = self.tick(&tx).await {
                tracing::error!("Heartbeat tick error: {}", e);
                
                if let Ok(mut state) = HEARTBEAT_STATE.write() {
                    state.errors.push(e.to_string());
                    if state.errors.len() > 10 {
                        state.errors.remove(0);
                    }
                }
            }
        }
    }
    
    async fn tick(&self, tx: &mpsc::Sender<HeartbeatEvent>) -> Result<(), String> {
        let _ = tx.send(HeartbeatEvent::Tick).await;
        
        // Load tasks from HEARTBEAT.md if exists
        if let Some(ref dir) = self.workspace_dir {
            let tasks_file = std::path::Path::new(dir).join("HEARTBEAT.md");
            if tasks_file.exists() {
                // Use spawn_blocking to avoid blocking the async runtime
                let tasks_file_path = tasks_file.clone();
                let content = tokio::task::spawn_blocking(move || {
                    std::fs::read_to_string(&tasks_file_path)
                })
                .await
                .map_err(|e| format!("Task join error: {}", e))?
                .map_err(|e| format!("Failed to read HEARTBEAT.md: {}", e))?;
                
                let tasks = Self::parse_tasks(&content);
                
                for task in tasks {
                    if task.enabled {
                        let _ = tx.send(HeartbeatEvent::TaskExecuting(task.clone())).await;
                        
                        // Execute the task (shell command)
                        let result = self.execute_task(&task).await;
                        
                        match result {
                            Ok(output) => {
                                let _ = tx.send(HeartbeatEvent::TaskCompleted(task, output)).await;
                            }
                            Err(e) => {
                                let _ = tx.send(HeartbeatEvent::TaskFailed(task, e)).await;
                            }
                        }
                    }
                }
            }
        }
        
        // Update state
        {
            if let Ok(mut state) = HEARTBEAT_STATE.write() {
                state.tasks_executed += 1;
                state.last_task_time = Some(
                    std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .map(|d| d.as_secs())
                        .unwrap_or(0)
                );
            }
        }
        
        Ok(())
    }
    
    fn parse_tasks(content: &str) -> Vec<HeartbeatTask> {
        content.lines()
            .filter_map(HeartbeatTask::from_line)
            .collect()
    }
    
    async fn execute_task(&self, task: &HeartbeatTask) -> Result<String, String> {
        let command = &task.command;
        
        // Check if command is blocked by security policy
        if crate::mcp::sandbox::is_command_blocked(command) {
            return Err(format!("Command blocked by security policy: {}", command));
        }
        
        let output = tokio::process::Command::new("sh")
            .args(["-c", command])
            .output()
            .await
            .map_err(|e| format!("Failed to execute command: {}", e))?;
        
        if output.status.success() {
            Ok(String::from_utf8_lossy(&output.stdout).to_string())
        } else {
            Err(String::from_utf8_lossy(&output.stderr).to_string())
        }
    }
    
    pub fn stop() {
        if let Ok(mut state) = HEARTBEAT_STATE.write() {
            state.running = false;
            tracing::info!("Heartbeat engine stopped");
        } else {
            tracing::error!("Failed to acquire write lock on heartbeat state");
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum HeartbeatEvent {
    Tick,
    TaskExecuting(HeartbeatTask),
    TaskCompleted(HeartbeatTask, String),
    TaskFailed(HeartbeatTask, String),
}

// ============================================================================
// Daemon Manager
// ============================================================================

pub struct DaemonManager {
    components: RwLock<Vec<DaemonComponent>>,
}

#[derive(Debug, Clone, Serialize)]
pub struct DaemonComponent {
    pub name: String,
    pub running: bool,
    pub restart_count: u32,
    pub last_error: Option<String>,
}

impl DaemonManager {
    pub fn new() -> Self {
        Self {
            components: RwLock::new(Vec::new()),
        }
    }
    
    pub fn register_component(&self, name: &str) {
        if let Ok(mut components) = self.components.write() {
            components.push(DaemonComponent {
                name: name.to_string(),
                running: false,
                restart_count: 0,
                last_error: None,
            });
        }
    }
    
    pub fn set_running(&self, name: &str, running: bool) {
        if let Ok(mut components) = self.components.write() {
            if let Some(comp) = components.iter_mut().find(|c| c.name == name) {
                comp.running = running;
                if running {
                    comp.last_error = None;
                }
            }
        }
    }
    
    pub fn record_error(&self, name: &str, error: &str) {
        if let Ok(mut components) = self.components.write() {
            if let Some(comp) = components.iter_mut().find(|c| c.name == name) {
                comp.last_error = Some(error.to_string());
                comp.restart_count += 1;
            }
        }
    }
    
    pub fn status(&self) -> Vec<DaemonComponent> {
        self.components.read().map(|c| c.clone()).unwrap_or_default()
    }
    
    pub fn all_running(&self) -> bool {
        self.components.read()
            .map(|c| c.iter().all(|c| c.running))
            .unwrap_or(false)
    }
}

impl Default for DaemonManager {
    fn default() -> Self {
        Self::new()
    }
}

lazy_static::lazy_static! {
    static ref DAEMON_MANAGER: DaemonManager = DaemonManager::new();
}

// ============================================================================
// Tauri Commands
// ============================================================================

#[tauri::command]
pub fn get_heartbeat_state() -> HeartbeatState {
    HEARTBEAT_STATE.read().map(|s| s.clone()).unwrap_or_default()
}

#[tauri::command]
pub fn get_daemon_status() -> Vec<DaemonComponent> {
    DAEMON_MANAGER.status()
}

#[tauri::command]
pub fn is_daemon_healthy() -> bool {
    DAEMON_MANAGER.all_running()
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_parse_tasks() {
        let content = "- echo hello\n- curl http://example.com\n\ntest";
        let tasks = HeartbeatEngine::parse_tasks(content);
        assert_eq!(tasks.len(), 2);
        assert_eq!(tasks[0].command, "echo hello");
    }
    
    #[test]
    fn test_daemon_manager() {
        let manager = DaemonManager::new();
        manager.register_component("test");
        
        manager.set_running("test", true);
        assert!(manager.all_running());
        
        manager.set_running("test", false);
        assert!(!manager.all_running());
    }
}
