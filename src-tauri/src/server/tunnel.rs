//! Tunnel Integration Module
//!
//! Provides tunnel support for exposing local services to the internet.
//! Reference: ZeroClaw tunnel implementation
//!
//! Supported tunnel providers:
//! - Cloudflare Tunnel
//! - Tailscale
//! - ngrok
//! - Custom command

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::process::Stdio;
use tokio::process::Command;
use tokio::sync::RwLock;

pub mod cloudflare;
pub mod tailscale;
pub mod ngrok;

pub use cloudflare::CloudflareTunnel;
pub use tailscale::TailscaleTunnel;
pub use ngrok::NgrokTunnel;

// ============================================================================
// Tunnel Trait
// ============================================================================

#[async_trait]
pub trait Tunnel: Send + Sync {
    fn name(&self) -> &str;
    async fn start(&self, local_host: &str, local_port: u16) -> Result<String, TunnelError>;
    async fn stop(&self) -> Result<(), TunnelError>;
    async fn health_check(&self) -> bool;
    fn public_url(&self) -> Option<String>;
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TunnelError {
    NotConfigured(String),
    StartFailed(String),
    StopFailed(String),
    HealthCheckFailed(String),
    NotRunning,
}

impl std::fmt::Display for TunnelError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TunnelError::NotConfigured(msg) => write!(f, "Tunnel not configured: {}", msg),
            TunnelError::StartFailed(msg) => write!(f, "Failed to start tunnel: {}", msg),
            TunnelError::StopFailed(msg) => write!(f, "Failed to stop tunnel: {}", msg),
            TunnelError::HealthCheckFailed(msg) => write!(f, "Health check failed: {}", msg),
            TunnelError::NotRunning => write!(f, "Tunnel not running"),
        }
    }
}

impl std::error::Error for TunnelError {}

// ============================================================================
// Tunnel Manager
// ============================================================================

pub struct TunnelManager {
    tunnel: RwLock<Option<Box<dyn Tunnel>>>,
    public_url: RwLock<Option<String>>,
    running: RwLock<bool>,
}

impl TunnelManager {
    pub fn new() -> Self {
        Self {
            tunnel: RwLock::new(None),
            public_url: RwLock::new(None),
            running: RwLock::new(false),
        }
    }
    
    pub async fn create_tunnel(&self, provider: &str, config: TunnelConfig) -> Result<(), TunnelError> {
        let tunnel: Box<dyn Tunnel> = match provider {
            "none" => return Ok(()),
            "cloudflare" => {
                let token = config.cloudflare_token.ok_or_else(|| 
                    TunnelError::NotConfigured("Cloudflare token required".to_string()))?;
                Box::new(CloudflareTunnel::new(token))
            }
            "tailscale" => {
                Box::new(TailscaleTunnel::new(
                    config.tailscale_hostname.clone(),
                ))
            }
            "ngrok" => {
                Box::new(NgrokTunnel::new(
                    config.ngrok_token.clone(),
                    config.ngrok_domain.clone(),
                ))
            }
            "custom" => {
                let cmd = config.custom_command.ok_or_else(|| 
                    TunnelError::NotConfigured("Custom command required".to_string()))?;
                Box::new(CustomTunnel::new(cmd))
            }
            _ => return Err(TunnelError::NotConfigured(format!("Unknown provider: {}", provider))),
        };
        
        *self.tunnel.write().await = Some(tunnel);
        Ok(())
    }
    
    pub async fn start(&self, local_host: &str, local_port: u16) -> Result<String, TunnelError> {
        let tunnel = self.tunnel.read().await;
        let tunnel = tunnel.as_ref().ok_or(TunnelError::NotRunning)?;
        
        let url = tunnel.start(local_host, local_port).await?;
        *self.public_url.write().await = Some(url.clone());
        *self.running.write().await = true;
        
        tracing::info!("Tunnel started: {}", url);
        Ok(url)
    }
    
    pub async fn stop(&self) -> Result<(), TunnelError> {
        let tunnel = self.tunnel.read().await;
        if let Some(tunnel) = tunnel.as_ref() {
            tunnel.stop().await?;
        }
        *self.public_url.write().await = None;
        *self.running.write().await = false;
        
        tracing::info!("Tunnel stopped");
        Ok(())
    }
    
    pub async fn health_check(&self) -> bool {
        let tunnel = self.tunnel.read().await;
        if let Some(tunnel) = tunnel.as_ref() {
            tunnel.health_check().await
        } else {
            false
        }
    }
    
    pub fn public_url(&self) -> Option<String> {
        None
    }
    
    pub fn is_running(&self) -> bool {
        false
    }
}

impl Default for TunnelManager {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Custom Tunnel Implementation
// ============================================================================

pub struct CustomTunnel {
    command: String,
    running: RwLock<bool>,
    url: RwLock<Option<String>>,
}

impl CustomTunnel {
    pub fn new(command: String) -> Self {
        Self {
            command,
            running: RwLock::new(false),
            url: RwLock::new(None),
        }
    }
}

#[async_trait]
impl Tunnel for CustomTunnel {
    fn name(&self) -> &str {
        "custom"
    }
    
    async fn start(&self, local_host: &str, local_port: u16) -> Result<String, TunnelError> {
        let mut cmd = Command::new("sh");
        cmd.args(["-c", &self.command])
            .env("LOCAL_HOST", local_host)
            .env("LOCAL_PORT", local_port.to_string())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        
        let output = cmd.output().await
            .map_err(|e| TunnelError::StartFailed(e.to_string()))?;
        
        if !output.status.success() {
            return Err(TunnelError::StartFailed(
                String::from_utf8_lossy(&output.stderr).to_string()
            ));
        }
        
        let url = String::from_utf8_lossy(&output.stdout).trim().to_string();
        *self.url.write().await = Some(url.clone());
        *self.running.write().await = true;
        
        Ok(url)
    }
    
    async fn stop(&self) -> Result<(), TunnelError> {
        *self.running.write().await = false;
        *self.url.write().await = None;
        Ok(())
    }
    
    async fn health_check(&self) -> bool {
        *self.running.read().await
    }
    
    fn public_url(&self) -> Option<String> {
        None
    }
}

// ============================================================================
// Tunnel Configuration
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TunnelConfig {
    #[serde(default = "default_tunnel_provider")]
    pub provider: String,
    #[serde(default)]
    pub cloudflare_token: Option<String>,
    #[serde(default)]
    pub tailscale_hostname: Option<String>,
    #[serde(default)]
    pub ngrok_token: Option<String>,
    #[serde(default)]
    pub ngrok_domain: Option<String>,
    #[serde(default)]
    pub custom_command: Option<String>,
}

fn default_tunnel_provider() -> String {
    "none".to_string()
}

impl Default for TunnelConfig {
    fn default() -> Self {
        Self {
            provider: "none".to_string(),
            cloudflare_token: None,
            tailscale_hostname: None,
            ngrok_token: None,
            ngrok_domain: None,
            custom_command: None,
        }
    }
}

// ============================================================================
// Tauri Commands
// ============================================================================

lazy_static::lazy_static! {
    static ref TUNNEL_MANAGER: TunnelManager = TunnelManager::new();
}

#[tauri::command]
pub async fn start_tunnel(host: String, port: u16) -> Result<String, String> {
    TUNNEL_MANAGER.start(&host, port)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn stop_tunnel() -> Result<(), String> {
    TUNNEL_MANAGER.stop()
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn tunnel_health_check() -> bool {
    TUNNEL_MANAGER.health_check().await
}

#[tauri::command]
pub fn get_tunnel_url() -> Option<String> {
    TUNNEL_MANAGER.public_url()
}

#[tauri::command]
pub fn is_tunnel_running() -> bool {
    TUNNEL_MANAGER.is_running()
}

#[tauri::command]
pub async fn configure_tunnel(config: TunnelConfig) -> Result<(), String> {
    let provider = config.provider.clone();
    TUNNEL_MANAGER.create_tunnel(&provider, config)
        .await
        .map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_tunnel_config_default() {
        let config = TunnelConfig::default();
        assert_eq!(config.provider, "none");
    }
    
    #[test]
    fn test_tunnel_manager_creation() {
        let manager = TunnelManager::new();
        assert!(!manager.is_running());
        assert!(manager.public_url().is_none());
    }
}
