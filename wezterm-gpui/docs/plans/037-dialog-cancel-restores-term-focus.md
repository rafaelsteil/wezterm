# Dialog cancel / Ctrl+Q leaves typing dead (037)

User 2026-08-27 after **032 “better now”** / **033 user-ok**: Ctrl+Q (quit confirm) or tab **X** leaves chrome clickable (Plus, new tabs) but **typing does not work**, including in a newly opened tab. Same symptom as 032 `exit`/tab close.

## Cause

032 restored AppShell focus only in the **confirm-close OK** path (`request_terminal_focus` after `close_tab_at`). Gaps:

1. **Cancel / Escape** never restored AppShell. gpui-component’s dialog previous-focus is the tab Close button (the click target). After dismiss that handle is still alive, so the 250ms restore focuses the X, not AppShell. `on_term_key` only runs while AppShell is focused (022).
2. **Ctrl+Q / last-tab X** is `confirm_quit`, which had **no** restore at all (OK calls `cx.quit()`; Cancel left whatever the dialog restored).
3. 032’s 300ms retry only set `focus_pending` + `notify`; it did not `focus_terminal` with a window. Plus / new tab could still lose the race to the 250ms restore.

AlertDialog `.on_close` is **not** usable: `build_surface` copies `button_props` onto the base Dialog and drops a base-level `on_close`. Restore is wired through `.on_ok` / `.on_cancel` instead.

## In this slice

- Focus AppShell **before** opening confirm/prompt so the dialog’s previous-focus target is AppShell, not the tab X.
- `on_close` restore after OK **or** Cancel (`dialog_restore` → `request_terminal_focus`).
- Delayed retry is 400ms via `spawn_in` and actually calls `focus_terminal`.
- Ctrl+Q / Ctrl+W `stop_propagation` so the key does not also go to the PTY.

## Out

- 026 monitor-move hang, 031 cursor until backspace
- Confirm-less X / Ctrl+Q (still confirms)
- Lua `exit_behavior`

**User-ok** 2026-08-29.

Record: `docs/decisions/037-dialog-cancel-restores-term-focus.json`.
