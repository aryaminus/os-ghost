//! Privacy settings and consent management
//! Handles user consent for screen capture and data transmission

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

const PRIVACY_FILE: &str = "privacy_settings.json";

/// Privacy settings stored locally
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PrivacySettings {
    /// User has consented to screen capture
    pub capture_consent: bool,
    /// User has consented to sending screenshots to AI
    pub ai_analysis_consent: bool,
    /// User has read the privacy notice
    pub privacy_notice_acknowledged: bool,
    /// Read-only mode (no screen capture or automation)
    #[serde(default)]
    pub read_only_mode: bool,
    /// Timestamp of consent
    pub consent_timestamp: Option<u64>,
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
        self.capture_consent
            && self.ai_analysis_consent
            && self.privacy_notice_acknowledged
            && !self.read_only_mode
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
    read_only_mode: bool,
) -> Result<PrivacySettings, String> {
    let mut settings = PrivacySettings::load();
    settings.capture_consent = capture_consent;
    settings.ai_analysis_consent = ai_analysis_consent;
    settings.privacy_notice_acknowledged = privacy_notice_acknowledged;
    settings.read_only_mode = read_only_mode;
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
   - Takes screenshots of your screen when you use Ghost actions (and, if enabled, periodically in the background)
   - Background monitoring captures roughly once per minute to power Companion behavior
   - Screenshots are processed locally and may be sent to Google's Gemini AI (only if you consent)
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

/// Redact PII from text before sending to AI
pub fn redact_pii(text: &str) -> String {
    use regex::Regex;
    use std::borrow::Cow;
    use std::sync::OnceLock;

    // Compile regexes once. If compilation ever fails (shouldn't), skip that redaction.
    static EMAIL_REGEX: OnceLock<Option<Regex>> = OnceLock::new();
    static PHONE_REGEX: OnceLock<Option<Regex>> = OnceLock::new();
    static CREDIT_CARD_REGEX: OnceLock<Option<Regex>> = OnceLock::new();
    static SSN_REGEX: OnceLock<Option<Regex>> = OnceLock::new();
    static IP_REGEX: OnceLock<Option<Regex>> = OnceLock::new();
    static API_KEY_REGEX: OnceLock<Option<Regex>> = OnceLock::new();

    let email_re = EMAIL_REGEX.get_or_init(|| Regex::new(r"(?i)[a-z0-9._%+-]+@[a-z0-9.-]+\.[a-z]{2,}").ok());
    let phone_re = PHONE_REGEX.get_or_init(|| {
        Regex::new(r"(\+\d{1,3}[-.\s]?)?\(?\d{3}\)?[-.\s]?\d{3}[-.\s]?\d{4}").ok()
    });
    let credit_card_re = CREDIT_CARD_REGEX.get_or_init(|| {
        Regex::new(r"\b\d{4}[-\s]?\d{4}[-\s]?\d{4}[-\s]?\d{4}\b").ok()
    });
    let ssn_re = SSN_REGEX.get_or_init(|| Regex::new(r"\b\d{3}[-\s]?\d{2}[-\s]?\d{4}\b").ok());
    let ip_re = IP_REGEX.get_or_init(|| {
        Regex::new(r"\b(?:(?:25[0-5]|2[0-4][0-9]|[01]?[0-9][0-9]?)\.){3}(?:25[0-5]|2[0-4][0-9]|[01]?[0-9][0-9]?)\b").ok()
    });
    let api_key_re = API_KEY_REGEX.get_or_init(|| {
        Regex::new(r"(?i)\b(?:sk_live_|ghp_|gho_|glpat-|xoxb-|xoxp-|AKIA|AIza)[a-zA-Z0-9_\-]{20,}\b").ok()
    });

    let mut out: Cow<'_, str> = Cow::Borrowed(text);

    if let Some(re) = email_re.as_ref() {
        out = Cow::Owned(re.replace_all(&out, "[REDACTED_EMAIL]").into_owned());
    }
    if let Some(re) = phone_re.as_ref() {
        out = Cow::Owned(re.replace_all(&out, "[REDACTED_PHONE]").into_owned());
    }
    if let Some(re) = credit_card_re.as_ref() {
        out = Cow::Owned(re.replace_all(&out, "[REDACTED_CARD]").into_owned());
    }
    if let Some(re) = ssn_re.as_ref() {
        out = Cow::Owned(re.replace_all(&out, "[REDACTED_SSN]").into_owned());
    }
    if let Some(re) = ip_re.as_ref() {
        out = Cow::Owned(re.replace_all(&out, "[REDACTED_IP]").into_owned());
    }
    if let Some(re) = api_key_re.as_ref() {
        out = Cow::Owned(re.replace_all(&out, "[REDACTED_API_KEY]").into_owned());
    }

    out.into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_redaction() {
        let _text = "Contact me at test@example.com or 555-0123-4567"; // Invalid phone for this regex but let's try standard
        let text2 = "Contact me at test@example.com or 555-123-4567";
        let redacted = redact_pii(text2);
        assert!(redacted.contains("[REDACTED_EMAIL]"));
        assert!(redacted.contains("[REDACTED_PHONE]"));
        assert!(!redacted.contains("test@example.com"));
    }
}
