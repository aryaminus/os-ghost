//! Integrations module - external service integrations

pub mod bridge;
pub mod email;
#[allow(clippy::module_inception)]
pub mod integrations;

// Re-export commonly used types and Tauri commands from integrations.rs
pub use integrations::{
    add_note, apply_email_triage, delete_note, email_begin_oauth, email_disconnect,
    email_oauth_status, get_calendar_settings, get_email_settings, get_files_settings,
    get_upcoming_events, list_email_inbox, list_notes, list_recent_files, triage_email_inbox,
    update_calendar_settings, update_email_settings, update_files_settings, update_note,
    CalendarSettings, EmailMessage, EmailSettings, EmailTriageDecision, FilesSettings,
};
