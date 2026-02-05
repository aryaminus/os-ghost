//! Intent module - intent recognition, autorun, idle detection, and smart suggestions

pub mod idle_detection;
pub mod intent;
pub mod intent_autorun;
pub mod smart_suggestions;

pub use idle_detection::{IdleDetector, IdleState};
pub use smart_suggestions::{SmartSuggestionEngine, SmartSuggestion, SuggestionTrigger, SuggestionStats};
