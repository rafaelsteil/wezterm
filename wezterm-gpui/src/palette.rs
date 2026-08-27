use gpui::prelude::FluentBuilder;
use gpui::*;
use gpui_component::{
    ActiveTheme, StyledExt,
    input::{Input, InputState},
    label::Label,
};

use crate::commands::{PALETTE_COMMANDS, PaletteCommand};

pub enum PaletteEvent {
    Executed(&'static str),
    Dismissed,
}

pub struct CommandPalette {
    query: Entity<InputState>,
    selected: usize,
    last_ran: Option<String>,
}

impl EventEmitter<PaletteEvent> for CommandPalette {}

impl CommandPalette {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let query = cx.new(|cx| {
            InputState::new(window, cx).placeholder("Type a command…  not-yet-implemented rows are dimmed")
        });
        cx.observe(&query, |this, _, cx| {
            this.selected = 0;
            cx.notify();
        })
        .detach();
        Self {
            query,
            selected: 0,
            last_ran: None,
        }
    }

    pub fn focus_search(&self, window: &mut Window, cx: &mut Context<Self>) {
        self.query.update(cx, |input, cx| {
            input.set_value("", window, cx);
            input.focus(window, cx);
        });
    }

    pub fn move_sel(&mut self, delta: isize, cx: &mut Context<Self>) {
        let n = self.filtered(cx).len() as isize;
        if n == 0 {
            self.selected = 0;
        } else {
            self.selected = (self.selected as isize + delta).rem_euclid(n) as usize;
        }
        cx.notify();
    }

    pub fn confirm(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let cmd = self.filtered(cx).get(self.selected).copied();
        if let Some(cmd) = cmd {
            self.run(cmd, window, cx);
        }
    }

    pub fn dismiss(&mut self, cx: &mut Context<Self>) {
        cx.emit(PaletteEvent::Dismissed);
        cx.notify();
    }

    fn filtered(&self, cx: &App) -> Vec<&'static PaletteCommand> {
        let q = self.query.read(cx).value().to_lowercase();
        PALETTE_COMMANDS
            .iter()
            .filter(|cmd| q.is_empty() || cmd.haystack().to_lowercase().contains(&q))
            .collect()
    }

    fn run(&mut self, cmd: &'static PaletteCommand, _window: &mut Window, cx: &mut Context<Self>) {
        if !cmd.is_wired() {
            self.last_ran = Some(format!(
                "Not yet implemented ({:?}): {}",
                cmd.kind, cmd.brief
            ));
            println!("wezterm-gpui palette listed (not wired): {} ({})", cmd.id, cmd.brief);
            cx.notify();
            return;
        }
        self.last_ran = Some(cmd.brief.to_string());
        println!("wezterm-gpui palette: {} ({})", cmd.id, cmd.brief);
        cx.emit(PaletteEvent::Executed(cmd.id));
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
            let wired = matches.iter().filter(|c| c.is_wired()).count();
            format!(
                "{n} commands · {wired} wired · dimmed = not yet · ↑↓ select · Enter run · Esc close"
            )
        });

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
            .bg(cx.theme().background)
            .text_color(cx.theme().foreground)
            .shadow_lg()
            .child(
                Label::new("Command Palette")
                    .text_size(px(14.))
                    .font_weight(FontWeight::SEMIBOLD),
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
                    .v_flex()
                    .gap_1()
                    .children(matches.into_iter().enumerate().map(|(ix, cmd)| {
                        let is_sel = ix == selected;
                        let wired = cmd.is_wired();
                        let fg = if wired {
                            cx.theme().foreground
                        } else {
                            cx.theme().muted_foreground
                        };
                        let title = format!("{}: {}", cmd.menubar, cmd.brief);
                        div()
                            .id(("cmd", ix as u64))
                            .w_full()
                            .px_3()
                            .py_2()
                            .rounded_md()
                            .opacity(if wired { 1. } else { 0.62 })
                            .bg(if is_sel {
                                cx.theme().accent.opacity(if wired { 0.22 } else { 0.10 })
                            } else {
                                cx.theme().background.opacity(0.)
                            })
                            .hover(|s| s.bg(cx.theme().accent.opacity(if wired { 0.15 } else { 0.08 })))
                            .when(wired, |s| s.cursor_pointer())
                            .on_click(cx.listener(move |this, _, window, cx| {
                                this.run(cmd, window, cx);
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
                                                    .text_color(fg),
                                            )
                                            .child(
                                                Label::new(cmd.doc.to_string())
                                                    .text_size(px(11.))
                                                    .text_color(cx.theme().muted_foreground),
                                            ),
                                    )
                                    .when(!cmd.keys.is_empty(), |this| {
                                        this.child(
                                            Label::new(cmd.keys.to_string())
                                                .text_size(px(11.))
                                                .text_color(cx.theme().muted_foreground),
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
