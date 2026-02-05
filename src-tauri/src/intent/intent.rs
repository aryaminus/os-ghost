//! Intent engine (lightweight heuristic prototype)

use crate::data::events_bus::{EventEntry, EventKind, EventPriority};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::fs;
use std::path::PathBuf;
use std::sync::Arc;

const INTENT_FEEDBACK_FILE: &str = "intent_feedback.json";
const DISMISS_TTL_SECS: u64 = 6 * 60 * 60;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum IntentKind {
    Suggestion,
    Reminder,
    Task,
    Insight,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IntentCandidate {
    pub id: String,
    pub summary: String,
    pub reason: String,
    pub confidence: f32,
    pub kind: IntentKind,
    pub created_at: u64,
    pub sources: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DismissedIntent {
    pub summary: String,
    pub dismissed_at: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct IntentFeedback {
    pub dismissed: Vec<DismissedIntent>,
}

fn feedback_path() -> PathBuf {
    let mut path = dirs::config_dir().unwrap_or_else(|| PathBuf::from("."));
    path.push("os-ghost");
    path.push(INTENT_FEEDBACK_FILE);
    path
}

fn load_feedback() -> IntentFeedback {
    let path = feedback_path();
    if path.exists() {
        if let Ok(contents) = fs::read_to_string(&path) {
            if let Ok(feedback) = serde_json::from_str::<IntentFeedback>(&contents) {
                return feedback;
            }
        }
    }
    IntentFeedback::default()
}

fn save_feedback(feedback: &IntentFeedback) -> Result<(), String> {
    let path = feedback_path();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let contents = serde_json::to_string_pretty(feedback).map_err(|e| e.to_string())?;
    fs::write(&path, contents).map_err(|e| e.to_string())?;
    Ok(())
}

fn is_dismissed(summary: &str) -> bool {
    let feedback = load_feedback();
    let now = crate::core::utils::current_timestamp();
    feedback.dismissed.iter().any(|entry| {
        entry.summary == summary && now.saturating_sub(entry.dismissed_at) < DISMISS_TTL_SECS
    })
}

fn make_intent(
    summary: impl Into<String>,
    reason: impl Into<String>,
    confidence: f32,
    kind: IntentKind,
    sources: Vec<String>,
) -> IntentCandidate {
    let created_at = crate::core::utils::current_timestamp();
    let id = format!("intent_{}_{}", created_at, sources.len());
    IntentCandidate {
        id,
        summary: summary.into(),
        reason: reason.into(),
        confidence: confidence.clamp(0.0, 1.0),
        kind,
        created_at,
        sources,
    }
}

fn adjust_confidence(base: f32, event: &EventEntry, now: u64) -> f32 {
    let age_secs = now.saturating_sub(event.timestamp);
    let recency_boost = if age_secs < 120 {
        0.25
    } else if age_secs < 300 {
        0.2
    } else if age_secs < 900 {
        0.1
    } else {
        0.0
    };
    let priority_boost = match event.priority {
        EventPriority::Critical => 0.2,
        EventPriority::High => 0.12,
        EventPriority::Normal => 0.05,
        EventPriority::Low => 0.0,
    };
    (base + recency_boost + priority_boost).min(1.0)
}

pub fn derive_intents(events: &[Arc<EventEntry>]) -> Vec<IntentCandidate> {
    let mut intents: Vec<IntentCandidate> = Vec::new();
    let mut seen: HashSet<String> = HashSet::new();
    let now = crate::core::utils::current_timestamp();

    for event in events.iter().take(30) {
        match event.kind {
            EventKind::Suggestion => {
                let summary = event.summary.clone();
                if seen.insert(summary.clone()) && !is_dismissed(&summary) {
                    intents.push(make_intent(
                        summary,
                        event
                            .detail
                            .clone()
                            .unwrap_or_else(|| "Recent suggestion".to_string()),
                        adjust_confidence(0.8, event, now),
                        IntentKind::Suggestion,
                        vec![event.id.clone()],
                    ));
                }
            }
            EventKind::Content => {
                let summary = "Summarize current page".to_string();
                if seen.insert(summary.clone()) && !is_dismissed(&summary) {
                    intents.push(make_intent(
                        summary,
                        "New page content detected".to_string(),
                        adjust_confidence(0.6, event, now),
                        IntentKind::Insight,
                        vec![event.id.clone()],
                    ));
                }
            }
            EventKind::Observation => {
                let app_category = event
                    .metadata
                    .get("app_category")
                    .and_then(|v| v.as_str())
                    .unwrap_or("unknown");
                let summary = match app_category {
                    "coding" => "Offer code review or lint".to_string(),
                    "communication" => "Draft a quick reply".to_string(),
                    "productivity" => "Create a task list from current work".to_string(),
                    "creative" => "Collect references or export assets".to_string(),
                    _ => "Provide a quick summary of activity".to_string(),
                };

                if seen.insert(summary.clone()) && !is_dismissed(&summary) {
                    let reason = event
                        .detail
                        .clone()
                        .unwrap_or_else(|| "Recent activity detected".to_string());
                    intents.push(make_intent(
                        summary,
                        reason,
                        adjust_confidence(0.5, event, now),
                        IntentKind::Task,
                        vec![event.id.clone()],
                    ));
                }
            }
            EventKind::Navigation => {
                let summary = "Check if this page needs follow-up".to_string();
                if seen.insert(summary.clone()) && !is_dismissed(&summary) {
                    intents.push(make_intent(
                        summary,
                        "Recent navigation event".to_string(),
                        adjust_confidence(0.4, event, now),
                        IntentKind::Reminder,
                        vec![event.id.clone()],
                    ));
                }
            }
            _ => {}
        }

        if intents.len() >= 6 {
            break;
        }
    }

    intents.sort_by(|a, b| {
        b.confidence
            .partial_cmp(&a.confidence)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    intents
}

#[tauri::command]
pub fn get_intents(limit: Option<usize>) -> Vec<IntentCandidate> {
    let events = crate::data::events_bus::list_recent_events(limit.unwrap_or(40), 0);
    derive_intents(&events)
}

#[tauri::command]
pub fn get_intent_actions() -> Vec<crate::actions::PendingAction> {
    crate::actions::ACTION_QUEUE
        .get_pending()
        .into_iter()
        .filter(|action| action.action_type.starts_with("intent."))
        .collect()
}

#[tauri::command]
pub fn auto_create_top_intent(
    session: tauri::State<'_, std::sync::Arc<crate::memory::SessionMemory>>,
) -> Result<Option<u64>, String> {
    let privacy = crate::config::privacy::PrivacySettings::load();
    if privacy.read_only_mode || !privacy.capture_consent || !privacy.ai_analysis_consent {
        return Ok(None);
    }
    let persona = crate::data::persona::get_persona();
    if !persona.allow_auto_intents {
        return Ok(None);
    }
    let autonomy = privacy.autonomy_level;
    let state = session.load().unwrap_or_default();
    let last_intent_at = state.last_intent_action_at;
    let cooldown_override = if state.intent_cooldown_secs > 0 {
        Some(state.intent_cooldown_secs)
    } else {
        None
    };

    if !crate::intent::intent_autorun::should_auto_create_intent(
        autonomy,
        last_intent_at,
        cooldown_override,
    ) {
        return Ok(None);
    }

    let events = crate::data::events_bus::list_recent_events(40, 0);
    let mut intents = derive_intents(&events);
    if intents.is_empty() {
        return Ok(None);
    }

    if crate::actions::ACTION_QUEUE
        .get_pending()
        .iter()
        .any(|action| action.action_type.starts_with("intent."))
    {
        return Ok(None);
    }

    let top = intents.remove(0);
    if top.confidence < 0.6 {
        return Ok(None);
    }

    // Guardrail: avoid intent actions when no content exists for content-dependent intents
    if top.summary == "Summarize current page" || top.summary == "Draft a quick reply" {
        let state = session.load().unwrap_or_default();
        if state.current_content.as_deref().unwrap_or("").is_empty() {
            return Ok(None);
        }
    }
    let action_id = create_intent_action_internal(top.summary.clone(), session.as_ref())?;
    let _ = session.record_intent_action();
    crate::data::events_bus::record_event(
        crate::data::events_bus::EventKind::Action,
        format!("Auto-created intent action: {}", top.summary),
        None,
        std::collections::HashMap::new(),
        crate::data::events_bus::EventPriority::Normal,
        Some(format!("intent_action:{}", action_id)),
        Some(600),
        Some("intent".to_string()),
    );
    Ok(Some(action_id))
}

fn create_intent_action_internal(
    summary: String,
    session: &crate::memory::SessionMemory,
) -> Result<u64, String> {
    if summary.trim().is_empty() {
        return Err("Intent summary cannot be empty".to_string());
    }

    let state = session.load().unwrap_or_default();
    let title = state.current_title.clone();
    let url = state.current_url.clone();

    let (action_type, description, target, risk, reason) = if summary == "Summarize current page" {
        (
            "intent.summarize_page".to_string(),
            if title.is_empty() {
                "Summarize current page".to_string()
            } else {
                format!("Summarize: {}", title)
            },
            if url.is_empty() {
                "page".to_string()
            } else {
                url.clone()
            },
            crate::actions::ActionRiskLevel::Low,
            Some("Intent-derived page summary".to_string()),
        )
    } else if summary == "Create a task list from current work" {
        (
            "intent.create_tasks".to_string(),
            if title.is_empty() {
                "Create tasks from recent activity".to_string()
            } else {
                format!("Create tasks: {}", title)
            },
            "tasks".to_string(),
            crate::actions::ActionRiskLevel::Low,
            Some("Intent-derived task list".to_string()),
        )
    } else if summary == "Draft a quick reply" {
        (
            "intent.draft_reply".to_string(),
            if title.is_empty() {
                "Draft a quick reply".to_string()
            } else {
                format!("Draft reply: {}", title)
            },
            if url.is_empty() {
                "reply".to_string()
            } else {
                url.clone()
            },
            crate::actions::ActionRiskLevel::Low,
            Some("Intent-derived reply draft".to_string()),
        )
    } else {
        (
            "intent.quick_ask".to_string(),
            format!("Ask: {}", summary),
            summary.clone(),
            crate::actions::ActionRiskLevel::Low,
            Some("Intent-derived quick ask".to_string()),
        )
    };

    let mut args = serde_json::json!({ "title": title, "url": url });
    if action_type == "intent.quick_ask" {
        args = serde_json::json!({ "prompt": summary });
    }

    let pending = crate::actions::PendingAction::new(
        action_type,
        description,
        target,
        risk,
        reason,
        Some(args),
    );

    let _preview_id = if let Some(manager) = crate::action_preview::get_preview_manager_mut() {
        let preview = manager.start_preview(&pending);
        manager.update_progress(&preview.id, 1.0);
        Some(preview.id)
    } else {
        None
    };

    let action_id = crate::actions::ACTION_QUEUE.add(pending.clone());

    // Clone description before it's moved to record_action_created
    let description_for_event = pending.description.clone();

    crate::actions::action_ledger::record_action_created(
        action_id,
        pending.action_type,
        pending.description,
        pending.target,
        "low".to_string(),
        pending.reason,
        pending.arguments,
        Some("intent".to_string()),
    );

    crate::data::events_bus::record_event(
        crate::data::events_bus::EventKind::Action,
        format!("Intent action queued: {}", description_for_event),
        None,
        std::collections::HashMap::new(),
        crate::data::events_bus::EventPriority::Normal,
        Some(format!("intent_action:{}", action_id)),
        Some(600),
        Some("intent".to_string()),
    );

    Ok(action_id)
}

#[tauri::command]
pub fn create_intent_action(
    summary: String,
    session: tauri::State<'_, std::sync::Arc<crate::memory::SessionMemory>>,
) -> Result<u64, String> {
    create_intent_action_internal(summary, session.as_ref())
}

#[tauri::command]
pub fn dismiss_intent(summary: String) -> Result<(), String> {
    let mut feedback = load_feedback();
    let now = crate::core::utils::current_timestamp();

    feedback
        .dismissed
        .retain(|entry| now.saturating_sub(entry.dismissed_at) < DISMISS_TTL_SECS);
    feedback.dismissed.push(DismissedIntent {
        summary,
        dismissed_at: now,
    });
    save_feedback(&feedback)
}
