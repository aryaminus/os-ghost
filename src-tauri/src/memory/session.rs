//! Session memory - short-term working memory for current game session
//! Stores current puzzle state, recent interactions, activity history, and mode state

use super::scoped_state::ScopedState;
use super::store::MemoryStore;
use crate::utils::current_timestamp;
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::sync::atomic::{AtomicU32, Ordering};

const SESSION_TREE: &str = "session";
const ACTIVITY_TREE: &str = "activity_log";
const SCOPED_STATE_TREE: &str = "scoped_state";

/// Atomic counter for unique activity IDs within a session
static ACTIVITY_COUNTER: AtomicU32 = AtomicU32::new(0);

fn default_true() -> bool {
    true
}

/// App mode - companion (passive) or game (active puzzle hunting)
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub enum AppMode {
    Game, // Active puzzle hunting mode
    #[default]
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
    /// Timestamp when current puzzle started
    #[serde(default)]
    pub puzzle_started_at: u64,
    /// Last activity timestamp
    pub last_activity: u64,
    /// Current app mode (runtime state)
    #[serde(default)]
    pub current_mode: AppMode,
    /// Preferred mode (persisted user preference)
    #[serde(default)]
    pub preferred_mode: AppMode,
    /// Auto-create puzzles from companion suggestions
    #[serde(default = "default_true")]
    pub auto_puzzle_from_companion: bool,
    /// Last mode change timestamp
    #[serde(default)]
    pub last_mode_change: u64,
    /// Puzzles solved this session
    #[serde(default)]
    pub puzzles_solved_session: usize,
    /// Screenshots taken this session
    #[serde(default)]
    pub screenshots_taken: usize,
    /// Last screenshot timestamp
    #[serde(default)]
    pub last_screenshot_at: u64,
    /// Timestamp of last successful AI analysis (seconds since UNIX epoch)
    pub last_analysis_at: u64,
    /// Timestamp of last auto-generated intent action
    pub last_intent_action_at: u64,
    /// Auto intent cooldown override (seconds)
    #[serde(default)]
    pub intent_cooldown_secs: u64,
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
            puzzle_started_at: 0,
            last_activity: now,
            current_mode: AppMode::Companion,
            preferred_mode: AppMode::Companion,
            auto_puzzle_from_companion: true,
            last_mode_change: now,
            puzzles_solved_session: 0,
            screenshots_taken: 0,
            last_screenshot_at: 0,
            last_analysis_at: 0,
            last_intent_action_at: 0,
            intent_cooldown_secs: 0,
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
        self.store
            .update(SESSION_TREE, "current", |old: Option<SessionState>| {
                let mut state = old.unwrap_or_default();
                state.last_activity = current_timestamp();
                Some(state)
            })?;
        Ok(())
    }

    /// Store latest page content
    pub fn store_content(&self, content: String) -> Result<()> {
        self.store
            .update(SESSION_TREE, "current", |old: Option<SessionState>| {
                let mut state = old.unwrap_or_default();
                state.current_content = Some(content.clone());
                Some(state)
            })?;
        Ok(())
    }

    /// Add URL to recent history
    /// Uses VecDeque for O(1) operations at both ends
    pub fn add_url(&self, url: &str) -> Result<()> {
        self.update_current_page(url, None)
    }

    /// Update current URL/title and recent URLs.
    ///
    /// This is used by both the browser bridge (passive updates) and the agent
    /// orchestrator (active cycles).
    pub fn update_current_page(&self, url: &str, title: Option<&str>) -> Result<()> {
        let url_string = url.to_string();
        let title_string = title.map(|t| t.to_string());

        self.store
            .update(SESSION_TREE, "current", move |old: Option<SessionState>| {
                let mut state = old.unwrap_or_default();
                state.current_url = url_string.clone();
                if let Some(ref t) = title_string {
                    state.current_title = t.clone();
                }
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
        self.store
            .update(SESSION_TREE, "current", |old: Option<SessionState>| {
                let mut state = old.unwrap_or_default();
                state.proximity = proximity;
                Some(state)
            })?;
        Ok(())
    }

    /// Set the current app mode
    pub fn set_mode(&self, mode: AppMode) -> Result<()> {
        // Avoid redundant writes/logs if mode is unchanged
        if self.get_mode()? == mode {
            return Ok(());
        }

        let mode_clone = mode.clone();

        // Update state (atomic at the key level)
        self.store
            .update(SESSION_TREE, "current", |old: Option<SessionState>| {
                let mut state = old.unwrap_or_default();
                state.current_mode = mode_clone.clone();
                state.last_mode_change = current_timestamp();
                Some(state)
            })?;

        // Log activity (best effort). This is intentionally outside the atomic update
        // because sled's update closure cannot return Result.
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

    /// Get preferred app mode
    pub fn get_preferred_mode(&self) -> Result<AppMode> {
        Ok(self.load()?.preferred_mode)
    }

    /// Set preferred app mode
    pub fn set_preferred_mode(&self, mode: AppMode) -> Result<()> {
        let mode_clone = mode.clone();
        self.store
            .update(SESSION_TREE, "current", |old: Option<SessionState>| {
                let mut state = old.unwrap_or_default();
                state.preferred_mode = mode_clone.clone();
                Some(state)
            })?;
        Ok(())
    }

    /// Get auto puzzle setting
    pub fn get_auto_puzzle_from_companion(&self) -> Result<bool> {
        Ok(self.load()?.auto_puzzle_from_companion)
    }

    /// Set auto puzzle setting
    pub fn set_auto_puzzle_from_companion(&self, enabled: bool) -> Result<()> {
        self.store
            .update(SESSION_TREE, "current", |old: Option<SessionState>| {
                let mut state = old.unwrap_or_default();
                state.auto_puzzle_from_companion = enabled;
                Some(state)
            })?;
        Ok(())
    }

    /// Increment screenshot counter
    pub fn record_screenshot(&self) -> Result<()> {
        self.store
            .update(SESSION_TREE, "current", |old: Option<SessionState>| {
                let mut state = old.unwrap_or_default();
                state.screenshots_taken += 1;
                state.last_activity = current_timestamp();
                state.last_screenshot_at = current_timestamp();
                Some(state)
            })?;
        Ok(())
    }

    /// Record successful AI analysis timestamp
    pub fn record_analysis(&self) -> Result<()> {
        self.store
            .update(SESSION_TREE, "current", |old: Option<SessionState>| {
                let mut state = old.unwrap_or_default();
                state.last_analysis_at = current_timestamp();
                Some(state)
            })?;
        Ok(())
    }

    /// Record an intent action creation timestamp
    pub fn record_intent_action(&self) -> Result<()> {
        self.store
            .update(SESSION_TREE, "current", |old: Option<SessionState>| {
                let mut state = old.unwrap_or_default();
                state.last_intent_action_at = current_timestamp();
                Some(state)
            })?;
        Ok(())
    }

    pub fn set_intent_cooldown(&self, seconds: u64) -> Result<()> {
        self.store
            .update(SESSION_TREE, "current", |old: Option<SessionState>| {
                let mut state = old.unwrap_or_default();
                state.intent_cooldown_secs = seconds;
                Some(state)
            })?;
        Ok(())
    }

    pub fn get_intent_cooldown(&self) -> Result<u64> {
        let state = self.load()?;
        Ok(state.intent_cooldown_secs)
    }

    /// Increment puzzles solved counter
    pub fn record_puzzle_solved(&self) -> Result<()> {
        self.store
            .update(SESSION_TREE, "current", |old: Option<SessionState>| {
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
        if counter.is_multiple_of(10) {
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
        let mut keys = self.store.list_keys(ACTIVITY_TREE)?;
        if keys.len() <= keep_count {
            return Ok(());
        }

        fn parse_key(key: &str) -> (u64, u32) {
            // Expected: activity_<timestamp>_<counter>
            let rest = key.strip_prefix("activity_").unwrap_or(key);
            let mut parts = rest.split('_');
            let ts = parts
                .next()
                .and_then(|p| p.parse::<u64>().ok())
                .unwrap_or(0);
            let counter = parts
                .next()
                .and_then(|p| p.parse::<u32>().ok())
                .unwrap_or(0);
            (ts, counter)
        }

        // Sort newest-first by (timestamp, counter)
        keys.sort_by(|a, b| {
            let (a_ts, a_ctr) = parse_key(a);
            let (b_ts, b_ctr) = parse_key(b);
            b_ts.cmp(&a_ts)
                .then_with(|| b_ctr.cmp(&a_ctr))
                .then_with(|| b.cmp(a))
        });

        for key in keys.into_iter().skip(keep_count) {
            let _ = self.store.delete(ACTIVITY_TREE, &key);
        }

        Ok(())
    }

    // --- Scoped State (ADK-style) ---

    /// Load scoped state from storage
    pub fn load_scoped_state(&self) -> Result<ScopedState> {
        Ok(self
            .store
            .get(SCOPED_STATE_TREE, "current")?
            .unwrap_or_default())
    }

    /// Save scoped state to storage
    pub fn save_scoped_state(&self, state: &ScopedState) -> Result<()> {
        self.store.set(SCOPED_STATE_TREE, "current", state)
    }

    /// Get a scoped value by key (supports temp:, user:, app:, session: prefixes)
    pub fn get_scoped(&self, key: &str) -> Result<Option<serde_json::Value>> {
        let state = self.load_scoped_state()?;
        Ok(state.get(key).cloned())
    }

    /// Set a scoped value by key (supports temp:, user:, app:, session: prefixes)
    pub fn set_scoped(&self, key: &str, value: serde_json::Value) -> Result<()> {
        self.store
            .update(SCOPED_STATE_TREE, "current", |old: Option<ScopedState>| {
                let mut state = old.unwrap_or_default();
                state.set(key, value.clone());
                Some(state)
            })?;
        Ok(())
    }

    /// Clear temp-scoped state (call at start of each invocation)
    /// This follows ADK pattern where temp: state is discarded after each turn
    pub fn clear_temp_state(&self) -> Result<()> {
        self.store
            .update(SCOPED_STATE_TREE, "current", |old: Option<ScopedState>| {
                let mut state = old.unwrap_or_default();
                state.clear_temp();
                Some(state)
            })?;
        Ok(())
    }

    /// Apply a state delta from an EventActions
    pub fn apply_state_delta(
        &self,
        delta: &std::collections::HashMap<String, serde_json::Value>,
    ) -> Result<()> {
        if delta.is_empty() {
            return Ok(());
        }
        self.store
            .update(SCOPED_STATE_TREE, "current", |old: Option<ScopedState>| {
                let mut state = old.unwrap_or_default();
                state.apply_delta(delta);
                Some(state)
            })?;
        Ok(())
    }

    /// Reset session for new game
    pub fn reset(&self) -> Result<()> {
        self.store.delete(SESSION_TREE, "current")?;
        self.store.clear_tree(ACTIVITY_TREE)?;
        // Clear session-scoped state but preserve user: and app: scopes
        self.store
            .update(SCOPED_STATE_TREE, "current", |old: Option<ScopedState>| {
                let mut state = old.unwrap_or_default();
                state.clear_session();
                state.clear_temp();
                Some(state)
            })?;
        Ok(())
    }

    /// Flush all pending writes to disk
    pub fn flush(&self) -> Result<()> {
        self.store.flush()
    }
}
