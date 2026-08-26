//! Mux `LocalPane` (same host as wezterm-gui), painted as monospaced GPUI text.
//!
//! Spawn goes through `mux::domain::LocalDomain::spawn_pane` → ConPTY +
//! `wezterm-term` inside `LocalPane`. Not a sample/demo shell. Not the
//! wezterm-gui glyph atlas.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use gpui::*;
use gpui_component::StyledExt;
use mux::pane::{Pane, PaneId};
use mux::{Mux, MuxNotification};
use wezterm_term::{KeyCode, KeyModifiers, TerminalSize};

const DEFAULT_ROWS: usize = 24;
const DEFAULT_COLS: usize = 80;

struct LiveMux {
    pane: Arc<dyn Pane>,
    alive: Arc<AtomicBool>,
}

pub struct TermPane {
    live: Result<LiveMux, String>,
    font_px: f32,
}

impl TermPane {
    pub fn spawn(font_px: f32, cx: &mut Context<Self>) -> Self {
        match spawn_live(cx) {
            Ok(live) => Self {
                live: Ok(live),
                font_px,
            },
            Err(err) => Self {
                live: Err(format!("{err:#}")),
                font_px,
            },
        }
    }

    pub fn set_font_px(&mut self, font_px: f32) {
        self.font_px = font_px;
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

    pub fn resize_to_pixels(&mut self, width: Pixels, height: Pixels) {
        let Ok(live) = self.live.as_mut() else {
            return;
        };
        let cell_w = self.font_px * 0.62;
        let cell_h = self.font_px * 1.28;
        if cell_w < 1. || cell_h < 1. {
            return;
        }
        let cols = ((f32::from(width) / cell_w).floor() as usize).clamp(8, 400);
        let rows = ((f32::from(height) / cell_h).floor() as usize).clamp(2, 200);
        let dims = live.pane.get_dimensions();
        if dims.cols == cols && dims.viewport_rows == rows {
            return;
        }
        let size = TerminalSize {
            rows,
            cols,
            pixel_width: f32::from(width) as usize,
            pixel_height: f32::from(height) as usize,
            dpi: 96,
        };
        let _ = live.pane.resize(size);
    }

    fn visible_text(&self) -> (Vec<String>, Option<(usize, usize)>, u32, u32) {
        match &self.live {
            Ok(live) => {
                let dims = live.pane.get_dimensions();
                let cursor = live.pane.get_cursor_position();
                let pal = live.pane.palette();
                let top = dims.physical_top;
                let end = top.saturating_add(dims.viewport_rows as isize);
                let (first, lines) = live.pane.get_lines(top..end);
                let text: Vec<String> = lines.iter().map(|line| line.as_str().into_owned()).collect();
                let vis_row = usize::try_from(cursor.y.saturating_sub(first)).ok();
                let vis_col = cursor.x;
                let (fr, fg, fb, _) = pal.foreground.as_rgba_u8();
                let (br, bg, bb, _) = pal.background.as_rgba_u8();
                let fg = u32::from_be_bytes([0, fr, fg, fb]);
                let bg = u32::from_be_bytes([0, br, bg, bb]);
                (text, vis_row.map(|row| (row, vis_col)), fg, bg)
            }
            Err(err) => (vec![err.clone()], None, 0xc8c8c8, 0x0c0c0c),
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
    fn render(&mut self, window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        let bounds = window.viewport_size();
        // Title + tabs + status are ~92px; keep a floor so we never resize to 0.
        let pane_h = (f32::from(bounds.height) - 92.).max(48.);
        let pane_w = f32::from(bounds.width).max(64.);
        self.resize_to_pixels(px(pane_w), px(pane_h));

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
