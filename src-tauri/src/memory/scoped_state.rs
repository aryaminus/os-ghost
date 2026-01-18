//! Scoped State Management - ADK-style state with prefix scoping
//!
//! Implements the ADK state scoping pattern:
//! - `temp:` - Invocation-scoped, discarded after each turn
//! - `user:` - User-scoped, persisted across sessions per user
//! - `app:` - Application-scoped, global settings and templates
//! - (no prefix) - Session-scoped, persisted per session (default)
//!
//! Reference: Google ADK Sessions and State documentation

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;

/// State scope prefix
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum StateScope {
    /// Temporary/invocation scope - discarded after each request
    Temp,
    /// User scope - persisted per user across sessions
    User,
    /// Application scope - global settings
    App,
    /// Session scope - persisted per session (default)
    Session,
}

impl StateScope {
    /// Parse scope from key prefix
    pub fn from_key(key: &str) -> (Self, &str) {
        if let Some(rest) = key.strip_prefix("temp:") {
            (StateScope::Temp, rest)
        } else if let Some(rest) = key.strip_prefix("user:") {
            (StateScope::User, rest)
        } else if let Some(rest) = key.strip_prefix("app:") {
            (StateScope::App, rest)
        } else {
            (StateScope::Session, key)
        }
    }

    /// Get prefix string for this scope
    pub fn prefix(&self) -> &'static str {
        match self {
            StateScope::Temp => "temp:",
            StateScope::User => "user:",
            StateScope::App => "app:",
            StateScope::Session => "",
        }
    }
}

/// Scoped state container - manages state with ADK-style scope prefixes
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ScopedState {
    /// Temporary state (cleared each invocation)
    #[serde(default)]
    pub temp: HashMap<String, Value>,
    
    /// User-scoped state (persisted per user)
    #[serde(default)]
    pub user: HashMap<String, Value>,
    
    /// Application-scoped state (global)
    #[serde(default)]
    pub app: HashMap<String, Value>,
    
    /// Session-scoped state (default)
    #[serde(default)]
    pub session: HashMap<String, Value>,
}

impl ScopedState {
    /// Create a new empty scoped state
    pub fn new() -> Self {
        Self::default()
    }

    /// Get a value by scoped key
    /// Automatically parses scope prefix from key
    pub fn get(&self, key: &str) -> Option<&Value> {
        let (scope, actual_key) = StateScope::from_key(key);
        match scope {
            StateScope::Temp => self.temp.get(actual_key),
            StateScope::User => self.user.get(actual_key),
            StateScope::App => self.app.get(actual_key),
            StateScope::Session => self.session.get(actual_key),
        }
    }

    /// Set a value by scoped key
    /// Automatically parses scope prefix from key
    pub fn set(&mut self, key: &str, value: Value) {
        let (scope, actual_key) = StateScope::from_key(key);
        let map = match scope {
            StateScope::Temp => &mut self.temp,
            StateScope::User => &mut self.user,
            StateScope::App => &mut self.app,
            StateScope::Session => &mut self.session,
        };
        map.insert(actual_key.to_string(), value);
    }

    /// Remove a value by scoped key
    pub fn remove(&mut self, key: &str) -> Option<Value> {
        let (scope, actual_key) = StateScope::from_key(key);
        match scope {
            StateScope::Temp => self.temp.remove(actual_key),
            StateScope::User => self.user.remove(actual_key),
            StateScope::App => self.app.remove(actual_key),
            StateScope::Session => self.session.remove(actual_key),
        }
    }

    /// Check if a key exists
    pub fn contains(&self, key: &str) -> bool {
        self.get(key).is_some()
    }

    /// Clear temporary state (call at start of each invocation)
    pub fn clear_temp(&mut self) {
        self.temp.clear();
    }

    /// Clear session state (call when session ends)
    pub fn clear_session(&mut self) {
        self.session.clear();
    }

    /// Apply a state delta (from EventActions)
    pub fn apply_delta(&mut self, delta: &HashMap<String, Value>) {
        for (key, value) in delta {
            self.set(key, value.clone());
        }
    }

    /// Get all keys in a specific scope
    pub fn keys_in_scope(&self, scope: StateScope) -> Vec<String> {
        let map = match scope {
            StateScope::Temp => &self.temp,
            StateScope::User => &self.user,
            StateScope::App => &self.app,
            StateScope::Session => &self.session,
        };
        map.keys()
            .map(|k| format!("{}{}", scope.prefix(), k))
            .collect()
    }

    /// Get typed value
    pub fn get_typed<T: serde::de::DeserializeOwned>(&self, key: &str) -> Option<T> {
        self.get(key)
            .and_then(|v| serde_json::from_value(v.clone()).ok())
    }

    /// Set typed value
    pub fn set_typed<T: Serialize>(&mut self, key: &str, value: &T) {
        if let Ok(v) = serde_json::to_value(value) {
            self.set(key, v);
        }
    }

    /// Merge another scoped state into this one
    /// Newer values overwrite older ones
    pub fn merge(&mut self, other: &ScopedState) {
        for (k, v) in &other.temp {
            self.temp.insert(k.clone(), v.clone());
        }
        for (k, v) in &other.user {
            self.user.insert(k.clone(), v.clone());
        }
        for (k, v) in &other.app {
            self.app.insert(k.clone(), v.clone());
        }
        for (k, v) in &other.session {
            self.session.insert(k.clone(), v.clone());
        }
    }

    /// Export session and user state for persistence
    /// (temp is not persisted, app is persisted separately)
    pub fn export_for_persistence(&self) -> (HashMap<String, Value>, HashMap<String, Value>) {
        (self.session.clone(), self.user.clone())
    }
}

/// Helper trait for converting to/from scoped state
pub trait IntoScopedKey {
    fn temp_key(&self) -> String;
    fn user_key(&self) -> String;
    fn app_key(&self) -> String;
    fn session_key(&self) -> String;
}

impl<T: AsRef<str>> IntoScopedKey for T {
    fn temp_key(&self) -> String {
        format!("temp:{}", self.as_ref())
    }

    fn user_key(&self) -> String {
        format!("user:{}", self.as_ref())
    }

    fn app_key(&self) -> String {
        format!("app:{}", self.as_ref())
    }

    fn session_key(&self) -> String {
        // Session is default, no prefix needed
        self.as_ref().to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_scope_parsing() {
        assert_eq!(StateScope::from_key("temp:foo"), (StateScope::Temp, "foo"));
        assert_eq!(StateScope::from_key("user:name"), (StateScope::User, "name"));
        assert_eq!(StateScope::from_key("app:version"), (StateScope::App, "version"));
        assert_eq!(StateScope::from_key("count"), (StateScope::Session, "count"));
    }

    #[test]
    fn test_scoped_state_operations() {
        let mut state = ScopedState::new();
        
        // Set values in different scopes
        state.set("temp:scratch", serde_json::json!("temporary"));
        state.set("user:name", serde_json::json!("Ghost"));
        state.set("app:version", serde_json::json!("1.0"));
        state.set("puzzle_id", serde_json::json!("puzzle_001"));
        
        // Verify values are in correct scopes
        assert_eq!(state.temp.len(), 1);
        assert_eq!(state.user.len(), 1);
        assert_eq!(state.app.len(), 1);
        assert_eq!(state.session.len(), 1);
        
        // Get values
        assert_eq!(state.get("temp:scratch"), Some(&serde_json::json!("temporary")));
        assert_eq!(state.get("user:name"), Some(&serde_json::json!("Ghost")));
        assert_eq!(state.get("puzzle_id"), Some(&serde_json::json!("puzzle_001")));
        
        // Clear temp
        state.clear_temp();
        assert!(state.temp.is_empty());
        assert_eq!(state.user.len(), 1); // User still has data
    }

    #[test]
    fn test_apply_delta() {
        let mut state = ScopedState::new();
        
        let mut delta = HashMap::new();
        delta.insert("temp:result".to_string(), serde_json::json!("success"));
        delta.insert("proximity".to_string(), serde_json::json!(0.75));
        
        state.apply_delta(&delta);
        
        assert_eq!(state.get("temp:result"), Some(&serde_json::json!("success")));
        assert_eq!(state.get("proximity"), Some(&serde_json::json!(0.75)));
    }

    #[test]
    fn test_into_scoped_key() {
        let key = "result";
        assert_eq!(key.temp_key(), "temp:result");
        assert_eq!(key.user_key(), "user:result");
        assert_eq!(key.app_key(), "app:result");
        assert_eq!(key.session_key(), "result");
    }

    #[test]
    fn test_typed_access() {
        let mut state = ScopedState::new();
        
        state.set_typed("user:score", &42i32);
        state.set_typed("user:active", &true);
        
        assert_eq!(state.get_typed::<i32>("user:score"), Some(42));
        assert_eq!(state.get_typed::<bool>("user:active"), Some(true));
    }
}
