//! Plugin system - Moltis-inspired hooks and extensibility
//!
//! This module provides:
//! - Hook system for lifecycle events
//! - Extensible plugin architecture

pub mod hooks;

pub use hooks::{
    disable_hook, disable_hook_cmd, enable_hook, enable_hook_cmd, fire_event, get_hook_state,
    get_hooks as list_hooks, reload_hooks, HookDefinition, HookEvent, HookPayload, HookResult,
};

pub use hooks::execute_hook as run_hook;
