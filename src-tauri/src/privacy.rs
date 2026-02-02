//! Privacy settings and consent management
//! Handles user consent for screen capture and data transmission

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

const PRIVACY_FILE: &str = "privacy_settings.json";

/// Autonomy levels for companion actions
/// Controls how much the companion can do without user confirmation
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum AutonomyLevel {
    /// Read-only observer: no actions, only watch and narrate
    #[default]
    Observer,
    /// Suggests actions, user must confirm each one
    Suggester,
    /// Executes with confirmation for high-risk actions only
    Supervised,
    /// Full autonomy within guardrails
    Autonomous,
}

/// Preview policy for side-effect actions
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum PreviewPolicy {
    /// Always show previews for side-effect actions
    #[default]
    Always,
    /// Only show previews for high-risk actions
    HighRisk,
    /// Disable previews (not recommended)
    Off,
}

impl AutonomyLevel {
    /// Check if this level requires confirmation for a given action
    pub fn requires_confirmation(&self, is_high_risk: bool) -> bool {
        match self {
            AutonomyLevel::Observer => true,           // Always blocked anyway
            AutonomyLevel::Suggester => true,          // Always confirm
            AutonomyLevel::Supervised => is_high_risk, // Only high-risk
            AutonomyLevel::Autonomous => false,        // No confirmation (future)
        }
    }

    /// Check if actions are allowed at all
    pub fn allows_actions(&self) -> bool {
        !matches!(self, AutonomyLevel::Observer)
    }

    /// Human-readable description
    pub fn description(&self) -> &'static str {
        match self {
            AutonomyLevel::Observer => "Observer: Watch only, no actions",
            AutonomyLevel::Suggester => "Suggester: Proposes actions, you confirm each",
            AutonomyLevel::Supervised => {
                "Supervised: Auto-executes safe actions, confirms risky ones"
            }
            AutonomyLevel::Autonomous => "Autonomous: Full control within guardrails",
        }
    }
}

/// Privacy settings stored locally
#[derive(Debug, Clone, Serialize, Deserialize)]
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
    /// Autonomy level for companion actions
    #[serde(default)]
    pub autonomy_level: AutonomyLevel,
    /// Redact PII before sending to AI
    #[serde(default)]
    pub redact_pii: bool,
    /// Allow browser data capture (URLs, history, page text)
    #[serde(default)]
    pub browser_content_consent: bool,
    /// Allow browser tab screenshots (captureVisibleTab)
    #[serde(default)]
    pub browser_tab_capture_consent: bool,
    /// Preview policy for side-effect actions
    #[serde(default)]
    pub preview_policy: PreviewPolicy,
    /// Trust profile preset
    #[serde(default)]
    pub trust_profile: String,
    /// Timestamp of consent
    pub consent_timestamp: Option<u64>,
}

impl Default for PrivacySettings {
    fn default() -> Self {
        Self {
            capture_consent: false,
            ai_analysis_consent: false,
            privacy_notice_acknowledged: false,
            read_only_mode: false,
            autonomy_level: AutonomyLevel::Observer,
            redact_pii: true,
            browser_content_consent: false,
            browser_tab_capture_consent: false,
            preview_policy: PreviewPolicy::Always,
            trust_profile: "balanced".to_string(),
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
    autonomy_level: Option<String>,
    redact_pii: Option<bool>,
    browser_content_consent: Option<bool>,
    browser_tab_capture_consent: Option<bool>,
    preview_policy: Option<String>,
    trust_profile: Option<String>,
) -> Result<PrivacySettings, String> {
    let mut settings = PrivacySettings::load();
    settings.capture_consent = capture_consent;
    settings.ai_analysis_consent = ai_analysis_consent;
    settings.privacy_notice_acknowledged = privacy_notice_acknowledged;
    settings.read_only_mode = read_only_mode;

    // Parse autonomy level if provided
    if let Some(level_str) = autonomy_level {
        settings.autonomy_level = match level_str.as_str() {
            "observer" => AutonomyLevel::Observer,
            "suggester" => AutonomyLevel::Suggester,
            "supervised" => AutonomyLevel::Supervised,
            "autonomous" => AutonomyLevel::Autonomous,
            _ => AutonomyLevel::Observer,
        };
    }

    if let Some(redact) = redact_pii {
        settings.redact_pii = redact;
    }

    if let Some(allow_browser) = browser_content_consent {
        settings.browser_content_consent = allow_browser;
    }

    if let Some(allow_tab) = browser_tab_capture_consent {
        settings.browser_tab_capture_consent = allow_tab;
    }

    if let Some(policy) = preview_policy {
        settings.preview_policy = match policy.as_str() {
            "always" => PreviewPolicy::Always,
            "high_risk" => PreviewPolicy::HighRisk,
            "off" => PreviewPolicy::Off,
            _ => PreviewPolicy::Always,
        };
    }

    if let Some(profile) = trust_profile {
        settings.trust_profile = profile;
    }

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
    settings.capture_consent && !settings.read_only_mode
}

/// Check if AI analysis is allowed
#[tauri::command]
pub fn can_analyze_with_ai() -> bool {
    let settings = PrivacySettings::load();
    settings.ai_analysis_consent
}

/// Check if browser content capture is allowed
#[tauri::command]
pub fn can_capture_browser_content() -> bool {
    let settings = PrivacySettings::load();
    settings.browser_content_consent && !settings.read_only_mode
}

/// Check if browser tab screenshots are allowed
#[tauri::command]
pub fn can_capture_browser_tab() -> bool {
    let settings = PrivacySettings::load();
    settings.browser_tab_capture_consent
        && settings.browser_content_consent
        && !settings.read_only_mode
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

2b. BROWSER CONTENT
    - Reads current page URL, title, and visible text
    - Used for puzzle context and assistant suggestions
    - You can disable this anytime in Privacy settings

2c. BROWSER TAB SCREENSHOTS (Optional)
    - Captures only the active browser tab when enabled
    - Requires explicit consent and can be disabled anytime

3. BROWSER ACTIONS
   - The companion can request browser navigation or visual highlights
   - These are cosmetic/gameplay actions only
   - Read-only mode disables all browser actions

4. AI ANALYSIS
   - Screenshots may be sent to Google Gemini for analysis
   - Google's privacy policy applies to AI processing
   - No personal data is stored by this game

5. AUTONOMY LEVELS
   - Observer: Ghost watches and narrates only, no actions taken
   - Suggester: Ghost proposes actions, you confirm each one
   - Supervised: Ghost auto-executes safe actions, confirms risky ones
   - Autonomous: Full control within guardrails (future feature)

RECOMMENDATIONS:
- Close sensitive applications before using screen capture
- Do not capture screens with passwords, banking info, etc.
- Start with Observer or Suggester mode until you trust the system
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

    let email_re =
        EMAIL_REGEX.get_or_init(|| Regex::new(r"(?i)[a-z0-9._%+-]+@[a-z0-9.-]+\.[a-z]{2,}").ok());
    let phone_re = PHONE_REGEX
        .get_or_init(|| Regex::new(r"(\+\d{1,3}[-.\s]?)?\(?\d{3}\)?[-.\s]?\d{3}[-.\s]?\d{4}").ok());
    let credit_card_re = CREDIT_CARD_REGEX
        .get_or_init(|| Regex::new(r"\b\d{4}[-\s]?\d{4}[-\s]?\d{4}[-\s]?\d{4}\b").ok());
    let ssn_re = SSN_REGEX.get_or_init(|| Regex::new(r"\b\d{3}[-\s]?\d{2}[-\s]?\d{4}\b").ok());
    let ip_re = IP_REGEX.get_or_init(|| {
        Regex::new(r"\b(?:(?:25[0-5]|2[0-4][0-9]|[01]?[0-9][0-9]?)\.){3}(?:25[0-5]|2[0-4][0-9]|[01]?[0-9][0-9]?)\b").ok()
    });
    let api_key_re = API_KEY_REGEX.get_or_init(|| {
        Regex::new(
            r"(?i)\b(?:sk_live_|ghp_|gho_|glpat-|xoxb-|xoxp-|AKIA|AIza)[a-zA-Z0-9_\-]{20,}\b",
        )
        .ok()
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

/// Conditionally redact PII based on user settings
pub fn maybe_redact_pii(text: &str, enabled: bool) -> String {
    if enabled {
        redact_pii(text)
    } else {
        text.to_string()
    }
}

/// Convenience helper: redact using current privacy settings
pub fn redact_with_settings(text: &str) -> String {
    let settings = PrivacySettings::load();
    maybe_redact_pii(text, settings.redact_pii)
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
