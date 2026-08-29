//! wezterm-font glyphs composited into cached per-line GPUI sprites.
//! Cell backgrounds live in the same row bitmap. Box-draw / block elements
//! (U+2500–259F) are geometry, not font sprites (023). Tight per-cell glyph
//! clip was reverted (025: 120dpi cut LCD/bearings). Dest is 1:1 device px (024/025).
//! Cell fills use abutting `cell_span` (029). Glyphs sit on wezterm-gui's
//! integer cell grid (`ceil` + `num_cells * cell_w`, not HarfBuzz `x_advance`)
//! so the cursor left edge matches the last glyph at 120dpi (030).
//! Focused cursor fills at `cursor.x` even when that col is past `visible_cells()` (031).
//! Not a viewport bitmap (010) and not the wezterm-gui GPU atlas. See decision 017.

use std::collections::{HashMap, VecDeque};
use std::rc::Rc;
use std::sync::Arc;

use gpui::{Bounds, ContentMask, Corners, Pixels, RenderImage, Window, fill, point, px, rgb, size};
use image::{Frame, RgbaImage};
use wezterm_bidi::Direction;
use wezterm_font::{FontConfiguration, LoadedFont, LoadedFontId, RasterizedGlyph};
use wezterm_term::Line;
use wezterm_term::color::ColorPalette;

const ROW_CACHE_CAP: usize = 256;

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
struct GlyphKey {
    font_id: LoadedFontId,
    glyph_pos: u32,
    font_idx: usize,
}

struct CachedGlyph {
    width: u32,
    height: u32,
    bearing_x: f64,
    bearing_y: f64,
    /// Color emoji: straight RGBA. Text: wezterm-font coverage (RGB sRGB-encoded,
    /// A linear) — tinted at blit so we match wezterm-gui, not `fg * alpha²`.
    data: Arc<Vec<u8>>,
    has_color: bool,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
struct RowKey {
    hash: [u8; 16],
    cursor_col: u16,
    /// Exclusive column range. `u16::MAX` start means no selection on this row.
    sel_start: u16,
    sel_end: u16,
    cols: u16,
    cell_w: u16,
    cell_h: u16,
    /// Inactive panes get `inactive_pane_hsb` (041). Must be in the key so
    /// focused and dimmed rows do not share a sprite.
    inactive: bool,
}

struct CachedRow {
    image: Arc<RenderImage>,
}

pub struct GlyphPainter {
    fonts: Rc<FontConfiguration>,
    glyphs: HashMap<GlyphKey, Rc<CachedGlyph>>,
    rows: HashMap<RowKey, Rc<CachedRow>>,
    row_lru: VecDeque<RowKey>,
    pending_drop: Vec<Arc<RenderImage>>,
}

pub struct CellSize {
    pub width: f32,
    pub height: f32,
}

#[derive(Clone)]
pub struct TermSprite {
    x: f32,
    y: f32,
    image: Arc<RenderImage>,
}

#[derive(Clone)]
pub struct TermPaint {
    pub bg: u32,
    pub sprites: Vec<TermSprite>,
    pub drop_images: Vec<Arc<RenderImage>>,
}

impl GlyphPainter {
    pub fn new(dpi: u32) -> anyhow::Result<Self> {
        let dpi = dpi.clamp(72, 384) as usize;
        let config = config::configuration();
        let fonts = Rc::new(FontConfiguration::new(Some(config), dpi)?);
        let _ = fonts.default_font()?;
        let _ = fonts.default_font_metrics()?;
        Ok(Self {
            fonts,
            glyphs: HashMap::new(),
            rows: HashMap::new(),
            row_lru: VecDeque::new(),
            pending_drop: Vec::new(),
        })
    }

    pub fn dpi(&self) -> u32 {
        self.fonts.get_dpi() as u32
    }

    fn dpr(&self) -> f64 {
        (self.fonts.get_dpi() as f64 / 96.0).max(0.5)
    }

    pub fn sync_font(&mut self, font_px: f32, dpi: u32) {
        let dpi = dpi.clamp(72, 384) as usize;
        let config = config::configuration();
        let base = config.font_size.max(1.0);
        let scale = (font_px as f64 / base).clamp(0.5, 4.0);
        let scale_changed = (scale - self.fonts.get_font_scale()).abs() > 0.01;
        let dpi_changed = dpi != self.fonts.get_dpi();
        if !scale_changed && !dpi_changed {
            return;
        }
        if dpi_changed {
            if let Ok(fonts) = FontConfiguration::new(Some(config), dpi) {
                fonts.change_scaling(scale, dpi);
                self.fonts = Rc::new(fonts);
                self.glyphs.clear();
                self.drain_rows();
                return;
            }
        }
        self.fonts.change_scaling(scale, dpi);
        self.glyphs.clear();
        self.drain_rows();
    }

    fn drain_rows(&mut self) {
        for (_, row) in self.rows.drain() {
            self.pending_drop.push(Arc::clone(&row.image));
        }
        self.row_lru.clear();
    }

    pub fn cell_size(&self) -> anyhow::Result<CellSize> {
        let m = self.fonts.default_font_metrics()?;
        let (cell_w, cell_h) = device_cell_size(m.cell_width.get(), m.cell_height.get());
        let dpr = self.dpr() as f32;
        Ok(CellSize {
            width: cell_w as f32 / dpr,
            height: cell_h as f32 / dpr,
        })
    }

    /// `sel_cols` is per visible row: exclusive column range, or `(u16::MAX, u16::MAX)` if none.
    /// `scale` is `window.scale_factor()` — dest size must match the bitmap in device pixels.
    /// `focused` false applies lua `inactive_pane_hsb` (wezterm-gui shader HSV).
    pub fn layout(
        &mut self,
        lines: &[Line],
        cursor: Option<(usize, usize)>,
        pal: &ColorPalette,
        cols: usize,
        sel_cols: &[(u16, u16)],
        scale: f32,
        focused: bool,
    ) -> anyhow::Result<TermPaint> {
        let font = self.fonts.default_font()?;
        let metrics = font.metrics();
        let dpr = scale.max(0.5) as f64;
        let (cell_w, cell_h) =
            device_cell_size(metrics.cell_width.get(), metrics.cell_height.get());
        let descender = metrics.descender.get();
        let (br, bg, bb, _) = pal.background.as_rgba_u8();
        let pane_bg = pack_rgb(br, bg, bb);
        let hsb = config::configuration().inactive_pane_hsb;
        let paint_bg = if focused {
            pane_bg
        } else {
            apply_hsb_u32(pane_bg, hsb)
        };
        let cols = cols.max(1).min(400);
        let phys_w = (cols as f64 * cell_w).max(1.0);
        let phys_h = cell_h.max(1.0);
        let key_cell_w = phys_w as u16;
        let key_cell_h = phys_h as u16;
        let logical_h = (phys_h / dpr) as f32;

        let mut sprites = Vec::with_capacity(lines.len());

        for (row, line) in lines.iter().enumerate() {
            let cursor_col = match cursor {
                Some((r, c)) if r == row => c.min(u16::MAX as usize - 1) as u16,
                _ => u16::MAX,
            };
            let (sel_start, sel_end) = sel_cols.get(row).copied().unwrap_or((u16::MAX, u16::MAX));
            if cursor_col == u16::MAX && sel_start == u16::MAX && line_is_blank(line, pal, pane_bg)
            {
                continue;
            }
            let key = RowKey {
                hash: line.compute_shape_hash(),
                cursor_col,
                sel_start,
                sel_end,
                cols: cols as u16,
                cell_w: key_cell_w,
                cell_h: key_cell_h,
                inactive: !focused,
            };
            let cached = match self.cached_row(key) {
                Some(c) => c,
                None => {
                    let image = self.composite_row(
                        &font,
                        line,
                        cursor.filter(|(r, _)| *r == row).map(|(_, c)| c),
                        sel_start,
                        sel_end,
                        pal,
                        pane_bg,
                        cols,
                        cell_w,
                        cell_h,
                        descender,
                        phys_w as u32,
                        phys_h as u32,
                        focused,
                    )?;
                    self.store_row(key, CachedRow { image })
                }
            };
            sprites.push(TermSprite {
                x: 0.0,
                y: row as f32 * logical_h,
                image: Arc::clone(&cached.image),
            });
        }

        Ok(TermPaint {
            bg: paint_bg,
            sprites,
            drop_images: std::mem::take(&mut self.pending_drop),
        })
    }

    fn cached_row(&mut self, key: RowKey) -> Option<Rc<CachedRow>> {
        if self.rows.contains_key(&key) {
            if let Some(pos) = self.row_lru.iter().position(|k| *k == key) {
                self.row_lru.remove(pos);
            }
            self.row_lru.push_back(key);
            return self.rows.get(&key).cloned();
        }
        None
    }

    fn store_row(&mut self, key: RowKey, row: CachedRow) -> Rc<CachedRow> {
        while self.rows.len() >= ROW_CACHE_CAP {
            if let Some(old) = self.row_lru.pop_front() {
                if let Some(evicted) = self.rows.remove(&old) {
                    self.pending_drop.push(Arc::clone(&evicted.image));
                }
            } else {
                break;
            }
        }
        let rc = Rc::new(row);
        self.rows.insert(key, Rc::clone(&rc));
        self.row_lru.push_back(key);
        rc
    }

    fn composite_row(
        &mut self,
        font: &LoadedFont,
        line: &Line,
        cursor_col: Option<usize>,
        sel_start: u16,
        sel_end: u16,
        pal: &ColorPalette,
        pane_bg: u32,
        cols: usize,
        cell_w: f64,
        cell_h: f64,
        descender: f64,
        phys_w: u32,
        phys_h: u32,
        focused: bool,
    ) -> anyhow::Result<Arc<RenderImage>> {
        let (br, bg, bb) = unpack_rgb(pane_bg);
        let mut pixels = vec![0u8; (phys_w * phys_h * 4) as usize];
        fill_rect(
            &mut pixels,
            phys_w,
            phys_h,
            0,
            0,
            phys_w,
            phys_h,
            br,
            bg,
            bb,
            255,
        );

        let custom_blocks = config::configuration().custom_block_glyphs;
        let aa_blocks = config::configuration().anti_alias_custom_block_glyphs;
        let sel_start_u = sel_start as usize;
        let sel_end_u = (sel_end as usize).min(cols);
        let has_sel = sel_start != u16::MAX && sel_end_u > sel_start_u;
        if has_sel {
            let (sr, sg, sb, _) = pal.selection_bg.as_rgba_u8();
            let (x, w) = cell_span(sel_start_u, sel_end_u - sel_start_u, cell_w);
            fill_rect(
                &mut pixels,
                phys_w,
                phys_h,
                x,
                0,
                w,
                phys_h,
                sr,
                sg,
                sb,
                255,
            );
        }

        for cell in line.visible_cells() {
            let col = cell.cell_index();
            if col >= cols {
                continue;
            }
            let attrs = cell.attrs();
            let is_cursor = focused && cursor_col == Some(col);
            let selected = has_sel && col >= sel_start_u && col < sel_end_u;
            let mut bgc = pal.resolve_bg(attrs.background());
            if attrs.reverse() {
                bgc = pal.resolve_fg(attrs.foreground());
            }
            if selected {
                bgc = pal.selection_bg;
            }
            if is_cursor {
                bgc = pal.cursor_bg;
            }
            let (r, g, b, _) = bgc.as_rgba_u8();
            let color = pack_rgb(r, g, b);
            if color == pane_bg && !is_cursor && !selected {
                continue;
            }
            let (x, w) = cell_span(col, cell.width(), cell_w);
            fill_rect(&mut pixels, phys_w, phys_h, x, 0, w, phys_h, r, g, b, 255);
        }

        // 031: VT cursor often sits past the last stored cell (empty after the
        // prompt, after a typed character, after space). visible_cells() then
        // has no matching col, so the loop above never fills. wezterm-gui
        // paints a cursor quad at cursor.x regardless. Always fill when focused.
        if focused {
            fill_cursor_block(
                &mut pixels,
                phys_w,
                phys_h,
                line,
                cursor_col,
                cols,
                cell_w,
                pal,
            );
        }

        if custom_blocks {
            for cell in line.visible_cells() {
                let col = cell.cell_index();
                if col >= cols {
                    continue;
                }
                let Some(ch) = only_char(cell.str()) else {
                    continue;
                };
                if !crate::boxdraw::is_box_draw(ch) {
                    continue;
                }
                let selected = has_sel && col >= sel_start_u && col < sel_end_u;
                let fg_u = cell_fg(cell.attrs(), pal, selected, focused && cursor_col == Some(col));
                let (fr, fg_g, fb) = unpack_rgb(fg_u);
                let (x0, w) = cell_span(col, cell.width(), cell_w);
                let x0 = x0 as f32;
                let x1 = x0 + w as f32;
                crate::boxdraw::paint(
                    &mut pixels,
                    phys_w,
                    phys_h,
                    x0,
                    0.0,
                    x1,
                    phys_h as f32,
                    ch,
                    (fr, fg_g, fb),
                    aa_blocks,
                );
            }
        }

        let text = line.as_str();
        let needs_shape = text
            .chars()
            .any(|c| c != ' ' && c != '\0' && !(custom_blocks && crate::boxdraw::is_box_draw(c)));
        if needs_shape {
            let shape_text: String = if custom_blocks {
                text.chars()
                    .map(|c| {
                        if crate::boxdraw::is_box_draw(c) {
                            ' '
                        } else {
                            c
                        }
                    })
                    .collect()
            } else {
                text.to_string()
            };
            match font.blocking_shape(
                shape_text.as_str(),
                None,
                Direction::LeftToRight,
                None,
                None,
            ) {
                Ok(glyphs) => {
                    // wezterm-gui default (`use_pixel_positioning: false`):
                    // advance `num_cells * cell_width`, not HarfBuzz `x_advance`.
                    // Accumulated `x_advance` drifts left of the integer cursor
                    // at 120dpi (029 `cell_span` only changed fill width).
                    let mut x_pos = 0.0;
                    for info in glyphs {
                        let advance = glyph_cell_advance(info.num_cells, cell_w);
                        if info.is_space || info.glyph_pos == 0 {
                            x_pos += advance;
                            continue;
                        }
                        let col = (x_pos / cell_w).floor().max(0.0) as usize;
                        if custom_blocks {
                            if let Some(ch) = line.get_cell(col).and_then(|c| only_char(c.str())) {
                                if crate::boxdraw::is_box_draw(ch) {
                                    x_pos += advance;
                                    continue;
                                }
                            }
                        }
                        let attrs = line
                            .get_cell(col)
                            .map(|c| c.attrs().clone())
                            .unwrap_or_default();
                        let is_cursor = focused && cursor_col == Some(col);
                        let selected = has_sel && col >= sel_start_u && col < sel_end_u;
                        let fg_u = cell_fg(&attrs, pal, selected, is_cursor);
                        if let Ok(glyph) = self.cached_glyph(font, info.glyph_pos, info.font_idx) {
                            let dx = (x_pos + info.x_offset.get() + glyph.bearing_x).round() as i32;
                            let dy = (cell_h + descender - (info.y_offset.get() + glyph.bearing_y))
                                .round() as i32;
                            blit_glyph(
                                &mut pixels,
                                phys_w,
                                phys_h,
                                dx,
                                dy,
                                glyph.width,
                                glyph.height,
                                &glyph.data,
                                glyph.has_color,
                                fg_u,
                            );
                        }
                        x_pos += advance;
                    }
                }
                Err(err) => eprintln!("wezterm-gpui shape: {err:#}"),
            }
        }

        if !focused {
            if let Some(col) = cursor_block_col(cursor_col, cols) {
                let (cr, cg, cb, _) = pal.cursor_border.as_rgba_u8();
                let (x, w) = cell_span(col, cursor_ncells(line, col), cell_w);
                stroke_rect(&mut pixels, phys_w, phys_h, x, 0, w, phys_h, cr, cg, cb);
            }
            let hsb = config::configuration().inactive_pane_hsb;
            apply_hsb_pixels(&mut pixels, hsb);
        }

        // GPUI RenderImage is BGRA (gpui::assets::RenderImage). image::Rgba is
        // RGBA; without this swap Dracula #282a36 paints as brown #362a28.
        for pixel in pixels.chunks_exact_mut(4) {
            pixel.swap(0, 2);
        }
        let rgba = RgbaImage::from_raw(phys_w, phys_h, pixels)
            .ok_or_else(|| anyhow::anyhow!("row sprite size mismatch"))?;
        Ok(Arc::new(RenderImage::new(vec![Frame::new(rgba)])))
    }

    fn cached_glyph(
        &mut self,
        font: &LoadedFont,
        glyph_pos: u32,
        font_idx: usize,
    ) -> anyhow::Result<Rc<CachedGlyph>> {
        let key = GlyphKey {
            font_id: font.id(),
            glyph_pos,
            font_idx,
        };
        if let Some(g) = self.glyphs.get(&key) {
            return Ok(Rc::clone(g));
        }
        let raster: RasterizedGlyph = font.rasterize_glyph(glyph_pos, font_idx)?;
        let data = glyph_coverage(&raster);
        let cached = Rc::new(CachedGlyph {
            width: raster.width.max(1) as u32,
            height: raster.height.max(1) as u32,
            bearing_x: raster.bearing_x.get(),
            bearing_y: raster.bearing_y.get(),
            data: Arc::new(data),
            has_color: raster.has_color,
        });
        self.glyphs.insert(key, Rc::clone(&cached));
        Ok(cached)
    }
}

pub fn paint_term(window: &mut Window, bounds: Bounds<Pixels>, paint: &TermPaint) {
    let scale = window.scale_factor().max(0.5);
    window.with_content_mask(Some(ContentMask { bounds }), |window| {
        window.paint_quad(fill(bounds, rgb(paint.bg)));
        for s in &paint.sprites {
            // Dest must occupy exactly `image` device pixels after GPUI's
            // scale snap. Snapping origin and size separately at 1.25 (120dpi)
            // made dest_device ≠ bitmap size → skip of columns (vertical slivers).
            // Lock: origin_device + image size.
            let img = s.image.size(0);
            let img_w = img.width.0 as f32;
            let img_h = img.height.0 as f32;
            if img_w < 1. || img_h < 1. {
                continue;
            }
            let ox = f32::from(bounds.origin.x) + s.x;
            let oy = f32::from(bounds.origin.y) + s.y;
            let x0 = (ox * scale).round();
            let y0 = (oy * scale).round();
            let dest = Bounds {
                origin: point(px(x0 / scale), px(y0 / scale)),
                size: size(px(img_w / scale), px(img_h / scale)),
            };
            if f32::from(dest.size.width) * scale < 1. || f32::from(dest.size.height) * scale < 1. {
                continue;
            }
            if let Err(err) = window.paint_image(
                dest,
                dest,
                Corners::default(),
                Arc::clone(&s.image),
                0,
                false,
            ) {
                eprintln!("wezterm-gpui paint_image: {err:#}");
            }
        }
    });
    for img in &paint.drop_images {
        let _ = window.drop_image(Arc::clone(img));
    }
}

fn line_is_blank(line: &Line, pal: &ColorPalette, pane_bg: u32) -> bool {
    let text = line.as_str();
    if !text.chars().all(|c| c == ' ' || c == '\0') {
        return false;
    }
    for cell in line.visible_cells() {
        let attrs = cell.attrs();
        let mut bgc = pal.resolve_bg(attrs.background());
        if attrs.reverse() {
            bgc = pal.resolve_fg(attrs.foreground());
        }
        let (r, g, b, _) = bgc.as_rgba_u8();
        if pack_rgb(r, g, b) != pane_bg {
            return false;
        }
    }
    true
}

fn only_char(s: &str) -> Option<char> {
    let mut chars = s.chars();
    let c = chars.next()?;
    if chars.next().is_some() {
        None
    } else {
        Some(c)
    }
}

fn cell_fg(
    attrs: &wezterm_term::CellAttributes,
    pal: &ColorPalette,
    selected: bool,
    is_cursor: bool,
) -> u32 {
    let mut fg = pal.resolve_fg(attrs.foreground());
    if attrs.reverse() {
        fg = pal.resolve_bg(attrs.background());
    }
    if selected {
        let (_, _, _, a) = pal.selection_fg.as_rgba_u8();
        if a > 0 {
            fg = pal.selection_fg;
        }
    }
    if is_cursor {
        fg = pal.cursor_fg;
    }
    let (r, g, b, _) = fg.as_rgba_u8();
    pack_rgb(r, g, b)
}

fn pack_rgb(r: u8, g: u8, b: u8) -> u32 {
    u32::from_be_bytes([0, r, g, b])
}

fn unpack_rgb(color: u32) -> (u8, u8, u8) {
    (
        ((color >> 16) & 0xff) as u8,
        ((color >> 8) & 0xff) as u8,
        (color & 0xff) as u8,
    )
}

/// Device-pixel cell size like wezterm-gui `RenderMetrics`: ceil after lua
/// `cell_width` / `line_height`. Fractional metrics + `x_advance` left a 1px
/// sliver before the block cursor at 120dpi (030).
fn device_cell_size(metric_w: f64, metric_h: f64) -> (f64, f64) {
    let cfg = config::configuration();
    (
        ceil_device_cell(metric_w, cfg.cell_width),
        ceil_device_cell(metric_h, cfg.line_height),
    )
}

fn ceil_device_cell(metric: f64, scale: f64) -> f64 {
    (metric * scale).max(1.0).ceil()
}

/// Default wezterm-gui glyph step: `num_cells * cell_width`, not `x_advance`.
fn glyph_cell_advance(num_cells: u8, cell_w: f64) -> f64 {
    num_cells.max(1) as f64 * cell_w
}

/// Device-pixel X of the left edge of `col`. With integer `cell_w` this is
/// `col * cell_w`. `round` kept so 029 `cell_span` still abuts if metrics are
/// ever fractional again.
fn cell_edge(col: usize, cell_w: f64) -> i32 {
    (col as f64 * cell_w).round() as i32
}

/// Fill span for `ncells` starting at `col`. Width is `edge(col+n) - edge(col)` so
/// consecutive cells abut. Independent `round(col*w)` + `round(w)` at 1.25 (120dpi)
/// left a 1px gap before the cursor (029).
fn cell_span(col: usize, ncells: usize, cell_w: f64) -> (i32, u32) {
    let x0 = cell_edge(col, cell_w);
    let x1 = cell_edge(col + ncells, cell_w);
    (x0, (x1 - x0).max(1) as u32)
}

/// Cursor column to paint, if it sits inside the sprite. Independent of whether
/// `visible_cells()` stored that col (031).
fn cursor_block_col(cursor_col: Option<usize>, cols: usize) -> Option<usize> {
    cursor_col.filter(|&c| c < cols)
}

fn cursor_ncells(line: &Line, col: usize) -> usize {
    line.get_cell(col).map(|c| c.width().max(1)).unwrap_or(1)
}

fn fill_cursor_block(
    pixels: &mut [u8],
    phys_w: u32,
    phys_h: u32,
    line: &Line,
    cursor_col: Option<usize>,
    cols: usize,
    cell_w: f64,
    pal: &ColorPalette,
) {
    let Some(col) = cursor_block_col(cursor_col, cols) else {
        return;
    };
    let (r, g, b, _) = pal.cursor_bg.as_rgba_u8();
    let (x, w) = cell_span(col, cursor_ncells(line, col), cell_w);
    fill_rect(pixels, phys_w, phys_h, x, 0, w, phys_h, r, g, b, 255);
}

fn stroke_rect(
    pixels: &mut [u8],
    img_w: u32,
    img_h: u32,
    x: i32,
    y: i32,
    w: u32,
    h: u32,
    r: u8,
    g: u8,
    b: u8,
) {
    if w == 0 || h == 0 {
        return;
    }
    fill_rect(pixels, img_w, img_h, x, y, w, 1, r, g, b, 255);
    fill_rect(
        pixels,
        img_w,
        img_h,
        x,
        y.saturating_add_unsigned(h.saturating_sub(1)),
        w,
        1,
        r,
        g,
        b,
        255,
    );
    fill_rect(pixels, img_w, img_h, x, y, 1, h, r, g, b, 255);
    fill_rect(
        pixels,
        img_w,
        img_h,
        x.saturating_add_unsigned(w.saturating_sub(1)),
        y,
        1,
        h,
        r,
        g,
        b,
        255,
    );
}

/// wezterm-gui `shader.wgsl` `apply_hsv`: RGB → HSV, multiply by
/// `inactive_pane_hsb`, HSV → RGB. Default brightness 0.8 / saturation 0.9.
pub(crate) fn apply_hsb_u32(color: u32, t: config::HsbTransform) -> u32 {
    let (r, g, b) = unpack_rgb(color);
    let (r, g, b) = apply_hsb_rgb(r, g, b, t);
    pack_rgb(r, g, b)
}

fn apply_hsb_pixels(pixels: &mut [u8], t: config::HsbTransform) {
    if hsb_is_identity(t) {
        return;
    }
    for px in pixels.chunks_exact_mut(4) {
        let (r, g, b) = apply_hsb_rgb(px[0], px[1], px[2], t);
        px[0] = r;
        px[1] = g;
        px[2] = b;
    }
}

fn hsb_is_identity(t: config::HsbTransform) -> bool {
    (t.hue - 1.0).abs() < 1e-4
        && (t.saturation - 1.0).abs() < 1e-4
        && (t.brightness - 1.0).abs() < 1e-4
}

fn apply_hsb_rgb(r: u8, g: u8, b: u8, t: config::HsbTransform) -> (u8, u8, u8) {
    let (h, s, v) = rgb2hsv(r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0);
    let (nr, ng, nb) = hsv2rgb(h * t.hue, s * t.saturation, v * t.brightness);
    (
        (nr * 255.0).round().clamp(0.0, 255.0) as u8,
        (ng * 255.0).round().clamp(0.0, 255.0) as u8,
        (nb * 255.0).round().clamp(0.0, 255.0) as u8,
    )
}

fn mix_f(a: f32, b: f32, t: f32) -> f32 {
    a * (1.0 - t) + b * t
}

fn step_f(edge: f32, x: f32) -> f32 {
    if x >= edge { 1.0 } else { 0.0 }
}

/// Port of `wezterm-gui/src/shader.wgsl` `rgb2hsv` (Iñigo Quilez).
fn rgb2hsv(r: f32, g: f32, b: f32) -> (f32, f32, f32) {
    let t = step_f(b, g);
    let p0 = mix_f(b, g, t);
    let p1 = mix_f(g, b, t);
    let p2 = mix_f(-1.0, 0.0, t);
    let p3 = mix_f(2.0 / 3.0, -1.0 / 3.0, t);
    let t2 = step_f(p0, r);
    let q0 = mix_f(p0, r, t2);
    let q1 = mix_f(p1, p1, t2);
    let q2 = mix_f(p3, p2, t2);
    let q3 = mix_f(r, p0, t2);
    let d = q0 - q3.min(q1);
    let e = 1.0e-10;
    let h = (q2 + (q3 - q1) / (6.0 * d + e)).abs();
    let s = d / (q0 + e);
    (h, s, q0)
}

/// Port of `wezterm-gui/src/shader.wgsl` `hsv2rgb`.
fn hsv2rgb(h: f32, s: f32, v: f32) -> (f32, f32, f32) {
    let fract = |x: f32| x - x.floor();
    let p0 = (fract(h + 1.0) * 6.0 - 3.0).abs();
    let p1 = (fract(h + 2.0 / 3.0) * 6.0 - 3.0).abs();
    let p2 = (fract(h + 1.0 / 3.0) * 6.0 - 3.0).abs();
    let mixk = |p: f32| mix_f(1.0, (p - 1.0).clamp(0.0, 1.0), s);
    (v * mixk(p0), v * mixk(p1), v * mixk(p2))
}

fn fill_rect(
    pixels: &mut [u8],
    img_w: u32,
    img_h: u32,
    x: i32,
    y: i32,
    w: u32,
    h: u32,
    r: u8,
    g: u8,
    b: u8,
    a: u8,
) {
    if w == 0 || h == 0 {
        return;
    }
    let x0 = x.max(0) as u32;
    let y0 = y.max(0) as u32;
    let x1 = (x.saturating_add_unsigned(w) as u32).min(img_w);
    let y1 = (y.saturating_add_unsigned(h) as u32).min(img_h);
    for py in y0..y1 {
        for px in x0..x1 {
            let i = ((py * img_w + px) * 4) as usize;
            pixels[i] = r;
            pixels[i + 1] = g;
            pixels[i + 2] = b;
            pixels[i + 3] = a;
        }
    }
}

fn blit_glyph(
    dest: &mut [u8],
    img_w: u32,
    img_h: u32,
    dx: i32,
    dy: i32,
    gw: u32,
    gh: u32,
    src: &[u8],
    has_color: bool,
    fg: u32,
) {
    let (fr, fg_g, fb) = unpack_rgb(fg);
    for gy in 0..gh {
        let py = dy + gy as i32;
        if py < 0 || py >= img_h as i32 {
            continue;
        }
        for gx in 0..gw {
            let px = dx + gx as i32;
            if px < 0 || px >= img_w as i32 {
                continue;
            }
            let si = ((gy * gw + gx) * 4) as usize;
            let sa = src.get(si + 3).copied().unwrap_or(0);
            if sa == 0 {
                continue;
            }
            let di = ((py as u32 * img_w + px as u32) * 4) as usize;
            if has_color {
                // Straight-alpha color emoji.
                let inv = 255 - sa as u32;
                dest[di] = ((src[si] as u32 * sa as u32 + dest[di] as u32 * inv) / 255) as u8;
                dest[di + 1] =
                    ((src[si + 1] as u32 * sa as u32 + dest[di + 1] as u32 * inv) / 255) as u8;
                dest[di + 2] =
                    ((src[si + 2] as u32 * sa as u32 + dest[di + 2] as u32 * inv) / 255) as u8;
                dest[di + 3] = 255;
                continue;
            }
            // wezterm-gui grayscale: out = sRGB_fg * linear_a + sRGB_bg * (1-linear_a).
            // LCD: per-channel coverage (RGB is sRGB-encoded linear coverage).
            let sr = src[si];
            let sg = src[si + 1];
            let sb = src[si + 2];
            let (cr, cg, cb) = if sr == sg && sg == sb {
                let c = sa as f32 / 255.0;
                (c, c, c)
            } else {
                (
                    srgb8_to_linear(sr),
                    srgb8_to_linear(sg),
                    srgb8_to_linear(sb),
                )
            };
            dest[di] = lerp_u8(dest[di], fr, cr);
            dest[di + 1] = lerp_u8(dest[di + 1], fg_g, cg);
            dest[di + 2] = lerp_u8(dest[di + 2], fb, cb);
            dest[di + 3] = 255;
        }
    }
}

fn glyph_coverage(raster: &RasterizedGlyph) -> Vec<u8> {
    let w = raster.width.max(1) as usize;
    let h = raster.height.max(1) as usize;
    let mut pixels = vec![0u8; w * h * 4];
    if raster.width > 0 && raster.height > 0 {
        let row_bytes = raster.width * 4;
        for gy in 0..raster.height {
            let src = gy * row_bytes;
            let dst = gy * w * 4;
            let n = row_bytes.min(w * 4);
            if src + n <= raster.data.len() && dst + n <= pixels.len() {
                pixels[dst..dst + n].copy_from_slice(&raster.data[src..src + n]);
            }
        }
    }
    pixels
}

fn lerp_u8(bg: u8, fg: u8, t: f32) -> u8 {
    (bg as f32 * (1.0 - t) + fg as f32 * t)
        .round()
        .clamp(0.0, 255.0) as u8
}

/// Inverse of wezterm-font `linear_u8_to_srgb8` (coverage stored as sRGB).
fn srgb8_to_linear(c: u8) -> f32 {
    let x = c as f32 / 255.0;
    if x <= 0.04045 {
        x / 12.92
    } else {
        ((x + 0.055) / 1.055).powf(2.4)
    }
}

#[cfg(test)]
mod cell_span_tests {
    use super::*;

    #[test]
    fn consecutive_cells_abut_at_fractional_width() {
        // 120dpi-ish: round(col*w)+round(w) can skip a pixel (col 6, w=10.4).
        let cell_w = 10.4;
        let mut x = 0i32;
        for col in 0..20 {
            let (x0, w) = cell_span(col, 1, cell_w);
            assert_eq!(x0, x, "gap before col {col}");
            x = x0 + w as i32;
        }
        assert_eq!(x, cell_edge(20, cell_w));
    }

    #[test]
    fn independent_round_would_gap() {
        let cell_w = 10.4;
        let col = 6;
        let old_x = (col as f64 * cell_w).round() as i32;
        let old_w = (cell_w).round().max(1.0) as u32;
        let next = ((col + 1) as f64 * cell_w).round() as i32;
        assert!(
            old_x + old_w as i32 != next,
            "fixture: expected independent round to gap"
        );
        let (x0, w) = cell_span(col, 1, cell_w);
        assert_eq!(x0 + w as i32, cell_span(col + 1, 1, cell_w).0);
    }

    #[test]
    fn ceil_device_cell_matches_wezterm_gui() {
        assert_eq!(ceil_device_cell(10.4, 1.0), 11.0);
        assert_eq!(ceil_device_cell(8.0, 1.0), 8.0);
        assert_eq!(ceil_device_cell(10.4, 1.1), 12.0);
    }

    #[test]
    fn force_width_stays_on_cursor_grid() {
        let cell_w = 11.0;
        let mut x = 0.0;
        for _ in 0..23 {
            x += glyph_cell_advance(1, cell_w);
        }
        assert_eq!(x, 23.0 * cell_w);
        assert_eq!(cell_edge(23, cell_w), 23 * 11);
        // HarfBuzz-style 10.4 advance would sit ~14px left of the cursor.
        let drifted = 23.0 * 10.4;
        assert!((23.0 * cell_w - drifted).round() >= 1.0);
    }

    #[test]
    fn hsb_identity_keeps_dracula_bg() {
        let t = config::HsbTransform {
            hue: 1.0,
            saturation: 1.0,
            brightness: 1.0,
        };
        let color = pack_rgb(0x28, 0x2a, 0x36);
        assert_eq!(apply_hsb_u32(color, t), color);
    }

    #[test]
    fn default_inactive_hsb_darkens_dracula_bg() {
        let t = config::HsbTransform {
            hue: 1.0,
            saturation: 0.9,
            brightness: 0.8,
        };
        let (r, g, b) = unpack_rgb(apply_hsb_u32(pack_rgb(0x28, 0x2a, 0x36), t));
        assert!(r < 0x28 && g < 0x2a && b < 0x36, "got #{r:02x}{g:02x}{b:02x}");
    }

    #[test]
    fn cursor_past_visible_cells_still_paints() {
        // Prompt / typed char / space: stored cells 0..n, VT cursor at n.
        let visible = [0usize, 1, 2];
        let cursor = 3;
        assert!(
            !visible.contains(&cursor),
            "fixture: cursor past last stored cell"
        );
        assert_eq!(cursor_block_col(Some(cursor), 80), Some(3));
        assert_eq!(cursor_block_col(Some(80), 80), None);
        assert_eq!(cursor_block_col(None, 80), None);
    }
}
