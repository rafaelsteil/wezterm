//! wezterm-font glyphs as cached GPUI sprites (stable `ImageId` per glyph).
//! Cell backgrounds are `paint_quad`. Not a viewport bitmap and not the
//! wezterm-gui GPU atlas. See decision 010.

use std::collections::HashMap;
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

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
struct GlyphKey {
    font_id: LoadedFontId,
    glyph_pos: u32,
    font_idx: usize,
    /// 0 for color glyphs; otherwise 0xRRGGBB of the cell foreground.
    fg: u32,
}

struct CachedGlyph {
    width: f32,
    height: f32,
    bearing_x: f64,
    bearing_y: f64,
    image: Arc<RenderImage>,
}

pub struct GlyphPainter {
    fonts: Rc<FontConfiguration>,
    cache: HashMap<GlyphKey, Rc<CachedGlyph>>,
    pending_drop: Vec<Arc<RenderImage>>,
}

pub struct CellSize {
    pub width: f32,
    pub height: f32,
}

pub struct TermQuad {
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    color: u32,
}

pub struct TermSprite {
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    image: Arc<RenderImage>,
}

pub struct TermPaint {
    pub bg: u32,
    pub quads: Vec<TermQuad>,
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
            cache: HashMap::new(),
            pending_drop: Vec::new(),
        })
    }

    pub fn sync_font_px(&mut self, font_px: f32) {
        let scale = (font_px as f64 / 12.0).clamp(0.5, 4.0);
        if (scale - self.fonts.get_font_scale()).abs() > 0.01 {
            self.fonts.change_scaling(scale, 96);
            for (_, g) in self.cache.drain() {
                self.pending_drop.push(Arc::clone(&g.image));
            }
        }
    }

    pub fn cell_size(&self) -> anyhow::Result<CellSize> {
        let m = self.fonts.default_font_metrics()?;
        Ok(CellSize {
            width: m.cell_width.get().max(1.0) as f32,
            height: m.cell_height.get().max(1.0) as f32,
        })
    }

    pub fn layout(
        &mut self,
        lines: &[Line],
        cursor: Option<(usize, usize)>,
        pal: &ColorPalette,
    ) -> anyhow::Result<TermPaint> {
        let font = self.fonts.default_font()?;
        let metrics = font.metrics();
        let cell_w = metrics.cell_width.get().max(1.0);
        let cell_h = metrics.cell_height.get().max(1.0);
        let descender = metrics.descender.get();
        let (br, bg, bb, _) = pal.background.as_rgba_u8();
        let pane_bg = pack_rgb(br, bg, bb);

        let mut quads = Vec::new();
        let mut sprites = Vec::new();

        for (row, line) in lines.iter().enumerate() {
            let y0 = (row as f64 * cell_h) as f32;
            for cell in line.visible_cells() {
                let col = cell.cell_index();
                let x0 = (col as f64 * cell_w) as f32;
                let attrs = cell.attrs();
                let is_cursor = cursor == Some((row, col));
                let mut bgc = pal.resolve_bg(attrs.background());
                if attrs.reverse() {
                    bgc = pal.resolve_fg(attrs.foreground());
                }
                if is_cursor {
                    bgc = pal.cursor_bg;
                }
                let (r, g, b, _) = bgc.as_rgba_u8();
                let color = pack_rgb(r, g, b);
                if color != pane_bg || is_cursor {
                    quads.push(TermQuad {
                        x: x0,
                        y: y0,
                        w: (cell.width() as f64 * cell_w).max(1.0) as f32,
                        h: cell_h as f32,
                        color,
                    });
                }
            }

            let text = line.as_str();
            if text.chars().all(|c| c == ' ' || c == '\0') {
                continue;
            }
            let glyphs = match font.blocking_shape(
                text.as_ref(),
                None,
                Direction::LeftToRight,
                None,
                None,
            ) {
                Ok(g) => g,
                Err(err) => {
                    eprintln!("wezterm-gpui shape: {err:#}");
                    continue;
                }
            };

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
                let is_cursor = cursor == Some((row, col));
                let mut fg = pal.resolve_fg(attrs.foreground());
                if attrs.reverse() {
                    fg = pal.resolve_bg(attrs.background());
                }
                if is_cursor {
                    fg = pal.cursor_fg;
                }
                let (fr, fg_g, fb, _) = fg.as_rgba_u8();
                let fg_u = pack_rgb(fr, fg_g, fb);

                if let Ok(glyph) = self.cached_glyph(&font, info.glyph_pos, info.font_idx, fg_u) {
                    let dx = (x_pos + info.x_offset.get() + glyph.bearing_x) as f32;
                    let dy = (y0 as f64 + cell_h + descender
                        - (info.y_offset.get() + glyph.bearing_y)) as f32;
                    sprites.push(TermSprite {
                        x: dx,
                        y: dy,
                        w: glyph.width,
                        h: glyph.height,
                        image: Arc::clone(&glyph.image),
                    });
                }
                x_pos += info.x_advance.get();
            }
        }

        Ok(TermPaint {
            bg: pane_bg,
            quads,
            sprites,
            drop_images: std::mem::take(&mut self.pending_drop),
        })
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
        if let Some(g) = self.cache.get(&tinted) {
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
        if let Some(g) = self.cache.get(&key) {
            return Ok(Rc::clone(g));
        }
        let image = glyph_to_image(&raster, fg)?;
        let cached = Rc::new(CachedGlyph {
            width: raster.width.max(1) as f32,
            height: raster.height.max(1) as f32,
            bearing_x: raster.bearing_x.get(),
            bearing_y: raster.bearing_y.get(),
            image,
        });
        self.cache.insert(key, Rc::clone(&cached));
        Ok(cached)
    }
}

pub fn paint_term(window: &mut Window, bounds: Bounds<Pixels>, paint: &TermPaint) {
    window.with_content_mask(Some(ContentMask { bounds }), |window| {
        window.paint_quad(fill(bounds, rgb(paint.bg)));
        for q in &paint.quads {
            let r = Bounds {
                origin: bounds.origin + point(px(q.x), px(q.y)),
                size: size(px(q.w), px(q.h)),
            };
            window.paint_quad(fill(r, rgb(q.color)));
        }
        for s in &paint.sprites {
            if s.w < 1. || s.h < 1. {
                continue;
            }
            let image_bounds = Bounds {
                origin: bounds.origin + point(px(s.x), px(s.y)),
                size: size(px(s.w), px(s.h)),
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

fn pack_rgb(r: u8, g: u8, b: u8) -> u32 {
    u32::from_be_bytes([0, r, g, b])
}

fn glyph_to_image(raster: &RasterizedGlyph, fg: u32) -> anyhow::Result<Arc<RenderImage>> {
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
    let rgba = RgbaImage::from_raw(w, h, pixels)
        .ok_or_else(|| anyhow::anyhow!("glyph sprite size mismatch"))?;
    Ok(Arc::new(RenderImage::new(vec![Frame::new(rgba)])))
}
