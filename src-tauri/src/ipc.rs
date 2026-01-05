//! IPC commands for Tauri frontend-backend communication
//! Exposes Rust functionality to JavaScript via Tauri commands

use crate::ai_client::GeminiClient;
use crate::capture;
use crate::game_state::{EffectMessage, EffectQueue};
use crate::history::{self, HistoryEntry};
use anyhow::Result;
use rand::Rng;
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tauri::{Emitter, State};

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

/// Force the browser to navigate to a URL (Computer Use)
#[tauri::command]
pub fn force_navigate(
    url: String,
    effect_queue: State<'_, Arc<EffectQueue>>,
) -> Result<(), String> {
    let msg = EffectMessage {
        action: "navigate".to_string(),
        effect: None,
        duration: None,
        text: None,
        url: Some(url),
    };
    effect_queue.push(msg);
    Ok(())
}

/// Game state exposed to frontend
#[derive(Debug, Serialize, Clone)]
pub struct GameState {
    pub current_puzzle: usize,
    pub clue: String,
    pub proximity: f32,
    pub state: String, // "idle", "thinking", "searching", "celebrate"
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

/// Puzzle configuration
#[derive(Debug, Deserialize)]
pub struct PuzzleConfig {
    pub puzzles: Vec<Puzzle>,
}

/// Capture screenshot and analyze with AI
#[tauri::command]
pub async fn capture_and_analyze(gemini: State<'_, Arc<GeminiClient>>) -> Result<String, String> {
    // Capture screen
    let screenshot = capture::capture_screen()
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

/// Get recent Chrome browsing history
#[tauri::command]
pub async fn get_browsing_history(limit: usize) -> Result<Vec<HistoryEntry>, String> {
    history::get_recent_history(limit).await
}

/// Validate if current URL solves the puzzle
#[tauri::command]
pub async fn validate_puzzle(
    url: String,
    puzzle_id: String,
    puzzles: State<'_, std::sync::RwLock<Vec<Puzzle>>>,
) -> Result<bool, String> {
    let puzzles = puzzles.read().map_err(|e| format!("Lock error: {}", e))?;
    let puzzle = puzzles
        .iter()
        .find(|p| p.id == puzzle_id)
        .ok_or_else(|| format!("Puzzle {} not found", puzzle_id))?;

    // Check if URL matches the target pattern
    let pattern =
        Regex::new(&puzzle.target_url_pattern).map_err(|e| format!("Invalid pattern: {}", e))?;

    Ok(pattern.is_match(&url))
}

/// Calculate semantic proximity to target (hot/cold feedback)
#[tauri::command]
pub async fn calculate_proximity(
    current_url: String,
    puzzle_id: String,
    gemini: State<'_, Arc<GeminiClient>>,
    puzzles: State<'_, std::sync::RwLock<Vec<Puzzle>>>,
) -> Result<f32, String> {
    let target_description = {
        let puzzles = puzzles.read().map_err(|e| format!("Lock error: {}", e))?;
        let puzzle = puzzles
            .iter()
            .find(|p| p.id == puzzle_id)
            .ok_or_else(|| format!("Puzzle {} not found", puzzle_id))?;
        puzzle.target_description.clone()
    };

    // Use AI to calculate semantic similarity
    gemini
        .calculate_url_similarity(&current_url, &target_description)
        .await
        .map_err(|e| format!("Proximity calculation failed: {}", e))
}

/// Verify if a screenshot matches the puzzle clue
#[tauri::command]
pub async fn verify_screenshot_proof(
    puzzle_id: String,
    gemini: State<'_, Arc<GeminiClient>>,
    puzzles: State<'_, std::sync::RwLock<Vec<Puzzle>>>,
) -> Result<crate::ai_client::VerificationResult, String> {
    // 1. Capture screen within the backend
    let image_base64 = capture::capture_screen()
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

/// Generate Ghost dialogue based on current context
#[tauri::command]
pub async fn generate_ghost_dialogue(
    context: String,
    gemini: State<'_, Arc<GeminiClient>>,
) -> Result<String, String> {
    let personality = "A mysterious, slightly mischievous AI trapped between dimensions. \
                       You speak in riddles but genuinely want to help. \
                       You're fascinated by human internet browsing.";

    gemini
        .generate_dialogue(&context, personality)
        .await
        .map_err(|e| format!("Dialogue generation failed: {}", e))
}

/// Get current puzzle info
#[tauri::command]
pub fn get_puzzle(
    puzzle_id: String,
    puzzles: State<'_, std::sync::RwLock<Vec<Puzzle>>>,
) -> Result<Puzzle, String> {
    let puzzles = puzzles.read().map_err(|e| format!("Lock error: {}", e))?;
    puzzles
        .iter()
        .find(|p| p.id == puzzle_id)
        .cloned()
        .ok_or_else(|| format!("Puzzle {} not found", puzzle_id))
}

/// Get all available puzzles
#[tauri::command]
pub fn get_all_puzzles(
    puzzles: State<'_, std::sync::RwLock<Vec<Puzzle>>>,
) -> Result<Vec<Puzzle>, String> {
    let puzzles = puzzles.read().map_err(|e| format!("Lock error: {}", e))?;
    Ok(puzzles.iter().cloned().collect())
}

/// Check if API key is configured
#[tauri::command]
pub fn check_api_key() -> Result<bool, String> {
    Ok(std::env::var("GEMINI_API_KEY").is_ok())
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

    // Set environment variable for current session
    std::env::set_var("GEMINI_API_KEY", trimmed_key);

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

/// Generate a dynamic puzzle based on current page context AND register it
#[tauri::command]
pub async fn generate_dynamic_puzzle(
    url: String,
    title: String,
    content: String,
    gemini: State<'_, Arc<GeminiClient>>,
    puzzles: State<'_, std::sync::RwLock<Vec<Puzzle>>>,
) -> Result<GeneratedPuzzle, String> {
    // 20% chance of sponsored puzzle
    let use_sponsored = rand::thread_rng().gen_range(0.0..1.0) < 0.2;

    // Mock Sponsored Puzzle
    if use_sponsored {
        let id = format!(
            "sponsored_{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_millis()
        );
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

        {
            let mut puzzles = puzzles.write().map_err(|e| format!("Lock error: {}", e))?;
            puzzles.push(puzzle.clone());
        }

        return Ok(GeneratedPuzzle {
            id,
            clue: puzzle.clue,
            hint: puzzle.hint,
            hints: vec!["Search for 'Google Cloud Console'".to_string()],
            target_url_pattern: puzzle.target_url_pattern,
            target_description: puzzle.target_description,
            is_sponsored: true,
            sponsor_id: puzzle.sponsor_id,
        });
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
        Err(e) => {
            tracing::warn!("Failed to fetch history context: {}", e);
            "No recent history available.".to_string()
        }
    };

    tracing::info!(
        "Using history context for puzzle generation:\n{}",
        history_context
    );

    let dynamic = gemini
        .generate_dynamic_puzzle(&redacted_url, &title, &redacted_content, &history_context)
        .await
        .map_err(|e| format!("Failed to generate puzzle: {}", e))?;

    // Generate unique ID
    let id = format!(
        "dynamic_{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis()
    );

    // Create puzzle struct
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

    // Register in backend storage
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
        tracing::info!(
            "Registered dynamic puzzle: {} (total puzzles: {})",
            id,
            puzzles.len()
        );
    }

    // Start timer for the new puzzle
    let mut state = crate::game_state::GameState::load();
    state.start_puzzle_timer();

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

    // Generate unique ID
    let id = format!(
        "history_{}_{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis(),
        rand::thread_rng().gen_range(1000..9999)
    );

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
    {
        let mut puzzles = puzzles.write().map_err(|e| format!("Lock error: {}", e))?;
        puzzles.push(puzzle);
    }

    // Start timer for the new puzzle
    let mut state = crate::game_state::GameState::load();
    state.start_puzzle_timer();

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
        ghost_mood: "observant".to_string(),
        metadata: std::collections::HashMap::new(),
    };

    // Clone orchestrator for the spawned task
    let orchestrator = orchestrator.inner().clone();

    // Spawn background task - doesn't block the command
    tauri::async_runtime::spawn(async move {
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
                    let _ = app_handle.emit("autonomous_progress", progress);
                }
            }
            Err(e) => {
                tracing::error!("Autonomous loop failed: {}", e);
                let _ = app_handle.emit(
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

    Ok("Autonomous mode started - listen for 'autonomous_progress' events".to_string())
}
