//! Native OS Input Control Module
//!
//! Provides cross-platform mouse and keyboard automation,
//! matching UI-TARS's pyautogui capabilities.
//!
//! Safety: All input actions require explicit approval based on AutonomyLevel

pub mod mouse;
pub mod keyboard;
pub mod safety;
pub mod desktop_capture;

use std::time::Duration;
use serde::{Deserialize, Serialize};
use tracing::{info, warn, error};

use crate::config::privacy::{AutonomyLevel, PrivacySettings};
use crate::input::safety::InputSafetyChecker;

/// Input controller for native OS automation
pub struct InputController {
    safety_checker: InputSafetyChecker,
    autonomy_level: AutonomyLevel,
    privacy_settings: PrivacySettings,
}

impl InputController {
    /// Create a new input controller
    pub fn new(
        autonomy_level: AutonomyLevel,
        privacy_settings: PrivacySettings,
    ) -> Self {
        Self {
            safety_checker: InputSafetyChecker::new(),
            autonomy_level,
            privacy_settings,
        }
    }

    /// Check if input automation is allowed
    pub fn can_automate(&self) -> bool {
        match self.autonomy_level {
            AutonomyLevel::Observer => false,
            AutonomyLevel::Suggester => true, // With preview
            AutonomyLevel::Supervised => true,
            AutonomyLevel::Autonomous => true,
        }
    }

    /// Move mouse to position
    pub async fn move_mouse(&self, x: i32, y: i32) -> Result<(), InputError> {
        if !self.can_automate() {
            return Err(InputError::NotAllowed(
                "Input automation not allowed in Observer mode".to_string()
            ));
        }

        // Safety check
        if let Err(e) = self.safety_checker.validate_mouse_move(x, y) {
            return Err(InputError::SafetyViolation(e));
        }

        info!("Moving mouse to ({}, {})", x, y);

        #[cfg(target_os = "macos")]
        mouse::macos::move_mouse(x, y)?;

        #[cfg(target_os = "windows")]
        mouse::windows::move_mouse(x, y)?;

        #[cfg(target_os = "linux")]
        mouse::linux::move_mouse(x, y).await?;

        Ok(())
    }

    /// Click mouse button
    pub async fn click_mouse(&self, button: MouseButton) -> Result<(), InputError> {
        if !self.can_automate() {
            return Err(InputError::NotAllowed(
                "Input automation not allowed in Observer mode".to_string()
            ));
        }

        info!("Clicking {:?} mouse button", button);

        #[cfg(target_os = "macos")]
        mouse::macos::click_mouse(button)?;

        #[cfg(target_os = "windows")]
        mouse::windows::click_mouse(button)?;

        #[cfg(target_os = "linux")]
        mouse::linux::click_mouse(button).await?;

        Ok(())
    }

    /// Scroll mouse
    pub async fn scroll(&self, direction: ScrollDirection, amount: i32) -> Result<(), InputError> {
        if !self.can_automate() {
            return Err(InputError::NotAllowed(
                "Input automation not allowed in Observer mode".to_string()
            ));
        }

        info!("Scrolling {:?} by {}", direction, amount);

        #[cfg(target_os = "macos")]
        mouse::macos::scroll(direction, amount)?;

        #[cfg(target_os = "windows")]
        mouse::windows::scroll(direction, amount)?;

        #[cfg(target_os = "linux")]
        mouse::linux::scroll(direction, amount).await?;

        Ok(())
    }

    /// Type text
    pub async fn type_text(&self, text: &str) -> Result<(), InputError> {
        if !self.can_automate() {
            return Err(InputError::NotAllowed(
                "Input automation not allowed in Observer mode".to_string()
            ));
        }

        // Safety check for sensitive content
        if let Err(e) = self.safety_checker.validate_text_input(text) {
            warn!("Potentially sensitive text input detected: {}", e);
            if self.autonomy_level == AutonomyLevel::Supervised {
                return Err(InputError::RequiresApproval(
                    "Text input requires approval".to_string()
                ));
            }
        }

        info!("Typing text (length: {})", text.len());

        #[cfg(target_os = "macos")]
        keyboard::macos::type_text(text)?;

        #[cfg(target_os = "windows")]
        keyboard::windows::type_text(text)?;

        #[cfg(target_os = "linux")]
        keyboard::linux::type_text(text).await?;

        Ok(())
    }

    /// Press key
    pub async fn press_key(&self, key: Key) -> Result<(), InputError> {
        if !self.can_automate() {
            return Err(InputError::NotAllowed(
                "Input automation not allowed in Observer mode".to_string()
            ));
        }

        info!("Pressing key: {:?}", key);

        #[cfg(target_os = "macos")]
        keyboard::macos::press_key(key)?;

        #[cfg(target_os = "windows")]
        keyboard::windows::press_key(key)?;

        #[cfg(target_os = "linux")]
        keyboard::linux::press_key(key).await?;

        Ok(())
    }

    /// Press key combination (e.g., Cmd+C, Ctrl+V)
    pub async fn press_combo(&self, keys: &[Key]) -> Result<(), InputError> {
        if !self.can_automate() {
            return Err(InputError::NotAllowed(
                "Input automation not allowed in Observer mode".to_string()
            ));
        }

        info!("Pressing key combination: {:?}", keys);

        // Safety check for dangerous combos
        if let Err(e) = self.safety_checker.validate_key_combo(keys) {
            return Err(InputError::SafetyViolation(e));
        }

        #[cfg(target_os = "macos")]
        keyboard::macos::press_combo(keys)?;

        #[cfg(target_os = "windows")]
        keyboard::windows::press_combo(keys)?;

        #[cfg(target_os = "linux")]
        keyboard::linux::press_combo(keys).await?;

        Ok(())
    }

    /// Get current mouse position
    pub fn get_mouse_position(&self) -> Result<(i32, i32), InputError> {
        #[cfg(target_os = "macos")]
        return mouse::macos::get_mouse_position();

        #[cfg(target_os = "windows")]
        return mouse::windows::get_mouse_position();

        #[cfg(target_os = "linux")]
        return mouse::linux::get_mouse_position();
    }

    /// Capture desktop screenshot
    pub async fn capture_desktop(&self) -> Result<Vec<u8>, InputError> {
        info!("Capturing desktop screenshot");

        #[cfg(target_os = "macos")]
        return desktop_capture::macos::capture_desktop().await;

        #[cfg(target_os = "windows")]
        return desktop_capture::windows::capture_desktop().await;

        #[cfg(target_os = "linux")]
        return desktop_capture::linux::capture_desktop().await;
    }

    /// Capture specific window
    pub async fn capture_window(&self, window_id: &str) -> Result<Vec<u8>, InputError> {
        info!("Capturing window: {}", window_id);

        #[cfg(target_os = "macos")]
        return desktop_capture::macos::capture_window(window_id).await;

        #[cfg(target_os = "windows")]
        return desktop_capture::windows::capture_window(window_id).await;

        #[cfg(target_os = "linux")]
        return desktop_capture::linux::capture_window(window_id).await;
    }

    /// Get list of windows
    pub async fn list_windows(&self) -> Result<Vec<WindowInfo>, InputError> {
        #[cfg(target_os = "macos")]
        return desktop_capture::macos::list_windows().await;

        #[cfg(target_os = "windows")]
        return desktop_capture::windows::list_windows().await;

        #[cfg(target_os = "linux")]
        return desktop_capture::linux::list_windows().await;
    }
}

/// Mouse button types
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub enum MouseButton {
    Left,
    Right,
    Middle,
}

/// Scroll direction
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub enum ScrollDirection {
    Up,
    Down,
    Left,
    Right,
}

/// Key codes
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub enum Key {
    A, B, C, D, E, F, G, H, I, J, K, L, M,
    N, O, P, Q, R, S, T, U, V, W, X, Y, Z,
    Num0, Num1, Num2, Num3, Num4, Num5, Num6, Num7, Num8, Num9,
    Return, Escape, Backspace, Tab, Space,
    Minus, Equal, LeftBracket, RightBracket,
    Backslash, Semicolon, Quote, Grave,
    Comma, Period, Slash,
    F1, F2, F3, F4, F5, F6, F7, F8, F9, F10, F11, F12,
    Home, End, PageUp, PageDown,
    Left, Right, Up, Down,
    Command, // macOS Cmd
    Control,
    Shift,
    Option, // macOS Alt
    Alt,    // Windows/Linux Alt
    Delete,
    Power,  // Power button
}

/// Window information
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct WindowInfo {
    pub id: String,
    pub title: String,
    pub app_name: String,
    pub bounds: (i32, i32, i32, i32), // x, y, width, height
    pub is_active: bool,
}

/// Input error types
#[derive(Debug, thiserror::Error)]
pub enum InputError {
    #[error("Input automation not allowed: {0}")]
    NotAllowed(String),
    
    #[error("Safety violation: {0}")]
    SafetyViolation(String),
    
    #[error("Requires approval: {0}")]
    RequiresApproval(String),
    
    #[error("Platform error: {0}")]
    PlatformError(String),
    
    #[error("Invalid coordinates: {0}")]
    InvalidCoordinates(String),
    
    #[error("Window not found: {0}")]
    WindowNotFound(String),
    
    #[error("Other error: {0}")]
    Other(String),
}

impl From<std::io::Error> for InputError {
    fn from(err: std::io::Error) -> Self {
        InputError::PlatformError(err.to_string())
    }
}

impl From<anyhow::Error> for InputError {
    fn from(err: anyhow::Error) -> Self {
        InputError::Other(err.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_input_controller_creation() {
        let privacy = PrivacySettings::default();
        let controller = InputController::new(AutonomyLevel::Supervised, privacy);
        assert!(controller.can_automate());

        let controller = InputController::new(AutonomyLevel::Observer, privacy);
        assert!(!controller.can_automate());
    }
}
