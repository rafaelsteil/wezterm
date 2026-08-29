# Tab sends PTY completion (054)

User: Tab currently changes focus (or does nothing). It should complete like a Linux/Windows terminal (`Tab` → shell autocomplete).

## Cause

`map_keystroke` already maps `"tab"` → `KeyCode::Tab`. AppShell `on_term_key` never sees it because **gpui-component `Root`** binds `tab` / `shift-tab` in context `"Root"` and calls `window.focus_next` / `focus_prev` (chrome Plus, tab Close, TitleBar buttons are `tab_stop`).

GPUI dispatches key **actions before** `on_key_down`.

## This slice

Bind `tab` / `shift-tab` in `AppShell` context (deeper than Root). Send `KeyCode::Tab` to the mux pane. Dialogs `cx.propagate()` so Root can still cycle OK/Cancel. Palette/picker already switch to `PaletteOpen` so Root Tab stays UI. Search / pane-select swallow Tab (no focus steal, no PTY).

Do not start 026/031, unix Attach, `window/` cutover.

Needs user-try: type a partial path or command in cmd.exe / WSL, press Tab, completion runs; Shift+Tab if the shell uses it; Ctrl+Q dialog Tab still moves between buttons.

**User-ok 2026-08-28.**

Record: `docs/decisions/054-tab-sends-pty-completion.json`.
