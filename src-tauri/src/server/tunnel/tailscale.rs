//! Tailscale Tunnel Implementation

use async_trait::async_trait;
use tokio::process::Command;
use tokio::sync::RwLock;

use super::{Tunnel, TunnelError};

pub struct TailscaleTunnel {
    hostname: Option<String>,
    running: RwLock<bool>,
    url: RwLock<Option<String>>,
}

impl TailscaleTunnel {
    pub fn new(hostname: Option<String>) -> Self {
        Self {
            hostname,
            running: RwLock::new(false),
            url: RwLock::new(None),
        }
    }
}

#[async_trait]
impl Tunnel for TailscaleTunnel {
    fn name(&self) -> &str {
        "tailscale"
    }
    
    async fn start(&self, local_host: &str, local_port: u16) -> Result<String, TunnelError> {
        // Check if tailscale is installed
        let check = Command::new("which")
            .arg("tailscale")
            .output()
            .await;
        
        if check.is_err() || !check.unwrap().status.success() {
            return Err(TunnelError::StartFailed(
                "Tailscale not installed. Install from: https://tailscale.com/download".to_string()
            ));
        }
        
        let url = if let Some(ref hostname) = self.hostname {
            format!("https://{}", hostname)
        } else {
            // Return the Tailscale IP-based URL
            // In production, you'd want to query the Tailscale API for your node's hostname
            "https://<your-tailscale-hostname>.tail-scale.ts.net".to_string()
        };
        
        *self.running.write().await = true;
        *self.url.write().await = Some(url.clone());
        
        tracing::info!("Tailscale tunnel configured for {}:{}", local_host, local_port);
        
        Ok(url)
    }
    
    async fn stop(&self) -> Result<(), TunnelError> {
        *self.running.write().await = false;
        *self.url.write().await = None;
        Ok(())
    }
    
    async fn health_check(&self) -> bool {
        let output = Command::new("tailscale")
            .args(["status", "--json"])
            .output()
            .await;
        
        match output {
            Ok(out) if out.status.success() => {
                let status: Option<serde_json::Value> = serde_json::from_slice(&out.stdout).ok();
                status
                    .and_then(|s| s.get("BackendState").cloned())
                    .and_then(|v| v.as_str().map(|s| s.to_string()))
                    .map(|s| s == "Running")
                    .unwrap_or(false)
            }
            _ => false,
        }
    }
    
    fn public_url(&self) -> Option<String> {
        None
    }
}
