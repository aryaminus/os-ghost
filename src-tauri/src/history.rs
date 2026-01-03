//! Safe Chrome history reader
//! Reads Chrome browsing history without locking the database

use anyhow::Result;
use rusqlite::Connection;
use serde::Serialize;
use std::path::PathBuf;

#[derive(Debug, Serialize, Clone)]
pub struct HistoryEntry {
    pub url: String,
    pub title: String,
    pub visit_count: i32,
    pub last_visit_time: i64,
}

/// Get Chrome history database path for current OS
fn get_chrome_history_path() -> Result<PathBuf> {
    #[cfg(target_os = "macos")]
    {
        let home = std::env::var("HOME")?;
        let path = PathBuf::from(format!(
            "{}/Library/Application Support/Google/Chrome/Default/History",
            home
        ));
        tracing::info!("Looking for Chrome history at: {:?}", path);
        Ok(path)
    }

    #[cfg(target_os = "windows")]
    {
        let local_app_data = std::env::var("LOCALAPPDATA")?;
        Ok(PathBuf::from(format!(
            "{}\\Google\\Chrome\\User Data\\Default\\History",
            local_app_data
        )))
    }

    #[cfg(target_os = "linux")]
    {
        let home = std::env::var("HOME")?;
        Ok(PathBuf::from(format!(
            "{}/.config/google-chrome/Default/History",
            home
        )))
    }
}

/// CRITICAL: Copy history DB to temp location to avoid locking Chrome's database
pub fn get_recent_urls(limit: usize) -> Result<Vec<HistoryEntry>> {
    let history_path = get_chrome_history_path()?;

    if !history_path.exists() {
        return Err(anyhow::anyhow!(
            "Chrome history database not found at {:?}",
            history_path
        ));
    }

    // Create temp copy to avoid database locking issues
    let temp_dir = std::env::temp_dir();
    let temp_path = temp_dir.join(format!("ghost_history_{}.db", uuid::Uuid::new_v4()));

    // Copy the database file
    std::fs::copy(&history_path, &temp_path)?;

    // Open and query the copy
    let conn = Connection::open(&temp_path)?;

    let mut stmt = conn.prepare(
        "SELECT url, title, visit_count, last_visit_time 
         FROM urls 
         ORDER BY last_visit_time DESC 
         LIMIT ?1",
    )?;

    let entries: Vec<HistoryEntry> = stmt
        .query_map([limit as i64], |row| {
            Ok(HistoryEntry {
                url: row.get(0)?,
                title: row.get(1)?,
                visit_count: row.get(2)?,
                last_visit_time: row.get(3)?,
            })
        })?
        .filter_map(|r| r.ok())
        .collect();

    // Clean up temp file
    let _ = std::fs::remove_file(&temp_path);

    Ok(entries)
}

/// Tauri command to get recent Chrome history
#[tauri::command]
pub async fn get_recent_history(limit: usize) -> Result<Vec<HistoryEntry>, String> {
    tokio::task::spawn_blocking(move || get_recent_urls(limit))
        .await
        .map_err(|e| e.to_string())?
        .map_err(|e| e.to_string())
}
