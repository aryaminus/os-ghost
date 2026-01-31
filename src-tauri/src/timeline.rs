//! Activity timeline for observability and audit

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

const TIMELINE_FILE: &str = "timeline.json";
const MAX_ENTRIES: usize = 300;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TimelineEntryType {
    Observation,
    Suggestion,
    Action,
    Routine,
    System,
    Guardrail,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TimelineStatus {
    Pending,
    Approved,
    Denied,
    Executed,
    Failed,
    Info,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimelineEntry {
    pub id: String,
    pub timestamp: u64,
    pub entry_type: TimelineEntryType,
    pub summary: String,
    pub reason: Option<String>,
    pub status: TimelineStatus,
}

fn timeline_path() -> PathBuf {
    let mut path = dirs::config_dir().unwrap_or_else(|| PathBuf::from("."));
    path.push("os-ghost");
    path.push(TIMELINE_FILE);
    path
}

fn load_entries() -> Vec<TimelineEntry> {
    let path = timeline_path();
    if path.exists() {
        if let Ok(contents) = fs::read_to_string(&path) {
            if let Ok(entries) = serde_json::from_str::<Vec<TimelineEntry>>(&contents) {
                return entries;
            }
        }
    }
    Vec::new()
}

fn save_entries(entries: &[TimelineEntry]) -> Result<(), String> {
    let path = timeline_path();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let contents = serde_json::to_string_pretty(entries).map_err(|e| e.to_string())?;
    fs::write(&path, contents).map_err(|e| e.to_string())?;
    Ok(())
}

pub fn record_timeline_event(
    summary: &str,
    reason: Option<String>,
    entry_type: TimelineEntryType,
    status: TimelineStatus,
) {
    let mut entries = load_entries();
    let now = crate::utils::current_timestamp();

    if entries.iter().rev().take(10).any(|entry| {
        entry.summary == summary
            && entry.reason == reason
            && entry.entry_type == entry_type
            && entry.status == status
            && now.saturating_sub(entry.timestamp) < 15 * 60
    }) {
        return;
    }
    let timestamp = crate::utils::current_timestamp();
    let id = format!("timeline_{}_{}", timestamp, entries.len() + 1);

    entries.push(TimelineEntry {
        id,
        timestamp,
        entry_type,
        summary: summary.to_string(),
        reason,
        status,
    });

    if entries.len() > MAX_ENTRIES {
        let start = entries.len().saturating_sub(MAX_ENTRIES);
        entries = entries.split_off(start);
    }

    let _ = save_entries(&entries);
}

#[tauri::command]
pub fn get_timeline(limit: Option<usize>, offset: Option<usize>) -> Vec<TimelineEntry> {
    let mut entries = load_entries();
    entries.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));
    let start = offset.unwrap_or(0);
    let end = start + limit.unwrap_or(50);
    entries.into_iter().skip(start).take(end - start).collect()
}

#[tauri::command]
pub fn clear_timeline() -> usize {
    let entries = load_entries();
    let count = entries.len();
    let _ = save_entries(&[]);
    count
}

pub fn get_recent_timeline(limit: usize) -> Vec<TimelineEntry> {
    let mut entries = load_entries();
    entries.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));
    entries.into_iter().take(limit).collect()
}
