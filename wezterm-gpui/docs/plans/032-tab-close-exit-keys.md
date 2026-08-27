# Tab close / `exit` kills typing (032)

User 2026-08-27 after **030 user-ok**: closing a tab (tab **X**, or typing `exit`) leaves chrome clickable (Plus, dropdown) but **typing does not work**, including in a newly opened tab. Not a full UI freeze.

## Causes

1. **`exit`:** default `exit_behavior` is `Close`. Mux removes the pane; the ShellTab stayed. Further keys called `LocalPane::key_down` → ConPTY `write_all`, which can block or no-op. The tab was not closed.
2. **Tab X:** confirm dialog’s previous focus is the Close button. After the tab is removed that handle is dead; gpui-component’s 250ms restore does not put focus on AppShell. `on_term_key` only runs while AppShell is focused (022). Plus still works because it is a click.

## In this slice

- Skip `key_down` when `pane.is_dead()`.
- Mux wake uses `try_send` (notify holds `subscribers.write()`).
- `TermPaneEvent::Exited` → close that tab; **last tab quits the app** (wezterm-gui Close + `quit_when_all_windows_are_closed`; user 2026-08-27 after first 032 try).
- After tab close / new tab: `request_terminal_focus` (`focus_pending` + 300ms retry).

User 2026-08-27: first 032 try “it's better now”; last-tab `exit` is 033 (quit, not spawn).

## Out

- 026 monitor-move hang, 031 cursor until backspace
- lua `exit_behavior` / Hold overlay text
- Confirm-less X (still confirms when 2+ tabs)

Record: `docs/decisions/032-tab-close-exit-keys.json`.
