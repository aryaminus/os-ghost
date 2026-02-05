//! Integrations module - external service integrations

pub mod bridge;
pub mod email;
#[allow(clippy::module_inception)]
pub mod integrations;

// Re-export commonly used types and Tauri commands from integrations.rs
pub use integrations::{
    CalendarSettings, FilesSettings, add_note, list_notes, delete_note, update_note,
    EmailSettings, EmailMessage, EmailTriageDecision, 
    get_calendar_settings, update_calendar_settings, get_upcoming_events,
    get_files_settings, update_files_settings, list_recent_files,
    get_email_settings, update_email_settings, email_oauth_status,
    email_begin_oauth, email_disconnect, list_email_inbox,
    triage_email_inbox, apply_email_triage
};
