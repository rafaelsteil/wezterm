# Inactive pane HSB (041)

User after **040 user-ok** (“works great”): wezterm-gui dims the unfocused pane. Dracula inactive is slightly darker; hollow block cursor vs solid on the focused side.

## wezterm-gui

- Lua `inactive_pane_hsb` (default `{ hue = 1.0, saturation = 0.9, brightness = 0.8 }`).
- GPU shader converts RGB → HSV, multiplies, HSV → RGB, on **all** pane colors (bg, text, cursor).
- Unfocused cursor is a **hollow block** (`cursor_border`), not a filled `cursor_bg`.

## In this slice

GPUI has no HSV shader. Apply the same transform in CPU on the line-sprite bitmap after composite (before BGRA swap). Blank rows use the transformed `TermPaint.bg`. Honor lua `inactive_pane_hsb`. Hollow outline on the inactive cursor cell.

**User-ok** 2026-08-27 (“perfect”).

Record: `docs/decisions/041-inactive-pane-hsb.json`.
