//! IPC commands for Tauri frontend-backend communication
//! Exposes Rust functionality to JavaScript via Tauri commands

use crate::ai_client::GeminiClient;
use crate::capture;
use crate::history::{self, HistoryEntry};
use anyhow::Result;
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tauri::{Emitter, State};

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
    puzzles: State<'_, Vec<Puzzle>>,
) -> Result<bool, String> {
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
    puzzles: State<'_, Vec<Puzzle>>,
) -> Result<f32, String> {
    let puzzle = puzzles
        .iter()
        .find(|p| p.id == puzzle_id)
        .ok_or_else(|| format!("Puzzle {} not found", puzzle_id))?;

    // Use AI to calculate semantic similarity
    gemini
        .calculate_url_similarity(&current_url, &puzzle.target_description)
        .await
        .map_err(|e| format!("Proximity calculation failed: {}", e))
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
pub fn get_puzzle(puzzle_id: String, puzzles: State<'_, Vec<Puzzle>>) -> Result<Puzzle, String> {
    puzzles
        .iter()
        .find(|p| p.id == puzzle_id)
        .cloned()
        .ok_or_else(|| format!("Puzzle {} not found", puzzle_id))
}

/// Get all available puzzles
#[tauri::command]
pub fn get_all_puzzles(puzzles: State<'_, Vec<Puzzle>>) -> Vec<Puzzle> {
    puzzles.iter().cloned().collect()
}

/// Check if API key is configured
#[tauri::command]
pub fn check_api_key() -> Result<bool, String> {
    Ok(std::env::var("GEMINI_API_KEY").is_ok())
}

/// Generate a dynamic puzzle based on current page context
#[tauri::command]
pub async fn generate_dynamic_puzzle(
    url: String,
    title: String,
    content: String,
    gemini: State<'_, Arc<GeminiClient>>,
) -> Result<crate::ai_client::DynamicPuzzle, String> {
    let redacted_url = crate::privacy::redact_pii(&url);
    let redacted_content = crate::privacy::redact_pii(&content);

    gemini
        .generate_dynamic_puzzle(&redacted_url, &title, &redacted_content)
        .await
        .map_err(|e| format!("Failed to generate puzzle: {}", e))
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
    puzzles: State<'_, Vec<Puzzle>>,
) -> Result<crate::agents::orchestrator::OrchestrationResult, String> {
    // Lookup puzzle to get target_pattern
    let puzzle = puzzles
        .iter()
        .find(|p| p.id == context.puzzle_id)
        .ok_or_else(|| format!("Puzzle {} not found", context.puzzle_id))?;

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
        target_pattern: puzzle.target_url_pattern.clone(),
        hints: context.hints,
        hints_revealed: context.hints_revealed,
        proximity: 0.0,                       // start fresh
        ghost_mood: "mysterious".to_string(), // default
        metadata: std::collections::HashMap::new(),
    };

    // Run pipeline
    orchestrator
        .process(&agent_context)
        .await
        .map_err(|e| format!("Agent cycle failed: {}", e))
}

/// Trigger background analysis (Parallel Workflow)
#[tauri::command]
pub async fn start_background_checks(
    context: PageContext,
    orchestrator: State<'_, Arc<crate::agents::AgentOrchestrator>>,
    puzzles: State<'_, Vec<Puzzle>>,
) -> Result<String, String> {
    // Lookup pattern
    let puzzle = puzzles
        .iter()
        .find(|p| p.id == context.puzzle_id)
        .ok_or_else(|| format!("Puzzle {} not found", context.puzzle_id))?;

    let agent_context = crate::agents::traits::AgentContext {
        current_url: context.url,
        current_title: context.title,
        page_content: context.content,
        puzzle_id: context.puzzle_id,
        puzzle_clue: context.puzzle_clue,
        target_pattern: puzzle.target_url_pattern.clone(),
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
    puzzles: State<'_, Vec<Puzzle>>,
) -> Result<String, String> {
    // Lookup pattern
    let puzzle = puzzles
        .iter()
        .find(|p| p.id == context.puzzle_id)
        .ok_or_else(|| format!("Puzzle {} not found", context.puzzle_id))?;

    let agent_context = crate::agents::traits::AgentContext {
        current_url: context.url,
        current_title: context.title,
        page_content: context.content,
        puzzle_id: context.puzzle_id,
        puzzle_clue: context.puzzle_clue,
        target_pattern: puzzle.target_url_pattern.clone(),
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
