//! Verifier Agent - Solution validation
//! Validates puzzle solutions using URL patterns and content matching

use super::traits::{Agent, AgentContext, AgentError, AgentOutput, AgentResult, NextAction};
use async_trait::async_trait;
use regex::Regex;
use std::collections::HashMap;

/// Verifier agent for puzzle solution validation
pub struct VerifierAgent {
    /// Cache of compiled regex patterns
    pattern_cache: std::sync::RwLock<HashMap<String, Regex>>,
}

impl VerifierAgent {
    pub fn new() -> Self {
        Self {
            pattern_cache: std::sync::RwLock::new(HashMap::new()),
        }
    }

    /// Validate URL against pattern (case-insensitive)
    fn validate_url(&self, url: &str, pattern: &str) -> AgentResult<bool> {
        // Make pattern case-insensitive by prepending (?i) if not already
        let case_insensitive_pattern = if pattern.starts_with("(?i)") {
            pattern.to_string()
        } else {
            format!("(?i){}", pattern)
        };

        // Check cache first - handle poisoning gracefully
        {
            let cache = match self.pattern_cache.read() {
                Ok(guard) => guard,
                Err(poisoned) => {
                    tracing::warn!("Pattern cache read lock poisoned, recovering");
                    poisoned.into_inner()
                }
            };
            if let Some(regex) = cache.get(&case_insensitive_pattern) {
                return Ok(regex.is_match(url));
            }
        }

        // Compile and cache the pattern
        let regex = Regex::new(&case_insensitive_pattern)
            .map_err(|e| AgentError::ConfigError(format!("Invalid pattern: {}", e)))?;

        let matches = regex.is_match(url);

        // Cache for future use - handle poisoning gracefully
        {
            let mut cache = match self.pattern_cache.write() {
                Ok(guard) => guard,
                Err(poisoned) => {
                    tracing::warn!("Pattern cache write lock poisoned, recovering");
                    poisoned.into_inner()
                }
            };
            cache.insert(case_insensitive_pattern, regex);
        }

        Ok(matches)
    }

    /// Check if page content contains expected keywords
    /// Uses cached regex when possible for performance
    fn validate_content(&self, content: &str, keywords: &[&str]) -> bool {
        // Build keywords pattern
        let filtered_keywords: Vec<&str> = keywords
            .iter()
            .map(|k| k.trim())
            .filter(|k| !k.is_empty())
            .collect();

        if filtered_keywords.is_empty() {
            return false;
        }

        // Sort keywords for consistent cache key and regex pattern
        let mut sorted_keywords = filtered_keywords.clone();
        sorted_keywords.sort();
        let keyword_pattern = sorted_keywords.join("|");
        let cache_key = format!("content_(?i)\\b({})\\b", keyword_pattern);

        // Check cache first
        {
            let cache = match self.pattern_cache.read() {
                Ok(guard) => guard,
                Err(poisoned) => poisoned.into_inner(),
            };
            if let Some(regex) = cache.get(&cache_key) {
                return regex.is_match(content);
            }
        }

        // Build and cache the pattern
        let pattern = format!("(?i)\\b({})\\b", keyword_pattern);
        match Regex::new(&pattern) {
            Ok(re) => {
                let matches = re.is_match(content);
                // Cache for future use
                {
                    let mut cache = match self.pattern_cache.write() {
                        Ok(guard) => guard,
                        Err(poisoned) => poisoned.into_inner(),
                    };
                    cache.insert(cache_key, re);
                }
                matches
            }
            Err(_) => {
                // Fallback to simple contains if regex fails (unlikely)
                let content_lower = content.to_lowercase();
                keywords
                    .iter()
                    .any(|kw| content_lower.contains(&kw.to_lowercase()))
            }
        }
    }
}

impl Default for VerifierAgent {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Agent for VerifierAgent {
    fn name(&self) -> &str {
        "Verifier"
    }

    fn description(&self) -> &str {
        "Validates puzzle solutions by checking URL patterns and page content"
    }

    async fn process(&self, context: &AgentContext) -> AgentResult<AgentOutput> {
        // Skip if no URL or pattern
        if context.current_url.is_empty() || context.target_pattern.is_empty() {
            return Ok(AgentOutput {
                agent_name: self.name().to_string(),
                result: "Nothing to verify".to_string(),
                confidence: 0.0,
                data: HashMap::new(),
                next_action: Some(NextAction::Continue),
            });
        }

        // 1. Validate URL pattern
        let url_matches = self.validate_url(&context.current_url, &context.target_pattern)?;

        let mut data = HashMap::new();
        data.insert(
            "url_matches".to_string(),
            serde_json::Value::Bool(url_matches),
        );
        data.insert(
            "pattern".to_string(),
            serde_json::Value::String(context.target_pattern.clone()),
        );

        if url_matches {
            return Ok(AgentOutput {
                agent_name: self.name().to_string(),
                result: "PUZZLE SOLVED! Pattern matched!".to_string(),
                confidence: 1.0,
                data,
                next_action: Some(NextAction::PuzzleSolved),
            });
        }

        // 2. Validate Page Content (Passive Verification)
        if let Some(target_desc) = context.metadata.get("target_description") {
            if !context.page_content.is_empty() {
                // Extract keywords from target description (simple heuristic)
                let keywords: Vec<&str> = target_desc
                    .split_whitespace()
                    .filter(|w| w.len() > 3) // Filter short words
                    .collect();

                if !keywords.is_empty() {
                    let content_matches = self.validate_content(&context.page_content, &keywords);

                    data.insert(
                        "content_matches".to_string(),
                        serde_json::Value::Bool(content_matches),
                    );

                    if content_matches {
                        // High confidence for content match, but slightly less than explicit URL match
                        
                        // Add tool call to highlight the matched keywords
                        let tool_call = serde_json::json!({
                            "tool": "browser.highlight_text",
                            "arguments": {
                                "text": keywords.join(" ") // Simple join for now
                            }
                        });
                        data.insert("tool_call".to_string(), tool_call);

                        return Ok(AgentOutput {
                            agent_name: self.name().to_string(),
                            result: format!(
                                "PUZZLE SOLVED! Content matched target: {}",
                                target_desc
                            ),
                            confidence: 0.9,
                            data,
                            next_action: Some(NextAction::PuzzleSolved),
                        });
                    }
                }
            }
        }

        // Calculate partial pattern match for debugging/feedback
        let url_lower = context.current_url.to_lowercase();
        let pattern_parts: Vec<&str> = context
            .target_pattern
            .split('|')
            .flat_map(|p| p.split(&['(', ')', '[', ']'][..]))
            .filter(|p| p.len() > 2)
            .collect();

        let partial_matches = pattern_parts
            .iter()
            .filter(|p| url_lower.contains(&p.to_lowercase()))
            .count();

        let confidence = if pattern_parts.is_empty() {
            0.0
        } else {
            partial_matches as f32 / pattern_parts.len() as f32
        };

        data.insert(
            "partial_confidence".to_string(),
            serde_json::Number::from_f64(confidence as f64)
                .map(serde_json::Value::Number)
                .unwrap_or(serde_json::Value::Null),
        );

        Ok(AgentOutput {
            agent_name: self.name().to_string(),
            result: format!(
                "Pattern not matched. Partial confidence: {:.0}%",
                confidence * 100.0
            ),
            confidence,
            data,
            next_action: Some(NextAction::Continue),
        })
    }

    fn can_handle(&self, context: &AgentContext) -> bool {
        !context.current_url.is_empty() && !context.target_pattern.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_url_validation() {
        let verifier = VerifierAgent::new();

        // Test simple pattern
        assert!(verifier
            .validate_url(
                "https://en.wikipedia.org/wiki/Alan_Turing",
                "(turing|bletchley)"
            )
            .unwrap());

        // Test non-match
        assert!(!verifier
            .validate_url("https://example.com", "(turing|bletchley)")
            .unwrap());
    }
}
