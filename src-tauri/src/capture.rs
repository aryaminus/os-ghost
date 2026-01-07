//! Screen capture functionality using screenshots crate
//! Provides screen capture with base64 encoding for AI analysis

use anyhow::Result;
use base64::{engine::general_purpose, Engine as _};
use screenshots::Screen;
use std::io::Cursor;
use tauri::Manager;

// Use the image types from screenshots crate to avoid version conflicts
use screenshots::image::ImageFormat;

/// Capture the primary monitor's screen and return as base64-encoded PNG
pub fn capture_primary_monitor() -> Result<String> {
    // Get all screens
    let screens = Screen::all()?;

    // Use primary (first) screen
    let primary = screens
        .first()
        .ok_or_else(|| anyhow::anyhow!("No screens found"))?;

    // Capture screenshot - returns an ImageBuffer<Rgba<u8>, Vec<u8>>
    let image = primary.capture()?;

    // Write to PNG using the screenshots crate's image types
    let mut png_buffer = Vec::new();
    image.write_to(&mut Cursor::new(&mut png_buffer), ImageFormat::Png)?;

    // Base64 encode
    let base64_image = general_purpose::STANDARD.encode(&png_buffer);

    Ok(base64_image)
}

/// Capture a specific region of the screen
pub fn capture_region(x: i32, y: i32, width: u32, height: u32) -> Result<String> {
    let screens = Screen::all()?;

    let primary = screens
        .first()
        .ok_or_else(|| anyhow::anyhow!("No screens found"))?;

    // Capture the region
    let image = primary.capture_area(x, y, width, height)?;

    // Write to PNG
    let mut png_buffer = Vec::new();
    image.write_to(&mut Cursor::new(&mut png_buffer), ImageFormat::Png)?;

    let base64_image = general_purpose::STANDARD.encode(&png_buffer);

    Ok(base64_image)
}

/// Tauri command to capture and return screenshot
/// Hides the main window before capturing to ensure the Ghost doesn't capture itself
#[tauri::command]
pub async fn capture_screen(app: tauri::AppHandle) -> Result<String, String> {
    // 1. Hide window if it exists
    let window = app.get_webview_window("main");
    if let Some(ref w) = window {
        // We use unwrap_or to safely ignore errors if window is already hidden/gone
        let _ = w.hide();
        // Small delay to ensure the window is fully hidden from the framebuffer
        // 50ms is usually sufficient for the compositor to update
        tokio::time::sleep(std::time::Duration::from_millis(150)).await;
    }

    // 2. Perform capture in blocking thread
    let result = tokio::task::spawn_blocking(|| capture_primary_monitor())
        .await
        .map_err(|e| e.to_string())?
        .map_err(|e| e.to_string());

    // 3. Show window again immediately
    if let Some(ref w) = window {
        let _ = w.show();
    }

    result
}
