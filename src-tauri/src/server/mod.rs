//! OS-Ghost Headless Server and CLI Module
//!
//! Provides HTTP REST API and WebSocket support for headless operation,
//! matching UI-TARS's server capabilities while maintaining OS-Ghost's
//! unique companion features.

pub mod api;
pub mod websocket;
pub mod auth;
pub mod state;

use std::net::SocketAddr;
use std::sync::Arc;
use axum::{
    routing::{get, post},
    Router,
};
use tokio::sync::RwLock;
use tracing::info;

use crate::server::state::ServerState;
use crate::server::api::{
    execute_task,
    get_status,
    get_workflows,
    start_recording,
    stop_recording,
    execute_workflow,
    get_agents,
    get_memory,
    get_pending_actions,
    approve_action,
    deny_action,
};
use crate::server::websocket::ws_handler;

// Re-export ServerConfig from config module
pub use crate::config::server::ServerConfig;

/// OS-Ghost Server instance
pub struct GhostServer {
    config: ServerConfig,
    state: Arc<RwLock<ServerState>>,
}

impl GhostServer {
    /// Create a new server instance
    pub fn new(config: ServerConfig) -> Self {
        let state = Arc::new(RwLock::new(ServerState::new()));
        
        Self { config, state }
    }

    /// Start the HTTP server
    pub async fn start(&self) -> anyhow::Result<()> {
        let addr: SocketAddr = format!("{}:{}", self.config.host, self.config.port)
            .parse()
            .map_err(|e| anyhow::anyhow!("Invalid address: {}", e))?;

        info!("Starting OS-Ghost server on {}", addr);

        // Build router
        let app = self.build_router().await?;

        // Start server
        let listener = tokio::net::TcpListener::bind(addr).await
            .map_err(|e| anyhow::anyhow!("Failed to bind: {}", e))?;
        
        info!("OS-Ghost server listening on {}", addr);
        info!("API endpoints available at http://{}/api/v1", addr);
        
        if self.config.enable_websocket {
            info!("WebSocket endpoint available at ws://{}/ws", addr);
        }

        axum::serve(listener, app).await
            .map_err(|e| anyhow::anyhow!("Server error: {}", e))?;

        Ok(())
    }

    /// Build the Axum router
    async fn build_router(&self) -> anyhow::Result<Router> {
        let state = self.state.clone();
        
        // CORS middleware
        let cors = if self.config.enable_cors {
            tower_http::cors::CorsLayer::permissive()
        } else {
            tower_http::cors::CorsLayer::new()
        };

        // Build routes
        let mut router = Router::new()
            // Health check
            .route("/", get(root_handler))
            .route("/health", get(health_handler))
            // API v1 routes
            .route("/api/v1/status", get(get_status))
            .route("/api/v1/execute", post(execute_task))
            .route("/api/v1/workflows", get(get_workflows))
            .route("/api/v1/workflows/:id/execute", post(execute_workflow))
            .route("/api/v1/record/start", post(start_recording))
            .route("/api/v1/record/stop", post(stop_recording))
            .route("/api/v1/agents", get(get_agents))
            .route("/api/v1/memory", get(get_memory))
            .route("/api/v1/pending-actions", get(get_pending_actions))
            .route("/api/v1/actions/:id/approve", post(approve_action))
            .route("/api/v1/actions/:id/deny", post(deny_action))
            .with_state(state)
            .layer(cors);

        // Add WebSocket route if enabled
        if self.config.enable_websocket {
            router = router.route("/ws", get(ws_handler));
        }

        Ok(router)
    }
}

/// Root handler - returns server info
async fn root_handler() -> impl axum::response::IntoResponse {
    axum::Json(serde_json::json!({
        "name": "OS-Ghost Server",
        "version": env!("CARGO_PKG_VERSION"),
        "status": "running",
        "endpoints": {
            "health": "/health",
            "api": "/api/v1",
            "websocket": "/ws"
        }
    }))
}

/// Health check handler
async fn health_handler(
    axum::extract::State(state): axum::extract::State<Arc<RwLock<ServerState>>>,
) -> impl axum::response::IntoResponse {
    let state = state.read().await;
    
    axum::Json(serde_json::json!({
        "status": "healthy",
        "version": env!("CARGO_PKG_VERSION"),
        "connected": state.connected,
        "active_agents": state.active_agents.len(),
        "pending_actions": state.pending_actions.len(),
        "workflows_count": state.workflows.len(),
        "memory_entries": state.memory_entries,
        "timestamp": chrono::Utc::now().to_rfc3339(),
    }))
}

/// Initialize server logging
pub fn init_logging() {
    tracing_subscriber::fmt()
        .with_env_filter("os_ghost_lib=info,server=info")
        .init();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_server_config() {
        let config = ServerConfig::default();
        assert_eq!(config.port, 7842);
        assert_eq!(config.host, "127.0.0.1");
    }
}
