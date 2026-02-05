//! System-level settings stored on disk

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use std::str::FromStr;
use tauri::Manager;
use tauri_plugin_global_shortcut::GlobalShortcutExt;
use tauri_plugin_global_shortcut::Shortcut;

const SETTINGS_FILE: &str = "system_settings.json";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemSettings {
    #[serde(default)]
    pub monitor_enabled: bool,
    pub monitor_interval_secs: u64,
    pub monitor_idle_secs: u64,
    #[serde(default)]
    pub monitor_ignore_idle: bool,
    pub monitor_allow_hidden: bool,
    pub monitor_only_companion: bool,
    pub monitor_recent_activity_count: usize,
    pub monitor_idle_streak_threshold: usize,
    pub monitor_category_window: usize,
    pub global_shortcut_enabled: bool,
    pub global_shortcut: String,
    #[serde(default)]
    pub adaptive_capture_enabled: bool,
    pub adaptive_min_interval_secs: u64,
    pub adaptive_max_interval_secs: u64,
    pub adaptive_idle_threshold_secs: u64,
    pub adaptive_low_activity_threshold_secs: u64,
    pub adaptive_high_activity_count: usize,
    #[serde(default)]
    pub change_detection_enabled: bool,
    pub change_pixel_threshold: u8,
    pub change_min_changed_percentage: f32,
    pub change_max_changed_percentage: f32,
    /// Minimum seconds between AI analysis calls
    #[serde(default)]
    pub analysis_cooldown_secs: u64,
    /// Performance mode for resource management
    #[serde(default)]
    pub performance_mode: PerformanceMode,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PerformanceMode {
    Eco,
    Balanced,
    High,
}

impl Default for PerformanceMode {
    fn default() -> Self {
        Self::Balanced
    }
}

impl Default for SystemSettings {
    fn default() -> Self {
        Self {
            monitor_enabled: true,
            monitor_interval_secs: 60,
            monitor_idle_secs: 15 * 60,
            monitor_ignore_idle: false,
            monitor_allow_hidden: false,
            monitor_only_companion: true,
            monitor_recent_activity_count: 5,
            monitor_idle_streak_threshold: 3,
            monitor_category_window: 10,
            global_shortcut_enabled: true,
            global_shortcut: "CmdOrCtrl+Shift+G".to_string(),
            adaptive_capture_enabled: true,
            adaptive_min_interval_secs: 10,
            adaptive_max_interval_secs: 300,
            adaptive_idle_threshold_secs: 300,
            adaptive_low_activity_threshold_secs: 60,
            adaptive_high_activity_count: 20,
            change_detection_enabled: true,
            change_pixel_threshold: 30,
            change_min_changed_percentage: 0.01,
            change_max_changed_percentage: 0.95,
            analysis_cooldown_secs: 90,
            performance_mode: PerformanceMode::Balanced,
        }
    }
}

impl SystemSettings {
    fn settings_path() -> PathBuf {
        let mut path = dirs::config_dir().unwrap_or_else(|| PathBuf::from("."));
        path.push("os-ghost");
        path.push(SETTINGS_FILE);
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
pub fn get_system_settings() -> SystemSettings {
    SystemSettings::load()
}

#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub fn update_system_settings(
    monitor_enabled: bool,
    monitor_interval_secs: u64,
    monitor_idle_secs: u64,
    monitor_ignore_idle: bool,
    monitor_allow_hidden: bool,
    monitor_only_companion: bool,
    monitor_recent_activity_count: usize,
    monitor_idle_streak_threshold: usize,
    monitor_category_window: usize,
    global_shortcut_enabled: bool,
    adaptive_capture_enabled: bool,
    adaptive_min_interval_secs: u64,
    adaptive_max_interval_secs: u64,
    adaptive_idle_threshold_secs: u64,
    adaptive_low_activity_threshold_secs: u64,
    adaptive_high_activity_count: usize,
    change_detection_enabled: bool,
    change_pixel_threshold: u8,
    change_min_changed_percentage: f32,
    change_max_changed_percentage: f32,
    analysis_cooldown_secs: u64,
    performance_mode: Option<String>,
) -> Result<SystemSettings, String> {
    let mut settings = SystemSettings::load();
    settings.monitor_enabled = monitor_enabled;
    settings.monitor_interval_secs = monitor_interval_secs.clamp(10, 3600);
    settings.monitor_idle_secs = monitor_idle_secs.clamp(60, 60 * 60 * 12);
    settings.monitor_ignore_idle = monitor_ignore_idle;
    settings.monitor_allow_hidden = monitor_allow_hidden;
    settings.monitor_only_companion = monitor_only_companion;
    settings.monitor_recent_activity_count = monitor_recent_activity_count.clamp(1, 20);
    settings.monitor_idle_streak_threshold = monitor_idle_streak_threshold.clamp(1, 10);
    settings.monitor_category_window = monitor_category_window.clamp(5, 30);
    settings.global_shortcut_enabled = global_shortcut_enabled;
    settings.adaptive_capture_enabled = adaptive_capture_enabled;
    settings.adaptive_min_interval_secs = adaptive_min_interval_secs.clamp(5, 60);
    settings.adaptive_max_interval_secs = adaptive_max_interval_secs.clamp(60, 3600);
    settings.adaptive_idle_threshold_secs = adaptive_idle_threshold_secs.clamp(30, 3600);
    settings.adaptive_low_activity_threshold_secs =
        adaptive_low_activity_threshold_secs.clamp(10, 300);
    settings.adaptive_high_activity_count = adaptive_high_activity_count.clamp(5, 100);
    settings.change_detection_enabled = change_detection_enabled;
    settings.change_pixel_threshold = change_pixel_threshold;
    settings.change_min_changed_percentage = change_min_changed_percentage.clamp(0.0, 1.0);
    settings.change_max_changed_percentage = change_max_changed_percentage.clamp(0.0, 1.0);
    settings.analysis_cooldown_secs = analysis_cooldown_secs.clamp(30, 3600);
    if let Some(mode_str) = performance_mode {
        if let Ok(mode) = serde_json::from_str::<PerformanceMode>(&format!("\"{}\"", mode_str)) {
            settings.performance_mode = mode;
        }
    }

    settings.save().map_err(|e| e.to_string())?;
    Ok(settings)
}

#[tauri::command]
pub fn set_global_shortcut(
    shortcut: String,
    app: tauri::AppHandle,
) -> Result<SystemSettings, String> {
    let parsed = Shortcut::from_str(&shortcut).map_err(|e| e.to_string())?;
    let manager = app.global_shortcut();

    let current = SystemSettings::load();
    if current.global_shortcut_enabled {
        if let Ok(existing) = Shortcut::from_str(&current.global_shortcut) {
            let _ = manager.unregister(existing);
        }
        if let Err(err) = manager.register(parsed) {
            return Err(err.to_string());
        }
        let app_handle_for_shortcut = app.clone();
        if let Err(err) = manager.on_shortcut(parsed, move |_, _, _| {
            if let Some(window) = app_handle_for_shortcut.get_webview_window("main") {
                let visible = window.is_visible().unwrap_or(true);
                if visible {
                    let _ = window.hide();
                } else {
                    let _ = window.show();
                    let _ = window.set_focus();
                }
            }
        }) {
            return Err(err.to_string());
        }
    }

    let mut settings = current;
    settings.global_shortcut = shortcut;
    settings.save().map_err(|e| e.to_string())?;
    Ok(settings)
}

#[tauri::command]
pub fn set_global_shortcut_enabled(
    enabled: bool,
    app: tauri::AppHandle,
) -> Result<SystemSettings, String> {
    let settings = SystemSettings::load();
    let shortcut = Shortcut::from_str(&settings.global_shortcut).map_err(|e| e.to_string())?;
    let manager = app.global_shortcut();

    if enabled {
        let _ = manager.unregister(shortcut);
        if let Err(err) = manager.register(shortcut) {
            return Err(err.to_string());
        }
        let app_handle_for_shortcut = app.clone();
        if let Err(err) = manager.on_shortcut(shortcut, move |_, _, _| {
            if let Some(window) = app_handle_for_shortcut.get_webview_window("main") {
                let visible = window.is_visible().unwrap_or(true);
                if visible {
                    let _ = window.hide();
                } else {
                    let _ = window.show();
                    let _ = window.set_focus();
                }
            }
        }) {
            return Err(err.to_string());
        }
    } else {
        let _ = manager.unregister(shortcut);
    }

    let mut settings = settings;
    settings.global_shortcut_enabled = enabled;
    settings.save().map_err(|e| e.to_string())?;
    Ok(settings)
}

#[tauri::command]
pub fn set_monitor_enabled(enabled: bool) -> Result<SystemSettings, String> {
    let mut settings = SystemSettings::load();
    settings.monitor_enabled = enabled;
    settings.save().map_err(|e| e.to_string())?;
    Ok(settings)
}

#[tauri::command]
pub fn get_change_detection_settings() -> crate::capture::change_detection::ChangeDetectionConfig {
    let settings = SystemSettings::load();
    crate::capture::change_detection::ChangeDetectionConfig {
        pixel_threshold: settings.change_pixel_threshold,
        min_changed_percentage: settings.change_min_changed_percentage,
        max_changed_percentage: settings.change_max_changed_percentage,
    }
}

#[tauri::command]
pub fn set_change_detection_settings(
    pixel_threshold: u8,
    min_changed_percentage: f32,
    max_changed_percentage: f32,
) -> Result<SystemSettings, String> {
    let mut settings = SystemSettings::load();
    settings.change_pixel_threshold = pixel_threshold;
    settings.change_min_changed_percentage = min_changed_percentage.clamp(0.0, 1.0);
    settings.change_max_changed_percentage = max_changed_percentage.clamp(0.0, 1.0);
    settings.save().map_err(|e| e.to_string())?;
    Ok(settings)
}
