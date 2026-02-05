//! HTTP API Routes
//!
//! REST API endpoints for the OS-Ghost server:
//! - /api/v1/status - Server status and health
//! - /api/v1/execute - Execute a task
//! - /api/v1/workflows - List workflows
//! - /api/v1/workflows/:id/execute - Execute a workflow
//! - /api/v1/record/start - Start workflow recording
//! - /api/v1/record/stop - Stop recording
//! - /api/v1/agents - List active agents
//! - /api/v1/memory - Get memory statistics

use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use serde::Serialize;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{error, info};

use crate::server::state::{ExecuteRequest, RecordingRequest, RecordingResponse, ServerState};

/// Standard API response wrapper
#[derive(Serialize)]
struct ApiResponse<T> {
    success: bool,
    data: Option<T>,
    error: Option<String>,
}

impl<T: Serialize> ApiResponse<T> {
    fn success(data: T) -> Self {
        Self {
            success: true,
            data: Some(data),
            error: None,
        }
    }

    fn error(message: &str) -> Self {
        Self {
            success: false,
            data: None,
            error: Some(message.to_string()),
        }
    }
}

// ============================================================================
// Status and Health
// ============================================================================

/// Get server status
pub async fn get_status(State(state): State<Arc<RwLock<ServerState>>>) -> impl IntoResponse {
    let state = state.read().await;

    let status = serde_json::json!({
        "connected": state.connected,
        "uptime_secs": state.uptime_secs(),
        "active_agents_count": state.active_agents.len(),
        "pending_actions_count": state.pending_actions.len(),
        "workflows_count": state.workflows.len(),
        "memory_entries": state.memory_entries,
        "api_version": state.api_config.version,
        "timestamp": chrono::Utc::now().to_rfc3339(),
    });

    Json(ApiResponse::success(status))
}

// ============================================================================
// Task Execution
// ============================================================================

/// Execute a task
pub async fn execute_task(
    State(state): State<Arc<RwLock<ServerState>>>,
    Json(request): Json<ExecuteRequest>,
) -> impl IntoResponse {
    info!("Received execute request: {}", request.task);

    let mut state = state.write().await;

    // Generate task ID
    let task_id = format!("task-{}", uuid::Uuid::new_v4());

    // Integrate with AgentOrchestrator if available
    if let Some(orchestrator) = &state.orchestrator {
        info!("Delegating task to orchestrator: {}", request.task);
        if let Err(e) = orchestrator
            .handle_assistance_request(request.task.clone())
            .await
        {
            error!("Orchestrator failed to start task: {}", e);
            return Json(ApiResponse::<serde_json::Value>::error(&format!(
                "Failed to start task: {}",
                e
            )))
            .into_response();
        }
    } else {
        info!("No orchestrator available, task queued only");
    }

    // Add an agent for this task
    state.add_agent(
        task_id.clone(),
        format!(
            "Task: {}",
            request.task.chars().take(30).collect::<String>()
        ),
        "executor".to_string(),
    );

    Json(ApiResponse::success(serde_json::json!({
        "task_id": task_id,
        "status": "pending",
        "message": "Task queued/delegated for execution",
    })))
    .into_response()
}

// ============================================================================
// Workflows
// ============================================================================

/// Get all workflows
pub async fn get_workflows(State(state): State<Arc<RwLock<ServerState>>>) -> impl IntoResponse {
    let state = state.read().await;

    let workflows: Vec<_> = state
        .workflows
        .iter()
        .map(|w| {
            serde_json::json!({
                "id": w.id,
                "name": w.name,
                "description": w.description,
                "step_count": w.step_count,
                "execution_count": w.execution_count,
                "success_rate": w.success_rate,
                "enabled": w.enabled,
            })
        })
        .collect();

    Json(ApiResponse::success(workflows))
}

/// Execute a workflow by ID
pub async fn execute_workflow(
    State(state): State<Arc<RwLock<ServerState>>>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    let state = state.read().await;

    match state.get_workflow(&id) {
        Some(workflow) => {
            info!("Executing workflow: {} ({})", workflow.name, id);

            let response = serde_json::json!({
                "workflow_id": id,
                "name": workflow.name,
                "status": "executing",
                "execution_id": format!("exec-{}", uuid::Uuid::new_v4()),
            });

            (StatusCode::OK, Json(ApiResponse::success(response))).into_response()
        }
        None => {
            error!("Workflow not found: {}", id);
            (
                StatusCode::NOT_FOUND,
                Json(ApiResponse::<()>::error("Workflow not found")),
            )
                .into_response()
        }
    }
}

// ============================================================================
// Recording
// ============================================================================

/// Start workflow recording
pub async fn start_recording(
    State(state): State<Arc<RwLock<ServerState>>>,
    Json(request): Json<RecordingRequest>,
) -> impl IntoResponse {
    info!("Starting workflow recording: {}", request.name);

    let mut state = state.write().await;

    let recording_id = format!("rec-{}", uuid::Uuid::new_v4());

    let response = RecordingResponse {
        recording_id: recording_id.clone(),
        status: "recording".to_string(),
        steps_recorded: 0,
        started_at: chrono::Utc::now(),
    };

    // Add recording as an agent
    state.add_agent(
        recording_id.clone(),
        format!("Recording: {}", request.name),
        "recorder".to_string(),
    );

    Json(ApiResponse::success(serde_json::json!({
        "recording_id": response.recording_id,
        "status": response.status,
        "started_at": response.started_at,
        "message": "Recording started. Perform actions in your browser.",
    })))
}

/// Stop workflow recording
pub async fn stop_recording(State(state): State<Arc<RwLock<ServerState>>>) -> impl IntoResponse {
    info!("Stopping workflow recording");

    let _state = state.read().await;

    Json(ApiResponse::success(serde_json::json!({
        "status": "stopped",
        "actions_recorded": 0,
        "message": "Recording stopped. Workflow saved.",
    })))
}

// ============================================================================
// Agents
// ============================================================================

/// Get all active agents
pub async fn get_agents(State(state): State<Arc<RwLock<ServerState>>>) -> impl IntoResponse {
    let state = state.read().await;

    let agents: Vec<_> = state
        .active_agents
        .values()
        .map(|agent| {
            serde_json::json!({
                "id": agent.id,
                "name": agent.name,
                "type": agent.agent_type,
                "status": agent.status,
                "last_activity": agent.last_activity,
            })
        })
        .collect();

    Json(ApiResponse::success(agents))
}

// ============================================================================
// Memory
// ============================================================================

/// Get memory statistics
pub async fn get_memory(State(state): State<Arc<RwLock<ServerState>>>) -> impl IntoResponse {
    let state = state.read().await;

    let stats = serde_json::json!({
        "total_entries": state.memory_entries,
        "last_updated": chrono::Utc::now().to_rfc3339(),
    });

    Json(ApiResponse::success(stats))
}

// ============================================================================
// Pending Actions
// ============================================================================

/// Get all pending actions
pub async fn get_pending_actions(
    State(state): State<Arc<RwLock<ServerState>>>,
) -> impl IntoResponse {
    let state = state.read().await;

    let actions: Vec<_> = state
        .pending_actions
        .iter()
        .map(|action| {
            serde_json::json!({
                "id": action.id,
                "type": action.action_type,
                "description": action.description,
                "risk_level": action.risk_level,
                "requires_approval": action.requires_approval,
                "requested_at": action.requested_at,
            })
        })
        .collect();

    Json(ApiResponse::success(actions))
}

/// Approve a pending action
pub async fn approve_action(
    State(state): State<Arc<RwLock<ServerState>>>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    let mut state = state.write().await;

    match state.remove_pending_action(&id) {
        Some(action) => {
            info!("Approved action: {}", id);
            (
                StatusCode::OK,
                Json(ApiResponse::success(serde_json::json!({
                    "action_id": id,
                    "status": "approved",
                    "action_type": action.action_type,
                }))),
            )
                .into_response()
        }
        None => (
            StatusCode::NOT_FOUND,
            Json(ApiResponse::<()>::error("Action not found")),
        )
            .into_response(),
    }
}

/// Deny a pending action
pub async fn deny_action(
    State(state): State<Arc<RwLock<ServerState>>>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    let mut state = state.write().await;

    match state.remove_pending_action(&id) {
        Some(action) => {
            info!("Denied action: {}", id);
            (
                StatusCode::OK,
                Json(ApiResponse::success(serde_json::json!({
                    "action_id": id,
                    "status": "denied",
                    "action_type": action.action_type,
                }))),
            )
                .into_response()
        }
        None => (
            StatusCode::NOT_FOUND,
            Json(ApiResponse::<()>::error("Action not found")),
        )
            .into_response(),
    }
}
