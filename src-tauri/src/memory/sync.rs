//! memory/sync.rs
//! Interface for Cloud Sync functionality (Vertex AI / Firestore)

use crate::game_state::GameState;
use anyhow::Result;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncStatus {
    pub last_sync: Option<u64>,
    pub connected: bool,
    pub provider: String,
}

#[async_trait]
pub trait SyncProvider: Send + Sync {
    /// Initialize connection
    async fn connect(&self) -> Result<()>;

    /// Push local state to cloud
    async fn sync_up(&self, state: &GameState) -> Result<()>;

    /// Pull cloud state to local
    async fn sync_down(&self) -> Result<Option<GameState>>;

    /// Get provider status
    fn status(&self) -> SyncStatus;
}

/// Stub implementation for Vertex AI
pub struct VertexAISync {
    pub _project_id: String,
    connected: bool,
}

impl VertexAISync {
    pub fn new(project_id: String) -> Self {
        Self {
            _project_id: project_id,
            connected: false,
        }
    }
}

#[async_trait]
impl SyncProvider for VertexAISync {
    async fn connect(&self) -> Result<()> {
        // TODO: Implement Vertex AI auth
        Ok(())
    }

    async fn sync_up(&self, _state: &GameState) -> Result<()> {
        // TODO: Implement Firestore write
        Ok(())
    }

    async fn sync_down(&self) -> Result<Option<GameState>> {
        // TODO: Implement Firestore read
        Ok(None)
    }

    fn status(&self) -> SyncStatus {
        SyncStatus {
            last_sync: None,
            connected: self.connected,
            provider: "Vertex AI".to_string(),
        }
    }
}
