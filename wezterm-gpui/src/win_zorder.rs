//! Window z-order for palette AlwaysOnTop / AlwaysOnBottom / Normal.
//!
//! GPUI has no `set_window_level`. wezterm-gui's `window` crate is a no-op
//! on Windows (default trait method). POC uses Win32 `HWND_TOPMOST` /
//! `HWND_NOTOPMOST` / `HWND_BOTTOM` via the GPUI HWND.

use gpui::Window;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WindowZOrder {
    Normal,
    AlwaysOnTop,
    AlwaysOnBottom,
}

impl WindowZOrder {
    pub fn from_palette_id(id: &str) -> Option<Self> {
        match id {
            "SetWindowLevel.Normal" => Some(Self::Normal),
            "SetWindowLevel.AlwaysOnTop" => Some(Self::AlwaysOnTop),
            "SetWindowLevel.AlwaysOnBottom" => Some(Self::AlwaysOnBottom),
            _ => None,
        }
    }

    pub fn toggle_top(self) -> Self {
        match self {
            Self::AlwaysOnTop => Self::Normal,
            Self::AlwaysOnBottom | Self::Normal => Self::AlwaysOnTop,
        }
    }

    pub fn toggle_bottom(self) -> Self {
        match self {
            Self::AlwaysOnBottom => Self::Normal,
            Self::AlwaysOnTop | Self::Normal => Self::AlwaysOnBottom,
        }
    }
}

pub fn apply(window: &Window, level: WindowZOrder) {
    #[cfg(windows)]
    windows::apply(window, level);
    #[cfg(not(windows))]
    let _ = (window, level);
}

#[cfg(windows)]
mod windows {
    use super::WindowZOrder;
    use gpui::Window;
    use raw_window_handle::RawWindowHandle;
    use std::ffi::c_void;

    type Hwnd = *mut c_void;

    const HWND_TOPMOST: isize = -1;
    const HWND_NOTOPMOST: isize = -2;
    const HWND_BOTTOM: isize = 1;
    const SWP_NOSIZE: u32 = 0x0001;
    const SWP_NOMOVE: u32 = 0x0002;
    const SWP_NOACTIVATE: u32 = 0x0010;

    unsafe extern "system" {
        fn SetWindowPos(
            hwnd: Hwnd,
            insert_after: Hwnd,
            x: i32,
            y: i32,
            cx: i32,
            cy: i32,
            flags: u32,
        ) -> i32;
    }

    pub fn apply(window: &Window, level: WindowZOrder) {
        let Ok(handle) = <Window as raw_window_handle::HasWindowHandle>::window_handle(window)
        else {
            return;
        };
        let RawWindowHandle::Win32(win32) = handle.as_raw() else {
            return;
        };
        let hwnd = win32.hwnd.get() as Hwnd;
        let after = match level {
            WindowZOrder::AlwaysOnTop => HWND_TOPMOST as Hwnd,
            WindowZOrder::AlwaysOnBottom => HWND_BOTTOM as Hwnd,
            WindowZOrder::Normal => HWND_NOTOPMOST as Hwnd,
        };
        unsafe {
            SetWindowPos(hwnd, after, 0, 0, 0, 0, SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE);
        }
    }
}
