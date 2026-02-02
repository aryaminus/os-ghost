//! Undo/Rollback Stack
//!
//! Implements reversible action tracking based on research insights:
//! - **OpenAI Operator**: Users can intervene and undo at any point
//! - **Anthropic Guidelines**: Sandbox with rollback capabilities
//! - **CUA.ai**: Action history with replay/undo
//!
//! Tracks browser actions and allows reverting to previous states.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

// ============================================================================
// Rollback Types
// ============================================================================

/// Maximum number of undoable actions to track
const MAX_UNDO_STACK_SIZE: usize = 50;

/// A reversible action that can be undone
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UndoableAction {
    /// Unique action ID (matches PendingAction ID)
    pub id: String,
    /// Action type
    pub action_type: String,
    /// Human-readable description
    pub description: String,
    /// State before the action (for restoration)
    pub before_state: ActionState,
    /// State after the action
    pub after_state: ActionState,
    /// When the action was executed
    pub executed_at: DateTime<Utc>,
    /// Whether this action can still be undone
    pub can_undo: bool,
    /// Why undo might not be possible
    pub undo_blocked_reason: Option<String>,
}

/// State snapshot for a specific action type
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "data")]
pub enum ActionState {
    /// Navigation state
    Navigation { url: String, title: Option<String> },
    /// Visual effect state
    Effect {
        effect_name: Option<String>,
        duration_remaining_ms: u64,
    },
    /// Text highlight state
    Highlight { highlighted_texts: Vec<String> },
    /// File write state
    FileWrite {
        path: String,
        previous_content: Option<String>,
        backup_path: Option<String>,
        created: bool,
    },
    /// Note change state
    NoteChange {
        note_id: String,
        before: Option<NoteSnapshot>,
        after: Option<NoteSnapshot>,
    },
    /// Generic state (for extensibility)
    Generic { data: serde_json::Value },
    /// Empty/initial state
    Empty,
}

impl ActionState {
    /// Create navigation state
    pub fn navigation(url: &str, title: Option<&str>) -> Self {
        Self::Navigation {
            url: url.to_string(),
            title: title.map(|t| t.to_string()),
        }
    }

    /// Create effect state
    pub fn effect(name: Option<&str>, remaining_ms: u64) -> Self {
        Self::Effect {
            effect_name: name.map(|n| n.to_string()),
            duration_remaining_ms: remaining_ms,
        }
    }

    /// Create highlight state
    pub fn highlight(texts: Vec<String>) -> Self {
        Self::Highlight {
            highlighted_texts: texts,
        }
    }

    /// Create file write state
    pub fn file_write(
        path: &str,
        previous_content: Option<String>,
        backup_path: Option<String>,
        created: bool,
    ) -> Self {
        Self::FileWrite {
            path: path.to_string(),
            previous_content,
            backup_path,
            created,
        }
    }

    /// Create note change state
    pub fn note_change(
        note_id: &str,
        before: Option<NoteSnapshot>,
        after: Option<NoteSnapshot>,
    ) -> Self {
        Self::NoteChange {
            note_id: note_id.to_string(),
            before,
            after,
        }
    }
}

/// Result of an undo operation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UndoResult {
    /// Whether undo succeeded
    pub success: bool,
    /// The action that was undone
    pub action: Option<UndoableAction>,
    /// Error message if failed
    pub error: Option<String>,
    /// The state that was restored
    pub restored_state: Option<ActionState>,
}

// ============================================================================
// Undo Stack
// ============================================================================

/// Type alias for undo executor callback
type UndoExecutor = Box<dyn Fn(&UndoableAction) -> Result<(), String> + Send + Sync>;

/// Stack of undoable actions
pub struct UndoStack {
    /// Stack of undoable actions (newest first)
    actions: Arc<Mutex<VecDeque<UndoableAction>>>,
    /// Redo stack (for redo after undo)
    redo_stack: Arc<Mutex<VecDeque<UndoableAction>>>,
    /// Callback for executing undo commands
    undo_executor: Arc<Mutex<Option<UndoExecutor>>>,
}

impl Default for UndoStack {
    fn default() -> Self {
        Self::new()
    }
}

impl UndoStack {
    /// Create a new undo stack
    pub fn new() -> Self {
        Self {
            actions: Arc::new(Mutex::new(VecDeque::new())),
            redo_stack: Arc::new(Mutex::new(VecDeque::new())),
            undo_executor: Arc::new(Mutex::new(None)),
        }
    }

    /// Set the undo executor callback
    pub fn set_undo_executor<F>(&self, executor: F)
    where
        F: Fn(&UndoableAction) -> Result<(), String> + Send + Sync + 'static,
    {
        let mut guard = self.undo_executor.lock().unwrap();
        *guard = Some(Box::new(executor));
    }

    /// Push an action onto the undo stack
    pub fn push(&self, action: UndoableAction) {
        let mut stack = self.actions.lock().unwrap();
        stack.push_front(action);

        // Limit stack size
        while stack.len() > MAX_UNDO_STACK_SIZE {
            stack.pop_back();
        }

        // Clear redo stack when new action is pushed
        let mut redo = self.redo_stack.lock().unwrap();
        redo.clear();
    }

    /// Record a navigation action
    pub fn record_navigation(
        &self,
        id: &str,
        from_url: &str,
        from_title: Option<&str>,
        to_url: &str,
        to_title: Option<&str>,
    ) {
        let action = UndoableAction {
            id: id.to_string(),
            action_type: "browser.navigate".to_string(),
            description: format!("Navigated to {}", to_url),
            before_state: ActionState::navigation(from_url, from_title),
            after_state: ActionState::navigation(to_url, to_title),
            executed_at: Utc::now(),
            can_undo: true,
            undo_blocked_reason: None,
        };
        self.push(action);
    }

    /// Record an effect action
    pub fn record_effect(&self, id: &str, effect_name: &str, duration_ms: u64) {
        let action = UndoableAction {
            id: id.to_string(),
            action_type: "browser.inject_effect".to_string(),
            description: format!("Applied {} effect", effect_name),
            before_state: ActionState::effect(None, 0),
            after_state: ActionState::effect(Some(effect_name), duration_ms),
            executed_at: Utc::now(),
            can_undo: true,
            undo_blocked_reason: None,
        };
        self.push(action);
    }

    /// Record a highlight action
    pub fn record_highlight(
        &self,
        id: &str,
        highlighted_text: &str,
        existing_highlights: Vec<String>,
    ) {
        let mut new_highlights = existing_highlights.clone();
        new_highlights.push(highlighted_text.to_string());

        let action = UndoableAction {
            id: id.to_string(),
            action_type: "browser.highlight_text".to_string(),
            description: format!("Highlighted \"{}\"", highlighted_text),
            before_state: ActionState::highlight(existing_highlights),
            after_state: ActionState::highlight(new_highlights),
            executed_at: Utc::now(),
            can_undo: true,
            undo_blocked_reason: None,
        };
        self.push(action);
    }

    /// Undo the most recent action
    pub fn undo(&self) -> UndoResult {
        let action = {
            let mut stack = self.actions.lock().unwrap();
            stack.pop_front()
        };

        match action {
            Some(mut action) => {
                if !action.can_undo {
                    return UndoResult {
                        success: false,
                        action: Some(action.clone()),
                        error: action.undo_blocked_reason.clone(),
                        restored_state: None,
                    };
                }

                // Execute the undo if we have an executor
                if let Some(executor) = &*self.undo_executor.lock().unwrap() {
                    if let Err(e) = executor(&action) {
                        // Put action back on stack if undo failed
                        let mut stack = self.actions.lock().unwrap();
                        stack.push_front(action.clone());

                        return UndoResult {
                            success: false,
                            action: Some(action),
                            error: Some(e),
                            restored_state: None,
                        };
                    }
                }

                // Move to redo stack
                action.can_undo = false;
                action.undo_blocked_reason = Some("Already undone".to_string());

                let restored_state = action.before_state.clone();

                let mut redo = self.redo_stack.lock().unwrap();
                redo.push_front(action.clone());

                UndoResult {
                    success: true,
                    action: Some(action),
                    error: None,
                    restored_state: Some(restored_state),
                }
            }
            None => UndoResult {
                success: false,
                action: None,
                error: Some("Nothing to undo".to_string()),
                restored_state: None,
            },
        }
    }

    /// Redo the most recently undone action
    pub fn redo(&self) -> UndoResult {
        let action = {
            let mut redo = self.redo_stack.lock().unwrap();
            redo.pop_front()
        };

        match action {
            Some(mut action) => {
                // Re-execute the action would go here
                // For now, we just move it back to the undo stack
                action.can_undo = true;
                action.undo_blocked_reason = None;

                let restored_state = action.after_state.clone();

                let mut stack = self.actions.lock().unwrap();
                stack.push_front(action.clone());

                UndoResult {
                    success: true,
                    action: Some(action),
                    error: None,
                    restored_state: Some(restored_state),
                }
            }
            None => UndoResult {
                success: false,
                action: None,
                error: Some("Nothing to redo".to_string()),
                restored_state: None,
            },
        }
    }

    /// Check if undo is available
    pub fn can_undo(&self) -> bool {
        let stack = self.actions.lock().unwrap();
        stack.front().is_some_and(|a| a.can_undo)
    }

    /// Check if redo is available
    pub fn can_redo(&self) -> bool {
        let redo = self.redo_stack.lock().unwrap();
        !redo.is_empty()
    }

    /// Get the description of what would be undone
    pub fn undo_description(&self) -> Option<String> {
        let stack = self.actions.lock().unwrap();
        stack.front().map(|a| a.description.clone())
    }

    /// Get the description of what would be redone
    pub fn redo_description(&self) -> Option<String> {
        let redo = self.redo_stack.lock().unwrap();
        redo.front().map(|a| a.description.clone())
    }

    /// Get recent undoable actions
    pub fn get_recent(&self, limit: usize) -> Vec<UndoableAction> {
        let stack = self.actions.lock().unwrap();
        stack.iter().take(limit).cloned().collect()
    }

    /// Clear all undo/redo history
    pub fn clear(&self) {
        let mut stack = self.actions.lock().unwrap();
        let mut redo = self.redo_stack.lock().unwrap();
        stack.clear();
        redo.clear();
    }

    /// Mark an action as no longer undoable
    pub fn block_undo(&self, action_id: &str, reason: &str) {
        let mut stack = self.actions.lock().unwrap();
        for action in stack.iter_mut() {
            if action.id == action_id {
                action.can_undo = false;
                action.undo_blocked_reason = Some(reason.to_string());
                break;
            }
        }
    }

    /// Get undo stack size
    pub fn size(&self) -> usize {
        self.actions.lock().unwrap().len()
    }
}

// ============================================================================
// Rollback Manager
// ============================================================================

/// High-level rollback manager that coordinates undo operations
pub struct RollbackManager {
    /// The undo stack
    pub undo_stack: UndoStack,
    /// Current browser URL (for navigation tracking)
    current_url: Arc<Mutex<String>>,
    /// Current browser title
    current_title: Arc<Mutex<Option<String>>>,
    /// Current highlights
    current_highlights: Arc<Mutex<Vec<String>>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NoteSnapshot {
    pub id: String,
    pub title: String,
    pub body: String,
    pub pinned: bool,
    pub created_at: u64,
    pub updated_at: u64,
}

impl From<&crate::integrations::Note> for NoteSnapshot {
    fn from(note: &crate::integrations::Note) -> Self {
        Self {
            id: note.id.clone(),
            title: note.title.clone(),
            body: note.body.clone(),
            pinned: note.pinned,
            created_at: note.created_at,
            updated_at: note.updated_at,
        }
    }
}

impl From<NoteSnapshot> for crate::integrations::Note {
    fn from(snapshot: NoteSnapshot) -> Self {
        Self {
            id: snapshot.id,
            title: snapshot.title,
            body: snapshot.body,
            pinned: snapshot.pinned,
            created_at: snapshot.created_at,
            updated_at: snapshot.updated_at,
        }
    }
}

impl Default for RollbackManager {
    fn default() -> Self {
        Self::new()
    }
}

impl RollbackManager {
    /// Create a new rollback manager
    pub fn new() -> Self {
        Self {
            undo_stack: UndoStack::new(),
            current_url: Arc::new(Mutex::new(String::new())),
            current_title: Arc::new(Mutex::new(None)),
            current_highlights: Arc::new(Mutex::new(Vec::new())),
        }
    }

    /// Set the undo executor
    pub fn set_undo_executor<F>(&self, executor: F)
    where
        F: Fn(&UndoableAction) -> Result<(), String> + Send + Sync + 'static,
    {
        self.undo_stack.set_undo_executor(executor);
    }

    /// Update current page state
    pub fn update_page_state(&self, url: &str, title: Option<&str>) {
        let mut current_url = self.current_url.lock().unwrap();
        let mut current_title = self.current_title.lock().unwrap();
        *current_url = url.to_string();
        *current_title = title.map(|t| t.to_string());
    }

    /// Record navigation with automatic state tracking
    pub fn record_navigation(&self, action_id: &str, new_url: &str, new_title: Option<&str>) {
        let (from_url, from_title) = {
            let url = self.current_url.lock().unwrap();
            let title = self.current_title.lock().unwrap();
            (url.clone(), title.clone())
        };

        if from_url.is_empty() {
            // No baseline state yet; just update current state without recording
            self.update_page_state(new_url, new_title);
            return;
        }

        self.undo_stack.record_navigation(
            action_id,
            &from_url,
            from_title.as_deref(),
            new_url,
            new_title,
        );

        // Update current state
        self.update_page_state(new_url, new_title);
    }

    /// Record effect application
    pub fn record_effect(&self, action_id: &str, effect_name: &str, duration_ms: u64) {
        self.undo_stack
            .record_effect(action_id, effect_name, duration_ms);
    }

    /// Record text highlight
    pub fn record_highlight(&self, action_id: &str, text: &str) {
        let existing = self.current_highlights.lock().unwrap().clone();
        self.undo_stack.record_highlight(action_id, text, existing);

        // Update current highlights
        let mut highlights = self.current_highlights.lock().unwrap();
        highlights.push(text.to_string());
    }

    /// Record a file write action
    pub fn record_file_write(
        &self,
        action_id: &str,
        path: &str,
        previous_content: Option<String>,
        backup_path: Option<String>,
        created: bool,
    ) {
        let can_undo = created || previous_content.is_some() || backup_path.is_some();
        let undo_blocked_reason = if can_undo {
            None
        } else {
            Some("No backup available for file restore".to_string())
        };

        let action = UndoableAction {
            id: action_id.to_string(),
            action_type: "sandbox.write_file".to_string(),
            description: format!("Write file: {}", path),
            before_state: ActionState::file_write(path, previous_content, backup_path, created),
            after_state: ActionState::Empty,
            executed_at: Utc::now(),
            can_undo,
            undo_blocked_reason,
        };
        self.undo_stack.push(action);
    }

    /// Record a note change action
    pub fn record_note_change(
        &self,
        action_id: &str,
        note_id: &str,
        before: Option<NoteSnapshot>,
        after: Option<NoteSnapshot>,
        description: String,
    ) {
        let can_undo = before.is_some() || after.is_some();
        let undo_blocked_reason = if can_undo {
            None
        } else {
            Some("No note state available".to_string())
        };

        let action = UndoableAction {
            id: action_id.to_string(),
            action_type: "notes.change".to_string(),
            description,
            before_state: ActionState::note_change(note_id, before, after),
            after_state: ActionState::Empty,
            executed_at: Utc::now(),
            can_undo,
            undo_blocked_reason,
        };
        self.undo_stack.push(action);
    }

    /// Clear all highlights
    pub fn clear_highlights(&self) {
        let mut highlights = self.current_highlights.lock().unwrap();
        highlights.clear();
    }

    /// Get current highlights
    pub fn get_highlights(&self) -> Vec<String> {
        self.current_highlights.lock().unwrap().clone()
    }

    /// Undo the last action
    pub fn undo(&self) -> UndoResult {
        let result = self.undo_stack.undo();

        // Update internal state based on restored state
        if let Some(state) = &result.restored_state {
            match state {
                ActionState::Navigation { url, title } => {
                    self.update_page_state(url, title.as_deref());
                }
                ActionState::Highlight { highlighted_texts } => {
                    let mut highlights = self.current_highlights.lock().unwrap();
                    *highlights = highlighted_texts.clone();
                }
                _ => {}
            }
        }

        result
    }

    /// Redo the last undone action
    pub fn redo(&self) -> UndoResult {
        let result = self.undo_stack.redo();

        // Update internal state based on restored state
        if let Some(state) = &result.restored_state {
            match state {
                ActionState::Navigation { url, title } => {
                    self.update_page_state(url, title.as_deref());
                }
                ActionState::Highlight { highlighted_texts } => {
                    let mut highlights = self.current_highlights.lock().unwrap();
                    *highlights = highlighted_texts.clone();
                }
                _ => {}
            }
        }

        result
    }

    /// Check if undo is available
    pub fn can_undo(&self) -> bool {
        self.undo_stack.can_undo()
    }

    /// Check if redo is available
    pub fn can_redo(&self) -> bool {
        self.undo_stack.can_redo()
    }

    /// Get undo/redo status for UI
    pub fn get_status(&self) -> RollbackStatus {
        RollbackStatus {
            can_undo: self.can_undo(),
            can_redo: self.can_redo(),
            undo_description: self.undo_stack.undo_description(),
            redo_description: self.undo_stack.redo_description(),
            stack_size: self.undo_stack.size(),
            recent_actions: self.undo_stack.get_recent(5),
        }
    }
}

/// Rollback status for UI
#[derive(Debug, Clone, Serialize)]
pub struct RollbackStatus {
    pub can_undo: bool,
    pub can_redo: bool,
    pub undo_description: Option<String>,
    pub redo_description: Option<String>,
    pub stack_size: usize,
    pub recent_actions: Vec<UndoableAction>,
}

// ============================================================================
// Global Instance
// ============================================================================

use lazy_static::lazy_static;

lazy_static! {
    /// Global rollback manager instance
    static ref ROLLBACK_MANAGER: RollbackManager = RollbackManager::new();
}

/// Initialize the global rollback manager (no-op with lazy_static, kept for API compatibility)
pub fn init_rollback_manager() {
    // The lazy_static initializes on first access
    let _ = ROLLBACK_MANAGER.can_undo();
}

#[cfg(test)]
pub fn default_undo_executor(
) -> impl Fn(&UndoableAction) -> Result<(), String> + Send + Sync + 'static {
    |_| Ok(())
}

/// Set the global undo executor
pub fn set_undo_executor<F>(executor: F)
where
    F: Fn(&UndoableAction) -> Result<(), String> + Send + Sync + 'static,
{
    if let Some(manager) = get_rollback_manager() {
        manager.set_undo_executor(executor);
    }
}

/// Get the global rollback manager
pub fn get_rollback_manager() -> Option<&'static RollbackManager> {
    Some(&*ROLLBACK_MANAGER)
}

#[cfg(not(test))]
pub fn default_undo_executor(
) -> impl Fn(&UndoableAction) -> Result<(), String> + Send + Sync + 'static {
    |action| match &action.before_state {
        ActionState::Navigation { .. }
        | ActionState::Highlight { .. }
        | ActionState::Effect { .. } => Ok(()),
        ActionState::FileWrite {
            path,
            previous_content,
            backup_path,
            created,
        } => {
            let path_buf = std::path::PathBuf::from(path);
            if *created {
                if path_buf.exists() {
                    std::fs::remove_file(&path_buf).map_err(|e| e.to_string())?;
                }
                if let Some(backup) = backup_path {
                    let backup_buf = std::path::PathBuf::from(backup);
                    if backup_buf.exists() {
                        let _ = std::fs::remove_file(backup_buf);
                    }
                }
                return Ok(());
            }

            if let Some(content) = previous_content {
                std::fs::write(&path_buf, content).map_err(|e| e.to_string())?;
                if let Some(backup) = backup_path {
                    let backup_buf = std::path::PathBuf::from(backup);
                    if backup_buf.exists() {
                        let _ = std::fs::remove_file(backup_buf);
                    }
                }
                return Ok(());
            }

            if let Some(backup) = backup_path {
                let backup_buf = std::path::PathBuf::from(backup);
                if backup_buf.exists() {
                    std::fs::copy(&backup_buf, &path_buf).map_err(|e| e.to_string())?;
                    let _ = std::fs::remove_file(backup_buf);
                    return Ok(());
                }
            }

            Err("No backup content available to restore file".to_string())
        }
        ActionState::NoteChange { before, after, .. } => {
            let store = crate::memory::MemoryStore::new().map_err(|e| e.to_string())?;
            match (before, after) {
                (Some(snapshot), Some(_)) => {
                    let note: crate::integrations::Note = snapshot.clone().into();
                    store
                        .set("notes", &note.id, &note)
                        .map_err(|e| e.to_string())?;
                    let _ = store.flush();
                    Ok(())
                }
                (Some(snapshot), None) => {
                    let note: crate::integrations::Note = snapshot.clone().into();
                    store
                        .set("notes", &note.id, &note)
                        .map_err(|e| e.to_string())?;
                    let _ = store.flush();
                    Ok(())
                }
                (None, Some(snapshot)) => {
                    store
                        .delete("notes", &snapshot.id)
                        .map_err(|e| e.to_string())?;
                    let _ = store.flush();
                    Ok(())
                }
                _ => Err("No note state available to rollback".to_string()),
            }
        }
        _ => Ok(()),
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_record_navigation() {
        let manager = RollbackManager::new();
        manager.update_page_state("https://start.com", Some("Start"));

        manager.record_navigation("nav_1", "https://example.com", Some("Example"));

        assert!(manager.can_undo());
        assert_eq!(manager.undo_stack.size(), 1);
    }

    #[test]
    fn test_undo_navigation() {
        let manager = RollbackManager::new();
        manager.update_page_state("https://start.com", Some("Start"));
        manager.record_navigation("nav_1", "https://example.com", Some("Example"));

        let result = manager.undo();

        assert!(result.success);
        assert!(!manager.can_undo());
        assert!(manager.can_redo());

        if let Some(ActionState::Navigation { url, .. }) = result.restored_state {
            assert_eq!(url, "https://start.com");
        } else {
            panic!("Expected navigation state");
        }
    }

    #[test]
    fn test_redo() {
        let manager = RollbackManager::new();
        manager.update_page_state("https://start.com", Some("Start"));
        manager.record_navigation("nav_1", "https://example.com", Some("Example"));

        manager.undo();
        let result = manager.redo();

        assert!(result.success);
        assert!(manager.can_undo());
        assert!(!manager.can_redo());
    }

    #[test]
    fn test_highlight_tracking() {
        let manager = RollbackManager::new();

        manager.record_highlight("hl_1", "first highlight");
        manager.record_highlight("hl_2", "second highlight");

        let highlights = manager.get_highlights();
        assert_eq!(highlights.len(), 2);

        // Undo last highlight
        manager.undo();

        let highlights = manager.get_highlights();
        assert_eq!(highlights.len(), 1);
        assert_eq!(highlights[0], "first highlight");
    }

    #[test]
    fn test_stack_limit() {
        let stack = UndoStack::new();

        for i in 0..60 {
            stack.push(UndoableAction {
                id: format!("action_{}", i),
                action_type: "test".to_string(),
                description: format!("Action {}", i),
                before_state: ActionState::Empty,
                after_state: ActionState::Empty,
                executed_at: Utc::now(),
                can_undo: true,
                undo_blocked_reason: None,
            });
        }

        assert_eq!(stack.size(), MAX_UNDO_STACK_SIZE);
    }
}
