//! Security Module
//!
//! Provides security features inspired by IronClaw:
//! - Leak detection for credential exfiltration
//! - HTTP endpoint allowlisting
//! - Tool output sanitization

pub mod leak_detector;
pub mod http_allowlist;

pub use leak_detector::{
    LeakMatch, LeakScanResult, LeakSeverity, LeakDetectionConfig, scan_for_leaks, 
    scan_request, scan_response, sanitize_content,
};
pub use http_allowlist::{
    HttpAllowlist, AllowlistCheckResult, AllowlistStats, check_url_allowed,
    allow_domain, block_domain, allow_path, get_allowed_domains, 
    enable_allowlist, is_allowlist_enabled,
};
