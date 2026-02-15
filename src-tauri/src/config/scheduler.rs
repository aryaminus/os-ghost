//! Lightweight scheduler for companion routines
//!
//! Enhanced with Moltis-inspired cron-based task scheduling.

use crate::data::events_bus::{record_event, EventKind, EventPriority};
use crate::data::timeline::{record_timeline_event, TimelineEntryType, TimelineStatus};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use std::sync::{Arc, RwLock};
use tauri::{AppHandle, Emitter, Manager};

const SCHEDULER_SETTINGS_FILE: &str = "scheduler_settings.json";

/// Scheduled task definition (Moltis-inspired)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScheduledTask {
    pub id: String,
    pub name: String,
    pub cron_expression: String,
    pub command: String,
    pub enabled: bool,
    pub last_run: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SchedulerSettings {
    pub daily_brief_enabled: bool,
    pub idle_suggestions_enabled: bool,
    pub focus_summary_enabled: bool,
    pub quiet_hours_enabled: bool,
    pub quiet_hours_start: String,
    pub quiet_hours_end: String,
    #[serde(default)]
    pub custom_tasks: Vec<ScheduledTask>,
}

impl Default for SchedulerSettings {
    fn default() -> Self {
        Self {
            daily_brief_enabled: true,
            idle_suggestions_enabled: true,
            focus_summary_enabled: false,
            quiet_hours_enabled: true,
            quiet_hours_start: "22:00".to_string(),
            quiet_hours_end: "07:00".to_string(),
            custom_tasks: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct SchedulerState {
    pub last_daily_brief_day: Option<String>,
    pub last_idle_suggestion_at: Option<u64>,
}

impl SchedulerSettings {
    fn settings_path() -> PathBuf {
        let mut path = dirs::config_dir().unwrap_or_else(|| PathBuf::from("."));
        path.push("os-ghost");
        path.push(SCHEDULER_SETTINGS_FILE);
        path
    }

    pub fn load() -> Self {
        let path = Self::settings_path();
        if path.exists() {
            if let Ok(contents) = fs::read_to_string(&path) {
                if let Ok(settings) = serde_json::from_str(&contents) {
                    return settings;
                }
            }
        }
        Self::default()
    }

    pub fn save(&self) -> anyhow::Result<()> {
        let path = Self::settings_path();
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let contents = serde_json::to_string_pretty(self)?;
        fs::write(&path, contents)?;
        Ok(())
    }
}

#[tauri::command]
pub fn get_scheduler_settings() -> SchedulerSettings {
    SchedulerSettings::load()
}

#[tauri::command]
pub fn update_scheduler_settings(
    daily_brief_enabled: bool,
    idle_suggestions_enabled: bool,
    focus_summary_enabled: bool,
    quiet_hours_enabled: bool,
    quiet_hours_start: String,
    quiet_hours_end: String,
) -> Result<SchedulerSettings, String> {
    let mut settings = SchedulerSettings::load();
    settings.daily_brief_enabled = daily_brief_enabled;
    settings.idle_suggestions_enabled = idle_suggestions_enabled;
    settings.focus_summary_enabled = focus_summary_enabled;
    settings.quiet_hours_enabled = quiet_hours_enabled;
    settings.quiet_hours_start = quiet_hours_start;
    settings.quiet_hours_end = quiet_hours_end;
    settings.save().map_err(|e| e.to_string())?;
    Ok(settings)
}

// ============================================================================
// Custom Scheduled Tasks (Moltis-inspired)
// ============================================================================

#[tauri::command]
pub fn get_scheduled_tasks() -> Vec<ScheduledTask> {
    SchedulerSettings::load().custom_tasks
}

#[tauri::command]
pub fn add_scheduled_task(
    name: String,
    cron_expression: String,
    command: String,
) -> Result<SchedulerSettings, String> {
    let mut settings = SchedulerSettings::load();
    
    let task = ScheduledTask {
        id: uuid::Uuid::new_v4().to_string(),
        name,
        cron_expression,
        command,
        enabled: true,
        last_run: None,
    };
    
    settings.custom_tasks.push(task);
    settings.save().map_err(|e| e.to_string())?;
    Ok(settings)
}

#[tauri::command]
pub fn remove_scheduled_task(task_id: String) -> Result<SchedulerSettings, String> {
    let mut settings = SchedulerSettings::load();
    settings.custom_tasks.retain(|t| t.id != task_id);
    settings.save().map_err(|e| e.to_string())?;
    Ok(settings)
}

#[tauri::command]
pub fn toggle_scheduled_task(task_id: String, enabled: bool) -> Result<SchedulerSettings, String> {
    let mut settings = SchedulerSettings::load();
    if let Some(task) = settings.custom_tasks.iter_mut().find(|t| t.id == task_id) {
        task.enabled = enabled;
    }
    settings.save().map_err(|e| e.to_string())?;
    Ok(settings)
}

pub fn start_scheduler_loop(app: AppHandle, state: Arc<RwLock<SchedulerState>>) {
    tauri::async_runtime::spawn(async move {
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(60));

        loop {
            interval.tick().await;

            let settings = SchedulerSettings::load();
            if settings.quiet_hours_enabled && is_quiet_hours(&settings) {
                continue;
            }

            if settings.daily_brief_enabled {
                handle_daily_brief(&app, &state).ok();
            }

            if settings.idle_suggestions_enabled {
                handle_idle_suggestion(&app, &state).ok();
            }
        }
    });
}

fn is_quiet_hours(settings: &SchedulerSettings) -> bool {
    let now = chrono::Local::now();
    let current = now.format("%H:%M").to_string();
    let start = settings.quiet_hours_start.as_str();
    let end = settings.quiet_hours_end.as_str();
    let current = current.as_str();

    if start <= end {
        current >= start && current <= end
    } else {
        current >= start || current <= end
    }
}

fn handle_daily_brief(app: &AppHandle, state: &Arc<RwLock<SchedulerState>>) -> anyhow::Result<()> {
    let today = chrono::Local::now().format("%Y-%m-%d").to_string();
    let hour = chrono::Local::now().hour();

    if !(8..=11).contains(&hour) {
        return Ok(());
    }

    let mut guard = state.write().map_err(|e| anyhow::anyhow!(e.to_string()))?;
    if guard.last_daily_brief_day.as_deref() == Some(&today) {
        return Ok(());
    }

    guard.last_daily_brief_day = Some(today);

    let payload = serde_json::json!({
        "behavior_type": "routine",
        "trigger_context": "daily_brief",
        "suggestion": "Daily brief ready. Want a quick recap of your recent activity?",
        "urgency": 0.2
    });

    let _ = app.emit("companion_behavior", payload);
    record_timeline_event(
        "Daily brief queued",
        Some("Scheduled routine".to_string()),
        TimelineEntryType::Routine,
        TimelineStatus::Pending,
    );

    record_event(
        EventKind::Routine,
        "Daily brief queued",
        Some("Scheduled routine".to_string()),
        std::collections::HashMap::new(),
        EventPriority::Low,
        Some("routine:daily_brief".to_string()),
        Some(3600),
        Some("scheduler".to_string()),
    );

    Ok(())
}

fn handle_idle_suggestion(
    app: &AppHandle,
    state: &Arc<RwLock<SchedulerState>>,
) -> anyhow::Result<()> {
    let session = app.state::<Arc<crate::memory::SessionMemory>>();
    let now = crate::core::utils::current_timestamp();

    let last_activity = session.load().map(|s| s.last_activity).unwrap_or(0);
    if now.saturating_sub(last_activity) < 20 * 60 {
        return Ok(());
    }

    let mut guard = state.write().map_err(|e| anyhow::anyhow!(e.to_string()))?;
    if let Some(last) = guard.last_idle_suggestion_at {
        if now.saturating_sub(last) < 60 * 60 {
            return Ok(());
        }
    }

    guard.last_idle_suggestion_at = Some(now);

    let payload = serde_json::json!({
        "behavior_type": "idle",
        "trigger_context": "idle_time",
        "suggestion": "Quiet moment detected. Want me to set up something useful before you return?",
        "urgency": 0.15
    });

    let _ = app.emit("companion_behavior", payload);
    record_timeline_event(
        "Idle suggestion queued",
        Some("User idle for 20+ minutes".to_string()),
        TimelineEntryType::Routine,
        TimelineStatus::Pending,
    );

    record_event(
        EventKind::Routine,
        "Idle suggestion queued",
        Some("User idle for 20+ minutes".to_string()),
        std::collections::HashMap::new(),
        EventPriority::Low,
        Some("routine:idle_suggestion".to_string()),
        Some(3600),
        Some("scheduler".to_string()),
    );

    Ok(())
}

use chrono::Timelike;
