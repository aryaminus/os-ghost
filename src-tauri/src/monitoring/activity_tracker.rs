//! Activity tracker for adaptive, event-driven screenshot capture
//! Uses rdev to detect global mouse/keyboard events and calculate activity levels

use rdev::EventType;
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::mpsc;
use tokio::sync::Notify;

/// Activity state that drives adaptive capture intervals
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ActivityState {
    /// High activity - fast capture (user actively working)
    Active,
    /// Moderate activity - normal capture
    Moderate,
    /// Low activity - slow capture (user reading or doing minor tasks)
    Low,
    /// Idle - very slow or no capture (user away from computer)
    Idle,
}

impl ActivityState {
    /// Get recommended capture interval in seconds based on state
    pub fn recommended_interval_secs(&self) -> u64 {
        match self {
            ActivityState::Active => 10,
            ActivityState::Moderate => 30,
            ActivityState::Low => 60,
            ActivityState::Idle => 300,
        }
    }

    /// Get activity multiplier for interval backoff
    pub fn multiplier(&self) -> f64 {
        match self {
            ActivityState::Active => 1.0,
            ActivityState::Moderate => 2.0,
            ActivityState::Low => 4.0,
            ActivityState::Idle => 8.0,
        }
    }
}

/// Global activity tracker state
#[derive(Debug)]
pub struct ActivityTracker {
    /// Last time any input event was detected
    last_activity: Arc<AtomicU64>,
    /// Count of keyboard events in recent window
    keyboard_count: Arc<AtomicUsize>,
    /// Count of mouse events in recent window
    mouse_count: Arc<AtomicUsize>,
    /// Last keyboard burst timestamp (rapid typing)
    last_keyboard_burst: Arc<AtomicU64>,
    /// Is the tracker running
    running: Arc<AtomicBool>,
    /// Notification when activity state changes
    state_change_notify: Arc<Notify>,
    /// Current activity state
    current_state: Arc<parking_lot::Mutex<ActivityState>>,
}

impl ActivityTracker {
    /// Create a new activity tracker
    pub fn new() -> Self {
        Self {
            last_activity: Arc::new(AtomicU64::new(crate::core::utils::current_timestamp())),
            keyboard_count: Arc::new(AtomicUsize::new(0)),
            mouse_count: Arc::new(AtomicUsize::new(0)),
            last_keyboard_burst: Arc::new(AtomicU64::new(0)),
            running: Arc::new(AtomicBool::new(false)),
            state_change_notify: Arc::new(Notify::new()),
            current_state: Arc::new(parking_lot::Mutex::new(ActivityState::Idle)),
        }
    }

    /// Start listening for global input events
    /// This runs on a separate thread and ONLY sends relevant events to reduce overhead
    pub fn start(&self, tx: mpsc::Sender<()>) -> anyhow::Result<()> {
        self.running.store(true, Ordering::Relaxed);

        let running_clone = self.running.clone();
        let last_activity_clone = self.last_activity.clone();
        let keyboard_count_clone = self.keyboard_count.clone();
        let mouse_count_clone = self.mouse_count.clone();
        let last_keyboard_burst_clone = self.last_keyboard_burst.clone();

        std::thread::spawn(move || {
            let mut keyboard_burst_count = 0;
            let mut burst_start = Instant::now();
            let mut last_mouse_update_ms: u64 = 0;

            if let Err(error) = rdev::listen(move |event| {
                if !running_clone.load(Ordering::Relaxed) {
                    return;
                }

                let event_type = event.event_type;

                let is_keyboard = matches!(
                    event_type,
                    EventType::KeyPress(_) | EventType::KeyRelease(_)
                );
                let is_mouse = matches!(
                    event_type,
                    EventType::MouseMove { .. } | EventType::ButtonPress(_) | EventType::ButtonRelease(_)
                );

                let now = crate::core::utils::current_timestamp();

                if is_keyboard {
                    last_activity_clone.store(now, Ordering::Relaxed);
                    keyboard_count_clone.fetch_add(1, Ordering::Relaxed);

                    // Detect keyboard bursts (rapid typing)
                    if burst_start.elapsed() < Duration::from_secs(3) {
                        keyboard_burst_count += 1;
                        if keyboard_burst_count >= 10 {
                            last_keyboard_burst_clone.store(now, Ordering::Relaxed);
                        }
                    } else {
                        burst_start = Instant::now();
                        keyboard_burst_count = 1;
                    }

                    // Only notify for keyboard events (mouse is throttled below)
                    let _ = tx.blocking_send(());
                }

                if is_mouse {
                    // Throttle mouse movement - only update every 100ms
                    if now.saturating_sub(last_mouse_update_ms) >= 100 {
                        last_activity_clone.store(now, Ordering::Relaxed);
                        mouse_count_clone.fetch_add(1, Ordering::Relaxed);
                        last_mouse_update_ms = now;

                        // Only notify for throttled mouse events
                        let _ = tx.blocking_send(());
                    }
                }

                // Don't send the full event - just a notification that something happened
                // This reduces channel overhead from ~100-1000 msg/sec to ~10-100 msg/sec
            }) {
                tracing::warn!("Activity tracker listener error: {:?}", error);
            }
        });

        tracing::info!("Activity tracker started");
        Ok(())
    }

    /// Stop the activity tracker
    pub fn stop(&self) {
        self.running.store(false, Ordering::Relaxed);
        tracing::info!("Activity tracker stopped");
    }

    /// Get the last activity timestamp
    pub fn last_activity(&self) -> u64 {
        self.last_activity.load(Ordering::Relaxed)
    }

    /// Check if user is currently active (within threshold seconds)
    pub fn is_active(&self, threshold_secs: u64) -> bool {
        let now = crate::core::utils::current_timestamp();
        let last = self.last_activity.load(Ordering::Relaxed);
        now.saturating_sub(last) <= threshold_secs * 1000
    }

    /// Get keyboard event count (resets periodically)
    pub fn keyboard_count(&self) -> usize {
        self.keyboard_count.load(Ordering::Relaxed)
    }

    /// Get mouse event count (resets periodically)
    pub fn mouse_count(&self) -> usize {
        self.mouse_count.load(Ordering::Relaxed)
    }

    /// Reset event counts (call periodically to create sliding windows)
    pub fn reset_counts(&self) {
        self.keyboard_count.store(0, Ordering::Relaxed);
        self.mouse_count.store(0, Ordering::Relaxed);
    }

    /// Check if there was a recent keyboard burst (rapid typing)
    pub fn had_keyboard_burst(&self, within_secs: u64) -> bool {
        let now = crate::core::utils::current_timestamp();
        let last_burst = self.last_keyboard_burst.load(Ordering::Relaxed);
        now.saturating_sub(last_burst) <= within_secs * 1000
    }

    /// Get current activity state based on recent events
    pub fn current_state(&self) -> ActivityState {
        *self.current_state.lock()
    }

    /// Update activity state based on observed metrics
    pub fn update_state(&self, state: ActivityState) {
        let mut current = self.current_state.lock();
        if *current != state {
            tracing::debug!("Activity state changed: {:?} -> {:?}", *current, state);
            *current = state;
            self.state_change_notify.notify_one();
        }
    }

    /// Wait for activity state to change
    pub async fn wait_for_state_change(&self) {
        self.state_change_notify.notified().await;
    }

    /// Calculate activity state based on time since last activity and event counts
    pub fn calculate_state(
        &self,
        idle_threshold_secs: u64,
        low_activity_threshold_secs: u64,
        high_activity_count: usize,
    ) -> ActivityState {
        let now = crate::core::utils::current_timestamp();
        let last = self.last_activity.load(Ordering::Relaxed);
        let idle_ms = now.saturating_sub(last);
        let idle_secs = idle_ms / 1000;

        let key_count = self.keyboard_count.load(Ordering::Relaxed);
        let mouse_count = self.mouse_count.load(Ordering::Relaxed);
        let total_count = key_count + mouse_count;

        if idle_secs > idle_threshold_secs {
            ActivityState::Idle
        } else if total_count >= high_activity_count || self.had_keyboard_burst(10) {
            ActivityState::Active
        } else if idle_secs > low_activity_threshold_secs {
            ActivityState::Low
        } else {
            ActivityState::Moderate
        }
    }
}

impl Default for ActivityTracker {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_activity_state_intervals() {
        assert_eq!(ActivityState::Active.recommended_interval_secs(), 10);
        assert_eq!(ActivityState::Moderate.recommended_interval_secs(), 30);
        assert_eq!(ActivityState::Low.recommended_interval_secs(), 60);
        assert_eq!(ActivityState::Idle.recommended_interval_secs(), 300);
    }

    #[test]
    fn test_activity_multipliers() {
        assert_eq!(ActivityState::Active.multiplier(), 1.0);
        assert_eq!(ActivityState::Moderate.multiplier(), 2.0);
        assert_eq!(ActivityState::Low.multiplier(), 4.0);
        assert_eq!(ActivityState::Idle.multiplier(), 8.0);
    }
}
