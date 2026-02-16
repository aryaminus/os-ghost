//! Pairing System for Gateway Security
//!
//! Provides secure authentication for the webhook gateway using one-time pairing codes.
//! Inspired by ZeroClaw: https://github.com/theonlyhennygod/zeroclaw
//!
//! Flow:
//! 1. Gateway generates a 6-digit pairing code on startup
//! 2. Client sends POST /pair with the code
//! 3. Server exchanges code for a bearer token
//! 4. Subsequent requests use Authorization: Bearer <token>

use rand::Rng;
use serde::{Deserialize, Serialize};
use std::sync::RwLock;
use std::time::{SystemTime, UNIX_EPOCH};

const PAIRING_CODE_LENGTH: usize = 6;
const PAIRING_CODE_EXPIRY_SECS: u64 = 300; // 5 minutes
const TOKEN_EXPIRY_SECS: u64 = 86400 * 30; // 30 days

lazy_static::lazy_static! {
    static ref PAIRING_STATE: RwLock<Option<PairingState>> = RwLock::new(None);
    static ref ACTIVE_TOKENS: RwLock<Vec<AuthToken>> = RwLock::new(vec![]);
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PairingState {
    pub code: String,
    pub created_at: u64,
    pub expires_at: u64,
    pub used: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthToken {
    pub token: String,
    pub created_at: u64,
    pub expires_at: u64,
    pub last_used: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PairingError {
    pub message: String,
}

impl std::fmt::Display for PairingError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for PairingError {}

fn current_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn constant_time_compare(a: &str, b: &str) -> bool {
    let a_bytes = a.as_bytes();
    let b_bytes = b.as_bytes();

    if a_bytes.len() != b_bytes.len() {
        return false;
    }

    let mut result = true;
    for (x, y) in a_bytes.iter().zip(b_bytes.iter()) {
        result &= x == y;
    }
    result
}

fn generate_code() -> String {
    use rand::RngCore;
    let mut rng = rand::rngs::OsRng;
    let mut bytes = [0u8; 4];
    rng.fill_bytes(&mut bytes);
    let code = u32::from_le_bytes(bytes) % 900000 + 100000;
    code.to_string()
}

fn generate_token() -> String {
    use rand::RngCore;
    let mut rng = rand::rngs::OsRng;
    let mut bytes = [0u8; 32];
    rng.fill_bytes(&mut bytes);
    base64::Engine::encode(&base64::engine::general_purpose::URL_SAFE, &bytes)
}

fn cleanup_expired_tokens() {
    if let Ok(mut tokens) = ACTIVE_TOKENS.write() {
        let now = current_timestamp();
        tokens.retain(|t| t.expires_at > now);
    }
}

// ============================================================================
// Pairing Functions
// ============================================================================

pub fn generate_pairing_code() -> Result<String, PairingError> {
    let code = generate_code();
    let now = current_timestamp();

    let state = PairingState {
        code: code.clone(),
        created_at: now,
        expires_at: now + PAIRING_CODE_EXPIRY_SECS,
        used: false,
    };

    if let Ok(mut pairing) = PAIRING_STATE.write() {
        *pairing = Some(state);
    }

    tracing::info!("Generated pairing code: {}", code);
    Ok(code)
}

pub fn validate_pairing_code(code: &str) -> Result<String, PairingError> {
    let pairing = if let Ok(p) = PAIRING_STATE.read() {
        p.clone()
    } else {
        return Err(PairingError {
            message: "Pairing system not initialized".to_string(),
        });
    };

    let Some(state) = pairing else {
        return Err(PairingError {
            message: "No pairing code generated".to_string(),
        });
    };

    let now = current_timestamp();

    if now > state.expires_at {
        return Err(PairingError {
            message: "Pairing code expired".to_string(),
        });
    }

    if state.used {
        return Err(PairingError {
            message: "Pairing code already used".to_string(),
        });
    }

    if state.code != code {
        return Err(PairingError {
            message: "Invalid pairing code".to_string(),
        });
    }

    // Mark code as used
    if let Ok(mut pairing) = PAIRING_STATE.write() {
        if let Some(ref mut s) = *pairing {
            s.used = true;
        }
    }

    // Generate and return auth token
    let token = generate_token();
    let auth_token = AuthToken {
        token: token.clone(),
        created_at: now,
        expires_at: now + TOKEN_EXPIRY_SECS,
        last_used: now,
    };

    if let Ok(mut tokens) = ACTIVE_TOKENS.write() {
        // Cleanup expired tokens before adding new one
        let now = current_timestamp();
        tokens.retain(|t| t.expires_at > now);
        tokens.push(auth_token);
    }

    tracing::info!("Pairing successful, token generated");
    Ok(token)
}

pub fn validate_token(token: &str) -> bool {
    // Cleanup expired tokens first
    cleanup_expired_tokens();

    let tokens = if let Ok(t) = ACTIVE_TOKENS.read() {
        t.clone()
    } else {
        return false;
    };

    let now = current_timestamp();

    for t in &tokens {
        // Use constant-time comparison to prevent timing attacks
        if constant_time_compare(&t.token, token) && now < t.expires_at {
            return true;
        }
    }

    false
}

pub fn revoke_token(token: &str) -> Result<(), PairingError> {
    if let Ok(mut tokens) = ACTIVE_TOKENS.write() {
        tokens.retain(|t| t.token != token);
    }
    Ok(())
}

pub fn get_pairing_code_status() -> Option<PairingCodeStatus> {
    if let Ok(pairing) = PAIRING_STATE.read() {
        if let Some(state) = &*pairing {
            let now = current_timestamp();
            let expired = now > state.expires_at;
            let status = if state.used {
                "used"
            } else if expired {
                "expired"
            } else {
                "active"
            };

            return Some(PairingCodeStatus {
                status: status.to_string(),
                expires_in: if expired { 0 } else { state.expires_at - now },
            });
        }
    }
    None
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PairingCodeStatus {
    pub status: String,
    pub expires_in: u64,
}

// ============================================================================
// Tauri Commands
// ============================================================================

#[tauri::command]
pub fn create_pairing_code() -> Result<String, String> {
    generate_pairing_code().map_err(|e| e.message)
}

#[tauri::command]
pub fn exchange_pairing_code(code: String) -> Result<String, String> {
    validate_pairing_code(&code).map_err(|e| e.message)
}

#[tauri::command]
pub fn validate_auth_token(token: String) -> bool {
    validate_token(&token)
}

#[tauri::command]
pub fn revoke_auth_token(token: String) -> Result<(), String> {
    revoke_token(&token).map_err(|e| e.message)
}

#[tauri::command]
pub fn get_pairing_status() -> Option<PairingCodeStatus> {
    get_pairing_code_status()
}

// ============================================================================
// Auth Middleware Helper
// ============================================================================

pub fn extract_token_from_header(auth_header: &str) -> Option<&str> {
    if auth_header.starts_with("Bearer ") {
        Some(&auth_header[7..])
    } else {
        None
    }
}

pub fn require_auth(auth_header: &str) -> Result<(), String> {
    let token = extract_token_from_header(auth_header).ok_or("Missing Authorization header")?;

    if validate_token(token) {
        Ok(())
    } else {
        Err("Invalid or expired token".to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_code_length() {
        let code = generate_code();
        assert_eq!(code.len(), 6);
        assert!(code.chars().all(|c| c.is_ascii_digit()));
    }

    #[test]
    fn test_pairing_flow() {
        // Generate code
        let code = generate_pairing_code().unwrap();
        assert_eq!(code.len(), 6);

        // Validate code
        let token = validate_pairing_code(&code).unwrap();
        assert!(!token.is_empty());

        // Validate token
        assert!(validate_token(&token));

        // Invalid code should fail
        assert!(validate_pairing_code("000000").is_err());
    }
}
