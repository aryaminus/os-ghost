//! Screen capture functionality using screenshots crate
//! Provides screen capture with base64 encoding for AI analysis

use anyhow::Result;
use base64::{engine::general_purpose, Engine as _};
use screenshots::Screen;
use std::io::Cursor;

// Use the image types from screenshots crate to avoid version conflicts
use screenshots::image::ImageFormat;

/// Helper: Get the primary screen
fn get_primary_screen() -> Result<Screen> {
    let screens = Screen::all()?;
    screens
        .first()
        .cloned()
        .ok_or_else(|| anyhow::anyhow!("No screens found"))
}

/// Capture the primary monitor's screen and return as base64-encoded PNG
pub fn capture_primary_monitor() -> Result<String> {
    let primary = get_primary_screen()?;

    // Capture screenshot - returns an ImageBuffer<Rgba<u8>, Vec<u8>>
    let image = primary.capture()?;

    // Write to JPEG instead of PNG for performance (10x faster encoding)
    // AI models handle JPEG compression artifacts well
    let mut jpeg_buffer = Vec::new();
    image.write_to(&mut Cursor::new(&mut jpeg_buffer), ImageFormat::Jpeg)?;

    // Base64 encode
    let base64_image = general_purpose::STANDARD.encode(&jpeg_buffer);

    Ok(base64_image)
}

/// Capture a specific region of the screen
pub fn capture_region(x: i32, y: i32, width: u32, height: u32) -> Result<String> {
    let primary = get_primary_screen()?;

    // Capture the region
    let image = primary.capture_area(x, y, width, height)?;

    // Write to JPEG
    let mut jpeg_buffer = Vec::new();
    image.write_to(&mut Cursor::new(&mut jpeg_buffer), ImageFormat::Jpeg)?;

    let base64_image = general_purpose::STANDARD.encode(&jpeg_buffer);

    Ok(base64_image)
}

/// Tauri command to capture and return screenshot
/// Captures the screen without hiding the window - the ghost appearing in the capture
/// is acceptable and provides better UX than flashing the UI
#[tauri::command]
pub async fn capture_screen(_app: tauri::AppHandle) -> Result<String, String> {
    // Perform capture in blocking thread (no window hiding - better UX)
    tokio::task::spawn_blocking(capture_primary_monitor)
        .await
        .map_err(|e| e.to_string())?
        .map_err(|e| e.to_string())
}
