//! WebSocket Server for Real-Time Events
//!
//! Provides real-time event streaming via WebSocket, enabling:
//! - Live agent status updates
//! - Action approval notifications
//! - Workflow recording progress
//! - System events and logs

use std::sync::Arc;
use axum::{
    extract::ws::{Message, WebSocket, WebSocketUpgrade},
    response::IntoResponse,
};
use tokio::sync::{RwLock, broadcast};
use serde::{Deserialize, Serialize};
use tracing::{info, error, debug};

use crate::server::state::ServerState;

/// WebSocket event types
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "type", content = "data")]
pub enum WsEvent {
    /// Server status update
    StatusUpdate(StatusData),
    /// Agent started executing
    AgentStarted(AgentData),
    /// Agent completed
    AgentCompleted(AgentData),
    /// Action requires approval
    ActionPending(ActionData),
    /// Action was approved
    ActionApproved(ActionData),
    /// Action was denied
    ActionDenied(ActionData),
    /// Workflow recording update
    RecordingUpdate(RecordingData),
    /// Workflow execution progress
    WorkflowProgress(WorkflowData),
    /// System log message
    LogMessage(LogData),
    /// Error occurred
    Error(ErrorData),
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct StatusData {
    pub connected: bool,
    pub active_agents: usize,
    pub pending_actions: usize,
    pub memory_entries: usize,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AgentData {
    pub id: String,
    pub name: String,
    pub agent_type: String,
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ActionData {
    pub id: String,
    pub action_type: String,
    pub description: String,
    pub risk_level: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RecordingData {
    pub recording_id: String,
    pub steps_recorded: usize,
    pub status: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct WorkflowData {
    pub workflow_id: String,
    pub current_step: usize,
    pub total_steps: usize,
    pub status: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LogData {
    pub level: String,
    pub message: String,
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ErrorData {
    pub code: String,
    pub message: String,
}

/// WebSocket event broadcaster
pub struct EventBroadcaster {
    tx: broadcast::Sender<WsEvent>,
}

impl EventBroadcaster {
    pub fn new() -> Self {
        // Create broadcast channel with buffer of 100 events
        let (tx, _) = broadcast::channel(100);
        Self { tx }
    }

    pub fn subscribe(&self) -> broadcast::Receiver<WsEvent> {
        self.tx.subscribe()
    }

    pub fn broadcast(&self, event: WsEvent) -> Result<(), broadcast::error::SendError<WsEvent>> {
        self.tx.send(event).map(|_| ())
    }
}

impl Default for EventBroadcaster {
    fn default() -> Self {
        Self::new()
    }
}

/// Handle WebSocket upgrade
pub async fn ws_handler(
    ws: WebSocketUpgrade,
) -> impl IntoResponse {
    info!("WebSocket connection request received");
    ws.on_upgrade(handle_socket)
}

/// Handle WebSocket connection
async fn handle_socket(mut socket: WebSocket) {
    info!("WebSocket connection established");
    
    // Send initial greeting
    let greeting = WsEvent::StatusUpdate(StatusData {
        connected: true,
        active_agents: 0,
        pending_actions: 0,
        memory_entries: 0,
    });
    
    if let Err(e) = send_event(&mut socket, greeting).await {
        error!("Failed to send greeting: {}", e);
        return;
    }
    
    // Main WebSocket loop
    loop {
        match socket.recv().await {
            Some(Ok(msg)) => {
                match msg {
                    Message::Text(text) => {
                        debug!("Received WebSocket message: {}", text);
                        
                        // Parse client message
                        match serde_json::from_str::<ClientMessage>(&text) {
                            Ok(client_msg) => {
                                // Handle different message types
                                match client_msg {
                                    ClientMessage::Ping => {
                                        let _ = send_event(&mut socket, WsEvent::StatusUpdate(StatusData {
                                            connected: true,
                                            active_agents: 0,
                                            pending_actions: 0,
                                            memory_entries: 0,
                                        })).await;
                                    }
                                    ClientMessage::Subscribe { channel } => {
                                        info!("Client subscribed to channel: {}", channel);
                                        // TODO: Implement channel-based subscriptions
                                    }
                                    ClientMessage::Execute { task } => {
                                        info!("Client requested execution: {}", task);
                                        // TODO: Trigger task execution
                                    }
                                    ClientMessage::Approve { action_id } => {
                                        info!("Client approved action: {}", action_id);
                                        // TODO: Trigger action approval
                                    }
                                    ClientMessage::Deny { action_id } => {
                                        info!("Client denied action: {}", action_id);
                                        // TODO: Trigger action denial
                                    }
                                }
                            }
                            Err(e) => {
                                error!("Failed to parse client message: {}", e);
                                let error = WsEvent::Error(ErrorData {
                                    code: "parse_error".to_string(),
                                    message: format!("Invalid message format: {}", e),
                                });
                                let _ = send_event(&mut socket, error).await;
                            }
                        }
                    }
                    Message::Close(_) => {
                        info!("WebSocket connection closed by client");
                        break;
                    }
                    Message::Ping(data) => {
                        if let Err(e) = socket.send(Message::Pong(data)).await {
                            error!("Failed to send pong: {}", e);
                            break;
                        }
                    }
                    _ => {}
                }
            }
            Some(Err(e)) => {
                error!("WebSocket error: {}", e);
                break;
            }
            None => {
                info!("WebSocket connection closed");
                break;
            }
        }
    }
    
    info!("WebSocket handler ending");
}

/// Send an event to the WebSocket
async fn send_event(socket: &mut WebSocket, event: WsEvent) -> Result<(), axum::Error> {
    let json = serde_json::to_string(&event).map_err(|e| {
        axum::Error::new(std::io::Error::new(std::io::ErrorKind::Other, e))
    })?;
    
    socket.send(Message::Text(json)).await
}

/// Client message types
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "type")]
#[serde(rename_all = "snake_case")]
pub enum ClientMessage {
    Ping,
    Subscribe { channel: String },
    Execute { task: String },
    Approve { action_id: String },
    Deny { action_id: String },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_event_broadcast() {
        let broadcaster = EventBroadcaster::new();
        
        let event = WsEvent::StatusUpdate(StatusData {
            connected: true,
            active_agents: 5,
            pending_actions: 2,
            memory_entries: 100,
        });
        
        // Should succeed with no subscribers
        assert!(broadcaster.broadcast(event).is_ok());
    }

    #[test]
    fn test_client_message_serialization() {
        let msg = ClientMessage::Execute {
            task: "Book a flight".to_string(),
        };
        
        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains("execute"));
        assert!(json.contains("Book a flight"));
    }
}
