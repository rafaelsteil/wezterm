# Integer cell grid (cursor vs glyph)

User after 029: the 1px dark sliver before the block cursor on 4K @ **120dpi** is still there. 96dpi and wezterm-gui stay tight.

029 only changed **fill width**. The cursor’s **left** edge is still `round(cursor_col * cell_w)`. Glyphs advanced by HarfBuzz `x_advance` (often a fraction less than the cell), so after the prompt they sit left of that edge. The last letter is pane-bg (not filled), so the skip shows as a gap.

wezterm-gui (`RenderMetrics` + default `use_pixel_positioning: false`):

- `cell_width = (metrics.cell_width * lua cell_width).ceil()` (integer device px)
- Glyphs step `num_cells * cell_width`, not `x_advance`
- Cursor quad is `cursor.x * cell_width`

Dest 1:1 (025) still holds: dest size is bitmap device px / `scale_factor`.

## In this slice

- `device_cell_size`: ceil like wezterm-gui; same values in `cell_size()` (mouse/PTY) and line sprites.
- Glyph loop: `x_pos += num_cells * cell_w` (including spaces). Bearing still applied to the blit, not the grid.

## Out

- `use_pixel_positioning` / experimental pixel layout
- Cursor shape (bar/underline)
- 026 monitor-move hang
- lua `default_prog` / `launch_menu`

Record: `docs/decisions/030-integer-cell-grid.json`.

User 2026-08-27: **user-ok** (“works great now”).
