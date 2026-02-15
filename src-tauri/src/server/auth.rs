//! Authentication Module
//!
//! Provides API key authentication for the OS-Ghost server.
//! In production, this should use more sophisticated auth mechanisms.

use axum::{
    extract::Request,
    http::{header::HeaderMap, StatusCode},
    middleware::Next,
    response::Response,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

/// Authentication configuration
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct AuthConfig {
    pub api_key: Option<String>,
    #[serde(default)]
    pub require_auth: bool,
}

/// API key authentication middleware
pub async fn auth_middleware(
    headers: HeaderMap,
    config: Arc<AuthConfig>,
    request: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    // If auth is not required, proceed
    if !config.require_auth {
        return Ok(next.run(request).await);
    }

    // Check for API key
    let api_key = headers
        .get("X-API-Key")
        .and_then(|v| v.to_str().ok())
        .or_else(|| {
            headers
                .get("Authorization")
                .and_then(|v| v.to_str().ok())
                .and_then(|v| v.strip_prefix("Bearer "))
        });

    match (&config.api_key, api_key) {
        (Some(expected), Some(provided)) if expected == provided => Ok(next.run(request).await),
        (None, _) => {
            // No API key configured, allow all
            Ok(next.run(request).await)
        }
        _ => Err(StatusCode::UNAUTHORIZED),
    }
}

/// Generate a secure API key
pub fn generate_api_key() -> String {
    use rand::Rng;

    const CHARSET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789";
    const KEY_LEN: usize = 32;

    let mut rng = rand::thread_rng();
    let key: String = (0..KEY_LEN)
        .map(|_| {
            let idx = rng.gen_range(0..CHARSET.len());
            CHARSET[idx] as char
        })
        .collect();

    format!("ghost_{}", key)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_api_key() {
        let key = generate_api_key();
        assert!(key.starts_with("ghost_"));
        assert_eq!(key.len(), 38); // "ghost_" + 32 chars
    }
}
