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
        self.setup_macos()?;

        #[cfg(target_os = "windows")]
        self.setup_windows()?;

        Ok(())
    }

    #[cfg(target_os = "macos")]
    fn setup_macos(&self) -> Result<()> {
        use cocoa::appkit::{NSMainMenuWindowLevel, NSWindow, NSWindowCollectionBehavior};
        use cocoa::base::id;

        unsafe {
            let ns_window: id = self.window.ns_window()? as id;

            // Set window level to float above others (main menu level + 1)
            let floating_level = NSMainMenuWindowLevel + 1;
            NSWindow::setLevel_(ns_window, floating_level as i64);

            // Make window appear on all spaces
            let behavior = NSWindowCollectionBehavior::NSWindowCollectionBehaviorCanJoinAllSpaces;
            NSWindow::setCollectionBehavior_(ns_window, behavior);
        }

        Ok(())
    }

    #[cfg(target_os = "windows")]
    fn setup_windows(&self) -> Result<()> {
        use windows::Win32::Foundation::HWND;
        use windows::Win32::UI::WindowsAndMessaging::*;

        unsafe {
            let hwnd = HWND(self.window.hwnd()?.0 as isize);

            // Set extended style for layered window
            let ex_style = GetWindowLongPtrW(hwnd, GWL_EXSTYLE);
            SetWindowLongPtrW(hwnd, GWL_EXSTYLE, ex_style | WS_EX_LAYERED.0 as isize);

            // Set window to topmost
            SetWindowPos(hwnd, HWND_TOPMOST, 0, 0, 0, 0, SWP_NOMOVE | SWP_NOSIZE)?;
        }

        Ok(())
    }

    /// Toggle click-through based on whether user is hovering Ghost
    pub fn set_ignore_cursor_events(&self, ignore: bool) -> Result<()> {
        self.window.set_ignore_cursor_events(ignore)?;
        Ok(())
    }
}

/// Tauri command to toggle click-through
#[tauri::command]
pub fn set_window_clickable(window: Window, clickable: bool) -> Result<(), String> {
    let ghost_window = GhostWindow::new(window);
    ghost_window
        .set_ignore_cursor_events(!clickable)
        .map_err(|e| e.to_string())
}
