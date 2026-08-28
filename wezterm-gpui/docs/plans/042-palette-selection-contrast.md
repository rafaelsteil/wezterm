# Palette selection contrast (042)

User after **041 user-ok** (“perfect”): the command palette selected-row background is almost impossible to distinguish from the foreground.

## Cause

GPUI used `cx.theme().accent.opacity(0.22)` behind `theme.foreground` text. On this gpui-component theme, accent is nearly the same as foreground, so the highlight (or a full-opacity accent if opacity does not apply) blends into the text.

## wezterm-gui

`wezterm-gui/src/termwindow/palette.rs` inverts lua `command_palette_fg_color` / `command_palette_bg_color` on the selected row (selected → bg=fg, text=bg). Defaults: fg gray 0.75, bg `#333333`.

## In this slice

Honor those lua colors for palette chrome. Selected row is a **solid invert** (no accent opacity). Unwired rows stay dimmed (`opacity(0.62)`). Hover accent fill removed (same contrast class). `command_palette_font` stays parked.

**User-ok** 2026-08-27 (“Confirmed colors work”).

Record: `docs/decisions/042-palette-selection-contrast.json`.
