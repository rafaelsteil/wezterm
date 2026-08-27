//! wezterm-font glyphs composited into cached per-line GPUI sprites.
//! Cell backgrounds live in the same row bitmap. Not a viewport bitmap
//! (decision 010) and not the wezterm-gui GPU atlas. See decision 017.

use std::collections::{HashMap, VecDeque};
use std::rc::Rc;
use std::sync::Arc;

use gpui::{
    fill, point, px, rgb, size, Bounds, ContentMask, Corners, Pixels, RenderImage, Window,
};
use image::{Frame, RgbaImage};
use wezterm_bidi::Direction;
use wezterm_font::{FontConfiguration, LoadedFont, LoadedFontId, RasterizedGlyph};
use wezterm_term::color::ColorPalette;
use wezterm_term::Line;

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
}

struct CachedRow {
    image: Arc<RenderImage>,
    width: f32,
    height: f32,
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
    w: f32,
    h: f32,
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
        let dpr = self.dpr() as f32;
        Ok(CellSize {
            width: m.cell_width.get().max(1.0) as f32 / dpr,
            height: m.cell_height.get().max(1.0) as f32 / dpr,
        })
    }

    /// `sel_cols` is per visible row: exclusive column range, or `(u16::MAX, u16::MAX)` if none.
    pub fn layout(
        &mut self,
        lines: &[Line],
        cursor: Option<(usize, usize)>,
        pal: &ColorPalette,
        cols: usize,
        sel_cols: &[(u16, u16)],
    ) -> anyhow::Result<TermPaint> {
        let font = self.fonts.default_font()?;
        let metrics = font.metrics();
        let dpr = self.dpr();
        let cell_w = metrics.cell_width.get().max(1.0);
        let cell_h = metrics.cell_height.get().max(1.0);
        let descender = metrics.descender.get();
        let (br, bg, bb, _) = pal.background.as_rgba_u8();
        let pane_bg = pack_rgb(br, bg, bb);
        let cols = cols.max(1).min(400);
        let phys_w = (cols as f64 * cell_w).round().max(1.0);
        let phys_h = cell_h.round().max(1.0);
        let key_cell_w = phys_w as u16;
        let key_cell_h = phys_h as u16;
        let logical_w = (phys_w / dpr) as f32;
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
                    )?;
                    self.store_row(
                        key,
                        CachedRow {
                            image,
                            width: logical_w,
                            height: logical_h,
                        },
                    )
                }
            };
            sprites.push(TermSprite {
                x: 0.0,
                y: (row as f64 * cell_h / dpr) as f32,
                w: cached.width,
                h: cached.height,
                image: Arc::clone(&cached.image),
            });
        }

        Ok(TermPaint {
            bg: pane_bg,
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

        let sel_start_u = sel_start as usize;
        let sel_end_u = (sel_end as usize).min(cols);
        let has_sel = sel_start != u16::MAX && sel_end_u > sel_start_u;
        if has_sel {
            let (sr, sg, sb, _) = pal.selection_bg.as_rgba_u8();
            let x = (sel_start_u as f64 * cell_w).round() as i32;
            let w = ((sel_end_u - sel_start_u) as f64 * cell_w).round().max(1.0) as u32;
            fill_rect(&mut pixels, phys_w, phys_h, x, 0, w, phys_h, sr, sg, sb, 255);
        }

        for cell in line.visible_cells() {
            let col = cell.cell_index();
            if col >= cols {
                continue;
            }
            let attrs = cell.attrs();
            let is_cursor = cursor_col == Some(col);
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
            let x = (col as f64 * cell_w).round() as i32;
            let w = (cell.width() as f64 * cell_w).round().max(1.0) as u32;
            fill_rect(&mut pixels, phys_w, phys_h, x, 0, w, phys_h, r, g, b, 255);
        }

        let text = line.as_str();
        if !text.chars().all(|c| c == ' ' || c == '\0') {
            match font.blocking_shape(text.as_ref(), None, Direction::LeftToRight, None, None) {
                Ok(glyphs) => {
                    let mut x_pos = 0.0;
                    for info in glyphs {
                        if info.is_space || info.glyph_pos == 0 {
                            x_pos += info.x_advance.get();
                            continue;
                        }
                        let col = (x_pos / cell_w).floor().max(0.0) as usize;
                        let attrs = line
                            .get_cell(col)
                            .map(|c| c.attrs().clone())
                            .unwrap_or_default();
                        let is_cursor = cursor_col == Some(col);
                        let selected = has_sel && col >= sel_start_u && col < sel_end_u;
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
                        let (fr, fg_g, fb, _) = fg.as_rgba_u8();
                        let fg_u = pack_rgb(fr, fg_g, fb);
                        if let Ok(glyph) = self.cached_glyph(font, info.glyph_pos, info.font_idx) {
                            let dx = (x_pos + info.x_offset.get() + glyph.bearing_x).round() as i32;
                            let dy = (cell_h + descender
                                - (info.y_offset.get() + glyph.bearing_y))
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
                        x_pos += info.x_advance.get();
                    }
                }
                Err(err) => eprintln!("wezterm-gpui shape: {err:#}"),
            }
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
    let snap_px = |v: f32| (v * scale).round() / scale;
    window.with_content_mask(Some(ContentMask { bounds }), |window| {
        window.paint_quad(fill(bounds, rgb(paint.bg)));
        for s in &paint.sprites {
            if s.w * scale < 1. || s.h * scale < 1. {
                continue;
            }
            let x0 = snap_px(s.x);
            let y0 = snap_px(s.y);
            let x1 = snap_px(s.x + s.w);
            let y1 = snap_px(s.y + s.h);
            let image_bounds = Bounds {
                origin: bounds.origin + point(px(x0), px(y0)),
                size: size(px((x1 - x0).max(0.)), px((y1 - y0).max(0.))),
            };
            if let Err(err) = window.paint_image(
                bounds,
                image_bounds,
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
    (bg as f32 * (1.0 - t) + fg as f32 * t).round().clamp(0.0, 255.0) as u8
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
