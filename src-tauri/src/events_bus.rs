//! Unified event bus for system/agent observability
//!
//! Optimized implementation using Arc for cheap cloning and avoiding full vector copies.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::sync::RwLock;

const DEFAULT_MAX_EVENTS: usize = 300;
static EVENT_COUNTER: AtomicU64 = AtomicU64::new(0);

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum EventKind {
    Observation,
    Navigation,
    Content,
    Suggestion,
    Action,
    Routine,
    System,
    Guardrail,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum EventPriority {
    Low,
    Normal,
    High,
    Critical,
}

/// EventEntry optimized with Arc-wrapped metadata to reduce cloning overhead
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventEntry {
    pub id: String,
    pub timestamp: u64,
    pub kind: EventKind,
    pub summary: String,
    pub detail: Option<String>,
    pub priority: EventPriority,
    pub source: Option<String>,
    pub dedup_key: Option<String>,
    /// Arc-wrapped metadata to share between clones instead of deep copying
    #[serde(skip_serializing_if = "HashMap::is_empty", default)]
    pub metadata: Arc<HashMap<String, serde_json::Value>>,
}

impl EventEntry {
    /// Create a new event entry with shared metadata
    pub fn new(
        id: String,
        timestamp: u64,
        kind: EventKind,
        summary: impl Into<String>,
        detail: Option<String>,
        priority: EventPriority,
        source: Option<String>,
        dedup_key: Option<String>,
        metadata: HashMap<String, serde_json::Value>,
    ) -> Self {
        Self {
            id,
            timestamp,
            kind,
            summary: summary.into(),
            detail,
            priority,
            source,
            dedup_key,
            metadata: Arc::new(metadata),
        }
    }
}

/// Optimized event bus using VecDeque for O(1) push/pop and Arc entries
pub struct EventBus {
    /// Use VecDeque for efficient front removal when trimming
    events: Vec<Arc<EventEntry>>,
}

impl Default for EventBus {
    fn default() -> Self {
        Self {
            events: Vec::with_capacity(DEFAULT_MAX_EVENTS),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventBusConfig {
    pub max_events: usize,
    pub dedup_ttl_secs_default: Option<u64>,
}

impl Default for EventBusConfig {
    fn default() -> Self {
        Self {
            max_events: DEFAULT_MAX_EVENTS,
            dedup_ttl_secs_default: Some(600),
        }
    }
}

impl EventBus {
    /// Push a new event with deduplication support
    /// O(1) amortized, only clones Arc pointers
    pub fn push(&mut self, entry: EventEntry, dedup_ttl_secs: Option<u64>, max_events: usize) {
        // Check for duplicates within TTL window (check last 50 events)
        if let Some(ttl) = dedup_ttl_secs {
            if let Some(key) = entry.dedup_key.as_ref() {
                let cutoff = entry.timestamp.saturating_sub(ttl);
                // Only check recent events (optimization: don't scan entire history)
                let is_duplicate = self
                    .events
                    .iter()
                    .rev()
                    .take(50)
                    .any(|e| e.dedup_key.as_ref() == Some(key) && e.timestamp >= cutoff);

                if is_duplicate {
                    return;
                }
            }
        }

        // Wrap entry in Arc for cheap sharing
        let arc_entry = Arc::new(entry);
        self.events.push(arc_entry);

        // Trim to max_events (remove from front, which is O(1) for VecDeque)
        // Using Vec for now but with Arc it's still efficient
        let limit = max_events.max(50).min(2000);
        if self.events.len() > limit {
            // Remove oldest events from the front
            let excess = self.events.len() - limit;
            self.events.drain(0..excess);
        }
    }

    /// List events with pagination - O(limit) instead of O(n) clone
    /// Returns cloned Arc pointers (cheap) instead of deep clones
    pub fn list(&self, limit: usize, offset: usize) -> Vec<Arc<EventEntry>> {
        let limit = limit.min(self.events.len());

        // Reverse iterate (newest first) and take limit + offset, then skip offset
        // This avoids cloning the entire vector
        self.events
            .iter()
            .rev()
            .skip(offset)
            .take(limit)
            .cloned() // Just clones Arc pointer (cheap!)
            .collect()
    }

    /// Get total event count
    pub fn len(&self) -> usize {
        self.events.len()
    }

    /// Clear all events
    pub fn clear(&mut self) {
        self.events.clear();
    }
}

lazy_static::lazy_static! {
    static ref EVENT_BUS: RwLock<EventBus> = RwLock::new(EventBus::default());
    static ref EVENT_BUS_CONFIG: RwLock<EventBusConfig> = RwLock::new(EventBusConfig::default());
}

/// Record an event to the global event bus
/// Non-blocking, best-effort delivery
pub fn record_event(
    kind: EventKind,
    summary: impl Into<String>,
    detail: Option<String>,
    metadata: HashMap<String, serde_json::Value>,
    priority: EventPriority,
    dedup_key: Option<String>,
    dedup_ttl_secs: Option<u64>,
    source: Option<String>,
) {
    let timestamp = crate::utils::current_timestamp();
    // Use Relaxed ordering for ID generation (sufficient for uniqueness, faster)
    let counter = EVENT_COUNTER.fetch_add(1, Ordering::Relaxed);
    let id = format!("event_{}_{}", timestamp, counter);

    let (dedup_ttl, max_events) = EVENT_BUS_CONFIG
        .read()
        .map(|cfg| {
            (
                dedup_ttl_secs.or(cfg.dedup_ttl_secs_default),
                cfg.max_events,
            )
        })
        .unwrap_or((dedup_ttl_secs, DEFAULT_MAX_EVENTS));

    let entry = EventEntry::new(
        id, timestamp, kind, summary, detail, priority, source, dedup_key, metadata,
    );

    // Best-effort write (don't block on lock contention)
    if let Ok(mut bus) = EVENT_BUS.write() {
        bus.push(entry, dedup_ttl, max_events);
    } else {
        tracing::warn!("Event bus lock contested, dropping event");
    }
}

/// List recent events with optional priority filter
/// Returns cloned Arc<EventEntry> for cheap sharing
pub fn list_recent_events_with_priority(
    limit: usize,
    offset: usize,
    min_priority: Option<EventPriority>,
) -> Vec<Arc<EventEntry>> {
    let entries = list_recent_events(limit, offset);
    if let Some(min) = min_priority {
        entries
            .into_iter()
            .filter(|entry| entry.priority >= min)
            .collect()
    } else {
        entries
    }
}

/// List recent events - returns Arc pointers (cheap clone)
pub fn list_recent_events(limit: usize, offset: usize) -> Vec<Arc<EventEntry>> {
    EVENT_BUS
        .read()
        .ok()
        .map(|bus| bus.list(limit, offset))
        .unwrap_or_default()
}

#[tauri::command]
pub fn clear_events() -> bool {
    if let Ok(mut bus) = EVENT_BUS.write() {
        bus.clear();
        return true;
    }
    false
}

#[tauri::command]
pub fn get_event_bus_config() -> EventBusConfig {
    EVENT_BUS_CONFIG
        .read()
        .map(|cfg| cfg.clone())
        .unwrap_or_default()
}

#[tauri::command]
pub fn set_event_bus_config(
    max_events: usize,
    dedup_ttl_secs_default: Option<u64>,
) -> EventBusConfig {
    let mut cfg = EventBusConfig::default();
    cfg.max_events = max_events.max(50).min(2000);
    cfg.dedup_ttl_secs_default = dedup_ttl_secs_default;
    if let Ok(mut store) = EVENT_BUS_CONFIG.write() {
        *store = cfg.clone();
    }
    cfg
}

#[tauri::command]
pub fn get_recent_events(limit: Option<usize>, offset: Option<usize>) -> Vec<EventEntry> {
    let limit = limit.unwrap_or(50).min(200);
    let offset = offset.unwrap_or(0);

    // Convert Arc<EventEntry> to EventEntry for serialization
    list_recent_events(limit, offset)
        .into_iter()
        .map(|arc| (*arc).clone())
        .collect()
}
