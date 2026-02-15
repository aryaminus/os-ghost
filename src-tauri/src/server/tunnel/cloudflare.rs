//! Cloudflare Tunnel Implementation

use async_trait::async_trait;
use std::process::Stdio;
use tokio::process::Command;
use tokio::sync::RwLock;

use super::{Tunnel, TunnelError};

pub struct CloudflareTunnel {
    token: String,
    running: RwLock<bool>,
    url: RwLock<Option<String>>,
    child: RwLock<Option<u32>>,
}

impl CloudflareTunnel {
    pub fn new(token: String) -> Self {
        Self {
            token,
            running: RwLock::new(false),
            url: RwLock::new(None),
            child: RwLock::new(None),
        }
    }
}

#[async_trait]
impl Tunnel for CloudflareTunnel {
    fn name(&self) -> &str {
        "cloudflare"
    }
    
    async fn start(&self, local_host: &str, local_port: u16) -> Result<String, TunnelError> {
        // Check if cloudflared is installed
        let check = Command::new("which")
            .arg("cloudflared")
            .output()
            .await;
        
        if check.is_err() || !check.unwrap().status.success() {
            return Err(TunnelError::StartFailed(
                "cloudflared not installed. Install from: https://developers.cloudflare.com/cloudflare-one/connections/connect-apps/install-and-setup/installation".to_string()
            ));
        }
        
        let mut cmd = Command::new("cloudflared");
        cmd.args([
            "tunnel",
            "--no-autoupdate",
            "run",
            "--token", &self.token,
            "--url", &format!("http://{}:{}", local_host, local_port),
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
        
        // In a real implementation, we'd spawn the process and parse the URL from stderr
        // For now, return a placeholder
        *self.running.write().await = true;
        
        // The real implementation would parse the public URL from cloudflared output
        // Format: "Your tunnel URL is https://xxxx.trycloudflare.com"
        let url = format!("https://{}.trycloudflare.com", 
            uuid::Uuid::new_v4().to_string().split('-').next().unwrap());
        
        *self.url.write().await = Some(url.clone());
        
        Ok(url)
    }
    
    async fn stop(&self) -> Result<(), TunnelError> {
        *self.running.write().await = false;
        *self.url.write().await = None;
        
        if let Some(mut child) = self.child.write().await.take() {
            let _ = child.kill().await;
        }
        
        Ok(())
    }
    
    async fn health_check(&self) -> bool {
        self.running.read().ok().map(|r| r).unwrap_or(false)
    }
    
    fn public_url(&self) -> Option<String> {
        self.url.read().ok().and_then(|u| u.clone())
    }
}
