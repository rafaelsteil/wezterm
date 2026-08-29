use gpui::prelude::FluentBuilder;
use gpui::*;
use gpui_component::{
    ActiveTheme, StyledExt,
    input::{Input, InputEvent, InputState},
    label::Label,
};

use crate::commands::{PALETTE_COMMANDS, PaletteCommand};

pub enum PaletteEvent {
    Executed(String),
    Dismissed,
}

#[derive(Clone)]
struct PaletteRow {
    id: String,
    brief: String,
    doc: String,
    menubar: String,
    keys: String,
    wired: bool,
}

impl PaletteRow {
    fn from_static(cmd: &'static PaletteCommand) -> Self {
        Self {
            id: cmd.id.to_string(),
            brief: cmd.brief.to_string(),
            doc: cmd.doc.to_string(),
            menubar: cmd.menubar.to_string(),
            keys: cmd.keys.to_string(),
            wired: cmd.is_wired(),
        }
    }

    fn haystack(&self) -> String {
        format!("{} {} {} {}", self.menubar, self.brief, self.doc, self.keys)
    }
}

pub struct CommandPalette {
    query: Entity<InputState>,
    selected: usize,
    last_ran: Option<String>,
    /// One execute per palette open (click + Enter used to fire twice).
    armed: bool,
    /// Overflow list does not follow `selected` on its own; wheel works
    /// because `.overflow_y_scroll` handles it. ↑↓ only mutated the index
    /// (045). Immediate children of the tracked div are the command rows.
    scroll: ScrollHandle,
}

impl EventEmitter<PaletteEvent> for CommandPalette {}

impl CommandPalette {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let query = cx.new(|cx| {
            InputState::new(window, cx).placeholder("Type a command…")
        });
        // InputState notifies on caret blink and arrow-key cursor moves, not
        // only on text. observe() used to zero selected on every notify, so
        // ↑↓ jumped back to row 0 (043). Only reset when the query changes.
        cx.subscribe(&query, |this, _, ev: &InputEvent, cx| {
            if matches!(ev, InputEvent::Change) {
                this.selected = 0;
                this.reveal_selected();
                cx.notify();
            }
        })
        .detach();
        Self {
            query,
            selected: 0,
            last_ran: None,
            armed: true,
            scroll: ScrollHandle::new(),
        }
    }

    pub fn focus_search(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.selected = 0;
        self.armed = true;
        self.reveal_selected();
        self.query.update(cx, |input, cx| {
            input.set_value("", window, cx);
            input.focus(window, cx);
        });
        cx.notify();
    }

    pub fn move_sel(&mut self, delta: isize, cx: &mut Context<Self>) {
        let n = self.filtered(cx).len() as isize;
        if n == 0 {
            self.selected = 0;
        } else {
            self.selected = (self.selected as isize + delta).rem_euclid(n) as usize;
        }
        self.reveal_selected();
        cx.notify();
    }

    fn reveal_selected(&self) {
        self.scroll.scroll_to_item(self.selected);
    }

    pub fn confirm(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let cmd = self.filtered(cx).get(self.selected).cloned();
        if let Some(cmd) = cmd {
            self.run(&cmd, window, cx);
        }
    }

    pub fn dismiss(&mut self, cx: &mut Context<Self>) {
        cx.emit(PaletteEvent::Dismissed);
        cx.notify();
    }

    fn all_rows(cx: &App) -> Vec<PaletteRow> {
        let mut rows: Vec<_> = PALETTE_COMMANDS
            .iter()
            .filter(|c| !c.id.starts_with("workspace:"))
            .map(PaletteRow::from_static)
            .collect();
        for profile in crate::mux_host::spawnable_domain_profiles() {
            rows.push(PaletteRow {
                id: format!("domain:{}", profile.id),
                brief: format!("New Tab (Domain {})", profile.label),
                doc: format!("Spawn a tab in mux domain {}", profile.id),
                menubar: "Shell".into(),
                keys: String::new(),
                wired: true,
            });
        }
        // wezterm-gui expanded_commands: Switch to each other workspace,
        // then Create new Workspace. Keep next/previous as extras.
        let current = crate::workspaces::current_view(cx)
            .unwrap_or_else(crate::mux_host::active_workspace);
        let mut names = crate::workspaces::known_names(cx);
        if names.is_empty() {
            names = crate::mux_host::workspace_names();
        }
        for name in names {
            if name == current {
                continue;
            }
            rows.push(PaletteRow {
                id: format!("workspace:switch:{name}"),
                brief: format!("Switch to workspace {name}"),
                doc: String::new(),
                menubar: "Window | Workspace".into(),
                keys: String::new(),
                wired: true,
            });
        }
        for cmd in PALETTE_COMMANDS
            .iter()
            .filter(|c| c.id.starts_with("workspace:"))
        {
            rows.push(PaletteRow::from_static(cmd));
        }
        rows
    }

    fn filtered(&self, cx: &App) -> Vec<PaletteRow> {
        let q = self.query.read(cx).value().to_lowercase();
        Self::all_rows(cx)
            .into_iter()
            .filter(|cmd| q.is_empty() || cmd.haystack().to_lowercase().contains(&q))
            .collect()
    }

    fn run(&mut self, cmd: &PaletteRow, _window: &mut Window, cx: &mut Context<Self>) {
        if !self.armed {
            return;
        }
        if !cmd.wired {
            self.last_ran = Some(format!("Not yet implemented: {}", cmd.brief));
            println!(
                "wezterm-gpui palette listed (not wired): {} ({})",
                cmd.id, cmd.brief
            );
            cx.notify();
            return;
        }
        self.armed = false;
        self.last_ran = Some(cmd.brief.clone());
        println!("wezterm-gpui palette: {} ({})", cmd.id, cmd.brief);
        cx.emit(PaletteEvent::Executed(cmd.id.clone()));
        cx.notify();
    }
}

impl Render for CommandPalette {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let matches = self.filtered(cx);
        let selected = if matches.is_empty() {
            0
        } else {
            self.selected.min(matches.len() - 1)
        };
        let status = self.last_ran.clone().unwrap_or_else(|| {
            let n = matches.len();
            let wired = matches.iter().filter(|c| c.wired).count();
            format!(
                "{n} commands · {wired} wired · dimmed = not yet · ↑↓ select · Enter run · Esc close"
            )
        });
        // wezterm-gui inverts command_palette_fg/bg on the selected row
        // (palette.rs). GPUI used theme.accent @ 0.22, which on this theme
        // sat next to the text color and hid the highlight (042).
        let (palette_bg, palette_fg) = palette_chrome();

        div()
            .id("command-palette")
            .v_flex()
            .w(px(720.))
            .max_h(px(520.))
            .p_3()
            .gap_2()
            .rounded_lg()
            .border_1()
            .border_color(cx.theme().border)
            .bg(palette_bg)
            .text_color(palette_fg)
            .shadow_lg()
            .child(
                Label::new("Command Palette")
                    .text_size(px(14.))
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(palette_fg),
            )
            .child(Input::new(&self.query))
            .child(
                div()
                    .id("command-list")
                    .flex_1()
                    .w_full()
                    .min_h(px(220.))
                    .max_h(px(360.))
                    .overflow_y_scroll()
                    .track_scroll(&self.scroll)
                    .v_flex()
                    .gap_1()
                    .children(matches.into_iter().enumerate().map(|(ix, cmd)| {
                        let is_sel = ix == selected;
                        let wired = cmd.wired;
                        let row_bg = if is_sel { palette_fg } else { palette_bg };
                        let row_fg = if is_sel { palette_bg } else { palette_fg };
                        let doc_fg = if is_sel {
                            palette_bg
                        } else {
                            cx.theme().muted_foreground
                        };
                        let title = format!("{}: {}", cmd.menubar, cmd.brief);
                        let keys = cmd.keys.clone();
                        let doc = cmd.doc.clone();
                        div()
                            .id(("cmd", ix as u64))
                            .w_full()
                            .px_3()
                            .py_2()
                            .rounded_md()
                            .opacity(if wired { 1. } else { 0.62 })
                            .bg(row_bg)
                            .when(wired, |s| s.cursor_pointer())
                            .on_click(cx.listener(move |this, _, window, cx| {
                                this.run(&cmd, window, cx);
                            }))
                            .child(
                                div()
                                    .w_full()
                                    .h_flex()
                                    .items_start()
                                    .gap_2()
                                    .child(
                                        div()
                                            .flex_1()
                                            .min_w(px(0.))
                                            .v_flex()
                                            .gap_1()
                                            .child(
                                                Label::new(title)
                                                    .text_size(px(13.))
                                                    .text_color(row_fg),
                                            )
                                            .child(
                                                Label::new(doc)
                                                    .text_size(px(11.))
                                                    .text_color(doc_fg),
                                            ),
                                    )
                                    .when(!keys.is_empty(), |this| {
                                        this.child(
                                            Label::new(keys)
                                                .text_size(px(11.))
                                                .text_color(doc_fg),
                                        )
                                    }),
                            )
                    })),
            )
            .child(
                Label::new(status)
                    .text_size(px(12.))
                    .text_color(cx.theme().muted_foreground),
            )
    }
}

/// Lua `command_palette_bg_color` / `command_palette_fg_color` (defaults
/// `#333333` / gray 0.75). Selected row inverts these, same as wezterm-gui.
pub(crate) fn palette_chrome() -> (Hsla, Hsla) {
    let cfg = config::configuration();
    (
        srgba_hsla(&cfg.command_palette_bg_color),
        srgba_hsla(&cfg.command_palette_fg_color),
    )
}

fn srgba_hsla(c: &config::RgbaColor) -> Hsla {
    let (r, g, b, _) = c.as_rgba_u8();
    rgb(u32::from_be_bytes([0, r, g, b])).into()
}
