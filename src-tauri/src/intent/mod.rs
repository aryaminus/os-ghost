//! Intent module - intent recognition, autorun, and idle detection

pub mod idle_detection;
pub mod intent;
pub mod intent_autorun;

pub use idle_detection::{IdleDetector, IdleState};
