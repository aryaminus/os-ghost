//! Pairing manager for trusted devices/channels

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

const PAIRING_FILE: &str = "pairing_state.json";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrustedSource {
    pub id: String,
    pub source_type: String,
    pub label: String,
    pub approved_at: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PairingState {
    pub trusted_sources: Vec<TrustedSource>,
    pub pending_code: Option<String>,
    pub pending_expires_at: Option<u64>,
}

fn pairing_path() -> PathBuf {
    let mut path = dirs::config_dir().unwrap_or_else(|| PathBuf::from("."));
    path.push("os-ghost");
    path.push(PAIRING_FILE);
    path
}

fn load_state() -> PairingState {
    let path = pairing_path();
    if path.exists() {
        if let Ok(contents) = fs::read_to_string(&path) {
            if let Ok(state) = serde_json::from_str(&contents) {
                return state;
            }
        }
    }
    PairingState::default()
}

fn save_state(state: &PairingState) -> Result<(), String> {
    let path = pairing_path();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let contents = serde_json::to_string_pretty(state).map_err(|e| e.to_string())?;
    fs::write(&path, contents).map_err(|e| e.to_string())?;
    Ok(())
}

pub fn ensure_trusted_source(id: &str, source_type: &str, label: &str) {
    let mut state = load_state();
    if state
        .trusted_sources
        .iter()
        .any(|s| s.id == id && s.source_type == source_type)
    {
        return;
    }

    state.trusted_sources.push(TrustedSource {
        id: id.to_string(),
        source_type: source_type.to_string(),
        label: label.to_string(),
        approved_at: crate::utils::current_timestamp(),
    });
    let _ = save_state(&state);
}

#[tauri::command]
pub fn get_pairing_status() -> PairingState {
    load_state()
}

#[tauri::command]
pub fn create_pairing_code() -> Result<PairingState, String> {
    let mut state = load_state();
    let code = format!("{}", rand::random::<u32>() % 1_000_000);
    let expires = crate::utils::current_timestamp().saturating_add(600);
    state.pending_code = Some(format!("{:06}", code.parse::<u32>().unwrap_or(0)));
    state.pending_expires_at = Some(expires);
    save_state(&state)?;
    Ok(state)
}

#[tauri::command]
pub fn approve_pairing(
    code: String,
    source_id: String,
    source_type: String,
    label: String,
) -> Result<PairingState, String> {
    let mut state = load_state();
    let now = crate::utils::current_timestamp();
    let valid = state
        .pending_code
        .as_deref()
        .map(|c| c == code)
        .unwrap_or(false)
        && state.pending_expires_at.unwrap_or(0) >= now;

    if !valid {
        return Err("Invalid or expired pairing code".to_string());
    }

    state.trusted_sources.push(TrustedSource {
        id: source_id,
        source_type,
        label,
        approved_at: now,
    });
    state.pending_code = None;
    state.pending_expires_at = None;
    save_state(&state)?;
    Ok(state)
}

#[tauri::command]
pub fn clear_pairing_code() -> Result<PairingState, String> {
    let mut state = load_state();
    state.pending_code = None;
    state.pending_expires_at = None;
    save_state(&state)?;
    Ok(state)
}

#[tauri::command]
pub fn reject_pairing() -> Result<PairingState, String> {
    clear_pairing_code()
}
