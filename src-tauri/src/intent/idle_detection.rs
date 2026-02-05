//! Idle Detection System
//!
//! Detects when the user is idle (no activity) and triggers smart suggestions.
//! Respects privacy by only tracking idle state, not actual activity content.

use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

/// Idle state information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IdleState {
    /// Whether user is currently idle
    pub is_idle: bool,
    /// How long user has been idle
    pub idle_duration: Duration,
    /// Timestamp when idle started
    pub idle_since: Option<u64>,
    /// Last activity timestamp
    pub last_activity: u64,
}

/// Idle detector monitors system activity
pub struct IdleDetector {
    /// Idle threshold (how long before considered idle)
    idle_threshold: Duration,
    /// Last time activity was detected
    last_activity: Arc<AtomicU64>,
    /// Current idle state
    is_idle: Arc<AtomicBool>,
    /// Idle start timestamp
    idle_since: Arc<AtomicU64>,
    /// Minimum idle time before triggering suggestions
    min_suggestion_idle: Duration,
    /// Whether detection is running
    running: Arc<AtomicBool>,
}

impl IdleDetector {
    /// Create a new idle detector
    pub fn new(idle_threshold_secs: u64) -> Self {
        Self {
            idle_threshold: Duration::from_secs(idle_threshold_secs),
            last_activity: Arc::new(AtomicU64::new(current_timestamp_secs())),
            is_idle: Arc::new(AtomicBool::new(false)),
            idle_since: Arc::new(AtomicU64::new(0)),
            min_suggestion_idle: Duration::from_secs(30), // 30 seconds
            running: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Start monitoring for idle state
    pub async fn start_monitoring(&self) {
        if self.running.swap(true, Ordering::SeqCst) {
            tracing::warn!("Idle detector already running");
            return;
        }

        tracing::info!(
            "Starting idle detection (threshold: {}s)",
            self.idle_threshold.as_secs()
        );

        let last_activity = Arc::clone(&self.last_activity);
        let is_idle = Arc::clone(&self.is_idle);
        let idle_since = Arc::clone(&self.idle_since);
        let idle_threshold = self.idle_threshold;

        // Spawn monitoring task
        tokio::spawn(async move {
            let check_interval = Duration::from_secs(5); // Check every 5 seconds

            loop {
                tokio::time::sleep(check_interval).await;

                // Get last activity time
                let last = last_activity.load(Ordering::SeqCst);
                let now = current_timestamp_secs();
                let elapsed = Duration::from_secs(now - last);

                let currently_idle = elapsed >= idle_threshold;
                let was_idle = is_idle.load(Ordering::SeqCst);

                if currently_idle && !was_idle {
                    // Just became idle
                    is_idle.store(true, Ordering::SeqCst);
                    idle_since.store(now, Ordering::SeqCst);
                    tracing::info!("User became idle (after {:?})", elapsed);
                } else if !currently_idle && was_idle {
                    // No longer idle
                    is_idle.store(false, Ordering::SeqCst);
                    idle_since.store(0, Ordering::SeqCst);
                    tracing::info!("User is no longer idle");
                }
            }
        });
    }

    /// Record activity (resets idle timer)
    pub fn record_activity(&self) {
        let now = current_timestamp_secs();
        self.last_activity.store(now, Ordering::SeqCst);

        // If was idle, mark as active
        if self.is_idle.swap(false, Ordering::SeqCst) {
            tracing::debug!("Activity detected - resetting idle timer");
        }
    }

    /// Check if user is currently idle
    pub fn is_idle(&self) -> bool {
        self.is_idle.load(Ordering::SeqCst)
    }

    /// Check if user has been idle long enough for suggestions
    pub fn is_idle_for_suggestions(&self) -> bool {
        if !self.is_idle() {
            return false;
        }

        let idle_since = self.idle_since.load(Ordering::SeqCst);
        if idle_since == 0 {
            return false;
        }

        let now = current_timestamp_secs();
        let idle_duration = Duration::from_secs(now - idle_since);

        idle_duration >= self.min_suggestion_idle
    }

    /// Get current idle duration
    pub fn get_idle_duration(&self) -> Duration {
        if !self.is_idle() {
            return Duration::from_secs(0);
        }

        let idle_since = self.idle_since.load(Ordering::SeqCst);
        if idle_since == 0 {
            return Duration::from_secs(0);
        }

        let now = current_timestamp_secs();
        Duration::from_secs(now - idle_since)
    }

    /// Get current idle state
    pub fn get_state(&self) -> IdleState {
        IdleState {
            is_idle: self.is_idle(),
            idle_duration: self.get_idle_duration(),
            idle_since: if self.is_idle() {
                Some(self.idle_since.load(Ordering::SeqCst))
            } else {
                None
            },
            last_activity: self.last_activity.load(Ordering::SeqCst),
        }
    }

    /// Update idle threshold
    pub fn set_idle_threshold(&mut self, secs: u64) {
        self.idle_threshold = Duration::from_secs(secs);
        tracing::info!("Idle threshold updated to {}s", secs);
    }

    /// Stop monitoring
    pub fn stop(&self) {
        self.running.store(false, Ordering::SeqCst);
        tracing::info!("Idle detector stopped");
    }
}

impl Default for IdleDetector {
    fn default() -> Self {
        Self::new(60) // Default: 60 seconds
    }
}

/// Get current timestamp in seconds
fn current_timestamp_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs()
}

/// Platform-specific idle time detection
/// Returns milliseconds since last input
#[cfg(target_os = "macos")]
pub async fn get_system_idle_time_ms() -> Option<u64> {
    // Use ioreg or similar to get idle time on macOS
    // For now, return None (not implemented)
    None
}

#[cfg(target_os = "windows")]
pub async fn get_system_idle_time_ms() -> Option<u64> {
    // Use Windows API to get last input info
    // For now, return None (not implemented)
    None
}

#[cfg(target_os = "linux")]
pub async fn get_system_idle_time_ms() -> Option<u64> {
    // Use X11 or similar to get idle time
    // For now, return None (not implemented)
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_idle_detector_creation() {
        let detector = IdleDetector::new(30);
        assert!(!detector.is_idle());
        assert_eq!(detector.get_idle_duration(), Duration::from_secs(0));
    }

    #[test]
    fn test_idle_state_serialization() {
        let state = IdleState {
            is_idle: true,
            idle_duration: Duration::from_secs(60),
            idle_since: Some(1234567890),
            last_activity: 1234567830,
        };

        let json = serde_json::to_string(&state).unwrap();
        assert!(json.contains("\"is_idle\":true"));
    }
}
