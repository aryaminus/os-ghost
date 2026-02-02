//! Permission policy for action execution and OS-level permission diagnostics

use crate::privacy::AutonomyLevel;
use serde::{Deserialize, Serialize};

// ============================================================================
// Action Permission Policy (new)
// ============================================================================

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PermissionDecision {
    Deny,
    RequireConfirmation,
    Allow,
}

/// Decide if an action should execute based on autonomy and risk
pub fn evaluate_action(autonomy: AutonomyLevel, is_high_risk: bool) -> PermissionDecision {
    if !autonomy.allows_actions() {
        return PermissionDecision::Deny;
    }

    let profile = crate::privacy::PrivacySettings::load().trust_profile;
    if profile == "strict" && is_high_risk {
        return PermissionDecision::RequireConfirmation;
    }

    if autonomy.requires_confirmation(is_high_risk) {
        return PermissionDecision::RequireConfirmation;
    }

    PermissionDecision::Allow
}

// ============================================================================
// OS-Level Permission Diagnostics (restored)
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PermissionState {
    Granted,
    Denied,
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PermissionCheck {
    pub status: PermissionState,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub action_url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PermissionDiagnostics {
    pub screen_recording: PermissionCheck,
    pub accessibility: PermissionCheck,
    pub input_monitoring: PermissionCheck,
}

fn check_screen_recording() -> PermissionCheck {
    match crate::capture::check_screen_recording_permission() {
        Ok(()) => PermissionCheck {
            status: PermissionState::Granted,
            message: "Screen recording available".to_string(),
            action_url: None,
        },
        Err(err) => PermissionCheck {
            status: PermissionState::Denied,
            message: err,
            action_url: action_url_screen_recording(),
        },
    }
}

fn unknown_check(label: &str, action_url: Option<String>) -> PermissionCheck {
    PermissionCheck {
        status: PermissionState::Unknown,
        message: format!("{} permission not checked", label),
        action_url,
    }
}

pub async fn get_permission_diagnostics() -> PermissionDiagnostics {
    let screen = tokio::task::spawn_blocking(check_screen_recording)
        .await
        .unwrap_or_else(|_| PermissionCheck {
            status: PermissionState::Unknown,
            message: "Screen recording check failed".to_string(),
            action_url: None,
        });

    PermissionDiagnostics {
        screen_recording: screen,
        accessibility: unknown_check("Accessibility", action_url_accessibility()),
        input_monitoring: unknown_check("Input monitoring", action_url_input_monitoring()),
    }
}

fn action_url_screen_recording() -> Option<String> {
    #[cfg(target_os = "macos")]
    {
        return Some(
            "x-apple.systempreferences:com.apple.preference.security?Privacy_ScreenCapture"
                .to_string(),
        );
    }
    #[cfg(target_os = "windows")]
    {
        return Some("ms-settings:privacy-screenrecording".to_string());
    }
    #[cfg(target_os = "linux")]
    {
        return Some("https://help.gnome.org/users/gnome-help/stable/privacy.html.en".to_string());
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
    {
        None
    }
}

fn action_url_accessibility() -> Option<String> {
    #[cfg(target_os = "macos")]
    {
        return Some(
            "x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility"
                .to_string(),
        );
    }
    #[cfg(target_os = "windows")]
    {
        return Some("ms-settings:easeofaccess".to_string());
    }
    #[cfg(target_os = "linux")]
    {
        return Some(
            "https://help.gnome.org/users/gnome-help/stable/assistive-technologies.html.en"
                .to_string(),
        );
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
    {
        None
    }
}

fn action_url_input_monitoring() -> Option<String> {
    #[cfg(target_os = "macos")]
    {
        return Some(
            "x-apple.systempreferences:com.apple.preference.security?Privacy_ListenEvent"
                .to_string(),
        );
    }
    #[cfg(target_os = "windows")]
    {
        return Some("ms-settings:privacy-speechtyping".to_string());
    }
    #[cfg(target_os = "linux")]
    {
        return Some("https://help.gnome.org/users/gnome-help/stable/privacy.html.en".to_string());
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
    {
        None
    }
}

#[tauri::command]
pub async fn get_permission_diagnostics_command() -> PermissionDiagnostics {
    get_permission_diagnostics().await
}
