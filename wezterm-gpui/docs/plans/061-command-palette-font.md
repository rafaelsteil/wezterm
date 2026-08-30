# Honor `command_palette_font` / `command_palette_font_size` (061)

User lua (`C:\Users\rafael\.wezterm.lua`): `command_palette_font = wezterm.font("Segoe UI")`, `command_palette_font_size = 12.0`. GPUI palette used theme UI font + hardcoded 11–14px.

## wezterm-gui

`FontConfiguration::command_palette_font`: size is `command_palette_font_size` (points, default 14). Family is `command_palette_font`, else `window_frame.font`, else the title/sys style.

## This slice

- Apply family + size on the Command Palette overlay (title, search Input, rows, status).
- Size: points → CSS px (`pt * 96/72`) so 12pt is 16px, matching wezterm-gui at 96dpi independent of window scale.
- Family: first named family from lua; no JetBrains/Noto fallback list (those are wezterm-font rasterizer).
- Picker / `char_select_font` / `pane_select_font` / `command_palette_line_height` stay parked.

Stay parked: 026, unix Attach, lua REPL, `window/` cutover.

Needs user-try: Ctrl+Shift+P should look like Segoe UI at 12pt vs wezterm-gui.

**User-ok** 2026-08-29 (“confirmed as working”).

Record: `docs/decisions/061-command-palette-font.json`.
