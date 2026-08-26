//! Sibling-window app chrome. Not a terminal; the glyph renderer stays in wezterm-gui.

use gpui::prelude::FluentBuilder;
use gpui::*;
use gpui_component::{
    button::*,
    label::Label,
    notification::Notification,
    tab::{Tab, TabBar},
    ActiveTheme, IconName, Sizable, TitleBar, WindowExt,
};

use crate::confirm::{open_confirm, open_line_prompt};
use crate::palette::{CommandPalette, PaletteEvent};
use crate::term_pane::TermPane;

actions!(
    wezterm_gpui_shell,
    [
        ToggleCommandPalette,
        ClosePalette,
        PaletteMoveUp,
        PaletteMoveDown,
        PaletteConfirm,
        NewTab,
        CloseTab,
        QuitPoc,
    ]
);

const APP_CONTEXT: &str = "AppShell";
const PALETTE_CONTEXT: &str = "PaletteOpen";

pub fn bind_keys(cx: &mut App) {
    cx.bind_keys([
        KeyBinding::new("ctrl-shift-p", ToggleCommandPalette, None),
        KeyBinding::new("ctrl-p", ToggleCommandPalette, None),
        KeyBinding::new("ctrl-t", NewTab, None),
        KeyBinding::new("ctrl-w", CloseTab, None),
        KeyBinding::new("ctrl-q", QuitPoc, None),
        KeyBinding::new("escape", ClosePalette, Some(PALETTE_CONTEXT)),
        KeyBinding::new("up", PaletteMoveUp, Some(PALETTE_CONTEXT)),
        KeyBinding::new("down", PaletteMoveDown, Some(PALETTE_CONTEXT)),
        KeyBinding::new("enter", PaletteConfirm, Some(PALETTE_CONTEXT)),
    ]);
}

struct ShellTab {
    title_override: Option<String>,
    term: Entity<TermPane>,
}

impl ShellTab {
    fn title(&self, cx: &App) -> String {
        self.title_override
            .clone()
            .unwrap_or_else(|| self.term.read(cx).title())
    }
}

pub struct AppShell {
    focus_handle: FocusHandle,
    tabs: Vec<ShellTab>,
    active: usize,
    font_px: f32,
    palette: Entity<CommandPalette>,
    palette_open: bool,
}

impl Focusable for AppShell {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl AppShell {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let palette = cx.new(|cx| CommandPalette::new(window, cx));
        cx.subscribe_in(&palette, window, |this, _, event, window, cx| {
            match event {
                PaletteEvent::Executed(brief) => {
                    this.palette_open = false;
                    this.apply_command(brief, window, cx);
                }
                PaletteEvent::Dismissed => {
                    this.palette_open = false;
                }
            }
            cx.notify();
        })
        .detach();

        Self {
            focus_handle: cx.focus_handle(),
            tabs: vec![Self::new_tab(14., cx)],
            active: 0,
            font_px: 14.,
            palette,
            palette_open: false,
        }
    }

    fn apply_command(&mut self, brief: &str, window: &mut Window, cx: &mut Context<Self>) {
        match brief {
            "New Tab" => self.add_tab(cx),
            "Close current tab" | "Close current pane" => self.confirm_close_active(window, cx),
            "Quit WezTerm" => self.confirm_quit(window, cx),
            "Increase font size" => self.bump_font(1., cx),
            "Decrease font size" => self.bump_font(-1., cx),
            "Reset font size" => self.set_font(14., cx),
            "Clear scrollback" => {
                if let Some(tab) = self.tabs.get(self.active) {
                    tab.term.update(cx, |term, cx| {
                        term.clear_scrollback();
                        cx.notify();
                    });
                }
            }
            "Activate Command Palette" => self.palette_open = true,
            "Rename tab" | "Prompt the user for a line of text" => {
                self.open_rename_prompt(window, cx);
            }
            "Prompt the user for confirmation" => self.open_demo_confirm(window, cx),
            _ => {}
        }
    }

    fn new_tab(font_px: f32, cx: &mut Context<Self>) -> ShellTab {
        let term = cx.new(|cx| TermPane::spawn(font_px, cx));
        ShellTab {
            title_override: None,
            term,
        }
    }

    fn add_tab(&mut self, cx: &mut Context<Self>) {
        self.tabs.push(Self::new_tab(self.font_px, cx));
        self.active = self.tabs.len() - 1;
    }

    fn bump_font(&mut self, delta: f32, cx: &mut Context<Self>) {
        self.set_font((self.font_px + delta).clamp(10., 28.), cx);
    }

    fn set_font(&mut self, font_px: f32, cx: &mut Context<Self>) {
        self.font_px = font_px;
        for tab in &self.tabs {
            tab.term.update(cx, |term, cx| {
                term.set_font_px(font_px);
                cx.notify();
            });
        }
    }

    fn close_tab_at(&mut self, index: usize) {
        if self.tabs.len() <= 1 || index >= self.tabs.len() {
            return;
        }
        self.tabs.remove(index);
        if self.active >= self.tabs.len() {
            self.active = self.tabs.len() - 1;
        } else if self.active > index {
            self.active -= 1;
        }
    }

    fn confirm_close_active(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.confirm_close_tab_at(self.active, window, cx);
    }

    fn confirm_close_tab_at(&mut self, index: usize, window: &mut Window, cx: &mut Context<Self>) {
        if index >= self.tabs.len() {
            return;
        }
        if self.tabs.len() <= 1 {
            self.confirm_quit(window, cx);
            return;
        }
        let title = self.tabs[index].title(cx);
        let shell = cx.entity();
        open_confirm(
            window,
            cx,
            "Close tab?",
            format!("🛑 Really kill tab `{title}` and all contained panes?"),
            "Close",
            true,
            move |_, cx| {
                shell.update(cx, |this, cx| {
                    this.close_tab_at(index);
                    cx.notify();
                });
            },
        );
    }

    fn confirm_quit(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        open_confirm(
            window,
            cx,
            "Quit WezTerm?",
            "🛑 Really Quit WezTerm?",
            "Quit",
            true,
            |_, cx| cx.quit(),
        );
    }

    fn open_rename_prompt(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let current = self
            .tabs
            .get(self.active)
            .map(|t| t.title(cx))
            .unwrap_or_default();
        let shell = cx.entity();
        open_line_prompt(
            window,
            cx,
            "Rename tab",
            "POC prompt (PromptInputLine). Overrides the PTY title for this tab.",
            "Tab title",
            current,
            move |value, window, cx| {
                let name = value.trim();
                if name.is_empty() {
                    window.push_notification(
                        Notification::info("POC: empty name, tab unchanged"),
                        cx,
                    );
                    return;
                }
                let name = name.to_string();
                shell.update(cx, |this, cx| {
                    if let Some(tab) = this.tabs.get_mut(this.active) {
                        tab.title_override = Some(name);
                    }
                    cx.notify();
                });
            },
        );
    }

    fn open_demo_confirm(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        open_confirm(
            window,
            cx,
            "Confirm",
            "POC confirm (termwiz Confirmation overlay). No mux action.",
            "OK",
            false,
            |window, cx| {
                window.push_notification(Notification::info("POC: confirmed"), cx);
            },
        );
    }

    fn open_palette(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.palette_open = true;
        self.palette.update(cx, |p, cx| p.focus_search(window, cx));
        cx.notify();
    }

    fn close_palette(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.palette_open = false;
        window.focus(&self.focus_handle, cx);
        cx.notify();
    }

    fn toggle_palette(&mut self, _: &ToggleCommandPalette, window: &mut Window, cx: &mut Context<Self>) {
        if self.palette_open {
            self.close_palette(window, cx);
        } else {
            self.open_palette(window, cx);
        }
    }

    fn on_close_palette(&mut self, _: &ClosePalette, window: &mut Window, cx: &mut Context<Self>) {
        if self.palette_open {
            self.close_palette(window, cx);
        }
    }

    fn on_palette_up(&mut self, _: &PaletteMoveUp, _: &mut Window, cx: &mut Context<Self>) {
        self.palette.update(cx, |p, cx| p.move_sel(-1, cx));
    }

    fn on_palette_down(&mut self, _: &PaletteMoveDown, _: &mut Window, cx: &mut Context<Self>) {
        self.palette.update(cx, |p, cx| p.move_sel(1, cx));
    }

    fn on_palette_confirm(&mut self, _: &PaletteConfirm, window: &mut Window, cx: &mut Context<Self>) {
        self.palette.update(cx, |p, cx| p.confirm(window, cx));
    }

    fn on_new_tab(&mut self, _: &NewTab, _: &mut Window, cx: &mut Context<Self>) {
        self.add_tab(cx);
        cx.notify();
    }

    fn on_close_tab(&mut self, _: &CloseTab, window: &mut Window, cx: &mut Context<Self>) {
        self.confirm_close_active(window, cx);
    }

    fn on_quit(&mut self, _: &QuitPoc, window: &mut Window, cx: &mut Context<Self>) {
        self.confirm_quit(window, cx);
    }

    fn on_term_key(&mut self, event: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        if self.palette_open || window.has_active_dialog(cx) {
            return;
        }
        if let Some(tab) = self.tabs.get(self.active) {
            tab.term.update(cx, |term, cx| {
                if term.key_down(event) {
                    cx.stop_propagation();
                    cx.notify();
                }
            });
        }
    }
}

impl Render for AppShell {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let active = self.active.min(self.tabs.len().saturating_sub(1));
        let term = self.tabs.get(active).map(|t| t.term.clone());
        let tab_titles: Vec<(usize, String)> = self
            .tabs
            .iter()
            .enumerate()
            .map(|(i, t)| (i, t.title(cx)))
            .collect();
        let palette_open = self.palette_open;
        let status_title = self
            .tabs
            .get(active)
            .map(|t| t.title(cx))
            .unwrap_or_else(|| "—".into());

        div()
            .id("app-shell")
            .track_focus(&self.focus_handle)
            .key_context(if palette_open {
                PALETTE_CONTEXT
            } else {
                APP_CONTEXT
            })
            .on_action(cx.listener(Self::toggle_palette))
            .on_action(cx.listener(Self::on_close_palette))
            .on_action(cx.listener(Self::on_palette_up))
            .on_action(cx.listener(Self::on_palette_down))
            .on_action(cx.listener(Self::on_palette_confirm))
            .on_action(cx.listener(Self::on_new_tab))
            .on_action(cx.listener(Self::on_close_tab))
            .on_action(cx.listener(Self::on_quit))
            .on_key_down(cx.listener(Self::on_term_key))
            .relative()
            .size_full()
            .flex()
            .flex_col()
            .bg(cx.theme().background)
            .text_color(cx.theme().foreground)
            .child(
                TitleBar::new().child(
                    div()
                        .flex()
                        .items_center()
                        .gap_2()
                        .child(
                            Label::new("WezTerm GPUI")
                                .font_weight(FontWeight::SEMIBOLD)
                                .text_size(px(13.)),
                        )
                        .child(
                            Label::new("POC chrome — live PTY, not the glyph atlas")
                                .text_size(px(12.))
                                .text_color(cx.theme().muted_foreground),
                        ),
                ),
            )
            .child(
                TabBar::new("tabs")
                    .selected_index(active)
                    .on_click(cx.listener(|this, index, _, cx| {
                        this.active = *index;
                        cx.notify();
                    }))
                    .children(tab_titles.into_iter().map(|(index, title)| {
                        Tab::new().label(title).suffix(
                            Button::new(("close-tab", index as u64))
                                .icon(IconName::Close)
                                .ghost()
                                .xsmall()
                                .on_click(cx.listener(move |this, _, window, cx| {
                                    this.confirm_close_tab_at(index, window, cx);
                                })),
                        )
                    }))
                    .suffix(
                        Button::new("new-tab")
                            .icon(IconName::Plus)
                            .ghost()
                            .xsmall()
                            .tooltip("New tab (Ctrl+T)")
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.add_tab(cx);
                                cx.notify();
                            })),
                    ),
            )
            .child(
                div()
                    .id("term-host")
                    .flex_1()
                    .w_full()
                    .min_h_0()
                    .bg(rgb(0x0c0c0c))
                    .when_some(term, |this, term| this.child(term)),
            )
            .child(
                div()
                    .id("status-bar")
                    .w_full()
                    .px_3()
                    .py_1()
                    .border_t_1()
                    .border_color(cx.theme().border)
                    .child(
                        Label::new(format!(
                            "Ctrl+Shift+P palette  ·  {}  ·  live PTY in chrome (not wezterm-gui glyphs)",
                            status_title
                        ))
                        .text_size(px(11.))
                        .text_color(cx.theme().muted_foreground),
                    ),
            )
            .when(palette_open, |this| {
                this.child(
                    div()
                        .id("palette-scrim")
                        .absolute()
                        .inset_0()
                        .flex()
                        .justify_center()
                        .pt(px(72.))
                        .bg(gpui::black().opacity(0.45))
                        .on_mouse_down(
                            MouseButton::Left,
                            cx.listener(|this, _, window, cx| {
                                this.close_palette(window, cx);
                            }),
                        )
                        .child(
                            div()
                                .on_mouse_down(MouseButton::Left, |_, _, cx| {
                                    cx.stop_propagation();
                                })
                                .child(self.palette.clone()),
                        ),
                )
            })
    }
}
