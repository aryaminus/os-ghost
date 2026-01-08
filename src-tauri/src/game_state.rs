//! Persistent game state management
//! Stores player progress, solved puzzles, and hint timers

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

const STATE_FILE: &str = "ghost_state.json";
const HINT_DELAY_SECS: u64 = 60; // First hint after 60 seconds
const MAX_HINTS: usize = 3;

/// Message to trigger effect in extension
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EffectMessage {
    pub action: String,
    pub effect: Option<String>,
    pub duration: Option<u64>,
    pub text: Option<String>,
    pub url: Option<String>,
}

/// Ephemeral queue for effects waiting to be sent to extension
pub struct EffectQueue {
    pub queue: std::sync::Mutex<Vec<EffectMessage>>,
}

impl Default for EffectQueue {
    fn default() -> Self {
        Self {
            queue: std::sync::Mutex::new(Vec::new()),
        }
    }
}

impl EffectQueue {
    pub fn push(&self, msg: EffectMessage) {
        if let Ok(mut q) = self.queue.lock() {
            q.push(msg);
        }
    }

    pub fn pop_all(&self) -> Vec<EffectMessage> {
        if let Ok(mut q) = self.queue.lock() {
            let items = q.clone();
            q.clear();
            items
        } else {
            Vec::new()
        }
    }
}

// ... existing commands ...
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GameState {
    pub current_puzzle_index: usize,
    pub solved_puzzles: Vec<String>,
    pub puzzle_start_time: Option<u64>,
    pub hints_revealed: usize,
    pub total_playtime_secs: u64,
    pub discoveries: Vec<Discovery>,
}

/// A player discovery/memory fragment
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Discovery {
    pub puzzle_id: String,
    pub url: String,
    pub title: String,
    pub timestamp: u64,
}

impl Default for GameState {
    fn default() -> Self {
        Self {
            current_puzzle_index: 0,
            solved_puzzles: Vec::new(),
            puzzle_start_time: Some(current_timestamp()),
            hints_revealed: 0,
            total_playtime_secs: 0,
            discoveries: Vec::new(),
        }
    }
}

impl GameState {
    /// Load state from disk or create default (Async)
    pub async fn load() -> Self {
        let path = Self::state_path();
        if path.exists() {
            match tokio::fs::read_to_string(&path).await {
                Ok(contents) => match serde_json::from_str(&contents) {
                    Ok(state) => {
                        tracing::info!(
                            "Loaded game state: {} puzzles solved",
                            serde_json::from_str::<GameState>(&contents)
                                .map(|s| s.solved_puzzles.len())
                                .unwrap_or(0)
                        );
                        return state;
                    }
                    Err(e) => tracing::warn!("Failed to parse state file: {}", e),
                },
                Err(e) => tracing::warn!("Failed to read state file: {}", e),
            }
        }

        tracing::debug!("Creating new game state");
        Self::default()
    }

    /// Save state to disk (Async)
    pub async fn save(&self) -> Result<()> {
        let path = Self::state_path();
        let contents = serde_json::to_string_pretty(self)?;
        tokio::fs::write(&path, contents).await?;
        tracing::debug!("Saved game state to {:?}", path);
        Ok(())
    }

    /// Get the state file path
    fn state_path() -> PathBuf {
        // Store in config directory
        let mut path = dirs::config_dir().unwrap_or_else(|| PathBuf::from("."));
        path.push("os-ghost");

        // Create directory if needed
        // Note: usage of std::fs here is acceptable as it's a one-off path check or could be moved
        // but for path construction avoiding sync I/O is best.
        // We will assume directory exists or lazily create it in save() if we wanted to be pure.
        // For now, let's keep the path builder synchronous as it generates a PathBuf,
        // but we should essentially ensure directories exist in save/load.
        if !path.exists() {
            let _ = std::fs::create_dir_all(&path);
        }

        path.push(STATE_FILE);
        path
    }

    /// Mark a puzzle as solved
    pub async fn solve_puzzle(&mut self, puzzle_id: &str, url: &str, title: &str) {
        if !self.solved_puzzles.contains(&puzzle_id.to_string()) {
            self.solved_puzzles.push(puzzle_id.to_string());
            self.discoveries.push(Discovery {
                puzzle_id: puzzle_id.to_string(),
                url: url.to_string(),
                title: title.to_string(),
                timestamp: current_timestamp(),
            });
            self.current_puzzle_index += 1;
            self.hints_revealed = 0;
            self.puzzle_start_time = Some(current_timestamp());

            let _ = self.save().await;
        }
    }

    /// Start timing for current puzzle
    pub async fn start_puzzle_timer(&mut self) {
        self.puzzle_start_time = Some(current_timestamp());
        self.hints_revealed = 0;
        let _ = self.save().await;
    }

    /// Check if a hint should be revealed
    pub fn should_reveal_hint(&self) -> bool {
        if self.hints_revealed >= MAX_HINTS {
            return false;
        }

        if let Some(start_time) = self.puzzle_start_time {
            let elapsed = current_timestamp().saturating_sub(start_time);
            let threshold = HINT_DELAY_SECS * (self.hints_revealed as u64 + 1);
            return elapsed >= threshold;
        }

        false
    }

    /// Reveal the next hint
    pub async fn reveal_hint(&mut self) -> Option<usize> {
        if self.hints_revealed < MAX_HINTS {
            self.hints_revealed += 1;
            let _ = self.save().await;
            Some(self.hints_revealed - 1)
        } else {
            None
        }
    }

    /// Get time until next hint
    pub fn time_until_next_hint(&self) -> Option<Duration> {
        if self.hints_revealed >= MAX_HINTS {
            return None;
        }

        if let Some(start_time) = self.puzzle_start_time {
            let elapsed = current_timestamp().saturating_sub(start_time);
            let threshold = HINT_DELAY_SECS * (self.hints_revealed as u64 + 1);

            if elapsed < threshold {
                return Some(Duration::from_secs(threshold - elapsed));
            }
        }

        Some(Duration::from_secs(0))
    }

    /// Reset game to start
    pub async fn reset(&mut self) {
        *self = Self::default();
        let _ = self.save().await;
    }

    /// Check if game is complete
    pub fn is_complete(&self, total_puzzles: usize) -> bool {
        self.solved_puzzles.len() >= total_puzzles
    }
}

/// Get current Unix timestamp
fn current_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

/// Tauri command to get current game state
#[tauri::command]
pub async fn get_game_state() -> GameState {
    GameState::load().await
}

/// Tauri command to reset game
#[tauri::command]
pub async fn reset_game() -> Result<(), String> {
    let mut state = GameState::load().await;
    state.reset().await;
    Ok(())
}

/// Tauri command to check for available hint
#[tauri::command]
pub async fn check_hint_available() -> bool {
    let state = GameState::load().await;
    state.should_reveal_hint()
}

/// Tauri command to get next hint
#[tauri::command]
pub async fn get_next_hint(hints: Vec<String>) -> Option<String> {
    let mut state = GameState::load().await;
    if let Some(hint_index) = state.reveal_hint().await {
        hints.get(hint_index).cloned()
    } else {
        None
    }
}
