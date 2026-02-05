//! Smart Suggestion Engine
//!
//! Generates proactive suggestions based on:
//! - App context (which app user is in)
//! - Idle time (user is not actively working)
//! - Calendar events (upcoming meetings)
//! - Email backlog (unread messages)
//! - Recent activity patterns
//!
//! Respects privacy by only using metadata, never content.

use crate::config::privacy::{AutonomyLevel, PrivacySettings};
use crate::monitoring::app_context::{AppCategory, AppContext, AppSwitchEvent};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// A smart suggestion for the user
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SmartSuggestion {
    /// Unique suggestion ID
    pub id: String,
    /// What triggered this suggestion
    pub trigger: SuggestionTrigger,
    /// Human-readable message
    pub message: String,
    /// Confidence score (0.0 - 1.0)
    pub confidence: f32,
    /// Priority level
    pub priority: SuggestionPriority,
    /// Whether this requires user action
    pub requires_action: bool,
    /// Suggested action (if any)
    pub suggested_action: Option<String>,
    /// When suggestion was generated
    pub timestamp: u64,
    /// Whether user has seen this
    pub seen: bool,
    /// User feedback (None, Some(true)=helpful, Some(false)=not helpful)
    pub feedback: Option<bool>,
}

/// What triggered the suggestion
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SuggestionTrigger {
    /// Calendar event coming up
    CalendarEvent {
        event_title: String,
        minutes_until: u32,
    },
    /// Email backlog detected
    EmailBacklog {
        unread_count: u32,
        important_count: u32,
    },
    /// Context switch detected
    ContextSwitch { from: AppCategory, to: AppCategory },
    /// User has been idle
    IdleTime { duration_secs: u64 },
    /// New file created
    FileCreated { filename: String, location: String },
    /// End of workday approaching
    EndOfDay { minutes_until_eod: u32 },
    /// Pattern detected (e.g., user always does X after Y)
    PatternDetected { pattern: String },
    /// Timer/Reminder triggered
    TimerElapsed { reminder_text: String },
}

impl SuggestionTrigger {
    /// Get human-readable description
    pub fn description(&self) -> String {
        match self {
            SuggestionTrigger::CalendarEvent {
                event_title,
                minutes_until,
            } => {
                format!("Calendar: '{}' in {} minutes", event_title, minutes_until)
            }
            SuggestionTrigger::EmailBacklog {
                unread_count,
                important_count,
            } => {
                format!(
                    "Email: {} unread ({} important)",
                    unread_count, important_count
                )
            }
            SuggestionTrigger::ContextSwitch { from, to } => {
                format!("Switched from {:?} to {:?}", from, to)
            }
            SuggestionTrigger::IdleTime { duration_secs } => {
                format!("Idle for {} seconds", duration_secs)
            }
            SuggestionTrigger::FileCreated { filename, location } => {
                format!("New file: {} in {}", filename, location)
            }
            SuggestionTrigger::EndOfDay { minutes_until_eod } => {
                format!("End of day in {} minutes", minutes_until_eod)
            }
            SuggestionTrigger::PatternDetected { pattern } => {
                format!("Pattern: {}", pattern)
            }
            SuggestionTrigger::TimerElapsed { reminder_text } => {
                format!("Reminder: {}", reminder_text)
            }
        }
    }

    /// Get type identifier for deduplication
    pub fn type_name(&self) -> &'static str {
        match self {
            SuggestionTrigger::CalendarEvent { .. } => "calendar",
            SuggestionTrigger::EmailBacklog { .. } => "email",
            SuggestionTrigger::ContextSwitch { .. } => "context_switch",
            SuggestionTrigger::IdleTime { .. } => "idle",
            SuggestionTrigger::FileCreated { .. } => "file",
            SuggestionTrigger::EndOfDay { .. } => "end_of_day",
            SuggestionTrigger::PatternDetected { .. } => "pattern",
            SuggestionTrigger::TimerElapsed { .. } => "timer",
        }
    }
}

/// Priority levels for suggestions
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum SuggestionPriority {
    Low = 1,    // Nice to have
    Medium = 2, // Helpful
    High = 3,   // Important
    Urgent = 4, // Time-sensitive
}

/// Smart suggestion engine
pub struct SmartSuggestionEngine {
    /// Autonomy level (controls how chatty Ghost is)
    autonomy_level: AutonomyLevel,
    /// Privacy settings
    privacy_settings: PrivacySettings,
    /// Suggestion history
    suggestion_history: Vec<SmartSuggestion>,
    /// Maximum suggestions per hour (rate limiting)
    max_suggestions_per_hour: u32,
    /// Cooldown between suggestions
    suggestion_cooldown: Duration,
    /// Last suggestion timestamp
    last_suggestion_time: Option<u64>,
    /// Context triggers configuration
    triggers: HashMap<String, bool>,
}

impl SmartSuggestionEngine {
    /// Create a new suggestion engine
    pub fn new(autonomy_level: AutonomyLevel, privacy_settings: PrivacySettings) -> Self {
        let mut triggers = HashMap::new();
        triggers.insert("calendar".to_string(), true);
        triggers.insert("email".to_string(), true);
        triggers.insert("context_switch".to_string(), true);
        triggers.insert("idle".to_string(), true);
        triggers.insert("files".to_string(), true);
        triggers.insert("end_of_day".to_string(), true);
        triggers.insert("patterns".to_string(), false); // Disabled by default

        Self {
            autonomy_level,
            privacy_settings,
            suggestion_history: Vec::new(),
            max_suggestions_per_hour: 10,
            suggestion_cooldown: Duration::from_secs(300), // 5 minutes
            last_suggestion_time: None,
            triggers,
        }
    }

    /// Generate suggestions based on current context
    pub fn generate_suggestions(
        &mut self,
        app_context: &AppContext,
        idle_duration: Duration,
    ) -> Vec<SmartSuggestion> {
        // Check rate limiting
        if !self.should_generate_suggestion() {
            return Vec::new();
        }

        let mut suggestions = Vec::new();

        // Calendar-based suggestions
        if self.triggers.get("calendar").copied().unwrap_or(false) {
            suggestions.extend(self.check_calendar_events());
        }

        // Email-based suggestions
        if self.triggers.get("email").copied().unwrap_or(false) {
            suggestions.extend(self.check_email_backlog());
        }

        // Idle-based suggestions
        if self.triggers.get("idle").copied().unwrap_or(false) {
            suggestions.extend(self.check_idle_suggestions(idle_duration, app_context));
        }

        // Context-based suggestions
        if self
            .triggers
            .get("context_switch")
            .copied()
            .unwrap_or(false)
        {
            suggestions.extend(self.check_context_suggestions(app_context));
        }

        // End of day suggestions
        if self.triggers.get("end_of_day").copied().unwrap_or(false) {
            suggestions.extend(self.check_end_of_day());
        }

        // Filter by confidence and priority
        let filtered: Vec<SmartSuggestion> = suggestions
            .into_iter()
            .filter(|s| s.confidence >= self.get_min_confidence())
            .filter(|s| self.should_show_suggestion(s))
            .take(self.get_max_suggestions())
            .collect();

        // Update history
        for suggestion in &filtered {
            self.suggestion_history.push(suggestion.clone());
        }

        // Update last suggestion time
        if !filtered.is_empty() {
            self.last_suggestion_time = Some(current_timestamp_secs());
        }

        filtered
    }

    /// Check if we should generate a suggestion (rate limiting)
    fn should_generate_suggestion(&self) -> bool {
        // Check suggestions per hour
        let recent_count = self.get_recent_suggestion_count(Duration::from_secs(3600));
        if recent_count >= self.max_suggestions_per_hour as usize {
            return false;
        }

        // Check cooldown
        if let Some(last_time) = self.last_suggestion_time {
            let now = current_timestamp_secs();
            let elapsed = Duration::from_secs(now - last_time);
            if elapsed < self.suggestion_cooldown {
                return false;
            }
        }

        true
    }

    /// Get minimum confidence based on autonomy level
    fn get_min_confidence(&self) -> f32 {
        match self.autonomy_level {
            AutonomyLevel::Observer => 1.0,  // Never suggest
            AutonomyLevel::Suggester => 0.8, // High confidence only
            AutonomyLevel::Supervised => 0.6,
            AutonomyLevel::Autonomous => 0.5,
        }
    }

    /// Get max suggestions based on autonomy level
    fn get_max_suggestions(&self) -> usize {
        match self.autonomy_level {
            AutonomyLevel::Observer => 0,
            AutonomyLevel::Suggester => 1,
            AutonomyLevel::Supervised => 2,
            AutonomyLevel::Autonomous => 3,
        }
    }

    /// Check if we should show a specific suggestion
    fn should_show_suggestion(&self, suggestion: &SmartSuggestion) -> bool {
        // Don't show low priority in Suggester mode
        if self.autonomy_level == AutonomyLevel::Suggester
            && suggestion.priority < SuggestionPriority::High
        {
            return false;
        }

        // Don't show same suggestion too often
        let similar_count = self
            .suggestion_history
            .iter()
            .filter(|s| s.trigger.type_name() == suggestion.trigger.type_name())
            .filter(|s| s.timestamp > current_timestamp_secs() - 3600) // Last hour
            .count();

        if similar_count >= 3 {
            return false;
        }

        true
    }

    /// Check calendar for upcoming events
    fn check_calendar_events(&self) -> Vec<SmartSuggestion> {
        let mut suggestions = Vec::new();

        // Mock calendar check - in real implementation, integrate with calendar API
        // This would check PrivacySettings.calendar_consent first

        // Example: Meeting in 15 minutes
        suggestions.push(SmartSuggestion {
            id: generate_suggestion_id(),
            trigger: SuggestionTrigger::CalendarEvent {
                event_title: "Team Standup".to_string(),
                minutes_until: 15,
            },
            message: "You have 'Team Standup' in 15 minutes. Want me to pull up the meeting notes?"
                .to_string(),
            confidence: 0.9,
            priority: SuggestionPriority::High,
            requires_action: false,
            suggested_action: Some("Open meeting notes".to_string()),
            timestamp: current_timestamp_secs(),
            seen: false,
            feedback: None,
        });

        suggestions
    }

    /// Check email backlog
    fn check_email_backlog(&self) -> Vec<SmartSuggestion> {
        let mut suggestions = Vec::new();

        // Mock email check - would integrate with email API
        // Check PrivacySettings.email_consent

        suggestions.push(SmartSuggestion {
            id: generate_suggestion_id(),
            trigger: SuggestionTrigger::EmailBacklog {
                unread_count: 12,
                important_count: 3,
            },
            message: "You have 12 unread emails (3 marked important). Want me to triage them?"
                .to_string(),
            confidence: 0.75,
            priority: SuggestionPriority::Medium,
            requires_action: false,
            suggested_action: Some("Triage emails".to_string()),
            timestamp: current_timestamp_secs(),
            seen: false,
            feedback: None,
        });

        suggestions
    }

    /// Check for idle-time suggestions
    fn check_idle_suggestions(
        &self,
        idle_duration: Duration,
        app_context: &AppContext,
    ) -> Vec<SmartSuggestion> {
        let mut suggestions = Vec::new();

        if idle_duration.as_secs() < 60 {
            return suggestions;
        }

        let message = match app_context.category {
            AppCategory::CodeEditor => {
                "You've been idle for a bit. Want me to show your recent git activity?"
            }
            AppCategory::Browser => {
                "Taking a break? While you're away, I noticed some interesting articles in your bookmarks."
            }
            AppCategory::Communication => {
                "You've been idle. I'll keep an eye on your messages and let you know if anything urgent comes in."
            }
            _ => "You've been idle. Is there anything I can help you with?",
        };

        suggestions.push(SmartSuggestion {
            id: generate_suggestion_id(),
            trigger: SuggestionTrigger::IdleTime {
                duration_secs: idle_duration.as_secs(),
            },
            message: message.to_string(),
            confidence: 0.7,
            priority: SuggestionPriority::Low,
            requires_action: false,
            suggested_action: None,
            timestamp: current_timestamp_secs(),
            seen: false,
            feedback: None,
        });

        suggestions
    }

    /// Check for context-based suggestions
    fn check_context_suggestions(&self, app_context: &AppContext) -> Vec<SmartSuggestion> {
        let mut suggestions = Vec::new();

        // Example: Switching from coding to email
        if app_context.category == AppCategory::Communication {
            if let Some(ref prev) = app_context.previous_app {
                if prev.to_lowercase().contains("code") || prev.to_lowercase().contains("studio") {
                    suggestions.push(SmartSuggestion {
                        id: generate_suggestion_id(),
                        trigger: SuggestionTrigger::ContextSwitch {
                            from: AppCategory::CodeEditor,
                            to: AppCategory::Communication,
                        },
                        message: "Switching from coding to messages? I see 3 unread emails about the project you were working on."
                            .to_string(),
                        confidence: 0.85,
                        priority: SuggestionPriority::High,
                        requires_action: false,
                        suggested_action: Some("Show project emails".to_string()),
                        timestamp: current_timestamp_secs(),
                        seen: false,
                        feedback: None,
                    });
                }
            }
        }

        suggestions
    }

    /// Check for end-of-day suggestions
    fn check_end_of_day(&self) -> Vec<SmartSuggestion> {
        let mut suggestions = Vec::new();

        // Mock time check - would check actual time
        let current_hour = 17; // 5 PM

        if current_hour >= 17 {
            suggestions.push(SmartSuggestion {
                id: generate_suggestion_id(),
                trigger: SuggestionTrigger::EndOfDay {
                    minutes_until_eod: 60,
                },
                message: "End of day approaching. You have 2 tasks still open. Want me to summarize what you accomplished today?"
                    .to_string(),
                confidence: 0.8,
                priority: SuggestionPriority::Medium,
                requires_action: false,
                suggested_action: Some("Generate daily summary".to_string()),
                timestamp: current_timestamp_secs(),
                seen: false,
                feedback: None,
            });
        }

        suggestions
    }

    /// Get recent suggestion count
    fn get_recent_suggestion_count(&self, duration: Duration) -> usize {
        let cutoff = current_timestamp_secs() - duration.as_secs();
        self.suggestion_history
            .iter()
            .filter(|s| s.timestamp > cutoff)
            .count()
    }

    /// Record user feedback for a suggestion
    pub fn record_feedback(&mut self, suggestion_id: &str, helpful: bool) {
        if let Some(suggestion) = self
            .suggestion_history
            .iter_mut()
            .find(|s| s.id == suggestion_id)
        {
            suggestion.feedback = Some(helpful);
            suggestion.seen = true;

            tracing::info!(
                "Suggestion {} marked as {} helpful",
                suggestion_id,
                if helpful { "" } else { "not " }
            );
        }
    }

    /// Get suggestion history
    pub fn get_history(&self, limit: usize) -> Vec<&SmartSuggestion> {
        self.suggestion_history.iter().rev().take(limit).collect()
    }

    /// Get stats about suggestions
    pub fn get_stats(&self) -> SuggestionStats {
        let total = self.suggestion_history.len();
        let helpful = self
            .suggestion_history
            .iter()
            .filter(|s| s.feedback == Some(true))
            .count();
        let not_helpful = self
            .suggestion_history
            .iter()
            .filter(|s| s.feedback == Some(false))
            .count();

        SuggestionStats {
            total_suggestions: total,
            helpful,
            not_helpful,
            no_feedback: total - helpful - not_helpful,
            acceptance_rate: if total > 0 {
                helpful as f32 / total as f32
            } else {
                0.0
            },
        }
    }

    /// Update trigger configuration
    pub fn set_trigger_enabled(&mut self, trigger: &str, enabled: bool) {
        self.triggers.insert(trigger.to_string(), enabled);
    }

    /// Get trigger configuration
    pub fn get_trigger_config(&self) -> &HashMap<String, bool> {
        &self.triggers
    }
}

/// Statistics about suggestions
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SuggestionStats {
    pub total_suggestions: usize,
    pub helpful: usize,
    pub not_helpful: usize,
    pub no_feedback: usize,
    pub acceptance_rate: f32,
}

/// Generate a unique suggestion ID
fn generate_suggestion_id() -> String {
    format!("sugg_{}", current_timestamp_secs())
}

/// Get current timestamp
fn current_timestamp_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_suggestion_creation() {
        let suggestion = SmartSuggestion {
            id: "test_123".to_string(),
            trigger: SuggestionTrigger::IdleTime { duration_secs: 60 },
            message: "Test suggestion".to_string(),
            confidence: 0.8,
            priority: SuggestionPriority::Medium,
            requires_action: false,
            suggested_action: None,
            timestamp: 1234567890,
            seen: false,
            feedback: None,
        };

        assert!(!suggestion.seen);
    }

    #[test]
    fn test_suggestion_priority_ordering() {
        assert!(SuggestionPriority::Urgent > SuggestionPriority::High);
        assert!(SuggestionPriority::High > SuggestionPriority::Medium);
        assert!(SuggestionPriority::Medium > SuggestionPriority::Low);
    }

    #[test]
    fn test_stats_calculation() {
        let mut engine =
            SmartSuggestionEngine::new(AutonomyLevel::Autonomous, PrivacySettings::default());

        // Simulate some history
        engine.suggestion_history = vec![
            SmartSuggestion {
                id: "1".to_string(),
                trigger: SuggestionTrigger::IdleTime { duration_secs: 60 },
                message: "Test".to_string(),
                confidence: 0.8,
                priority: SuggestionPriority::Medium,
                requires_action: false,
                suggested_action: None,
                timestamp: 0,
                seen: true,
                feedback: Some(true),
            },
            SmartSuggestion {
                id: "2".to_string(),
                trigger: SuggestionTrigger::IdleTime { duration_secs: 60 },
                message: "Test 2".to_string(),
                confidence: 0.8,
                priority: SuggestionPriority::Medium,
                requires_action: false,
                suggested_action: None,
                timestamp: 0,
                seen: true,
                feedback: Some(false),
            },
        ];

        let stats = engine.get_stats();
        assert_eq!(stats.total_suggestions, 2);
        assert_eq!(stats.helpful, 1);
        assert_eq!(stats.not_helpful, 1);
        assert_eq!(stats.acceptance_rate, 0.5);
    }
}
