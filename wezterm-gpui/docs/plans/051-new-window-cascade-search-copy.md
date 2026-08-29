# New-window cascade, focus, pane badge, search copy (051)

User after **050**:

1. Later New Window HWNDs stacked on the first offset slot. Cascade until no visible HWND shares that top-left (Win32 `GetWindowRect`).
2. New Window left focus on the source HWND. Defer `activate_window` + `SetForegroundWindow` on the opened handle (same as ActivateWindow 1–10).
3. Pane Select badge is larger (56px square) with `line_height` matching `text_size` so the letter is vertically centered.
4. Palette / Copy while Search is open copies the **current** hit text (not “nothing selected”).

Stay parked: 026, 031, `window/` cutover, debug lua REPL.

Needs user-try of the four items.

**User-ok 2026-08-28.**

Record: `docs/decisions/051-new-window-cascade-search-copy.json`.
