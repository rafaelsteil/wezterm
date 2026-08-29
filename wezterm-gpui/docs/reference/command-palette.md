# Command palette matrix (GPUI)

**Source of truth:** [`docs/command-palette.json`](../command-palette.json).

Human index only. After a palette user-try or a newly wired id, update the JSON (`stats`, `commands[]`, `backlog[]`); never delete rows.

wezterm-gui builds the list in `wezterm-gui/src/commands.rs` (`compute_default_actions` + `derive_command_from_key_assignment`) then `termwindow/palette.rs` `build_commands` (lua augment, `launch_menu`, mux domains/workspaces, InputMap). Execute is `perform_key_assignment` — the same as a key binding.

GPUI lists the Windows default catalog in `wezterm-gpui/src/commands.rs`. **Wired** rows run. **Listed** was dimmed; **049** wired the remaining catalog (89/89). Dynamic mux/lua rows stay out of the static list.

**042:** selected row inverts lua `command_palette_fg_color` / `command_palette_bg_color` (wezterm-gui), not `theme.accent` opacity. **User-ok.**

**043:** ↑↓ keep the highlight. `InputEvent::Change` resets to row 0; caret blink / Input arrows do not. **User-ok.**

**044:** scroll, reset, open link, Help URLs, minimize, ActivateTab / MoveTab. **User-ok.**

**045:** ↑↓ scroll the selected row into view (`ScrollHandle` on `#command-list`). **User-ok.**

**046:** ActivatePaneDirection / RotatePanes / TogglePaneZoom on the GPUI split tree. **User-ok.**

**047:** ToggleFullScreen, ResetFontAndWindowSize, AlwaysOnTop/Bottom/Normal. **User-ok.**

**048:** AdjustPaneSize Left/Right/Up/Down on `ResizableState`. **User-ok.**

**049:** remaining catalog (SpawnWindow, ActivateWindow, ReloadConfiguration, launcher/tab/pane/char/search/quickselect/debug pickers, copy mode, primary=clipboard). **Needs user-try.** No dimmed rows.

**050:** Search / Quick Select / Pane Select match wezterm-gui (bottom search bar, in-pane labels, pane letter badges). Ctrl+Shift+F is Search. FPS HUD is palette **Toggle FPS HUD** only. 049 Picker path stays behind a flag. **User-ok.**

## Stats (2026-08-28)

From `command-palette.json` `stats`:

| Bucket | Count |
|---|---|
| Catalog rows (Rust) | 90 |
| Wired (Enter runs) | 90 |
| Listed (dimmed) | 0 |
| POC extras (not in Windows defaults) | 5 (Quit, Rename, Prompt, Confirm, ToggleFpsHud) |
| Dynamic (mux/lua, not in static catalog) | 4 |

## Kind (do we call core or build GPUI?)

| `kind` | What to do |
|---|---|
| `call_core` | Thin AppShell/TermPane call (`erase_scrollback`, `spawn_pane`, RIS reset, `config::reload`) |
| `gpui_window` | Tabs / font / quit / hide / fullscreen / second window |
| `gpui_ui` | Overlay (`Picker` for search, charselect, launcher, debug, pane select; copy mode on TermPane) |
| `open_url` | `wezterm-open-url` (already a dep) |
| `input_map` | Key tables — ClearKeyTableStack notifies empty |
| `dynamic` | `launch_menu` is in ShowLauncher; **053** WSL/exec domain rows; **055** workspace create/switch. Unix attach still out. |

## POC shortcuts (049)

- Primary selection = clipboard (Windows)
- DetachDomain / ClearKeyTableStack = notify
- Search picker = visible lines; Ctrl+Shift+F is still FPS HUD
- Copy mode = hjkl/v/y, not the full key table
- Debug = dump, not lua REPL
- No unix attach. **053** wires WSL/exec LocalDomain rows. **055** wires workspace create/switch.
