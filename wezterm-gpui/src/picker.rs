//! Searchable overlay used by launcher, tab navigator, charselect, and (when
//! `find::PICKER_SEARCH_QUICKSELECT_PANESELECT` is true) search / quickselect /
//! paneselect. Same chrome as the command palette.

use gpui::prelude::FluentBuilder;
use gpui::*;
use gpui_component::{
    ActiveTheme, StyledExt,
    input::{Input, InputEvent, InputState},
    label::Label,
};

use crate::palette::palette_chrome;

#[derive(Clone)]
pub struct PickerItem {
    pub id: String,
    pub title: String,
    pub subtitle: String,
}

pub enum PickerEvent {
    Confirmed(String),
    Dismissed,
}

pub struct Picker {
    title: SharedString,
    placeholder: SharedString,
    items: Vec<PickerItem>,
    query: Entity<InputState>,
    selected: usize,
    armed: bool,
    scroll: ScrollHandle,
}

impl EventEmitter<PickerEvent> for Picker {}

impl Picker {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let query = cx.new(|cx| InputState::new(window, cx).placeholder("Filter…"));
        cx.subscribe(&query, |this, _, ev: &InputEvent, cx| {
            if matches!(ev, InputEvent::Change) {
                this.selected = 0;
                this.reveal_selected();
                cx.notify();
            }
        })
        .detach();
        Self {
            title: "Picker".into(),
            placeholder: "Filter…".into(),
            items: Vec::new(),
            query,
            selected: 0,
            armed: true,
            scroll: ScrollHandle::new(),
        }
    }

    pub fn open(
        &mut self,
        title: impl Into<SharedString>,
        items: Vec<PickerItem>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.title = title.into();
        self.items = items;
        self.selected = 0;
        self.armed = true;
        self.reveal_selected();
        let placeholder = self.placeholder.clone();
        self.query.update(cx, |input, cx| {
            input.set_placeholder(placeholder, window, cx);
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

    pub fn confirm(&mut self, cx: &mut Context<Self>) {
        if !self.armed {
            return;
        }
        if let Some(item) = self.filtered(cx).into_iter().nth(self.selected) {
            self.armed = false;
            cx.emit(PickerEvent::Confirmed(item.id));
        }
    }

    pub fn dismiss(&mut self, cx: &mut Context<Self>) {
        cx.emit(PickerEvent::Dismissed);
        cx.notify();
    }

    fn filtered(&self, cx: &App) -> Vec<PickerItem> {
        let q = self.query.read(cx).value().to_lowercase();
        self.items
            .iter()
            .filter(|item| {
                q.is_empty()
                    || item.title.to_lowercase().contains(&q)
                    || item.subtitle.to_lowercase().contains(&q)
                    || item.id.to_lowercase().contains(&q)
            })
            .cloned()
            .collect()
    }
}

impl Render for Picker {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let matches = self.filtered(cx);
        let selected = if matches.is_empty() {
            0
        } else {
            self.selected.min(matches.len() - 1)
        };
        let status = format!(
            "{} items · ↑↓ select · Enter run · Esc close",
            matches.len()
        );
        let (palette_bg, palette_fg) = palette_chrome();
        let title = self.title.clone();

        div()
            .id("command-picker")
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
                Label::new(title)
                    .text_size(px(14.))
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(palette_fg),
            )
            .child(Input::new(&self.query))
            .child(
                div()
                    .id("picker-list")
                    .flex_1()
                    .w_full()
                    .min_h(px(220.))
                    .max_h(px(360.))
                    .overflow_y_scroll()
                    .track_scroll(&self.scroll)
                    .v_flex()
                    .gap_1()
                    .children(matches.into_iter().enumerate().map(|(ix, item)| {
                        let is_sel = ix == selected;
                        let row_bg = if is_sel { palette_fg } else { palette_bg };
                        let row_fg = if is_sel { palette_bg } else { palette_fg };
                        let doc_fg = if is_sel {
                            palette_bg
                        } else {
                            cx.theme().muted_foreground
                        };
                        let id = item.id.clone();
                        div()
                            .id(("pick", ix as u64))
                            .w_full()
                            .px_3()
                            .py_2()
                            .rounded_md()
                            .bg(row_bg)
                            .cursor_pointer()
                            .on_click(cx.listener(move |this, _, _, cx| {
                                if !this.armed {
                                    return;
                                }
                                this.armed = false;
                                cx.emit(PickerEvent::Confirmed(id.clone()));
                            }))
                            .child(
                                div()
                                    .w_full()
                                    .v_flex()
                                    .gap_1()
                                    .child(
                                        Label::new(item.title.clone())
                                            .text_size(px(13.))
                                            .text_color(row_fg),
                                    )
                                    .when(!item.subtitle.is_empty(), |this| {
                                        this.child(
                                            Label::new(item.subtitle.clone())
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

pub fn char_select_items() -> Vec<PickerItem> {
    let mut items = Vec::new();
    let named: &[(&str, char)] = &[
        ("Space", ' '),
        ("No-break space", '\u{00A0}'),
        ("Em dash", '—'),
        ("En dash", '–'),
        ("Ellipsis", '…'),
        ("Left arrow", '←'),
        ("Up arrow", '↑'),
        ("Right arrow", '→'),
        ("Down arrow", '↓'),
        ("Left-right arrow", '↔'),
        ("Up-down arrow", '↕'),
        ("Check mark", '✓'),
        ("Ballot X", '✗'),
        ("Star", '★'),
        ("Heart", '♥'),
        ("Diamond", '♦'),
        ("Club", '♣'),
        ("Spade", '♠'),
        ("Degree", '°'),
        ("Plus-minus", '±'),
        ("Multiply", '×'),
        ("Divide", '÷'),
        ("Not equal", '≠'),
        ("Less-or-equal", '≤'),
        ("Greater-or-equal", '≥'),
        ("Infinity", '∞'),
        ("Pi", 'π'),
        ("Micro", 'µ'),
        ("Section", '§'),
        ("Copyright", '©'),
        ("Registered", '®'),
        ("Trademark", '™'),
        ("Bullet", '•'),
        ("Middle dot", '·'),
        ("Euro", '€'),
        ("Pound", '£'),
        ("Yen", '¥'),
        ("Smiling face", '☺'),
        ("Black smiling face", '☻'),
        ("Sun", '☀'),
        ("Cloud", '☁'),
        ("Umbrella", '☂'),
        ("Snowman", '☃'),
        ("Comet", '☄'),
        ("Hot beverage", '☕'),
        ("Shamrock", '☘'),
        ("Pointing index", '☞'),
        ("Peace", '☮'),
        ("Yin yang", '☯'),
        ("Smile emoji", '😀'),
        ("Grin emoji", '😁'),
        ("Joy emoji", '😂'),
        ("Heart eyes", '😍'),
        ("Thumbs up", '👍'),
        ("Fire", '🔥'),
        ("Rocket", '🚀'),
        ("Check emoji", '✅'),
        ("Cross mark emoji", '❌'),
        ("Warning", '⚠'),
        ("Light bulb", '💡'),
        ("Folder", '📁'),
        ("Memo", '📝'),
        ("Laptop", '💻'),
        ("Terminal emoji", '🖥'),
    ];
    for (name, ch) in named {
        items.push(PickerItem {
            id: format!("char:{ch}"),
            title: format!("{ch}  {name}"),
            subtitle: format!("U+{:04X}", *ch as u32),
        });
    }
    for cp in 0x2500u32..=0x259F {
        if let Some(ch) = char::from_u32(cp) {
            items.push(PickerItem {
                id: format!("char:{ch}"),
                title: format!("{ch}  U+{cp:04X}"),
                subtitle: "Box drawing / block".into(),
            });
        }
    }
    items
}
