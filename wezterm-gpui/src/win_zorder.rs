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

/// Raise HWND above siblings. `Window::activate_window` is a no-op while we
/// are still inside another window's update (ActivateWindow 1–10).
pub fn bring_to_front(window: &Window) {
    #[cfg(windows)]
    windows::bring_to_front(window);
    #[cfg(not(windows))]
    let _ = window;
}

/// Top-left of visible top-level windows, in screen pixels (Win32 `GetWindowRect`).
pub fn visible_hwnd_origins() -> Vec<(i32, i32)> {
    #[cfg(windows)]
    {
        windows::visible_hwnd_origins()
    }
    #[cfg(not(windows))]
    {
        Vec::new()
    }
}

/// Outer origin of this GPUI window in screen pixels.
pub fn hwnd_origin(window: &Window) -> Option<(i32, i32)> {
    #[cfg(windows)]
    {
        windows::hwnd_origin(window)
    }
    #[cfg(not(windows))]
    {
        let _ = window;
        None
    }
}

#[cfg(windows)]
mod windows {
    use super::WindowZOrder;
    use gpui::Window;
    use raw_window_handle::RawWindowHandle;
    use std::ffi::c_void;

    type Hwnd = *mut c_void;

    const HWND_TOP: isize = 0;
    const HWND_TOPMOST: isize = -1;
    const HWND_NOTOPMOST: isize = -2;
    const HWND_BOTTOM: isize = 1;
    const SWP_NOSIZE: u32 = 0x0001;
    const SWP_NOMOVE: u32 = 0x0002;
    const SWP_NOACTIVATE: u32 = 0x0010;
    const SWP_SHOWWINDOW: u32 = 0x0040;

    #[repr(C)]
    struct WinRect {
        left: i32,
        top: i32,
        right: i32,
        bottom: i32,
    }

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
        fn SetForegroundWindow(hwnd: Hwnd) -> i32;
        fn GetWindowRect(hwnd: Hwnd, rect: *mut WinRect) -> i32;
        fn IsWindowVisible(hwnd: Hwnd) -> i32;
        fn EnumWindows(cb: unsafe extern "system" fn(Hwnd, isize) -> i32, lparam: isize) -> i32;
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

    pub fn bring_to_front(window: &Window) {
        let Ok(handle) = <Window as raw_window_handle::HasWindowHandle>::window_handle(window)
        else {
            return;
        };
        let RawWindowHandle::Win32(win32) = handle.as_raw() else {
            return;
        };
        let hwnd = win32.hwnd.get() as Hwnd;
        unsafe {
            SetForegroundWindow(hwnd);
            SetWindowPos(
                hwnd,
                HWND_TOP as Hwnd,
                0,
                0,
                0,
                0,
                SWP_NOMOVE | SWP_NOSIZE | SWP_SHOWWINDOW,
            );
        }
    }

    fn hwnd_from_window(window: &Window) -> Option<Hwnd> {
        let handle = <Window as raw_window_handle::HasWindowHandle>::window_handle(window).ok()?;
        let RawWindowHandle::Win32(win32) = handle.as_raw() else {
            return None;
        };
        Some(win32.hwnd.get() as Hwnd)
    }

    pub fn hwnd_origin(window: &Window) -> Option<(i32, i32)> {
        let hwnd = hwnd_from_window(window)?;
        let mut rect = WinRect {
            left: 0,
            top: 0,
            right: 0,
            bottom: 0,
        };
        let ok = unsafe { GetWindowRect(hwnd, &mut rect) };
        if ok == 0 {
            return None;
        }
        Some((rect.left, rect.top))
    }

    pub fn visible_hwnd_origins() -> Vec<(i32, i32)> {
        let mut origins = Vec::new();
        unsafe {
            EnumWindows(enum_origins, &mut origins as *mut Vec<(i32, i32)> as isize);
        }
        origins
    }

    unsafe extern "system" fn enum_origins(hwnd: Hwnd, lparam: isize) -> i32 {
        let origins = unsafe { &mut *(lparam as *mut Vec<(i32, i32)>) };
        if unsafe { IsWindowVisible(hwnd) } == 0 {
            return 1;
        }
        let mut rect = WinRect {
            left: 0,
            top: 0,
            right: 0,
            bottom: 0,
        };
        if unsafe { GetWindowRect(hwnd, &mut rect) } == 0 {
            return 1;
        }
        if rect.right - rect.left < 80 || rect.bottom - rect.top < 80 {
            return 1;
        }
        origins.push((rect.left, rect.top));
        1
    }
}
