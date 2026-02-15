//! Actions module - handles action execution, preview, ledger, and workflows

pub mod action_ledger;
pub mod action_preview;
#[allow(clippy::module_inception)]
pub mod actions;
pub mod rollback;
pub mod workflows;

// Re-export commonly used types from actions.rs
pub use action_ledger::{
    export_action_ledger, get_action_ledger, ActionLedger, ActionLedgerEntry, ActionLedgerStatus,
};
pub use action_preview::{
    ActionPreview, PreviewManager, PreviewState, VisualPreview, VisualPreviewType,
};
pub use actions::{
    approve_action, clear_action_history, clear_pending_actions, deny_action,
    execute_approved_action, get_action_history, get_pending_actions, ActionRiskLevel,
    ActionStatus, HandlerContext, PendingAction, ACTION_QUEUE,
};
pub use rollback::{
    get_rollback_manager, init_rollback_manager, set_undo_executor, RollbackManager,
    RollbackStatus, UndoResult, UndoableAction,
};
pub use workflows::{execute_plan, WorkflowResult, WorkflowStep};
