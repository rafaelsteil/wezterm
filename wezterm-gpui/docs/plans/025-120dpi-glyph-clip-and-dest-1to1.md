# 120dpi glyph shredding (clip + dest 1:1)

User after 024: 4K @ **120dpi** (scale 1.25) still shredded; 1080p @ **96dpi** OK. New shot is **vertical slivers inside glyphs** (`<DIR>` → `<CIF>`-like), not 024’s horizontal row gaps. Status: Cascadia 11pt, line sprites, pty/view 111×30.

## In this slice

- **Drop tight per-cell X clip (023).** Clip to exact `cell_w` cut FreeType LCD padding and bearings at 1.25; 96dpi hid it. `blit_glyph` clips only to the row bitmap again. Box-draw geometry stays.
- **Lock dest device size to the bitmap.** Origin rounded in device px, size = `image_w/h / scale` so after GPUI `snap_bounds` dest device size equals the sprite. Separate origin/size snaps at 1.25 could skip columns.

## Out

- Re-introducing padded overflow-only clip (only after 120dpi is user-ok)
- Atlas tile splitting, Powerline, palette, ConPTY junk, window/ cutover

Record: `docs/decisions/025-120dpi-glyph-clip-and-dest-1to1.json`.

**User-ok 2026-08-27** on 4K 120dpi (and 96dpi). Monitor-move hang is backlog (026), not this slice.
