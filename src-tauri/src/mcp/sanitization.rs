//! Tool Result Sanitization - Moltis-inspired output sanitization
//!
//! Enhances tool results before feeding back to the LLM:
//! - Strip base64 data URIs
//! - Remove long hex blobs
//! - Truncate oversized results (configurable limit)
//! - Configurable size limit (default 50KB)

use lazy_static::lazy_static;
use regex::Regex;

lazy_static! {
    static ref SANITIZER: ToolResultSanitizer = ToolResultSanitizer::new();
}

const DEFAULT_MAX_SIZE: usize = 50 * 1024; // 50KB

pub struct ToolResultSanitizer {
    base64_pattern: Regex,
    hex_pattern: Regex,
    data_uri_pattern: Regex,
    pub max_size: usize,
}

impl ToolResultSanitizer {
    pub fn new() -> Self {
        Self {
            base64_pattern: Regex::new(r"(?i)[a-z0-9+/]{50,}={0,2}").unwrap(),
            hex_pattern: Regex::new(r"(?i)0x[0-9a-f]{32,}").unwrap(),
            data_uri_pattern: Regex::new(r"data:[^;]+;base64,[^\s]+").unwrap(),
            max_size: DEFAULT_MAX_SIZE,
        }
    }

    pub fn with_max_size(mut self, max_size: usize) -> Self {
        self.max_size = max_size;
        self
    }

    /// Main sanitization entry point
    pub fn sanitize(&self, content: &str) -> String {
        let mut result = content.to_string();

        // Strip data URIs
        result = self.strip_data_uris(&result);

        // Strip long hex blobs
        result = self.strip_hex_blobs(&result);

        // Strip base64 strings (but not in data URIs which we already stripped)
        result = self.strip_base64(&result);

        // Truncate if too large
        result = self.truncate(&result);

        result
    }

    fn strip_data_uris(&self, content: &str) -> String {
        self.data_uri_pattern
            .replace_all(content, "[DATA_URI_REDACTED]")
            .to_string()
    }

    fn strip_hex_blobs(&self, content: &str) -> String {
        self.hex_pattern
            .replace_all(content, "[HEX_REDACTED]")
            .to_string()
    }

    fn strip_base64(&self, content: &str) -> String {
        // Only strip standalone base64 strings (not part of other content)
        let mut result = content.to_string();

        // Find base64-like strings and replace
        for mat in self.base64_pattern.find_iter(content) {
            // Check if it's a standalone string (surrounded by whitespace or boundaries)
            let start = mat.start();
            let end = mat.end();

            let prev_char = if start > 0 {
                content.chars().nth(start - 1)
            } else {
                None
            };

            let next_char = if end < content.len() {
                content.chars().nth(end)
            } else {
                None
            };

            let is_valid = match (prev_char, next_char) {
                (Some(c1), Some(c2)) => !c1.is_alphanumeric() && !c2.is_alphanumeric(),
                (Some(c), None) | (None, Some(c)) => !c.is_alphanumeric(),
                _ => false,
            };

            if is_valid {
                result = result[..start].to_string() + "[BASE64_REDACTED]" + &result[end..];
            }
        }

        result
    }

    fn truncate(&self, content: &str) -> String {
        if content.len() > self.max_size {
            format!(
                "{}\n\n... [OUTPUT TRUNCATED - {} bytes -> {} bytes]",
                &content[..self.max_size],
                content.len(),
                self.max_size
            )
        } else {
            content.to_string()
        }
    }
}

/// Sanitize tool result before returning to LLM
pub fn sanitize_tool_result(content: &str) -> String {
    SANITIZER.sanitize(content)
}

/// Sanitize with custom max size
pub fn sanitize_tool_result_with_limit(content: &str, max_size: usize) -> String {
    ToolResultSanitizer::new()
        .with_max_size(max_size)
        .sanitize(content)
}

// ============================================================================
// Tauri Commands
// ============================================================================

#[tauri::command]
pub fn sanitize_output(content: String) -> String {
    sanitize_tool_result(&content)
}

#[tauri::command]
pub fn sanitize_output_with_limit(content: String, max_bytes: usize) -> String {
    sanitize_tool_result_with_limit(&content, max_bytes)
}

#[tauri::command]
pub fn get_sanitizer_max_size() -> usize {
    SANITIZER.max_size
}
