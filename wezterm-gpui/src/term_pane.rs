//! Live PTY + wezterm-term model, painted as monospaced GPUI text.
//!
//! Not the wezterm-gui glyph atlas. No mux. No `window/` event loop.

use std::io::Read;
use std::sync::Arc;

use anyhow::Context as _;
use gpui::*;
use gpui_component::StyledExt;
use portable_pty::{native_pty_system, CommandBuilder, MasterPty, PtySize};
use wezterm_term::color::ColorPalette;
use wezterm_term::{
    CursorPosition, KeyCode, KeyModifiers, Terminal, TerminalConfiguration, TerminalSize,
};

const DEFAULT_ROWS: usize = 24;
const DEFAULT_COLS: usize = 80;

#[derive(Debug)]
struct PocTermConfig;

impl TerminalConfiguration for PocTermConfig {
    fn color_palette(&self) -> ColorPalette {
        ColorPalette::default()
    }
}

struct LivePty {
    terminal: Terminal,
    master: Box<dyn MasterPty>,
    child: Box<dyn portable_pty::Child + Send>,
    size: TerminalSize,
}

pub struct TermPane {
    live: Result<LivePty, String>,
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
                let title = live.terminal.get_title();
                if title.is_empty() || title.eq_ignore_ascii_case("wezterm") {
                    "shell".into()
                } else {
                    title.to_string()
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
        live.terminal.key_down(key, mods).ok();
        true
    }

    pub fn clear_scrollback(&mut self) {
        if let Ok(live) = self.live.as_mut() {
            live.terminal.erase_scrollback();
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
        if live.size.cols == cols && live.size.rows == rows {
            return;
        }
        let size = TerminalSize {
            rows,
            cols,
            pixel_width: f32::from(width) as usize,
            pixel_height: f32::from(height) as usize,
            dpi: 96,
        };
        let pty_size = PtySize {
            rows: rows as u16,
            cols: cols as u16,
            pixel_width: size.pixel_width as u16,
            pixel_height: size.pixel_height as u16,
        };
        if live.master.resize(pty_size).is_ok() {
            live.terminal.resize(size);
            live.size = size;
        }
    }

    fn visible_text(&self) -> (Vec<String>, Option<CursorPosition>, u32, u32) {
        match &self.live {
            Ok(live) => {
                let cursor = live.terminal.cursor_pos();
                let screen = live.terminal.screen();
                let rows = screen.physical_rows;
                let mut buf = std::collections::VecDeque::with_capacity(rows);
                screen.for_each_phys_line(|_, line| {
                    if buf.len() == rows {
                        buf.pop_front();
                    }
                    buf.push_back(line.as_str().into_owned());
                });
                let lines: Vec<String> = buf.into_iter().collect();
                let pal = live.terminal.palette();
                let (fr, fg, fb, _) = pal.foreground.as_rgba_u8();
                let (br, bg, bb, _) = pal.background.as_rgba_u8();
                let fg = u32::from_be_bytes([0, fr, fg, fb]);
                let bg = u32::from_be_bytes([0, br, bg, bb]);
                (lines, Some(cursor), fg, bg)
            }
            Err(err) => (vec![err.clone()], None, 0xc8c8c8, 0x0c0c0c),
        }
    }
}

impl Drop for TermPane {
    fn drop(&mut self) {
        if let Ok(live) = self.live.as_mut() {
            let _ = live.child.kill();
        }
    }
}

fn spawn_live(cx: &mut Context<TermPane>) -> anyhow::Result<LivePty> {
    let size = TerminalSize {
        rows: DEFAULT_ROWS,
        cols: DEFAULT_COLS,
        pixel_width: DEFAULT_COLS * 8,
        pixel_height: DEFAULT_ROWS * 16,
        dpi: 96,
    };
    let pty_system = native_pty_system();
    let pair = pty_system
        .openpty(PtySize {
            rows: size.rows as u16,
            cols: size.cols as u16,
            pixel_width: size.pixel_width as u16,
            pixel_height: size.pixel_height as u16,
        })
        .context("openpty")?;

    let cmd = CommandBuilder::new_default_prog();
    let child = pair
        .slave
        .spawn_command(cmd)
        .context("spawn default shell")?;
    drop(pair.slave);

    let mut reader = pair.master.try_clone_reader().context("clone pty reader")?;
    let writer = pair.master.take_writer().context("pty writer")?;

    let mut terminal = Terminal::new(
        size,
        Arc::new(PocTermConfig),
        "WezTerm",
        "gpui-poc",
        Box::new(writer),
    );
    #[cfg(windows)]
    terminal.enable_conpty_quirks();

    let (tx, rx) = async_channel::unbounded::<Vec<u8>>();
    std::thread::Builder::new()
        .name("wezterm-gpui-pty".into())
        .spawn(move || {
            let mut buf = [0u8; 8192];
            loop {
                match reader.read(&mut buf) {
                    Ok(0) => break,
                    Ok(n) => {
                        if tx.send_blocking(buf[..n].to_vec()).is_err() {
                            break;
                        }
                    }
                    Err(_) => break,
                }
            }
        })
        .context("pty reader thread")?;

    cx.spawn(async move |this, cx| {
        while let Ok(chunk) = rx.recv().await {
            let ok = this.update(cx, |this, cx| {
                if let Ok(live) = this.live.as_mut() {
                    live.terminal.advance_bytes(&chunk);
                }
                cx.notify();
            });
            if ok.is_err() {
                break;
            }
        }
    })
    .detach();

    Ok(LivePty {
        terminal,
        master: pair.master,
        child,
        size,
    })
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
        let cursor_row = cursor.map(|c| c.y as usize);
        let cursor_col = cursor.map(|c| c.x).unwrap_or(0);

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
