//! Native Messaging bridge for Chrome extension communication
//! Handles real-time browser events from the extension via TCP
//!
//! MCP Integration:
//! This bridge now exposes the browser as an MCP-compatible server, allowing
//! agents to discover and invoke browser capabilities through a standardized
//! interface. See `mcp::browser` for the full MCP implementation.

use crate::game_state::EffectQueue;
use crate::mcp::browser::{BrowserMcpServer, BrowserState};
use crate::mcp::{McpServer, ToolRequest};
use crate::system_status;
use crate::timeline::{record_timeline_event, TimelineEntryType, TimelineStatus};
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::Arc;
use tauri::{AppHandle, Emitter, Manager};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::mpsc;
use uuid::Uuid;

const BRIDGE_PORT: u16 = 9876;
/// Maximum concurrent connections to prevent DoS
const MAX_CONNECTIONS: usize = 10;

/// Global connection counter for limiting concurrent connections
static ACTIVE_CONNECTIONS: AtomicUsize = AtomicUsize::new(0);
static LAST_STATUS_EMIT: AtomicU64 = AtomicU64::new(0);
const STATUS_EMIT_THROTTLE_SECS: u64 = 2;

fn emit_status_throttled(app: &AppHandle) {
    let now = crate::utils::current_timestamp();
    let last = LAST_STATUS_EMIT.load(Ordering::SeqCst);
    if now.saturating_sub(last) < STATUS_EMIT_THROTTLE_SECS {
        return;
    }
    LAST_STATUS_EMIT.store(now, Ordering::SeqCst);
    crate::ipc::emit_system_status_update(app);
}

/// Message received from Chrome extension (via native_bridge)
#[derive(Debug, Deserialize)]
pub struct BrowserMessage {
    #[serde(rename = "type")]
    pub msg_type: String,
    pub url: Option<String>,
    pub title: Option<String>,
    pub body_text: Option<String>,
    pub timestamp: Option<i64>,
    pub recent_history: Option<Vec<serde_json::Value>>,
    pub top_sites: Option<Vec<serde_json::Value>>,
    pub protocol_version: Option<String>,
    pub extension_version: Option<String>,
    pub extension_id: Option<String>,
    pub capabilities: Option<serde_json::Value>,
}

/// Response sent back to native_bridge
#[derive(Debug, Serialize)]
pub struct NativeResponse {
    pub action: String,
    pub success: bool,
    pub data: serde_json::Value,
}

/// Connection guard that decrements counter on drop
struct ConnectionGuard;

impl Drop for ConnectionGuard {
    fn drop(&mut self) {
        ACTIVE_CONNECTIONS.fetch_sub(1, Ordering::SeqCst);
        tracing::debug!(
            "Connection closed. Active connections: {}",
            ACTIVE_CONNECTIONS.load(Ordering::SeqCst)
        );
    }
}

/// MCP-aware bridge context passed to handlers
pub struct McpBridgeContext {
    pub mcp_server: Arc<BrowserMcpServer>,
    pub effect_receiver: Arc<tokio::sync::Mutex<mpsc::Receiver<serde_json::Value>>>,
}

/// Handle a single client connection (Async)
async fn handle_client(mut stream: TcpStream, app: AppHandle, mcp_ctx: Arc<McpBridgeContext>) {
    // Create guard to ensure connection count is decremented on exit
    let _guard = ConnectionGuard;

    tracing::info!(
        "Native bridge connected from {:?}. Active connections: {}",
        stream.peer_addr(),
        ACTIVE_CONNECTIONS.load(Ordering::SeqCst)
    );

    // Mark browser as connected in MCP state
    mcp_ctx
        .mcp_server
        .state()
        .is_connected
        .store(true, Ordering::SeqCst);

    system_status::update_status(|status| {
        status.extension_connected = true;
        status.mcp_browser_connected = true;
        status.last_extension_heartbeat = Some(crate::utils::current_timestamp());
        status.extension_operational = true;
    });

    emit_status_throttled(&app);

    record_timeline_event(
        "Extension connected",
        None,
        TimelineEntryType::System,
        TimelineStatus::Info,
    );

    // Emit connection event to frontend
    let _ = app.emit(
        "extension_connected",
        serde_json::json!({ "connected": true }),
    );

    // Log MCP manifest for debugging/discovery
    let manifest = mcp_ctx.mcp_server.manifest();
    tracing::info!(
        "MCP Browser Server ready: {} tools, {} resources, {} prompts",
        manifest.tools.len(),
        manifest.resources.len(),
        manifest.prompts.len()
    );

    // No set_read_timeout in tokio TcpStream, use tokio::time::timeout if needed logic

    loop {
        // Read length prefix (4 bytes, little-endian)
        let mut len_buf = [0u8; 4];
        
        // Use timeout for reads to detect dead connections
        let read_result = tokio::time::timeout(
            std::time::Duration::from_secs(30),
            stream.read_exact(&mut len_buf)
        ).await;

        match read_result {
            Ok(Ok(_)) => {} // Success
            Ok(Err(_)) => break, // Connection closed or error
            Err(_) => {
                // Timeout
                tracing::debug!("Connection timed out waiting for message header");
                break; 
            }, 
        }

        let msg_len = u32::from_le_bytes(len_buf) as usize;
        if msg_len == 0 || msg_len > 1024 * 1024 {
            tracing::warn!("Invalid message length: {}", msg_len);
            continue;
        }

        // Read message
        let mut msg_buf = vec![0u8; msg_len];
        if stream.read_exact(&mut msg_buf).await.is_err() {
            break;
        }

        // Parse and handle message
        if let Ok(message) = serde_json::from_slice::<BrowserMessage>(&msg_buf) {
            tracing::debug!("Received from Chrome: {:?}", message);

            // Update MCP state based on message type
            let mcp_state = mcp_ctx.mcp_server.state();

            match message.msg_type.as_str() {
                "hello" => {
                    let protocol = message.protocol_version.clone();
                    let version = message.extension_version.clone();
                    let extension_id = message.extension_id.clone();
                    let capabilities = message.capabilities.clone();

                    system_status::update_status(|status| {
                        status.extension_protocol_version = protocol;
                        status.extension_version = version;
                        status.extension_id = extension_id.clone();
                        status.extension_capabilities = capabilities;
                    });

                    emit_status_throttled(&app);

                    if let Some(id) = extension_id {
                        crate::pairing::ensure_trusted_source(
                            &id,
                            "extension",
                            "Chrome Extension",
                        );
                    }
                }
                "heartbeat" => {
                    let now = crate::utils::current_timestamp();
                    system_status::update_status(|status| {
                        status.last_extension_heartbeat = Some(now);
                        status.extension_operational = true;
                    });

                    emit_status_throttled(&app);
                }
                "page_load" | "tab_changed" => {
                    let url = message.url.clone().unwrap_or_default();
                    let title = message.title.clone().unwrap_or_default();
                    let timestamp = message.timestamp.unwrap_or(0);

                    // Update MCP Resource state
                    mcp_state
                        .update_page(url.clone(), title.clone(), String::new(), timestamp)
                        .await;

                    // Keep SessionMemory in sync even when not running agent cycles
                    if let Some(session_mem) = app.try_state::<Arc<crate::memory::SessionMemory>>() {
                        if let Err(e) = session_mem.update_current_page(&url, Some(&title)) {
                            tracing::warn!("Failed to update session page: {}", e);
                        }
                    }

                    let now = crate::utils::current_timestamp();
                    system_status::update_status(|status| {
                        status.last_known_url = Some(url.clone());
                        status.last_page_update = Some(now);
                    });

                    emit_status_throttled(&app);

                    // Emit to frontend (legacy event for backward compatibility)
                    let _ = app.emit(
                        "browser_navigation",
                        serde_json::json!({
                            "url": url,
                            "title": title,
                            "timestamp": timestamp
                        }),
                    );
                }
                "page_content" => {
                    let url = message.url.clone().unwrap_or_default();
                    let title = message.title.clone().unwrap_or_default();
                    let body_text = message.body_text.clone().unwrap_or_default();
                    let timestamp = message.timestamp.unwrap_or(chrono::Utc::now().timestamp());

                    tracing::info!(
                        "Received page_content: url={} ({} bytes)",
                        url,
                        body_text.len()
                    );

                    // Update MCP Resource state with full content
                    mcp_state
                        .update_page(url.clone(), title.clone(), body_text.clone(), timestamp)
                        .await;

                    // 1. Store in memory (legacy path)
                    use crate::memory::SessionMemory;
                    if let Some(session_mem) = app.try_state::<Arc<SessionMemory>>() {
                        if let Err(e) = session_mem.update_current_page(&url, Some(&title)) {
                            tracing::warn!("Failed to update session page: {}", e);
                        }
                        if let Err(e) = session_mem.store_content(body_text.clone()) {
                            tracing::error!("Failed to store content: {}", e);
                        }
                    }

                    let now = crate::utils::current_timestamp();
                    system_status::update_status(|status| {
                        status.last_known_url = Some(url.clone());
                        status.last_page_update = Some(now);
                    });

                    emit_status_throttled(&app);

                    // 2. Emit to frontend
                    let _ = app.emit(
                        "page_content",
                        serde_json::json!({
                            "url": url,
                            "body_text": body_text
                        }),
                    );
                }
                "browsing_context" => {
                    let history = message.recent_history.clone().unwrap_or_default();
                    let top_sites = message.top_sites.clone().unwrap_or_default();

                    tracing::info!(
                        "Received browsing context: {} history items, {} top sites",
                        history.len(),
                        top_sites.len()
                    );

                    // Update MCP Resource state
                    mcp_state.update_context(history.clone(), top_sites.clone()).await;

                    // Emit to frontend (legacy event)
                    let _ = app.emit(
                        "browsing_context",
                        serde_json::json!({
                            "recent_history": history,
                            "top_sites": top_sites
                        }),
                    );
                }
                _ => {
                    tracing::debug!("Unknown message type: {}", message.msg_type);
                }
            }

            // Send response
            let response = NativeResponse {
                action: "acknowledged".to_string(),
                success: true,
                data: serde_json::json!({}),
            };

            if let Ok(json) = serde_json::to_vec(&response) {
                let len = (json.len() as u32).to_le_bytes();
                if stream.write_all(&len).await.is_ok() {
                    let _ = stream.write_all(&json).await;
                    let _ = stream.flush().await;
                }
            }

            // Check for pending effects to send (from MCP tools or legacy EffectQueue)
            // First: Check legacy EffectQueue (backward compatibility)
            let effect_queue = app.state::<Arc<EffectQueue>>();
            let hidden_queue = effect_queue.clone();
            let pending = hidden_queue.pop_all();

            for effect in pending {
                tracing::info!("Sending queued effect to extension: {:?}", effect);
                match serde_json::to_vec(&effect) {
                    Ok(json) => {
                        let len = (json.len() as u32).to_le_bytes();
                        let _ = stream.write_all(&len).await;
                        let _ = stream.write_all(&json).await;
                        let _ = stream.flush().await;
                    }
                    Err(e) => tracing::error!("Failed to serialize effect: {}", e),
                }
            }

            // Second: Check MCP effect channel (non-blocking)
            if let Ok(mut receiver) = mcp_ctx.effect_receiver.try_lock() {
                while let Ok(effect) = receiver.try_recv() {
                    tracing::info!("Sending MCP tool effect to extension: {:?}", effect);
                    match serde_json::to_vec(&effect) {
                        Ok(json) => {
                            let len = (json.len() as u32).to_le_bytes();
                            let _ = stream.write_all(&len).await;
                            let _ = stream.write_all(&json).await;
                            let _ = stream.flush().await;
                        }
                        Err(e) => tracing::error!("Failed to serialize MCP effect: {}", e),
                    }
                }
            }
        }
    }

    // Mark browser as disconnected in MCP state
    mcp_ctx
        .mcp_server
        .state()
        .is_connected
        .store(false, Ordering::SeqCst);

    system_status::update_status(|status| {
        status.extension_connected = false;
        status.extension_operational = false;
        status.mcp_browser_connected = false;
    });

    emit_status_throttled(&app);

    record_timeline_event(
        "Extension disconnected",
        None,
        TimelineEntryType::System,
        TimelineStatus::Info,
    );

    // Emit disconnection event to frontend
    let _ = app.emit(
        "extension_disconnected",
        serde_json::json!({ "connected": false }),
    );
    tracing::info!("Native bridge disconnected");
}

/// Start the TCP server for native messaging bridge (runs in background tokio task)
/// Now creates and manages the MCP Browser Server for standardized agent access
pub fn start_native_messaging_server(app: AppHandle) {
    tauri::async_runtime::spawn(async move {
        tracing::info!(
            "Starting native messaging TCP server on port {}",
            BRIDGE_PORT
        );

        let listener = match TcpListener::bind(format!("127.0.0.1:{}", BRIDGE_PORT)).await {
            Ok(l) => l,
            Err(e) => {
                tracing::error!("Failed to bind TCP server: {}", e);
                return;
            }
        };

        tracing::info!(
            "Native messaging server listening on 127.0.0.1:{}",
            BRIDGE_PORT
        );

        // Create MCP Browser Server with effect channel
        let (effect_tx, effect_rx) = mpsc::channel::<serde_json::Value>(64);
        let browser_state = Arc::new(BrowserState::new());
        let mcp_server = Arc::new(BrowserMcpServer::new(browser_state.clone(), effect_tx));

        // Register MCP server as managed state for orchestrator access
        app.manage(mcp_server.clone());

        // Create shared MCP context for connection handlers
        let mcp_ctx = Arc::new(McpBridgeContext {
            mcp_server: mcp_server.clone(),
            effect_receiver: Arc::new(tokio::sync::Mutex::new(effect_rx)),
        });

        tracing::info!("MCP Browser Server initialized");

        loop {
            match listener.accept().await {
                Ok((stream, _addr)) => {
                    // Check connection limit before accepting
                    let current = ACTIVE_CONNECTIONS.load(Ordering::SeqCst);
                    if current >= MAX_CONNECTIONS {
                        tracing::warn!(
                            "Connection limit reached ({}/{}), rejecting new connection",
                            current,
                            MAX_CONNECTIONS
                        );
                        // Stream dropped immediately
                        continue;
                    }

                    // Increment connection count
                    ACTIVE_CONNECTIONS.fetch_add(1, Ordering::SeqCst);

                    let app_clone = app.clone();
                    let mcp_ctx_clone = mcp_ctx.clone();
                    
                    // Spawn per-connection task
                    tauri::async_runtime::spawn(async move {
                        handle_client(stream, app_clone, mcp_ctx_clone).await;
                    });
                }
                Err(e) => {
                    tracing::error!("Connection error: {}", e);
                }
            }
        }
    });
}

// ============================================================================
// MCP Integration Helpers
// ============================================================================

/// Invoke an MCP tool by name with arguments (convenience function for orchestrator)
pub async fn invoke_mcp_tool(
    mcp_server: &BrowserMcpServer,
    tool_name: &str,
    arguments: serde_json::Value,
) -> Result<serde_json::Value, String> {
    let request = ToolRequest {
        tool_name: tool_name.to_string(),
        arguments,
        request_id: Uuid::new_v4().to_string(),
    };

    let response = mcp_server.invoke_tool(request).await;

    if response.success {
        Ok(response.data)
    } else {
        Err(response.error.unwrap_or_else(|| "Unknown error".to_string()))
    }
}

/// Read current page content from MCP resource (convenience function)
pub async fn read_current_page(mcp_server: &BrowserMcpServer) -> Option<serde_json::Value> {
    use crate::mcp::ResourceRequest;

    let request = ResourceRequest {
        uri: "browser://current-page".to_string(),
        request_id: Uuid::new_v4().to_string(),
        query: None,
    };

    let response = mcp_server.read_resource(request).await;
    if response.success {
        Some(response.content)
    } else {
        None
    }
}

/// Read browsing history from MCP resource (convenience function)
pub async fn read_browsing_history(
    mcp_server: &BrowserMcpServer,
    limit: Option<usize>,
) -> Option<serde_json::Value> {
    use crate::mcp::ResourceRequest;
    use std::collections::HashMap;

    let query = limit.map(|l| {
        let mut q = HashMap::new();
        q.insert("limit".to_string(), l.to_string());
        q
    });

    let request = ResourceRequest {
        uri: "browser://history".to_string(),
        request_id: Uuid::new_v4().to_string(),
        query,
    };

    let response = mcp_server.read_resource(request).await;
    if response.success {
        Some(response.content)
    } else {
        None
    }
}
