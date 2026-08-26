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
use mux::{Mux, MuxNotification};
use wezterm_term::color::ColorPalette;
use wezterm_term::{KeyCode, KeyModifiers, Line, TerminalSize};

use crate::glyph_paint::GlyphPainter;

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
    /// First valid layout calls `Pane::resize` (PTY + terminal). Later layouts
    /// call `resize_display` only — live `ResizePseudoConsole` smears cmd.exe.
    pty_synced: bool,
    pty_cols: usize,
    pty_rows: usize,
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
            },
            Err(err) => Self {
                live: Err(format!("{err:#}")),
                font_px,
                painter,
                pty_synced: false,
                pty_cols: DEFAULT_COLS,
                pty_rows: DEFAULT_ROWS,
            },
        }
    }

    pub fn set_font_px(&mut self, font_px: f32) {
        self.font_px = font_px;
        if let Some(painter) = &mut self.painter {
            painter.sync_font_px(font_px);
        }
    }

    pub fn paint_mode(&self) -> &'static str {
        if self.painter.is_some() {
            "wezterm-font glyphs (sprite cache)"
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
        format!(
            "{mode}  pty {}×{}  view {}×{}",
            self.pty_cols, self.pty_rows, dims.cols, dims.viewport_rows
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
        live.pane.key_down(key, mods).ok();
        true
    }

    pub fn clear_scrollback(&mut self) {
        if let Ok(live) = self.live.as_mut() {
            live.pane
                .erase_scrollback(config::keyassignment::ScrollbackEraseMode::ScrollbackOnly);
        }
    }

    fn cell_px(&self) -> (f32, f32) {
        if let Some(painter) = &self.painter {
            if let Ok(size) = painter.cell_size() {
                return (size.width, size.height);
            }
        }
        (self.font_px * 0.62, self.font_px * 1.28)
    }

    pub fn resize_to_pixels(&mut self, width: Pixels, height: Pixels) {
        let Some(size) = self.size_from_pixels(width, height) else {
            return;
        };
        if !self.grid_differs(&size) {
            return;
        }
        if self.pty_synced {
            if let Ok(live) = &self.live {
                let _ = live.pane.resize_display(size);
            }
            return;
        }
        if let Ok(live) = &mut self.live {
            let _ = live.pane.resize(size);
            self.pty_synced = true;
            self.pty_cols = size.cols;
            self.pty_rows = size.rows;
        }
    }

    fn grid_differs(&self, size: &TerminalSize) -> bool {
        let Ok(live) = &self.live else {
            return false;
        };
        let dims = live.pane.get_dimensions();
        dims.cols != size.cols || dims.viewport_rows != size.rows
    }

    fn size_from_pixels(&self, width: Pixels, height: Pixels) -> Option<TerminalSize> {
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
        Some(TerminalSize {
            rows,
            cols,
            pixel_width: (cols as f32 * cell_w).round() as usize,
            pixel_height: (rows as f32 * cell_h).round() as usize,
            dpi: 96,
        })
    }

    fn visible_lines(&self) -> (Vec<Line>, Option<(usize, usize)>, ColorPalette) {
        match &self.live {
            Ok(live) => {
                let dims = live.pane.get_dimensions();
                let cursor = live.pane.get_cursor_position();
                let pal = live.pane.palette();
                let top = dims.physical_top;
                let end = top.saturating_add(dims.viewport_rows as isize);
                let (first, lines) = live.pane.get_lines(top..end);
                let vis_row = usize::try_from(cursor.y.saturating_sub(first)).ok();
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

    pub(crate) fn try_glyph_paint(&mut self) -> Option<crate::glyph_paint::TermPaint> {
        if self.painter.is_none() || self.live.is_err() {
            return None;
        }
        let (lines, cursor, pal) = self.visible_lines();
        let painter = self.painter.as_mut()?;
        painter.sync_font_px(self.font_px);
        match painter.layout(&lines, cursor, &pal) {
            Ok(paint) => Some(paint),
            Err(err) => {
                eprintln!("wezterm-gpui glyph paint: {err:#}");
                self.painter = None;
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
        _window: &mut Window,
        cx: &mut App,
    ) {
        self.term.update(cx, |term, _cx| {
            term.resize_to_pixels(bounds.size.width, bounds.size.height);
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
