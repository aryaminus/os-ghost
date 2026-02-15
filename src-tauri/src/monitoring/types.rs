//! Monitoring and Metrics Types
//!
//! Implements ADK-style evaluation and monitoring patterns:
//! - Agent metrics collection (latency, success rate, token usage)
//! - Tracing with span hierarchy
//! - Quality evaluation criteria
//! - Error recovery with retry logic

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

// =============================================================================
// Metrics Collection
// =============================================================================

/// Metrics for a single agent invocation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InvocationMetrics {
    /// Unique invocation identifier
    pub invocation_id: String,
    /// Agent name that handled this invocation
    pub agent_name: String,
    /// Total end-to-end latency in milliseconds
    pub total_latency_ms: u64,
    /// LLM call latency in milliseconds
    pub llm_latency_ms: u64,
    /// Tool execution latencies by tool name
    pub tool_latencies: HashMap<String, u64>,
    /// Input token count
    pub input_tokens: u32,
    /// Output token count
    pub output_tokens: u32,
    /// Total tokens used
    pub total_tokens: u32,
    /// List of tool calls made
    pub tool_calls: Vec<ToolCallRecord>,
    /// Whether the invocation succeeded
    pub success: bool,
    /// Error message if failed
    pub error: Option<String>,
    /// Timestamp when invocation started
    pub started_at: u64,
    /// Confidence score from agent output
    pub confidence: f32,
}

impl InvocationMetrics {
    /// Create a new metrics instance for an invocation
    pub fn new(invocation_id: impl Into<String>, agent_name: impl Into<String>) -> Self {
        Self {
            invocation_id: invocation_id.into(),
            agent_name: agent_name.into(),
            total_latency_ms: 0,
            llm_latency_ms: 0,
            tool_latencies: HashMap::new(),
            input_tokens: 0,
            output_tokens: 0,
            total_tokens: 0,
            tool_calls: Vec::new(),
            success: false,
            error: None,
            started_at: current_timestamp_ms(),
            confidence: 0.0,
        }
    }

    /// Record a successful completion
    pub fn complete_success(mut self, latency_ms: u64, confidence: f32) -> Self {
        self.total_latency_ms = latency_ms;
        self.success = true;
        self.confidence = confidence;
        self
    }

    /// Record a failure
    pub fn complete_failure(mut self, latency_ms: u64, error: impl Into<String>) -> Self {
        self.total_latency_ms = latency_ms;
        self.success = false;
        self.error = Some(error.into());
        self
    }

    /// Add LLM metrics
    pub fn with_llm_metrics(mut self, latency_ms: u64, input: u32, output: u32) -> Self {
        self.llm_latency_ms = latency_ms;
        self.input_tokens = input;
        self.output_tokens = output;
        self.total_tokens = input + output;
        self
    }

    /// Add a tool call record
    pub fn add_tool_call(&mut self, record: ToolCallRecord) {
        if let Some(latency) = record.latency_ms {
            self.tool_latencies
                .insert(record.tool_name.clone(), latency);
        }
        self.tool_calls.push(record);
    }
}

/// Record of a tool call
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCallRecord {
    pub tool_name: String,
    pub arguments: HashMap<String, serde_json::Value>,
    pub success: bool,
    pub latency_ms: Option<u64>,
    pub error: Option<String>,
}

// =============================================================================
// Aggregate Metrics
// =============================================================================

/// Aggregate metrics across multiple invocations
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AggregateMetrics {
    /// Total number of invocations
    pub total_invocations: u64,
    /// Number of successful invocations
    pub successful_invocations: u64,
    /// Number of failed invocations
    pub failed_invocations: u64,
    /// Success rate (0.0 - 1.0)
    pub success_rate: f32,
    /// Average latency in milliseconds
    pub avg_latency_ms: f64,
    /// P95 latency in milliseconds
    pub p95_latency_ms: u64,
    /// Total tokens used
    pub total_tokens: u64,
    /// Average tokens per invocation
    pub avg_tokens_per_invocation: f64,
    /// Total tool calls made
    pub total_tool_calls: u64,
    /// Tool success rate by tool name
    pub tool_success_rates: HashMap<String, f32>,
    /// Average confidence score
    pub avg_confidence: f32,
}

impl AggregateMetrics {
    /// Calculate aggregate metrics from individual invocations
    pub fn from_invocations(invocations: &[InvocationMetrics]) -> Self {
        if invocations.is_empty() {
            return Self::default();
        }

        let total = invocations.len() as u64;
        let successful = invocations.iter().filter(|i| i.success).count() as u64;
        let failed = total - successful;

        // Latency calculations
        let mut latencies: Vec<u64> = invocations.iter().map(|i| i.total_latency_ms).collect();
        latencies.sort_unstable();
        let avg_latency = latencies.iter().sum::<u64>() as f64 / total as f64;
        let p95_idx = (total as f64 * 0.95) as usize;
        let p95_latency = latencies
            .get(p95_idx.min(latencies.len() - 1))
            .copied()
            .unwrap_or(0);

        // Token calculations
        let total_tokens: u64 = invocations.iter().map(|i| i.total_tokens as u64).sum();
        let avg_tokens = total_tokens as f64 / total as f64;

        // Tool statistics
        let mut tool_successes: HashMap<String, (u64, u64)> = HashMap::new();
        let mut total_tool_calls = 0u64;
        for inv in invocations {
            for tool in &inv.tool_calls {
                total_tool_calls += 1;
                let entry = tool_successes
                    .entry(tool.tool_name.clone())
                    .or_insert((0, 0));
                entry.1 += 1; // Total
                if tool.success {
                    entry.0 += 1; // Successes
                }
            }
        }

        let tool_success_rates: HashMap<String, f32> = tool_successes
            .into_iter()
            .map(|(name, (success, total))| (name, success as f32 / total as f32))
            .collect();

        // Average confidence
        let confidence_sum: f32 = invocations.iter().map(|i| i.confidence).sum();
        let avg_confidence = confidence_sum / total as f32;

        Self {
            total_invocations: total,
            successful_invocations: successful,
            failed_invocations: failed,
            success_rate: successful as f32 / total as f32,
            avg_latency_ms: avg_latency,
            p95_latency_ms: p95_latency,
            total_tokens,
            avg_tokens_per_invocation: avg_tokens,
            total_tool_calls,
            tool_success_rates,
            avg_confidence,
        }
    }
}

// =============================================================================
// Tracing / Spans
// =============================================================================

/// Span types for tracing
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SpanType {
    /// Full invocation (user request lifecycle)
    Invocation,
    /// Individual agent execution
    AgentRun,
    /// LLM API call
    LlmCall,
    /// Tool execution
    ToolExecution,
    /// Guardrail check
    GuardrailCheck,
    /// Workflow step
    WorkflowStep,
}

/// A tracing span representing a unit of work
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Span {
    /// Unique span ID
    pub span_id: String,
    /// Parent span ID (None for root spans)
    pub parent_id: Option<String>,
    /// Trace ID (shared across all spans in an invocation)
    pub trace_id: String,
    /// Span type
    pub span_type: SpanType,
    /// Span name (e.g., agent name, tool name)
    pub name: String,
    /// Start timestamp (ms since epoch)
    pub start_time: u64,
    /// End timestamp (ms since epoch)
    pub end_time: Option<u64>,
    /// Duration in milliseconds
    pub duration_ms: Option<u64>,
    /// Status (success, error)
    pub status: SpanStatus,
    /// Custom attributes
    pub attributes: HashMap<String, serde_json::Value>,
}

/// Span status
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub enum SpanStatus {
    #[default]
    InProgress,
    Success,
    Error(String),
}

impl Span {
    /// Create a new root span
    pub fn root(trace_id: impl Into<String>, span_type: SpanType, name: impl Into<String>) -> Self {
        Self {
            span_id: generate_span_id(),
            parent_id: None,
            trace_id: trace_id.into(),
            span_type,
            name: name.into(),
            start_time: current_timestamp_ms(),
            end_time: None,
            duration_ms: None,
            status: SpanStatus::InProgress,
            attributes: HashMap::new(),
        }
    }

    /// Create a child span
    pub fn child(&self, span_type: SpanType, name: impl Into<String>) -> Self {
        Self {
            span_id: generate_span_id(),
            parent_id: Some(self.span_id.clone()),
            trace_id: self.trace_id.clone(),
            span_type,
            name: name.into(),
            start_time: current_timestamp_ms(),
            end_time: None,
            duration_ms: None,
            status: SpanStatus::InProgress,
            attributes: HashMap::new(),
        }
    }

    /// Add an attribute to the span
    pub fn with_attribute<K: Into<String>, V: Into<serde_json::Value>>(
        mut self,
        key: K,
        value: V,
    ) -> Self {
        self.attributes.insert(key.into(), value.into());
        self
    }

    /// Complete the span successfully
    pub fn complete_success(mut self) -> Self {
        let now = current_timestamp_ms();
        self.end_time = Some(now);
        self.duration_ms = Some(now.saturating_sub(self.start_time));
        self.status = SpanStatus::Success;
        self
    }

    /// Complete the span with an error
    pub fn complete_error(mut self, error: impl Into<String>) -> Self {
        let now = current_timestamp_ms();
        self.end_time = Some(now);
        self.duration_ms = Some(now.saturating_sub(self.start_time));
        self.status = SpanStatus::Error(error.into());
        self
    }
}

// =============================================================================
// Metrics Collector
// =============================================================================

/// Thread-safe metrics collector
pub struct MetricsCollector {
    /// Recent invocation metrics (ring buffer)
    invocations: Mutex<Vec<InvocationMetrics>>,
    /// Maximum invocations to keep
    max_invocations: usize,
    /// Total invocation count (atomic for fast access)
    total_count: AtomicU64,
    /// Success count
    success_count: AtomicU64,
    /// Total tokens used
    total_tokens: AtomicU64,
}

impl Default for MetricsCollector {
    fn default() -> Self {
        Self::new(1000)
    }
}

impl MetricsCollector {
    /// Create a new metrics collector with specified capacity
    pub fn new(max_invocations: usize) -> Self {
        Self {
            invocations: Mutex::new(Vec::with_capacity(max_invocations)),
            max_invocations,
            total_count: AtomicU64::new(0),
            success_count: AtomicU64::new(0),
            total_tokens: AtomicU64::new(0),
        }
    }

    /// Record an invocation's metrics
    pub fn record(&self, metrics: InvocationMetrics) {
        // Update atomic counters
        self.total_count.fetch_add(1, Ordering::Relaxed);
        if metrics.success {
            self.success_count.fetch_add(1, Ordering::Relaxed);
        }
        self.total_tokens
            .fetch_add(metrics.total_tokens as u64, Ordering::Relaxed);

        // Add to ring buffer
        if let Ok(mut invocations) = self.invocations.lock() {
            if invocations.len() >= self.max_invocations {
                invocations.remove(0);
            }
            invocations.push(metrics);
        }
    }

    /// Get aggregate metrics
    pub fn get_aggregate(&self) -> AggregateMetrics {
        if let Ok(invocations) = self.invocations.lock() {
            AggregateMetrics::from_invocations(&invocations)
        } else {
            AggregateMetrics::default()
        }
    }

    /// Get quick stats (from atomic counters - fast)
    pub fn get_quick_stats(&self) -> QuickStats {
        let total = self.total_count.load(Ordering::Relaxed);
        let success = self.success_count.load(Ordering::Relaxed);
        let tokens = self.total_tokens.load(Ordering::Relaxed);
        QuickStats {
            total_invocations: total,
            successful_invocations: success,
            success_rate: if total > 0 {
                success as f32 / total as f32
            } else {
                0.0
            },
            total_tokens: tokens,
        }
    }

    /// Get recent invocations
    pub fn get_recent(&self, limit: usize) -> Vec<InvocationMetrics> {
        if let Ok(invocations) = self.invocations.lock() {
            invocations.iter().rev().take(limit).cloned().collect()
        } else {
            Vec::new()
        }
    }

    /// Reset all metrics
    pub fn reset(&self) {
        self.total_count.store(0, Ordering::Relaxed);
        self.success_count.store(0, Ordering::Relaxed);
        self.total_tokens.store(0, Ordering::Relaxed);
        if let Ok(mut invocations) = self.invocations.lock() {
            invocations.clear();
        }
    }
}

/// Quick stats from atomic counters
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuickStats {
    pub total_invocations: u64,
    pub successful_invocations: u64,
    pub success_rate: f32,
    pub total_tokens: u64,
}

// =============================================================================
// Retry Logic
// =============================================================================

/// Configuration for retry logic
#[derive(Debug, Clone)]
pub struct RetryConfig {
    /// Maximum number of retry attempts
    pub max_retries: u32,
    /// Initial delay between retries (ms)
    pub initial_delay_ms: u64,
    /// Maximum delay between retries (ms)
    pub max_delay_ms: u64,
    /// Backoff multiplier (e.g., 2.0 for exponential)
    pub backoff_multiplier: f64,
    /// Whether to throw on max retries exceeded
    pub throw_on_exceed: bool,
}

impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            max_retries: 3,
            initial_delay_ms: 100,
            max_delay_ms: 5000,
            backoff_multiplier: 2.0,
            throw_on_exceed: true,
        }
    }
}

impl RetryConfig {
    /// Calculate delay for a given attempt (0-indexed)
    pub fn delay_for_attempt(&self, attempt: u32) -> Duration {
        let delay = self.initial_delay_ms as f64 * self.backoff_multiplier.powi(attempt as i32);
        let capped = (delay as u64).min(self.max_delay_ms);
        Duration::from_millis(capped)
    }
}

/// Result of a retry operation
#[derive(Debug)]
pub enum RetryResult<T, E> {
    /// Operation succeeded
    Success(T),
    /// Operation failed after all retries
    Failed(E),
    /// Retries exceeded but throw_on_exceed was false
    Exhausted,
}

/// Execute an async operation with retry logic
pub async fn with_retry<T, E, F, Fut>(config: &RetryConfig, operation: F) -> RetryResult<T, E>
where
    F: Fn() -> Fut,
    Fut: std::future::Future<Output = Result<T, E>>,
    E: std::fmt::Debug,
{
    let mut attempts = 0;

    loop {
        match operation().await {
            Ok(result) => return RetryResult::Success(result),
            Err(e) if attempts < config.max_retries => {
                attempts += 1;
                let delay = config.delay_for_attempt(attempts - 1);
                tracing::warn!(
                    "Retry {}/{}: {:?} (waiting {:?})",
                    attempts,
                    config.max_retries,
                    e,
                    delay
                );
                tokio::time::sleep(delay).await;
            }
            Err(e) => {
                if config.throw_on_exceed {
                    return RetryResult::Failed(e);
                }
                tracing::error!("All {} retries exhausted: {:?}", config.max_retries, e);
                return RetryResult::Exhausted;
            }
        }
    }
}

// =============================================================================
// Helpers
// =============================================================================

static SPAN_COUNTER: AtomicU64 = AtomicU64::new(0);

fn generate_span_id() -> String {
    let counter = SPAN_COUNTER.fetch_add(1, Ordering::SeqCst);
    let timestamp = current_timestamp_ms();
    format!("span_{}_{}", timestamp, counter)
}

fn current_timestamp_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_invocation_metrics() {
        let metrics = InvocationMetrics::new("inv_001", "Observer")
            .with_llm_metrics(150, 100, 50)
            .complete_success(200, 0.9);

        assert!(metrics.success);
        assert_eq!(metrics.total_latency_ms, 200);
        assert_eq!(metrics.total_tokens, 150);
        assert_eq!(metrics.confidence, 0.9);
    }

    #[test]
    fn test_aggregate_metrics() {
        let invocations = vec![
            InvocationMetrics::new("inv_001", "Observer").complete_success(100, 0.8),
            InvocationMetrics::new("inv_002", "Observer").complete_success(200, 0.9),
            InvocationMetrics::new("inv_003", "Observer").complete_failure(50, "timeout"),
        ];

        let aggregate = AggregateMetrics::from_invocations(&invocations);

        assert_eq!(aggregate.total_invocations, 3);
        assert_eq!(aggregate.successful_invocations, 2);
        assert_eq!(aggregate.failed_invocations, 1);
        assert!((aggregate.success_rate - 0.666).abs() < 0.01);
    }

    #[test]
    fn test_span_hierarchy() {
        let root = Span::root("trace_001", SpanType::Invocation, "main");
        let child = root.child(SpanType::AgentRun, "Observer");
        let grandchild = child.child(SpanType::LlmCall, "generate");

        assert!(root.parent_id.is_none());
        assert_eq!(child.parent_id, Some(root.span_id.clone()));
        assert_eq!(grandchild.parent_id, Some(child.span_id.clone()));
        assert_eq!(grandchild.trace_id, root.trace_id);
    }

    #[test]
    fn test_metrics_collector() {
        let collector = MetricsCollector::new(100);

        collector.record(InvocationMetrics::new("inv_001", "Observer").complete_success(100, 0.8));
        collector.record(InvocationMetrics::new("inv_002", "Observer").complete_success(200, 0.9));

        let stats = collector.get_quick_stats();
        assert_eq!(stats.total_invocations, 2);
        assert_eq!(stats.successful_invocations, 2);
        assert_eq!(stats.success_rate, 1.0);
    }

    #[test]
    fn test_retry_config_delay() {
        let config = RetryConfig {
            initial_delay_ms: 100,
            backoff_multiplier: 2.0,
            max_delay_ms: 1000,
            ..Default::default()
        };

        assert_eq!(config.delay_for_attempt(0), Duration::from_millis(100));
        assert_eq!(config.delay_for_attempt(1), Duration::from_millis(200));
        assert_eq!(config.delay_for_attempt(2), Duration::from_millis(400));
        assert_eq!(config.delay_for_attempt(3), Duration::from_millis(800));
        assert_eq!(config.delay_for_attempt(4), Duration::from_millis(1000)); // Capped
    }

    #[tokio::test]
    async fn test_with_retry_success() {
        let config = RetryConfig::default();
        let attempts = std::sync::atomic::AtomicU32::new(0);

        let result = with_retry(&config, || {
            let current = attempts.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            async move {
                if current < 1 {
                    Err("temporary failure")
                } else {
                    Ok("success")
                }
            }
        })
        .await;

        match result {
            RetryResult::Success(v) => assert_eq!(v, "success"),
            _ => panic!("Expected success"),
        }
    }
}
