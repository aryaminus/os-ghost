//! Window management for the Ghost overlay
//! Handles transparent, always-on-top, click-through window behavior

use anyhow::Result;
use tauri::Window;

pub struct GhostWindow {
    window: Window,
}

impl GhostWindow {
    pub fn new(window: Window) -> Self {
        Self { window }
    }

    /// Initialize window with Ghost-specific settings
    pub fn setup(&self) -> Result<()> {
        // Set always on top
        self.window.set_always_on_top(true)?;

        // Remove decorations for borderless window
        self.window.set_decorations(false)?;

        // Platform-specific transparency setup
        #[cfg(target_os = "macos")]
        if let Err(e) = self.setup_macos() {
            tracing::warn!("Failed to configure macOS specific window settings: {}", e);
        }

        #[cfg(target_os = "windows")]
        if let Err(e) = self.setup_windows() {
            tracing::warn!(
                "Failed to configure Windows specific window settings: {}",
                e
            );
        }

        Ok(())
    }

    #[cfg(target_os = "macos")]
    fn setup_macos(&self) -> Result<()> {
        use objc2::msg_send;
        use objc2::runtime::AnyObject;

        unsafe {
            let ns_window = self.window.ns_window()? as *mut AnyObject;

            // Set window level to floating (level 25 is above main menu level 24)
            // NSMainMenuWindowLevel = 24, we use 25 to float above
            let _: () = msg_send![ns_window, setLevel: 25_i64];

            // Make window appear on all spaces
            // NSWindowCollectionBehaviorCanJoinAllSpaces = 1 << 0 = 1
            let _: () = msg_send![ns_window, setCollectionBehavior: 1_u64];
        }

        Ok(())
    }

    #[cfg(target_os = "windows")]
    fn setup_windows(&self) -> Result<()> {
        use windows::Win32::Foundation::HWND;
        use windows::Win32::UI::WindowsAndMessaging::*;

        unsafe {
            let hwnd = HWND(self.window.hwnd()?.0 as *mut std::ffi::c_void);

            // Set extended style for layered window
            let ex_style = GetWindowLongPtrW(hwnd, GWL_EXSTYLE);
            SetWindowLongPtrW(hwnd, GWL_EXSTYLE, ex_style | WS_EX_LAYERED.0 as isize);

            // Set window to topmost
            SetWindowPos(hwnd, HWND_TOPMOST, 0, 0, 0, 0, SWP_NOMOVE | SWP_NOSIZE)?;
        }

        Ok(())
    }


    /// Position window in bottom-right corner of primary monitor
    pub fn position_bottom_right(&self) -> Result<()> {
        if let Ok(Some(monitor)) = self.window.primary_monitor() {
            let monitor_size = monitor.size();
            let scale = monitor.scale_factor();

            if let Ok(window_size) = self.window.outer_size() {
                // Calculate position with padding from edges
                let padding_x = 20.0;
                let padding_y = 40.0; // Extra padding for taskbar/dock

                let x = (monitor_size.width as f64 / scale)
                    - (window_size.width as f64 / scale)
                    - padding_x;
                let y = (monitor_size.height as f64 / scale)
                    - (window_size.height as f64 / scale)
                    - padding_y;

                self.window
                    .set_position(tauri::Position::Logical(tauri::LogicalPosition::new(x, y)))?;
            }
        }
        Ok(())
    }
}

/// Tauri command to start dragging the window
#[tauri::command]
pub fn start_window_drag(window: Window) -> Result<(), String> {
    window.start_dragging().map_err(|e| e.to_string())
}
