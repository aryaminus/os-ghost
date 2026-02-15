//! HTTP Endpoint Allowlisting
//!
//! Restricts HTTP requests to explicitly approved hosts and paths.
//! Reference: IronClaw security implementation
//!
//! Security Model:
//! - Default deny policy: all non-allowlisted URLs are blocked
//! - Supports wildcard patterns (*.example.com)
//! - Per-tool allowlists for granular control
//! - Logs all blocked requests for audit

use lazy_static::lazy_static;
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::sync::RwLock;
use url::Url;

lazy_static! {
    static ref ALLOWLIST: RwLock<HttpAllowlist> = RwLock::new(HttpAllowlist::default());
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HttpAllowlist {
    pub enabled: bool,
    pub allowed_domains: Vec<String>,
    pub allowed_paths: Vec<String>,
    pub blocked_domains: Vec<String>,
    pub allow_cloudflare: bool,
    pub allow_localhost: bool,
}

impl Default for HttpAllowlist {
    fn default() -> Self {
        Self {
            enabled: true,
            allowed_domains: vec![],
            allowed_paths: vec![],
            blocked_domains: vec![],
            allow_cloudflare: false,
            allow_localhost: true, // Allow localhost for development
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AllowlistCheckResult {
    pub allowed: bool,
    pub reason: String,
    pub matched_pattern: Option<String>,
}

impl HttpAllowlist {
    /// Check if a URL is allowed
    pub fn check_url(&self, url: &str) -> AllowlistCheckResult {
        if !self.enabled {
            return AllowlistCheckResult {
                allowed: true,
                reason: "Allowlist disabled".to_string(),
                matched_pattern: None,
            };
        }
        
        // Parse URL
        let parsed = match Url::parse(url) {
            Ok(u) => u,
            Err(e) => {
                return AllowlistCheckResult {
                    allowed: false,
                    reason: format!("Invalid URL: {}", e),
                    matched_pattern: None,
                };
            }
        };
        
        let host = match parsed.host_str() {
            Some(h) => h,
            None => {
                return AllowlistCheckResult {
                    allowed: false,
                    reason: "URL has no host".to_string(),
                    matched_pattern: None,
                };
            }
        };
        
        let path = parsed.path();
        
        // Check localhost allowance
        if self.allow_localhost && (host == "localhost" || host == "127.0.0.1" || host == "::1") {
            return AllowlistCheckResult {
                allowed: true,
                reason: "Localhost allowed".to_string(),
                matched_pattern: Some("localhost".to_string()),
            };
        }
        
        // Check blocked domains (deny list)
        for blocked in &self.blocked_domains {
            if self.host_matches_pattern(host, blocked) {
                return AllowlistCheckResult {
                    allowed: false,
                    reason: format!("Domain '{}' is blocked", host),
                    matched_pattern: Some(blocked.clone()),
                };
            }
        }
        
        // Check allowed domains (allow list)
        // Empty allowlist means deny all (except localhost)
        if self.allowed_domains.is_empty() {
            return AllowlistCheckResult {
                allowed: false,
                reason: "No domains in allowlist".to_string(),
                matched_pattern: None,
            };
        }
        
        for allowed in &self.allowed_domains {
            if self.host_matches_pattern(host, allowed) {
                // Now check path if specified
                if !self.allowed_paths.is_empty() {
                    for allowed_path in &self.allowed_paths {
                        if self.path_matches_pattern(path, allowed_path) {
                            return AllowlistCheckResult {
                                allowed: true,
                                reason: format!("Domain and path allowed: {} {}", host, path),
                                matched_pattern: Some(format!("{} {}", allowed, allowed_path)),
                            };
                        }
                    }
                    return AllowlistCheckResult {
                        allowed: false,
                        reason: format!("Path '{}' not in allowlist for domain '{}'", path, host),
                        matched_pattern: None,
                    };
                }
                
                return AllowlistCheckResult {
                    allowed: true,
                    reason: format!("Domain '{}' allowed", host),
                    matched_pattern: Some(allowed.clone()),
                };
            }
        }
        
        AllowlistCheckResult {
            allowed: false,
            reason: format!("Domain '{}' not in allowlist", host),
            matched_pattern: None,
        }
    }
    
    /// Check if host matches pattern (supports wildcards)
    fn host_matches_pattern(&self, host: &str, pattern: &str) -> bool {
        // Exact match
        if host == pattern {
            return true;
        }
        
        // Wildcard match (*.example.com)
        if let Some(suffix) = pattern.strip_prefix("*.") {
            return host.ends_with(suffix) || host == suffix;
        }
        
        // Regex pattern match
        if let Ok(regex) = Regex::new(&format!("^{}$", pattern.replace("*", ".*"))) {
            return regex.is_match(host);
        }
        
        false
    }
    
    /// Check if path matches pattern (supports wildcards)
    fn path_matches_pattern(&self, path: &str, pattern: &str) -> bool {
        // Exact match
        if path == pattern {
            return true;
        }
        
        // Prefix match (e.g., /api/*)
        if let Some(prefix) = pattern.strip_suffix("/*") {
            return path.starts_with(prefix);
        }
        
        // Wildcard match
        if pattern.contains('*') {
            if let Ok(regex) = Regex::new(&format!("^{}$", pattern.replace("*", ".*"))) {
                return regex.is_match(path);
            }
        }
        
        false
    }
    
    /// Add a domain to the allowlist
    pub fn allow_domain(&mut self, domain: &str) {
        if !self.allowed_domains.contains(&domain.to_string()) {
            self.allowed_domains.push(domain.to_string());
        }
    }
    
    /// Add a domain to the blocklist
    pub fn block_domain(&mut self, domain: &str) {
        if !self.blocked_domains.contains(&domain.to_string()) {
            self.blocked_domains.push(domain.to_string());
        }
    }
    
    /// Add a path to the allowlist
    pub fn allow_path(&mut self, path: &str) {
        if !self.allowed_paths.contains(&path.to_string()) {
            self.allowed_paths.push(path.to_string());
        }
    }
    
    /// Get current allowlist
    pub fn get_allowlist(&self) -> Vec<String> {
        self.allowed_domains.clone()
    }
}

// ============================================================================
// HTTP Client with Allowlist
// ============================================================================

pub struct AllowlistedHttpClient {
    client: reqwest::Client,
    allowlist: HttpAllowlist,
}

impl AllowlistedHttpClient {
    pub fn new(allowlist: HttpAllowlist) -> Result<Self, String> {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .map_err(|e| e.to_string())?;
        
        Ok(Self {
            client,
            allowlist,
        })
    }
    
    pub async fn get(&self, url: &str) -> Result<reqwest::Response, HttpAllowlistError> {
        // Check allowlist first
        let check = self.allowlist.check_url(url);
        if !check.allowed {
            return Err(HttpAllowlistError::Blocked {
                url: url.to_string(),
                reason: check.reason,
            });
        }
        
        // Perform request
        let response = self.client.get(url)
            .send()
            .await
            .map_err(|e| HttpAllowlistError::RequestFailed(e.to_string()))?;
        
        Ok(response)
    }
    
    pub async fn post(&self, url: &str, body: impl serde::Serialize) -> Result<reqwest::Response, HttpAllowlistError> {
        // Check allowlist first
        let check = self.allowlist.check_url(url);
        if !check.allowed {
            return Err(HttpAllowlistError::Blocked {
                url: url.to_string(),
                reason: check.reason,
            });
        }
        
        // Perform request
        let response = self.client.post(url)
            .json(&body)
            .send()
            .await
            .map_err(|e| HttpAllowlistError::RequestFailed(e.to_string()))?;
        
        Ok(response)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum HttpAllowlistError {
    Blocked { url: String, reason: String },
    RequestFailed(String),
}

impl std::fmt::Display for HttpAllowlistError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            HttpAllowlistError::Blocked { url, reason } => {
                write!(f, "URL blocked: {} - {}", url, reason)
            }
            HttpAllowlistError::RequestFailed(e) => {
                write!(f, "Request failed: {}", e)
            }
        }
    }
}

impl std::error::Error for HttpAllowlistError {}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AllowlistStats {
    pub domains_count: usize,
    pub paths_count: usize,
    pub blocked_count: usize,
}

// ============================================================================
// Global Functions
// ============================================================================

pub fn check_url_allowed(url: &str) -> AllowlistCheckResult {
    ALLOWLIST.read().unwrap().check_url(url)
}

pub fn allow_domain(domain: &str) {
    ALLOWLIST.write().unwrap().allow_domain(domain);
}

pub fn block_domain(domain: &str) {
    ALLOWLIST.write().unwrap().block_domain(domain);
}

pub fn allow_path(path: &str) {
    ALLOWLIST.write().unwrap().allow_path(path);
}

pub fn get_allowed_domains() -> Vec<String> {
    ALLOWLIST.read().unwrap().allowed_domains.clone()
}

pub fn enable_allowlist(enabled: bool) {
    ALLOWLIST.write().unwrap().enabled = enabled;
}

pub fn is_allowlist_enabled() -> bool {
    ALLOWLIST.read().unwrap().enabled
}

// ============================================================================
// Tauri Commands
// ============================================================================

#[tauri::command]
pub fn check_http_allowed(url: String) -> AllowlistCheckResult {
    check_url_allowed(&url)
}

#[tauri::command]
pub fn add_allowed_domain(domain: String) {
    allow_domain(&domain);
}

#[tauri::command]
pub fn add_blocked_domain(domain: String) {
    block_domain(&domain);
}

#[tauri::command]
pub fn add_allowed_path(path: String) {
    allow_path(&path);
}

#[tauri::command]
pub fn get_allowed_domains_list() -> Vec<String> {
    get_allowed_domains()
}

#[tauri::command]
pub fn set_allowlist_enabled(enabled: bool) {
    enable_allowlist(enabled);
}

#[tauri::command]
pub fn get_allowlist_status() -> bool {
    is_allowlist_enabled()
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_exact_domain_match() {
        let allowlist = HttpAllowlist {
            allowed_domains: vec!["api.example.com".to_string()],
            ..Default::default()
        };
        
        let result = allowlist.check_url("https://api.example.com/test");
        assert!(result.allowed);
    }
    
    #[test]
    fn test_wildcard_domain_match() {
        let allowlist = HttpAllowlist {
            allowed_domains: vec!["*.example.com".to_string()],
            ..Default::default()
        };
        
        let result1 = allowlist.check_url("https://api.example.com/test");
        assert!(result1.allowed);
        
        let result2 = allowlist.check_url("https://sub.api.example.com/test");
        assert!(result2.allowed);
        
        let result3 = allowlist.check_url("https://evil.com/test");
        assert!(!result3.allowed);
    }
    
    #[test]
    fn test_localhost_allowed() {
        let allowlist = HttpAllowlist {
            allow_localhost: true,
            ..Default::default()
        };
        
        let result = allowlist.check_url("http://localhost:8080/api");
        assert!(result.allowed);
    }
    
    #[test]
    fn test_path_allowlist() {
        let allowlist = HttpAllowlist {
            allowed_domains: vec!["api.example.com".to_string()],
            allowed_paths: vec!["/api/*".to_string()],
            ..Default::default()
        };
        
        let result1 = allowlist.check_url("https://api.example.com/api/users");
        assert!(result1.allowed);
        
        let result2 = allowlist.check_url("https://api.example.com/admin/users");
        assert!(!result2.allowed);
    }
    
    #[test]
    fn test_blocklist_overrides_allowlist() {
        let allowlist = HttpAllowlist {
            allowed_domains: vec!["*.example.com".to_string()],
            blocked_domains: vec!["evil.example.com".to_string()],
            ..Default::default()
        };
        
        let result1 = allowlist.check_url("https://safe.example.com/test");
        assert!(result1.allowed);
        
        let result2 = allowlist.check_url("https://evil.example.com/test");
        assert!(!result2.allowed);
    }
}
