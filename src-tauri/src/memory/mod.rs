//! Memory system using sled embedded database
//! Provides persistent storage for game state, sessions, and vector embeddings
//!
//! ## ADK-Style State Scoping
//! Supports scoped state with prefixes:
//! - `temp:` - Invocation-scoped (discarded each turn)
//! - `user:` - User-scoped (persisted per user)
//! - `app:` - Application-scoped (global settings)
//! - (no prefix) - Session-scoped (default)

pub mod compaction;
pub mod hybrid;
pub mod long_term;
pub mod scoped_state;
pub mod session;
pub mod store;

pub use compaction::{get_boot_tasks, inject_workspace_context, run_silent_memory_turn, should_compact};
pub use long_term::LongTermMemory;
pub use scoped_state::{IntoScopedKey, ScopedState, StateScope};
pub use session::{ActivityEntry, AppMode, SessionMemory};
pub use store::MemoryStore;
