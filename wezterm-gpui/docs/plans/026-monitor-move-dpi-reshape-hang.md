# Monitor-move hang (parked)

User 2026-08-27 after **025 user-ok**: dragging the window between monitors (4K 120dpi ↔ 1080p 96dpi) **hangs 2–3 seconds**. Looks like recompute/redraw, not a freeze-forever.

## Status

**Backlog. Do not start** unless the user asks. Same class as FPS HUD later / ConPTY junk later (018).

## Likely cause (not proven)

`GlyphPainter::sync_font` on DPI change rebuilds `FontConfiguration`, `glyphs.clear()`, `drain_rows()`. Then `layout` HarfBuzz-shapes and CPU-composites every visible line on the UI thread. Debug binary vs official release.

## Out of this parked note

- Do not keep both DPI glyph caches “just in case” until asked (memory).
- Do not start GPUI `text_system`.
- Do not change 013/016 ConPTY debounce as a guess.

Record: `docs/decisions/026-monitor-move-dpi-reshape-hang.json`.
