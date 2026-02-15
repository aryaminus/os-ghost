//! Config module - system configuration and settings

pub mod permissions;
pub mod privacy;
pub mod scheduler;
pub mod secrets;
pub mod server;
pub mod system_settings;
pub mod system_status;
pub mod toml_config;

// Re-export commonly used types
pub use permissions::{get_permission_diagnostics, PermissionCheck, PermissionDiagnostics};
pub use privacy::{
    can_analyze_with_ai, can_capture_browser_content, can_capture_browser_tab, can_capture_screen,
    get_privacy_notice, get_privacy_settings, update_privacy_settings, AutonomyLevel,
    PreviewPolicy, PrivacySettings,
};
pub use scheduler::{
    get_scheduler_settings, start_scheduler_loop, update_scheduler_settings, SchedulerSettings,
    SchedulerState,
};
pub use secrets::{
    delete_api_key, delete_secret, get_api_key, get_secret, has_api_key, has_secret, store_api_key,
    store_secret, SecretError,
};
pub use server::ServerConfig;
pub use system_settings::{get_system_settings, update_system_settings, SystemSettings};
pub use system_status::{
    get_status_snapshot, update_status, SystemStatusStore, HEARTBEAT_TIMEOUT_SECS,
};
pub use toml_config::{get_toml_config, load_toml_config, save_toml_config, TomlConfig};
