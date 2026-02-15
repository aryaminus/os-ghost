//! Observability Module - Prometheus and OpenTelemetry Support
//!
//! Provides metrics and tracing capabilities for production monitoring.
//! Reference: ZeroClaw observability design

use serde::{Deserialize, Serialize};
use std::sync::RwLock;

lazy_static::lazy_static! {
    static ref METRICS: RwLock<Metrics> = RwLock::new(Metrics::default());
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Metrics {
    pub requests_total: u64,
    pub requests_success: u64,
    pub requests_failed: u64,
    pub ai_calls_total: u64,
    pub ai_calls_gemini: u64,
    pub ai_calls_ollama: u64,
    pub actions_executed: u64,
    pub actions_approved: u64,
    pub actions_denied: u64,
    pub memory_entries: u64,
    pub memory_recalls: u64,
    pub channel_messages_sent: u64,
    pub channel_messages_received: u64,
    pub start_time_secs: u64,
}

impl Metrics {
    pub fn new() -> Self {
        Self {
            start_time_secs: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0),
            ..Default::default()
        }
    }

    pub fn increment_requests(&self) {
        if let Ok(mut m) = METRICS.write() {
            m.requests_total += 1;
        }
    }

    pub fn increment_success(&self) {
        if let Ok(mut m) = METRICS.write() {
            m.requests_success += 1;
        }
    }

    pub fn increment_failed(&self) {
        if let Ok(mut m) = METRICS.write() {
            m.requests_failed += 1;
        }
    }

    pub fn increment_ai_calls(&self, provider: &str) {
        if let Ok(mut m) = METRICS.write() {
            m.ai_calls_total += 1;
            match provider {
                "gemini" => m.ai_calls_gemini += 1,
                "ollama" => m.ai_calls_ollama += 1,
                _ => {}
            }
        }
    }

    pub fn increment_actions(&self) {
        if let Ok(mut m) = METRICS.write() {
            m.actions_executed += 1;
        }
    }

    pub fn increment_actions_approved(&self) {
        if let Ok(mut m) = METRICS.write() {
            m.actions_approved += 1;
        }
    }

    pub fn increment_actions_denied(&self) {
        if let Ok(mut m) = METRICS.write() {
            m.actions_denied += 1;
        }
    }

    pub fn increment_memory_entries(&self) {
        if let Ok(mut m) = METRICS.write() {
            m.memory_entries += 1;
        }
    }

    pub fn increment_memory_recalls(&self) {
        if let Ok(mut m) = METRICS.write() {
            m.memory_recalls += 1;
        }
    }

    pub fn increment_messages_sent(&self) {
        if let Ok(mut m) = METRICS.write() {
            m.channel_messages_sent += 1;
        }
    }

    pub fn increment_messages_received(&self) {
        if let Ok(mut m) = METRICS.write() {
            m.channel_messages_received += 1;
        }
    }

    pub fn get(&self) -> Metrics {
        METRICS.read().map(|m| m.clone()).unwrap_or_default()
    }

    pub fn uptime_secs(&self) -> u64 {
        if let Ok(m) = METRICS.read() {
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0);
            now.saturating_sub(m.start_time_secs)
        } else {
            0
        }
    }
}

pub fn get_metrics() -> Metrics {
    Metrics::new().get()
}

pub fn get_metrics_prometheus() -> String {
    let m = get_metrics();
    let uptime = Metrics::new().uptime_secs();

    format!(
        r#"# HELP os_ghost_requests_total Total HTTP requests
# TYPE os_ghost_requests_total counter
os_ghost_requests_total {}

# HELP os_ghost_requests_success Successful HTTP requests
# TYPE os_ghost_requests_success counter
os_ghost_requests_success {}

# HELP os_ghost_requests_failed Failed HTTP requests
# TYPE os_ghost_requests_failed counter
os_ghost_requests_failed {}

# HELP os_ghost_ai_calls_total Total AI API calls
# TYPE os_ghost_ai_calls_total counter
os_ghost_ai_calls_total {}

# HELP os_ghost_ai_calls_gemini Gemini API calls
# TYPE os_ghost_ai_calls_gemini counter
os_ghost_ai_calls_gemini {}

# HELP os_ghost_ai_calls_ollama Ollama API calls
# TYPE os_ghost_ai_calls_ollama counter
os_ghost_ai_calls_ollama {}

# HELP os_ghost_actions_executed Total actions executed
# TYPE os_ghost_actions_executed counter
os_ghost_actions_executed {}

# HELP os_ghost_actions_approved Actions approved
# TYPE os_ghost_actions_approved counter
os_ghost_actions_approved {}

# HELP os_ghost_actions_denied Actions denied
# TYPE os_ghost_actions_denied counter
os_ghost_actions_denied {}

# HELP os_ghost_memory_entries Total memory entries
# TYPE os_ghost_memory_entries gauge
os_ghost_memory_entries {}

# HELP os_ghost_memory_recalls Memory recall operations
# TYPE os_ghost_memory_recalls counter
os_ghost_memory_recalls {}

# HELP os_ghost_messages_sent Messages sent via channels
# TYPE os_ghost_messages_sent counter
os_ghost_messages_sent {}

# HELP os_ghost_messages_received Messages received from channels
# TYPE os_ghost_messages_received counter
os_ghost_messages_received {}

# HELP os_ghost_uptime_seconds Server uptime in seconds
# TYPE os_ghost_uptime_seconds gauge
os_ghost_uptime_seconds {}
"#,
        m.requests_total,
        m.requests_success,
        m.requests_failed,
        m.ai_calls_total,
        m.ai_calls_gemini,
        m.ai_calls_ollama,
        m.actions_executed,
        m.actions_approved,
        m.actions_denied,
        m.memory_entries,
        m.memory_recalls,
        m.channel_messages_sent,
        m.channel_messages_received,
        uptime
    )
}

pub fn reset_metrics() {
    if let Ok(mut m) = METRICS.write() {
        *m = Metrics::new();
    }
}

// ============================================================================
// OpenTelemetry Trace Support
// ============================================================================

pub mod otel {
    use std::sync::RwLock;
    use std::time::{SystemTime, UNIX_EPOCH};

    lazy_static::lazy_static! {
        static ref TRACES: RwLock<Vec<Trace>> = RwLock::new(Vec::new());
    }

    #[derive(Debug, Clone)]
    pub struct Trace {
        pub id: String,
        pub name: String,
        pub start_time_ms: u64,
        pub end_time_ms: Option<u64>,
        pub status: TraceStatus,
        pub attributes: std::collections::HashMap<String, String>,
    }

    #[derive(Debug, Clone)]
    pub enum TraceStatus {
        Ok,
        Error(String),
    }

    pub fn start_trace(name: &str) -> String {
        let id = uuid::Uuid::new_v4().to_string();
        let trace = Trace {
            id: id.clone(),
            name: name.to_string(),
            start_time_ms: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|d| d.as_millis() as u64)
                .unwrap_or(0),
            end_time_ms: None,
            status: TraceStatus::Ok,
            attributes: std::collections::HashMap::new(),
        };

        if let Ok(mut traces) = TRACES.write() {
            traces.push(trace);
        }

        id
    }

    pub fn end_trace(id: &str, status: TraceStatus) {
        if let Ok(mut traces) = TRACES.write() {
            if let Some(trace) = traces.iter_mut().find(|t| t.id == id) {
                trace.end_time_ms = Some(
                    SystemTime::now()
                        .duration_since(UNIX_EPOCH)
                        .map(|d| d.as_millis() as u64)
                        .unwrap_or(0),
                );
                trace.status = status;
            }
        }
    }

    pub fn add_attribute(id: &str, key: &str, value: &str) {
        if let Ok(mut traces) = TRACES.write() {
            if let Some(trace) = traces.iter_mut().find(|t| t.id == id) {
                trace.attributes.insert(key.to_string(), value.to_string());
            }
        }
    }

    pub fn get_traces(limit: usize) -> Vec<Trace> {
        TRACES
            .read()
            .map(|traces| traces.iter().rev().take(limit).cloned().collect())
            .unwrap_or_default()
    }

    pub fn clear_traces() {
        if let Ok(mut traces) = TRACES.write() {
            traces.clear();
        }
    }
}

// ============================================================================
// Tauri Commands
// ============================================================================

#[tauri::command]
pub fn get_current_metrics() -> Metrics {
    get_metrics()
}

#[tauri::command]
pub fn get_prometheus_metrics() -> String {
    get_metrics_prometheus()
}

#[tauri::command]
pub fn metrics_reset() {
    reset_metrics()
}

#[tauri::command]
pub fn get_traces(limit: Option<usize>) -> Vec<otel::Trace> {
    otel::get_traces(limit.unwrap_or(100))
}

#[tauri::command]
pub fn traces_clear() {
    otel::clear_traces();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_metrics() {
        let metrics = Metrics::new();
        assert_eq!(metrics.requests_total, 0);

        metrics.increment_requests();
        metrics.increment_success();

        let m = metrics.get();
        assert_eq!(m.requests_total, 1);
        assert_eq!(m.requests_success, 1);
    }

    #[test]
    fn test_prometheus_format() {
        let output = get_metrics_prometheus();
        assert!(output.contains("os_ghost_requests_total"));
    }

    #[test]
    fn test_trace() {
        let id = otel::start_trace("test_operation");
        assert!(!id.is_empty());

        otel::add_attribute(&id, "key", "value");
        otel::end_trace(&id, otel::TraceStatus::Ok);

        let traces = otel::get_traces(10);
        assert!(!traces.is_empty());
    }
}
