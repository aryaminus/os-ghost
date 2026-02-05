//! Capture module - handles screen capture and change detection

#[allow(clippy::module_inception)]
pub mod capture;
pub mod change_detection;
pub mod vision;

// Re-export commonly used types
pub use capture::{
    capture_primary_monitor, capture_primary_monitor_raw, capture_screen, CaptureSettings,
};
pub use change_detection::{ChangeDetectionConfig, ChangeDetector, ChangeResult};
pub use vision::{AnalyzedScreenshot, ElementMatch, VisionCapture};
