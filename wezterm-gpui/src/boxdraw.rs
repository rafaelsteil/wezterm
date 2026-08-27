//! Geometry for Unicode box-drawing and block elements (U+2500–259F).
//!
//! Painted into the per-line CPU sprite (decision 017), not as font glyphs.
//! Thickness snaps to device pixels so adjacent cells tile. Approach follows
//! tty7 `src/terminal/boxdraw.rs` (Apache-2.0); rewritten for a bitmap instead
//! of GPUI `Path` / `paint_quad`. Powerline triangles are out of this slice.

/// True for U+2500 BOX DRAWINGS LIGHT HORIZONTAL through U+259F QUADRANT.

pub fn is_box_draw(c: char) -> bool {
    ('\u{2500}'..='\u{259f}').contains(&c)
}

/// Rasterize `c` into the cell `[x0, x1) × [y0, y1)` of `pixels` (RGBA, row-major).
/// Returns true if the codepoint is in range (caller should skip the font glyph).
pub fn paint(
    pixels: &mut [u8],
    img_w: u32,
    img_h: u32,
    x0: f32,
    y0: f32,
    x1: f32,
    y1: f32,
    c: char,
    fg: (u8, u8, u8),
    aa: bool,
) -> bool {
    if !is_box_draw(c) {
        return false;
    }
    let cell = Cell::new(x0, y0, x1, y1);
    let mut t = Target {
        pixels,
        img_w,
        img_h,
        clip_x0: x0.round() as i32,
        clip_y0: y0.round() as i32,
        clip_x1: x1.round() as i32,
        clip_y1: y1.round() as i32,
        fg,
        aa,
    };
    if let Some((u, d, l, r)) = arms_of(c) {
        cell.arms(&mut t, u, d, l, r);
        return true;
    }
    cell.doubles(&mut t, c)
        || cell.rounded(&mut t, c)
        || cell.dashed(&mut t, c)
        || cell.diagonal(&mut t, c)
        || cell.blocks(&mut t, c)
}

#[derive(Clone, Copy, PartialEq)]
enum Arm {
    None,
    Light,
    Heavy,
}

#[derive(Clone, Copy)]
struct Cell {
    x0: f32,
    y0: f32,
    x1: f32,
    y1: f32,
    cx: f32,
    cy: f32,
    t: f32,
}

struct Target<'a> {
    pixels: &'a mut [u8],
    img_w: u32,
    img_h: u32,
    clip_x0: i32,
    clip_y0: i32,
    clip_x1: i32,
    clip_y1: i32,
    fg: (u8, u8, u8),
    aa: bool,
}

impl Cell {
    fn new(x0: f32, y0: f32, x1: f32, y1: f32) -> Self {
        let w = (x1 - x0).max(1.0);
        Cell {
            x0,
            y0,
            x1,
            y1,
            cx: (x0 + x1) * 0.5,
            cy: (y0 + y1) * 0.5,
            t: (w * 0.15).round().max(1.0),
        }
    }

    fn snap(v: f32) -> f32 {
        v.round()
    }

    fn weight(&self, a: Arm) -> f32 {
        match a {
            Arm::None => 0.0,
            Arm::Light => self.t,
            Arm::Heavy => self.t * 2.0,
        }
    }

    fn vstroke(&self, t: &mut Target<'_>, x: f32, w: f32, ya: f32, yb: f32) {
        let x0 = Self::snap(x - w * 0.5);
        let y0 = Self::snap(ya);
        let y1 = Self::snap(yb);
        let ww = Self::snap(x + w * 0.5) - x0;
        t.fill_rect(x0, y0, ww.max(1.0), (y1 - y0).max(1.0), 1.0);
    }

    fn hstroke(&self, t: &mut Target<'_>, y: f32, w: f32, xa: f32, xb: f32) {
        let y0 = Self::snap(y - w * 0.5);
        let x0 = Self::snap(xa);
        let x1 = Self::snap(xb);
        let hh = Self::snap(y + w * 0.5) - y0;
        t.fill_rect(x0, y0, (x1 - x0).max(1.0), hh.max(1.0), 1.0);
    }

    fn rect(&self, t: &mut Target<'_>, x: f32, y: f32, w: f32, h: f32) {
        let x0 = Self::snap(x);
        let y0 = Self::snap(y);
        let x1 = Self::snap(x + w);
        let y1 = Self::snap(y + h);
        t.fill_rect(x0, y0, (x1 - x0).max(1.0), (y1 - y0).max(1.0), 1.0);
    }

    fn shade(&self, t: &mut Target<'_>, alpha: f32) {
        let x0 = Self::snap(self.x0);
        let y0 = Self::snap(self.y0);
        let x1 = Self::snap(self.x1);
        let y1 = Self::snap(self.y1);
        t.fill_rect(x0, y0, (x1 - x0).max(1.0), (y1 - y0).max(1.0), alpha);
    }

    fn arms(&self, t: &mut Target<'_>, u: Arm, d: Arm, l: Arm, r: Arm) {
        let (wu, wd, wl, wr) = (
            self.weight(u),
            self.weight(d),
            self.weight(l),
            self.weight(r),
        );
        let m = wu.max(wd).max(wl).max(wr) * 0.5;
        if wu > 0.0 {
            self.vstroke(t, self.cx, wu, self.y0, self.cy + m);
        }
        if wd > 0.0 {
            self.vstroke(t, self.cx, wd, self.cy - m, self.y1);
        }
        if wl > 0.0 {
            self.hstroke(t, self.cy, wl, self.x0, self.cx + m);
        }
        if wr > 0.0 {
            self.hstroke(t, self.cy, wr, self.cx - m, self.x1);
        }
    }

    fn doubles(&self, t: &mut Target<'_>, c: char) -> bool {
        let th = self.t;
        let h = th * 0.5;
        let d = (th * 1.5).max(2.0);
        let (x0, x1, y0, y1, cx, cy) = (self.x0, self.x1, self.y0, self.y1, self.cx, self.cy);
        let (va, vb) = (cx - d, cx + d);
        let (ha, hb) = (cy - d, cy + d);
        match c {
            '═' => {
                self.hstroke(t, ha, th, x0, x1);
                self.hstroke(t, hb, th, x0, x1);
            }
            '║' => {
                self.vstroke(t, va, th, y0, y1);
                self.vstroke(t, vb, th, y0, y1);
            }
            '╒' => {
                self.hstroke(t, ha, th, cx - h, x1);
                self.hstroke(t, hb, th, cx - h, x1);
                self.vstroke(t, cx, th, ha - h, y1);
            }
            '╓' => {
                self.hstroke(t, cy, th, va - h, x1);
                self.vstroke(t, va, th, cy - h, y1);
                self.vstroke(t, vb, th, cy - h, y1);
            }
            '╔' => {
                self.vstroke(t, va, th, ha - h, y1);
                self.hstroke(t, ha, th, va - h, x1);
                self.vstroke(t, vb, th, hb - h, y1);
                self.hstroke(t, hb, th, vb - h, x1);
            }
            '╕' => {
                self.hstroke(t, ha, th, x0, cx + h);
                self.hstroke(t, hb, th, x0, cx + h);
                self.vstroke(t, cx, th, ha - h, y1);
            }
            '╖' => {
                self.hstroke(t, cy, th, x0, vb + h);
                self.vstroke(t, va, th, cy - h, y1);
                self.vstroke(t, vb, th, cy - h, y1);
            }
            '╗' => {
                self.vstroke(t, vb, th, ha - h, y1);
                self.hstroke(t, ha, th, x0, vb + h);
                self.vstroke(t, va, th, hb - h, y1);
                self.hstroke(t, hb, th, x0, va + h);
            }
            '╘' => {
                self.vstroke(t, cx, th, y0, hb + h);
                self.hstroke(t, ha, th, cx - h, x1);
                self.hstroke(t, hb, th, cx - h, x1);
            }
            '╙' => {
                self.vstroke(t, va, th, y0, cy + h);
                self.vstroke(t, vb, th, y0, cy + h);
                self.hstroke(t, cy, th, va - h, x1);
            }
            '╚' => {
                self.vstroke(t, va, th, y0, hb + h);
                self.hstroke(t, hb, th, va - h, x1);
                self.vstroke(t, vb, th, y0, ha + h);
                self.hstroke(t, ha, th, vb - h, x1);
            }
            '╛' => {
                self.vstroke(t, cx, th, y0, hb + h);
                self.hstroke(t, ha, th, x0, cx + h);
                self.hstroke(t, hb, th, x0, cx + h);
            }
            '╜' => {
                self.vstroke(t, va, th, y0, cy + h);
                self.vstroke(t, vb, th, y0, cy + h);
                self.hstroke(t, cy, th, x0, vb + h);
            }
            '╝' => {
                self.vstroke(t, vb, th, y0, hb + h);
                self.hstroke(t, hb, th, x0, vb + h);
                self.vstroke(t, va, th, y0, ha + h);
                self.hstroke(t, ha, th, x0, va + h);
            }
            '╞' => {
                self.vstroke(t, cx, th, y0, y1);
                self.hstroke(t, ha, th, cx - h, x1);
                self.hstroke(t, hb, th, cx - h, x1);
            }
            '╟' => {
                self.vstroke(t, va, th, y0, y1);
                self.vstroke(t, vb, th, y0, y1);
                self.hstroke(t, cy, th, vb - h, x1);
            }
            '╠' => {
                self.vstroke(t, va, th, y0, y1);
                self.vstroke(t, vb, th, y0, ha + h);
                self.vstroke(t, vb, th, hb - h, y1);
                self.hstroke(t, ha, th, vb - h, x1);
                self.hstroke(t, hb, th, vb - h, x1);
            }
            '╡' => {
                self.vstroke(t, cx, th, y0, y1);
                self.hstroke(t, ha, th, x0, cx + h);
                self.hstroke(t, hb, th, x0, cx + h);
            }
            '╢' => {
                self.vstroke(t, va, th, y0, y1);
                self.vstroke(t, vb, th, y0, y1);
                self.hstroke(t, cy, th, x0, va + h);
            }
            '╣' => {
                self.vstroke(t, vb, th, y0, y1);
                self.vstroke(t, va, th, y0, ha + h);
                self.vstroke(t, va, th, hb - h, y1);
                self.hstroke(t, ha, th, x0, va + h);
                self.hstroke(t, hb, th, x0, va + h);
            }
            '╤' => {
                self.hstroke(t, ha, th, x0, x1);
                self.hstroke(t, hb, th, x0, x1);
                self.vstroke(t, cx, th, hb - h, y1);
            }
            '╥' => {
                self.hstroke(t, cy, th, x0, x1);
                self.vstroke(t, va, th, cy - h, y1);
                self.vstroke(t, vb, th, cy - h, y1);
            }
            '╦' => {
                self.hstroke(t, ha, th, x0, x1);
                self.hstroke(t, hb, th, x0, va + h);
                self.hstroke(t, hb, th, vb - h, x1);
                self.vstroke(t, va, th, hb - h, y1);
                self.vstroke(t, vb, th, hb - h, y1);
            }
            '╧' => {
                self.hstroke(t, ha, th, x0, x1);
                self.hstroke(t, hb, th, x0, x1);
                self.vstroke(t, cx, th, y0, ha + h);
            }
            '╨' => {
                self.hstroke(t, cy, th, x0, x1);
                self.vstroke(t, va, th, y0, cy + h);
                self.vstroke(t, vb, th, y0, cy + h);
            }
            '╩' => {
                self.hstroke(t, hb, th, x0, x1);
                self.hstroke(t, ha, th, x0, va + h);
                self.hstroke(t, ha, th, vb - h, x1);
                self.vstroke(t, va, th, y0, ha + h);
                self.vstroke(t, vb, th, y0, ha + h);
            }
            '╪' => {
                self.vstroke(t, cx, th, y0, y1);
                self.hstroke(t, ha, th, x0, x1);
                self.hstroke(t, hb, th, x0, x1);
            }
            '╫' => {
                self.vstroke(t, va, th, y0, y1);
                self.vstroke(t, vb, th, y0, y1);
                self.hstroke(t, cy, th, x0, x1);
            }
            '╬' => {
                self.vstroke(t, va, th, y0, ha + h);
                self.vstroke(t, vb, th, y0, ha + h);
                self.vstroke(t, va, th, hb - h, y1);
                self.vstroke(t, vb, th, hb - h, y1);
                self.hstroke(t, ha, th, x0, va + h);
                self.hstroke(t, ha, th, vb - h, x1);
                self.hstroke(t, hb, th, x0, va + h);
                self.hstroke(t, hb, th, vb - h, x1);
            }
            _ => return false,
        }
        true
    }

    fn rounded(&self, t: &mut Target<'_>, c: char) -> bool {
        let (sx, sy): (f32, f32) = match c {
            '╭' => (1.0, 1.0),
            '╮' => (-1.0, 1.0),
            '╯' => (-1.0, -1.0),
            '╰' => (1.0, -1.0),
            _ => return false,
        };
        let h = self.t * 0.5;
        let r = ((self.x1 - self.x0).min(self.y1 - self.y0) * 0.5).max(h * 2.0);
        let (cx, cy) = (self.cx, self.cy);
        if sy > 0.0 {
            self.vstroke(t, cx, self.t, cy + r - 1.0, self.y1);
        } else {
            self.vstroke(t, cx, self.t, self.y0, cy - r + 1.0);
        }
        if sx > 0.0 {
            self.hstroke(t, cy, self.t, cx + r - 1.0, self.x1);
        } else {
            self.hstroke(t, cy, self.t, self.x0, cx - r + 1.0);
        }
        let ax = cx + sx * r;
        let ay = cy + sy * r;
        t.stroke_arc_quarter(ax, ay, r, self.t, sx, sy);
        true
    }

    fn dashed(&self, t: &mut Target<'_>, c: char) -> bool {
        let (n, heavy, vertical) = match c {
            '╌' => (2, false, false),
            '╍' => (2, true, false),
            '╎' => (2, false, true),
            '╏' => (2, true, true),
            '┄' => (3, false, false),
            '┅' => (3, true, false),
            '┆' => (3, false, true),
            '┇' => (3, true, true),
            '┈' => (4, false, false),
            '┉' => (4, true, false),
            '┊' => (4, false, true),
            '┋' => (4, true, true),
            _ => return false,
        };
        let w = if heavy { self.t * 2.0 } else { self.t };
        let (a0, a1) = if vertical {
            (self.y0, self.y1)
        } else {
            (self.x0, self.x1)
        };
        let seg = (a1 - a0) / n as f32;
        for i in 0..n {
            let s = a0 + seg * (i as f32 + 0.15);
            let len = seg * 0.7;
            if vertical {
                self.vstroke(t, self.cx, w, s, s + len);
            } else {
                self.hstroke(t, self.cy, w, s, s + len);
            }
        }
        true
    }

    fn diagonal(&self, t: &mut Target<'_>, c: char) -> bool {
        match c {
            '╱' => t.stroke_diag(self.x1, self.y0, self.x0, self.y1, self.t),
            '╲' => t.stroke_diag(self.x0, self.y0, self.x1, self.y1, self.t),
            '╳' => {
                t.stroke_diag(self.x1, self.y0, self.x0, self.y1, self.t);
                t.stroke_diag(self.x0, self.y0, self.x1, self.y1, self.t);
            }
            _ => return false,
        }
        true
    }

    fn blocks(&self, t: &mut Target<'_>, c: char) -> bool {
        let (x0, x1, y0, y1, cx, cy) = (self.x0, self.x1, self.y0, self.y1, self.cx, self.cy);
        let (w, hgt) = (x1 - x0, y1 - y0);
        match c {
            '▀' => self.rect(t, x0, y0, w, hgt * 0.5),
            '▁'..='█' => {
                let k = (c as u32 - 0x2580) as f32;
                let hh = hgt * k / 8.0;
                self.rect(t, x0, y1 - hh, w, hh);
            }
            '▉'..='▏' => {
                let k = (0x2590 - c as u32) as f32;
                self.rect(t, x0, y0, w * k / 8.0, hgt);
            }
            '▐' => self.rect(t, cx, y0, x1 - cx, hgt),
            '░' => self.shade(t, 0.25),
            '▒' => self.shade(t, 0.5),
            '▓' => self.shade(t, 0.75),
            '▔' => self.rect(t, x0, y0, w, hgt / 8.0),
            '▕' => self.rect(t, x1 - w / 8.0, y0, w / 8.0, hgt),
            '▖' => self.rect(t, x0, cy, cx - x0, y1 - cy),
            '▗' => self.rect(t, cx, cy, x1 - cx, y1 - cy),
            '▘' => self.rect(t, x0, y0, cx - x0, cy - y0),
            '▙' => {
                self.rect(t, x0, y0, cx - x0, cy - y0);
                self.rect(t, x0, cy, cx - x0, y1 - cy);
                self.rect(t, cx, cy, x1 - cx, y1 - cy);
            }
            '▚' => {
                self.rect(t, x0, y0, cx - x0, cy - y0);
                self.rect(t, cx, cy, x1 - cx, y1 - cy);
            }
            '▛' => {
                self.rect(t, x0, y0, cx - x0, cy - y0);
                self.rect(t, cx, y0, x1 - cx, cy - y0);
                self.rect(t, x0, cy, cx - x0, y1 - cy);
            }
            '▜' => {
                self.rect(t, x0, y0, cx - x0, cy - y0);
                self.rect(t, cx, y0, x1 - cx, cy - y0);
                self.rect(t, cx, cy, x1 - cx, y1 - cy);
            }
            '▝' => self.rect(t, cx, y0, x1 - cx, cy - y0),
            '▞' => {
                self.rect(t, cx, y0, x1 - cx, cy - y0);
                self.rect(t, x0, cy, cx - x0, y1 - cy);
            }
            '▟' => {
                self.rect(t, cx, y0, x1 - cx, cy - y0);
                self.rect(t, x0, cy, cx - x0, y1 - cy);
                self.rect(t, cx, cy, x1 - cx, y1 - cy);
            }
            _ => return false,
        }
        true
    }
}

impl Target<'_> {
    fn put(&mut self, px: i32, py: i32, cover: f32) {
        if cover <= 0.0
            || px < self.clip_x0
            || px >= self.clip_x1
            || py < self.clip_y0
            || py >= self.clip_y1
            || px < 0
            || py < 0
            || px >= self.img_w as i32
            || py >= self.img_h as i32
        {
            return;
        }
        let i = ((py as u32 * self.img_w + px as u32) * 4) as usize;
        let a = cover.clamp(0.0, 1.0);
        let (fr, fg, fb) = self.fg;
        self.pixels[i] = lerp_u8(self.pixels[i], fr, a);
        self.pixels[i + 1] = lerp_u8(self.pixels[i + 1], fg, a);
        self.pixels[i + 2] = lerp_u8(self.pixels[i + 2], fb, a);
        self.pixels[i + 3] = 255;
    }

    fn fill_rect(&mut self, x: f32, y: f32, w: f32, h: f32, alpha: f32) {
        if w <= 0.0 || h <= 0.0 || alpha <= 0.0 {
            return;
        }
        let x0 = x.round() as i32;
        let y0 = y.round() as i32;
        let x1 = (x + w).round() as i32;
        let y1 = (y + h).round() as i32;
        for py in y0..y1 {
            for px in x0..x1 {
                self.put(px, py, alpha);
            }
        }
    }

    fn stroke_diag(&mut self, x0: f32, y0: f32, x1: f32, y1: f32, thickness: f32) {
        let dx = x1 - x0;
        let dy = y1 - y0;
        let len = (dx * dx + dy * dy).sqrt().max(1.0);
        let half = thickness * 0.5;
        let xmin = x0.min(x1).floor() as i32 - thickness.ceil() as i32;
        let xmax = x0.max(x1).ceil() as i32 + thickness.ceil() as i32;
        let ymin = y0.min(y1).floor() as i32 - thickness.ceil() as i32;
        let ymax = y0.max(y1).ceil() as i32 + thickness.ceil() as i32;
        for py in ymin..ymax {
            for px in xmin..xmax {
                let (x, y) = (px as f32 + 0.5, py as f32 + 0.5);
                let dist = (dy * (x - x0) - dx * (y - y0)).abs() / len;
                let cover = if self.aa {
                    (half + 0.5 - dist).clamp(0.0, 1.0)
                } else if dist <= half {
                    1.0
                } else {
                    0.0
                };
                self.put(px, py, cover);
            }
        }
    }

    fn stroke_arc_quarter(&mut self, ax: f32, ay: f32, r: f32, thickness: f32, sx: f32, sy: f32) {
        let half = thickness * 0.5;
        let pad = thickness.ceil() as i32 + 1;
        let xmin = (ax - r).min(ax).floor() as i32 - pad;
        let xmax = (ax + r).max(ax).ceil() as i32 + pad;
        let ymin = (ay - r).min(ay).floor() as i32 - pad;
        let ymax = (ay + r).max(ay).ceil() as i32 + pad;
        for py in ymin..ymax {
            for px in xmin..xmax {
                let (x, y) = (px as f32 + 0.5, py as f32 + 0.5);
                if (x - ax) * sx > 0.0 || (y - ay) * sy > 0.0 {
                    continue;
                }
                let dist = ((x - ax) * (x - ax) + (y - ay) * (y - ay)).sqrt();
                let d = (dist - r).abs();
                let cover = if self.aa {
                    (half + 0.5 - d).clamp(0.0, 1.0)
                } else if d <= half {
                    1.0
                } else {
                    0.0
                };
                self.put(px, py, cover);
            }
        }
    }
}

fn lerp_u8(bg: u8, fg: u8, t: f32) -> u8 {
    (bg as f32 * (1.0 - t) + fg as f32 * t)
        .round()
        .clamp(0.0, 255.0) as u8
}

fn arms_of(c: char) -> Option<(Arm, Arm, Arm, Arm)> {
    use Arm::{Heavy as H, Light as L, None as N};
    Some(match c {
        '─' => (N, N, L, L),
        '━' => (N, N, H, H),
        '│' => (L, L, N, N),
        '┃' => (H, H, N, N),
        '┌' => (N, L, N, L),
        '┍' => (N, L, N, H),
        '┎' => (N, H, N, L),
        '┏' => (N, H, N, H),
        '┐' => (N, L, L, N),
        '┑' => (N, L, H, N),
        '┒' => (N, H, L, N),
        '┓' => (N, H, H, N),
        '└' => (L, N, N, L),
        '┕' => (L, N, N, H),
        '┖' => (H, N, N, L),
        '┗' => (H, N, N, H),
        '┘' => (L, N, L, N),
        '┙' => (L, N, H, N),
        '┚' => (H, N, L, N),
        '┛' => (H, N, H, N),
        '├' => (L, L, N, L),
        '┝' => (L, L, N, H),
        '┞' => (H, L, N, L),
        '┟' => (L, H, N, L),
        '┠' => (H, H, N, L),
        '┡' => (H, L, N, H),
        '┢' => (L, H, N, H),
        '┣' => (H, H, N, H),
        '┤' => (L, L, L, N),
        '┥' => (L, L, H, N),
        '┦' => (H, L, L, N),
        '┧' => (L, H, L, N),
        '┨' => (H, H, L, N),
        '┩' => (H, L, H, N),
        '┪' => (L, H, H, N),
        '┫' => (H, H, H, N),
        '┬' => (N, L, L, L),
        '┭' => (N, L, H, L),
        '┮' => (N, L, L, H),
        '┯' => (N, L, H, H),
        '┰' => (N, H, L, L),
        '┱' => (N, H, H, L),
        '┲' => (N, H, L, H),
        '┳' => (N, H, H, H),
        '┴' => (L, N, L, L),
        '┵' => (L, N, H, L),
        '┶' => (L, N, L, H),
        '┷' => (L, N, H, H),
        '┸' => (H, N, L, L),
        '┹' => (H, N, H, L),
        '┺' => (H, N, L, H),
        '┻' => (H, N, H, H),
        '┼' => (L, L, L, L),
        '┽' => (L, L, H, L),
        '┾' => (L, L, L, H),
        '┿' => (L, L, H, H),
        '╀' => (H, L, L, L),
        '╁' => (L, H, L, L),
        '╂' => (H, H, L, L),
        '╃' => (H, L, H, L),
        '╄' => (H, L, L, H),
        '╅' => (L, H, H, L),
        '╆' => (L, H, L, H),
        '╇' => (H, L, H, H),
        '╈' => (L, H, H, H),
        '╉' => (H, H, H, L),
        '╊' => (H, H, L, H),
        '╋' => (H, H, H, H),
        '╴' => (N, N, L, N),
        '╵' => (L, N, N, N),
        '╶' => (N, N, N, L),
        '╷' => (N, L, N, N),
        '╸' => (N, N, H, N),
        '╹' => (H, N, N, N),
        '╺' => (N, N, N, H),
        '╻' => (N, H, N, N),
        '╼' => (N, N, L, H),
        '╽' => (L, H, N, N),
        '╾' => (N, N, H, L),
        '╿' => (H, L, N, N),
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn paint_cell(c: char) -> (Vec<u8>, u32, u32) {
        let (w, h) = (9u32, 18u32);
        let mut pixels = vec![0u8; (w * h * 4) as usize];
        assert!(
            paint(
                &mut pixels,
                w,
                h,
                0.0,
                0.0,
                w as f32,
                h as f32,
                c,
                (255, 255, 255),
                true
            ),
            "U+{:04X} {c} not handled",
            c as u32
        );
        (pixels, w, h)
    }

    fn ink_bounds(pixels: &[u8], w: u32, h: u32) -> Option<(u32, u32, u32, u32)> {
        let mut minx = w;
        let mut maxx = 0u32;
        let mut miny = h;
        let mut maxy = 0u32;
        let mut any = false;
        for y in 0..h {
            for x in 0..w {
                let i = ((y * w + x) * 4) as usize;
                if pixels[i] > 8 || pixels[i + 1] > 8 || pixels[i + 2] > 8 {
                    any = true;
                    minx = minx.min(x);
                    maxx = maxx.max(x);
                    miny = miny.min(y);
                    maxy = maxy.max(y);
                }
            }
        }
        any.then_some((minx, maxx, miny, maxy))
    }

    #[test]
    fn whole_range_is_handled() {
        for cp in 0x2500u32..=0x259f {
            let c = char::from_u32(cp).unwrap();
            let _ = paint_cell(c);
        }
    }

    #[test]
    fn full_block_covers_the_cell() {
        let (pixels, w, h) = paint_cell('█');
        let (minx, maxx, miny, maxy) = ink_bounds(&pixels, w, h).unwrap();
        assert_eq!((minx, maxx, miny, maxy), (0, w - 1, 0, h - 1));
    }

    #[test]
    fn light_vertical_reaches_top_and_bottom() {
        let (pixels, w, h) = paint_cell('│');
        let (_, _, miny, maxy) = ink_bounds(&pixels, w, h).unwrap();
        assert_eq!(miny, 0, "│ misses the top edge");
        assert_eq!(maxy, h - 1, "│ misses the bottom edge");
    }

    #[test]
    fn light_horizontal_reaches_left_and_right() {
        let (pixels, w, h) = paint_cell('─');
        let (minx, maxx, _, _) = ink_bounds(&pixels, w, h).unwrap();
        assert_eq!(minx, 0, "─ misses the left edge");
        assert_eq!(maxx, w - 1, "─ misses the right edge");
    }

    #[test]
    fn shade_is_not_opaque() {
        let (pixels, w, h) = paint_cell('░');
        let i = ((h / 2 * w + w / 2) * 4) as usize;
        assert!(pixels[i] > 0 && pixels[i] < 255, "░ should wash, not fill");
    }

    #[test]
    fn ascii_is_not_box_draw() {
        assert!(!is_box_draw('A'));
        assert!(!paint(
            &mut [],
            0,
            0,
            0.0,
            0.0,
            1.0,
            1.0,
            'A',
            (0, 0, 0),
            false
        ));
    }
}
