//! Monitoring module - screen capture and observation

pub mod app_context;
pub mod monitor;
pub mod perf;
pub mod types;

// Re-export commonly used types
pub use app_context::{AppContext, AppContextDetector, AppCategory, AppSwitchEvent};
pub use types::{
    InvocationMetrics, MetricsCollector, ToolCallRecord, AggregateMetrics,
    Span, SpanType, SpanStatus, RetryConfig, RetryResult, with_retry,
};

// Activity tracker temporarily disabled - requires rdev dependency not in Cargo.toml
// pub mod activity_tracker;