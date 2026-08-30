# Geometry box-draw + per-cell glyph clip

Visual leftovers vs wezterm-gui while waiting for the next user bug list (016 / steal-list §4–5). Do not change `wezterm-gui`.

## In this slice

- **Per-cell clip:** `blit_glyph` clipped to `[col, col+num_cells) × cell` so wide fallback glyphs cannot smear into neighbors. **Reverted in 025** — at 120dpi (scale 1.25) that cut LCD/bearings (`<DIR>` → `<CIF>`). Clip is row-bitmap only until a padded overflow clip is proven.
- **Geometry box-draw:** U+2500–259F rasterized into the row bitmap (`src/boxdraw.rs`). Honors `custom_block_glyphs` / `anti_alias_custom_block_glyphs`. Those codepoints are replaced with spaces before HarfBuzz so the font’s box glyphs (wrong advance, gaps) are not used.
- Thickness snaps to device pixels so `│`/`─` tile across cells.

## Out (not 023 leftovers)

- Powerline triangles (U+E0B0…) — later slice if nerd-font prompts look wrong
- wezterm-gui `customglyph.rs` / tiny-skia (do not path-dep `wezterm-gui`)
- GPUI `Path` / `paint_quad` for box-draw (paint is still CPU line sprites)
- GPUI `text_system`, palette, ConPTY junk, window/ cutover

**Finished** 2026-08-29. Box-draw **user-ok** (`echo` / `tree`). Tight per-cell clip stays reverted (025). Do not reopen 023 for powerline or padded clip unless asked.

Record: `docs/decisions/023-geometry-boxdraw-and-cell-clip.json`.
