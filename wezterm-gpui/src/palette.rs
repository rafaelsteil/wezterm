use gpui::*;
use gpui_component::{
    ActiveTheme, StyledExt, WindowExt,
    input::{Input, InputState},
    label::Label,
    notification::Notification,
};

/// Hardcoded WezTerm-like commands for the POC. Not wired to mux/key assignments.
const SAMPLE_COMMANDS: &[&str] = &[
    "Copy to clipboard",
    "Paste from clipboard",
    "New Window",
    "New Tab",
    "Split pane horizontally",
    "Split pane vertically",
    "Toggle full screen mode",
    "Search pane output",
    "Clear scrollback",
    "Activate Command Palette",
    "Character Selector",
    "Pane Selector",
    "Rename tab",
    "Prompt the user for confirmation",
    "Prompt the user for a line of text",
    "Reload configuration",
    "Show debug overlay",
    "Increase font size",
    "Decrease font size",
    "Reset font size",
    "Close current pane",
    "Close current tab",
    "Quit WezTerm",
];

pub enum PaletteEvent {
    Executed(String),
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
        let query = cx.new(|cx| InputState::new(window, cx).placeholder("Type a command…"));
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
        let brief = self.filtered(cx).get(self.selected).copied();
        if let Some(brief) = brief {
            self.run(brief, window, cx);
        }
    }

    pub fn dismiss(&mut self, cx: &mut Context<Self>) {
        cx.emit(PaletteEvent::Dismissed);
        cx.notify();
    }

    fn filtered(&self, cx: &App) -> Vec<&'static str> {
        let q = self.query.read(cx).value().to_lowercase();
        SAMPLE_COMMANDS
            .iter()
            .copied()
            .filter(|brief| q.is_empty() || brief.to_lowercase().contains(&q))
            .collect()
    }

    fn run(&mut self, brief: &str, window: &mut Window, cx: &mut Context<Self>) {
        self.last_ran = Some(brief.to_string());
        if !matches!(brief, "Copy to clipboard" | "Paste from clipboard") {
            window.push_notification(Notification::info(format!("POC: would run `{brief}`")), cx);
        }
        println!("wezterm-gpui palette: {brief}");
        cx.emit(PaletteEvent::Executed(brief.to_string()));
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
        let status = self
            .last_ran
            .as_deref()
            .unwrap_or("Type to filter · ↑↓ select · Enter run · Esc close");

        div()
            .id("command-palette")
            .v_flex()
            .w(px(560.))
            .max_h(px(440.))
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
                    .min_h(px(180.))
                    .max_h(px(280.))
                    .overflow_y_scroll()
                    .v_flex()
                    .gap_1()
                    .children(matches.into_iter().enumerate().map(|(ix, brief)| {
                        let label = brief.to_string();
                        let run_brief = brief;
                        let is_sel = ix == selected;
                        div()
                            .id(("cmd", ix as u64))
                            .w_full()
                            .px_3()
                            .py_2()
                            .rounded_md()
                            .bg(if is_sel {
                                cx.theme().accent.opacity(0.22)
                            } else {
                                cx.theme().background.opacity(0.)
                            })
                            .hover(|s| s.bg(cx.theme().accent.opacity(0.15)))
                            .cursor_pointer()
                            .on_click(cx.listener(move |this, _, window, cx| {
                                this.run(run_brief, window, cx);
                            }))
                            .child(Label::new(label))
                    })),
            )
            .child(
                Label::new(status.to_string())
                    .text_size(px(12.))
                    .text_color(cx.theme().muted_foreground),
            )
    }
}
