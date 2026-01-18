//! Memory system using sled embedded database
//! Provides persistent storage for game state, sessions, and vector embeddings
//!
//! ## ADK-Style State Scoping
//! Supports scoped state with prefixes:
//! - `temp:` - Invocation-scoped (discarded each turn)
//! - `user:` - User-scoped (persisted per user)
//! - `app:` - Application-scoped (global settings)
//! - (no prefix) - Session-scoped (default)

pub mod long_term;
pub mod scoped_state;
pub mod session;
pub mod store;

pub use long_term::LongTermMemory;
pub use scoped_state::{ScopedState, StateScope, IntoScopedKey};
pub use session::{ActivityEntry, AppMode, SessionMemory};
pub use store::MemoryStore;
