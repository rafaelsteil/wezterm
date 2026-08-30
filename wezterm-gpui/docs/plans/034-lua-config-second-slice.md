# Lua config second slice (`~/.wezterm.lua` leftovers)

020 already loads the file and applies font / size / scheme / scrollback / bell. This slice honors the **rest of the keys in `C:\Users\rafael\.wezterm.lua` that are cheap and already have a GPUI surface**.

## Apply now

Chrome (`shell.rs`):

- `show_tab_index_in_tab_bar` (user `false` — already matched; wire so flipping it works)
- `tab_and_split_indices_are_zero_based` (only if index is shown)
- `tab_max_width` (user `32`; we currently do not truncate)
- `hide_tab_bar_if_only_one_tab` (user `false` — already matched)
- `enable_tab_bar` (default true)
- `switch_to_last_active_tab_when_closing_tab` (user `true`; we currently pick a neighbor index)

Mouse (`term_pane.rs`), **user `mouse_bindings` only** (not wezterm-gui’s default InputMap):

- `OpenLinkAtMouseCursor` (Ctrl+left up) via mux `apply_hyperlinks` + `wezterm-open-url`
- `Nop` (Ctrl+left down) so the click is not a selection
- `ScrollByPage` (Ctrl+wheel)

## Already matched (no extra code)

- `adjust_window_size_when_changing_font_size = false` — GPUI never grows the HWND on palette font bump
- `use_fancy_tab_bar = true` — only gpui-component `TabBar`

## Still out

- `enable_scroll_bar` is **050** (user-ok)
- `max_fps` (GPUI owns refresh)
- `command_palette_font` / `command_palette_font_size` — **061**
- `window_decorations` — **062**
- Full default mouse InputMap
- `default_prog` / `launch_menu` / live reload / `wezterm.on`

Hover underline / hand cursor is **058** (user-ok).

## User-try 2026-08-27

- **ok:** `show_tab_index_in_tab_bar`, `hide_tab_bar_if_only_one_tab`, `tab_max_width`, `switch_to_last_active_tab_when_closing_tab`, Ctrl+down Nop, Ctrl+wheel ScrollByPage
- **ok:** Ctrl+click OpenLink **opens**
- **058:** hover underline + PointingHand **user-ok** 2026-08-29
- **no visible change (expected):** `use_fancy_tab_bar`
- **062:** `window_decorations` **user-ok** 2026-08-29 (“confirmed as working”)
- **not tested:** `adjust_window_size_when_changing_font_size`

Per-key matrix: [`docs/lua-config.json`](../lua-config.json).

