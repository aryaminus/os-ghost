//! Shared system status store for runtime health signals

use serde::{Deserialize, Serialize};
use std::sync::{Arc, OnceLock, RwLock};

pub const HEARTBEAT_TIMEOUT_SECS: u64 = 30;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SystemStatusStore {
    pub extension_connected: bool,
    pub extension_operational: bool,
    pub last_extension_heartbeat: Option<u64>,
    pub last_extension_hello: Option<u64>,
    pub last_page_update: Option<u64>,
    pub last_tab_screenshot_at: Option<u64>,
    pub last_known_url: Option<String>,
    pub extension_protocol_version: Option<String>,
    pub extension_version: Option<String>,
    pub extension_id: Option<String>,
    pub extension_capabilities: Option<serde_json::Value>,
    pub mcp_browser_connected: bool,
    pub last_error: Option<String>,
    pub active_provider: Option<String>,
}

static STATUS_STORE: OnceLock<Arc<RwLock<SystemStatusStore>>> = OnceLock::new();

pub fn init_system_status_store(store: Arc<RwLock<SystemStatusStore>>) {
    let _ = STATUS_STORE.set(store);
}

pub fn get_system_status_store() -> Option<Arc<RwLock<SystemStatusStore>>> {
    STATUS_STORE.get().cloned()
}

pub fn get_status_snapshot() -> Option<SystemStatusStore> {
    STATUS_STORE
        .get()
        .and_then(|store| store.read().ok().map(|s| s.clone()))
}

pub fn update_status<F>(update: F) -> Option<SystemStatusStore>
where
    F: FnOnce(&mut SystemStatusStore),
{
    let store = STATUS_STORE.get()?;
    let mut guard = store.write().ok()?;
    update(&mut guard);
    Some(guard.clone())
}
