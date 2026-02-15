//! Mouse Control Implementation
//!
//! Cross-platform mouse automation:
//! - macOS: CoreGraphics CGEvent
//! - Windows: SendInput
//! - Linux: X11 xtest

use super::{InputError, MouseButton, ScrollDirection};

// ============================================================================
// macOS Implementation
// ============================================================================
#[cfg(target_os = "macos")]
pub mod macos {
    use super::*;

    pub fn move_mouse(_x: i32, _y: i32) -> Result<(), InputError> {
        // TODO: Implement using CoreGraphics
        // Requires proper API setup - stubbed for now
        Err(InputError::PlatformError(
            "macOS mouse control requires accessibility permissions".to_string(),
        ))
    }

    pub fn click_mouse(_button: MouseButton) -> Result<(), InputError> {
        // TODO: Implement using CoreGraphics
        Err(InputError::PlatformError(
            "macOS mouse control requires accessibility permissions".to_string(),
        ))
    }

    pub fn scroll(_direction: ScrollDirection, _amount: i32) -> Result<(), InputError> {
        // TODO: Implement using CoreGraphics
        Err(InputError::PlatformError(
            "macOS mouse control requires accessibility permissions".to_string(),
        ))
    }

    pub fn get_mouse_position() -> Result<(i32, i32), InputError> {
        // TODO: Implement using CoreGraphics
        // Return placeholder
        Ok((0, 0))
    }
}

// ============================================================================
// Windows Implementation
// ============================================================================
#[cfg(target_os = "windows")]
pub mod windows {
    use super::*;
    use ::windows::Win32::Foundation::POINT;
    use ::windows::Win32::UI::Input::KeyboardAndMouse::{
        SendInput, INPUT, INPUT_0, INPUT_MOUSE, MOUSEEVENTF_LEFTDOWN, MOUSEEVENTF_LEFTUP,
        MOUSEEVENTF_MIDDLEDOWN, MOUSEEVENTF_MIDDLEUP, MOUSEEVENTF_MOVE, MOUSEEVENTF_RIGHTDOWN,
        MOUSEEVENTF_RIGHTUP, MOUSEEVENTF_WHEEL, MOUSEINPUT, MOUSE_EVENT_FLAGS,
    };
    use ::windows::Win32::UI::WindowsAndMessaging::GetCursorPos;

    /// Helper to create a mouse INPUT struct
    fn make_mouse_input(dx: i32, dy: i32, mouse_data: u32, flags: MOUSE_EVENT_FLAGS) -> INPUT {
        INPUT {
            r#type: INPUT_MOUSE,
            Anonymous: INPUT_0 {
                mi: MOUSEINPUT {
                    dx,
                    dy,
                    mouseData: mouse_data,
                    dwFlags: flags,
                    time: 0,
                    dwExtraInfo: 0,
                },
            },
        }
    }

    pub fn move_mouse(x: i32, y: i32) -> Result<(), InputError> {
        unsafe {
            let input = make_mouse_input(x, y, 0, MOUSEEVENTF_MOVE);
            SendInput(&[input], std::mem::size_of::<INPUT>() as i32);
            Ok(())
        }
    }

    pub fn click_mouse(button: MouseButton) -> Result<(), InputError> {
        unsafe {
            let (down_flag, up_flag) = match button {
                MouseButton::Left => (MOUSEEVENTF_LEFTDOWN, MOUSEEVENTF_LEFTUP),
                MouseButton::Right => (MOUSEEVENTF_RIGHTDOWN, MOUSEEVENTF_RIGHTUP),
                MouseButton::Middle => (MOUSEEVENTF_MIDDLEDOWN, MOUSEEVENTF_MIDDLEUP),
            };

            // Mouse down
            let down_input = make_mouse_input(0, 0, 0, down_flag);
            SendInput(&[down_input], std::mem::size_of::<INPUT>() as i32);

            // Small delay
            std::thread::sleep(std::time::Duration::from_millis(50));

            // Mouse up
            let up_input = make_mouse_input(0, 0, 0, up_flag);
            SendInput(&[up_input], std::mem::size_of::<INPUT>() as i32);

            Ok(())
        }
    }

    pub fn scroll(direction: ScrollDirection, amount: i32) -> Result<(), InputError> {
        unsafe {
            let wheel_delta = match direction {
                ScrollDirection::Up | ScrollDirection::Down => {
                    let delta = if matches!(direction, ScrollDirection::Up) {
                        amount
                    } else {
                        -amount
                    };
                    (delta * 120) as u32 // WHEEL_DELTA is typically 120
                }
                _ => 0,
            };

            let flags = if matches!(direction, ScrollDirection::Up | ScrollDirection::Down) {
                MOUSEEVENTF_WHEEL
            } else {
                MOUSE_EVENT_FLAGS(0) // Horizontal scroll not implemented yet
            };

            let input = make_mouse_input(0, 0, wheel_delta, flags);
            SendInput(&[input], std::mem::size_of::<INPUT>() as i32);

            Ok(())
        }
    }

    pub fn get_mouse_position() -> Result<(i32, i32), InputError> {
        unsafe {
            let mut point = POINT { x: 0, y: 0 };
            GetCursorPos(&mut point)?;
            Ok((point.x, point.y))
        }
    }
}

// ============================================================================
// Linux Implementation
// ============================================================================
#[cfg(target_os = "linux")]
pub mod linux {
    use super::*;

    pub async fn move_mouse(x: i32, y: i32) -> Result<(), InputError> {
        use x11rb::connection::Connection;
        use x11rb::protocol::xtest::ConnectionExt as XtestConnectionExt;

        let (conn, _) = x11rb::connect(None)?;
        let root = conn.setup().roots[0].root;

        // Move pointer - detail=0 for motion events
        conn.xtest_fake_input(
            x11rb::protocol::xproto::MOTION_NOTIFY_EVENT,
            0, // detail (unused for motion)
            x11rb::CURRENT_TIME,
            root,
            x as i16,
            y as i16,
            0, // deviceid
        )?;

        conn.flush()?;

        Ok(())
    }

    pub async fn click_mouse(button: MouseButton) -> Result<(), InputError> {
        use x11rb::connection::Connection;
        use x11rb::protocol::xtest::ConnectionExt as XtestConnectionExt;

        let (conn, _) = x11rb::connect(None)?;
        let root = conn.setup().roots[0].root;

        let button_num: u8 = match button {
            MouseButton::Left => 1,
            MouseButton::Middle => 2,
            MouseButton::Right => 3,
        };

        // Button press - detail is the button number
        conn.xtest_fake_input(
            x11rb::protocol::xproto::BUTTON_PRESS_EVENT,
            button_num, // detail (button number)
            x11rb::CURRENT_TIME,
            root,
            0,
            0,
            0, // deviceid
        )?;

        conn.flush()?;

        // Small delay
        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

        // Button release
        conn.xtest_fake_input(
            x11rb::protocol::xproto::BUTTON_RELEASE_EVENT,
            button_num, // detail (button number)
            x11rb::CURRENT_TIME,
            root,
            0,
            0,
            0, // deviceid
        )?;

        conn.flush()?;

        Ok(())
    }

    pub async fn scroll(direction: ScrollDirection, amount: i32) -> Result<(), InputError> {
        use x11rb::connection::Connection;
        use x11rb::protocol::xtest::ConnectionExt as XtestConnectionExt;

        let (conn, _) = x11rb::connect(None)?;
        let root = conn.setup().roots[0].root;

        let button: u8 = match direction {
            ScrollDirection::Up => 4,
            ScrollDirection::Down => 5,
            ScrollDirection::Left => 6,
            ScrollDirection::Right => 7,
        };

        for _ in 0..amount {
            // Button press - detail is the button number
            conn.xtest_fake_input(
                x11rb::protocol::xproto::BUTTON_PRESS_EVENT,
                button, // detail (button number)
                x11rb::CURRENT_TIME,
                root,
                0,
                0,
                0, // deviceid
            )?;

            conn.flush()?;

            // Button release
            conn.xtest_fake_input(
                x11rb::protocol::xproto::BUTTON_RELEASE_EVENT,
                button, // detail (button number)
                x11rb::CURRENT_TIME,
                root,
                0,
                0,
                0, // deviceid
            )?;

            conn.flush()?;
        }

        Ok(())
    }

    pub fn get_mouse_position() -> Result<(i32, i32), InputError> {
        use x11rb::connection::Connection;
        use x11rb::protocol::xproto::ConnectionExt as XprotoConnectionExt;

        let (conn, _) = x11rb::connect(None)?;
        let root = conn.setup().roots[0].root;

        let reply = conn.query_pointer(root)?.reply()?;

        Ok((reply.root_x as i32, reply.root_y as i32))
    }
}

// ============================================================================
// Stub for unsupported platforms
// ============================================================================
#[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
pub mod stub {
    use super::*;

    pub fn move_mouse(_x: i32, _y: i32) -> Result<(), InputError> {
        Err(InputError::PlatformError(
            "Platform not supported".to_string(),
        ))
    }

    pub fn click_mouse(_button: MouseButton) -> Result<(), InputError> {
        Err(InputError::PlatformError(
            "Platform not supported".to_string(),
        ))
    }

    pub fn scroll(_direction: ScrollDirection, _amount: i32) -> Result<(), InputError> {
        Err(InputError::PlatformError(
            "Platform not supported".to_string(),
        ))
    }

    pub fn get_mouse_position() -> Result<(i32, i32), InputError> {
        Err(InputError::PlatformError(
            "Platform not supported".to_string(),
        ))
    }
}
