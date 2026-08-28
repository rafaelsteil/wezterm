# Palette window chrome (047)

User after **046 user-ok** (“works like a charm”): continue wiring command palette items.

## This slice

Thin `gpui_window` rows:

- Toggle full screen (`Window::toggle_fullscreen`)
- Reset font and window size (lua font size + resize to launch content size; exits fullscreen first)
- Always on top / bottom / normal. GPUI has no `set_window_level`; wezterm-gui is a no-op on Windows. POC uses Win32 `HWND_TOPMOST` / `HWND_NOTOPMOST` / `HWND_BOTTOM` from the GPUI HWND. Bottom is send-to-back, not sticky.

Stay listed: charselect, copy-mode, search, launcher, WSL/domains, SpawnWindow, ActivateWindow, PaneSelect overlay, AdjustPaneSize, reload config, primary selection.

**User-ok** 2026-08-27 (“everything works”).

Record: `docs/decisions/047-palette-window-chrome.json`.
