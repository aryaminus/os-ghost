//! App Context Detection
//!
//! Detects when user switches between applications (metadata only - no content).
//! Tracks app names and categories for context-aware suggestions.
//! Privacy-first: never captures app content, only app identity.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

/// Categories of applications for context understanding
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum AppCategory {
    Browser,
    CodeEditor,
    Communication, // Slack, Teams, Discord
    Document,      // PDF, Word, Notes
    Media,         // Video, Music players
    System,        // Finder, Settings, Terminal
    Creative,      // Photoshop, Figma, Design tools
    Productivity,  // Calendar, Email, Task managers
    Game,
    Other,
}

impl AppCategory {
    /// Get human-readable description
    pub fn description(&self) -> &'static str {
        match self {
            AppCategory::Browser => "Web Browser",
            AppCategory::CodeEditor => "Code Editor",
            AppCategory::Communication => "Communication",
            AppCategory::Document => "Document Viewer",
            AppCategory::Media => "Media Player",
            AppCategory::System => "System Application",
            AppCategory::Creative => "Creative Tool",
            AppCategory::Productivity => "Productivity App",
            AppCategory::Game => "Game",
            AppCategory::Other => "Application",
        }
    }
}

/// Information about the currently active application
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppContext {
    /// Name of the active application (e.g., "Google Chrome", "Visual Studio Code")
    pub app_name: String,
    /// Bundle ID or executable name (for identification)
    pub app_identifier: String,
    /// Category of the application
    pub category: AppCategory,
    /// Time spent in this app so far (continuously updated)
    pub time_in_app: Duration,
    /// Previous active app (if any)
    pub previous_app: Option<String>,
    /// Timestamp when user switched to this app
    pub switch_timestamp: u64,
    /// Platform-specific process ID (optional)
    pub process_id: Option<u32>,
}

impl AppContext {
    /// Create a new app context
    pub fn new(app_name: String, app_identifier: String, category: AppCategory) -> Self {
        Self {
            app_name,
            app_identifier,
            category,
            time_in_app: Duration::from_secs(0),
            previous_app: None,
            switch_timestamp: current_timestamp_secs(),
            process_id: None,
        }
    }

    /// Update time spent in app
    pub fn update_duration(&mut self) {
        let now = current_timestamp_secs();
        self.time_in_app = Duration::from_secs(now - self.switch_timestamp);
    }

    /// Check if this is a "work" category app
    pub fn is_work_related(&self) -> bool {
        matches!(
            self.category,
            AppCategory::CodeEditor
                | AppCategory::Productivity
                | AppCategory::Document
                | AppCategory::Browser
        )
    }

    /// Check if this is a "communication" app
    pub fn is_communication(&self) -> bool {
        matches!(self.category, AppCategory::Communication)
    }
}

/// App context detector - monitors active application changes
pub struct AppContextDetector {
    /// Current app context
    current_context: Arc<Mutex<Option<AppContext>>>,
    /// History of app switches (last N switches)
    switch_history: Arc<Mutex<Vec<AppSwitchEvent>>>,
    /// App category mappings (bundle_id -> category)
    app_categories: Arc<Mutex<HashMap<String, AppCategory>>>,
    /// Maximum history size
    max_history: usize,
    /// Last detection time
    last_detection: Arc<Mutex<Instant>>,
    /// Detection interval
    detection_interval: Duration,
}

/// Record of an app switch event
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppSwitchEvent {
    pub from_app: Option<String>,
    pub to_app: String,
    pub to_category: AppCategory,
    pub timestamp: u64,
    pub duration_in_previous: Option<Duration>,
}

impl AppContextDetector {
    /// Create a new app context detector
    pub fn new() -> Self {
        Self {
            current_context: Arc::new(Mutex::new(None)),
            switch_history: Arc::new(Mutex::new(Vec::new())),
            app_categories: Arc::new(Mutex::new(default_app_categories())),
            max_history: 100,
            last_detection: Arc::new(Mutex::new(Instant::now())),
            detection_interval: Duration::from_secs(2), // Check every 2 seconds
        }
    }

    /// Start monitoring app switches
    pub async fn start_monitoring(&self) {
        tracing::info!("Starting app context monitoring");

        loop {
            tokio::time::sleep(self.detection_interval).await;

            match self.detect_active_app().await {
                Ok(Some(new_context)) => {
                    self.handle_app_switch(new_context).await;
                }
                Ok(None) => {
                    // No app detected (possibly idle)
                }
                Err(e) => {
                    tracing::debug!("App detection error: {}", e);
                    // Don't spam logs - detection might not be available
                }
            }

            // Update last detection time
            if let Ok(mut last) = self.last_detection.lock() {
                *last = Instant::now();
            }
        }
    }

    /// Detect currently active application (platform-specific)
    async fn detect_active_app(&self) -> Result<Option<AppContext>, String> {
        #[cfg(target_os = "macos")]
        {
            self.detect_macos().await
        }

        #[cfg(target_os = "windows")]
        {
            self.detect_windows().await
        }

        #[cfg(target_os = "linux")]
        {
            self.detect_linux().await
        }

        #[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
        {
            Err("App context detection not supported on this platform".to_string())
        }
    }

    /// macOS: Use NSWorkspace to get frontmost application
    #[cfg(target_os = "macos")]
    async fn detect_macos(&self) -> Result<Option<AppContext>, String> {
        // Use AppleScript to get frontmost app
        let output = tokio::process::Command::new("osascript")
            .args([
                "-e",
                "tell application \"System Events\" to get name of first application process whose frontmost is true",
            ])
            .output()
            .await
            .map_err(|e| format!("Failed to execute AppleScript: {}", e))?;

        if !output.status.success() {
            return Err("AppleScript execution failed".to_string());
        }

        let app_name = String::from_utf8_lossy(&output.stdout)
            .trim()
            .to_string();

        if app_name.is_empty() {
            return Ok(None);
        }

        // Get bundle identifier
        let bundle_output = tokio::process::Command::new("osascript")
            .args([
                "-e",
                &format!(
                    "tell application \"System Events\" to get bundle identifier of application process \"{}\"",
                    app_name
                ),
            ])
            .output()
            .await;

        let app_identifier = match bundle_output {
            Ok(output) if output.status.success() => {
                String::from_utf8_lossy(&output.stdout).trim().to_string()
            }
            _ => app_name.to_lowercase().replace(" ", "."),
        };

        // Determine category
        let category = self.categorize_app(&app_name, &app_identifier);

        Ok(Some(AppContext::new(app_name, app_identifier, category)))
    }

    /// Windows: Use Windows API to get foreground window process
    #[cfg(target_os = "windows")]
    async fn detect_windows(&self) -> Result<Option<AppContext>, String> {
        // Use PowerShell to get foreground window process
        let output = tokio::process::Command::new("powershell")
            .args(&[
                "-Command",
                "Get-Process | Where-Object {$_.MainWindowTitle -ne ''} | Sort-Object -Property MainWindowHandle -Descending | Select-Object -First 1 -ExpandProperty ProcessName",
            ])
            .output()
            .await
            .map_err(|e| format!("Failed to execute PowerShell: {}", e))?;

        if !output.status.success() {
            return Err("PowerShell execution failed".to_string());
        }

        let app_name = String::from_utf8_lossy(&output.stdout)
            .trim()
            .to_string();

        if app_name.is_empty() {
            return Ok(None);
        }

        let app_identifier = app_name.to_lowercase();
        let category = self.categorize_app(&app_name, &app_identifier);

        Ok(Some(AppContext::new(app_name, app_identifier, category)))
    }

    /// Linux: Use xprop or similar to get active window
    #[cfg(target_os = "linux")]
    async fn detect_linux(&self) -> Result<Option<AppContext>, String> {
        // Try xprop first (X11)
        let output = tokio::process::Command::new("xprop")
            .args(&["-id", "$(xprop -root _NET_ACTIVE_WINDOW | cut -d ' ' -f 5)", "WM_CLASS"])
            .output()
            .await;

        let (app_name, app_identifier) = match output {
            Ok(output) if output.status.success() => {
                let result = String::from_utf8_lossy(&output.stdout);
                // Parse WM_CLASS output
                let parts: Vec<&str> = result.split('"').collect();
                if parts.len() >= 3 {
                    (parts[1].to_string(), parts[1].to_lowercase())
                } else {
                    ("unknown".to_string(), "unknown".to_string())
                }
            }
            _ => {
                // Fallback: try using pgrep or similar
                ("unknown".to_string(), "unknown".to_string())
            }
        };

        if app_name == "unknown" {
            return Ok(None);
        }

        let category = self.categorize_app(&app_name, &app_identifier);
        Ok(Some(AppContext::new(app_name, app_identifier, category)))
    }

    /// Handle app switch detection
    async fn handle_app_switch(&self, new_context: AppContext) {
        let mut current = self.current_context.lock().unwrap();

        // Check if app actually changed
        if let Some(ref current_ctx) = *current {
            if current_ctx.app_name == new_context.app_name {
                // Same app, just update duration
                if let Some(ref mut ctx) = *current {
                    ctx.update_duration();
                }
                return;
            }

            // App changed - record the switch
            let event = AppSwitchEvent {
                from_app: Some(current_ctx.app_name.clone()),
                to_app: new_context.app_name.clone(),
                to_category: new_context.category,
                timestamp: current_timestamp_secs(),
                duration_in_previous: Some(current_ctx.time_in_app),
            };

            self.record_switch_event(event);
        } else {
            // First app detected
            let event = AppSwitchEvent {
                from_app: None,
                to_app: new_context.app_name.clone(),
                to_category: new_context.category,
                timestamp: current_timestamp_secs(),
                duration_in_previous: None,
            };

            self.record_switch_event(event);
        }

        // Update current context
        *current = Some(new_context);
    }

    /// Record an app switch event
    fn record_switch_event(&self, event: AppSwitchEvent) {
        let mut history = self.switch_history.lock().unwrap();

        history.push(event);

        // Trim history if too large
        if history.len() > self.max_history {
            history.remove(0);
        }

        tracing::debug!(
            "App switch: {} -> {}",
            history.last().unwrap().from_app.as_deref().unwrap_or("None"),
            history.last().unwrap().to_app
        );
    }

    /// Categorize an app based on name and identifier
    fn categorize_app(&self, app_name: &str, app_identifier: &str) -> AppCategory {
        // Check custom mappings first
        let categories = self.app_categories.lock().unwrap();
        if let Some(&category) = categories.get(app_identifier) {
            return category;
        }

        // Fall back to heuristics
        categorize_by_name(app_name)
    }

    /// Get current app context
    pub fn get_current_context(&self) -> Option<AppContext> {
        self.current_context.lock().unwrap().clone()
    }

    /// Get recent app switch history
    pub fn get_switch_history(&self, limit: usize) -> Vec<AppSwitchEvent> {
        let history = self.switch_history.lock().unwrap();
        history.iter().rev().take(limit).cloned().collect()
    }

    /// Check if user is currently idle (no app switches recently)
    pub fn is_user_idle(&self, threshold_secs: u64) -> bool {
        if let Ok(last) = self.last_detection.lock() {
            last.elapsed().as_secs() > threshold_secs
        } else {
            false
        }
    }

    /// Add custom app category mapping
    pub fn set_app_category(&self, app_identifier: String, category: AppCategory) {
        let mut categories = self.app_categories.lock().unwrap();
        categories.insert(app_identifier, category);
    }
}

impl Default for AppContextDetector {
    fn default() -> Self {
        Self::new()
    }
}

/// Default app category mappings
fn default_app_categories() -> HashMap<String, AppCategory> {
    let mut map = HashMap::new();

    // Browsers
    map.insert("com.google.Chrome".to_string(), AppCategory::Browser);
    map.insert("com.apple.Safari".to_string(), AppCategory::Browser);
    map.insert("org.mozilla.firefox".to_string(), AppCategory::Browser);
    map.insert("com.microsoft.Edge".to_string(), AppCategory::Browser);
    map.insert("com.brave.Browser".to_string(), AppCategory::Browser);
    map.insert("com.vivaldi.Vivaldi".to_string(), AppCategory::Browser);

    // Code Editors
    map.insert("com.microsoft.VSCode".to_string(), AppCategory::CodeEditor);
    map.insert("com.apple.Xcode".to_string(), AppCategory::CodeEditor);
    map.insert("com.jetbrains.intellij".to_string(), AppCategory::CodeEditor);
    map.insert("com.sublimetext.4".to_string(), AppCategory::CodeEditor);
    map.insert("com.github.atom".to_string(), AppCategory::CodeEditor);
    map.insert("com.cursor.Cursor".to_string(), AppCategory::CodeEditor);

    // Communication
    map.insert("com.tinyspeck.slackmacgap".to_string(), AppCategory::Communication);
    map.insert("com.microsoft.teams".to_string(), AppCategory::Communication);
    map.insert("com.hnc.Discord".to_string(), AppCategory::Communication);
    map.insert("us.zoom.xos".to_string(), AppCategory::Communication);

    // Productivity
    map.insert("com.apple.mail".to_string(), AppCategory::Productivity);
    map.insert("com.microsoft.Outlook".to_string(), AppCategory::Productivity);
    map.insert("com.apple.iCal".to_string(), AppCategory::Productivity);
    map.insert("com.notion.id".to_string(), AppCategory::Productivity);
    map.insert("com.atlassian.trello".to_string(), AppCategory::Productivity);

    // System
    map.insert("com.apple.finder".to_string(), AppCategory::System);
    map.insert("com.apple.systempreferences".to_string(), AppCategory::System);
    map.insert("com.apple.Terminal".to_string(), AppCategory::System);

    map
}

/// Categorize app by name heuristics
fn categorize_by_name(app_name: &str) -> AppCategory {
    let name_lower = app_name.to_lowercase();

    if name_lower.contains("chrome")
        || name_lower.contains("safari")
        || name_lower.contains("firefox")
        || name_lower.contains("edge")
    {
        AppCategory::Browser
    } else if name_lower.contains("code")
        || name_lower.contains("xcode")
        || name_lower.contains("intellij")
        || name_lower.contains("sublime")
        || name_lower.contains("atom")
        || name_lower.contains("cursor")
        || name_lower.contains("vim")
        || name_lower.contains("emacs")
    {
        AppCategory::CodeEditor
    } else if name_lower.contains("slack")
        || name_lower.contains("discord")
        || name_lower.contains("teams")
        || name_lower.contains("zoom")
        || name_lower.contains("skype")
    {
        AppCategory::Communication
    } else if name_lower.contains("mail")
        || name_lower.contains("outlook")
        || name_lower.contains("calendar")
        || name_lower.contains("notion")
        || name_lower.contains("trello")
    {
        AppCategory::Productivity
    } else if name_lower.contains("finder")
        || name_lower.contains("terminal")
        || name_lower.contains("settings")
        || name_lower.contains("system")
    {
        AppCategory::System
    } else if name_lower.contains("photo")
        || name_lower.contains("design")
        || name_lower.contains("figma")
        || name_lower.contains("sketch")
        || name_lower.contains("photoshop")
    {
        AppCategory::Creative
    } else if name_lower.contains("music")
        || name_lower.contains("video")
        || name_lower.contains("spotify")
        || name_lower.contains("vlc")
    {
        AppCategory::Media
    } else if name_lower.contains("game")
        || name_lower.contains("steam")
    {
        AppCategory::Game
    } else {
        AppCategory::Other
    }
}

/// Get current timestamp in seconds
fn current_timestamp_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_app_category_description() {
        assert_eq!(AppCategory::Browser.description(), "Web Browser");
        assert_eq!(AppCategory::CodeEditor.description(), "Code Editor");
    }

    #[test]
    fn test_categorize_by_name() {
        assert_eq!(categorize_by_name("Google Chrome"), AppCategory::Browser);
        assert_eq!(categorize_by_name("Visual Studio Code"), AppCategory::CodeEditor);
        assert_eq!(categorize_by_name("Slack"), AppCategory::Communication);
    }

    #[test]
    fn test_app_context_creation() {
        let ctx = AppContext::new(
            "Google Chrome".to_string(),
            "com.google.Chrome".to_string(),
            AppCategory::Browser,
        );

        assert_eq!(ctx.app_name, "Google Chrome");
        assert!(ctx.is_work_related());
        assert!(!ctx.is_communication());
    }
}
