//! Background monitor for autonomous companion behavior
//! Provides contextual awareness by periodically analyzing the screen
//! and detecting user activity patterns for adaptive behavior

use crate::ai_client::GeminiClient;
use crate::capture;
use crate::memory::{ActivityEntry, LongTermMemory, SessionMemory};
use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Emitter, Manager};
use tokio::time::{sleep, Duration};

const MONITOR_INTERVAL_SECS: u64 = 60;

/// Detected application category
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum AppCategory {
    Browser,
    Coding,
    Creative,
    Communication,
    Entertainment,
    Productivity,
    Gaming,
    System,
    Unknown,
}

impl Default for AppCategory {
    fn default() -> Self {
        Self::Unknown
    }
}

/// Enhanced observation result with app categorization
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ObservationResult {
    /// Main activity description
    pub activity: String,
    /// Whether user appears idle
    pub is_idle: bool,
    /// Any new interesting fact discovered
    pub new_fact: Option<String>,
    /// Detected application name (if identifiable)
    #[serde(default)]
    pub app_name: Option<String>,
    /// Application category
    #[serde(default)]
    pub app_category: AppCategory,
    /// Content context (what they're looking at)
    #[serde(default)]
    pub content_context: Option<String>,
    /// Suggested puzzle theme based on activity
    #[serde(default)]
    pub puzzle_theme: Option<String>,
}

/// Companion behavior suggestion based on observation
#[derive(Debug, Clone, Serialize)]
pub struct CompanionBehavior {
    /// Type of behavior: "comment", "suggestion", "puzzle", "idle"
    pub behavior_type: String,
    /// Context that triggered this behavior
    pub trigger_context: String,
    /// Suggested dialogue or action
    pub suggestion: String,
    /// Urgency level (0-1, higher = more immediate)
    pub urgency: f32,
}

/// Main background loop with shared memory access
pub async fn start_monitor_loop(
    app: AppHandle,
    gemini: Arc<GeminiClient>,
    long_term: Arc<Mutex<LongTermMemory>>,
    session: Arc<Mutex<SessionMemory>>,
) {
    tracing::info!("Starting autonomous background monitor with adaptive behavior...");

    // Track consecutive observations for pattern detection
    let mut recent_categories: Vec<AppCategory> = Vec::new();
    let mut consecutive_idle_count = 0;

    loop {
        sleep(Duration::from_secs(MONITOR_INTERVAL_SECS)).await;

        // 1. Capture Screen (Self-hiding)
        let window = app.get_webview_window("main");

        if let Some(ref w) = window {
            let _ = w.hide();
            sleep(Duration::from_millis(150)).await;
        }

        let screenshot_res =
            tokio::task::spawn_blocking(|| capture::capture_primary_monitor()).await;

        if let Some(ref w) = window {
            let _ = w.show();
        }

        let base64_image = match screenshot_res {
            Ok(Ok(img)) => img,
            _ => {
                tracing::warn!("Monitor failed to capture screen");
                continue;
            }
        };

        // 2. Build rich context from memory
        let (user_facts, current_url, recent_activities) = {
            let facts = if let Ok(ltm) = long_term.lock() {
                ltm.get_user_facts()
                    .unwrap_or_default()
                    .iter()
                    .map(|(k, v)| format!("{}: {}", k, v))
                    .collect::<Vec<_>>()
                    .join(", ")
            } else {
                String::new()
            };

            let (url, activities) = if let Ok(sess) = session.lock() {
                let state = sess.load().unwrap_or_default();
                let recent = sess
                    .get_recent_activity(5)
                    .unwrap_or_default()
                    .iter()
                    .map(|a| a.description.clone())
                    .collect::<Vec<_>>()
                    .join("; ");
                (state.current_url, recent)
            } else {
                (String::new(), String::new())
            };

            (facts, url, activities)
        };

        // 3. Enhanced AI Analysis with app detection
        let prompt = format!(
            r#"You are an AI companion observing the user's desktop to provide helpful context-aware assistance.

Current Context:
- Known Facts: [{}]
- Last URL: [{}]
- Recent Activities: [{}]

Analyze the screenshot and respond with a JSON object:
{{
    "activity": "Brief description of what user is doing",
    "is_idle": false,
    "new_fact": "Any new interesting information (or null)",
    "app_name": "Detected application name (VS Code, Chrome, Slack, etc.) or null",
    "app_category": "browser|coding|creative|communication|entertainment|productivity|gaming|system|unknown",
    "content_context": "What content they're focused on (code file, article topic, video title, etc.) or null",
    "puzzle_theme": "A topic for a fun puzzle related to their activity (or null if not applicable)"
}}

Categories:
- browser: Chrome, Firefox, Safari, Edge, Arc
- coding: VS Code, IntelliJ, Xcode, Terminal, GitHub
- creative: Figma, Photoshop, Illustrator, Blender
- communication: Slack, Discord, Teams, Email, Messages
- entertainment: YouTube, Netflix, Spotify, Music apps
- productivity: Notes, Calendar, Docs, Sheets, Notion
- gaming: Any games
- system: Finder, Settings, System utilities"#,
            user_facts, current_url, recent_activities
        );

        match gemini.analyze_image(&base64_image, &prompt).await {
            Ok(json_str) => {
                let clean_json = json_str
                    .trim()
                    .trim_start_matches("```json")
                    .trim_start_matches("```")
                    .trim_end_matches("```")
                    .trim();

                match serde_json::from_str::<ObservationResult>(clean_json) {
                    Ok(observation) => {
                        tracing::debug!("Monitor observed: {:?}", observation);

                        let now = std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .unwrap_or_default()
                            .as_secs();

                        // Track idle patterns
                        if observation.is_idle {
                            consecutive_idle_count += 1;
                        } else {
                            consecutive_idle_count = 0;
                        }

                        // Track category patterns
                        recent_categories.push(observation.app_category.clone());
                        if recent_categories.len() > 10 {
                            recent_categories.remove(0);
                        }

                        // Store facts
                        if let Some(ref fact) = observation.new_fact {
                            if let Ok(ltm) = long_term.lock() {
                                let _ = ltm.record_fact("last_activity", &observation.activity);
                                let _ = ltm.record_fact("last_new_fact", fact);
                                if let Some(ref app) = observation.app_name {
                                    let _ = ltm.record_fact("last_app", app);
                                }
                                tracing::info!("Recorded new fact: {}", fact);
                            }
                        }

                        // Update session with detailed activity
                        if let Ok(sess) = session.lock() {
                            let _ = sess.touch();

                            if !observation.is_idle {
                                let _ = sess.add_activity(ActivityEntry {
                                    activity_type: "observation".to_string(),
                                    description: observation.activity.clone(),
                                    timestamp: now,
                                    metadata: Some(serde_json::json!({
                                        "app_name": observation.app_name,
                                        "app_category": observation.app_category,
                                        "content_context": observation.content_context,
                                        "puzzle_theme": observation.puzzle_theme,
                                        "is_idle": observation.is_idle,
                                    })),
                                });
                            }
                        }

                        // Generate companion behavior based on observation
                        let behavior = generate_companion_behavior(
                            &observation,
                            &recent_categories,
                            consecutive_idle_count,
                        );

                        // Emit enhanced observation to frontend
                        if !observation.is_idle {
                            let _ = app.emit("ghost_observation", &observation);
                        }

                        // Emit companion behavior if applicable
                        if let Some(b) = behavior {
                            tracing::info!(
                                "Companion behavior triggered: {} - {}",
                                b.behavior_type,
                                b.suggestion
                            );
                            let _ = app.emit("companion_behavior", &b);
                        }
                    }
                    Err(e) => {
                        tracing::debug!(
                            "Failed to parse observation JSON: {} - Raw: {}",
                            e,
                            clean_json
                        );
                    }
                }
            }
            Err(e) => tracing::warn!("Monitor analysis failed: {}", e),
        }
    }
}

/// Generate companion behavior based on observation context
fn generate_companion_behavior(
    observation: &ObservationResult,
    recent_categories: &[AppCategory],
    consecutive_idle: usize,
) -> Option<CompanionBehavior> {
    // If user has been idle for a while, suggest a puzzle
    if consecutive_idle >= 3 {
        return Some(CompanionBehavior {
            behavior_type: "puzzle".to_string(),
            trigger_context: "User has been idle".to_string(),
            suggestion: "Perhaps a quick puzzle to refresh your mind?".to_string(),
            urgency: 0.3,
        });
    }

    // If there's a puzzle theme, suggest it
    if let Some(ref theme) = observation.puzzle_theme {
        if !theme.is_empty() && observation.app_category != AppCategory::Gaming {
            return Some(CompanionBehavior {
                behavior_type: "puzzle".to_string(),
                trigger_context: observation.activity.clone(),
                suggestion: format!(
                    "I see you're interested in {}. Want a puzzle about that?",
                    theme
                ),
                urgency: 0.4,
            });
        }
    }

    // Context-aware comments based on category
    match observation.app_category {
        AppCategory::Coding => {
            if let Some(ref context) = observation.content_context {
                if context.to_lowercase().contains("error")
                    || context.to_lowercase().contains("bug")
                {
                    return Some(CompanionBehavior {
                        behavior_type: "comment".to_string(),
                        trigger_context: observation.activity.clone(),
                        suggestion: "Debugging can be tricky. Take a break if needed!".to_string(),
                        urgency: 0.2,
                    });
                }
            }
        }
        AppCategory::Entertainment => {
            // Count how long they've been in entertainment
            let entertainment_streak = recent_categories
                .iter()
                .rev()
                .take_while(|c| **c == AppCategory::Entertainment)
                .count();

            if entertainment_streak >= 3 {
                return Some(CompanionBehavior {
                    behavior_type: "suggestion".to_string(),
                    trigger_context: observation.activity.clone(),
                    suggestion:
                        "Enjoying some downtime? When you're ready, I have mysteries waiting..."
                            .to_string(),
                    urgency: 0.1,
                });
            }
        }
        AppCategory::Browser => {
            // Browser activity is prime puzzle territory
            if let Some(ref context) = observation.content_context {
                return Some(CompanionBehavior {
                    behavior_type: "puzzle".to_string(),
                    trigger_context: format!("Browsing: {}", context),
                    suggestion: format!(
                        "Your browsing gave me an idea for a puzzle about {}!",
                        context
                    ),
                    urgency: 0.5,
                });
            }
        }
        _ => {}
    }

    None
}
