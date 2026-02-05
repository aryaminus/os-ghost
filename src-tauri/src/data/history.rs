//! Safe Chrome history reader
//! Reads Chrome browsing history without locking the database

use anyhow::Result;
use rusqlite::Connection;
use serde::Serialize;
use std::path::PathBuf;
use tempfile::TempDir;

#[derive(Debug, Serialize, Clone)]
pub struct HistoryEntry {
    pub url: String,
    pub title: String,
    pub visit_count: i32,
    pub last_visit_time: i64,
}

/// Get a Chromium-family history database path for current OS
///
/// Tries a few common browser locations (Chrome, Chromium, Brave, Edge, Arc) and
/// returns the first existing path.
fn get_chrome_history_path() -> Result<PathBuf> {
    #[cfg(target_os = "macos")]
    {
        let home = dirs::home_dir().ok_or_else(|| anyhow::anyhow!("No home directory found"))?;
        let candidates = [
            "Library/Application Support/Google/Chrome/Default/History",
            "Library/Application Support/Chromium/Default/History",
            "Library/Application Support/BraveSoftware/Brave-Browser/Default/History",
            "Library/Application Support/Microsoft Edge/Default/History",
            "Library/Application Support/Arc/User Data/Default/History",
        ];

        for rel in candidates {
            let path = home.join(rel);
            if path.exists() {
                tracing::debug!("Using browser history at: {:?}", path);
                return Ok(path);
            }
        }

        // Default Chrome path (even if it doesn't exist)
        Ok(home.join("Library/Application Support/Google/Chrome/Default/History"))
    }

    #[cfg(target_os = "windows")]
    {
        let local_app_data = std::env::var("LOCALAPPDATA")
            .map(PathBuf::from)
            .map_err(|e| anyhow::anyhow!("LOCALAPPDATA not set: {}", e))?;

        let candidates = [
            local_app_data.join("Google/Chrome/User Data/Default/History"),
            local_app_data.join("Chromium/User Data/Default/History"),
            local_app_data.join("BraveSoftware/Brave-Browser/User Data/Default/History"),
            local_app_data.join("Microsoft/Edge/User Data/Default/History"),
        ];

        for path in candidates {
            if path.exists() {
                return Ok(path);
            }
        }

        Ok(local_app_data.join("Google/Chrome/User Data/Default/History"))
    }

    #[cfg(target_os = "linux")]
    {
        let home = dirs::home_dir().ok_or_else(|| anyhow::anyhow!("No home directory found"))?;
        let candidates = [
            home.join(".config/google-chrome/Default/History"),
            home.join(".config/chromium/Default/History"),
            home.join(".config/BraveSoftware/Brave-Browser/Default/History"),
        ];

        for path in candidates {
            if path.exists() {
                return Ok(path);
            }
        }

        Ok(home.join(".config/google-chrome/Default/History"))
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

    // Create temp copy to avoid database locking issues.
    // Use a temp directory so cleanup happens even on early returns.
    let temp_dir = TempDir::new()?;
    let temp_path = temp_dir
        .path()
        .join(format!("ghost_history_{}.db", uuid::Uuid::new_v4()));

    // Copy the database file
    std::fs::copy(&history_path, &temp_path)?;

    // Open and query the copy
    let conn = Connection::open(&temp_path)?;

    let mut stmt = conn.prepare(
        "SELECT url, title, visit_count, last_visit_time 
         FROM urls 
         ORDER BY last_visit_time DESC 
         LIMIT ?",
    )?;

    let entries: Vec<HistoryEntry> = stmt
        .query_map([limit], |row| {
            Ok(HistoryEntry {
                url: row.get(0)?,
                title: row.get(1)?,
                visit_count: row.get(2)?,
                last_visit_time: row.get(3)?,
            })
        })?
        .filter_map(|r| r.ok())
        .collect();

    // temp_dir is dropped here, cleaning up the copied DB
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
