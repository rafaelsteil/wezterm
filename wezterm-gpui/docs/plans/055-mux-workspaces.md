# Mux workspaces (055)

User after **054**: implement workspaces. A workspace is a mux window label (tmux-session-like). The GUI shows only MuxWindows tagged with the active name.

## This slice

Mux stays (it is not `window/` / HWND). `AppShell` is a view of one MuxWindow, like wezterm-gui `TermWindow`.

| Piece | How |
|---|---|
| Client | `ClientId::new` + `register_client` + `replace_identity` so `active_workspace` is per-client |
| HWND ↔ MuxWindow | `new_empty_window(active_workspace)` at `AppShell::build`; `workspaces::touch` on paint |
| New GPUI tabs | `Domain::spawn` into that MuxWindow (mux Tab). Split leaves still `spawn_pane` (040 `SplitLayout`) |
| Create workspace | `generate_workspace_name` (no prompt), `set_active_workspace`, hide other HWNDs, spawn one HWND |
| Switch | `set_active_workspace`; if the name has mux windows, `ShowWindow` hide/show; else hide + spawn |
| Last tab / last HWND in a workspace | If another workspace still has MuxWindows, switch there; else 052 (`remove_window` / quit) |

Do **not** teardown AppShell on switch (split geometry lives on the GPUI tree, not mux). Do **not** rewrite 040–046 onto `Mux::split_pane`. Do not start unix Attach, 026, 031, `window/` cutover, `RenameWorkspace`, lua `window:set_workspace`, or `SwitchToWorkspace.spawn` custom command.

Surfaces: ShowLauncher + command palette `Create new Workspace` / `Switch to workspace \`name\`` / relative next/previous. Title bar and status show the active name.

User 2026-08-28: OS X on a workspace HWND does not show the previous workspace and leaves a ghost process (`exit` is fine). Retry after `on_window_should_close` → `close_self_or_quit`. **User-ok.**

Record: `docs/decisions/055-mux-workspaces.json`.
