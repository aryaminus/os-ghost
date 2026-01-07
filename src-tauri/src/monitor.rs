//! Background monitor for autonomous companion behavior
//! Improves contextual awareness by periodically analyzing the screen

use crate::ai_client::GeminiClient;
use crate::capture;
use crate::memory::{LongTermMemory, SessionMemory};
use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Emitter, Manager};
use tokio::time::{sleep, Duration};

const MONITOR_INTERVAL_SECS: u64 = 60;

/// Response structure from AI observation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ObservationResult {
    pub activity: String,
    pub is_idle: bool,
    pub new_fact: Option<String>,
}

/// Main background loop with shared memory access
pub async fn start_monitor_loop(
    app: AppHandle,
    gemini: Arc<GeminiClient>,
    long_term: Arc<Mutex<LongTermMemory>>,
    session: Arc<Mutex<SessionMemory>>,
) {
    tracing::info!("Starting autonomous background monitor with shared memory...");

    loop {
        sleep(Duration::from_secs(MONITOR_INTERVAL_SECS)).await;

        // 1. Capture Screen (Self-hiding)
        let window = app.get_webview_window("main");

        // Hide window
        if let Some(ref w) = window {
            let _ = w.hide();
            sleep(Duration::from_millis(150)).await;
        }

        // Capture
        let screenshot_res =
            tokio::task::spawn_blocking(|| capture::capture_primary_monitor()).await;

        // Show window
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

        // 2. Build context from both memory sources
        let (user_facts, current_url) = {
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

            let url = if let Ok(sess) = session.lock() {
                sess.load().map(|s| s.current_url).unwrap_or_default()
            } else {
                String::new()
            };

            (facts, url)
        };

        // 3. AI Analysis with enriched context
        let prompt = format!(
            "You are observing the user's desktop. 
            Current Context/Facts: [{}]
            Last Known URL: [{}]
            
            Identify the MAIN application or activity visible (e.g., 'Writing code in VS Code', 'Browsing Amazon', 'Watching YouTube').
            If it matches a known fact, ignore it. If it's new or interesting, be specific.
            
            Respond with a JSON object:
            {{
                \"activity\": \"string\", 
                \"is_idle\": boolean,
                \"new_fact\": \"string or null\"
            }}",
            user_facts, current_url
        );

        match gemini.analyze_image(&base64_image, &prompt).await {
            Ok(json_str) => {
                // Clean up markdown code blocks if present
                let clean_json = json_str
                    .trim()
                    .trim_start_matches("```json")
                    .trim_start_matches("```")
                    .trim_end_matches("```")
                    .trim();

                // Parse with proper serde struct
                match serde_json::from_str::<ObservationResult>(clean_json) {
                    Ok(observation) => {
                        tracing::debug!("Monitor observed: {:?}", observation);

                        // Store new fact if present
                        if let Some(ref fact) = observation.new_fact {
                            if let Ok(ltm) = long_term.lock() {
                                let _ = ltm.record_fact("last_activity", &observation.activity);
                                let _ = ltm.record_fact("last_new_fact", fact);
                                tracing::info!("Recorded new fact: {}", fact);
                            }
                        }

                        // Update session with observed activity
                        if let Ok(sess) = session.lock() {
                            let _ = sess.touch(); // Update last activity timestamp
                        }

                        // Emit to frontend if not idle
                        if !observation.is_idle {
                            let _ = app.emit("ghost_observation", &observation);
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
