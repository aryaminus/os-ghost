//! Actions module - handles action execution, preview, ledger, and workflows

pub mod action_ledger;
pub mod action_preview;
#[allow(clippy::module_inception)]
pub mod actions;
pub mod rollback;
pub mod workflows;

// Re-export commonly used types from actions.rs
pub use actions::{
    ActionRiskLevel, ActionStatus, HandlerContext, PendingAction, 
    ACTION_QUEUE, get_pending_actions, approve_action, deny_action, 
    get_action_history, clear_pending_actions, clear_action_history,
    execute_approved_action
};
pub use action_ledger::{ActionLedger, ActionLedgerEntry, ActionLedgerStatus, get_action_ledger, export_action_ledger};
pub use action_preview::{ActionPreview, PreviewManager, PreviewState, VisualPreview, VisualPreviewType};
pub use rollback::{RollbackManager, UndoResult, UndoableAction, RollbackStatus, init_rollback_manager, set_undo_executor, get_rollback_manager};
pub use workflows::{WorkflowResult, WorkflowStep, execute_plan};
