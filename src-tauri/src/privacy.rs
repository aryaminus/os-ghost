//! Privacy settings and consent management
//! Handles user consent for screen capture and data transmission

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

const PRIVACY_FILE: &str = "privacy_settings.json";

/// Privacy settings stored locally
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrivacySettings {
    /// User has consented to screen capture
    pub capture_consent: bool,
    /// User has consented to sending screenshots to AI
    pub ai_analysis_consent: bool,
    /// User has read the privacy notice
    pub privacy_notice_acknowledged: bool,
    /// Timestamp of consent
    pub consent_timestamp: Option<u64>,
}

impl Default for PrivacySettings {
    fn default() -> Self {
        Self {
            capture_consent: false,
            ai_analysis_consent: false,
            privacy_notice_acknowledged: false,
            consent_timestamp: None,
        }
    }
}

impl PrivacySettings {
    /// Load settings from disk
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

    /// Save settings to disk
    pub fn save(&self) -> anyhow::Result<()> {
        let path = Self::settings_path();
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let contents = serde_json::to_string_pretty(self)?;
        fs::write(&path, contents)?;
        Ok(())
    }

    fn settings_path() -> PathBuf {
        let mut path = dirs::config_dir().unwrap_or_else(|| PathBuf::from("."));
        path.push("os-ghost");
        path.push(PRIVACY_FILE);
        path
    }

    /// Check if all required consents are given
    pub fn has_full_consent(&self) -> bool {
        self.capture_consent && self.ai_analysis_consent && self.privacy_notice_acknowledged
    }
}

/// Get current privacy settings
#[tauri::command]
pub fn get_privacy_settings() -> PrivacySettings {
    PrivacySettings::load()
}

/// Update privacy settings (grant consent)
#[tauri::command]
pub fn update_privacy_settings(
    capture_consent: bool,
    ai_analysis_consent: bool,
    privacy_notice_acknowledged: bool,
) -> Result<PrivacySettings, String> {
    let mut settings = PrivacySettings::load();
    settings.capture_consent = capture_consent;
    settings.ai_analysis_consent = ai_analysis_consent;
    settings.privacy_notice_acknowledged = privacy_notice_acknowledged;
    settings.consent_timestamp = Some(
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs(),
    );
    settings.save().map_err(|e| e.to_string())?;
    Ok(settings)
}

/// Check if screen capture is allowed
#[tauri::command]
pub fn can_capture_screen() -> bool {
    let settings = PrivacySettings::load();
    settings.capture_consent
}

/// Check if AI analysis is allowed
#[tauri::command]
pub fn can_analyze_with_ai() -> bool {
    let settings = PrivacySettings::load();
    settings.ai_analysis_consent
}

/// Privacy notice text
pub const PRIVACY_NOTICE: &str = r#"
THE OS GHOST - PRIVACY NOTICE

This game uses the following features that access your data:

1. SCREEN CAPTURE
   - Takes screenshots of your screen when you click the Ghost
   - Screenshots are processed locally and sent to Google's Gemini AI
   - Screenshots are NOT stored permanently

2. BROWSER HISTORY
   - Reads your Chrome browsing history (read-only)
   - Used to track puzzle progress
   - History data stays on your device

3. AI ANALYSIS
   - Screenshots may be sent to Google Gemini for analysis
   - Google's privacy policy applies to AI processing
   - No personal data is stored by this game

RECOMMENDATIONS:
- Close sensitive applications before using screen capture
- Do not capture screens with passwords, banking info, etc.
- The game works without AI features if you decline

By continuing, you consent to these data practices.
"#;

/// Get the privacy notice text
#[tauri::command]
pub fn get_privacy_notice() -> String {
    PRIVACY_NOTICE.to_string()
}
