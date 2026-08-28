# Overlay parity + scrollbar (050)

User after trying **049** overlays against official wezterm-gui screenshots.

## This slice

| Item | GPUI surface |
|---|---|
| New Window overlap | Offset origin +48px from the current HWND; reuse current size |
| Activate Window 1–10 | Defer `activate_window` + Win32 `SetForegroundWindow` (re-entrant `h.update` was a no-op) |
| Search pane output | Vim-style bar at the bottom; yellow current / pink other hits in the pane. **Ctrl+Shift+F** |
| Quick Select | Letter labels on visible regex matches (wezterm-gui `PATTERNS`); type the label to copy |
| Pane Select | Centered letter badge on each split leaf (`quick_select_alphabet`) |
| Debug overlay | Notify only — lua REPL later |
| FPS HUD | Palette **Toggle FPS HUD** only; no shortcut |
| `enable_scroll_bar` | Per-pane rail + thumb (wezterm-gui `ScrollHit` formula) |

The 049 searchable `Picker` for Search / QuickSelect / PaneSelect stays in `picker.rs` behind `find::PICKER_SEARCH_QUICKSELECT_PANESELECT` (default **false**). Revisit later; do not delete it.

POC shortcuts (not wezterm-gui parity):

- Search is case-sensitive by default; **Ctrl+R** toggles ignore-case. Not the full copy_mode search key table / regex mode.
- Quick select uses the same 14 regexes as wezterm-gui; labels are overlays, not a list.
- Debug overlay is still not a lua REPL (backlog).
- Scrollbar is a GPUI rail, not the wezterm-gui OpenGL thumb.

Stay parked: 026 monitor-move hang, 031 cursor until backspace, `window/` cutover.

Needs user-try: New Window offset, Activate Window 2, Ctrl+Shift+F search, Quick Select, pane letter keys, lua scrollbar.

Record: `docs/decisions/050-overlay-parity-and-scrollbar.json`.
