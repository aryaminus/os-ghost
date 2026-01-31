//! Action confirmation system
//! Manages pending actions that require user confirmation before execution

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::RwLock;
use crate::timeline::{record_timeline_event, TimelineEntryType, TimelineStatus};

/// Unique action ID counter
static ACTION_ID_COUNTER: AtomicU64 = AtomicU64::new(1);

/// Risk level for actions
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ActionRiskLevel {
    /// Low risk: visual effects, highlights
    Low,
    /// Medium risk: navigation within known domains
    Medium,
    /// High risk: navigation to external sites, form submissions
    High,
}

impl ActionRiskLevel {
    /// Determine if this is considered high-risk for confirmation purposes
    pub fn is_high_risk(&self) -> bool {
        matches!(self, ActionRiskLevel::High)
    }
}

/// Status of a pending action
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ActionStatus {
    /// Waiting for user confirmation
    Pending,
    /// User approved the action
    Approved,
    /// User denied the action
    Denied,
    /// Action expired (timed out)
    Expired,
    /// Action was executed successfully
    Executed,
    /// Action failed during execution
    Failed,
}

/// A pending action awaiting user confirmation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PendingAction {
    /// Unique action ID
    pub id: u64,
    /// Type of action (e.g., "browser.navigate", "browser.inject_effect")
    pub action_type: String,
    /// Human-readable description of what the action will do
    pub description: String,
    /// The target (e.g., URL, element)
    pub target: String,
    /// Risk level
    pub risk_level: ActionRiskLevel,
    /// Current status
    pub status: ActionStatus,
    /// Timestamp when action was created (seconds since UNIX epoch)
    pub created_at: u64,
    /// Optional reason for the action (from AI)
    pub reason: Option<String>,
    /// The original arguments to pass to the tool
    #[serde(skip_serializing_if = "Option::is_none")]
    pub arguments: Option<serde_json::Value>,
}

impl PendingAction {
    /// Create a new pending action
    pub fn new(
        action_type: String,
        description: String,
        target: String,
        risk_level: ActionRiskLevel,
        reason: Option<String>,
        arguments: Option<serde_json::Value>,
    ) -> Self {
        let id = ACTION_ID_COUNTER.fetch_add(1, Ordering::SeqCst);
        let created_at = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        Self {
            id,
            action_type,
            description,
            target,
            risk_level,
            status: ActionStatus::Pending,
            created_at,
            reason,
            arguments,
        }
    }

    /// Check if this action has expired (default: 60 seconds)
    pub fn is_expired(&self) -> bool {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        now.saturating_sub(self.created_at) > 60
    }
}

/// Manages pending actions globally
#[derive(Debug, Default)]
pub struct ActionQueue {
    /// Map of action ID to pending action
    actions: RwLock<HashMap<u64, PendingAction>>,
    /// Action history (for audit log)
    history: RwLock<Vec<PendingAction>>,
}

impl ActionQueue {
    /// Create a new action queue
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a new pending action
    pub fn add(&self, action: PendingAction) -> u64 {
        let id = action.id;
        if let Ok(mut actions) = self.actions.write() {
            actions.insert(id, action);
        }
        id
    }

    /// Get a pending action by ID
    pub fn get(&self, id: u64) -> Option<PendingAction> {
        self.actions.read().ok()?.get(&id).cloned()
    }

    /// Get all pending actions
    pub fn get_pending(&self) -> Vec<PendingAction> {
        self.actions
            .read()
            .ok()
            .map(|actions| {
                actions
                    .values()
                    .filter(|a| a.status == ActionStatus::Pending && !a.is_expired())
                    .cloned()
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Approve an action
    pub fn approve(&self, id: u64) -> Option<PendingAction> {
        self.update_status_in_queue(id, ActionStatus::Approved)
    }

    /// Deny an action
    pub fn deny(&self, id: u64) -> Option<PendingAction> {
        self.remove_and_archive(id, ActionStatus::Denied)
    }

    /// Mark action as executed
    pub fn mark_executed(&self, id: u64) -> Option<PendingAction> {
        self.remove_and_archive(id, ActionStatus::Executed)
    }

    /// Mark action as failed
    pub fn mark_failed(&self, id: u64) -> Option<PendingAction> {
        self.remove_and_archive(id, ActionStatus::Failed)
    }

    /// Update action status in queue (keeps action available for execution)
    fn update_status_in_queue(&self, id: u64, status: ActionStatus) -> Option<PendingAction> {
        let mut actions = self.actions.write().ok()?;
        let action = actions.get_mut(&id)?;
        action.status = status;
        Some(action.clone())
    }

    /// Update action arguments in queue
    pub fn update_arguments(&self, id: u64, arguments: Option<serde_json::Value>) -> Option<PendingAction> {
        let mut actions = self.actions.write().ok()?;
        let action = actions.get_mut(&id)?;
        action.arguments = arguments;
        Some(action.clone())
    }

    /// Remove action from queue and move to history
    fn remove_and_archive(&self, id: u64, status: ActionStatus) -> Option<PendingAction> {
        let mut action = {
            let mut actions = self.actions.write().ok()?;
            actions.remove(&id)?
        };
        action.status = status;

        // Add to history
        if let Ok(mut history) = self.history.write() {
            history.push(action.clone());
            // Keep only last 100 actions in history
            if history.len() > 100 {
                history.remove(0);
            }
        }

        Some(action)
    }

    /// Get action history
    pub fn get_history(&self, limit: usize) -> Vec<PendingAction> {
        self.history
            .read()
            .ok()
            .map(|history| history.iter().rev().take(limit).cloned().collect())
            .unwrap_or_default()
    }

    /// Clean up expired actions
    pub fn cleanup_expired(&self) {
        if let Ok(mut actions) = self.actions.write() {
            let expired_ids: Vec<u64> = actions
                .iter()
                .filter(|(_, a)| a.is_expired())
                .map(|(id, _)| *id)
                .collect();

            for id in expired_ids {
                if let Some(mut action) = actions.remove(&id) {
                    action.status = ActionStatus::Expired;
                    if let Ok(mut history) = self.history.write() {
                        history.push(action);
                        if history.len() > 100 {
                            history.remove(0);
                        }
                    }
                }
            }
        }
    }
}

// Global action queue instance
lazy_static::lazy_static! {
    pub static ref ACTION_QUEUE: ActionQueue = ActionQueue::new();
}

// ============================================================================
// Tauri Commands
// ============================================================================

/// Get all pending actions awaiting confirmation
#[tauri::command]
pub fn get_pending_actions() -> Vec<PendingAction> {
    ACTION_QUEUE.cleanup_expired();
    ACTION_QUEUE.get_pending()
}

/// Approve a pending action
#[tauri::command]
pub fn approve_action(action_id: u64) -> Result<PendingAction, String> {
    let action = ACTION_QUEUE
        .approve(action_id)
        .ok_or_else(|| format!("Action {} not found or already processed", action_id))?;

    record_timeline_event(
        &format!("Action approved: {}", action.action_type),
        action.reason.clone(),
        TimelineEntryType::Action,
        TimelineStatus::Approved,
    );

    Ok(action)
}

/// Deny a pending action
#[tauri::command]
pub fn deny_action(action_id: u64) -> Result<PendingAction, String> {
    let action = ACTION_QUEUE
        .deny(action_id)
        .ok_or_else(|| format!("Action {} not found or already processed", action_id))?;

    record_timeline_event(
        &format!("Action denied: {}", action.action_type),
        action.reason.clone(),
        TimelineEntryType::Action,
        TimelineStatus::Denied,
    );

    Ok(action)
}

/// Get action history (audit log)
#[tauri::command]
pub fn get_action_history(limit: Option<usize>) -> Vec<PendingAction> {
    ACTION_QUEUE.get_history(limit.unwrap_or(50))
}

/// Clear all pending actions (deny all)
#[tauri::command]
pub fn clear_pending_actions() -> usize {
    let pending = ACTION_QUEUE.get_pending();
    let count = pending.len();
    for action in pending {
        ACTION_QUEUE.deny(action.id);
    }
    count
}

/// Clear action history
#[tauri::command]
pub fn clear_action_history() -> usize {
    let history = ACTION_QUEUE.get_history(1000);
    let count = history.len();
    if let Ok(mut history_guard) = ACTION_QUEUE.history.write() {
        history_guard.clear();
    }
    count
}

/// Execute an approved action by ID
/// Returns the action data on success, or an error message
#[tauri::command]
pub async fn execute_approved_action(
    action_id: u64,
    effect_queue: tauri::State<'_, std::sync::Arc<crate::game_state::EffectQueue>>,
) -> Result<serde_json::Value, String> {
    // Get the action and verify it's approved
    let action = ACTION_QUEUE
        .get(action_id)
        .ok_or_else(|| format!("Action {} not found", action_id))?;

    if action.status != ActionStatus::Approved {
        return Err(format!(
            "Action {} is not approved (status: {:?})",
            action_id, action.status
        ));
    }

    // Get the arguments
    let args = action
        .arguments
        .clone()
        .unwrap_or_else(|| serde_json::json!({}));

    // Execute based on action type
    let result: Result<serde_json::Value, String> = match action.action_type.as_str() {
        "browser.navigate" => {
            let url = args
                .get("url")
                .and_then(|v| v.as_str())
                .ok_or("Missing URL")?;
            effect_queue.push(crate::game_state::EffectMessage {
                action: "navigate".to_string(),
                effect: None,
                duration: None,
                text: None,
                url: Some(url.to_string()),
            });
            if let Some(rollback) = crate::rollback::get_rollback_manager() {
                rollback.record_navigation(&action_id.to_string(), url, None);
            }
            Ok(serde_json::json!({ "navigated_to": url }))
        }
        "browser.inject_effect" => {
            let effect = args
                .get("effect")
                .and_then(|v| v.as_str())
                .ok_or("Missing effect")?;
            let duration = args
                .get("duration")
                .and_then(|v| v.as_u64())
                .map(|d| d.clamp(100, 10_000));
            effect_queue.push(crate::game_state::EffectMessage {
                action: "inject_effect".to_string(),
                effect: Some(effect.to_string()),
                duration,
                text: None,
                url: None,
            });
            if let Some(rollback) = crate::rollback::get_rollback_manager() {
                rollback.record_effect(&action_id.to_string(), effect, duration.unwrap_or(1000));
            }
            Ok(serde_json::json!({ "effect_applied": effect }))
        }
        "browser.highlight_text" => {
            let text = args
                .get("text")
                .and_then(|v| v.as_str())
                .ok_or("Missing text")?;
            effect_queue.push(crate::game_state::EffectMessage {
                action: "highlight_text".to_string(),
                effect: None,
                duration: None,
                text: Some(text.to_string()),
                url: None,
            });
            if let Some(rollback) = crate::rollback::get_rollback_manager() {
                rollback.record_highlight(&action_id.to_string(), text);
            }
            Ok(serde_json::json!({ "highlighted": text }))
        }
        _ => Err(format!("Unknown action type: {}", action.action_type)),
    };

    let result = match result {
        Ok(value) => value,
        Err(err) => {
            ACTION_QUEUE.mark_failed(action_id);
            record_timeline_event(
                &format!("Action failed: {}", action.action_type),
                Some(err.clone()),
                TimelineEntryType::Action,
                TimelineStatus::Failed,
            );
            return Err(err);
        }
    };

    // Mark as executed
    ACTION_QUEUE.mark_executed(action_id);
    record_timeline_event(
        &format!("Action executed: {}", action.action_type),
        action.reason.clone(),
        TimelineEntryType::Action,
        TimelineStatus::Executed,
    );

    Ok(result)
}

// ============================================================================
// Action Preview Tauri Commands
// ============================================================================

/// Get the currently active action preview
#[tauri::command]
pub fn get_active_preview() -> Option<crate::action_preview::ActionPreview> {
    crate::action_preview::get_preview_manager()
        .and_then(|m| m.get_active_preview())
}

/// Approve a preview and execute it
#[tauri::command]
pub async fn approve_preview(
    preview_id: String,
    effect_queue: tauri::State<'_, std::sync::Arc<crate::game_state::EffectQueue>>,
) -> Result<(), String> {
    let action_id = approve_preview_internal(&preview_id)?;

    execute_approved_action(action_id, effect_queue).await?;
    Ok(())
}

fn approve_preview_internal(preview_id: &str) -> Result<u64, String> {
    let manager = crate::action_preview::get_preview_manager_mut()
        .ok_or("Preview manager not initialized")?;
    let preview = manager
        .get_active_preview()
        .ok_or("No active preview".to_string())?;

    if preview.id != preview_id {
        return Err("Preview ID mismatch".to_string());
    }

    let updated_args = preview.updated_arguments();
    let action_id = preview.action.id;

    if updated_args.is_some() {
        ACTION_QUEUE.update_arguments(action_id, updated_args);
    }

    manager.approve_preview(preview_id)?;

    ACTION_QUEUE
        .approve(action_id)
        .ok_or_else(|| format!("Action {} not found or already processed", action_id))?;

    Ok(action_id)
}

/// Deny a preview
#[tauri::command]
pub fn deny_preview(preview_id: String, reason: Option<String>) -> Result<(), String> {
    let action_id = {
        let manager = crate::action_preview::get_preview_manager_mut()
            .ok_or("Preview manager not initialized")?;
        let preview = manager
            .get_active_preview()
            .ok_or("No active preview".to_string())?;

        if preview.id != preview_id {
            return Err("Preview ID mismatch".to_string());
        }

        let action_id = preview.action.id;
        manager.deny_preview(&preview_id, reason)?;
        action_id
    };

    ACTION_QUEUE.deny(action_id);
    Ok(())
}

/// Update a preview parameter
#[tauri::command]
pub fn update_preview_param(
    preview_id: String,
    param_name: String,
    value: serde_json::Value,
) -> Result<crate::action_preview::ActionPreview, String> {
    let manager = crate::action_preview::get_preview_manager_mut()
        .ok_or("Preview manager not initialized")?;
    manager.update_param(&preview_id, &param_name, value)?;
    manager.get_active_preview().ok_or("No active preview".to_string())
}

// ============================================================================
// Rollback/Undo Tauri Commands
// ============================================================================

/// Get the current rollback status
#[tauri::command]
pub fn get_rollback_status() -> crate::rollback::RollbackStatus {
    crate::rollback::get_rollback_manager()
        .map(|m| m.get_status())
        .unwrap_or_else(|| crate::rollback::RollbackStatus {
            can_undo: false,
            can_redo: false,
            undo_description: None,
            redo_description: None,
            stack_size: 0,
            recent_actions: vec![],
        })
}

/// Undo the last action
#[tauri::command]
pub fn undo_action() -> crate::rollback::UndoResult {
    crate::rollback::get_rollback_manager()
        .map(|m| m.undo())
        .unwrap_or_else(|| crate::rollback::UndoResult {
            success: false,
            action: None,
            error: Some("Rollback manager not initialized".to_string()),
            restored_state: None,
        })
}

/// Redo the last undone action
#[tauri::command]
pub fn redo_action() -> crate::rollback::UndoResult {
    crate::rollback::get_rollback_manager()
        .map(|m| m.redo())
        .unwrap_or_else(|| crate::rollback::UndoResult {
            success: false,
            action: None,
            error: Some("Rollback manager not initialized".to_string()),
            restored_state: None,
        })
}
