//! Session memory - short-term working memory for current game session
//! Stores current puzzle state, recent interactions, activity history, and mode state

use super::store::MemoryStore;
use anyhow::Result;
use serde::{Deserialize, Serialize};

const SESSION_TREE: &str = "session";
const ACTIVITY_TREE: &str = "activity_log";

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
    /// Recent URLs visited (last 10)
    pub recent_urls: Vec<String>,
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
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        Self {
            puzzle_id: String::new(), // Start empty for dynamic generation
            puzzle_index: 0,
            current_url: String::new(),
            current_title: String::new(),
            recent_urls: Vec::new(),
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
    pub fn load(&self) -> Result<SessionState> {
        self.store
            .get(SESSION_TREE, "current")?
            .ok_or_else(|| anyhow::anyhow!("No session found"))
            .or_else(|_| Ok(SessionState::default()))
    }

    /// Save session state
    pub fn save(&self, state: &SessionState) -> Result<()> {
        self.store.set(SESSION_TREE, "current", state)
    }

    /// Update last activity timestamp
    pub fn touch(&self) -> Result<()> {
        let mut state = self.load()?;
        state.last_activity = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        self.save(&state)
    }

    /// Store latest page content
    pub fn store_content(&self, content: String) -> Result<()> {
        let mut state = self.load()?;
        state.current_content = Some(content);
        self.save(&state)
    }

    /// Add URL to recent history
    pub fn add_url(&self, url: &str) -> Result<()> {
        let mut state = self.load()?;
        state.current_url = url.to_string();
        state.recent_urls.push(url.to_string());
        if state.recent_urls.len() > 10 {
            state.recent_urls.remove(0);
        }
        state.last_activity = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        self.save(&state)
    }

    /// Update proximity score
    pub fn set_proximity(&self, proximity: f32) -> Result<()> {
        let mut state = self.load()?;
        state.proximity = proximity;
        self.save(&state)
    }

    /// Set the current app mode
    pub fn set_mode(&self, mode: AppMode) -> Result<()> {
        let mut state = self.load()?;
        if state.current_mode != mode {
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs();
            state.current_mode = mode.clone();
            state.last_mode_change = now;

            // Log mode change
            self.add_activity(ActivityEntry {
                activity_type: "mode_change".to_string(),
                description: format!("Switched to {:?} mode", mode),
                timestamp: now,
                metadata: None,
            })?;

            self.save(&state)?;
        }
        Ok(())
    }

    /// Get current app mode
    pub fn get_mode(&self) -> Result<AppMode> {
        Ok(self.load()?.current_mode)
    }

    /// Increment screenshot counter
    pub fn record_screenshot(&self) -> Result<()> {
        let mut state = self.load()?;
        state.screenshots_taken += 1;
        state.last_activity = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        self.save(&state)
    }

    /// Increment puzzles solved counter
    pub fn record_puzzle_solved(&self) -> Result<()> {
        let mut state = self.load()?;
        state.puzzles_solved_session += 1;
        self.save(&state)
    }

    // --- Activity Log ---

    /// Add an activity entry to the log and prune old entries
    pub fn add_activity(&self, entry: ActivityEntry) -> Result<()> {
        let key = format!("activity_{}", entry.timestamp);
        self.store.set(ACTIVITY_TREE, &key, &entry)?;
        // Auto-prune to keep history manageable (prevent unlimited growth)
        // Optimization: Only prune if we exceed limit + buffer (e.g. 250 items for 200 limit)
        // This prevents expensive O(N) sort/delete on every single write
        let count = self.store.count(ACTIVITY_TREE)?;
        if count > 250 {
            self.prune_activity(200)?;
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
}
