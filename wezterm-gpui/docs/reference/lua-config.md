# Lua config matrix (GPUI)

**Source of truth for per-key honor:** [`docs/lua-config.json`](../lua-config.json).

Do not keep a second copy of every key in `HANDOFF.md` or `STATE.json`. Those files point here. After a lua user-try or a new key, update the JSON (`stats`, `keys[]`, `backlog[]`); never delete rows.

User file: `C:\Users\rafael\.wezterm.lua`.

## Stats (2026-08-30)

From `lua-config.json` `stats`:

| Bucket | Count |
|---|---|
| Keys tracked | 30 |
| User-ok | 25 |
| Partial | 0 |
| Not tested | 1 (`adjust_window_size`) |
| No visible change (already matched) | 1 (fancy tab bar) |
| Not wired / parked | 3 |

## Slices

| Slice | What |
|---|---|
| [020](../plans/020-lua-config-first-slice.md) | Load file. Font, size, scheme, scrollback, bell. **User-ok.** |
| [034](../plans/034-lua-config-second-slice.md) | Tab chrome + user `mouse_bindings`. **User-ok.** |
| [058](../plans/058-hyperlink-hover-highlight.md) | Hover underline + hand cursor. **User-ok.** |
| [059](../plans/059-plain-click-opens-link.md) | Plain click opens hovered link (default InputMap). **User-ok.** |
| [060](../plans/060-disable-default-mouse-bindings.md) | Honor `disable_default_mouse_bindings` (plain click must not open). **User-ok.** |
| [061](../plans/061-command-palette-font.md) | Honor `command_palette_font` / `command_palette_font_size`. **User-ok.** |
| [062](../plans/062-window-decorations.md) | Honor `window_decorations` (native TITLE vs INTEGRATED_BUTTONS). **User-ok.** |
| [041](../plans/041-inactive-pane-hsb.md) | `inactive_pane_hsb`. **User-ok.** |
| [042](../plans/042-palette-selection-contrast.md) | `command_palette_fg_color` / `command_palette_bg_color` invert on selected row. **User-ok.** |
| [050](../plans/050-overlay-parity-and-scrollbar.md) | `enable_scroll_bar` rail + thumb. **User-ok.** |

## Do not start unless asked

- `max_fps`
- `char_select_font` / `pane_select_font` / `command_palette_line_height`
- `default_prog` / `launch_menu` / live reload / `wezterm.on`

When adding a key: append a `keys[]` object, bump `stats`, add `backlog[]` if it is later work.
