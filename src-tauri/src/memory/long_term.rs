//! Long-term memory - persistent storage for game history and discoveries
//! Stores solved puzzles, player discoveries, and semantic embeddings
//!
//! Enhanced with Human-in-the-Loop (HITL) feedback mechanism (Chapter 13):
//! - User feedback on hints and dialogue (thumbs up/down)
//! - Escalation tracking for when the agent is confused
//! - Learning from corrections to improve future responses

use super::store::MemoryStore;
use crate::utils::current_timestamp;
use anyhow::Result;
use serde::{Deserialize, Serialize};

const PUZZLES_TREE: &str = "solved_puzzles";
const DISCOVERIES_TREE: &str = "discoveries";
const STATS_TREE: &str = "stats";
const FEEDBACK_TREE: &str = "user_feedback";
const ESCALATIONS_TREE: &str = "escalations";

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
    // HITL feedback statistics
    pub total_positive_feedback: usize,
    pub total_negative_feedback: usize,
    pub total_escalations: usize,
}

/// Type of content being rated
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum FeedbackTarget {
    /// Feedback on a hint the ghost gave
    Hint,
    /// Feedback on dialogue/narration
    Dialogue,
    /// Feedback on puzzle difficulty
    PuzzleDifficulty,
    /// Feedback on overall experience
    Experience,
}

/// User feedback entry (HITL - Chapter 13)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserFeedback {
    /// Unique ID for this feedback
    pub id: String,
    /// Type of content being rated
    pub target: FeedbackTarget,
    /// The content that was rated (e.g., the hint text, dialogue)
    pub content: String,
    /// Rating: true = positive (thumbs up), false = negative (thumbs down)
    pub is_positive: bool,
    /// Optional user comment
    pub comment: Option<String>,
    /// Context: what puzzle was active
    pub puzzle_id: Option<String>,
    /// Context: what URL was being viewed
    pub url: Option<String>,
    /// When the feedback was given
    pub timestamp: u64,
}

/// Escalation request (when user says "I'm stuck")
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Escalation {
    /// Unique ID
    pub id: String,
    /// What puzzle was the user stuck on
    pub puzzle_id: String,
    /// How long had they been trying (seconds)
    pub time_stuck_secs: u64,
    /// How many hints had been revealed
    pub hints_revealed: usize,
    /// Current URL when escalating
    pub current_url: String,
    /// User's description of why they're stuck (optional)
    pub description: Option<String>,
    /// Whether this was resolved
    pub resolved: bool,
    /// How it was resolved (if applicable)
    pub resolution: Option<String>,
    /// When escalation was created
    pub timestamp: u64,
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

        // Update stats atomically
        let hints_used = puzzle.hints_used;
        self.store.update(STATS_TREE, "player", move |old: Option<PlayerStats>| {
            let mut stats = old.unwrap_or_else(|| {
                // Initialize if missing
                let now = crate::utils::current_timestamp();
                PlayerStats {
                    first_played: now,
                    last_played: now,
                    ..Default::default()
                }
            });
            stats.puzzles_solved += 1;
            stats.total_hints_used += hints_used;
            Some(stats)
        })?;

        Ok(())
    }

    /// Check if a puzzle was solved (currently unused - kept for future puzzle progression)
    #[allow(dead_code)]
    pub fn is_solved(&self, puzzle_id: &str) -> Result<bool> {
        Ok(self
            .store
            .get::<SolvedPuzzle>(PUZZLES_TREE, puzzle_id)?
            .is_some())
    }

    /// Get all solved puzzles (currently unused - kept for leaderboards)
    #[allow(dead_code)]
    pub fn get_solved_puzzles(&self) -> Result<Vec<SolvedPuzzle>> {
        self.store.get_all(PUZZLES_TREE)
    }

    /// Get count of solved puzzles (currently unused - kept for stats)
    #[allow(dead_code)]
    pub fn solved_count(&self) -> Result<usize> {
        self.store.count(PUZZLES_TREE)
    }

    // --- Discoveries ---

    /// Record a discovery (currently unused - kept for future features)
    #[allow(dead_code)]
    pub fn record_discovery(&self, discovery: Discovery) -> Result<()> {
        self.store
            .set(DISCOVERIES_TREE, &discovery.id, &discovery)?;

        // Update stats atomically
        self.store.update(STATS_TREE, "player", |old: Option<PlayerStats>| {
            let mut stats = old.unwrap_or_else(|| {
                let now = crate::utils::current_timestamp();
                PlayerStats {
                    first_played: now,
                    last_played: now,
                    ..Default::default()
                }
            });
            stats.discoveries_made += 1;
            Some(stats)
        })?;

        Ok(())
    }

    /// Get all discoveries (currently unused - kept for future features)
    #[allow(dead_code)]
    pub fn get_discoveries(&self) -> Result<Vec<Discovery>> {
        self.store.get_all(DISCOVERIES_TREE)
    }

    // --- User Facts & Context ---

    /// Record a fact about the user/environment
    pub fn record_fact(&self, key: &str, value: &str) -> Result<()> {
        let now = current_timestamp();

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
                let now = current_timestamp();
                PlayerStats {
                    first_played: now,
                    last_played: now,
                    ..Default::default()
                }
            })
            .pipe(Ok)
    }

    /// Save player statistics (currently unused - use update() instead)
    #[allow(dead_code)]
    pub fn save_stats(&self, stats: &PlayerStats) -> Result<()> {
        self.store.set(STATS_TREE, "player", stats)
    }

    /// Add playtime (currently unused - kept for session tracking)
    #[allow(dead_code)]
    pub fn add_playtime(&self, seconds: u64) -> Result<()> {
        self.store.update(STATS_TREE, "player", move |old: Option<PlayerStats>| {
            let mut stats = old.unwrap_or_else(|| {
                let now = crate::utils::current_timestamp();
                PlayerStats {
                    first_played: now,
                    last_played: now,
                    ..Default::default()
                }
            });
            stats.total_playtime_secs += seconds;
            stats.last_played = crate::utils::current_timestamp();
            Some(stats)
        })?;
        Ok(())
    }

    // --- HITL Feedback (Chapter 13) ---

    /// Record user feedback (thumbs up/down)
    pub fn record_feedback(&self, feedback: UserFeedback) -> Result<()> {
        self.store.set(FEEDBACK_TREE, &feedback.id, &feedback)?;

        // Update stats atomically
        let is_positive = feedback.is_positive;
        self.store.update(STATS_TREE, "player", move |old: Option<PlayerStats>| {
            let mut stats = old.unwrap_or_else(|| {
                let now = crate::utils::current_timestamp();
                PlayerStats {
                    first_played: now,
                    last_played: now,
                    ..Default::default()
                }
            });
            if is_positive {
                stats.total_positive_feedback += 1;
            } else {
                stats.total_negative_feedback += 1;
            }
            Some(stats)
        })?;

        Ok(())
    }

    /// Create feedback with auto-generated ID
    pub fn record_quick_feedback(
        &self,
        target: FeedbackTarget,
        content: &str,
        is_positive: bool,
        puzzle_id: Option<String>,
    ) -> Result<()> {
        let feedback = UserFeedback {
            id: format!("feedback_{}_{}", current_timestamp(), rand_suffix()),
            target,
            content: content.to_string(),
            is_positive,
            comment: None,
            puzzle_id,
            url: None,
            timestamp: current_timestamp(),
        };
        self.record_feedback(feedback)
    }

    /// Get all feedback
    pub fn get_feedback(&self) -> Result<Vec<UserFeedback>> {
        self.store.get_all(FEEDBACK_TREE)
    }

    /// Get feedback by type (currently unused - kept for analytics)
    #[allow(dead_code)]
    pub fn get_feedback_by_target(&self, target: FeedbackTarget) -> Result<Vec<UserFeedback>> {
        let all = self.get_feedback()?;
        Ok(all.into_iter().filter(|f| f.target == target).collect())
    }

    /// Get negative feedback for learning (what to avoid)
    pub fn get_negative_feedback(&self) -> Result<Vec<UserFeedback>> {
        let all = self.get_feedback()?;
        Ok(all.into_iter().filter(|f| !f.is_positive).collect())
    }

    /// Calculate feedback ratio (positive / total)
    pub fn get_feedback_ratio(&self) -> Result<f32> {
        let stats = self.get_stats()?;
        let total = stats.total_positive_feedback + stats.total_negative_feedback;
        if total == 0 {
            return Ok(1.0); // No feedback yet, assume positive
        }
        Ok(stats.total_positive_feedback as f32 / total as f32)
    }

    // --- Escalations (HITL "I'm stuck" mechanism) ---

    /// Record an escalation (user is stuck)
    pub fn record_escalation(&self, escalation: Escalation) -> Result<()> {
        self.store.set(ESCALATIONS_TREE, &escalation.id, &escalation)?;

        // Update stats atomically
        self.store.update(STATS_TREE, "player", |old: Option<PlayerStats>| {
            let mut stats = old.unwrap_or_else(|| {
                let now = crate::utils::current_timestamp();
                PlayerStats {
                    first_played: now,
                    last_played: now,
                    ..Default::default()
                }
            });
            stats.total_escalations += 1;
            Some(stats)
        })?;

        Ok(())
    }

    /// Create an escalation with auto-generated ID
    pub fn create_escalation(
        &self,
        puzzle_id: &str,
        time_stuck_secs: u64,
        hints_revealed: usize,
        current_url: &str,
        description: Option<String>,
    ) -> Result<Escalation> {
        let escalation = Escalation {
            id: format!("esc_{}_{}", current_timestamp(), rand_suffix()),
            puzzle_id: puzzle_id.to_string(),
            time_stuck_secs,
            hints_revealed,
            current_url: current_url.to_string(),
            description,
            resolved: false,
            resolution: None,
            timestamp: current_timestamp(),
        };
        self.record_escalation(escalation.clone())?;
        Ok(escalation)
    }

    /// Mark an escalation as resolved
    pub fn resolve_escalation(&self, escalation_id: &str, resolution: &str) -> Result<()> {
        let res_str = resolution.to_string();
        self.store.update(ESCALATIONS_TREE, escalation_id, move |old: Option<Escalation>| {
            if let Some(mut esc) = old {
                esc.resolved = true;
                esc.resolution = Some(res_str.clone());
                Some(esc)
            } else {
                None
            }
        })?;
        Ok(())
    }

    /// Get unresolved escalations (currently unused - kept for admin dashboard)
    #[allow(dead_code)]
    pub fn get_pending_escalations(&self) -> Result<Vec<Escalation>> {
        let all: Vec<Escalation> = self.store.get_all(ESCALATIONS_TREE)?;
        Ok(all.into_iter().filter(|e| !e.resolved).collect())
    }

    /// Get patterns from negative feedback for agent learning
    /// Returns common content that received negative feedback
    pub fn get_learning_patterns(&self) -> Result<Vec<String>> {
        let negative = self.get_negative_feedback()?;
        // Return the content that was rated negatively (for the agent to learn to avoid)
        Ok(negative.into_iter().map(|f| f.content).collect())
    }

    // --- Full Reset ---

    /// Reset all long-term memory (currently unused - kept for settings UI)
    #[allow(dead_code)]
    pub fn reset_all(&self) -> Result<()> {
        self.store.clear_tree(PUZZLES_TREE)?;
        self.store.clear_tree(DISCOVERIES_TREE)?;
        self.store.clear_tree(STATS_TREE)?;
        self.store.clear_tree(FEEDBACK_TREE)?;
        self.store.clear_tree(ESCALATIONS_TREE)?;
        Ok(())
    }
}

/// Generate random suffix for unique IDs
/// Uses atomic counter + nanoseconds to minimize collision risk
fn rand_suffix() -> u32 {
    use std::sync::atomic::{AtomicU32, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};
    
    static COUNTER: AtomicU32 = AtomicU32::new(0);
    
    let counter = COUNTER.fetch_add(1, Ordering::Relaxed);
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .subsec_nanos();
    
    // Combine counter and nanos for better uniqueness
    (counter.wrapping_mul(31) ^ nanos) % 1_000_000
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
