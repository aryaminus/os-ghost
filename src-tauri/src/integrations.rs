//! Calendar and notes integrations (local-first)

use crate::memory::MemoryStore;
use chrono::{DateTime, Local, NaiveDate, NaiveDateTime, TimeZone, Utc};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

const CALENDAR_SETTINGS_FILE: &str = "calendar_settings.json";
const NOTES_TREE: &str = "notes";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CalendarSettings {
    pub enabled: bool,
    pub ics_path: Option<String>,
    pub lookahead_days: u32,
}

impl Default for CalendarSettings {
    fn default() -> Self {
        Self {
            enabled: false,
            ics_path: None,
            lookahead_days: 7,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CalendarEvent {
    pub id: String,
    pub summary: String,
    pub starts_at: u64,
    pub ends_at: Option<u64>,
    pub location: Option<String>,
    pub all_day: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Note {
    pub id: String,
    pub title: String,
    pub body: String,
    pub pinned: bool,
    pub created_at: u64,
    pub updated_at: u64,
}

pub struct NotesStore {
    store: MemoryStore,
}

impl NotesStore {
    pub fn new(store: MemoryStore) -> Self {
        Self { store }
    }

    pub fn list_notes(&self) -> anyhow::Result<Vec<Note>> {
        let mut notes: Vec<Note> = self.store.get_all(NOTES_TREE)?;
        notes.sort_by(|a, b| {
            b.pinned
                .cmp(&a.pinned)
                .then_with(|| b.updated_at.cmp(&a.updated_at))
        });
        Ok(notes)
    }

    pub fn add_note(&self, title: String, body: String) -> anyhow::Result<Note> {
        let now = crate::utils::current_timestamp();
        let id = format!("note_{}_{}", now, rand::random::<u32>() % 10000);
        let note = Note {
            id: id.clone(),
            title,
            body,
            pinned: false,
            created_at: now,
            updated_at: now,
        };
        self.store.set(NOTES_TREE, &id, &note)?;
        Ok(note)
    }

    pub fn update_note(&self, note: Note) -> anyhow::Result<Note> {
        let mut updated = note;
        updated.updated_at = crate::utils::current_timestamp();
        self.store.set(NOTES_TREE, &updated.id, &updated)?;
        Ok(updated)
    }

    pub fn delete_note(&self, id: &str) -> anyhow::Result<()> {
        self.store.delete(NOTES_TREE, id)?;
        Ok(())
    }
}

impl CalendarSettings {
    fn settings_path() -> PathBuf {
        let mut path = dirs::config_dir().unwrap_or_else(|| PathBuf::from("."));
        path.push("os-ghost");
        path.push(CALENDAR_SETTINGS_FILE);
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
pub fn get_calendar_settings() -> CalendarSettings {
    CalendarSettings::load()
}

#[tauri::command]
pub fn update_calendar_settings(
    enabled: bool,
    ics_path: Option<String>,
    lookahead_days: u32,
) -> Result<CalendarSettings, String> {
    let mut settings = CalendarSettings::load();
    settings.enabled = enabled;
    settings.ics_path = ics_path;
    settings.lookahead_days = lookahead_days.clamp(1, 30);
    settings.save().map_err(|e| e.to_string())?;
    Ok(settings)
}

#[tauri::command]
pub fn get_upcoming_events(limit: Option<usize>) -> Vec<CalendarEvent> {
    let settings = CalendarSettings::load();
    if !settings.enabled {
        return Vec::new();
    }
    let path = match settings.ics_path.as_ref() {
        Some(path) if !path.trim().is_empty() => path,
        _ => return Vec::new(),
    };

    let contents = match fs::read_to_string(path) {
        Ok(contents) => contents,
        Err(_) => return Vec::new(),
    };

    let events = parse_ics_events(&contents);
    let now = Local::now().timestamp() as u64;
    let end_window = now.saturating_add(settings.lookahead_days as u64 * 86400);
    let mut upcoming: Vec<CalendarEvent> = events
        .into_iter()
        .filter(|event| {
            let start = event.starts_at;
            let end = event.ends_at.unwrap_or(start);
            start <= end_window && end >= now
        })
        .collect();

    upcoming.sort_by(|a, b| a.starts_at.cmp(&b.starts_at));
    if let Some(limit) = limit {
        upcoming.truncate(limit);
    }
    upcoming
}

#[tauri::command]
pub fn list_notes(notes_store: tauri::State<'_, std::sync::Arc<NotesStore>>) -> Vec<Note> {
    notes_store.list_notes().unwrap_or_default()
}

#[tauri::command]
pub fn add_note(
    title: String,
    body: String,
    notes_store: tauri::State<'_, std::sync::Arc<NotesStore>>,
) -> Result<Note, String> {
    notes_store.add_note(title, body).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn update_note(
    note: Note,
    notes_store: tauri::State<'_, std::sync::Arc<NotesStore>>,
) -> Result<Note, String> {
    notes_store.update_note(note).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn delete_note(
    id: String,
    notes_store: tauri::State<'_, std::sync::Arc<NotesStore>>,
) -> Result<(), String> {
    notes_store.delete_note(&id).map_err(|e| e.to_string())
}

fn parse_ics_events(contents: &str) -> Vec<CalendarEvent> {
    let lines = unfold_ics_lines(contents);
    let mut events = Vec::new();
    let mut in_event = false;
    let mut current = EventBuilder::default();

    for line in lines {
        let trimmed = line.trim();
        if trimmed == "BEGIN:VEVENT" {
            in_event = true;
            current = EventBuilder::default();
            continue;
        }
        if trimmed == "END:VEVENT" {
            if in_event {
                let builder = std::mem::take(&mut current);
                if let Some(event) = builder.build() {
                    events.push(event);
                }
            }
            in_event = false;
            continue;
        }
        if !in_event {
            continue;
        }

        if let Some((key, value)) = split_ics_line(trimmed) {
            current.apply(key, value);
        }
    }

    events
}

fn unfold_ics_lines(contents: &str) -> Vec<String> {
    let mut lines: Vec<String> = Vec::new();
    for line in contents.lines() {
        if line.starts_with(' ') || line.starts_with('\t') {
            if let Some(last) = lines.last_mut() {
                last.push_str(line.trim_start());
            }
        } else {
            lines.push(line.to_string());
        }
    }
    lines
}

fn split_ics_line(line: &str) -> Option<(&str, &str)> {
    let mut parts = line.splitn(2, ':');
    let key = parts.next()?;
    let value = parts.next()?;
    let key = key.split(';').next().unwrap_or(key);
    Some((key, value))
}

#[derive(Default)]
struct EventBuilder {
    id: Option<String>,
    summary: Option<String>,
    starts_at: Option<(u64, bool)>,
    ends_at: Option<u64>,
    location: Option<String>,
}

impl EventBuilder {
    fn apply(&mut self, key: &str, value: &str) {
        match key {
            "UID" => self.id = Some(value.to_string()),
            "SUMMARY" => self.summary = Some(value.to_string()),
            "DTSTART" => self.starts_at = parse_ics_datetime(value),
            "DTEND" => self.ends_at = parse_ics_datetime(value).map(|(ts, _)| ts),
            "LOCATION" => self.location = Some(value.to_string()),
            _ => {}
        }
    }

    fn build(self) -> Option<CalendarEvent> {
        let (starts_at, all_day) = self.starts_at?;
        Some(CalendarEvent {
            id: self.id.unwrap_or_else(|| format!("event_{}", starts_at)),
            summary: self.summary.unwrap_or_else(|| "(untitled)".to_string()),
            starts_at,
            ends_at: self.ends_at,
            location: self.location,
            all_day,
        })
    }
}

fn parse_ics_datetime(value: &str) -> Option<(u64, bool)> {
    if value.len() == 8 {
        let date = NaiveDate::parse_from_str(value, "%Y%m%d").ok()?;
        let local = Local
            .from_local_datetime(&date.and_hms_opt(0, 0, 0)?)
            .single()?;
        return Some((local.timestamp() as u64, true));
    }

    if value.ends_with('Z') {
        let trimmed = value.trim_end_matches('Z');
        let dt = NaiveDateTime::parse_from_str(trimmed, "%Y%m%dT%H%M%S").ok()?;
        let utc_dt: DateTime<Utc> = Utc.from_utc_datetime(&dt);
        let local = utc_dt.with_timezone(&Local);
        return Some((local.timestamp() as u64, false));
    }

    let dt = NaiveDateTime::parse_from_str(value, "%Y%m%dT%H%M%S").ok()?;
    let local = Local.from_local_datetime(&dt).single()?;
    Some((local.timestamp() as u64, false))
}
