//! Input Safety Checker
//!
//! Validates input actions to prevent dangerous operations:
//! - Screen edge detection (prevents moving mouse off-screen)
//! - Dangerous key combo detection (e.g., Cmd+Q, Alt+F4)
//! - Sensitive text pattern detection
//! - Rate limiting
//! - Coordinate validation

use crate::input::Key;

/// Safety checker for input operations
pub struct InputSafetyChecker {
    screen_width: i32,
    screen_height: i32,
    last_action_time: std::time::Instant,
    min_action_interval_ms: u64,
}

impl InputSafetyChecker {
    /// Create a new safety checker
    pub fn new() -> Self {
        // Get screen dimensions
        let (width, height) = Self::get_screen_dimensions();

        Self {
            screen_width: width,
            screen_height: height,
            // Set last_action_time in the past to avoid rate limit on first call
            last_action_time: std::time::Instant::now() - std::time::Duration::from_millis(100),
            min_action_interval_ms: 10, // Minimum 10ms between actions
        }
    }

    /// Create a new safety checker with custom screen dimensions (for testing)
    #[cfg(test)]
    pub fn new_with_dimensions(width: i32, height: i32) -> Self {
        Self {
            screen_width: width,
            screen_height: height,
            last_action_time: std::time::Instant::now() - std::time::Duration::from_millis(100),
            min_action_interval_ms: 10,
        }
    }

    /// Validate mouse move coordinates
    pub fn validate_mouse_move(&self, x: i32, y: i32) -> Result<(), String> {
        // Check if coordinates are within screen bounds (with some margin)
        let margin = 10;

        if x < -margin || x > self.screen_width + margin {
            return Err(format!(
                "X coordinate {} is outside screen bounds (0-{})",
                x, self.screen_width
            ));
        }

        if y < -margin || y > self.screen_height + margin {
            return Err(format!(
                "Y coordinate {} is outside screen bounds (0-{})",
                y, self.screen_height
            ));
        }

        // Check rate limiting
        self.check_rate_limit()?;

        Ok(())
    }

    /// Validate text input for sensitive patterns
    pub fn validate_text_input(&self, text: &str) -> Result<(), String> {
        // Check for common password patterns
        let sensitive_patterns = [
            "password",
            "passwd",
            "pwd",
            "secret",
            "token",
            "api_key",
            "private_key",
            "secret_key",
        ];

        let lower_text = text.to_lowercase();

        for pattern in &sensitive_patterns {
            if lower_text.contains(pattern) {
                return Err(format!(
                    "Potentially sensitive content detected: {}",
                    pattern
                ));
            }
        }

        // Check for credit card patterns (simple regex-like check)
        if Self::looks_like_credit_card(text) {
            return Err("Potential credit card number detected".to_string());
        }

        // Check for SSN patterns
        if Self::looks_like_ssn(text) {
            return Err("Potential SSN detected".to_string());
        }

        Ok(())
    }

    /// Validate key combinations for dangerous shortcuts
    pub fn validate_key_combo(&self, keys: &[Key]) -> Result<(), String> {
        // Check for dangerous combinations
        let dangerous_combos = [
            // Application quit
            (vec![Key::Command, Key::Q], "Quit application"),
            (vec![Key::Control, Key::Q], "Quit application"),
            (vec![Key::Alt, Key::F4], "Close window/quit"),
            // System shutdown
            (
                vec![Key::Command, Key::Control, Key::Power],
                "System shutdown",
            ),
            (
                vec![Key::Control, Key::Alt, Key::Delete],
                "System interrupt",
            ),
            // Force quit
            (vec![Key::Command, Key::Option, Key::Escape], "Force quit"),
            // Sleep
            (vec![Key::Command, Key::Option, Key::Power], "Sleep"),
            // Log out
            (vec![Key::Command, Key::Shift, Key::Q], "Log out"),
            // Lock screen
            (vec![Key::Command, Key::Control, Key::Q], "Lock screen"),
        ];

        for (combo, description) in &dangerous_combos {
            if Self::combo_matches(keys, combo) {
                return Err(format!(
                    "Dangerous key combination detected: {} ({})",
                    Self::format_combo(keys),
                    description
                ));
            }
        }

        // Check rate limiting
        self.check_rate_limit()?;

        Ok(())
    }

    /// Check rate limiting
    fn check_rate_limit(&self) -> Result<(), String> {
        let elapsed = self.last_action_time.elapsed();
        let min_interval = std::time::Duration::from_millis(self.min_action_interval_ms);

        if elapsed < min_interval {
            return Err(format!(
                "Rate limit exceeded. Please wait {}ms between actions",
                self.min_action_interval_ms
            ));
        }

        Ok(())
    }

    /// Update last action time
    pub fn record_action(&mut self) {
        self.last_action_time = std::time::Instant::now();
    }

    /// Check if text looks like a credit card number
    fn looks_like_credit_card(text: &str) -> bool {
        // Simple check: 13-19 digits, possibly with spaces or hyphens
        let digits: String = text.chars().filter(|c| c.is_ascii_digit()).collect();

        if digits.len() < 13 || digits.len() > 19 {
            return false;
        }

        // Luhn algorithm check (simplified)
        Self::luhn_check(&digits)
    }

    /// Luhn algorithm for credit card validation
    fn luhn_check(digits: &str) -> bool {
        let mut sum = 0;
        let mut alternate = false;

        for c in digits.chars().rev() {
            if let Some(mut n) = c.to_digit(10) {
                if alternate {
                    n *= 2;
                    if n > 9 {
                        n -= 9;
                    }
                }
                sum += n;
                alternate = !alternate;
            }
        }

        sum % 10 == 0
    }

    /// Check if text looks like an SSN
    fn looks_like_ssn(text: &str) -> bool {
        // Proper SSN pattern: XXX-XX-XXXX or XXXXXXXXX
        // Must check for the dashes format or validate area/group numbers
        let digits: String = text.chars().filter(|c| c.is_ascii_digit()).collect();

        if digits.len() != 9 {
            return false;
        }

        // Check for valid SSN format (area number cannot be 000, 666, or 900-999)
        // Group number cannot be 00, serial number cannot be 0000
        let area: u32 = digits[0..3].parse().unwrap_or(0);
        let group: u32 = digits[3..5].parse().unwrap_or(0);
        let serial: u32 = digits[5..9].parse().unwrap_or(0);

        (area != 0 && area != 666 && area < 900) && group != 0 && serial != 0
    }

    /// Check if key combo matches
    fn combo_matches(pressed: &[Key], target: &[Key]) -> bool {
        if pressed.len() != target.len() {
            return false;
        }

        // Sort both and compare
        let mut pressed_sorted: Vec<_> = pressed.iter().map(|k| format!("{:?}", k)).collect();
        let mut target_sorted: Vec<_> = target.iter().map(|k| format!("{:?}", k)).collect();

        pressed_sorted.sort();
        target_sorted.sort();

        pressed_sorted == target_sorted
    }

    /// Format key combo for display
    fn format_combo(keys: &[Key]) -> String {
        keys.iter()
            .map(|k| format!("{:?}", k))
            .collect::<Vec<_>>()
            .join("+")
    }

    /// Get screen dimensions
    #[cfg(target_os = "macos")]
    fn get_screen_dimensions() -> (i32, i32) {
        use core_graphics::display::CGDisplay;
        let display = CGDisplay::main();
        let bounds = display.bounds();
        (bounds.size.width as i32, bounds.size.height as i32)
    }

    #[cfg(target_os = "windows")]
    fn get_screen_dimensions() -> (i32, i32) {
        unsafe {
            use windows::Win32::Foundation::HWND;
            use windows::Win32::Graphics::Gdi::{
                GetDC, GetDeviceCaps, ReleaseDC, HORZRES, VERTRES,
            };

            let hwnd = HWND(std::ptr::null_mut());
            let hdc = GetDC(hwnd);
            let width = GetDeviceCaps(hdc, HORZRES);
            let height = GetDeviceCaps(hdc, VERTRES);
            ReleaseDC(hwnd, hdc);

            (width, height)
        }
    }

    #[cfg(target_os = "linux")]
    fn get_screen_dimensions() -> (i32, i32) {
        use x11rb::connection::Connection;

        if let Ok((conn, _)) = x11rb::connect(None) {
            let screen = &conn.setup().roots[0];
            (
                screen.width_in_pixels as i32,
                screen.height_in_pixels as i32,
            )
        } else {
            (1920, 1080) // Fallback
        }
    }

    #[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
    fn get_screen_dimensions() -> (i32, i32) {
        (1920, 1080) // Default fallback
    }
}

impl Default for InputSafetyChecker {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_coordinates() {
        // Use custom dimensions for predictable testing
        let checker = InputSafetyChecker::new_with_dimensions(1920, 1080);

        // Valid coordinates
        assert!(checker.validate_mouse_move(100, 100).is_ok());

        // Out of bounds (using large negative values)
        assert!(checker.validate_mouse_move(-1000, 100).is_err());
        assert!(checker.validate_mouse_move(100, -1000).is_err());
    }

    #[test]
    fn test_sensitive_text_detection() {
        let checker = InputSafetyChecker::new_with_dimensions(1920, 1080);

        // Normal text
        assert!(checker.validate_text_input("Hello world").is_ok());

        // Sensitive patterns
        assert!(checker.validate_text_input("my password is").is_err());
        assert!(checker.validate_text_input("api_key=123").is_err());
    }

    #[test]
    fn test_dangerous_combos() {
        let checker = InputSafetyChecker::new_with_dimensions(1920, 1080);

        // Normal combo
        assert!(checker.validate_key_combo(&[Key::Command, Key::C]).is_ok());

        // Dangerous combo
        assert!(checker.validate_key_combo(&[Key::Command, Key::Q]).is_err());
    }

    #[test]
    fn test_luhn_check() {
        // Valid credit card (test number)
        assert!(InputSafetyChecker::luhn_check("4532015112830366"));

        // Invalid
        assert!(!InputSafetyChecker::luhn_check("4532015112830367"));
    }
}
