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

/// Hide (workspace switch) without destroying the GPUI tree / mux window.
pub fn set_hidden(window: &Window, hidden: bool) {
    #[cfg(windows)]
    windows::set_hidden(window, hidden);
    #[cfg(not(windows))]
    {
        if hidden {
            window.minimize_window();
        } else {
            window.activate_window();
        }
    }
}

/// Win32 HWND as `isize` (0 if unknown / not Windows).
pub fn raw_hwnd(window: &Window) -> Option<isize> {
    #[cfg(windows)]
    {
        windows::raw_hwnd(window)
    }
    #[cfg(not(windows))]
    {
        let _ = window;
        None
    }
}

/// `ShowWindow` without a GPUI `Window` (nested `handle.update` is a no-op).
pub fn set_hidden_raw(hwnd: isize, hidden: bool) {
    #[cfg(windows)]
    windows::set_hidden_hwnd(hwnd, hidden);
    #[cfg(not(windows))]
    {
        let _ = (hwnd, hidden);
    }
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

/// GPUI Windows never sets `WS_CAPTION` (Zed always uses a client title bar).
/// wezterm-gui `TITLE` is `WS_CAPTION|WS_SYSMENU|WS_MINIMIZEBOX|WS_MAXIMIZEBOX`.
/// Call after the HWND exists so lua `TITLE` gets a real OS caption.
pub fn apply_native_caption(window: &Window, native: bool, resizable: bool) {
    #[cfg(windows)]
    windows::apply_native_caption(window, native, resizable);
    #[cfg(not(windows))]
    {
        let _ = (window, native, resizable);
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
    const SWP_NOZORDER: u32 = 0x0004;
    const SWP_NOACTIVATE: u32 = 0x0010;
    const SWP_FRAMECHANGED: u32 = 0x0020;
    const GWL_STYLE: i32 = -16;
    const WS_CAPTION: isize = 0x00C0_0000;
    const WS_SYSMENU: isize = 0x0008_0000;
    const WS_THICKFRAME: isize = 0x0004_0000;
    const WS_MINIMIZEBOX: isize = 0x0002_0000;
    const WS_MAXIMIZEBOX: isize = 0x0001_0000;

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
        fn BringWindowToTop(hwnd: Hwnd) -> i32;
        fn AllowSetForegroundWindow(process_id: u32) -> i32;
        fn GetWindowRect(hwnd: Hwnd, rect: *mut WinRect) -> i32;
        fn IsWindowVisible(hwnd: Hwnd) -> i32;
        fn ShowWindow(hwnd: Hwnd, n_cmd_show: i32) -> i32;
        fn EnumWindows(cb: unsafe extern "system" fn(Hwnd, isize) -> i32, lparam: isize) -> i32;
        fn GetWindowLongPtrW(hwnd: Hwnd, index: i32) -> isize;
        fn SetWindowLongPtrW(hwnd: Hwnd, index: i32, new_long: isize) -> isize;
    }

    const SW_HIDE: i32 = 0;
    const SW_SHOW: i32 = 5;
    const ASFW_ANY: u32 = 0xFFFFFFFF;

    pub fn set_hidden(window: &Window, hidden: bool) {
        let Some(hwnd) = hwnd_from_window(window) else {
            return;
        };
        unsafe {
            ShowWindow(hwnd, if hidden { SW_HIDE } else { SW_SHOW });
        }
        if !hidden {
            bring_to_front(window);
        }
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
            BringWindowToTop(hwnd);
            // No SWP_SHOWWINDOW: that flag un-hides a workspace HWND we
            // just SW_HIDE'd (055 create was re-showing the source).
            SetWindowPos(
                hwnd,
                HWND_TOP as Hwnd,
                0,
                0,
                0,
                0,
                SWP_NOMOVE | SWP_NOSIZE,
            );
        }
    }

    pub fn raw_hwnd(window: &Window) -> Option<isize> {
        hwnd_from_window(window).map(|h| h as isize)
    }

    pub fn set_hidden_hwnd(hwnd: isize, hidden: bool) {
        if hwnd == 0 {
            return;
        }
        unsafe {
            if hidden {
                ShowWindow(hwnd as Hwnd, SW_HIDE);
            } else {
                // Caller must still own the foreground (show target before
                // hiding the source) or SetForegroundWindow is ignored.
                AllowSetForegroundWindow(ASFW_ANY);
                ShowWindow(hwnd as Hwnd, SW_SHOW);
                BringWindowToTop(hwnd as Hwnd);
                SetForegroundWindow(hwnd as Hwnd);
                SetWindowPos(
                    hwnd as Hwnd,
                    HWND_TOP as Hwnd,
                    0,
                    0,
                    0,
                    0,
                    SWP_NOMOVE | SWP_NOSIZE,
                );
            }
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

    pub fn apply_native_caption(window: &Window, native: bool, resizable: bool) {
        if !native {
            return;
        }
        let Some(hwnd) = hwnd_from_window(window) else {
            return;
        };
        unsafe {
            let mut style = GetWindowLongPtrW(hwnd, GWL_STYLE);
            style |= WS_CAPTION | WS_SYSMENU | WS_MINIMIZEBOX | WS_MAXIMIZEBOX;
            if resizable {
                style |= WS_THICKFRAME;
            } else {
                style &= !WS_THICKFRAME;
            }
            SetWindowLongPtrW(hwnd, GWL_STYLE, style);
            SetWindowPos(
                hwnd,
                std::ptr::null_mut(),
                0,
                0,
                0,
                0,
                SWP_NOMOVE | SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE | SWP_FRAMECHANGED,
            );
        }
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
