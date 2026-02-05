//! Server Configuration
//!
//! Production configuration management for the headless server.
//! Supports environment variables and config files.

use serde::{Deserialize, Serialize};
use std::env;
use std::path::PathBuf;

/// Server configuration
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ServerConfig {
    /// Host to bind to
    pub host: String,
    /// Port to listen on
    pub port: u16,
    /// API key for authentication
    pub api_key: Option<String>,
    /// Enable WebSocket support
    pub enable_websocket: bool,
    /// Enable CORS
    pub enable_cors: bool,
    /// Run in headless mode
    pub headless: bool,
    /// Data directory path
    pub data_dir: PathBuf,
    /// Log level
    pub log_level: String,
    /// Maximum request size in bytes
    pub max_request_size: usize,
    /// Request timeout in seconds
    pub request_timeout_secs: u64,
}

impl ServerConfig {
    /// Load configuration from environment variables
    pub fn from_env() -> Self {
        Self {
            host: env::var("OSGHOST_HOST").unwrap_or_else(|_| "127.0.0.1".to_string()),
            port: env::var("OSGHOST_PORT")
                .ok()
                .and_then(|p| p.parse().ok())
                .unwrap_or(7842),
            api_key: env::var("OSGHOST_API_KEY").ok(),
            enable_websocket: env::var("OSGHOST_ENABLE_WEBSOCKET")
                .map(|v| v != "false")
                .unwrap_or(true),
            enable_cors: env::var("OSGHOST_ENABLE_CORS")
                .map(|v| v != "false")
                .unwrap_or(true),
            headless: env::var("OSGHOST_HEADLESS")
                .map(|v| v == "true")
                .unwrap_or(false),
            data_dir: env::var("OSGHOST_DATA_DIR")
                .map(PathBuf::from)
                .unwrap_or_else(|_| dirs::data_dir().unwrap_or_default().join("os-ghost")),
            log_level: env::var("OSGHOST_LOG_LEVEL").unwrap_or_else(|_| "info".to_string()),
            max_request_size: env::var("OSGHOST_MAX_REQUEST_SIZE")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(10 * 1024 * 1024), // 10MB
            request_timeout_secs: env::var("OSGHOST_REQUEST_TIMEOUT")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(60),
        }
    }

    /// Create config from CLI arguments
    pub fn from_cli(
        host: String,
        port: u16,
        api_key: Option<String>,
        enable_websocket: bool,
        enable_cors: bool,
        headless: bool,
    ) -> Self {
        let mut config = Self::from_env();
        config.host = host;
        config.port = port;
        if api_key.is_some() {
            config.api_key = api_key;
        }
        config.enable_websocket = enable_websocket;
        config.enable_cors = enable_cors;
        config.headless = headless;
        config
    }

    /// Validate configuration
    pub fn validate(&self) -> Result<(), String> {
        // Check port range
        if self.port == 0 || self.port > 65535 {
            return Err(format!("Invalid port: {}", self.port));
        }

        // Check data directory
        if !self.data_dir.exists() {
            std::fs::create_dir_all(&self.data_dir)
                .map_err(|e| format!("Failed to create data directory: {}", e))?;
        }

        // Warn if no API key in production
        if self.api_key.is_none() {
            eprintln!("WARNING: No API key configured. Server will accept all requests.");
        }

        Ok(())
    }

    /// Get bind address
    pub fn bind_addr(&self) -> String {
        format!("{}:{}", self.host, self.port)
    }

    /// Check if authentication is required
    pub fn auth_required(&self) -> bool {
        self.api_key.is_some()
    }
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            host: "127.0.0.1".to_string(),
            port: 7842,
            api_key: None,
            enable_websocket: true,
            enable_cors: true,
            headless: false,
            data_dir: dirs::data_dir().unwrap_or_default().join("os-ghost"),
            log_level: "info".to_string(),
            max_request_size: 10 * 1024 * 1024,
            request_timeout_secs: 60,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = ServerConfig::default();
        assert_eq!(config.host, "127.0.0.1");
        assert_eq!(config.port, 7842);
        assert!(config.enable_websocket);
        assert!(config.enable_cors);
    }

    #[test]
    fn test_bind_addr() {
        let config = ServerConfig {
            host: "0.0.0.0".to_string(),
            port: 8080,
            ..Default::default()
        };
        assert_eq!(config.bind_addr(), "0.0.0.0:8080");
    }
}
