//! Mux `LocalPane` (same host as wezterm-gui).
//!
//! Spawn goes through `mux::domain::LocalDomain::spawn_pane` → ConPTY +
//! `wezterm-term` inside `LocalPane`. Paint prefers wezterm-font sprites
//! (cached `paint_image` + cell quads). Consolas GPUI text is the fallback
//! if FreeType/shaper init fails. Not the wezterm-gui glyph atlas.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use gpui::*;
use gpui_component::StyledExt;
use mux::pane::{Pane, PaneId};
use mux::renderable::RenderableDimensions;
use mux::{Mux, MuxNotification};
use wezterm_term::color::ColorPalette;
use wezterm_term::input::{
    MouseButton as TermMouseButton, MouseEvent as TermMouseEvent, MouseEventKind,
};
use wezterm_term::{Alert, KeyCode, KeyModifiers, Line, StableRowIndex, TerminalSize};

use crate::glyph_paint::{GlyphPainter, TermPaint};
use crate::shells::ShellProfile;

const DEFAULT_ROWS: usize = 24;
const DEFAULT_COLS: usize = 80;

/// Shell process ended (`exit`, CloseOnCleanExit, mux `PaneRemoved`).
pub enum TermPaneEvent {
    Exited,
}

struct LiveMux {
    pane: Arc<dyn Pane>,
    alive: Arc<AtomicBool>,
}

pub struct TermPane {
    live: Result<LiveMux, String>,
    font_px: f32,
    painter: Option<GlyphPainter>,
    /// First valid layout calls `Pane::resize` (PTY + terminal). Drag still
    /// uses `resize_display` only. After the view grid is stable ~450ms we
    /// commit one ConPTY `resize` (decision 016). Live drag still smears.
    pty_synced: bool,
    pty_cols: usize,
    pty_rows: usize,
    /// GUI viewport into scrollback. `None` follows `physical_top` (live bottom).
    /// Same model as wezterm-gui `pane_state.viewport`, not alacritty `display_offset`.
    viewport: Option<StableRowIndex>,
    /// Bumped on every view-grid change so delayed ConPTY commits can no-op.
    pty_commit_gen: u64,
    /// Skip HarfBuzz + row composite when the pane has not changed.
    paint_cache: Option<PaintCache>,
    /// Last `window.scale_factor()` from prepaint. Line-sprite dest must use
    /// this exact value (not `round(96*scale)/96`) or 4K rows paint as slivers.
    paint_scale: f32,
    /// GUI-side selection (same model as wezterm-gui `TermWindow.selection`).
    /// Not stored in the mux pane.
    selection: Selection,
    /// AppShell's focus handle. TermScreen swallows left-click, so we
    /// focus the shell here or typing stays dead until a right-click bubbles.
    shell_focus: FocusHandle,
    /// Tab title when the PTY has not set one yet (profile label).
    fallback_title: String,
}

struct PaintCache {
    seq: usize,
    top: StableRowIndex,
    cursor: Option<(usize, usize)>,
    cols: usize,
    rows: usize,
    dpi: u32,
    font_px: u32,
    scale: u32,
    sel: Option<(i64, u32, i64, u32)>,
    paint: TermPaint,
}

impl TermPane {
    pub fn spawn(
        font_px: f32,
        shell_focus: FocusHandle,
        profile: &ShellProfile,
        cx: &mut Context<Self>,
    ) -> Self {
        let _ = crate::mux_host::ensure_init();
        let fallback_title = profile.label.clone();
        let painter = match GlyphPainter::new(96) {
            Ok(p) => Some(p),
            Err(err) => {
                eprintln!("wezterm-gpui wezterm-font init: {err:#}");
                None
            }
        };
        match spawn_live(profile.command(), cx) {
            Ok(live) => Self {
                live: Ok(live),
                font_px,
                painter,
                pty_synced: false,
                pty_cols: DEFAULT_COLS,
                pty_rows: DEFAULT_ROWS,
                viewport: None,
                pty_commit_gen: 0,
                paint_cache: None,
                paint_scale: 1.0,
                selection: Selection::default(),
                shell_focus,
                fallback_title,
            },
            Err(err) => Self {
                live: Err(format!("{err:#}")),
                font_px,
                painter,
                pty_synced: false,
                pty_cols: DEFAULT_COLS,
                pty_rows: DEFAULT_ROWS,
                viewport: None,
                pty_commit_gen: 0,
                paint_cache: None,
                paint_scale: 1.0,
                selection: Selection::default(),
                shell_focus,
                fallback_title,
            },
        }
    }

    pub fn set_font_px(&mut self, font_px: f32) {
        self.font_px = font_px;
        self.paint_cache = None;
        if let Some(painter) = &mut self.painter {
            painter.sync_font(font_px, painter.dpi());
        }
    }

    pub fn paint_mode(&self) -> &'static str {
        if self.painter.is_some() {
            "wezterm-font glyphs (line sprites)"
        } else {
            "Consolas GPUI text (glyph paint unavailable)"
        }
    }

    pub fn status_line(&self) -> String {
        let mode = self.paint_mode();
        let Ok(live) = &self.live else {
            return mode.to_string();
        };
        let dims = live.pane.get_dimensions();
        let top = self.paint_top(&dims);
        let scroll = if self.viewport.is_some() && top < dims.physical_top {
            format!("  scroll {}", dims.physical_top - top)
        } else {
            String::new()
        };
        format!(
            "{mode}  pty {}×{}  view {}×{}  {}dpi{scroll}",
            self.pty_cols, self.pty_rows, dims.cols, dims.viewport_rows, dims.dpi
        )
    }

    pub fn title(&self) -> String {
        match &self.live {
            Ok(live) => {
                let title = live.pane.get_title();
                if title.is_empty() || title.eq_ignore_ascii_case("wezterm") {
                    self.fallback_title.clone()
                } else {
                    title
                }
            }
            Err(_) => "error".into(),
        }
    }

    pub fn key_down(&mut self, event: &KeyDownEvent, _cx: &mut App) -> bool {
        let Ok(live) = self.live.as_mut() else {
            return false;
        };
        // Default exit_behavior is Close. After `exit`, ConPTY write_all can
        // block the GPUI thread so chrome still clicks but typing is dead (032).
        if live.pane.is_dead() {
            return false;
        }
        let Some((key, mods)) = map_keystroke(&event.keystroke) else {
            return false;
        };
        if self.selection.clear() {
            self.paint_cache = None;
        }
        self.viewport = None;
        live.pane.key_down(key, mods).ok();
        true
    }

    pub fn clear_scrollback(&mut self) {
        self.viewport = None;
        if let Ok(live) = self.live.as_mut() {
            live.pane
                .erase_scrollback(config::keyassignment::ScrollbackEraseMode::ScrollbackOnly);
        }
    }

    pub fn on_scroll_wheel(&mut self, event: &ScrollWheelEvent) {
        let Ok(live) = &self.live else {
            return;
        };
        let (_, cell_h) = self.cell_px();
        let y = wheel_y_lines(event, cell_h);
        if y == 0 {
            return;
        }
        if live.pane.is_mouse_grabbed() || live.pane.is_alt_screen_active() {
            let button = if y > 0 {
                TermMouseButton::WheelUp(y as usize)
            } else {
                TermMouseButton::WheelDown((-y) as usize)
            };
            let _ = live.pane.mouse_event(TermMouseEvent {
                kind: MouseEventKind::Press,
                x: 0,
                y: 0,
                x_pixel_offset: 0,
                y_pixel_offset: 0,
                button,
                modifiers: gpui_mods(&event.modifiers),
            });
            return;
        }
        // GPUI Windows: positive y is wheel away from user (see older history).
        self.scroll_by_line(-y);
    }

    pub fn copy_selection(&self, cx: &mut App) -> bool {
        let text = self.selection_text();
        if text.is_empty() {
            return false;
        }
        cx.write_to_clipboard(ClipboardItem::new_string(text));
        true
    }

    pub fn paste_clipboard(&mut self, cx: &mut App) -> bool {
        let Some(text) = cx.read_from_clipboard().and_then(|item| item.text()) else {
            return false;
        };
        if text.is_empty() {
            return false;
        }
        let Ok(live) = &self.live else {
            return false;
        };
        if self.selection.clear() {
            self.paint_cache = None;
        }
        self.viewport = None;
        live.pane.send_paste(&text).ok();
        true
    }

    fn selection_text(&self) -> String {
        let Ok(live) = &self.live else {
            return String::new();
        };
        let Some(sel) = self.selection.range.map(SelRange::normalize) else {
            return String::new();
        };
        let mut s = String::new();
        let mut last_was_wrapped = false;
        let first_row = sel.start.y;
        let last_row = sel.end.y + 1;
        for line in live.pane.get_logical_lines(first_row..last_row) {
            if !s.is_empty() && !last_was_wrapped {
                s.push('\n');
            }
            let last_idx = line.physical_lines.len().saturating_sub(1);
            for (idx, phys) in line.physical_lines.iter().enumerate() {
                let this_row = line.first_row + idx as StableRowIndex;
                if this_row < first_row || this_row >= last_row {
                    continue;
                }
                let last_phys_idx = phys.len().saturating_sub(1);
                let cols = sel.cols_for_row(this_row);
                let last_col_idx = cols.end.saturating_sub(1).min(last_phys_idx);
                let col_span = phys.columns_as_str(cols);
                if idx == last_idx {
                    s.push_str(col_span.trim_end());
                } else {
                    s.push_str(&col_span);
                }
                last_was_wrapped = last_col_idx == last_phys_idx
                    && phys
                        .get_cell(last_col_idx)
                        .map(|c| c.attrs().wrapped())
                        .unwrap_or(false);
            }
        }
        s
    }

    fn on_mouse_down(
        &mut self,
        pos: Point<Pixels>,
        bounds: Bounds<Pixels>,
        click_count: usize,
        modifiers: &Modifiers,
    ) {
        let Ok(live) = &self.live else {
            return;
        };
        let Some(hit) = self.hit_cell(pos, bounds, false) else {
            return;
        };
        if live.pane.is_mouse_grabbed() && !modifiers.shift {
            self.send_pty_mouse(MouseEventKind::Press, TermMouseButton::Left, hit, modifiers);
            return;
        }
        self.selection.dragging = true;
        self.selection.origin = Some(hit.pos);
        self.selection.mode = match click_count {
            2 => SelMode::Word,
            n if n >= 3 => SelMode::Line,
            _ => SelMode::Cell,
        };
        self.selection.range = match self.selection.mode {
            SelMode::Cell => None,
            SelMode::Word => Some(word_around(&*live.pane, hit.pos)),
            SelMode::Line => Some(line_around(&*live.pane, hit.pos)),
        };
        self.paint_cache = None;
    }

    fn on_mouse_drag(
        &mut self,
        pos: Point<Pixels>,
        bounds: Bounds<Pixels>,
        modifiers: &Modifiers,
    ) -> bool {
        let Ok(live) = &self.live else {
            return false;
        };
        let Some(hit) = self.hit_cell(pos, bounds, true) else {
            return false;
        };
        if live.pane.is_mouse_grabbed() && !modifiers.shift && !self.selection.dragging {
            self.send_pty_mouse(MouseEventKind::Move, TermMouseButton::Left, hit, modifiers);
            return false;
        }
        if !self.selection.dragging {
            return false;
        }
        let origin = self.selection.origin.unwrap_or(hit.pos);
        // wezterm-gui: Cell selection exists only after the hit leaves origin.
        // Same-cell (click, or a MouseMove on the press) must not paint a 1-col box.
        let next = match self.selection.mode {
            SelMode::Cell => {
                if origin == hit.pos {
                    None
                } else {
                    Some(SelRange {
                        start: origin,
                        end: hit.pos,
                    })
                }
            }
            SelMode::Word => Some(union_range(
                word_around(&*live.pane, origin),
                word_around(&*live.pane, hit.pos),
            )),
            SelMode::Line => Some(union_range(
                line_around(&*live.pane, origin),
                line_around(&*live.pane, hit.pos),
            )),
        };
        if self.selection.range != next {
            self.selection.range = next;
            self.paint_cache = None;
            true
        } else {
            false
        }
    }

    fn on_mouse_up(&mut self, pos: Point<Pixels>, bounds: Bounds<Pixels>, modifiers: &Modifiers) {
        let was_dragging = self.selection.dragging;
        self.selection.dragging = false;
        let Ok(live) = &self.live else {
            return;
        };
        if live.pane.is_mouse_grabbed() && !modifiers.shift && !was_dragging {
            if let Some(hit) = self.hit_cell(pos, bounds, true) {
                self.send_pty_mouse(
                    MouseEventKind::Release,
                    TermMouseButton::Left,
                    hit,
                    modifiers,
                );
            }
        }
    }

    fn send_pty_mouse(
        &self,
        kind: MouseEventKind,
        button: TermMouseButton,
        hit: CellHit,
        modifiers: &Modifiers,
    ) {
        let Ok(live) = &self.live else {
            return;
        };
        let _ = live.pane.mouse_event(TermMouseEvent {
            kind,
            x: hit.pos.x,
            y: hit.vis_row as i64,
            x_pixel_offset: hit.x_pixel_offset,
            y_pixel_offset: hit.y_pixel_offset,
            button,
            modifiers: gpui_mods(modifiers),
        });
    }

    fn hit_cell(&self, pos: Point<Pixels>, bounds: Bounds<Pixels>, clamp: bool) -> Option<CellHit> {
        let Ok(live) = &self.live else {
            return None;
        };
        let (cw, ch) = self.cell_px();
        if cw < 1. || ch < 1. {
            return None;
        }
        let dims = live.pane.get_dimensions();
        if dims.cols < 1 || dims.viewport_rows < 1 {
            return None;
        }
        let mut x = f32::from(pos.x - bounds.origin.x);
        let mut y = f32::from(pos.y - bounds.origin.y);
        if !clamp
            && (x < 0.
                || y < 0.
                || x >= f32::from(bounds.size.width)
                || y >= f32::from(bounds.size.height))
        {
            return None;
        }
        x = x.max(0.0);
        y = y.max(0.0);
        let col = (x / cw).floor() as usize;
        let vis_row = (y / ch).floor() as usize;
        let col = col.min(dims.cols.saturating_sub(1));
        let vis_row = vis_row.min(dims.viewport_rows.saturating_sub(1));
        let stable = self.paint_top(&dims).saturating_add(vis_row as isize);
        Some(CellHit {
            pos: CellPos { y: stable, x: col },
            vis_row,
            x_pixel_offset: (x - col as f32 * cw) as isize,
            y_pixel_offset: (y - vis_row as f32 * ch) as isize,
        })
    }

    fn cell_px(&self) -> (f32, f32) {
        if let Some(painter) = &self.painter {
            if let Ok(size) = painter.cell_size() {
                return (size.width, size.height);
            }
        }
        (self.font_px * 0.62, self.font_px * 1.28)
    }

    pub fn apply_layout(
        &mut self,
        width: Pixels,
        height: Pixels,
        scale: f32,
        cx: &mut Context<Self>,
    ) {
        let dpi = dpi_from_scale(scale);
        if (self.paint_scale - scale).abs() > 0.001 {
            self.paint_cache = None;
        }
        self.paint_scale = scale.max(0.5);
        if let Some(painter) = &mut self.painter {
            painter.sync_font(self.font_px, dpi);
        }
        let Some(size) = self.size_from_pixels(width, height, dpi) else {
            return;
        };
        if !self.grid_differs(&size) {
            return;
        }
        self.paint_cache = None;
        if self.pty_synced {
            if let Ok(live) = &self.live {
                let _ = live.pane.resize_display(size);
            }
            self.clamp_viewport();
            self.schedule_pty_commit(size, cx);
            return;
        }
        if let Ok(live) = &mut self.live {
            let _ = live.pane.resize(size);
            self.pty_synced = true;
            self.pty_cols = size.cols;
            self.pty_rows = size.rows;
        }
        self.clamp_viewport();
    }

    fn schedule_pty_commit(&mut self, size: TerminalSize, cx: &mut Context<Self>) {
        if size.cols == self.pty_cols && size.rows == self.pty_rows {
            return;
        }
        self.pty_commit_gen = self.pty_commit_gen.wrapping_add(1);
        let generation = self.pty_commit_gen;
        cx.spawn(async move |this, cx| {
            cx.background_executor()
                .timer(std::time::Duration::from_millis(450))
                .await;
            this.update(cx, |term, cx| {
                term.commit_pty_if_stable(generation);
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    fn commit_pty_if_stable(&mut self, generation: u64) {
        if generation != self.pty_commit_gen {
            return;
        }
        let Ok(live) = &self.live else {
            return;
        };
        let dims = live.pane.get_dimensions();
        if dims.cols < 2 || dims.viewport_rows < 2 {
            return;
        }
        if dims.cols == self.pty_cols && dims.viewport_rows == self.pty_rows {
            return;
        }
        let (cell_w, cell_h) = self.cell_px();
        let dpr = size_dpr(dims.dpi);
        let size = TerminalSize {
            rows: dims.viewport_rows,
            cols: dims.cols,
            pixel_width: (dims.cols as f32 * cell_w * dpr).round() as usize,
            pixel_height: (dims.viewport_rows as f32 * cell_h * dpr).round() as usize,
            dpi: dims.dpi,
        };
        let _ = live.pane.resize(size);
        self.pty_cols = size.cols;
        self.pty_rows = size.rows;
    }

    fn scroll_by_line(&mut self, amount: isize) {
        self.paint_cache = None;
        let Ok(live) = &self.live else {
            return;
        };
        let dims = live.pane.get_dimensions();
        let position = self
            .viewport
            .unwrap_or(dims.physical_top)
            .saturating_add(amount);
        self.set_viewport(Some(position), &dims);
    }

    fn set_viewport(&mut self, position: Option<StableRowIndex>, dims: &RenderableDimensions) {
        self.viewport = match position {
            Some(pos) if pos >= dims.physical_top => None,
            Some(pos) => Some(pos.max(dims.scrollback_top)),
            None => None,
        };
    }

    fn clamp_viewport(&mut self) {
        let Ok(live) = &self.live else {
            return;
        };
        let dims = live.pane.get_dimensions();
        let pos = self.viewport;
        self.set_viewport(pos, &dims);
    }

    fn paint_top(&self, dims: &RenderableDimensions) -> StableRowIndex {
        match self.viewport {
            Some(pos) if pos < dims.physical_top => pos.max(dims.scrollback_top),
            _ => dims.physical_top,
        }
    }

    fn grid_differs(&self, size: &TerminalSize) -> bool {
        let Ok(live) = &self.live else {
            return false;
        };
        let dims = live.pane.get_dimensions();
        dims.cols != size.cols || dims.viewport_rows != size.rows || dims.dpi != size.dpi
    }

    fn size_from_pixels(&self, width: Pixels, height: Pixels, dpi: u32) -> Option<TerminalSize> {
        let (cell_w, cell_h) = self.cell_px();
        if cell_w < 1. || cell_h < 1. {
            return None;
        }
        let width = f32::from(width);
        let height = f32::from(height);
        // Skip 0×0 / sub-cell GPUI bounds (live drag). Do not clamp them up.
        if width < cell_w * 2. || height < cell_h * 2. {
            return None;
        }
        let cols = (width / cell_w).floor() as usize;
        let rows = (height / cell_h).floor() as usize;
        if cols < 2 || rows < 2 {
            return None;
        }
        let cols = cols.min(400);
        let rows = rows.min(200);
        let dpr = size_dpr(dpi);
        Some(TerminalSize {
            rows,
            cols,
            pixel_width: (cols as f32 * cell_w * dpr).round() as usize,
            pixel_height: (rows as f32 * cell_h * dpr).round() as usize,
            dpi,
        })
    }

    fn visible_lines(&self) -> (Vec<Line>, Option<(usize, usize)>, ColorPalette) {
        match &self.live {
            Ok(live) => {
                let dims = live.pane.get_dimensions();
                let cursor = live.pane.get_cursor_position();
                let pal = live.pane.palette();
                let top = self.paint_top(&dims);
                let end = top.saturating_add(dims.viewport_rows as isize);
                let (first, lines) = live.pane.get_lines(top..end);
                let vis_row = usize::try_from(cursor.y.saturating_sub(first))
                    .ok()
                    .filter(|row| *row < lines.len());
                (lines, vis_row.map(|row| (row, cursor.x)), pal)
            }
            Err(_) => (vec![], None, ColorPalette::default()),
        }
    }

    fn visible_text(&self) -> (Vec<String>, Option<(usize, usize)>, u32, u32) {
        match &self.live {
            Ok(_) => {
                let (lines, cursor, pal) = self.visible_lines();
                let text: Vec<String> = lines
                    .iter()
                    .map(|line| line.as_str().into_owned())
                    .collect();
                let (fr, fg, fb, _) = pal.foreground.as_rgba_u8();
                let (br, bg, bb, _) = pal.background.as_rgba_u8();
                let fg = u32::from_be_bytes([0, fr, fg, fb]);
                let bg = u32::from_be_bytes([0, br, bg, bb]);
                (text, cursor, fg, bg)
            }
            Err(err) => (vec![err.clone()], None, 0xc8c8c8, 0x0c0c0c),
        }
    }

    pub(crate) fn try_glyph_paint(&mut self) -> Option<TermPaint> {
        if self.painter.is_none() || self.live.is_err() {
            return None;
        }
        let Ok(live) = &self.live else {
            return None;
        };
        let dims = live.pane.get_dimensions();
        let seq = live.pane.get_current_seqno();
        let cursor_pos = live.pane.get_cursor_position();
        let top = self.paint_top(&dims);
        let vis_row = usize::try_from(cursor_pos.y.saturating_sub(top))
            .ok()
            .filter(|row| *row < dims.viewport_rows);
        let cursor = vis_row.map(|row| (row, cursor_pos.x));
        let font_px = self.font_px.to_bits();
        let sel = self.selection.fingerprint();
        let scale_bits = self.paint_scale.to_bits();
        if let Some(cache) = &self.paint_cache {
            if cache.seq == seq
                && cache.top == top
                && cache.cursor == cursor
                && cache.cols == dims.cols
                && cache.rows == dims.viewport_rows
                && cache.dpi == dims.dpi
                && cache.font_px == font_px
                && cache.scale == scale_bits
                && cache.sel == sel
            {
                let mut paint = cache.paint.clone();
                paint.drop_images = Vec::new();
                return Some(paint);
            }
        }
        let pal = live.pane.palette();
        let end = top.saturating_add(dims.viewport_rows as isize);
        let (_, lines) = live.pane.get_lines(top..end);
        let sel_cols: Vec<(u16, u16)> = (0..lines.len())
            .map(|row| {
                self.selection
                    .range
                    .map(|r| {
                        r.normalize()
                            .sel_cols(top.saturating_add(row as isize), dims.cols)
                    })
                    .unwrap_or((u16::MAX, u16::MAX))
            })
            .collect();
        let painter = self.painter.as_mut()?;
        painter.sync_font(self.font_px, dpi_from_scale(self.paint_scale));
        match painter.layout(
            &lines,
            cursor,
            &pal,
            dims.cols,
            &sel_cols,
            self.paint_scale,
        ) {
            Ok(paint) => {
                self.paint_cache = Some(PaintCache {
                    seq,
                    top,
                    cursor,
                    cols: dims.cols,
                    rows: dims.viewport_rows,
                    dpi: dims.dpi,
                    font_px,
                    scale: scale_bits,
                    sel,
                    paint: TermPaint {
                        bg: paint.bg,
                        sprites: paint.sprites.clone(),
                        drop_images: Vec::new(),
                    },
                });
                Some(paint)
            }
            Err(err) => {
                eprintln!("wezterm-gpui glyph paint: {err:#}");
                self.painter = None;
                self.paint_cache = None;
                None
            }
        }
    }
}

impl EventEmitter<TermPaneEvent> for TermPane {}

impl Drop for TermPane {
    fn drop(&mut self) {
        if let Ok(live) = &self.live {
            live.alive.store(false, Ordering::Relaxed);
            live.pane.kill();
            if let Some(mux) = Mux::try_get() {
                mux.remove_pane(live.pane.pane_id());
            }
        }
    }
}

#[derive(Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord)]
struct CellPos {
    y: StableRowIndex,
    x: usize,
}

#[derive(Clone, Copy, PartialEq, Eq)]
struct SelRange {
    start: CellPos,
    end: CellPos,
}

#[derive(Clone, Copy, Default, PartialEq, Eq)]
enum SelMode {
    #[default]
    Cell,
    Word,
    Line,
}

#[derive(Clone, Copy, Default)]
struct Selection {
    origin: Option<CellPos>,
    range: Option<SelRange>,
    dragging: bool,
    mode: SelMode,
}

struct CellHit {
    pos: CellPos,
    vis_row: usize,
    x_pixel_offset: isize,
    y_pixel_offset: isize,
}

impl Selection {
    fn clear(&mut self) -> bool {
        let changed = self.range.is_some() || self.origin.is_some() || self.dragging;
        self.range = None;
        self.origin = None;
        self.dragging = false;
        changed
    }

    fn fingerprint(&self) -> Option<(i64, u32, i64, u32)> {
        let r = self.range?.normalize();
        Some((
            r.start.y as i64,
            r.start.x.min(u32::MAX as usize) as u32,
            r.end.y as i64,
            r.end.x.min(u32::MAX as usize) as u32,
        ))
    }
}

impl SelRange {
    fn normalize(self) -> Self {
        if self.start <= self.end {
            self
        } else {
            Self {
                start: self.end,
                end: self.start,
            }
        }
    }

    fn cols_for_row(self, row: StableRowIndex) -> std::ops::Range<usize> {
        let n = self.normalize();
        if row < n.start.y || row > n.end.y {
            0..0
        } else if n.start.y == n.end.y {
            let (lo, hi) = if n.start.x <= n.end.x {
                (n.start.x, n.end.x)
            } else {
                (n.end.x, n.start.x)
            };
            lo..hi.saturating_add(1)
        } else if row == n.end.y {
            0..n.end.x.saturating_add(1)
        } else if row == n.start.y {
            n.start.x..usize::MAX
        } else {
            0..usize::MAX
        }
    }

    fn sel_cols(self, row: StableRowIndex, cols: usize) -> (u16, u16) {
        let range = self.cols_for_row(row);
        if range.is_empty() {
            return (u16::MAX, u16::MAX);
        }
        let start = range.start.min(cols) as u16;
        let end = range.end.min(cols) as u16;
        if start >= end {
            (u16::MAX, u16::MAX)
        } else {
            (start, end)
        }
    }
}

fn union_range(a: SelRange, b: SelRange) -> SelRange {
    let a = a.normalize();
    let b = b.normalize();
    SelRange {
        start: a.start.min(b.start),
        end: a.end.max(b.end),
    }
}

fn is_word_cell(line: &Line, col: usize, boundary: &str) -> bool {
    let Some(cell) = line.get_cell(col) else {
        return false;
    };
    let s = cell.str();
    match s.chars().count() {
        0 => false,
        1 => !boundary.contains(s),
        _ => true,
    }
}

fn word_around(pane: &dyn Pane, pos: CellPos) -> SelRange {
    for logical in pane.get_logical_lines(pos.y..pos.y + 1) {
        if !logical.contains_y(pos.y) {
            continue;
        }
        let click = logical.xy_to_logical_x(pos.x, pos.y);
        let boundary = &config::configuration().selection_word_boundary;
        if !is_word_cell(&logical.logical, click, boundary) {
            return SelRange {
                start: pos,
                end: pos,
            };
        }
        let mut start = click;
        let mut end = click;
        while start > 0 && is_word_cell(&logical.logical, start - 1, boundary) {
            start -= 1;
        }
        while is_word_cell(&logical.logical, end + 1, boundary) {
            end += 1;
        }
        let (sy, sx) = logical.logical_x_to_physical_coord(start);
        let (ey, ex) = logical.logical_x_to_physical_coord(end);
        return SelRange {
            start: CellPos { y: sy, x: sx },
            end: CellPos { y: ey, x: ex },
        };
    }
    SelRange {
        start: pos,
        end: pos,
    }
}

fn line_around(pane: &dyn Pane, pos: CellPos) -> SelRange {
    for logical in pane.get_logical_lines(pos.y..pos.y + 1) {
        if !logical.contains_y(pos.y) {
            continue;
        }
        let last = logical.physical_lines.len().saturating_sub(1);
        return SelRange {
            start: CellPos {
                y: logical.first_row,
                x: 0,
            },
            end: CellPos {
                y: logical.first_row + last as StableRowIndex,
                x: usize::MAX,
            },
        };
    }
    SelRange {
        start: pos,
        end: pos,
    }
}

fn spawn_live(
    cmd: portable_pty::CommandBuilder,
    cx: &mut Context<TermPane>,
) -> anyhow::Result<LiveMux> {
    let size = TerminalSize {
        rows: DEFAULT_ROWS,
        cols: DEFAULT_COLS,
        pixel_width: DEFAULT_COLS * 8,
        pixel_height: DEFAULT_ROWS * 16,
        dpi: 96,
    };
    let pane = crate::mux_host::spawn_command(size, cmd)?;
    let pane_id = pane.pane_id();
    let alive = Arc::new(AtomicBool::new(true));
    let (tx, rx) = async_channel::unbounded::<()>();
    {
        let alive = Arc::clone(&alive);
        Mux::get().subscribe(move |n| {
            if !alive.load(Ordering::Relaxed) {
                return false;
            }
            if let MuxNotification::Alert {
                pane_id: id,
                alert: Alert::Bell,
            } = &n
            {
                if *id == pane_id {
                    crate::mux_host::maybe_audible_bell();
                }
            }
            if notification_is_pane(&n, pane_id) {
                // try_send: mux::notify holds subscribers.write(); blocking
                // send from that lock can stall the mux executor (032).
                let _ = tx.try_send(());
            }
            true
        });
    }

    cx.spawn(async move |this, cx| {
        while let Ok(()) = rx.recv().await {
            while rx.try_recv().is_ok() {}
            if this
                .update(cx, |term, cx| {
                    if term.live.as_ref().is_ok_and(|live| live.pane.is_dead()) {
                        cx.emit(TermPaneEvent::Exited);
                    }
                    cx.notify();
                })
                .is_err()
            {
                break;
            }
        }
    })
    .detach();

    Ok(LiveMux { pane, alive })
}

fn notification_is_pane(n: &MuxNotification, pane_id: PaneId) -> bool {
    match n {
        MuxNotification::PaneOutput(id)
        | MuxNotification::PaneRemoved(id)
        | MuxNotification::PaneFocused(id)
        | MuxNotification::PaneAdded(id) => *id == pane_id,
        MuxNotification::Alert { pane_id: id, .. }
        | MuxNotification::AssignClipboard { pane_id: id, .. } => *id == pane_id,
        _ => false,
    }
}

fn gpui_mods(m: &Modifiers) -> KeyModifiers {
    let mut mods = KeyModifiers::NONE;
    if m.control {
        mods |= KeyModifiers::CTRL;
    }
    if m.alt {
        mods |= KeyModifiers::ALT;
    }
    if m.shift {
        mods |= KeyModifiers::SHIFT;
    }
    if m.platform {
        mods |= KeyModifiers::SUPER;
    }
    mods
}

fn dpi_from_scale(scale: f32) -> u32 {
    (96.0 * scale).round().clamp(72.0, 384.0) as u32
}

fn size_dpr(dpi: u32) -> f32 {
    (dpi as f32 / 96.0).max(0.5)
}

fn wheel_y_lines(event: &ScrollWheelEvent, cell_h: f32) -> isize {
    let y = match event.delta {
        ScrollDelta::Lines(p) => p.y,
        ScrollDelta::Pixels(p) => f32::from(p.y) / cell_h.max(1.),
    };
    y.round() as isize
}

fn map_keystroke(ks: &Keystroke) -> Option<(KeyCode, KeyModifiers)> {
    let mut mods = KeyModifiers::NONE;
    if ks.modifiers.control {
        mods |= KeyModifiers::CTRL;
    }
    if ks.modifiers.alt {
        mods |= KeyModifiers::ALT;
    }
    if ks.modifiers.shift {
        mods |= KeyModifiers::SHIFT;
    }
    if ks.modifiers.platform {
        mods |= KeyModifiers::SUPER;
    }

    // Chrome shortcuts stay on AppShell actions; do not send them to the PTY.
    if ks.modifiers.control
        && ks.modifiers.shift
        && matches!(ks.key.as_str(), "p" | "c" | "v" | "f")
    {
        return None;
    }
    if ks.modifiers.control && matches!(ks.key.as_str(), "t" | "w" | "q" | "p") {
        return None;
    }
    if ks.key == "insert"
        && ((ks.modifiers.control && !ks.modifiers.shift)
            || (ks.modifiers.shift && !ks.modifiers.control))
    {
        return None;
    }

    let key = match ks.key.as_str() {
        "enter" | "return" => KeyCode::Enter,
        "tab" => KeyCode::Tab,
        "escape" => KeyCode::Escape,
        "backspace" => KeyCode::Backspace,
        "delete" => KeyCode::Delete,
        "up" => KeyCode::UpArrow,
        "down" => KeyCode::DownArrow,
        "left" => KeyCode::LeftArrow,
        "right" => KeyCode::RightArrow,
        "home" => KeyCode::Home,
        "end" => KeyCode::End,
        "pageup" => KeyCode::PageUp,
        "pagedown" => KeyCode::PageDown,
        "space" => KeyCode::Char(' '),
        "insert" => KeyCode::Insert,
        other => {
            if let Some(n) = other.strip_prefix('f').and_then(|s| s.parse::<u8>().ok()) {
                if (1..=24).contains(&n) {
                    KeyCode::Function(n)
                } else {
                    return None;
                }
            } else if !ks.modifiers.control && !ks.modifiers.alt {
                let ch = ks
                    .key_char
                    .as_deref()
                    .and_then(|s| s.chars().next())
                    .or_else(|| {
                        other
                            .chars()
                            .next()
                            .filter(|c| c.is_ascii() && other.len() == 1)
                    })?;
                // Shift is already applied in key_char.
                if ks.key_char.is_some() {
                    mods.remove(KeyModifiers::SHIFT);
                }
                KeyCode::Char(ch)
            } else if other.len() == 1 {
                let c = other.chars().next()?;
                KeyCode::Char(c)
            } else {
                return None;
            }
        }
    };
    Some((key, mods))
}

impl Render for TermPane {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if self.painter.is_some() {
            return div()
                .id("term-pane")
                .size_full()
                .min_h_0()
                .overflow_hidden()
                .on_scroll_wheel(cx.listener(|this, event, _, cx| {
                    this.on_scroll_wheel(event);
                    cx.stop_propagation();
                    cx.notify();
                }))
                .child(TermScreen { term: cx.entity() })
                .into_any_element();
        }

        let font_px = self.font_px;
        let (lines, cursor, fg, bg) = self.visible_text();
        let cursor_row = cursor.map(|(row, _)| row);
        let cursor_col = cursor.map(|(_, col)| col).unwrap_or(0);

        div()
            .id("term-pane")
            .size_full()
            .min_h_0()
            .p_2()
            .bg(rgb(bg))
            .text_color(rgb(fg))
            .text_size(px(font_px))
            .font_family("Consolas")
            .overflow_hidden()
            .on_scroll_wheel(cx.listener(|this, event, _, cx| {
                this.on_scroll_wheel(event);
                cx.stop_propagation();
                cx.notify();
            }))
            .child(div().v_flex().children(lines.into_iter().enumerate().map(
                move |(row, line)| {
                    let display = if line.is_empty() {
                        " ".to_string()
                    } else {
                        line
                    };
                    let is_cursor = cursor_row == Some(row);
                    div().whitespace_nowrap().child(if is_cursor {
                        with_cursor_block(&display, cursor_col)
                    } else {
                        display
                    })
                },
            )))
            .into_any_element()
    }
}

/// Persistent GPUI element: `canvas()` is FnOnce and goes blank if a resize
/// paints without rebuilding the tree.
struct TermScreen {
    term: Entity<TermPane>,
}

impl IntoElement for TermScreen {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

impl Element for TermScreen {
    type RequestLayoutState = Style;
    type PrepaintState = Hitbox;

    fn id(&self) -> Option<ElementId> {
        Some("term-screen".into())
    }

    fn source_location(&self) -> Option<&'static core::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, Self::RequestLayoutState) {
        let mut style = Style::default();
        style.size = gpui::Size::full();
        style.overflow = point(Overflow::Hidden, Overflow::Hidden);
        let layout_id = window.request_layout(style.clone(), [], cx);
        (layout_id, style)
    }

    fn prepaint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _request_layout: &mut Style,
        window: &mut Window,
        cx: &mut App,
    ) -> Self::PrepaintState {
        let scale = window.scale_factor();
        self.term.update(cx, |term, cx| {
            term.apply_layout(bounds.size.width, bounds.size.height, scale, cx);
        });
        window.insert_hitbox(bounds, HitboxBehavior::Normal)
    }

    fn paint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        style: &mut Style,
        hitbox: &mut Hitbox,
        window: &mut Window,
        cx: &mut App,
    ) {
        window.set_cursor_style(CursorStyle::IBeam, hitbox);
        style.paint(bounds, window, cx, |window, cx| {
            let paint = self.term.update(cx, |term, _| term.try_glyph_paint());
            if let Some(paint) = paint {
                crate::glyph_paint::paint_term(window, bounds, &paint);
            } else {
                window.paint_quad(fill(bounds, rgb(0x0c0c0c)));
            }
        });

        let entity = self.term.clone();
        let hitbox = hitbox.clone();
        window.on_mouse_event({
            let entity = entity.clone();
            let hitbox = hitbox.clone();
            move |event: &MouseDownEvent, phase, window, cx| {
                if phase != DispatchPhase::Bubble {
                    return;
                }
                if !hitbox.is_hovered(window) {
                    return;
                }
                // Left-click used to stop_propagation before AppShell.track_focus
                // ran, so typing stayed dead until a right-click bubbled. Focus
                // the shell on any press in the pane.
                entity.update(cx, |term, cx| {
                    window.focus(&term.shell_focus, cx);
                    if event.button != MouseButton::Left {
                        return;
                    }
                    term.on_mouse_down(event.position, bounds, event.click_count, &event.modifiers);
                    cx.notify();
                });
                if event.button == MouseButton::Left {
                    cx.stop_propagation();
                }
            }
        });
        window.on_mouse_event({
            let entity = entity.clone();
            move |event: &MouseMoveEvent, phase, _, cx| {
                if phase != DispatchPhase::Bubble {
                    return;
                }
                if event.pressed_button != Some(MouseButton::Left) {
                    return;
                }
                entity.update(cx, |term, cx| {
                    if term.on_mouse_drag(event.position, bounds, &event.modifiers) {
                        cx.notify();
                    }
                });
            }
        });
        window.on_mouse_event({
            move |event: &MouseUpEvent, phase, _, cx| {
                if phase != DispatchPhase::Bubble || event.button != MouseButton::Left {
                    return;
                }
                entity.update(cx, |term, cx| {
                    term.on_mouse_up(event.position, bounds, &event.modifiers);
                    cx.notify();
                });
            }
        });
    }
}

fn with_cursor_block(line: &str, col: usize) -> String {
    let mut chars: Vec<char> = line.chars().collect();
    while chars.len() < col + 1 {
        chars.push(' ');
    }
    if let Some(ch) = chars.get_mut(col) {
        if *ch == ' ' {
            *ch = '█';
        }
    }
    chars.into_iter().collect()
}
