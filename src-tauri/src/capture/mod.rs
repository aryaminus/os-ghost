//! Capture module - handles screen capture and change detection

pub mod capture;
pub mod change_detection;

// Re-export commonly used types
pub use capture::{CaptureSettings, capture_primary_monitor, capture_screen};
pub use change_detection::{ChangeDetectionConfig, ChangeDetector, ChangeResult};
