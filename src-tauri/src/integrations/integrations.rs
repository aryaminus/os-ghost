//! Calendar and notes integrations (local-first)

use crate::memory::MemoryStore;
use chrono::{DateTime, Local, NaiveDate, NaiveDateTime, TimeZone, Utc};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use std::sync::Arc;
use tauri::State;

const CALENDAR_SETTINGS_FILE: &str = "calendar_settings.json";
const FILES_SETTINGS_FILE: &str = "files_settings.json";
const EMAIL_SETTINGS_FILE: &str = "email_settings.json";
const NOTES_TREE: &str = "notes";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CalendarSettings {
    pub enabled: bool,
    pub ics_path: Option<String>,
    pub lookahead_days: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FilesSettings {
    pub enabled: bool,
    pub roots: Vec<String>,
    pub max_results: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmailSettings {
    pub enabled: bool,
    pub provider: String,
    pub inbox_limit: usize,
    #[serde(default)]
    pub connected: bool,
    #[serde(default)]
    pub account_email: Option<String>,
    #[serde(default)]
    pub last_sync_at: Option<u64>,
}

impl Default for EmailSettings {
    fn default() -> Self {
        Self {
            enabled: false,
            provider: "none".to_string(),
            inbox_limit: 10,
            connected: false,
            account_email: None,
            last_sync_at: None,
        }
    }
}

impl Default for FilesSettings {
    fn default() -> Self {
        Self {
            enabled: false,
            roots: vec![],
            max_results: 10,
        }
    }
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileEntry {
    pub path: String,
    pub name: String,
    pub modified_at: Option<u64>,
    pub size_bytes: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmailMessage {
    pub id: String,
    pub thread_id: Option<String>,
    pub subject: String,
    pub from: String,
    pub to: Vec<String>,
    pub snippet: String,
    pub received_at: u64,
    pub is_unread: bool,
    pub labels: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmailTriageDecision {
    pub message_id: String,
    pub action: String,
    pub summary: String,
    pub confidence: f32,
    pub tags: Vec<String>,
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
        let now = crate::core::utils::current_timestamp();
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
        let _ = self.store.flush();
        Ok(note)
    }

    pub fn update_note(&self, note: Note) -> anyhow::Result<Note> {
        let mut updated = note;
        updated.updated_at = crate::core::utils::current_timestamp();
        self.store.set(NOTES_TREE, &updated.id, &updated)?;
        let _ = self.store.flush();
        Ok(updated)
    }

    pub fn delete_note(&self, id: &str) -> anyhow::Result<()> {
        self.store.delete(NOTES_TREE, id)?;
        let _ = self.store.flush();
        Ok(())
    }

    pub fn get_note(&self, id: &str) -> anyhow::Result<Option<Note>> {
        self.store.get(NOTES_TREE, id)
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

impl FilesSettings {
    fn settings_path() -> PathBuf {
        let mut path = dirs::config_dir().unwrap_or_else(|| PathBuf::from("."));
        path.push("os-ghost");
        path.push(FILES_SETTINGS_FILE);
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

impl EmailSettings {
    fn settings_path() -> PathBuf {
        let mut path = dirs::config_dir().unwrap_or_else(|| PathBuf::from("."));
        path.push("os-ghost");
        path.push(EMAIL_SETTINGS_FILE);
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
pub fn get_files_settings() -> FilesSettings {
    FilesSettings::load()
}

#[tauri::command]
pub fn update_files_settings(
    enabled: bool,
    roots: Vec<String>,
    max_results: usize,
) -> Result<FilesSettings, String> {
    let mut settings = FilesSettings::load();
    settings.enabled = enabled;
    settings.roots = roots;
    settings.max_results = max_results.clamp(1, 50);
    settings.save().map_err(|e| e.to_string())?;
    Ok(settings)
}

#[tauri::command]
pub fn list_recent_files() -> Vec<FileEntry> {
    let settings = FilesSettings::load();
    if !settings.enabled {
        return Vec::new();
    }

    let mut entries: Vec<FileEntry> = Vec::new();
    for root in settings.roots.iter() {
        let path = PathBuf::from(root);
        let Ok(read_dir) = fs::read_dir(&path) else {
            continue;
        };
        for entry in read_dir.flatten() {
            let path = entry.path();
            if !path.is_file() {
                continue;
            }
            let name = path
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string();
            let metadata = entry.metadata().ok();
            let modified_at = metadata
                .as_ref()
                .and_then(|m| m.modified().ok())
                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                .map(|d| d.as_secs());
            let size_bytes = metadata.map(|m| m.len());
            entries.push(FileEntry {
                path: path.to_string_lossy().to_string(),
                name,
                modified_at,
                size_bytes,
            });
        }
    }

    entries.sort_by(|a, b| b.modified_at.cmp(&a.modified_at));
    entries.truncate(settings.max_results);
    entries
}

#[tauri::command]
pub fn get_email_settings() -> EmailSettings {
    EmailSettings::load()
}

#[tauri::command]
pub fn update_email_settings(
    enabled: bool,
    provider: String,
    inbox_limit: usize,
) -> Result<EmailSettings, String> {
    let mut settings = EmailSettings::load();
    settings.enabled = enabled;
    settings.provider = provider;
    settings.inbox_limit = inbox_limit.clamp(1, 50);
    if settings.provider == "none" || !settings.enabled {
        settings.connected = false;
        settings.account_email = None;
        settings.last_sync_at = None;
    }
    settings.save().map_err(|e| e.to_string())?;
    Ok(settings)
}

#[tauri::command]
pub fn email_oauth_status() -> EmailSettings {
    EmailSettings::load()
}

#[tauri::command]
pub fn email_begin_oauth(provider: String) -> Result<EmailSettings, String> {
    if provider != "gmail" {
        return Err("Only Gmail is supported".to_string());
    }
    tauri::async_runtime::block_on(async {
        crate::integrations::email::begin_oauth().await.map_err(|e| e.to_string())
    })
}

#[tauri::command]
pub fn email_disconnect() -> Result<EmailSettings, String> {
    tauri::async_runtime::block_on(async {
        crate::integrations::email::disconnect().await.map_err(|e| e.to_string())
    })
}

#[tauri::command]
pub fn list_email_inbox(limit: Option<usize>) -> Vec<EmailMessage> {
    let settings = EmailSettings::load();
    if !settings.enabled || !settings.connected {
        return Vec::new();
    }
    let count = limit.unwrap_or(settings.inbox_limit).clamp(1, 50);
    tauri::async_runtime::block_on(async {
        crate::integrations::email::list_inbox(count)
            .await
            .unwrap_or_default()
    })
}

#[tauri::command]
pub fn triage_email_inbox(
    limit: Option<usize>,
    ai_router: State<'_, Arc<crate::ai::ai_provider::SmartAiRouter>>,
) -> Vec<EmailTriageDecision> {
    let settings = EmailSettings::load();
    if !settings.enabled || !settings.connected {
        return Vec::new();
    }
    let count = limit.unwrap_or(settings.inbox_limit).clamp(1, 50);
    tauri::async_runtime::block_on(async {
        crate::integrations::email::triage_inbox(count, Some(ai_router.as_ref()))
            .await
            .unwrap_or_default()
    })
}

#[tauri::command]
pub fn apply_email_triage(decisions: Vec<EmailTriageDecision>) -> Result<(), String> {
    tauri::async_runtime::block_on(async {
        crate::integrations::email::apply_triage(&decisions)
            .await
            .map_err(|e| e.to_string())
    })
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
    let note = notes_store
        .add_note(title, body)
        .map_err(|e| e.to_string())?;
    if let Some(rollback) = crate::actions::rollback::get_rollback_manager() {
        rollback.record_note_change(
            &format!("note:add:{}", note.id),
            &note.id,
            None,
            Some(crate::actions::rollback::NoteSnapshot::from(&note)),
            format!("Add note: {}", note.title),
        );
    }
    Ok(note)
}

#[tauri::command]
pub fn update_note(
    note: Note,
    notes_store: tauri::State<'_, std::sync::Arc<NotesStore>>,
) -> Result<Note, String> {
    let before = notes_store
        .get_note(&note.id)
        .map_err(|e| e.to_string())?
        .map(|n| crate::actions::rollback::NoteSnapshot::from(&n));
    let updated = notes_store.update_note(note).map_err(|e| e.to_string())?;
    if let Some(rollback) = crate::actions::rollback::get_rollback_manager() {
        rollback.record_note_change(
            &format!("note:update:{}", updated.id),
            &updated.id,
            before,
            Some(crate::actions::rollback::NoteSnapshot::from(&updated)),
            format!("Update note: {}", updated.title),
        );
    }
    Ok(updated)
}

#[tauri::command]
pub fn delete_note(
    id: String,
    notes_store: tauri::State<'_, std::sync::Arc<NotesStore>>,
) -> Result<(), String> {
    let before = notes_store
        .get_note(&id)
        .map_err(|e| e.to_string())?
        .map(|n| crate::actions::rollback::NoteSnapshot::from(&n));
    notes_store.delete_note(&id).map_err(|e| e.to_string())?;
    if let Some(rollback) = crate::actions::rollback::get_rollback_manager() {
        rollback.record_note_change(
            &format!("note:delete:{}", id),
            &id,
            before,
            None,
            format!("Delete note: {}", id),
        );
    }
    Ok(())
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
    let (key, value) = line.split_once(':')?;
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
