//! Screen capture functionality using screenshots crate
//! Provides screen capture with base64 encoding for AI analysis

use anyhow::Result;
use base64::{engine::general_purpose, Engine as _};
use screenshots::Screen;
use std::io::Cursor;

// Use the image types from screenshots crate to avoid version conflicts
use screenshots::image::ImageFormat;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

/// Helper: Get the primary screen
fn get_primary_screen() -> Result<Screen> {
    let screens = Screen::all()?;
    screens
        .first()
        .cloned()
        .ok_or_else(|| anyhow::anyhow!("No screens found"))
}

const CAPTURE_SETTINGS_FILE: &str = "capture_settings.json";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CaptureSettings {
    pub image_format: String,
}

impl Default for CaptureSettings {
    fn default() -> Self {
        Self {
            image_format: "jpeg".to_string(),
        }
    }
}

impl CaptureSettings {
    fn settings_path() -> PathBuf {
        let mut path = dirs::config_dir().unwrap_or_else(|| PathBuf::from("."));
        path.push("os-ghost");
        path.push(CAPTURE_SETTINGS_FILE);
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

fn resolve_image_format() -> ImageFormat {
    let settings = CaptureSettings::load();
    match settings.image_format.to_lowercase().as_str() {
        "png" => ImageFormat::Png,
        _ => ImageFormat::Jpeg,
    }
}

/// Capture the primary monitor's screen and return as base64-encoded image
pub fn capture_primary_monitor() -> Result<String> {
    let primary = get_primary_screen()?;

    // Capture screenshot - returns an ImageBuffer<Rgba<u8>, Vec<u8>>
    let image = primary.capture()?;

    // Write to configured format (JPEG default for speed)
    let mut jpeg_buffer = Vec::new();
    image.write_to(&mut Cursor::new(&mut jpeg_buffer), resolve_image_format())?;

    // Base64 encode
    let base64_image = general_purpose::STANDARD.encode(&jpeg_buffer);

    Ok(base64_image)
}

/// Capture a specific region of the screen
pub fn capture_region(x: i32, y: i32, width: u32, height: u32) -> Result<String> {
    let primary = get_primary_screen()?;

    // Capture the region
    let image = primary.capture_area(x, y, width, height)?;

    // Write to configured format
    let mut jpeg_buffer = Vec::new();
    image.write_to(&mut Cursor::new(&mut jpeg_buffer), resolve_image_format())?;

    let base64_image = general_purpose::STANDARD.encode(&jpeg_buffer);

    Ok(base64_image)
}

pub fn check_screen_recording_permission() -> Result<(), String> {
    match capture_primary_monitor() {
        Ok(_) => Ok(()),
        Err(err) => Err(format!("Screen recording blocked: {}", err)),
    }
}

/// Tauri command to capture and return screenshot
/// Captures the screen without hiding the window - the ghost appearing in the capture
/// is acceptable and provides better UX than flashing the UI
#[tauri::command]
pub async fn capture_screen(_app: tauri::AppHandle) -> Result<String, String> {
    let privacy = crate::config::privacy::PrivacySettings::load();
    if privacy.read_only_mode {
        return Err("Read-only mode enabled".to_string());
    }
    if !privacy.capture_consent {
        return Err("Screen capture consent not granted".to_string());
    }
    // Perform capture in blocking thread (no window hiding - better UX)
    tokio::task::spawn_blocking(capture_primary_monitor)
        .await
        .map_err(|e| e.to_string())?
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_capture_settings() -> CaptureSettings {
    CaptureSettings::load()
}

#[tauri::command]
pub fn set_capture_settings(image_format: String) -> Result<CaptureSettings, String> {
    let normalized = image_format.to_lowercase();
    if normalized != "jpeg" && normalized != "png" {
        return Err("Invalid image format".to_string());
    }
    let mut settings = CaptureSettings::load();
    settings.image_format = normalized;
    settings.save().map_err(|e| e.to_string())?;
    Ok(settings)
}
