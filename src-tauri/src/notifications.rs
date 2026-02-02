//! In-app notifications queue (local-only)

use serde::{Deserialize, Serialize};
use std::sync::RwLock;
use tauri_plugin_notification::NotificationExt;

const MAX_NOTIFICATIONS: usize = 100;
const NOTIFICATION_SETTINGS_FILE: &str = "notification_settings.json";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NotificationEntry {
    pub id: String,
    pub timestamp: u64,
    pub title: String,
    pub body: String,
    pub level: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NotificationSettings {
    pub system_enabled: bool,
}

impl Default for NotificationSettings {
    fn default() -> Self {
        Self {
            system_enabled: true,
        }
    }
}

fn settings_path() -> std::path::PathBuf {
    let mut path = dirs::config_dir().unwrap_or_else(|| std::path::PathBuf::from("."));
    path.push("os-ghost");
    path.push(NOTIFICATION_SETTINGS_FILE);
    path
}

#[tauri::command]
pub fn get_notification_settings() -> NotificationSettings {
    let path = settings_path();
    if path.exists() {
        if let Ok(contents) = std::fs::read_to_string(&path) {
            if let Ok(settings) = serde_json::from_str::<NotificationSettings>(&contents) {
                return settings;
            }
        }
    }
    NotificationSettings::default()
}

#[tauri::command]
pub fn set_notification_settings(system_enabled: bool) -> NotificationSettings {
    let settings = NotificationSettings { system_enabled };
    let path = settings_path();
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    if let Ok(contents) = serde_json::to_string_pretty(&settings) {
        let _ = std::fs::write(&path, contents);
    }
    settings
}

lazy_static::lazy_static! {
    static ref NOTIFICATIONS: RwLock<Vec<NotificationEntry>> = RwLock::new(Vec::new());
}

pub fn push_notification_internal(title: String, body: String, level: String) {
    let entry = NotificationEntry {
        id: format!("note_{}", crate::utils::current_timestamp()),
        timestamp: crate::utils::current_timestamp(),
        title,
        body,
        level,
    };

    if let Ok(mut list) = NOTIFICATIONS.write() {
        list.push(entry);
        if list.len() > MAX_NOTIFICATIONS {
            let start = list.len().saturating_sub(MAX_NOTIFICATIONS);
            *list = list.split_off(start);
        }
    }
}

#[tauri::command]
pub fn push_notification(
    app: tauri::AppHandle,
    title: String,
    body: String,
    level: Option<String>,
) {
    let level = level.unwrap_or_else(|| "info".to_string());
    push_notification_internal(title.clone(), body.clone(), level.clone());
    if get_notification_settings().system_enabled {
        let _ = app.notification().builder().title(title).body(body).show();
    }
}

#[tauri::command]
pub fn list_notifications(limit: Option<usize>) -> Vec<NotificationEntry> {
    let limit = limit.unwrap_or(20).min(100);
    if let Ok(list) = NOTIFICATIONS.read() {
        let mut items = list.clone();
        items.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));
        return items.into_iter().take(limit).collect();
    }
    Vec::new()
}

#[tauri::command]
pub fn clear_notifications() -> bool {
    if let Ok(mut list) = NOTIFICATIONS.write() {
        list.clear();
        return true;
    }
    false
}
