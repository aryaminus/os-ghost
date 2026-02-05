//! Background monitor for autonomous companion behavior
//! Provides contextual awareness by periodically analyzing the screen
//! and detecting user activity patterns for adaptive behavior
//!
//! OPTIMIZED VERSION:
//! - Screenshot deduplication using hash (skips AI analysis on unchanged screens)
//! - try_lock() instead of lock() to avoid blocking async runtime
//! - Context caching to reduce redundant string building
//! - More efficient memory access patterns

use crate::ai::ai_provider::SmartAiRouter;
use crate::capture::capture;
use crate::core::utils::{clean_json_response, current_timestamp};
use crate::data::events_bus::{record_event, EventKind, EventPriority};
use crate::memory::{ActivityEntry, LongTermMemory, SessionMemory};
use crate::resources::monitor::ResourceMonitor;
use base64::{engine::general_purpose, Engine as _};
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Emitter, Manager};
use tokio::time::Duration;

/// Detected application category
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
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
    #[default]
    Unknown,
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

/// Monitor state for tracking between iterations
struct MonitorState {
    /// Hash of last screenshot to detect duplicates
    last_screenshot_hash: u64,
    /// Recent app categories for pattern detection (using VecDeque for O(1) push/pop)
    recent_categories: VecDeque<AppCategory>,
    /// Consecutive idle observations counter
    consecutive_idle_count: usize,
    /// Cache for user context to avoid rebuilding every iteration
    cached_context: Option<ContextCache>,
    /// When the cache was last updated
    cache_timestamp: u64,
    /// Adaptive backoff duration in seconds (for handling AI timeouts)
    backoff_secs: u64,
}

/// Cached context to avoid rebuilding strings every iteration
struct ContextCache {
    user_facts: String,
    current_url: String,
    recent_activities: String,
}

impl MonitorState {
    fn new(category_window: usize) -> Self {
        Self {
            last_screenshot_hash: 0,
            recent_categories: VecDeque::with_capacity(category_window + 5),
            consecutive_idle_count: 0,
            cached_context: None,
            cache_timestamp: 0,
            backoff_secs: 0,
        }
    }

    /// Check if cache is still valid (within 30 seconds)
    fn is_cache_valid(&self) -> bool {
        self.cached_context.is_some() && {
            let now = current_timestamp();
            now.saturating_sub(self.cache_timestamp) < 30
        }
    }

    /// Reset backoff on success
    fn reset_backoff(&mut self) {
        self.backoff_secs = 0;
    }

    /// Increase backoff on failure (exponential, max 120s)
    fn increase_backoff(&mut self) {
        if self.backoff_secs == 0 {
            self.backoff_secs = 10;
        } else {
            self.backoff_secs = (self.backoff_secs * 2).min(120);
        }
    }
}

/// Simple hash function for screenshot bytes (fast, non-cryptographic)
fn hash_bytes(data: &[u8]) -> u64 {
    // ... existing hash_bytes ...
    // Using a simple FNV-like hash for speed
    let mut hash: u64 = 0xcbf29ce484222325;
    for byte in data.iter().step_by(4) {
        hash = hash.wrapping_mul(0x100000001b3);
        hash ^= *byte as u64;
    }
    hash
}

/// Main background loop with optimized memory access and deduplication
pub async fn start_monitor_loop(
    app: AppHandle,
    ai_router: Arc<SmartAiRouter>,
    long_term: Arc<Mutex<LongTermMemory>>,
    session: Arc<Mutex<SessionMemory>>,
) {
    tracing::info!("Starting optimized autonomous background monitor...");

    // Initialize state
    let mut state = MonitorState::new(10);
    let resource_monitor = ResourceMonitor::new();

    loop {
        let settings = crate::config::system_settings::SystemSettings::load();

        // Calculate sleep duration (base + backoff)
        let sleep_duration = if state.backoff_secs > 0 {
            tracing::warn!(
                "Monitor: Backing off for {}s due to previous errors/timeouts",
                state.backoff_secs
            );
            settings.monitor_interval_secs + state.backoff_secs
        } else {
            settings.monitor_interval_secs
        };

        // Wait for next tick
        tokio::time::sleep(Duration::from_secs(sleep_duration)).await;

        // Check resource limits before proceeding
        if resource_monitor.should_pause(settings.performance_mode) {
            tracing::info!(
                "Monitor: Pausing due to high system load (Performance Mode: {:?})",
                settings.performance_mode
            );
            continue;
        }

        if !settings.monitor_enabled {
            // ... existing checks ...
            tracing::debug!("Monitor: disabled in settings; skipping");
            continue;
        }

        // Pause monitoring when window is hidden
        if !settings.monitor_allow_hidden {
            if let Some(window) = app.get_webview_window("main") {
                if let Ok(false) = window.is_visible() {
                    tracing::debug!("Monitor: window hidden; skipping");
                    continue;
                }
            }
        }

        // Respect privacy consent (no capture / no AI without user opt-in)
        let privacy = crate::config::privacy::PrivacySettings::load();
        if privacy.read_only_mode {
            tracing::debug!("Monitor: read-only mode enabled; skipping");
            continue;
        }
        if !privacy.capture_consent {
            tracing::debug!("Monitor: capture consent not granted; skipping");
            continue;
        }
        if !privacy.ai_analysis_consent {
            tracing::debug!("Monitor: AI analysis consent not granted; skipping");
            continue;
        }

        // Check current mode using try_lock to avoid blocking
        let mode = {
            match session.try_lock() {
                Ok(sess_guard) => sess_guard.load().ok().map(|s| s.current_mode),
                Err(_) => {
                    tracing::debug!("Monitor: session lock contested, skipping");
                    continue;
                }
            }
        };

        if settings.monitor_only_companion && mode != Some(crate::memory::AppMode::Companion) {
            tracing::debug!("Monitor: not in companion mode; skipping");
            continue;
        }

        // Check idle status using try_lock
        let last_activity = {
            match session.try_lock() {
                Ok(sess_guard) => sess_guard.load().ok().map(|s| s.last_activity).unwrap_or(0),
                Err(_) => {
                    tracing::debug!("Monitor: session lock contested for idle check, skipping");
                    continue;
                }
            }
        };

        let now = current_timestamp();
        if !settings.monitor_ignore_idle
            && last_activity > 0
            && now.saturating_sub(last_activity) > settings.monitor_idle_secs
        {
            tracing::debug!("Monitor: user idle; skipping");
            continue;
        }

        // Check analysis cooldown using try_lock
        let last_analysis_at = {
            match session.try_lock() {
                Ok(sess_guard) => sess_guard
                    .load()
                    .ok()
                    .map(|s| s.last_analysis_at)
                    .unwrap_or(0),
                Err(_) => 0, // Continue without cooldown check if lock contested
            }
        };

        let analysis_cooldown = settings
            .analysis_cooldown_secs
            .max(settings.monitor_interval_secs);
        if last_analysis_at > 0 && now.saturating_sub(last_analysis_at) < analysis_cooldown {
            tracing::debug!("Monitor: analysis cooldown active; skipping");
            continue;
        }

        // 1. Capture Screen with deduplication (Resize to max 512px for faster local AI performance)
        let screenshot_result = tokio::task::spawn_blocking(|| {
            // Resize to 512x512 max - optimal for Ollama vision performance vs context
            capture::capture_primary_monitor_resized(512, 512)
        })
        .await;

        let (base64_image, screenshot_hash) = match screenshot_result {
            Ok(Ok(img)) => {
                // Decode base64 to bytes for hashing
                if let Ok(bytes) = general_purpose::STANDARD.decode(&img) {
                    let hash = hash_bytes(&bytes);
                    (img, hash)
                } else {
                    (img, 0)
                }
            }
            _ => {
                tracing::warn!("Monitor failed to capture screen");
                continue;
            }
        };

        // Check for duplicate screenshot (screen hasn't changed)
        if screenshot_hash != 0 && screenshot_hash == state.last_screenshot_hash {
            tracing::debug!("Monitor: screen unchanged, skipping AI analysis");
            continue;
        }
        state.last_screenshot_hash = screenshot_hash;

        // Record capture for session metrics using try_lock (best effort)
        if let Ok(session_guard) = session.try_lock() {
            let _ = session_guard.record_screenshot();
        }

        // 2. Build context (use cache if valid, otherwise rebuild)
        let (user_facts, current_url, recent_activities) = if state.is_cache_valid() {
            // Use cached context
            let cache = state.cached_context.as_ref().unwrap();
            (
                cache.user_facts.clone(),
                cache.current_url.clone(),
                cache.recent_activities.clone(),
            )
        } else {
            // Rebuild context with try_lock to avoid blocking
            let facts = match long_term.try_lock() {
                Ok(ltm) => ltm
                    .get_user_facts()
                    .unwrap_or_default()
                    .iter()
                    .take(20) // Limit to 20 facts to reduce prompt size
                    .map(|(k, v)| {
                        format!(
                            "{}: {}",
                            k,
                            crate::config::privacy::maybe_redact_pii(v, privacy.redact_pii)
                        )
                    })
                    .collect::<Vec<_>>()
                    .join(", "),
                Err(_) => {
                    tracing::debug!("Monitor: LTM lock contested, using empty facts");
                    String::new()
                }
            };

            let (url, activities) = match session.try_lock() {
                Ok(sess) => {
                    let state = sess.load().unwrap_or_default();
                    let recent = sess
                        .get_recent_activity(settings.monitor_recent_activity_count.min(10)) // Cap at 10
                        .unwrap_or_default()
                        .iter()
                        .map(|a| {
                            crate::config::privacy::maybe_redact_pii(
                                &a.description,
                                privacy.redact_pii,
                            )
                        })
                        .collect::<Vec<_>>()
                        .join("; ");
                    (
                        crate::config::privacy::maybe_redact_pii(
                            &state.current_url,
                            privacy.redact_pii,
                        ),
                        recent,
                    )
                }
                Err(_) => {
                    tracing::debug!("Monitor: session lock contested, using empty context");
                    (String::new(), String::new())
                }
            };

            // Update cache
            let cache = ContextCache {
                user_facts: facts.clone(),
                current_url: url.clone(),
                recent_activities: activities.clone(),
            };
            state.cached_context = Some(cache);
            state.cache_timestamp = now;

            (facts, url, activities)
        };

        // 3. Enhanced AI Analysis with app detection
        if !privacy.ai_analysis_consent {
            tracing::debug!("Monitor: AI analysis consent not granted; skipping");
            continue;
        }

        // Build optimized prompt (shorter, more focused)
        let prompt = format!(
            r#"Analyze the screenshot. Context: Facts=[{}], URL=[{}], Recent=[{}]
Respond in JSON: {{"activity":"brief description","is_idle":bool,"new_fact":string|null,"app_name":string|null,"app_category":"browser|coding|creative|communication|entertainment|productivity|gaming|system|unknown","content_context":string|null,"puzzle_theme":string|null}}"#,
            if user_facts.is_empty() {
                "none"
            } else {
                &user_facts
            },
            if current_url.is_empty() {
                "none"
            } else {
                &current_url
            },
            if recent_activities.is_empty() {
                "none"
            } else {
                &recent_activities
            }
        );

        // AI analysis with timeout to prevent hanging (increased to 30s for slower local models)
        let analysis_result = tokio::time::timeout(Duration::from_secs(30), async {
            tracing::debug!(
                "Calling AI analysis from Monitor Loop (Thread: {:?})",
                std::thread::current().id()
            );
            ai_router.analyze_image(&base64_image, &prompt).await
        })
        .await;

        match analysis_result {
            Ok(Ok(json_str)) => {
                let clean_json = clean_json_response(&json_str);

                match serde_json::from_str::<ObservationResult>(clean_json) {
                    Ok(observation) => {
                        // Success! Reset backoff
                        state.reset_backoff();

                        tracing::debug!("Monitor observed: {:?}", observation);

                        let now = current_timestamp();

                        // Track idle patterns
                        if observation.is_idle {
                            state.consecutive_idle_count += 1;
                        } else {
                            state.consecutive_idle_count = 0;
                        }

                        // Track category patterns (using VecDeque for efficiency)
                        state
                            .recent_categories
                            .push_back(observation.app_category.clone());
                        if state.recent_categories.len() > settings.monitor_category_window {
                            state.recent_categories.pop_front();
                        }

                        // Store facts using try_lock (best effort)
                        if let Some(ref fact) = observation.new_fact {
                            if let Ok(ltm_guard) = long_term.try_lock() {
                                let _ =
                                    ltm_guard.record_fact("last_activity", &observation.activity);
                                let _ = ltm_guard.record_fact("last_new_fact", fact);
                                if let Some(ref app) = observation.app_name {
                                    let _ = ltm_guard.record_fact("last_app", app);
                                }
                                tracing::info!("Recorded new fact: {}", fact);
                            }
                        }

                        // Update session using try_lock (best effort)
                        if let Ok(sess_guard) = session.try_lock() {
                            let _ = sess_guard.touch();
                            let _ = sess_guard.record_analysis();

                            if !observation.is_idle {
                                let _ = sess_guard.add_activity(ActivityEntry {
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

                        // Generate companion behavior
                        let behavior = generate_companion_behavior(
                            &observation,
                            &state.recent_categories.iter().collect::<Vec<_>>(),
                            state.consecutive_idle_count,
                            settings.monitor_idle_streak_threshold,
                        );

                        // Emit events
                        if !observation.is_idle {
                            let _ = app.emit("ghost_observation", &observation);
                        }

                        if let Some(b) = behavior {
                            tracing::info!(
                                "Companion behavior triggered: {} - {}",
                                b.behavior_type,
                                b.suggestion
                            );
                            let _ = app.emit("companion_behavior", &b);

                            let mut metadata = serde_json::Map::new();
                            metadata.insert(
                                "behavior_type".to_string(),
                                serde_json::Value::String(b.behavior_type.clone()),
                            );
                            metadata.insert(
                                "urgency".to_string(),
                                serde_json::Value::Number(
                                    serde_json::Number::from_f64(b.urgency as f64)
                                        .unwrap_or_else(|| serde_json::Number::from(0)),
                                ),
                            );
                            record_event(
                                EventKind::Suggestion,
                                b.suggestion.clone(),
                                Some(b.trigger_context.clone()),
                                metadata.into_iter().collect(),
                                EventPriority::Normal,
                                Some(format!("suggestion:{}", b.behavior_type)),
                                Some(300),
                                Some("monitor".to_string()),
                            );
                        }

                        // Record observation event
                        let mut metadata = serde_json::Map::new();
                        metadata.insert(
                            "app_category".to_string(),
                            serde_json::to_value(&observation.app_category).unwrap_or_default(),
                        );
                        metadata.insert(
                            "app_name".to_string(),
                            serde_json::to_value(&observation.app_name).unwrap_or_default(),
                        );
                        metadata.insert(
                            "content_context".to_string(),
                            serde_json::to_value(&observation.content_context).unwrap_or_default(),
                        );
                        record_event(
                            EventKind::Observation,
                            observation.activity.clone(),
                            observation.new_fact.clone(),
                            metadata.into_iter().collect(),
                            if observation.is_idle {
                                EventPriority::Low
                            } else {
                                EventPriority::Normal
                            },
                            Some("observation".to_string()),
                            Some(120),
                            Some("monitor".to_string()),
                        );
                    }
                    Err(e) => {
                        let snippet: String = clean_json.chars().take(200).collect();
                        tracing::warn!(
                            "Failed to parse observation JSON: {} - Raw: {}",
                            e,
                            snippet
                        );
                        state.increase_backoff();
                    }
                }
            }
            Ok(Err(e)) => {
                tracing::warn!("Monitor analysis failed: {}", e);
                state.increase_backoff();
            }
            Err(_) => {
                tracing::warn!("Monitor analysis timed out after 30s");
                state.increase_backoff();
            }
        }
    }
}

/// Generate companion behavior based on observation context
fn generate_companion_behavior(
    observation: &ObservationResult,
    recent_categories: &[&AppCategory],
    consecutive_idle: usize,
    idle_streak_threshold: usize,
) -> Option<CompanionBehavior> {
    // If user has been idle for a while, suggest a puzzle
    if consecutive_idle >= idle_streak_threshold {
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
                let ctx_lower = context.to_lowercase();
                if ctx_lower.contains("error") || ctx_lower.contains("bug") {
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
                .take_while(|c| ***c == AppCategory::Entertainment)
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
