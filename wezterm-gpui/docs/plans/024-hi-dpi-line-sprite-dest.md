# Hi-DPI line-sprite dest (4K shredding)

User: 4K main monitor showed shredded rows; other screen OK; resize on 4K did not help. 023 box-draw was already user-ok.

## In this slice

- `paint_image(dest, dest)` — do not pass the pane rect as the first arg (object-fit crop)
- Dest size = `RenderImage` device pixels / `window.scale_factor()` so the atlas tile is 1:1
- Layout Y stride uses that same scale, not `fonts.dpi/96`
- Paint cache key includes scale

## Out

- Atlas tile splitting for very wide 4K rows (only if shredding remains)
- Powerline triangles, palette, ConPTY junk, window/ cutover

Record: `docs/decisions/024-hi-dpi-line-sprite-dest.json`.
