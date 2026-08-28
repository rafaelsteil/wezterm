# Command palette matrix (GPUI)

**Source of truth:** [`docs/command-palette.json`](../command-palette.json).

Human index only. After a palette user-try or a newly wired id, update the JSON (`stats`, `commands[]`, `backlog[]`); never delete rows.

wezterm-gui builds the list in `wezterm-gui/src/commands.rs` (`compute_default_actions` + `derive_command_from_key_assignment`) then `termwindow/palette.rs` `build_commands` (lua augment, `launch_menu`, mux domains/workspaces, InputMap). Execute is `perform_key_assignment` — the same as a key binding.

GPUI lists the Windows default catalog in `wezterm-gpui/src/commands.rs`. **Wired** rows run. **Listed** rows are dimmed and do not execute.

**042:** selected row inverts lua `command_palette_fg_color` / `command_palette_bg_color` (wezterm-gui), not `theme.accent` opacity. **User-ok.**

**043:** ↑↓ keep the highlight. `InputEvent::Change` resets to row 0; caret blink / Input arrows do not. **User-ok.**

**044:** scroll, reset, open link, Help URLs, minimize, ActivateTab / MoveTab. **41 wired / 48 listed.** **User-ok.**

**045:** ↑↓ scroll the selected row into view (`ScrollHandle` on `#command-list`). **User-ok.**

**046:** ActivatePaneDirection / RotatePanes / TogglePaneZoom on the GPUI split tree. **48 wired / 41 listed.** **User-ok.**

**047:** ToggleFullScreen, ResetFontAndWindowSize, AlwaysOnTop/Bottom/Normal. **55 wired / 34 listed.** **User-ok.**

## Stats (2026-08-28)

From `command-palette.json` `stats`:

| Bucket | Count |
|---|---|
| Catalog rows (Rust) | 89 |
| Wired (Enter runs) | 55 |
| Listed (dimmed) | 34 |
| POC extras (not in Windows defaults) | 4 (Quit, Rename, Prompt, Confirm) |
| Dynamic (mux/lua, not in static catalog) | 4 |

## Kind (do we call core or build GPUI?)

| `kind` | What to do |
|---|---|
| `call_core` | Thin AppShell/TermPane call (`erase_scrollback`, `spawn_pane`, RIS reset, `config::reload`) |
| `gpui_window` | Tabs / font / quit / hide / fullscreen / second window |
| `gpui_ui` | New overlay (search, copy mode, charselect, launcher, debug, pane select) |
| `open_url` | `wezterm-open-url` (already a dep) |
| `input_map` | Key tables — not this POC |
| `dynamic` | Mux domains / workspaces / `launch_menu` at runtime |

## Do not start unless asked

- Charselect
- Copy mode / search overlay (Ctrl+Shift+F is the FPS HUD in this POC)
- Pane select overlay / adjust size
- WSL + unix mux domains
- lua `launch_menu` / `augment-command-palette`
- Cloning the launcher overlay (`docs/launcher.json`)
- SpawnWindow / ActivateWindow / ReloadConfiguration
