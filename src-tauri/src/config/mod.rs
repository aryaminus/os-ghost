//! Config module - system configuration and settings

pub mod permissions;
pub mod privacy;
pub mod scheduler;
pub mod server;
pub mod system_settings;
pub mod system_status;

// Re-export commonly used types
pub use permissions::{PermissionCheck, PermissionDiagnostics, get_permission_diagnostics};
pub use privacy::{AutonomyLevel, PreviewPolicy, PrivacySettings, can_analyze_with_ai, can_capture_browser_content, can_capture_browser_tab, can_capture_screen, get_privacy_notice, get_privacy_settings, update_privacy_settings};
pub use scheduler::{SchedulerSettings, SchedulerState, start_scheduler_loop, get_scheduler_settings, update_scheduler_settings};
pub use server::ServerConfig;
pub use system_settings::{SystemSettings, get_system_settings, update_system_settings};
pub use system_status::{SystemStatusStore, get_status_snapshot, update_status, HEARTBEAT_TIMEOUT_SECS};
