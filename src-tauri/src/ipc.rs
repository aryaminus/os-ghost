//! IPC commands for Tauri frontend-backend communication
//! Exposes Rust functionality to JavaScript via Tauri commands

use crate::ai_client::GeminiClient;
use crate::capture;
use crate::game_state::{EffectMessage, EffectQueue};
use crate::utils::current_timestamp_millis;
use anyhow::Result;
use rand::Rng;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tauri::{Emitter, State};

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
    /// API key configured
    pub api_key_configured: bool,
    /// Last known browsing URL (from extension or history)
    pub last_known_url: Option<String>,
    /// Current app mode
    pub current_mode: String,
}

/// Detect Chrome/Chromium browser installation
#[tauri::command]
pub fn detect_chrome() -> SystemStatus {
    let chrome_path = find_chrome_path();
    let chrome_installed = chrome_path.is_some();
    let api_key_configured = crate::utils::runtime_config().has_api_key();

    SystemStatus {
        chrome_path,
        chrome_installed,
        extension_connected: false, // Will be updated by bridge events
        extension_operational: false,
        api_key_configured,
        last_known_url: None,
        current_mode: "game".to_string(),
    }
}

/// Find Chrome/Chromium installation path based on platform (Cached)
fn find_chrome_path() -> Option<String> {
    static CHROME_PATH: std::sync::OnceLock<Option<String>> = std::sync::OnceLock::new();

    CHROME_PATH
        .get_or_init(|| {
            #[cfg(target_os = "macos")]
            {
                let paths = [
                    "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome",
                    "/Applications/Chromium.app/Contents/MacOS/Chromium",
                    "/Applications/Google Chrome Canary.app/Contents/MacOS/Google Chrome Canary",
                    "/Applications/Brave Browser.app/Contents/MacOS/Brave Browser",
                    "/Applications/Microsoft Edge.app/Contents/MacOS/Microsoft Edge",
                    "/Applications/Arc.app/Contents/MacOS/Arc",
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
// Session & Mode Management Commands - REMOVED (Unused)
// ============================================================================

// ============================================================================
// Adaptive Behavior Commands
// ============================================================================

/// Helper function to convert ActivityEntry to ActivityContext
fn activity_to_context(
    entry: &crate::memory::ActivityEntry,
) -> Option<crate::ai_client::ActivityContext> {
    let metadata = entry.metadata.as_ref()?;
    Some(crate::ai_client::ActivityContext {
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
#[tauri::command]
pub async fn generate_adaptive_puzzle(
    orchestrator: State<'_, Arc<crate::agents::AgentOrchestrator>>,
    gemini: State<'_, Arc<crate::ai_client::GeminiClient>>,
) -> Result<crate::ai_client::AdaptivePuzzle, String> {
    // Get recent activity from session
    let activities = orchestrator
        .get_recent_activity(10)
        .map_err(|e| e.to_string())?;

    // Convert to ActivityContext format using helper
    let activity_contexts: Vec<crate::ai_client::ActivityContext> =
        activities.iter().filter_map(activity_to_context).collect();

    if activity_contexts.is_empty() {
        return Err("No activity history available. Wait for some observations.".to_string());
    }

    // Get current app/content from most recent activity
    let latest = activity_contexts.first();
    let current_app = latest.map(|a| a.app_name.as_str());
    let current_content = latest.and_then(|a| a.content_context.as_deref());

    gemini
        .generate_adaptive_puzzle(&activity_contexts, current_app, current_content)
        .await
        .map_err(|e| e.to_string())
}

/// Generate contextual dialogue based on observation history
#[tauri::command]
pub async fn generate_contextual_dialogue(
    context: String,
    mood: String,
    orchestrator: State<'_, Arc<crate::agents::AgentOrchestrator>>,
    gemini: State<'_, Arc<crate::ai_client::GeminiClient>>,
) -> Result<String, String> {
    // Get recent activity
    let activities = orchestrator
        .get_recent_activity(5)
        .map_err(|e| e.to_string())?;

    // Convert to ActivityContext using helper
    let activity_contexts: Vec<crate::ai_client::ActivityContext> =
        activities.iter().filter_map(activity_to_context).collect();

    gemini
        .generate_contextual_dialogue(&activity_contexts, &context, &mood)
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

/// Puzzle definition
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct Puzzle {
    pub id: String,
    pub clue: String,
    pub hint: String,
    pub target_url_pattern: String,
    pub target_description: String,
    // Sponsored fields
    pub sponsor_id: Option<String>,
    pub sponsor_url: Option<String>,
    pub is_sponsored: bool,
}

/// Capture screenshot and analyze with AI
#[tauri::command]
pub async fn capture_and_analyze(
    app: tauri::AppHandle,
    gemini: State<'_, Arc<GeminiClient>>,
) -> Result<String, String> {
    // Capture screen (handles hiding window internally)
    let screenshot = capture::capture_screen(app)
        .await
        .map_err(|e| format!("Capture failed: {}", e))?;

    // Analyze with AI
    let prompt =
        "You are a detective analyzing a screen. Describe what the user is looking at briefly. \
                  Note any interesting patterns, websites, or content that could be puzzle clues. \
                  Focus on: URLs visible, page titles, main content topics. \
                  Be concise (max 200 words).";

    let analysis = gemini
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
    gemini: State<'_, Arc<GeminiClient>>,
    puzzles: State<'_, std::sync::RwLock<Vec<Puzzle>>>,
) -> Result<crate::ai_client::VerificationResult, String> {
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

    gemini
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
        Ok(contents) => serde_json::from_str(&contents).unwrap_or_default(),
        Err(_) => AppConfig::default(),
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
    let test_client = crate::ai_client::GeminiClient::new(trimmed_key.to_string());

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

/// Generated puzzle with ID for frontend
#[derive(Debug, Serialize, Clone)]
pub struct GeneratedPuzzle {
    pub id: String,
    pub clue: String,
    pub hint: String,
    pub hints: Vec<String>,
    pub target_url_pattern: String,
    pub target_description: String,
    pub is_sponsored: bool,
    pub sponsor_id: Option<String>,
}

/// Helper: Create a sponsored puzzle
fn create_sponsored_puzzle() -> (Puzzle, GeneratedPuzzle) {
    let id = generate_puzzle_id("sponsored");
    let puzzle = Puzzle {
        id: id.clone(),
        clue: "Seek the cloud where giants build their dreams. Find the console of the Titans."
            .to_string(),
        hint: "Search for 'Google Cloud Console'".to_string(),
        target_url_pattern: "console\\.cloud\\.google\\.com".to_string(),
        target_description: "Google Cloud Console".to_string(),
        sponsor_id: Some("google_cloud".to_string()),
        sponsor_url: Some("https://cloud.google.com".to_string()),
        is_sponsored: true,
    };

    let generated = GeneratedPuzzle {
        id,
        clue: puzzle.clue.clone(),
        hint: puzzle.hint.clone(),
        hints: vec!["Search for 'Google Cloud Console'".to_string()],
        target_url_pattern: puzzle.target_url_pattern.clone(),
        target_description: puzzle.target_description.clone(),
        is_sponsored: true,
        sponsor_id: puzzle.sponsor_id.clone(),
    };

    (puzzle, generated)
}

/// Helper: Register a puzzle and start the timer
async fn register_puzzle(
    puzzles: &std::sync::RwLock<Vec<Puzzle>>,
    puzzle: Puzzle,
) -> Result<(), String> {
    {
        let mut puzzles = puzzles.write().map_err(|e| format!("Lock error: {}", e))?;
        // Count dynamic puzzles before cleanup
        let dynamic_count = puzzles
            .iter()
            .filter(|p| p.id.starts_with("dynamic_"))
            .count();
        // Remove oldest dynamic puzzles to prevent buildup (keep max 5)
        if dynamic_count >= 5 {
            puzzles.retain(|p| !p.id.starts_with("dynamic_"));
        }
        puzzles.push(puzzle);
    }

    // Start timer for the new puzzle
    let mut state = crate::game_state::GameState::load().await;
    state.start_puzzle_timer().await;

    Ok(())
}

// ============================================================================
// Shared Helper Functions & Constants
// ============================================================================

/// Chance of generating a sponsored puzzle (0.0 - 1.0)
const SPONSORED_PUZZLE_CHANCE: f64 = 0.2;

/// State wrapper for the autonomous background task
pub struct AutonomousTask(pub tokio::sync::Mutex<Option<tauri::async_runtime::JoinHandle<()>>>);

/// Helper: Generate a unique puzzle ID
fn generate_puzzle_id(prefix: &str) -> String {
    format!(
        "{}_{}_{}",
        prefix,
        current_timestamp_millis(),
        rand::thread_rng().gen_range(1000..9999)
    )
}

/// Helper: Common logic to register a dynamic puzzle
async fn register_dynamic_puzzle(
    puzzles: &std::sync::RwLock<Vec<Puzzle>>,
    dynamic: crate::ai_client::DynamicPuzzle,
    id_prefix: &str,
) -> Result<GeneratedPuzzle, String> {
    let id = generate_puzzle_id(id_prefix);

    let puzzle = Puzzle {
        id: id.clone(),
        clue: dynamic.clue.clone(),
        hint: dynamic.hints.first().cloned().unwrap_or_default(),
        target_url_pattern: dynamic.target_url_pattern.clone(),
        target_description: dynamic.target_description.clone(),
        sponsor_id: None,
        sponsor_url: None,
        is_sponsored: false,
    };

    register_puzzle(puzzles, puzzle).await?;

    Ok(GeneratedPuzzle {
        id,
        clue: dynamic.clue,
        hint: dynamic.hints.first().cloned().unwrap_or_default(),
        hints: dynamic.hints,
        target_url_pattern: dynamic.target_url_pattern,
        target_description: dynamic.target_description,
        is_sponsored: false,
        sponsor_id: None,
    })
}

/// Start an investigation (Unified command for puzzle generation)
/// Decides best source (Content vs History) based on SessionMemory
#[tauri::command]
pub async fn start_investigation(
    _app_handle: tauri::AppHandle,
    gemini: State<'_, Arc<GeminiClient>>,
    puzzles: State<'_, std::sync::RwLock<Vec<Puzzle>>>,
    session: State<'_, Arc<crate::memory::SessionMemory>>, // Inject SessionMemory
) -> Result<GeneratedPuzzle, String> {
    // 1. Load Session State
    let state = session
        .load()
        .map_err(|e| format!("Failed to load session: {}", e))?;

    let url = state.current_url.clone();
    let title = state.current_title.clone();

    // If we have content in memory, allow using it for passive verification
    // even if the regex doesn't match perfectly
    let content = state.current_content.clone().unwrap_or_default();

    // 20% chance of sponsored puzzle
    // Generic chance of sponsored puzzle
    let use_sponsored = rand::thread_rng().gen_range(0.0..1.0) < SPONSORED_PUZZLE_CHANCE;

    if use_sponsored {
        let (puzzle, generated) = create_sponsored_puzzle();

        register_puzzle(&puzzles, puzzle.clone()).await?;

        // Update session
        if let Ok(mut s) = session.load() {
            s.puzzle_id = puzzle.id.clone();
            let _ = session.save(&s);
        }

        return Ok(generated);
    }

    let redacted_url = crate::privacy::redact_pii(&url);
    let redacted_content = crate::privacy::redact_pii(&content);

    // Fetch recent history for context
    let history_context = match crate::history::get_recent_history(10).await {
        Ok(history) => history
            .into_iter()
            .map(|h| {
                format!(
                    "- {} ({})",
                    crate::privacy::redact_pii(&h.title),
                    crate::privacy::redact_pii(&h.url)
                )
            })
            .collect::<Vec<String>>()
            .join("\n"),
        Err(_) => "No recent history available.".to_string(),
    };

    tracing::info!("Starting investigation for url: {}", redacted_url);

    // Call AI
    let dynamic = gemini
        .generate_dynamic_puzzle(&redacted_url, &title, &redacted_content, &history_context)
        .await
        .map_err(|e| format!("Failed to generate puzzle: {}", e))?;

    // Use helper to register
    let generated = register_dynamic_puzzle(&puzzles, dynamic, "dynamic").await?;

    // Update session
    if let Ok(mut s) = session.load() {
        s.puzzle_id = generated.id.clone();
        let _ = session.save(&s);
    }

    Ok(generated)
}

#[derive(Debug, Deserialize)]
pub struct HistoryItem {
    pub url: String,
    pub title: String,
    #[serde(rename = "visitCount", default)]
    pub visit_count: Option<i32>,
    #[serde(rename = "lastVisitTime", default)]
    pub last_visit_time: Option<f64>,
}

#[derive(Debug, Deserialize)]
pub struct TopSite {
    pub url: String,
    pub title: String,
}

/// Generate a puzzle based on browsing history (for immediate puzzle without page visit)
#[tauri::command]
pub async fn generate_puzzle_from_history(
    seed_url: String,
    seed_title: String,
    recent_history: Vec<HistoryItem>,
    top_sites: Vec<TopSite>,
    gemini: State<'_, Arc<GeminiClient>>,
    puzzles: State<'_, std::sync::RwLock<Vec<Puzzle>>>,
) -> Result<GeneratedPuzzle, String> {
    let redacted_url = crate::privacy::redact_pii(&seed_url);

    // Build history context from recent history
    let history_context = recent_history
        .iter()
        .take(20)
        .map(|h| {
            format!(
                "- {} ({})",
                crate::privacy::redact_pii(&h.title),
                crate::privacy::redact_pii(&h.url)
            )
        })
        .collect::<Vec<_>>()
        .join("\n");

    // Build top sites context
    let top_sites_context = top_sites
        .iter()
        .map(|s| format!("- {} ({})", s.title, s.url))
        .collect::<Vec<_>>()
        .join("\n");

    let combined_context = format!(
        "Recent Browsing History:\n{}\n\nTop Sites:\n{}",
        history_context, top_sites_context
    );

    tracing::info!("Generating puzzle from history with seed: {}", redacted_url);

    let dynamic = gemini
        .generate_dynamic_puzzle(&redacted_url, &seed_title, "", &combined_context)
        .await
        .map_err(|e| format!("Failed to generate puzzle: {}", e))?;

    // Generate unique ID using shared helper
    let id = generate_puzzle_id("history");

    let puzzle = Puzzle {
        id: id.clone(),
        clue: dynamic.clue.clone(),
        hint: dynamic.hints.first().cloned().unwrap_or_default(),
        target_url_pattern: dynamic.target_url_pattern.clone(),
        target_description: dynamic.target_description.clone(),
        sponsor_id: None,
        sponsor_url: None,
        is_sponsored: false,
    };

    // Register puzzle
    register_puzzle(&puzzles, puzzle).await?;

    Ok(GeneratedPuzzle {
        id,
        clue: dynamic.clue,
        hint: dynamic.hints.first().cloned().unwrap_or_default(),
        hints: dynamic.hints,
        target_url_pattern: dynamic.target_url_pattern,
        target_description: dynamic.target_description,
        is_sponsored: false,
        sponsor_id: None,
    })
}

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
) -> Result<crate::agents::orchestrator::OrchestrationResult, String> {
    tracing::info!(
        "Starting agent cycle for puzzle '{}' at URL: {}",
        context.puzzle_id,
        context.url
    );

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
    };

    // Run pipeline
    tracing::debug!("Running orchestrator pipeline...");
    let result = orchestrator.process(&agent_context).await.map_err(|e| {
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

/// Start autonomous monitoring (Loop Workflow) - runs in background with events
#[tauri::command]
pub async fn enable_autonomous_mode(
    context: PageContext,
    app_handle: tauri::AppHandle,
    orchestrator: State<'_, Arc<crate::agents::AgentOrchestrator>>,
    puzzles: State<'_, std::sync::RwLock<Vec<Puzzle>>>,
    autonomous_task: State<'_, AutonomousTask>,
) -> Result<String, String> {
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
