//! GPUI HWND registry for mux windows. Workspace switch hides inactive HWNDs.
//!
//! Mux owns the window list and workspace tags. Split geometry stays on AppShell
//! (`SplitLayout`), so we must not destroy HWNDs when switching.

use std::sync::atomic::{AtomicBool, Ordering};

use gpui::*;
use mux::Mux;
use mux::window::WindowId as MuxWindowId;

use crate::win_zorder;

/// One in-flight create / spawn-into-empty-workspace.
static SPAWNING: AtomicBool = AtomicBool::new(false);

#[derive(Clone)]
struct RegisteredHwnd {
    workspace: String,
    mux_window: MuxWindowId,
    handle: AnyWindowHandle,
    /// Win32 HWND; hide/show this directly — `handle.update` is a no-op
    /// while we are still inside that window's update (051/055).
    hwnd: isize,
}

#[derive(Default)]
struct WorkspaceGui {
    windows: Vec<RegisteredHwnd>,
    /// Workspace of the HWND that last painted un-hidden (palette skip list).
    current_view: String,
}

impl Global for WorkspaceGui {}

fn gui(cx: &mut App) -> &mut WorkspaceGui {
    cx.default_global::<WorkspaceGui>()
}

fn gui_ref(cx: &App) -> Option<&WorkspaceGui> {
    cx.try_global::<WorkspaceGui>()
}

pub fn try_begin_spawn() -> bool {
    SPAWNING
        .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
        .is_ok()
}

pub fn end_spawn() {
    SPAWNING.store(false, Ordering::SeqCst);
}

pub struct SpawnGuard;

impl Drop for SpawnGuard {
    fn drop(&mut self) {
        end_spawn();
    }
}

/// Keep the HWND ↔ MuxWindow map current (called from AppShell paint).
pub fn touch(window: &Window, mux_window: MuxWindowId, workspace: &str, cx: &mut App) {
    let handle = window.window_handle();
    let hwnd = win_zorder::raw_hwnd(window).unwrap_or(0);
    let gui = gui(cx);
    if let Some(row) = gui
        .windows
        .iter_mut()
        .find(|w| w.mux_window == mux_window || w.handle == handle)
    {
        row.handle = handle;
        row.hwnd = hwnd;
        row.workspace = workspace.to_string();
        return;
    }
    gui.windows.push(RegisteredHwnd {
        workspace: workspace.to_string(),
        mux_window,
        handle,
        hwnd,
    });
}

pub fn unregister(mux_window: MuxWindowId, cx: &mut App) {
    gui(cx).windows.retain(|w| w.mux_window != mux_window);
}

pub fn hide_all(cx: &mut App) {
    let rows = gui(cx).windows.clone();
    for row in rows {
        apply_hidden(&row, true, cx);
    }
}

/// Show HWNDs tagged with `workspace`; hide the rest. Show/raise first while
/// this process still owns the foreground (hiding first makes Win32 ignore
/// `SetForegroundWindow`). `handle.update` while another window is updating is
/// a no-op (051), so raise is deferred.
pub fn show_workspace(workspace: &str, cx: &mut App) {
    let rows = gui(cx).windows.clone();
    let mut show_rows = Vec::new();
    let mut hide_rows = Vec::new();
    for row in rows {
        if row.workspace == workspace {
            show_rows.push(row);
        } else {
            hide_rows.push(row);
        }
    }
    for row in &show_rows {
        apply_hidden(row, false, cx);
    }
    for row in &hide_rows {
        apply_hidden(row, true, cx);
    }
    let handles: Vec<AnyWindowHandle> = show_rows.iter().map(|r| r.handle).collect();
    if handles.is_empty() {
        return;
    }
    cx.spawn(async move |cx| {
        let _ = cx.update(|cx| {
            for h in handles {
                let _ = h.update(cx, |_, window, _| {
                    window.activate_window();
                    win_zorder::bring_to_front(window);
                });
            }
        });
    })
    .detach();
}

fn apply_hidden(row: &RegisteredHwnd, hidden: bool, cx: &mut App) {
    if row.hwnd != 0 {
        win_zorder::set_hidden_raw(row.hwnd, hidden);
        return;
    }
    let _ = row.handle.update(cx, |_, window, _| {
        win_zorder::set_hidden(window, hidden);
    });
}

pub fn set_current_view(name: &str, cx: &mut App) {
    gui(cx).current_view = name.to_string();
}

pub fn current_view(cx: &App) -> Option<String> {
    gui_ref(cx).and_then(|g| {
        if g.current_view.is_empty() {
            None
        } else {
            Some(g.current_view.clone())
        }
    })
}

pub fn known_names(cx: &App) -> Vec<String> {
    let mut names: Vec<String> = gui_ref(cx)
        .map(|g| g.windows.iter().map(|w| w.workspace.clone()).collect())
        .unwrap_or_default();
    if let Some(mux) = Mux::try_get() {
        names.extend(mux.iter_workspaces());
    }
    names.sort();
    names.dedup();
    names
}

pub fn has_workspace(name: &str, cx: &App) -> bool {
    if gui_ref(cx).is_some_and(|g| g.windows.iter().any(|w| w.workspace == name)) {
        return true;
    }
    Mux::try_get().is_some_and(|m| !m.iter_windows_in_workspace(name).is_empty())
}

/// HWNDs still in the registry (call after `unregister` of the closing window).
pub fn remaining(cx: &App) -> Vec<(String, isize)> {
    gui_ref(cx)
        .map(|g| {
            g.windows
                .iter()
                .map(|w| (w.workspace.clone(), w.hwnd))
                .collect()
        })
        .unwrap_or_default()
}

/// Last GPUI HWND we still know about (hidden workspaces count).
pub fn is_last_hwnd(window: &Window, cx: &App) -> bool {
    let this = win_zorder::raw_hwnd(window).unwrap_or(0);
    let handle = window.window_handle();
    let Some(gui) = gui_ref(cx) else {
        return cx.windows().len() <= 1;
    };
    if gui.windows.is_empty() {
        return cx.windows().len() <= 1;
    }
    gui.windows.iter().all(|row| {
        (this != 0 && row.hwnd == this) || row.handle == handle
    })
}
