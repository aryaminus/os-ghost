//! IPC commands for Tauri frontend-backend communication
//! Exposes Rust functionality to JavaScript via Tauri commands

use crate::ai_client::GeminiClient;
use crate::capture;
use crate::history::{self, HistoryEntry};
use anyhow::Result;
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tauri::State;

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
