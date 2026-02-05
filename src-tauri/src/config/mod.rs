//! Config module - system configuration and settings

pub mod permissions;
pub mod privacy;
pub mod scheduler;
pub mod system_settings;
pub mod system_status;

// Re-export commonly used types
pub use permissions::{PermissionCheck, PermissionDiagnostics, get_permission_diagnostics, check_permission, open_permission_settings};
pub use privacy::{AutonomyLevel, PreviewPolicy, PrivacySettings, can_analyze_with_ai, can_capture_browser_content, can_capture_browser_tab, can_capture_screen, get_privacy_notice, get_privacy_settings, has_full_consent, update_privacy_settings};
pub use scheduler::{SchedulerSettings, SchedulerState, start_scheduler_loop, stop_scheduler_loop, is_scheduler_running, get_scheduler_settings, update_scheduler_settings};
pub use system_settings::{SystemSettings, get_system_settings, update_system_settings};
pub use system_status::{SystemStatusStore, get_status_snapshot, update_status, HEARTBEAT_TIMEOUT_SECS};
