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
    /// 0 for color glyphs; otherwise 0xRRGGBB of the cell foreground.
    fg: u32,
}

struct CachedGlyph {
    width: u32,
    height: u32,
    bearing_x: f64,
    bearing_y: f64,
    /// RGBA, non-premultiplied.
    data: Arc<Vec<u8>>,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
struct RowKey {
    hash: [u8; 16],
    cursor_col: u16,
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
    pub fn new() -> anyhow::Result<Self> {
        let fonts = Rc::new(FontConfiguration::new(None, 96)?);
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
        let scale = (font_px as f64 / 12.0).clamp(0.5, 4.0);
        if (scale - self.fonts.get_font_scale()).abs() > 0.01 || dpi != self.fonts.get_dpi() {
            self.fonts.change_scaling(scale, dpi);
            self.glyphs.clear();
            self.drain_rows();
        }
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

    pub fn layout(
        &mut self,
        lines: &[Line],
        cursor: Option<(usize, usize)>,
        pal: &ColorPalette,
        cols: usize,
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
            if cursor_col == u16::MAX && line_is_blank(line, pal, pane_bg) {
                continue;
            }
            let key = RowKey {
                hash: line.compute_shape_hash(),
                cursor_col,
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

        for cell in line.visible_cells() {
            let col = cell.cell_index();
            if col >= cols {
                continue;
            }
            let attrs = cell.attrs();
            let is_cursor = cursor_col == Some(col);
            let mut bgc = pal.resolve_bg(attrs.background());
            if attrs.reverse() {
                bgc = pal.resolve_fg(attrs.foreground());
            }
            if is_cursor {
                bgc = pal.cursor_bg;
            }
            let (r, g, b, _) = bgc.as_rgba_u8();
            let color = pack_rgb(r, g, b);
            if color == pane_bg && !is_cursor {
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
                        let mut fg = pal.resolve_fg(attrs.foreground());
                        if attrs.reverse() {
                            fg = pal.resolve_bg(attrs.background());
                        }
                        if is_cursor {
                            fg = pal.cursor_fg;
                        }
                        let (fr, fg_g, fb, _) = fg.as_rgba_u8();
                        let fg_u = pack_rgb(fr, fg_g, fb);
                        if let Ok(glyph) =
                            self.cached_glyph(font, info.glyph_pos, info.font_idx, fg_u)
                        {
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
                            );
                        }
                        x_pos += info.x_advance.get();
                    }
                }
                Err(err) => eprintln!("wezterm-gpui shape: {err:#}"),
            }
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
        fg: u32,
    ) -> anyhow::Result<Rc<CachedGlyph>> {
        let tinted = GlyphKey {
            font_id: font.id(),
            glyph_pos,
            font_idx,
            fg,
        };
        if let Some(g) = self.glyphs.get(&tinted) {
            return Ok(Rc::clone(g));
        }
        let raster: RasterizedGlyph = font.rasterize_glyph(glyph_pos, font_idx)?;
        let key_fg = if raster.has_color { 0 } else { fg };
        let key = GlyphKey {
            font_id: font.id(),
            glyph_pos,
            font_idx,
            fg: key_fg,
        };
        if let Some(g) = self.glyphs.get(&key) {
            return Ok(Rc::clone(g));
        }
        let data = glyph_to_rgba(&raster, fg);
        let cached = Rc::new(CachedGlyph {
            width: raster.width.max(1) as u32,
            height: raster.height.max(1) as u32,
            bearing_x: raster.bearing_x.get(),
            bearing_y: raster.bearing_y.get(),
            data: Arc::new(data),
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
) {
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
            let sa = src.get(si + 3).copied().unwrap_or(0) as u32;
            if sa == 0 {
                continue;
            }
            let di = ((py as u32 * img_w + px as u32) * 4) as usize;
            let inv = 255 - sa;
            dest[di] = ((src[si] as u32 * sa + dest[di] as u32 * inv) / 255) as u8;
            dest[di + 1] = ((src[si + 1] as u32 * sa + dest[di + 1] as u32 * inv) / 255) as u8;
            dest[di + 2] = ((src[si + 2] as u32 * sa + dest[di + 2] as u32 * inv) / 255) as u8;
            dest[di + 3] = 255;
        }
    }
}

fn glyph_to_rgba(raster: &RasterizedGlyph, fg: u32) -> Vec<u8> {
    let w = raster.width.max(1) as u32;
    let h = raster.height.max(1) as u32;
    let fr = ((fg >> 16) & 0xff) as u32;
    let fg_g = ((fg >> 8) & 0xff) as u32;
    let fb = (fg & 0xff) as u32;
    let mut pixels = vec![0u8; (w * h * 4) as usize];
    if raster.width > 0 && raster.height > 0 {
        for gy in 0..raster.height {
            for gx in 0..raster.width {
                let si = (gy * raster.width + gx) * 4;
                let sa = raster.data.get(si + 3).copied().unwrap_or(0) as u32;
                if sa == 0 {
                    continue;
                }
                let (sr, sg, sb) = if raster.has_color {
                    (
                        raster.data[si] as u32,
                        raster.data[si + 1] as u32,
                        raster.data[si + 2] as u32,
                    )
                } else {
                    (fr * sa / 255, fg_g * sa / 255, fb * sa / 255)
                };
                let di = ((gy as u32 * w + gx as u32) * 4) as usize;
                pixels[di] = sr.min(255) as u8;
                pixels[di + 1] = sg.min(255) as u8;
                pixels[di + 2] = sb.min(255) as u8;
                pixels[di + 3] = sa.min(255) as u8;
            }
        }
    }
    pixels
}
