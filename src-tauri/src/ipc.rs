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

    let dynamic = gemini
        .generate_dynamic_puzzle(&redacted_url, &title, &redacted_content)
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
        metadata: std::collections::HashMap::new(),
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
