//! Session memory - short-term working memory for current game session
//! Stores current puzzle state, recent interactions, and temporary data

use super::store::MemoryStore;
use anyhow::Result;
use serde::{Deserialize, Serialize};

const SESSION_TREE: &str = "session";

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
}

impl Default for SessionState {
    fn default() -> Self {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        Self {
            puzzle_id: "puzzle_001".to_string(),
            puzzle_index: 0,
            current_url: String::new(),
            current_title: String::new(),
            recent_urls: Vec::new(),
            proximity: 0.0,
            ghost_state: "idle".to_string(),
            hints_revealed: 0,
            started_at: now,
            last_activity: now,
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

    /// Add URL to recent history
    pub fn add_url(&self, url: &str) -> Result<()> {
        let mut state = self.load()?;
        state.current_url = url.to_string();
        state.recent_urls.push(url.to_string());
        if state.recent_urls.len() > 10 {
            state.recent_urls.remove(0);
        }
        self.save(&state)
    }

    /// Update proximity score
    pub fn set_proximity(&self, proximity: f32) -> Result<()> {
        let mut state = self.load()?;
        state.proximity = proximity;
        self.save(&state)
    }

    /// Reset session for new game
    pub fn reset(&self) -> Result<()> {
        self.store.delete(SESSION_TREE, "current")
    }
}
