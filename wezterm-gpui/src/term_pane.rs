//! Mux `LocalPane` (same host as wezterm-gui).
//!
//! Spawn goes through `mux::domain::LocalDomain::spawn_pane` → ConPTY +
//! `wezterm-term` inside `LocalPane`. Paint prefers wezterm-font sprites
//! (cached `paint_image` + cell quads). Consolas GPUI text is the fallback
//! if FreeType/shaper init fails. Not the wezterm-gui glyph atlas.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use gpui::*;
use gpui_component::StyledExt;
use mux::pane::{Pane, PaneId};
use mux::renderable::RenderableDimensions;
use mux::{Mux, MuxNotification};
use wezterm_term::color::ColorPalette;
use wezterm_term::input::{MouseButton, MouseEvent, MouseEventKind};
use wezterm_term::{KeyCode, KeyModifiers, Line, StableRowIndex, TerminalSize};

use crate::glyph_paint::{GlyphPainter, TermPaint};

const DEFAULT_ROWS: usize = 24;
const DEFAULT_COLS: usize = 80;

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
}

struct PaintCache {
    seq: usize,
    top: StableRowIndex,
    cursor: Option<(usize, usize)>,
    cols: usize,
    rows: usize,
    dpi: u32,
    font_px: u32,
    paint: TermPaint,
}

impl TermPane {
    pub fn spawn(font_px: f32, cx: &mut Context<Self>) -> Self {
        let _ = crate::mux_host::ensure_init();
        let painter = match GlyphPainter::new() {
            Ok(p) => Some(p),
            Err(err) => {
                eprintln!("wezterm-gpui wezterm-font init: {err:#}");
                None
            }
        };
        match spawn_live(cx) {
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
                    "cmd.exe".into()
                } else {
                    title
                }
            }
            Err(_) => "error".into(),
        }
    }

    pub fn key_down(&mut self, event: &KeyDownEvent) -> bool {
        let Ok(live) = self.live.as_mut() else {
            return false;
        };
        let Some((key, mods)) = map_keystroke(&event.keystroke) else {
            return false;
        };
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
                MouseButton::WheelUp(y as usize)
            } else {
                MouseButton::WheelDown((-y) as usize)
            };
            let _ = live.pane.mouse_event(MouseEvent {
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
                let text: Vec<String> = lines.iter().map(|line| line.as_str().into_owned()).collect();
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
        if let Some(cache) = &self.paint_cache {
            if cache.seq == seq
                && cache.top == top
                && cache.cursor == cursor
                && cache.cols == dims.cols
                && cache.rows == dims.viewport_rows
                && cache.dpi == dims.dpi
                && cache.font_px == font_px
            {
                let mut paint = cache.paint.clone();
                paint.drop_images = Vec::new();
                return Some(paint);
            }
        }
        let pal = live.pane.palette();
        let end = top.saturating_add(dims.viewport_rows as isize);
        let (_, lines) = live.pane.get_lines(top..end);
        let painter = self.painter.as_mut()?;
        painter.sync_font(self.font_px, painter.dpi());
        match painter.layout(&lines, cursor, &pal, dims.cols) {
            Ok(paint) => {
                self.paint_cache = Some(PaintCache {
                    seq,
                    top,
                    cursor,
                    cols: dims.cols,
                    rows: dims.viewport_rows,
                    dpi: dims.dpi,
                    font_px,
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

fn spawn_live(cx: &mut Context<TermPane>) -> anyhow::Result<LiveMux> {
    let size = TerminalSize {
        rows: DEFAULT_ROWS,
        cols: DEFAULT_COLS,
        pixel_width: DEFAULT_COLS * 8,
        pixel_height: DEFAULT_ROWS * 16,
        dpi: 96,
    };
    let pane = crate::mux_host::spawn_cmd_exe(size)?;
    let pane_id = pane.pane_id();
    let alive = Arc::new(AtomicBool::new(true));
    let (tx, rx) = async_channel::unbounded::<()>();
    {
        let alive = Arc::clone(&alive);
        Mux::get().subscribe(move |n| {
            if !alive.load(Ordering::Relaxed) {
                return false;
            }
            if notification_is_pane(&n, pane_id) {
                let _ = tx.send_blocking(());
            }
            true
        });
    }

    cx.spawn(async move |this, cx| {
        while let Ok(()) = rx.recv().await {
            while rx.try_recv().is_ok() {}
            if this.update(cx, |_, cx| cx.notify()).is_err() {
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
    if ks.modifiers.control && ks.modifiers.shift && ks.key == "p" {
        return None;
    }
    if ks.modifiers.control && matches!(ks.key.as_str(), "t" | "w" | "q" | "p") {
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
                    .or_else(|| other.chars().next().filter(|c| c.is_ascii() && other.len() == 1))?;
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
                .child(TermScreen {
                    term: cx.entity(),
                })
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
                    div()
                        .whitespace_nowrap()
                        .child(if is_cursor {
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
    type PrepaintState = ();

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
    ) {
        let scale = window.scale_factor();
        self.term.update(cx, |term, cx| {
            term.apply_layout(bounds.size.width, bounds.size.height, scale, cx);
        });
    }

    fn paint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        style: &mut Style,
        _prepaint: &mut (),
        window: &mut Window,
        cx: &mut App,
    ) {
        style.paint(bounds, window, cx, |window, cx| {
            let paint = self.term.update(cx, |term, _| term.try_glyph_paint());
            if let Some(paint) = paint {
                crate::glyph_paint::paint_term(window, bounds, &paint);
            } else {
                window.paint_quad(fill(bounds, rgb(0x0c0c0c)));
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
