# Palette catalog complete (049)

User after **048** (and earlier “continue on command palette items”): **wire every remaining command-palette row in one shot.** Stop only when the static catalog has no dimmed rows.

## This slice

Every `PALETTE_COMMANDS` id is `Wired`. The leftover 30 listed rows:

| Cluster | GPUI surface |
|---|---|
| SpawnWindow / ActivateWindow.0–9 / Relative ±1 | Second GPUI HWND; `cx.windows()` activate |
| ReloadConfiguration | `config::reload()` + rebuild painters |
| PrimarySelection copy/paste | Clipboard on Windows (no X11 primary) |
| DetachDomain.CurrentPaneDomain | Notify: local domain cannot detach |
| ClearKeyTableStack | Notify: no InputMap yet |
| ShowLauncher / ShowTabNavigator | Shared searchable `Picker` overlay |
| CharSelect / QuickSelect / Search / ShowDebugOverlay | Same picker |
| ActivateCopyMode | Simplified hjkl/v/y on TermPane (not termwiz CopyOverlay) |
| PaneSelect.* | Picker over GPUI split leaves; extract/swap/move |

POC shortcuts (not wezterm-gui parity):

- Search is a picker of **visible lines**, not incremental scrollback search. Ctrl+Shift+F stays the FPS HUD (019).
- Copy mode is a **small hjkl/v/y** mode, not the full copy_mode key table.
- Debug overlay is a **dump + copy**, not a lua REPL.
- Launcher includes shells + lua `launch_menu` + split/window; not WSL/unix/workspaces (mux LocalDomain only).
- Primary selection aliases the clipboard.

Stay parked: 026 monitor-move hang, 031 cursor until backspace, `window/` cutover.

Needs user-try of the newly wired overlays (New Window, launcher, char select, copy mode, search, pane select, reload).

Record: `docs/decisions/049-palette-complete.json`.
