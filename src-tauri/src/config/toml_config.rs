//! TOML Configuration with Environment Variable Overrides
//!
//! ZeroClaw-style configuration system that supports:
//! - TOML configuration file
//! - Environment variable overrides
//! - Atomic writes with backup
//!
//! Reference: https://github.com/theonlyhennygod/zeroclaw

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use std::sync::RwLock;

lazy_static::lazy_static! {
    static ref TOML_CONFIG: RwLock<Option<TomlConfig>> = RwLock::new(None);
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TomlConfig {
    #[serde(default)]
    pub core: CoreConfig,
    #[serde(default)]
    pub memory: MemoryConfig,
    #[serde(default)]
    pub gateway: GatewayConfig,
    #[serde(default)]
    pub autonomy: AutonomyConfig,
    #[serde(default)]
    pub runtime: RuntimeConfig,
    #[serde(default)]
    pub heartbeat: HeartbeatConfig,
    #[serde(default)]
    pub tunnel: TunnelConfig,
    #[serde(default)]
    pub browser: BrowserConfig,
    #[serde(default)]
    pub identity: IdentityConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CoreConfig {
    #[serde(default)]
    pub api_key: Option<String>,
    #[serde(default)]
    pub default_provider: Option<String>,
    #[serde(default)]
    pub default_model: Option<String>,
    #[serde(default = "default_temperature")]
    pub default_temperature: f64,
}

fn default_temperature() -> f64 {
    0.7
}

impl Default for CoreConfig {
    fn default() -> Self {
        Self {
            api_key: None,
            default_provider: Some("gemini".to_string()),
            default_model: None,
            default_temperature: 0.7,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryConfig {
    #[serde(default = "default_memory_backend")]
    pub backend: String,
    #[serde(default = "default_true")]
    pub auto_save: bool,
    #[serde(default)]
    pub embedding_provider: Option<String>,
    #[serde(default = "default_vector_weight")]
    pub vector_weight: f64,
    #[serde(default = "default_keyword_weight")]
    pub keyword_weight: f64,
}

fn default_memory_backend() -> String {
    "sled".to_string()
}
fn default_true() -> bool {
    true
}
fn default_vector_weight() -> f64 {
    0.7
}
fn default_keyword_weight() -> f64 {
    0.3
}

impl Default for MemoryConfig {
    fn default() -> Self {
        Self {
            backend: "sled".to_string(),
            auto_save: true,
            embedding_provider: None,
            vector_weight: 0.7,
            keyword_weight: 0.3,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GatewayConfig {
    #[serde(default = "default_true")]
    pub require_pairing: bool,
    #[serde(default = "default_false")]
    pub allow_public_bind: bool,
    #[serde(default = "default_port")]
    pub port: u16,
}

fn default_false() -> bool {
    false
}
fn default_port() -> u16 {
    8080
}

impl Default for GatewayConfig {
    fn default() -> Self {
        Self {
            require_pairing: true,
            allow_public_bind: false,
            port: 8080,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AutonomyConfig {
    #[serde(default = "default_autonomy_level")]
    pub level: String,
    #[serde(default = "default_true")]
    pub workspace_only: bool,
    #[serde(default)]
    pub allowed_commands: Vec<String>,
    #[serde(default)]
    pub forbidden_paths: Vec<String>,
}

fn default_autonomy_level() -> String {
    "supervised".to_string()
}

impl Default for AutonomyConfig {
    fn default() -> Self {
        Self {
            level: "supervised".to_string(),
            workspace_only: true,
            allowed_commands: vec![
                "git".to_string(),
                "npm".to_string(),
                "cargo".to_string(),
                "ls".to_string(),
                "cat".to_string(),
                "grep".to_string(),
            ],
            forbidden_paths: vec![
                "/etc".to_string(),
                "/root".to_string(),
                "/proc".to_string(),
                "/sys".to_string(),
            ],
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuntimeConfig {
    #[serde(default = "default_runtime_kind")]
    pub kind: String,
}

fn default_runtime_kind() -> String {
    "native".to_string()
}

impl Default for RuntimeConfig {
    fn default() -> Self {
        Self {
            kind: "native".to_string(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HeartbeatConfig {
    #[serde(default = "default_false")]
    pub enabled: bool,
    #[serde(default = "default_interval")]
    pub interval_minutes: u32,
}

fn default_interval() -> u32 {
    30
}

impl Default for HeartbeatConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            interval_minutes: 30,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TunnelConfig {
    #[serde(default = "default_tunnel_provider")]
    pub provider: String,
}

fn default_tunnel_provider() -> String {
    "none".to_string()
}

impl Default for TunnelConfig {
    fn default() -> Self {
        Self {
            provider: "none".to_string(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct BrowserConfig {
    #[serde(default = "default_false")]
    pub enabled: bool,
    #[serde(default)]
    pub allowed_domains: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct IdentityConfig {
    #[serde(default = "default_identity_format")]
    pub format: String,
    #[serde(default)]
    pub path: Option<String>,
    #[serde(default)]
    pub inline: Option<String>,
}

fn default_identity_format() -> String {
    "openclaw".to_string()
}

fn get_config_path() -> PathBuf {
    let mut path = dirs::config_dir().unwrap_or_else(|| PathBuf::from("."));
    path.push("os-ghost");
    path.push("config.toml");
    path
}

pub fn load_toml_config() -> TomlConfig {
    // Check if already loaded
    if let Ok(config) = TOML_CONFIG.read() {
        if let Some(ref cfg) = *config {
            return cfg.clone();
        }
    }

    let path = get_config_path();

    // Try to load from file
    if path.exists() {
        if let Ok(contents) = fs::read_to_string(&path) {
            if let Ok(config) = toml::from_str::<TomlConfig>(&contents) {
                // Apply environment variable overrides
                let config = apply_env_overrides(config);

                // Cache it
                if let Ok(mut cfg) = TOML_CONFIG.write() {
                    *cfg = Some(config.clone());
                }

                tracing::info!("Loaded TOML config from {:?}", path);
                return config;
            }
        }
    }

    // Return default config
    let default_config = TomlConfig::default();
    let config = apply_env_overrides(default_config);

    if let Ok(mut cfg) = TOML_CONFIG.write() {
        *cfg = Some(config.clone());
    }

    config
}

pub fn save_toml_config(config: &TomlConfig) -> Result<(), String> {
    let path = get_config_path();

    // Create parent directory if needed
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }

    // Create backup if file exists
    if path.exists() {
        let backup_path = path.with_extension("toml.bak");
        let _ = fs::copy(&path, &backup_path);
    }

    // Serialize and write atomically using temp file
    let contents = toml::to_string_pretty(config).map_err(|e| e.to_string())?;

    // Write to temp file first
    let temp_path = path.with_extension("toml.tmp");
    fs::write(&temp_path, &contents).map_err(|e| e.to_string())?;

    // Atomic rename
    fs::rename(&temp_path, &path).map_err(|e| e.to_string())?;

    // Update cache
    if let Ok(mut cfg) = TOML_CONFIG.write() {
        *cfg = Some(config.clone());
    }

    tracing::info!("Saved TOML config to {:?}", path);
    Ok(())
}

pub fn get_toml_config() -> TomlConfig {
    load_toml_config()
}

fn apply_env_overrides(mut config: TomlConfig) -> TomlConfig {
    // Environment variable overrides (ZeroClaw style)
    // OS_GHOST_API_KEY, OS_GHOST_PROVIDER, OS_GHOST_MODEL, etc.

    if let Ok(api_key) = std::env::var("OS_GHOST_API_KEY") {
        if !api_key.is_empty() {
            config.core.api_key = Some(api_key);
        }
    }

    if let Ok(provider) = std::env::var("OS_GHOST_PROVIDER") {
        if !provider.is_empty() {
            config.core.default_provider = Some(provider);
        }
    }

    if let Ok(model) = std::env::var("OS_GHOST_MODEL") {
        if !model.is_empty() {
            config.core.default_model = Some(model);
        }
    }

    if let Ok(temp) = std::env::var("OS_GHOST_TEMPERATURE") {
        if let Ok(t) = temp.parse::<f64>() {
            config.core.default_temperature = t;
        }
    }

    if let Ok(level) = std::env::var("OS_GHOST_AUTONOMY_LEVEL") {
        if !level.is_empty() {
            config.autonomy.level = level;
        }
    }

    if let Ok(workspace_only) = std::env::var("OS_GHOST_WORKSPACE_ONLY") {
        config.autonomy.workspace_only = workspace_only != "false";
    }

    if let Ok(enabled) = std::env::var("OS_GHOST_HEARTBEAT") {
        config.heartbeat.enabled = enabled == "true";
    }

    if let Ok(port) = std::env::var("OS_GHOST_PORT") {
        if let Ok(p) = port.parse::<u16>() {
            config.gateway.port = p;
        }
    }

    if let Ok(require_pairing) = std::env::var("OS_GHOST_REQUIRE_PAIRING") {
        config.gateway.require_pairing = require_pairing == "true";
    }

    if let Ok(allow_public) = std::env::var("OS_GHOST_ALLOW_PUBLIC") {
        config.gateway.allow_public_bind = allow_public == "true";
    }

    config
}

// ============================================================================
// Tauri Commands
// ============================================================================

#[tauri::command]
pub fn get_toml_settings() -> TomlConfig {
    load_toml_config()
}

#[tauri::command]
pub fn save_toml_settings(config: TomlConfig) -> Result<(), String> {
    save_toml_config(&config)
}

#[tauri::command]
pub fn get_toml_config_path() -> String {
    get_config_path().to_string_lossy().to_string()
}

#[tauri::command]
pub fn reset_toml_config() -> Result<(), String> {
    let default_config = TomlConfig::default();
    save_toml_config(&default_config)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = TomlConfig::default();
        assert_eq!(config.core.default_temperature, 0.7);
        assert_eq!(config.autonomy.level, "supervised");
    }

    #[test]
    fn test_env_override() {
        std::env::set_var("OS_GHOST_AUTONOMY_LEVEL", "full");
        std::env::set_var("OS_GHOST_PORT", "9000");

        let config = load_toml_config();
        assert_eq!(config.autonomy.level, "full");
        assert_eq!(config.gateway.port, 9000);

        std::env::remove_var("OS_GHOST_AUTONOMY_LEVEL");
        std::env::remove_var("OS_GHOST_PORT");
    }
}
