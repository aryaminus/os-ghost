//! Leak Detection System
//!
//! Scans requests and responses for potential credential exfiltration.
//! Reference: IronClaw security implementation
//!
//! Security Flow:
//! 1. Scan outgoing requests before tool execution
//! 2. Scan incoming responses before returning to tool/LLM
//! 3. Block or sanitize detected leaks

use lazy_static::lazy_static;
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

lazy_static! {
    static ref LEAK_DETECTOR: LeakDetector = LeakDetector::new();
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LeakMatch {
    pub pattern_name: String,
    pub matched_text: String,
    pub start: usize,
    pub end: usize,
    pub severity: LeakSeverity,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Ord, PartialOrd)]
#[serde(rename_all = "lowercase")]
pub enum LeakSeverity {
    Critical,
    High,
    Medium,
    Low,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LeakScanResult {
    pub blocked: bool,
    pub matches: Vec<LeakMatch>,
    pub sanitized_content: Option<String>,
}

pub struct LeakDetector {
    patterns: Vec<LeakPattern>,
}

struct LeakPattern {
    pattern: Regex,
    name: String,
    severity: LeakSeverity,
}

impl LeakDetector {
    pub fn new() -> Self {
        let patterns = vec![
            // AWS Keys
            LeakPattern {
                name: "aws_access_key".to_string(),
                pattern: Regex::new(r"(?i)(?:aws_access_key_id|aws_secret_access_key)\s*=\s*[\w/+]{16,}").unwrap(),
                severity: LeakSeverity::Critical,
            },
            // AWS Token
            LeakPattern {
                name: "aws_token".to_string(),
                pattern: Regex::new(r"(?i)aws[\w-]*token[:\s]+([A-Za-z0-9/+=]{40,})").unwrap(),
                severity: LeakSeverity::Critical,
            },
            // Generic API Key
            LeakPattern {
                name: "api_key".to_string(),
                pattern: Regex::new(r"(?i)(?:api[_-]?key|apikey)\s*[:=]\s*([a-zA-Z0-9_\-]{20,})").unwrap(),
                severity: LeakSeverity::High,
            },
            // OpenAI API Key
            LeakPattern {
                name: "openai_api_key".to_string(),
                pattern: Regex::new(r"sk-[A-Za-z0-9]{20,}").unwrap(),
                severity: LeakSeverity::Critical,
            },
            // Anthropic API Key
            LeakPattern {
                name: "anthropic_api_key".to_string(),
                pattern: Regex::new(r"(?i)sk-ant-[a-zA-Z0-9_-]{20,}").unwrap(),
                severity: LeakSeverity::Critical,
            },
            // Google API Key
            LeakPattern {
                name: "google_api_key".to_string(),
                pattern: Regex::new(r"AIza[0-9A-Za-z_-]{35}").unwrap(),
                severity: LeakSeverity::Critical,
            },
            // GitHub Token
            LeakPattern {
                name: "github_token".to_string(),
                pattern: Regex::new(r"(?i)gh[pousr]_[A-Za-z0-9_]{36,}").unwrap(),
                severity: LeakSeverity::Critical,
            },
            // Generic Bearer Token
            LeakPattern {
                name: "bearer_token".to_string(),
                pattern: Regex::new(r"(?i)bearer\s+[a-zA-Z0-9\-_.~+/]+={0,2}").unwrap(),
                severity: LeakSeverity::High,
            },
            // Basic Auth
            LeakPattern {
                name: "basic_auth".to_string(),
                pattern: Regex::new(r"(?i)authorization\s*:\s*basic\s+[A-Za-z0-9+/]+={0,2}").unwrap(),
                severity: LeakSeverity::High,
            },
            // Private Key
            LeakPattern {
                name: "private_key".to_string(),
                pattern: Regex::new(r"-----BEGIN\s+(?:RSA\s+|EC\s+|DSA\s+|OPENSSH\s+)PRIVATE\s+KEY-----").unwrap(),
                severity: LeakSeverity::Critical,
            },
            // GitHub Personal Access Token
            LeakPattern {
                name: "github_pat".to_string(),
                pattern: Regex::new(r"ghp_[A-Za-z0-9]{36}").unwrap(),
                severity: LeakSeverity::Critical,
            },
            // Slack Token
            LeakPattern {
                name: "slack_token".to_string(),
                pattern: Regex::new(r"xox[baprs]-[0-9]{10,13}-[0-9]{10,13}[a-zA-Z0-9-]*").unwrap(),
                severity: LeakSeverity::High,
            },
            // Discord Token
            LeakPattern {
                name: "discord_token".to_string(),
                pattern: Regex::new(r"[MN][A-Za-z\d]{23,}\.[\w-]{6}\.[\w-]{27}").unwrap(),
                severity: LeakSeverity::High,
            },
            // Database Connection String
            LeakPattern {
                name: "database_url".to_string(),
                pattern: Regex::new(r"(?i)(?:mysql|postgres|postgresql|mongodb|redis)://[^\s]+").unwrap(),
                severity: LeakSeverity::Critical,
            },
            // JWT Token
            LeakPattern {
                name: "jwt_token".to_string(),
                pattern: Regex::new(r"eyJ[A-Za-z0-9_-]+\.eyJ[A-Za-z0-9_-]+\.[A-Za-z0-9_-]+").unwrap(),
                severity: LeakSeverity::High,
            },
            // Generic Secret
            LeakPattern {
                name: "generic_secret".to_string(),
                pattern: Regex::new(r"(?i)(?:secret|password|passwd|pwd|token|auth)[_-]?(?:key|token|secret)?\s*[:=]\s*[^\s]+").unwrap(),
                severity: LeakSeverity::Medium,
            },
            // SSH Private Key
            LeakPattern {
                name: "ssh_key".to_string(),
                pattern: Regex::new(r"-----BEGIN\s+OPENSSH\s+PRIVATE\s+KEY-----").unwrap(),
                severity: LeakSeverity::Critical,
            },
            // Stripe API Key
            LeakPattern {
                name: "stripe_key".to_string(),
                pattern: Regex::new(r"(?:sk|pk)_(?:live|test)_[a-zA-Z0-9]{24,}").unwrap(),
                severity: LeakSeverity::Critical,
            },
            // SendGrid API Key
            LeakPattern {
                name: "sendgrid_key".to_string(),
                pattern: Regex::new(r"SG\.[a-zA-Z0-9_-]{22}\.[a-zA-Z0-9_-]{43}").unwrap(),
                severity: LeakSeverity::Critical,
            },
        ];

        Self { patterns }
    }

    pub fn scan(&self, content: &str) -> LeakScanResult {
        let mut matches = Vec::new();

        for pattern in &self.patterns {
            for mat in pattern.pattern.find_iter(content) {
                matches.push(LeakMatch {
                    pattern_name: pattern.name.clone(),
                    matched_text: mat.as_str().to_string(),
                    start: mat.start(),
                    end: mat.end(),
                    severity: pattern.severity,
                });
            }
        }

        // Sort by severity
        matches.sort_by(|a, b| b.severity.cmp(&a.severity));

        // Determine if blocked (critical or high severity)
        let blocked = matches
            .iter()
            .any(|m| m.severity == LeakSeverity::Critical || m.severity == LeakSeverity::High);

        // Generate sanitized content
        let sanitized_content = if !matches.is_empty() {
            Some(self.sanitize(content, &matches))
        } else {
            None
        };

        LeakScanResult {
            blocked,
            matches,
            sanitized_content,
        }
    }

    pub fn sanitize(&self, content: &str, matches: &[LeakMatch]) -> String {
        let mut result = content.to_string();

        // Process in reverse order to maintain correct positions
        for m in matches.iter().rev() {
            let replacement = match m.severity {
                LeakSeverity::Critical => "[REDACTED]".to_string(),
                LeakSeverity::High => "[REDACTED]".to_string(),
                LeakSeverity::Medium => "[MASKED]".to_string(),
                LeakSeverity::Low => m.matched_text.chars().map(|_| '*').collect(),
            };

            if m.start < result.len() && m.end <= result.len() {
                result.replace_range(m.start..m.end, &replacement);
            }
        }

        result
    }

    pub fn scan_request(
        &self,
        url: &str,
        headers: Option<&HashMap<String, String>>,
        body: Option<&str>,
    ) -> LeakScanResult {
        let mut content = url.to_string();

        if let Some(h) = headers {
            for (k, v) in h {
                content.push_str(&format!("\n{}: {}", k, v));
            }
        }

        if let Some(b) = body {
            content.push_str(b);
        }

        self.scan(&content)
    }

    pub fn scan_response(
        &self,
        status: u16,
        headers: Option<&HashMap<String, String>>,
        body: Option<&str>,
    ) -> LeakScanResult {
        let mut content = format!("status: {}", status);

        if let Some(h) = headers {
            for (k, v) in h {
                content.push_str(&format!("\n{}: {}", k, v));
            }
        }

        if let Some(b) = body {
            // Limit body scan size to prevent DoS
            let body_to_scan = if b.len() > 10000 { &b[..10000] } else { b };
            content.push_str(body_to_scan);
        }

        self.scan(&content)
    }
}

impl Default for LeakDetector {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Configuration
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LeakDetectionConfig {
    pub enabled: bool,
    pub block_on_critical: bool,
    pub block_on_high: bool,
    pub log_matches: bool,
    pub custom_patterns: Vec<CustomPattern>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CustomPattern {
    pub name: String,
    pub pattern: String,
    pub severity: LeakSeverity,
    pub replacement: Option<String>,
}

impl Default for LeakDetectionConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            block_on_critical: true,
            block_on_high: true,
            log_matches: true,
            custom_patterns: vec![],
        }
    }
}

impl LeakDetectionConfig {
    pub fn should_block(&self, severity: LeakSeverity) -> bool {
        match severity {
            LeakSeverity::Critical => self.block_on_critical,
            LeakSeverity::High => self.block_on_high,
            LeakSeverity::Medium | LeakSeverity::Low => false,
        }
    }
}

// ============================================================================
// Global Functions
// ============================================================================

pub fn scan_for_leaks(content: &str) -> LeakScanResult {
    LEAK_DETECTOR.scan(content)
}

pub fn scan_request(
    url: &str,
    headers: Option<&HashMap<String, String>>,
    body: Option<&str>,
) -> LeakScanResult {
    LEAK_DETECTOR.scan_request(url, headers, body)
}

pub fn scan_response(
    status: u16,
    headers: Option<&HashMap<String, String>>,
    body: Option<&str>,
) -> LeakScanResult {
    LEAK_DETECTOR.scan_response(status, headers, body)
}

pub fn sanitize_content(content: &str) -> String {
    let result = LEAK_DETECTOR.scan(content);
    result
        .sanitized_content
        .unwrap_or_else(|| content.to_string())
}

// ============================================================================
// Tauri Commands
// ============================================================================

#[tauri::command]
pub fn detect_leaks(content: String) -> LeakScanResult {
    LEAK_DETECTOR.scan(&content)
}

#[tauri::command]
pub fn sanitize_text(content: String) -> String {
    sanitize_content(&content)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_openai_key_detection() {
        let content = "API Key: sk-1234567890abcdefghijklmnopqrstuvwxyz";
        let result = scan_for_leaks(content);
        assert!(result.blocked);
        assert!(!result.matches.is_empty());
    }

    #[test]
    fn test_github_token_detection() {
        let content = "Token: ghp_1234567890abcdefghijklmnopqrstuvwxyzAB";
        let result = scan_for_leaks(content);
        assert!(result.blocked);
    }

    #[test]
    fn test_sanitization() {
        let content = "My API key is sk-1234567890abcdefghijklmnop";
        let result = scan_for_leaks(content);
        assert!(result.sanitized_content.is_some());

        let sanitized = result.sanitized_content.unwrap();
        assert!(!sanitized.contains("sk-1234567890"));
    }

    #[test]
    fn test_safe_content() {
        let content = "Hello, this is a normal message with no secrets.";
        let result = scan_for_leaks(content);
        assert!(!result.blocked);
        assert!(result.matches.is_empty());
    }
}
