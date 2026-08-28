# Lua config matrix (GPUI)

**Source of truth for per-key honor:** [`docs/lua-config.json`](../lua-config.json).

Do not keep a second copy of every key in `HANDOFF.md` or `STATE.json`. Those files point here. After a lua user-try or a new key, update the JSON (`stats`, `keys[]`, `backlog[]`); never delete rows.

User file: `C:\Users\rafael\.wezterm.lua`.

## Stats (2026-08-28)

From `lua-config.json` `stats`:

| Bucket | Count |
|---|---|
| Keys tracked | 27 |
| User-ok | 14 |
| Partial | 1 (Ctrl+click opens; no hover highlight) |
| Not tested | 3 (`adjust_window_size`, skip-close keys) |
| No visible change (already matched) | 3 (fancy tab bar, window decorations, title-button alignment) |
| Not wired / parked | 6 |

## Slices

| Slice | What |
|---|---|
| [020](../plans/020-lua-config-first-slice.md) | Load file. Font, size, scheme, scrollback, bell. **User-ok.** |
| [034](../plans/034-lua-config-second-slice.md) | Tab chrome + user `mouse_bindings`. **User-ok** except hover highlight. |
| [041](../plans/041-inactive-pane-hsb.md) | `inactive_pane_hsb`. **User-ok.** |
| [042](../plans/042-palette-selection-contrast.md) | `command_palette_fg_color` / `command_palette_bg_color` invert on selected row. **User-ok.** |

## Do not start unless asked

- Hyperlink **hover underline / hand cursor** (Ctrl+click open is already user-ok)
- `enable_scroll_bar`
- `max_fps`
- Palette fonts (palette parked)
- `default_prog` / `launch_menu` / live reload / `wezterm.on`

When adding a key: append a `keys[]` object, bump `stats`, add `backlog[]` if it is later work.
