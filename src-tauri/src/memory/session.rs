//! Session memory - short-term working memory for current game session
//! Stores current puzzle state, recent interactions, activity history, and mode state

use super::store::MemoryStore;
use crate::utils::current_timestamp;
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::sync::atomic::{AtomicU32, Ordering};

const SESSION_TREE: &str = "session";
const ACTIVITY_TREE: &str = "activity_log";

/// Atomic counter for unique activity IDs within a session
static ACTIVITY_COUNTER: AtomicU32 = AtomicU32::new(0);

/// App mode - companion (passive) or game (active puzzle hunting)
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub enum AppMode {
    #[default]
    Game, // Active puzzle hunting mode
    Companion, // Passive observation mode
}

/// An activity entry for the activity log
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActivityEntry {
    /// Type of activity: "screenshot", "observation", "puzzle_attempt", "url_visit", "mode_change"
    pub activity_type: String,
    /// Human-readable description
    pub description: String,
    /// Unix timestamp
    pub timestamp: u64,
    /// Optional metadata (JSON value)
    #[serde(default)]
    pub metadata: Option<serde_json::Value>,
}

/// Current session state
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionState {
    /// Current puzzle ID
    pub puzzle_id: String,
    /// Current puzzle index
    pub puzzle_index: usize,
    /// Current URL being viewed
    pub current_url: String,
    /// Current page title
    pub current_title: String,
    /// Recent URLs visited (last 10) - VecDeque for O(1) push/pop at both ends
    pub recent_urls: VecDeque<String>,
    /// Current proximity score
    pub proximity: f32,
    /// Ghost mood/state
    pub ghost_state: String,
    /// Hints revealed count
    pub hints_revealed: usize,
    /// Session start timestamp
    pub started_at: u64,
    /// Last activity timestamp
    pub last_activity: u64,
    /// Current app mode
    #[serde(default)]
    pub current_mode: AppMode,
    /// Last mode change timestamp
    #[serde(default)]
    pub last_mode_change: u64,
    /// Puzzles solved this session
    #[serde(default)]
    pub puzzles_solved_session: usize,
    /// Screenshots taken this session
    #[serde(default)]
    pub screenshots_taken: usize,
    /// Latest page content for analysis
    #[serde(default, skip)]
    pub current_content: Option<String>,
}

impl Default for SessionState {
    fn default() -> Self {
        let now = current_timestamp();

        Self {
            puzzle_id: String::new(), // Start empty for dynamic generation
            puzzle_index: 0,
            current_url: String::new(),
            current_title: String::new(),
            recent_urls: VecDeque::with_capacity(10),
            proximity: 0.0,
            ghost_state: "idle".to_string(),
            hints_revealed: 0,
            started_at: now,
            last_activity: now,
            current_mode: AppMode::Game,
            last_mode_change: now,
            puzzles_solved_session: 0,
            screenshots_taken: 0,
            current_content: None,
        }
    }
}

/// Session memory manager
pub struct SessionMemory {
    store: MemoryStore,
}

impl SessionMemory {
    pub fn new(store: MemoryStore) -> Self {
        Self { store }
    }

    /// Load current session state
    /// Returns the saved state if it exists, otherwise returns a new default state
    pub fn load(&self) -> Result<SessionState> {
        Ok(self.store.get(SESSION_TREE, "current")?.unwrap_or_default())
    }

    /// Save session state
    pub fn save(&self, state: &SessionState) -> Result<()> {
        self.store.set(SESSION_TREE, "current", state)
    }

    /// Update last activity timestamp
    pub fn touch(&self) -> Result<()> {
        self.store.update(SESSION_TREE, "current", |old: Option<SessionState>| {
            let mut state = old.unwrap_or_default();
            state.last_activity = current_timestamp();
            Some(state)
        })?;
        Ok(())
    }

    /// Store latest page content
    pub fn store_content(&self, content: String) -> Result<()> {
        self.store.update(SESSION_TREE, "current", |old: Option<SessionState>| {
            let mut state = old.unwrap_or_default();
            state.current_content = Some(content.clone());
            Some(state)
        })?;
        Ok(())
    }

    /// Add URL to recent history
    /// Uses VecDeque for O(1) operations at both ends
    pub fn add_url(&self, url: &str) -> Result<()> {
        let url_string = url.to_string();
        self.store.update(SESSION_TREE, "current", move |old: Option<SessionState>| {
            let mut state = old.unwrap_or_default();
            state.current_url = url_string.clone();
            state.recent_urls.push_back(url_string.clone());
            // Keep only last 10 URLs - O(1) pop from front with VecDeque
            while state.recent_urls.len() > 10 {
                state.recent_urls.pop_front();
            }
            state.last_activity = current_timestamp();
            Some(state)
        })?;
        Ok(())
    }

    /// Update proximity score
    pub fn set_proximity(&self, proximity: f32) -> Result<()> {
        self.store.update(SESSION_TREE, "current", |old: Option<SessionState>| {
            let mut state = old.unwrap_or_default();
            state.proximity = proximity;
            Some(state)
        })?;
        Ok(())
    }

    /// Set the current app mode
    pub fn set_mode(&self, mode: AppMode) -> Result<()> {
        let mode_clone = mode.clone();
        
        // We need to check if mode changed to log it, so we do this in two steps or inside update
        // Inside update is safer for consistency
        self.store.update(SESSION_TREE, "current", |old: Option<SessionState>| {
            let mut state = old.unwrap_or_default();
            if state.current_mode != mode_clone {
                state.current_mode = mode_clone.clone();
                state.last_mode_change = current_timestamp();
                // Note: We can't easily log activity inside this closure because add_activity 
                // requires &self and returns Result, which doesn't fit the closure signature.
                // We'll accept a small race here or move logging outside.
                // Given logging is append-only, it's safe to do outside.
            }
            Some(state)
        })?;
        
        // Log activity (best effort)
        self.add_activity(ActivityEntry {
            activity_type: "mode_change".to_string(),
            description: format!("Switched to {:?} mode", mode),
            timestamp: current_timestamp(),
            metadata: None,
        })?;
        
        Ok(())
    }

    /// Get current app mode
    pub fn get_mode(&self) -> Result<AppMode> {
        Ok(self.load()?.current_mode)
    }

    /// Increment screenshot counter
    pub fn record_screenshot(&self) -> Result<()> {
        self.store.update(SESSION_TREE, "current", |old: Option<SessionState>| {
            let mut state = old.unwrap_or_default();
            state.screenshots_taken += 1;
            state.last_activity = current_timestamp();
            Some(state)
        })?;
        Ok(())
    }

    /// Increment puzzles solved counter
    pub fn record_puzzle_solved(&self) -> Result<()> {
        self.store.update(SESSION_TREE, "current", |old: Option<SessionState>| {
            let mut state = old.unwrap_or_default();
            state.puzzles_solved_session += 1;
            Some(state)
        })?;
        Ok(())
    }

    // --- Activity Log ---

    /// Add an activity entry to the log with lazy pruning
    /// Uses atomic counter to prevent key collisions for entries in the same second
    pub fn add_activity(&self, entry: ActivityEntry) -> Result<()> {
        // Use timestamp + atomic counter to guarantee unique keys
        let counter = ACTIVITY_COUNTER.fetch_add(1, Ordering::Relaxed);
        let key = format!("activity_{}_{:04}", entry.timestamp, counter % 10000);
        self.store.set(ACTIVITY_TREE, &key, &entry)?;

        // Probabilistic pruning: only check count occasionally (1 in 10 writes)
        // This reduces the overhead of count() calls while still preventing unbounded growth
        if counter % 10 == 0 {
            let count = self.store.count(ACTIVITY_TREE)?;
            // Use higher threshold to reduce pruning frequency
            if count > 300 {
                self.prune_activity(200)?;
            }
        }
        Ok(())
    }

    /// Get recent activity entries (last N)
    pub fn get_recent_activity(&self, limit: usize) -> Result<Vec<ActivityEntry>> {
        let mut entries: Vec<ActivityEntry> = self.store.get_all(ACTIVITY_TREE)?;
        // Sort by timestamp descending
        entries.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));
        entries.truncate(limit);
        Ok(entries)
    }

    /// Clear activity log (keeps last N entries)
    pub fn prune_activity(&self, keep_count: usize) -> Result<()> {
        let mut entries: Vec<ActivityEntry> = self.store.get_all(ACTIVITY_TREE)?;
        if entries.len() <= keep_count {
            return Ok(());
        }

        entries.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));
        let to_delete: Vec<_> = entries.into_iter().skip(keep_count).collect();

        for entry in to_delete {
            let key = format!("activity_{}", entry.timestamp);
            let _ = self.store.delete(ACTIVITY_TREE, &key);
        }
        Ok(())
    }

    /// Reset session for new game
    pub fn reset(&self) -> Result<()> {
        self.store.delete(SESSION_TREE, "current")?;
        self.store.clear_tree(ACTIVITY_TREE)?;
        Ok(())
    }

    /// Flush all pending writes to disk
    pub fn flush(&self) -> Result<()> {
        self.store.flush()
    }
}
