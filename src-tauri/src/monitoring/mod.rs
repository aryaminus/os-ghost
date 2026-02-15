//! Monitoring module - screen capture and observation

pub mod app_context;
pub mod monitor;
pub mod perf;
pub mod types;

// Re-export commonly used types
pub use app_context::{AppCategory, AppContext, AppContextDetector, AppSwitchEvent};
pub use types::{
    with_retry, AggregateMetrics, InvocationMetrics, MetricsCollector, RetryConfig, RetryResult,
    Span, SpanStatus, SpanType, ToolCallRecord,
};

// Activity tracker temporarily disabled - requires rdev dependency not in Cargo.toml
// pub mod activity_tracker;
