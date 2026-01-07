//! Long-term memory - persistent storage for game history and discoveries
//! Stores solved puzzles, player discoveries, and semantic embeddings

use super::store::MemoryStore;
use anyhow::Result;
use serde::{Deserialize, Serialize};

const PUZZLES_TREE: &str = "solved_puzzles";
const DISCOVERIES_TREE: &str = "discoveries";
const STATS_TREE: &str = "stats";

/// A solved puzzle record
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SolvedPuzzle {
    pub puzzle_id: String,
    pub solved_at: u64,
    pub time_to_solve_secs: u64,
    pub hints_used: usize,
    pub solution_url: String,
}

/// A discovery made during gameplay
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Discovery {
    pub id: String,
    pub title: String,
    pub description: String,
    pub url: String,
    pub discovered_at: u64,
    pub puzzle_id: Option<String>,
}

/// Player statistics
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PlayerStats {
    pub total_playtime_secs: u64,
    pub puzzles_solved: usize,
    pub total_hints_used: usize,
    pub discoveries_made: usize,
    pub total_urls_visited: usize,
    pub first_played: u64,
    pub last_played: u64,
}

/// Long-term memory manager
pub struct LongTermMemory {
    store: MemoryStore,
}

impl LongTermMemory {
    pub fn new(store: MemoryStore) -> Self {
        Self { store }
    }

    // --- Solved Puzzles ---

    /// Record a solved puzzle
    pub fn record_solved(&self, puzzle: SolvedPuzzle) -> Result<()> {
        self.store.set(PUZZLES_TREE, &puzzle.puzzle_id, &puzzle)?;

        // Update stats
        let mut stats = self.get_stats()?;
        stats.puzzles_solved += 1;
        stats.total_hints_used += puzzle.hints_used;
        self.save_stats(&stats)?;

        Ok(())
    }

    /// Check if a puzzle was solved
    pub fn is_solved(&self, puzzle_id: &str) -> Result<bool> {
        Ok(self
            .store
            .get::<SolvedPuzzle>(PUZZLES_TREE, puzzle_id)?
            .is_some())
    }

    /// Get all solved puzzles
    pub fn get_solved_puzzles(&self) -> Result<Vec<SolvedPuzzle>> {
        self.store.get_all(PUZZLES_TREE)
    }

    /// Get count of solved puzzles
    pub fn solved_count(&self) -> Result<usize> {
        Ok(self.store.list_keys(PUZZLES_TREE)?.len())
    }

    // --- Discoveries ---

    /// Record a discovery
    pub fn record_discovery(&self, discovery: Discovery) -> Result<()> {
        self.store
            .set(DISCOVERIES_TREE, &discovery.id, &discovery)?;

        // Update stats
        let mut stats = self.get_stats()?;
        stats.discoveries_made += 1;
        self.save_stats(&stats)?;

        Ok(())
    }

    /// Get all discoveries
    pub fn get_discoveries(&self) -> Result<Vec<Discovery>> {
        self.store.get_all(DISCOVERIES_TREE)
    }

    // --- User Facts & Context ---

    /// Record a fact about the user/environment
    pub fn record_fact(&self, key: &str, value: &str) -> Result<()> {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        // Use a simple key-value structure for now
        // In a real app, this could be a vector store
        self.store.set(
            "user_facts",
            key,
            &serde_json::json!({
                "value": value,
                "updated_at": now
            }),
        )
    }

    /// Get all recorded user facts
    pub fn get_user_facts(&self) -> Result<std::collections::HashMap<String, String>> {
        // Retrieve all keys from the "user_facts" tree
        let facts = self.store.list_keys("user_facts")?;
        let mut result = std::collections::HashMap::new();

        for key in facts {
            if let Some(data) = self.store.get::<serde_json::Value>("user_facts", &key)? {
                if let Some(val) = data.get("value").and_then(|v| v.as_str()) {
                    result.insert(key, val.to_string());
                }
            }
        }
        Ok(result)
    }

    // --- Statistics ---

    /// Get player statistics
    pub fn get_stats(&self) -> Result<PlayerStats> {
        self.store
            .get(STATS_TREE, "player")?
            .unwrap_or_else(|| {
                let now = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs();
                PlayerStats {
                    first_played: now,
                    last_played: now,
                    ..Default::default()
                }
            })
            .pipe(Ok)
    }

    /// Save player statistics
    pub fn save_stats(&self, stats: &PlayerStats) -> Result<()> {
        self.store.set(STATS_TREE, "player", stats)
    }

    /// Add playtime
    pub fn add_playtime(&self, seconds: u64) -> Result<()> {
        let mut stats = self.get_stats()?;
        stats.total_playtime_secs += seconds;
        stats.last_played = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        self.save_stats(&stats)
    }

    // --- Full Reset ---

    /// Reset all long-term memory
    pub fn reset_all(&self) -> Result<()> {
        self.store.clear_tree(PUZZLES_TREE)?;
        self.store.clear_tree(DISCOVERIES_TREE)?;
        self.store.clear_tree(STATS_TREE)?;
        Ok(())
    }
}

/// Helper trait for pipe syntax
trait Pipe: Sized {
    fn pipe<F, R>(self, f: F) -> R
    where
        F: FnOnce(Self) -> R,
    {
        f(self)
    }
}

impl<T> Pipe for T {}
