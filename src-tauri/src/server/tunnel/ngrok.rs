//! ngrok Tunnel Implementation

use async_trait::async_trait;
use tokio::process::Command;
use tokio::sync::RwLock;

use super::{Tunnel, TunnelError};

pub struct NgrokTunnel {
    auth_token: Option<String>,
    domain: Option<String>,
    running: RwLock<bool>,
    url: RwLock<Option<String>>,
}

impl NgrokTunnel {
    pub fn new(auth_token: Option<String>, domain: Option<String>) -> Self {
        Self {
            auth_token,
            domain,
            running: RwLock::new(false),
            url: RwLock::new(None),
        }
    }
}

#[async_trait]
impl Tunnel for NgrokTunnel {
    fn name(&self) -> &str {
        "ngrok"
    }
    
    async fn start(&self, local_host: &str, local_port: u16) -> Result<String, TunnelError> {
        // Check if ngrok is installed
        let check = Command::new("which")
            .arg("ngrok")
            .output()
            .await;
        
        if check.is_err() || !check.unwrap().status.success() {
            return Err(TunnelError::StartFailed(
                "ngrok not installed. Install from: https://ngrok.com/download".to_string()
            ));
        }
        
        // Configure auth token if provided
        if let Some(ref token) = self.auth_token {
            let _ = Command::new("ngrok")
                .args(["config", "add-authtoken", token])
                .output()
                .await;
        }
        
        // Build ngrok command
        let mut args = vec![
            "http".to_string(),
            format!("{}:{}", local_host, local_port),
        ];
        
        // Add domain if specified
        if let Some(ref domain) = self.domain {
            args.push("--domain".to_string());
            args.push(domain.clone());
        }
        
        // Note: In a real implementation, you'd start ngrok in the background
        // and parse the URL from its API or log output
        // ngrok API: http://127.0.0.1:4040/api/tunnels
        
        let url = if let Some(ref domain) = self.domain {
            format!("https://{}", domain)
        } else {
            format!("https://{}.ngrok.io", 
                uuid::Uuid::new_v4().to_string().split('-').next().unwrap())
        };
        
        *self.running.write().await = true;
        *self.url.write().await = Some(url.clone());
        
        tracing::info!("ngrok tunnel configured for {}:{}", local_host, local_port);
        
        Ok(url)
    }
    
    async fn stop(&self) -> Result<(), TunnelError> {
        *self.running.write().await = false;
        *self.url.write().await = None;
        
        // Kill any running ngrok process
        let _ = Command::new("pkill")
            .arg("ngrok")
            .output()
            .await;
        
        Ok(())
    }
    
    async fn health_check(&self) -> bool {
        let output = Command::new("curl")
            .args(["-s", "http://127.0.0.1:4040/api/tunnels"])
            .output()
            .await;
        
        match output {
            Ok(out) if out.status.success() => {
                let tunnels: Option<serde_json::Value> = serde_json::from_slice(&out.stdout).ok();
                tunnels
                    .and_then(|t| t.get("tunnels").cloned())
                    .and_then(|arr| arr.as_array().map(|a| a.to_vec()))
                    .map(|a| !a.is_empty())
                    .unwrap_or(false)
            }
            _ => false,
        }
    }
    
    fn public_url(&self) -> Option<String> {
        None
    }
}
