//! IPC commands for Tauri frontend-backend communication
//! Exposes Rust functionality to JavaScript via Tauri commands

use crate::ai_provider::SmartAiRouter;
use crate::capture;
use crate::game_state::{EffectMessage, EffectQueue};
use crate::utils::current_timestamp_millis;
use anyhow::Result;
use rand::Rng;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tauri::{Emitter, Manager, State, WebviewUrl, WebviewWindowBuilder};
use tauri_plugin_opener::OpenerExt;

pub mod puzzles;
pub use puzzles::{
    generate_puzzle_from_history, start_investigation, GeneratedPuzzle, HistoryItem, Puzzle,
    TopSite,
};

// ============================================================================
// System Detection Types & Commands
// ============================================================================

/// System detection status for browser and extension
#[derive(Debug, Serialize, Clone, Default)]
pub struct SystemStatus {
    /// Chrome/Chromium installation path (if found)
    pub chrome_path: Option<String>,
    /// Chrome is installed
    pub chrome_installed: bool,
    /// Extension connection status (connected to TCP bridge)
    pub extension_connected: bool,
    /// Extension is operational (responding to messages)
    pub extension_operational: bool,
    /// Last heartbeat timestamp (unix seconds)
    pub last_extension_heartbeat: Option<u64>,
    /// Last hello timestamp (unix seconds)
    pub last_extension_hello: Option<u64>,
    /// Extension protocol version
    pub extension_protocol_version: Option<String>,
    /// Extension version
    pub extension_version: Option<String>,
    /// Extension ID
    pub extension_id: Option<String>,
    /// Extension capabilities
    pub extension_capabilities: Option<serde_json::Value>,
    /// MCP browser connection state
    pub mcp_browser_connected: bool,
    /// Last page update timestamp
    pub last_page_update: Option<u64>,
    /// API key configured
    pub api_key_configured: bool,
    /// Source of the API key ("env", "user", "none")
    pub api_key_source: String,
    /// Last known browsing URL (from extension or history)
    pub last_known_url: Option<String>,
    /// Last screenshot timestamp
    pub last_screenshot_at: Option<u64>,
    /// Active AI provider
    pub active_provider: Option<String>,
    /// Current app mode
    pub current_mode: String,
    /// Preferred app mode
    pub preferred_mode: String,
    /// Auto-create puzzles from companion suggestions
    pub auto_puzzle_from_companion: bool,
}

/// Aggregated settings payload for Settings window
#[derive(Debug, Serialize, Clone)]
pub struct SettingsState {
    pub privacy: crate::privacy::PrivacySettings,
    pub privacy_notice: String,
    pub system_status: SystemStatus,
    pub autonomy_settings: AutonomySettings,
    pub intelligent_mode: IntelligentModeStatus,
    pub sandbox_settings: crate::mcp::sandbox::SandboxConfig,
    pub system_settings: crate::system_settings::SystemSettings,
    pub capture_settings: crate::capture::CaptureSettings,
    pub scheduler_settings: crate::scheduler::SchedulerSettings,
    pub pairing_status: crate::pairing::PairingState,
    pub permission_diagnostics: crate::permissions::PermissionDiagnostics,
    pub recent_timeline: Vec<crate::timeline::TimelineEntry>,
    pub calendar_settings: crate::integrations::CalendarSettings,
    pub notes: Vec<crate::integrations::Note>,
    pub files_settings: crate::integrations::FilesSettings,
    pub email_settings: crate::integrations::EmailSettings,
}

#[derive(Debug, Serialize, Clone)]
pub struct HealthReport {
    pub timestamp: u64,
    pub system_status: SystemStatus,
    pub permissions: crate::permissions::PermissionDiagnostics,
}

fn mode_to_string(mode: crate::memory::AppMode) -> String {
    match mode {
        crate::memory::AppMode::Game => "game".to_string(),
        crate::memory::AppMode::Companion => "companion".to_string(),
    }
}

pub(crate) fn detect_system_status(session: Option<&crate::memory::SessionMemory>) -> SystemStatus {
    let status_snapshot = crate::system_status::get_status_snapshot();
    let chrome_path = find_chrome_path();
    let chrome_installed = chrome_path.is_some();
    let runtime = crate::utils::runtime_config();
    let api_key_configured = runtime.has_api_key();
    let api_key_source = if runtime.is_using_user_key() {
        "user".to_string()
    } else if std::env::var("GEMINI_API_KEY").is_ok() {
        "env".to_string()
    } else {
        "none".to_string()
    };

    let session_state = session.and_then(|s| s.load().ok());

    let (current_mode, preferred_mode, auto_puzzle_from_companion) = session_state
        .as_ref()
        .map(|state| {
            (
                mode_to_string(state.current_mode.clone()),
                mode_to_string(state.preferred_mode.clone()),
                state.auto_puzzle_from_companion,
            )
        })
        .unwrap_or_else(|| ("companion".to_string(), "companion".to_string(), true));

    let last_known_url = status_snapshot
        .as_ref()
        .and_then(|s| s.last_known_url.clone())
        .or_else(|| session_state.as_ref().map(|s| s.current_url.clone()))
        .filter(|url| !url.is_empty());

    let last_screenshot_at = session_state.as_ref().map(|s| s.last_screenshot_at).filter(|v| *v > 0);

    SystemStatus {
        chrome_path,
        chrome_installed,
        extension_connected: status_snapshot
            .as_ref()
            .map(|s| s.extension_connected)
            .unwrap_or(false),
        extension_operational: status_snapshot
            .as_ref()
            .map(|s| s.extension_operational)
            .unwrap_or(false),
        last_extension_heartbeat: status_snapshot
            .as_ref()
            .and_then(|s| s.last_extension_heartbeat),
        extension_protocol_version: status_snapshot
            .as_ref()
            .and_then(|s| s.extension_protocol_version.clone()),
        extension_version: status_snapshot
            .as_ref()
            .and_then(|s| s.extension_version.clone()),
        extension_id: status_snapshot.as_ref().and_then(|s| s.extension_id.clone()),
        extension_capabilities: status_snapshot
            .as_ref()
            .and_then(|s| s.extension_capabilities.clone()),
        last_extension_hello: status_snapshot
            .as_ref()
            .and_then(|s| s.last_extension_hello),
        mcp_browser_connected: status_snapshot
            .as_ref()
            .map(|s| s.mcp_browser_connected)
            .unwrap_or(false),
        last_page_update: status_snapshot.as_ref().and_then(|s| s.last_page_update),
        api_key_configured,
        api_key_source,
        last_known_url,
        last_screenshot_at,
        active_provider: status_snapshot
            .as_ref()
            .and_then(|s| s.active_provider.clone()),
        current_mode,
        preferred_mode,
        auto_puzzle_from_companion,
    }
}

pub fn emit_system_status_update(app: &tauri::AppHandle) {
    let session = app.state::<Arc<crate::memory::SessionMemory>>();
    let status = detect_system_status(Some(session.as_ref()));
    let _ = app.emit("system_status_update", status);
}

#[tauri::command]
pub async fn health_check(
    session: State<'_, Arc<crate::memory::SessionMemory>>,
) -> Result<HealthReport, String> {
    let system_status = detect_system_status(Some(session.as_ref()));
    let permissions = crate::permissions::get_permission_diagnostics().await;
    Ok(HealthReport {
        timestamp: crate::utils::current_timestamp(),
        system_status,
        permissions,
    })
}

/// Detect Chrome/Chromium browser installation
#[tauri::command]
pub fn detect_chrome(session: State<'_, Arc<crate::memory::SessionMemory>>) -> SystemStatus {
    detect_system_status(Some(session.as_ref()))
}

/// Open the System Settings window and optionally navigate to a section
#[tauri::command]
pub fn open_settings(section: Option<String>, app: tauri::AppHandle) -> Result<(), String> {
    let label = "settings";
    let window = if let Some(existing) = app.get_webview_window(label) {
        existing
    } else {
        let target = if let Some(ref section) = section {
            format!("settings.html?section={}", section)
        } else {
            "settings.html".to_string()
        };
        WebviewWindowBuilder::new(&app, label, WebviewUrl::App(target.into()))
            .title("System Settings")
            .inner_size(980.0, 720.0)
            .min_inner_size(820.0, 600.0)
            .resizable(true)
            .build()
            .map_err(|e| e.to_string())?
    };

    let _ = window.show();
    let _ = window.set_focus();

    if let Some(section) = section {
        let _ = window.emit(
            "settings:navigate",
            serde_json::json!({ "section": section }),
        );
    }

    Ok(())
}

/// Aggregate settings state for Settings UI
#[tauri::command]
pub async fn get_settings_state(
    orchestrator: State<'_, Arc<crate::agents::AgentOrchestrator>>,
    session: State<'_, Arc<crate::memory::SessionMemory>>,
    notes_store: State<'_, Arc<crate::integrations::NotesStore>>,
) -> Result<SettingsState, String> {
    let privacy = crate::privacy::PrivacySettings::load();
    let privacy_notice = crate::privacy::PRIVACY_NOTICE.to_string();
    let system_status = detect_system_status(Some(session.as_ref()));
    let autonomy_settings = AutonomySettings {
        auto_puzzle_from_companion: session
            .get_auto_puzzle_from_companion()
            .map_err(|e| e.to_string())?,
    };
    let intelligent_mode = IntelligentModeStatus {
        intelligent_mode: orchestrator.use_intelligent_mode(),
        reflection: orchestrator.use_reflection(),
        guardrails: orchestrator.use_guardrails(),
    };
    let sandbox_settings = crate::mcp::sandbox::get_sandbox_settings();
    let system_settings = crate::system_settings::SystemSettings::load();
    let capture_settings = crate::capture::CaptureSettings::load();
    let scheduler_settings = crate::scheduler::SchedulerSettings::load();
    let pairing_status = crate::pairing::get_pairing_status();
    let permission_diagnostics = crate::permissions::get_permission_diagnostics().await;
    let recent_timeline = crate::timeline::get_recent_timeline(5);
    let calendar_settings = crate::integrations::CalendarSettings::load();
    let notes = notes_store.list_notes().unwrap_or_default();
    let files_settings = crate::integrations::FilesSettings::load();
    let email_settings = crate::integrations::EmailSettings::load();

    Ok(SettingsState {
        privacy,
        privacy_notice,
        system_status,
        autonomy_settings,
        intelligent_mode,
        sandbox_settings,
        system_settings,
        capture_settings,
        scheduler_settings,
        pairing_status,
        permission_diagnostics,
        recent_timeline,
        calendar_settings,
        notes,
        files_settings,
        email_settings,
    })
}

/// Open an external URL in the system default browser
#[tauri::command]
pub fn open_external_url(url: String, app: tauri::AppHandle) -> Result<(), String> {
    let normalized = url.trim();
    let allowed = normalized.starts_with("https://")
        || normalized.starts_with("http://")
        || normalized.starts_with("x-apple.systempreferences:")
        || normalized.starts_with("ms-settings:");
    if !allowed {
        return Err("Unsupported URL scheme".to_string());
    }
    app.opener()
        .open_url(normalized, None::<String>)
        .map_err(|e| e.to_string())
}

/// Get current app mode ("game" or "companion")
#[tauri::command]
pub fn get_app_mode(
    session: State<'_, Arc<crate::memory::SessionMemory>>,
) -> Result<String, String> {
    session
        .get_mode()
        .map(mode_to_string)
        .map_err(|e| e.to_string())
}

/// Set app mode ("game" or "companion")
#[tauri::command]
pub fn set_app_mode(
    mode: String,
    persist_preference: Option<bool>,
    session: State<'_, Arc<crate::memory::SessionMemory>>,
) -> Result<String, String> {
    let normalized = mode.trim().to_lowercase();
    let parsed = match normalized.as_str() {
        "game" => crate::memory::AppMode::Game,
        "companion" => crate::memory::AppMode::Companion,
        _ => return Err("Invalid mode. Expected 'game' or 'companion'".to_string()),
    };

    if persist_preference.unwrap_or(true) {
        session
            .set_preferred_mode(parsed.clone())
            .map_err(|e| e.to_string())?;
    }

    session.set_mode(parsed).map_err(|e| e.to_string())?;
    get_app_mode(session)
}

#[derive(Debug, Serialize, Clone)]
pub struct AutonomySettings {
    pub auto_puzzle_from_companion: bool,
}

/// Get autonomy settings
#[tauri::command]
pub fn get_autonomy_settings(
    session: State<'_, Arc<crate::memory::SessionMemory>>,
) -> Result<AutonomySettings, String> {
    session
        .get_auto_puzzle_from_companion()
        .map(|v| AutonomySettings {
            auto_puzzle_from_companion: v,
        })
        .map_err(|e| e.to_string())
}

/// Set autonomy settings
#[tauri::command]
pub fn set_autonomy_settings(
    auto_puzzle_from_companion: bool,
    session: State<'_, Arc<crate::memory::SessionMemory>>,
) -> Result<AutonomySettings, String> {
    session
        .set_auto_puzzle_from_companion(auto_puzzle_from_companion)
        .map_err(|e| e.to_string())?;

    get_autonomy_settings(session)
}

/// Find Chrome/Chromium installation path based on platform (Cached)
fn find_chrome_path() -> Option<String> {
    static CHROME_PATH: std::sync::OnceLock<Option<String>> = std::sync::OnceLock::new();

    CHROME_PATH
        .get_or_init(|| {
            #[cfg(target_os = "macos")]
            {
                // Prefer app bundle paths so `open -a <path>` works reliably
                let paths = [
                    "/Applications/Google Chrome.app",
                    "/Applications/Chromium.app",
                    "/Applications/Google Chrome Canary.app",
                    "/Applications/Brave Browser.app",
                    "/Applications/Microsoft Edge.app",
                    "/Applications/Arc.app",
                ];
                for path in paths {
                    if std::path::Path::new(path).exists() {
                        return Some(path.to_string());
                    }
                }
            }

            #[cfg(target_os = "windows")]
            {
                let paths = [
                    r"C:\Program Files\Google\Chrome\Application\chrome.exe",
                    r"C:\Program Files (x86)\Google\Chrome\Application\chrome.exe",
                    r"C:\Program Files\Chromium\Application\chrome.exe",
                    r"C:\Program Files\BraveSoftware\Brave-Browser\Application\brave.exe",
                    r"C:\Program Files (x86)\Microsoft\Edge\Application\msedge.exe",
                ];
                for path in paths {
                    if std::path::Path::new(path).exists() {
                        return Some(path.to_string());
                    }
                }
                // Also check user-specific installs
                if let Ok(local_app_data) = std::env::var("LOCALAPPDATA") {
                    let user_chrome =
                        format!(r"{}\Google\Chrome\Application\chrome.exe", local_app_data);
                    if std::path::Path::new(&user_chrome).exists() {
                        return Some(user_chrome);
                    }
                }
            }

            #[cfg(target_os = "linux")]
            {
                use std::process::Command;
                // Try which command first
                let commands = [
                    "google-chrome",
                    "chromium",
                    "chromium-browser",
                    "brave-browser",
                ];
                for cmd in commands {
                    if let Ok(output) = Command::new("which").arg(cmd).output() {
                        if output.status.success() {
                            let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
                            if !path.is_empty() {
                                return Some(path);
                            }
                        }
                    }
                }
                // Fallback to common paths
                let paths = [
                    "/usr/bin/google-chrome",
                    "/usr/bin/chromium",
                    "/usr/bin/chromium-browser",
                    "/snap/bin/chromium",
                ];
                for path in paths {
                    if std::path::Path::new(path).exists() {
                        return Some(path.to_string());
                    }
                }
            }

            None
        })
        .clone()
}

/// Launch Chrome browser with optional URL
#[tauri::command]
pub async fn launch_chrome(url: Option<String>) -> Result<(), String> {
    let chrome_path = find_chrome_path().ok_or("Chrome not found")?;

    #[cfg(target_os = "macos")]
    {
        let mut cmd = std::process::Command::new("open");
        cmd.arg("-a").arg(&chrome_path);
        if let Some(u) = url {
            cmd.arg(u);
        }
        cmd.spawn()
            .map_err(|e| format!("Failed to launch Chrome: {}", e))?;
    }

    #[cfg(target_os = "windows")]
    {
        let mut cmd = std::process::Command::new(&chrome_path);
        if let Some(u) = url {
            cmd.arg(u);
        }
        cmd.spawn()
            .map_err(|e| format!("Failed to launch Chrome: {}", e))?;
    }

    #[cfg(target_os = "linux")]
    {
        let mut cmd = std::process::Command::new(&chrome_path);
        if let Some(u) = url {
            cmd.arg(u);
        }
        cmd.spawn()
            .map_err(|e| format!("Failed to launch Chrome: {}", e))?;
    }

    Ok(())
}

// ============================================================================
// Adaptive Behavior Commands
// ============================================================================

/// Helper function to convert ActivityEntry to ActivityContext
fn activity_to_context(
    entry: &crate::memory::ActivityEntry,
) -> Option<crate::gemini_client::ActivityContext> {
    let metadata = entry.metadata.as_ref()?;
    Some(crate::gemini_client::ActivityContext {
        app_name: metadata
            .get("app_name")
            .and_then(|v| v.as_str())
            .unwrap_or("Unknown")
            .to_string(),
        app_category: metadata
            .get("app_category")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown")
            .to_string(),
        description: entry.description.clone(),
        content_context: metadata
            .get("content_context")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string()),
    })
}

/// Generate an adaptive puzzle based on user's activity history
///
/// Unlike the raw AI call, this registers the puzzle in the backend puzzle list
/// so verification + agent cycles work end-to-end.
#[tauri::command]
pub async fn generate_adaptive_puzzle(
    orchestrator: State<'_, Arc<crate::agents::AgentOrchestrator>>,
    ai_router: State<'_, Arc<SmartAiRouter>>,
    puzzles: State<'_, std::sync::RwLock<Vec<Puzzle>>>,
    session: State<'_, Arc<crate::memory::SessionMemory>>,
) -> Result<GeneratedPuzzle, String> {
    // Get recent activity from session
    let activities = orchestrator
        .get_recent_activity(10)
        .map_err(|e| e.to_string())?;

    // Convert to ActivityContext format using helper
    let activity_contexts: Vec<crate::gemini_client::ActivityContext> =
        activities.iter().filter_map(activity_to_context).collect();

    if activity_contexts.is_empty() {
        return Err("No activity history available. Wait for some observations.".to_string());
    }

    // Get current app/content from most recent activity
    let latest = activity_contexts.first();
    let current_app = latest.map(|a| a.app_name.as_str());
    let current_content = latest.and_then(|a| a.content_context.as_deref());

    let adaptive = ai_router
        .generate_adaptive_puzzle(&activity_contexts, current_app, current_content)
        .await
        .map_err(|e| e.to_string())?;

    // Register as a normal puzzle so downstream flows (verify, agent cycles) work.
    let id = crate::ipc::puzzles::generate_puzzle_id("adaptive");

    let puzzle = Puzzle {
        id: id.clone(),
        clue: adaptive.clue.clone(),
        hint: adaptive.hints.first().cloned().unwrap_or_default(),
        target_url_pattern: adaptive.target_url_pattern.clone(),
        target_description: adaptive.target_description.clone(),
        sponsor_id: None,
        sponsor_url: None,
        is_sponsored: false,
    };

    {
        let mut puzzles = puzzles.write().map_err(|e| format!("Lock error: {}", e))?;
        while puzzles
            .iter()
            .filter(|p| p.id.starts_with("adaptive_"))
            .count()
            >= 5
        {
            if let Some(idx) = puzzles.iter().position(|p| p.id.starts_with("adaptive_")) {
                puzzles.remove(idx);
            } else {
                break;
            }
        }
        puzzles.push(puzzle);
    }

    // Start timer for the new puzzle
    let mut state = crate::game_state::GameState::load().await;
    state.start_puzzle_timer().await;

    // Update session
    if let Ok(mut s) = session.load() {
        s.puzzle_id = id.clone();
        let _ = session.save(&s);
    }

    Ok(GeneratedPuzzle {
        id,
        clue: adaptive.clue,
        hint: adaptive.hints.first().cloned().unwrap_or_default(),
        hints: adaptive.hints,
        target_url_pattern: adaptive.target_url_pattern,
        target_description: adaptive.target_description,
        is_sponsored: false,
        sponsor_id: None,
    })
}

/// Generate contextual dialogue based on observation history
#[tauri::command]
pub async fn generate_contextual_dialogue(
    context: String,
    mood: String,
    orchestrator: State<'_, Arc<crate::agents::AgentOrchestrator>>,
    ai_router: State<'_, Arc<SmartAiRouter>>,
) -> Result<String, String> {
    // Get recent activity
    let activities = orchestrator
        .get_recent_activity(5)
        .map_err(|e| e.to_string())?;

    // Convert to ActivityContext using helper
    let activity_contexts: Vec<crate::gemini_client::ActivityContext> =
        activities.iter().filter_map(activity_to_context).collect();

    ai_router
        .generate_contextual_dialogue(&activity_contexts, &context, &mood)
        .await
        .map_err(|e| e.to_string())
}

/// Quick ask - minimal prompt/response for fast assistance
#[tauri::command]
pub async fn quick_ask(
    prompt: String,
    include_context: Option<bool>,
    session: State<'_, Arc<crate::memory::SessionMemory>>,
    ai_router: State<'_, Arc<SmartAiRouter>>,
) -> Result<String, String> {
    let trimmed = prompt.trim();
    if trimmed.is_empty() {
        return Err("Prompt cannot be empty".to_string());
    }

    let include_context = include_context.unwrap_or(true);
    let full_prompt = if include_context {
        let state = session.load().unwrap_or_default();
        let redacted_url = crate::privacy::redact_with_settings(&state.current_url);
        let redacted_title = crate::privacy::redact_with_settings(&state.current_title);
        format!(
            "You are a fast desktop assistant. Answer succinctly (1-4 sentences).\n\nUser question: {}\n\nContext (if relevant):\n- Current URL: {}\n- Page title: {}",
            trimmed, redacted_url, redacted_title
        )
    } else {
        format!(
            "You are a fast desktop assistant. Answer succinctly (1-4 sentences).\n\nUser question: {}",
            trimmed
        )
    };

    ai_router
        .generate_text(&full_prompt)
        .await
        .map_err(|e| e.to_string())
}

// ============================================================================
// Original IPC Commands
// ============================================================================

/// Trigger a visual effect in the browser (queued for extension)
#[tauri::command]
pub fn trigger_browser_effect(
    effect: String,
    duration: Option<u64>,
    text: Option<String>,
    effect_queue: State<'_, Arc<EffectQueue>>,
) -> Result<(), String> {
    let msg = EffectMessage {
        action: if text.is_some() {
            "highlight_text".to_string()
        } else {
            "inject_effect".to_string()
        },
        effect: Some(effect),
        duration,
        text,
        url: None,
    };
    effect_queue.push(msg);
    Ok(())
}

/// Send a ping to the browser extension
#[tauri::command]
pub fn request_extension_ping(
    effect_queue: State<'_, Arc<EffectQueue>>,
) -> Result<(), String> {
    let msg = EffectMessage {
        action: "ping".to_string(),
        effect: None,
        duration: None,
        text: None,
        url: None,
    };
    effect_queue.push(msg);
    Ok(())
}

/// Capture screenshot and analyze with AI
#[tauri::command]
pub async fn capture_and_analyze(
    app: tauri::AppHandle,
    session: State<'_, Arc<crate::memory::SessionMemory>>,
    ai_router: State<'_, Arc<SmartAiRouter>>,
) -> Result<String, String> {
    // Enforce explicit user consent
    let privacy = crate::privacy::PrivacySettings::load();
    if privacy.read_only_mode {
        return Err("Read-only mode enabled".to_string());
    }
    if !privacy.capture_consent {
        return Err("Screen capture consent not granted".to_string());
    }
    if !privacy.ai_analysis_consent {
        return Err("AI analysis consent not granted".to_string());
    }

    // Capture screen (handles hiding window internally)
    let screenshot = capture::capture_screen(app)
        .await
        .map_err(|e| format!("Capture failed: {}", e))?;

    // Record capture for session metrics (best effort)
    let _ = session.record_screenshot();

    // Analyze with AI
    let prompt =
        "You are a detective analyzing a screen. Describe what the user is looking at briefly. \
                  Note any interesting patterns, websites, or content that could be puzzle clues. \
                  Focus on: URLs visible, page titles, main content topics. \
                  Be concise (max 200 words).";

    let analysis = ai_router
        .analyze_image(&screenshot, prompt)
        .await
        .map_err(|e| format!("Analysis failed: {}", e))?;

    Ok(analysis)
}

/// Verify if a screenshot matches the puzzle clue
#[tauri::command]
pub async fn verify_screenshot_proof(
    app: tauri::AppHandle,
    puzzle_id: String,
    ai_router: State<'_, Arc<SmartAiRouter>>,
    puzzles: State<'_, std::sync::RwLock<Vec<Puzzle>>>,
) -> Result<crate::gemini_client::VerificationResult, String> {
    // Enforce explicit user consent
    let privacy = crate::privacy::PrivacySettings::load();
    if privacy.read_only_mode {
        return Err("Read-only mode enabled".to_string());
    }
    if !privacy.capture_consent {
        return Err("Screen capture consent not granted".to_string());
    }
    if !privacy.ai_analysis_consent {
        return Err("AI analysis consent not granted".to_string());
    }

    // 1. Capture screen within the backend
    let image_base64 = capture::capture_screen(app)
        .await
        .map_err(|e| format!("Screen capture failed: {}", e))?;

    let target_description = {
        let puzzles = puzzles.read().map_err(|e| format!("Lock error: {}", e))?;
        let puzzle = puzzles
            .iter()
            .find(|p| p.id == puzzle_id)
            .ok_or_else(|| format!("Puzzle {} not found", puzzle_id))?;

        // Use both clue and target description for better context
        format!(
            "Clue: {}. Target: {}",
            puzzle.clue, puzzle.target_description
        )
    };

    ai_router
        .verify_screenshot_clue(&image_base64, &target_description)
        .await
        .map_err(|e| format!("Verification failed: {}", e))
}

/// Check if API key is configured
#[tauri::command]
pub fn check_api_key() -> Result<bool, String> {
    Ok(crate::utils::runtime_config().has_api_key())
}

/// Get the config file path for storing API key
fn get_config_path() -> Result<std::path::PathBuf, String> {
    let config_dir = dirs::config_dir()
        .ok_or_else(|| "Could not find config directory".to_string())?
        .join("os-ghost");

    // Create config directory if it doesn't exist
    std::fs::create_dir_all(&config_dir)
        .map_err(|e| format!("Failed to create config directory: {}", e))?;

    Ok(config_dir.join("config.json"))
}

/// Config structure for persisting settings
#[derive(Debug, Serialize, Deserialize, Default)]
struct AppConfig {
    gemini_api_key: Option<String>,
    ollama_url: Option<String>,
    ollama_vision_model: Option<String>,
    ollama_text_model: Option<String>,
}

/// Load config from file
fn load_config() -> AppConfig {
    let config_path = match get_config_path() {
        Ok(path) => path,
        Err(_) => return AppConfig::default(),
    };

    if !config_path.exists() {
        return AppConfig::default();
    }

    match std::fs::read_to_string(&config_path) {
        Ok(contents) => match serde_json::from_str(&contents) {
            Ok(cfg) => cfg,
            Err(e) => {
                tracing::warn!("Failed to parse config file {:?}: {}", config_path, e);
                AppConfig::default()
            }
        },
        Err(e) => {
            tracing::warn!("Failed to read config file {:?}: {}", config_path, e);
            AppConfig::default()
        }
    }
}

/// Save config to file
fn save_config(config: &AppConfig) -> Result<(), String> {
    let config_path = get_config_path()?;
    let contents = serde_json::to_string_pretty(config)
        .map_err(|e| format!("Failed to serialize config: {}", e))?;
    std::fs::write(&config_path, contents)
        .map_err(|e| format!("Failed to write config file: {}", e))?;
    Ok(())
}

/// Set API key at runtime and persist to config file
#[tauri::command]
pub async fn set_api_key(api_key: String) -> Result<(), String> {
    // Validate the key is not empty
    let trimmed_key = api_key.trim();
    if trimmed_key.is_empty() {
        return Err("API key cannot be empty".to_string());
    }

    // Set in thread-safe runtime config (instead of env::set_var which is not thread-safe)
    crate::utils::runtime_config().set_api_key(trimmed_key.to_string());

    // Persist to config file
    let mut config = load_config();
    config.gemini_api_key = Some(trimmed_key.to_string());
    save_config(&config)?;

    tracing::info!("API key set and persisted to config");
    Ok(())
}

/// Clear runtime API key (reverting to env if present)
#[tauri::command]
pub fn clear_api_key() -> Result<(), String> {
    crate::utils::runtime_config().clear_api_key();

    // Also clear from persisted config
    let mut config = load_config();
    config.gemini_api_key = None;
    save_config(&config)?;

    tracing::info!("API key cleared from runtime and config");
    Ok(())
}

/// Validate an API key by testing with Gemini API
#[tauri::command]
pub async fn validate_api_key(api_key: String) -> Result<bool, String> {
    let trimmed_key = api_key.trim();
    if trimmed_key.is_empty() {
        return Err("API key cannot be empty".to_string());
    }

    // Basic format check - Gemini API keys are typically 39 characters
    if trimmed_key.len() < 20 {
        return Err("API key appears too short".to_string());
    }

    // Create a temporary client with the provided key
    let test_client = crate::gemini_client::GeminiClient::new(trimmed_key.to_string());

    // Try to make a simple API call to validate the key
    match test_client
        .generate_dialogue("test", "You are a test assistant. Say 'OK'.")
        .await
    {
        Ok(_) => {
            tracing::info!("API key validation successful");
            Ok(true)
        }
        Err(e) => {
            let error_msg = e.to_string().to_lowercase();
            tracing::warn!("API key validation failed: {}", error_msg);

            // Parse specific error types for user-friendly messages
            if error_msg.contains("401") || error_msg.contains("api_key_invalid") {
                Err("Invalid API key. Please check and try again.".to_string())
            } else if error_msg.contains("403") || error_msg.contains("permission") {
                Err("API key lacks required permissions.".to_string())
            } else if error_msg.contains("429")
                || error_msg.contains("rate")
                || error_msg.contains("quota")
            {
                Err("Rate limit or quota exceeded. Try again later.".to_string())
            } else if error_msg.contains("500")
                || error_msg.contains("503")
                || error_msg.contains("unavailable")
            {
                Err("Gemini API temporarily unavailable. Try again.".to_string())
            } else if error_msg.contains("timeout")
                || error_msg.contains("connect")
                || error_msg.contains("network")
            {
                Err("Network error. Check your internet connection.".to_string())
            } else if error_msg.contains("billing") || error_msg.contains("payment") {
                Err("Billing issue with API key. Check your Google Cloud account.".to_string())
            } else {
                // Generic error with first 80 chars of message
                let short_msg: String = error_msg.chars().take(80).collect();
                Err(format!("Validation failed: {}", short_msg))
            }
        }
    }
}

// ============================================================================
// Ollama Configuration Commands
// ============================================================================

/// Ollama configuration response
#[derive(Debug, Serialize)]
pub struct OllamaConfig {
    pub url: String,
    pub vision_model: String,
    pub text_model: String,
    pub default_url: String,
    pub default_vision_model: String,
    pub default_text_model: String,
}

/// Get current Ollama configuration
#[tauri::command]
pub fn get_ollama_config() -> OllamaConfig {
    let config = crate::utils::runtime_config();
    OllamaConfig {
        url: config.get_ollama_url(),
        vision_model: config.get_ollama_vision_model(),
        text_model: config.get_ollama_text_model(),
        default_url: crate::utils::DEFAULT_OLLAMA_URL.to_string(),
        default_vision_model: crate::utils::DEFAULT_OLLAMA_VISION_MODEL.to_string(),
        default_text_model: crate::utils::DEFAULT_OLLAMA_TEXT_MODEL.to_string(),
    }
}

/// Set Ollama configuration
#[tauri::command]
pub fn set_ollama_config(
    url: String,
    vision_model: String,
    text_model: String,
) -> Result<(), String> {
    let config = crate::utils::runtime_config();

    // Validate URL format
    let url = url.trim();
    if url.is_empty() || !url.starts_with("http") {
        return Err("Invalid URL format. Must start with http:// or https://".to_string());
    }

    // Validate model names
    let vision_model = vision_model.trim();
    let text_model = text_model.trim();
    if vision_model.is_empty() || text_model.is_empty() {
        return Err("Model names cannot be empty".to_string());
    }

    config.set_ollama_url(url.to_string());
    config.set_ollama_vision_model(vision_model.to_string());
    config.set_ollama_text_model(text_model.to_string());

    // Persist to config file
    let mut saved_config = load_config();
    saved_config.ollama_url = Some(url.to_string());
    saved_config.ollama_vision_model = Some(vision_model.to_string());
    saved_config.ollama_text_model = Some(text_model.to_string());
    save_config(&saved_config)?;

    tracing::info!(
        "Ollama config updated: url={}, vision={}, text={}",
        url,
        vision_model,
        text_model
    );
    Ok(())
}

/// Reset Ollama configuration to defaults
#[tauri::command]
pub fn reset_ollama_config() -> Result<OllamaConfig, String> {
    let config = crate::utils::runtime_config();
    config.reset_ollama_to_defaults();

    // Clear from saved config
    let mut saved_config = load_config();
    saved_config.ollama_url = None;
    saved_config.ollama_vision_model = None;
    saved_config.ollama_text_model = None;
    save_config(&saved_config)?;

    tracing::info!("Ollama config reset to defaults");
    Ok(get_ollama_config())
}

/// Check if Ollama is running and return status
#[tauri::command]
pub async fn get_ollama_status(
    ai_router: State<'_, Arc<SmartAiRouter>>,
) -> Result<serde_json::Value, String> {
    // Refresh Ollama availability
    ai_router.refresh_ollama_status().await;

    let available = ai_router.has_ollama();
    let has_gemini = ai_router.has_gemini();
    let active = ai_router.active_provider().to_string();

    Ok(serde_json::json!({
        "ollama_available": available,
        "gemini_configured": has_gemini,
        "active_provider": active
    }))
}

// ============================================================================
// Shared Helper Functions & Constants
// ============================================================================

/// State wrapper for the autonomous background task
pub struct AutonomousTask(pub tokio::sync::Mutex<Option<tauri::async_runtime::JoinHandle<()>>>);

/// Helper struct for frontend context
#[derive(Deserialize)]
pub struct PageContext {
    pub url: String,
    pub title: String,
    pub content: String,
    pub puzzle_id: String,
    pub puzzle_clue: String,
    pub target_pattern: String,
    pub hints: Vec<String>,
    pub hints_revealed: usize,
}

/// Run a full multi-agent cycle (Observer -> Verifier -> Narrator)
#[tauri::command]
pub async fn process_agent_cycle(
    context: PageContext,
    orchestrator: State<'_, Arc<crate::agents::AgentOrchestrator>>,
    puzzles: State<'_, std::sync::RwLock<Vec<Puzzle>>>,
    app_handle: tauri::AppHandle,
) -> Result<crate::agents::orchestrator::OrchestrationResult, String> {
    tracing::info!(
        "Starting agent cycle for puzzle '{}' at URL: {}",
        context.puzzle_id,
        context.url
    );

    // Get MCP server (if available)
    use tauri::Manager;
    let mcp_server = app_handle.try_state::<Arc<crate::mcp::BrowserMcpServer>>();
    let mcp_ref = mcp_server.as_ref().map(|arc| arc.as_ref());

    // Lookup puzzle to get target_pattern
    let target_url_pattern = {
        let puzzles = puzzles.read().map_err(|e| format!("Lock error: {}", e))?;
        let puzzle = puzzles
            .iter()
            .find(|p| p.id == context.puzzle_id)
            .ok_or_else(|| {
                tracing::warn!("Puzzle '{}' not found in puzzle list", context.puzzle_id);
                format!("Puzzle {} not found", context.puzzle_id)
            })?;
        puzzle.target_url_pattern.clone()
    };

    // Record URL visit for session tracking
    if let Err(e) = orchestrator.record_url(&context.url) {
        tracing::warn!("Failed to record URL: {}", e);
    }

    // Build metadata
    let mut metadata = std::collections::HashMap::new();

    // Get target description for content verification
    if let Some(puzzle) = {
        let puzzles = puzzles.read().map_err(|e| format!("Lock error: {}", e))?;
        puzzles.iter().find(|p| p.id == context.puzzle_id).cloned()
    } {
        metadata.insert("target_description".to_string(), puzzle.target_description);
    }

    // Build agent context
    let agent_context = crate::agents::traits::AgentContext {
        current_url: context.url,
        current_title: context.title,
        page_content: context.content,
        puzzle_id: context.puzzle_id,
        puzzle_clue: context.puzzle_clue,
        target_pattern: target_url_pattern,
        hints: context.hints,
        hints_revealed: context.hints_revealed,
        proximity: 0.0,                       // start fresh
        ghost_mood: "mysterious".to_string(), // default
        metadata,
        // New planning/reflection fields (use defaults)
        planning: Default::default(),
        last_reflection: None,
        reflection_iterations: 0,
        previous_outputs: Vec::new(),
    };

    // Run pipeline
    tracing::debug!("Running orchestrator pipeline...");
    let result = orchestrator
        .process(&agent_context, mcp_ref)
        .await
        .map_err(|e| {
            tracing::error!("Agent cycle failed: {}", e);
            format!("Agent cycle failed: {}", e)
        })?;

    tracing::info!(
        "Agent cycle completed: proximity={}, solved={}, state={}",
        result.proximity,
        result.solved,
        result.ghost_state
    );

    Ok(result)
}

/// Trigger background analysis (Parallel Workflow)
#[tauri::command]
pub async fn start_background_checks(
    context: PageContext,
    orchestrator: State<'_, Arc<crate::agents::AgentOrchestrator>>,
    puzzles: State<'_, std::sync::RwLock<Vec<Puzzle>>>,
) -> Result<String, String> {
    let privacy = crate::privacy::PrivacySettings::load();
    if privacy.read_only_mode {
        return Err("Read-only mode enabled".to_string());
    }
    // Lookup pattern
    let target_url_pattern = {
        let puzzles = puzzles.read().map_err(|e| format!("Lock error: {}", e))?;
        let puzzle = puzzles
            .iter()
            .find(|p| p.id == context.puzzle_id)
            .ok_or_else(|| format!("Puzzle {} not found", context.puzzle_id))?;
        puzzle.target_url_pattern.clone()
    };

    let agent_context = crate::agents::traits::AgentContext {
        current_url: context.url,
        current_title: context.title,
        page_content: context.content,
        puzzle_id: context.puzzle_id,
        puzzle_clue: context.puzzle_clue,
        target_pattern: target_url_pattern,
        hints: context.hints,
        hints_revealed: context.hints_revealed,
        proximity: 0.0,
        ghost_mood: "analytical".to_string(), // Different mood for background tasks
        metadata: std::collections::HashMap::new(),
        // New planning/reflection fields (use defaults)
        planning: Default::default(),
        last_reflection: None,
        reflection_iterations: 0,
        previous_outputs: Vec::new(),
    };

    let results = orchestrator
        .run_parallel_checks(&agent_context)
        .await
        .map_err(|e| format!("Background checks failed: {}", e))?;

    Ok(format!("Completed {} background checks", results.len()))
}

/// Autonomous mode progress event payload
#[derive(Clone, Serialize)]
pub struct AutonomousProgress {
    pub iteration: usize,
    pub proximity: f32,
    pub message: String,
    pub solved: bool,
    pub finished: bool,
}

// ============================================================================
// HITL Feedback Commands (Chapter 13)
// ============================================================================

/// Submit user feedback on hints or dialogue
#[tauri::command]
pub async fn submit_feedback(
    target: String,
    content: String,
    is_positive: bool,
    puzzle_id: Option<String>,
    comment: Option<String>,
    ltm: State<'_, Arc<crate::memory::LongTermMemory>>,
) -> Result<(), String> {
    let feedback_target = match target.as_str() {
        "hint" => crate::memory::long_term::FeedbackTarget::Hint,
        "dialogue" => crate::memory::long_term::FeedbackTarget::Dialogue,
        "puzzle" => crate::memory::long_term::FeedbackTarget::PuzzleDifficulty,
        "experience" => crate::memory::long_term::FeedbackTarget::Experience,
        _ => crate::memory::long_term::FeedbackTarget::Dialogue,
    };

    let feedback = crate::memory::long_term::UserFeedback {
        id: format!(
            "feedback_{}_{}",
            current_timestamp_millis(),
            rand::thread_rng().gen_range(1000..9999)
        ),
        target: feedback_target,
        content,
        is_positive,
        comment,
        puzzle_id,
        url: None,
        timestamp: crate::utils::current_timestamp(),
    };

    ltm.record_feedback(feedback)
        .map_err(|e| format!("Failed to record feedback: {}", e))
}

/// Submit an escalation when user is stuck
#[tauri::command]
pub async fn submit_escalation(
    puzzle_id: String,
    time_stuck_secs: u64,
    hints_revealed: usize,
    current_url: String,
    description: Option<String>,
    ltm: State<'_, Arc<crate::memory::LongTermMemory>>,
) -> Result<crate::memory::long_term::Escalation, String> {
    ltm.create_escalation(
        &puzzle_id,
        time_stuck_secs,
        hints_revealed,
        &current_url,
        description,
    )
    .map_err(|e| format!("Failed to create escalation: {}", e))
}

/// Resolve an existing escalation
#[tauri::command]
pub async fn resolve_escalation(
    escalation_id: String,
    resolution: String,
    ltm: State<'_, Arc<crate::memory::LongTermMemory>>,
) -> Result<(), String> {
    ltm.resolve_escalation(&escalation_id, &resolution)
        .map_err(|e| format!("Failed to resolve escalation: {}", e))
}

/// Get player statistics including feedback counts
#[tauri::command]
pub async fn get_player_stats(
    ltm: State<'_, Arc<crate::memory::LongTermMemory>>,
) -> Result<crate::memory::long_term::PlayerStats, String> {
    ltm.get_stats()
        .map_err(|e| format!("Failed to get stats: {}", e))
}

/// Get feedback ratio (positive feedback / total)
#[tauri::command]
pub async fn get_feedback_ratio(
    ltm: State<'_, Arc<crate::memory::LongTermMemory>>,
) -> Result<f32, String> {
    ltm.get_feedback_ratio()
        .map_err(|e| format!("Failed to get feedback ratio: {}", e))
}

/// Get patterns from negative feedback (for learning)
#[tauri::command]
pub async fn get_learning_patterns(
    ltm: State<'_, Arc<crate::memory::LongTermMemory>>,
) -> Result<Vec<String>, String> {
    ltm.get_learning_patterns()
        .map_err(|e| format!("Failed to get learning patterns: {}", e))
}

// ============================================================================
// Intelligent Mode Commands
// ============================================================================

/// Response for intelligent mode status
#[derive(Debug, Serialize, Clone)]
pub struct IntelligentModeStatus {
    pub intelligent_mode: bool,
    pub reflection: bool,
    pub guardrails: bool,
}

/// Get current intelligent mode settings
#[tauri::command]
pub async fn get_intelligent_mode(
    orchestrator: State<'_, Arc<crate::agents::AgentOrchestrator>>,
) -> Result<IntelligentModeStatus, String> {
    Ok(IntelligentModeStatus {
        intelligent_mode: orchestrator.use_intelligent_mode(),
        reflection: orchestrator.use_reflection(),
        guardrails: orchestrator.use_guardrails(),
    })
}

/// Toggle intelligent mode (planning + reflection)
#[tauri::command]
pub async fn set_intelligent_mode(
    enabled: bool,
    orchestrator: State<'_, Arc<crate::agents::AgentOrchestrator>>,
) -> Result<IntelligentModeStatus, String> {
    orchestrator.set_intelligent_mode(enabled);
    get_intelligent_mode(orchestrator).await
}

/// Toggle reflection (generator-critic loop)
#[tauri::command]
pub async fn set_reflection_mode(
    enabled: bool,
    orchestrator: State<'_, Arc<crate::agents::AgentOrchestrator>>,
) -> Result<IntelligentModeStatus, String> {
    orchestrator.set_reflection(enabled);
    get_intelligent_mode(orchestrator).await
}

/// Toggle guardrails (input/output safety checks)
#[tauri::command]
pub async fn set_guardrails_mode(
    enabled: bool,
    orchestrator: State<'_, Arc<crate::agents::AgentOrchestrator>>,
) -> Result<IntelligentModeStatus, String> {
    orchestrator.set_guardrails(enabled);
    get_intelligent_mode(orchestrator).await
}

// ============================================================================
// Autonomous Mode Commands
// ============================================================================

/// Start autonomous monitoring (Loop Workflow) - runs in background with events
#[tauri::command]
pub async fn enable_autonomous_mode(
    context: PageContext,
    app_handle: tauri::AppHandle,
    orchestrator: State<'_, Arc<crate::agents::AgentOrchestrator>>,
    puzzles: State<'_, std::sync::RwLock<Vec<Puzzle>>>,
    autonomous_task: State<'_, AutonomousTask>,
) -> Result<String, String> {
    let privacy = crate::privacy::PrivacySettings::load();
    if privacy.read_only_mode {
        return Err("Read-only mode enabled".to_string());
    }
    // 1. Cancel previous task if running
    {
        let mut task_guard = autonomous_task.0.lock().await;
        if let Some(handle) = task_guard.take() {
            tracing::info!("Aborting previous autonomous agent task...");
            handle.abort();
        }
    }

    // Lookup pattern
    let target_url_pattern = {
        let puzzles = puzzles.read().map_err(|e| format!("Lock error: {}", e))?;
        let puzzle = puzzles
            .iter()
            .find(|p| p.id == context.puzzle_id)
            .ok_or_else(|| format!("Puzzle {} not found", context.puzzle_id))?;
        puzzle.target_url_pattern.clone()
    };

    let agent_context = crate::agents::traits::AgentContext {
        current_url: context.url,
        current_title: context.title,
        page_content: context.content,
        puzzle_id: context.puzzle_id,
        puzzle_clue: context.puzzle_clue,
        target_pattern: target_url_pattern,
        hints: context.hints,
        hints_revealed: context.hints_revealed,
        proximity: 0.0,
        ghost_mood: "observant".to_string(),
        metadata: std::collections::HashMap::new(),
        // New planning/reflection fields (use defaults)
        planning: Default::default(),
        last_reflection: None,
        reflection_iterations: 0,
        previous_outputs: Vec::new(),
    };

    // Clone orchestrator for the spawned task
    let orchestrator = orchestrator.inner().clone();
    let app_handle_clone = app_handle.clone();

    // Spawn background task - doesn't block the command
    let handle = tauri::async_runtime::spawn(async move {
        tracing::info!("Starting new autonomous agent loop...");
        match orchestrator.run_autonomous_loop(&agent_context).await {
            Ok(outputs) => {
                // Emit progress for each iteration
                for (i, output) in outputs.iter().enumerate() {
                    let solved = matches!(
                        output.next_action,
                        Some(crate::agents::traits::NextAction::PuzzleSolved)
                    );
                    let progress = AutonomousProgress {
                        iteration: i + 1,
                        proximity: output.confidence,
                        message: output.result.clone(),
                        solved,
                        finished: i == outputs.len() - 1,
                    };
                    let _ = app_handle_clone.emit("autonomous_progress", progress);
                }
            }
            Err(e) => {
                tracing::error!("Autonomous loop failed: {}", e);
                let _ = app_handle_clone.emit(
                    "autonomous_progress",
                    AutonomousProgress {
                        iteration: 0,
                        proximity: 0.0,
                        message: format!("Error: {}", e),
                        solved: false,
                        finished: true,
                    },
                );
            }
        }
    });

    // Store the handle
    {
        let mut task_guard = autonomous_task.0.lock().await;
        *task_guard = Some(handle);
    }

    Ok("Autonomous mode started - listen for 'autonomous_progress' events".to_string())
}

// ============================================================================
// Model Capabilities & Token Usage (P1/P2 from Audit)
// ============================================================================

/// Model capabilities for graceful degradation
#[derive(Debug, Serialize, Clone)]
pub struct ModelCapabilities {
    /// Primary provider being used ("Gemini", "Ollama", "None")
    pub provider: String,
    /// Vision capability available
    pub has_vision: bool,
    /// Tool calling capability available
    pub has_tool_calling: bool,
    /// Estimated context window size
    pub context_window: u32,
    /// Warnings/recommendations for user
    pub warnings: Vec<String>,
    /// Whether Ollama is available as fallback
    pub ollama_available: bool,
    /// Whether Gemini is configured
    pub gemini_configured: bool,
}

/// Get model capabilities for graceful degradation (P1)
#[tauri::command]
pub async fn get_model_capabilities(
    ai_router: State<'_, Arc<SmartAiRouter>>,
) -> Result<ModelCapabilities, String> {
    let provider = ai_router.active_provider();
    let has_gemini = ai_router.has_gemini();
    let has_ollama = ai_router.has_ollama();
    
    let mut warnings = Vec::new();
    
    // Determine capabilities based on provider
    let (has_vision, has_tool_calling, context_window) = match provider {
        crate::ai_provider::ProviderType::Gemini => {
            // Gemini 2.0 Flash capabilities
            (true, true, 1_000_000)
        }
        crate::ai_provider::ProviderType::Ollama => {
            // Ollama capabilities depend on model - check vision model availability
            let vision_available = ai_router.has_ollama(); // Simplified check
            if !vision_available {
                warnings.push("Ollama vision model not detected. Run: ollama pull llama3.2-vision".to_string());
            }
            (vision_available, false, 8192) // Most Ollama models have smaller context
        }
        crate::ai_provider::ProviderType::None => {
            warnings.push("No AI provider available. Configure Gemini API key or start Ollama.".to_string());
            (false, false, 0)
        }
    };
    
    // Add warnings for degraded functionality
    if !has_gemini && has_ollama {
        warnings.push("Using local Ollama only. Some features may be limited.".to_string());
    }
    if !has_tool_calling {
        warnings.push("Tool calling not available with current provider.".to_string());
    }
    if context_window < 32000 {
        warnings.push(format!("Context window is {}k tokens. Complex puzzles may be affected.", context_window / 1000));
    }
    
    Ok(ModelCapabilities {
        provider: provider.to_string(),
        has_vision,
        has_tool_calling,
        context_window,
        warnings,
        ollama_available: has_ollama,
        gemini_configured: has_gemini,
    })
}

/// Token usage tracking for cost visibility (P2)
#[derive(Debug, Serialize, Clone)]
pub struct TokenUsage {
    /// Gemini API calls this session
    pub gemini_calls: u64,
    /// Ollama API calls this session
    pub ollama_calls: u64,
    /// Estimated Gemini tokens (rough: 1 call ≈ 500 tokens avg)
    pub estimated_gemini_tokens: u64,
    /// Estimated cost in USD (Gemini 2.0 Flash: ~$0.075/1M input, $0.30/1M output)
    pub estimated_cost_usd: f64,
}

/// Get token usage for cost visibility (P2)
#[tauri::command]
pub fn get_token_usage(
    ai_router: State<'_, Arc<SmartAiRouter>>,
) -> TokenUsage {
    let (gemini_calls, ollama_calls) = ai_router.get_call_counts();
    
    // Rough estimate: average 500 tokens per call (250 input + 250 output)
    let estimated_gemini_tokens = gemini_calls * 500;
    
    // Gemini 2.0 Flash pricing (as of 2024):
    // Input: $0.075 per 1M tokens, Output: $0.30 per 1M tokens
    // Blended average: ~$0.19 per 1M tokens
    let estimated_cost_usd = (estimated_gemini_tokens as f64 / 1_000_000.0) * 0.19;
    
    TokenUsage {
        gemini_calls,
        ollama_calls,
        estimated_gemini_tokens,
        estimated_cost_usd,
    }
}

/// Reset token usage counters (start of new session)
#[tauri::command]
pub fn reset_token_usage(
    ai_router: State<'_, Arc<SmartAiRouter>>,
) {
    ai_router.reset_call_counts();
}

// ============================================================================
// Unified Polling (Consolidates multiple frontend polling intervals)
// ============================================================================

/// Unified status for all agent-related polling
/// Combines pending actions, preview, rollback, and token usage
/// This reduces frontend polling from 3 intervals to 1
#[derive(Debug, Serialize, Clone)]
pub struct AgentPollStatus {
    /// Pending actions requiring confirmation
    pub pending_actions: Vec<crate::actions::PendingAction>,
    /// Active action preview (if any)
    pub action_preview: Option<crate::action_preview::ActionPreview>,
    /// Rollback/undo status
    pub rollback_status: crate::rollback::RollbackStatus,
    /// Token usage stats
    pub token_usage: TokenUsage,
    /// Timestamp of poll (for staleness detection)
    pub timestamp_ms: u64,
}

/// Unified polling endpoint for agent status
/// Frontend should call this once every ~1.5s instead of multiple intervals
#[tauri::command]
pub fn poll_agent_status(
    ai_router: State<'_, Arc<SmartAiRouter>>,
) -> AgentPollStatus {
    // Get pending actions
    crate::actions::ACTION_QUEUE.cleanup_expired();
    let pending_actions = crate::actions::ACTION_QUEUE.get_pending();
    
    // Get action preview
    let action_preview = crate::action_preview::get_preview_manager()
        .and_then(|m| m.get_active_preview());
    
    // Get rollback status
    let rollback_status = crate::rollback::get_rollback_manager()
        .map(|m| m.get_status())
        .unwrap_or_else(|| crate::rollback::RollbackStatus {
            can_undo: false,
            can_redo: false,
            undo_description: None,
            redo_description: None,
            stack_size: 0,
            recent_actions: vec![],
        });
    
    // Get token usage
    let (gemini_calls, ollama_calls) = ai_router.get_call_counts();
    let estimated_gemini_tokens = gemini_calls * 500;
    let estimated_cost_usd = (estimated_gemini_tokens as f64 / 1_000_000.0) * 0.19;
    let token_usage = TokenUsage {
        gemini_calls,
        ollama_calls,
        estimated_gemini_tokens,
        estimated_cost_usd,
    };
    
    // Get timestamp
    let timestamp_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0);
    
    AgentPollStatus {
        pending_actions,
        action_preview,
        rollback_status,
        token_usage,
        timestamp_ms,
    }
}
