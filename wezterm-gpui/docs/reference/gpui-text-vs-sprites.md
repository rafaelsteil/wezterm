# GPUI native text vs wezterm-font sprites

Companion to [open-questions.md](open-questions.md) §2. No code change until this is picked.

## What each stack actually paints

```text
wezterm-gpui today
  wezterm-term Line
    → LoadedFont::blocking_shape (HarfBuzz)
    → rasterize_glyph (FreeType via freetype-sys)
    → cached RenderImage per (font, glyph_pos, fg)
    → paint_quad (bg) + paint_image (sprite)

tty7 / Zed / gpui-terminal
  VT cells
    → GPUI text_system.shape_line (DirectWrite / font-kit)
    → shaped.paint (GPUI GPU atlas)
    → paint_quad for bg / box-draw / cursor
```

gpui-terminal’s `BatchedTextRun` is unused; it still `shape_line`s **one character**. Treat tty7 `paint_glyphs` (and Zed only as ideas) as the real native-text design.

## Decision table

| Concern | Sprites (now) | GPUI `text_system` |
|---|---|---|
| Shaper | wezterm-font / HarfBuzz | GPUI (DirectWrite on this host) |
| Ligatures | Possible (font features) | tty7/Zed **disable** for the cell grid |
| Fallback / italic CJK | wezterm-font | Stock GPUI has a known Windows bug (tty7 forked Zed) |
| Color emoji | wezterm raster (COLR unproven on freetype-sys 0.20) | Apple Color Emoji / DirectWrite bitmaps; tty7 scales with `seg_budget` |
| Cell metrics | FT `cell_width`/`cell_height`, dpi hardcoded 96 | Shape `M` or `advance('m')` in the same Window; `scale_factor` |
| Atlas | We own `ImageId`s; viewport bitmap already failed (010) | GPUI owns glyph atlas |
| Resize blank | `TermScreen` Element; sprites survived shrink-grow | Native text never uploaded a viewport bitmap |
| Match wezterm-gui later | Same font crate | Will still need wezterm-font at cutover unless we abandon it |
| Copy license | ours | tty7 Apache rewrite OK; Zed GPL **no paste** |

## What “dig deeper” means (spike, not a cutover)

Keep `GlyphPainter` working. Add an optional path in `TermScreen::paint` that:

1. Copies visible cells (attrs + text), drops pane lock.
2. Merges adjacent same-style runs.
3. `shape_line` + `with_content_mask` per run (`force_width` = cell).
4. Box-draw still as font sprites or skip (geometry is a later steal).

Compare side-by-side on Windows: `dir /a`, vim if available, italic prompt, resize, font-size key. If GPUI text is cleaner *and* italic CJK is not mojibake, write a new decision replacing 009/010 for the POC paint path. If it is worse, keep sprites and steal only snap/mask/batch ideas.

Do not path-dep tty7. Do not take `l0ng-ai/zed` until the italic bug is actually hit.
