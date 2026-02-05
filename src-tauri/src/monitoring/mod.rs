//! Monitoring module - screen capture and observation

pub mod monitor;
pub mod perf;
pub mod types;

// Re-export commonly used types
pub use types::{
    InvocationMetrics, MetricsCollector, ToolCallRecord, AggregateMetrics,
    Span, SpanType, SpanStatus, RetryConfig, RetryResult, with_retry,
};

// Activity tracker temporarily disabled - requires rdev dependency not in Cargo.toml
// pub mod activity_tracker;