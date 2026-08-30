# 036 — Command palette + launcher matrices

User 2026-08-27: prepare for every command-palette and launcher operation. For each, decide **call wezterm core** vs **needs a GPUI surface**. Same pattern as `docs/lua-config.json`.

## Sources of truth

| Surface | Tracker | Human index | Runtime list |
|---|---|---|---|
| Ctrl+Shift+P palette | [`docs/command-palette.json`](../command-palette.json) | [`docs/reference/command-palette.md`](../reference/command-palette.md) | `wezterm-gpui/src/commands.rs` |
| Right-click Plus launcher | [`docs/launcher.json`](../launcher.json) | [`docs/reference/launcher.md`](../reference/launcher.md) | not cloned as a second menu |

Never delete rows. Append. `gpui` / `kind` / `user_try` live in the JSON.

## Where wezterm-gui builds the lists

**Palette** (`wezterm-gui/src/termwindow/palette.rs` `build_commands`):

1. `CommandDef::actions_for_palette_and_menubar` → `compute_default_actions()` + `derive_command_from_key_assignment` in `wezterm-gui/src/commands.rs`
2. Lua `augment-command-palette`
3. `config.launch_menu` as “{label} (New Tab)”
4. Mux domains (New Tab / Attach / Detach) and workspaces
5. Extra `InputMap` key assignments not already in the default list
6. Hide `CopyMode(*)` unless the copy overlay is active

Executing a row is `TermWindow::perform_key_assignment` (same as a key binding). **Do not path-dep `wezterm-gui`.** GPUI dispatches by catalog `id`.

**Launcher** (`wezterm-gui/src/overlay/launcher.rs`): termwiz overlay. Right-click the tab-bar Plus (`ShowLauncher`). Default flags: `LAUNCH_MENU_ITEMS | WORKSPACES | DOMAINS | KEY_ASSIGNMENTS | COMMANDS`. That dumps domain/workspace rows **plus the entire command list plus InputMap** — which is why it feels like a second, worse palette.

## Kind (every row)

| `kind` | Meaning | GPUI work |
|---|---|---|
| `call_core` | mux / `Pane` / `config` already has the operation | Thin call from AppShell / TermPane |
| `gpui_window` | window chrome (tabs, font, quit, hide, fullscreen, second HWND) | GPUI window APIs |
| `gpui_ui` | overlay/modal (search, copy mode, charselect, launcher, debug, pane select) | New gpui-component surface |
| `open_url` | Help links | `wezterm-open-url` (already a dep) |
| `input_map` | key tables | Not this POC until asked |
| `dynamic` | mux domains / workspaces / lua `launch_menu` at runtime | Register domains first |

## This slice

- Tracking JSON + this plan + decision 036
- GPUI palette lists the Windows default catalog; **wired** rows run; **listed** rows are dimmed/disabled
- Do **not** clone the launcher overlay. Unique launcher ops (WSL tabs, unix attach, workspaces) are tracked in `launcher.json`
- Do **not** start charselect / copy mode / search / splits / WSL domains unless asked
- Do **not** honor lua `launch_menu` / `augment-command-palette` unless asked

## Out

- Palette fonts (`command_palette_font`) — **061 user-ok**
- Frecency / `recent-commands.json`
- macOS menubar rebuild (`CommandDef::recreate_menubar`)
