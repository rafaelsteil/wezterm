//! Sibling-window app chrome. Shells are mux `LocalPane`; paint prefers wezterm-font.

use gpui::prelude::FluentBuilder;
use gpui::*;
use gpui_component::{
    ActiveTheme, IconName, Root, Sizable, TitleBar, WindowExt,
    button::*,
    label::Label,
    menu::PopupMenuItem,
    notification::Notification,
    tab::{Tab, TabBar},
};

use crate::confirm::{open_confirm, open_line_prompt};
use crate::lua_ui::{
    active_after_close, format_tab_title, show_tab_bar, wants_quit_prompt, wants_tab_close_prompt,
};
use crate::palette::{CommandPalette, PaletteEvent};
use crate::shells::ShellProfile;
use crate::term_pane::{TermPane, TermPaneEvent};

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
        ToggleFps,
        CopySelection,
        PasteClipboard,
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
        KeyBinding::new("ctrl-shift-f", ToggleFps, None),
        KeyBinding::new("ctrl-shift-c", CopySelection, Some(APP_CONTEXT)),
        KeyBinding::new("ctrl-shift-v", PasteClipboard, Some(APP_CONTEXT)),
        KeyBinding::new("ctrl-insert", CopySelection, Some(APP_CONTEXT)),
        KeyBinding::new("shift-insert", PasteClipboard, Some(APP_CONTEXT)),
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
    /// Previous `active` index, for `switch_to_last_active_tab_when_closing_tab`.
    last_active: Option<usize>,
    font_px: f32,
    palette: Entity<CommandPalette>,
    palette_open: bool,
    /// gpui-fps HUD. Off by default (019). Ctrl+Shift+F toggles.
    /// While visible the stock monitor is continuous (sustain FPS).
    show_fps: bool,
    /// Root wraps us after `new`; focus once on first paint so keys work
    /// without a right-click.
    focus_pending: bool,
    /// Plus / Ctrl+T uses `shells[0]`; the chevron lists all of them.
    shells: Vec<ShellProfile>,
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
                PaletteEvent::Executed(id) => {
                    this.palette_open = false;
                    this.apply_command(id, window, cx);
                    if !this.palette_open && !window.has_active_dialog(cx) {
                        this.focus_terminal(window, cx);
                    }
                }
                PaletteEvent::Dismissed => {
                    this.palette_open = false;
                    window.focus(&this.focus_handle, cx);
                }
            }
            cx.notify();
        })
        .detach();

        let font_px = crate::mux_host::config_font_size();
        let focus_handle = cx.focus_handle();
        let shells = crate::shells::available_shells();
        window.focus(&focus_handle, cx);
        window.activate_window();
        let default = shells.first().cloned().unwrap_or_else(crate::shells::default_shell);
        let first = Self::new_tab(font_px, focus_handle.clone(), &default, cx);
        let first_term = first.term.clone();
        let mut this = Self {
            focus_handle: focus_handle.clone(),
            tabs: vec![first],
            active: 0,
            last_active: None,
            font_px,
            palette,
            palette_open: false,
            show_fps: false,
            focus_pending: true,
            shells,
        };
        this.watch_pane(first_term, cx);
        this
    }

    pub fn focus_terminal(&self, window: &mut Window, cx: &mut Context<Self>) {
        window.focus(&self.focus_handle, cx);
        window.activate_window();
    }

    /// AppShell keys only work while this handle is focused. Tab X / dialog
    /// restore a Close-button handle (still alive on Cancel, reused id on OK);
    /// Plus can steal focus after we set it. Immediate focus when no dialog,
    /// then a delayed retry with the window (beats gpui-component's 250ms
    /// restore). 032 only retried after confirm-close; Cancel / Ctrl+Q left
    /// typing dead (037).
    fn request_terminal_focus(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.focus_pending = true;
        if !self.palette_open && !window.has_active_dialog(cx) {
            self.focus_terminal(window, cx);
        }
        cx.spawn_in(window, async move |this, cx| {
            cx.background_executor()
                .timer(std::time::Duration::from_millis(400))
                .await;
            this.update_in(cx, |this, window, cx| {
                if this.palette_open {
                    return;
                }
                this.focus_pending = true;
                if !window.has_active_dialog(cx) {
                    this.focus_terminal(window, cx);
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    /// Capture AppShell as the dialog's previous-focus target, then restore
    /// after OK **or** Cancel (gpui-component otherwise restores the tab X).
    fn dialog_restore(shell: Entity<Self>) -> impl Fn(&mut Window, &mut App) + 'static {
        move |window, cx| {
            shell.update(cx, |this, cx| {
                this.request_terminal_focus(window, cx);
            });
        }
    }

    fn apply_command(&mut self, id: &str, window: &mut Window, cx: &mut Context<Self>) {
        match id {
            "SpawnTab.CurrentPaneDomain" => {
                self.add_tab(cx);
                self.request_terminal_focus(window, cx);
            }
            "CloseCurrentTab.confirm" | "CloseCurrentPane.confirm" => {
                self.confirm_close_active(window, cx)
            }
            "QuitApplication" => self.confirm_quit(window, cx),
            "IncreaseFontSize" => self.bump_font(1., cx),
            "DecreaseFontSize" => self.bump_font(-1., cx),
            "ResetFontSize" => self.set_font(crate::mux_host::config_font_size(), cx),
            "ClearScrollback.ScrollbackOnly" => {
                if let Some(tab) = self.tabs.get(self.active) {
                    tab.term.update(cx, |term, cx| {
                        term.clear_scrollback();
                        cx.notify();
                    });
                }
            }
            "CopyTo.Clipboard" => {
                self.copy_selection(window, cx, true);
            }
            "PasteFrom.Clipboard" => {
                self.paste_clipboard(window, cx, true);
            }
            "ActivateCommandPalette" => self.palette_open = true,
            "RenameTab" | "PromptInputLine" => {
                self.open_rename_prompt(window, cx);
            }
            "Confirmation" => self.open_demo_confirm(window, cx),
            _ => {}
        }
    }

    fn default_profile(&self) -> ShellProfile {
        self.shells
            .first()
            .cloned()
            .unwrap_or_else(crate::shells::default_shell)
    }

    fn new_tab(
        font_px: f32,
        shell_focus: FocusHandle,
        profile: &ShellProfile,
        cx: &mut Context<Self>,
    ) -> ShellTab {
        let term = cx.new(|cx| TermPane::spawn(font_px, shell_focus, profile, cx));
        ShellTab {
            title_override: None,
            term,
        }
    }

    fn add_tab(&mut self, cx: &mut Context<Self>) {
        let profile = self.default_profile();
        self.add_tab_profile(&profile, cx);
    }

    fn add_tab_profile(&mut self, profile: &ShellProfile, cx: &mut Context<Self>) {
        let tab = Self::new_tab(
            self.font_px,
            self.focus_handle.clone(),
            profile,
            cx,
        );
        let term = tab.term.clone();
        if !self.tabs.is_empty() {
            self.last_active = Some(self.active);
        }
        self.tabs.push(tab);
        self.active = self.tabs.len() - 1;
        self.watch_pane(term, cx);
    }

    fn activate_tab(&mut self, index: usize) {
        if index >= self.tabs.len() || index == self.active {
            return;
        }
        self.last_active = Some(self.active);
        self.active = index;
    }

    fn watch_pane(&mut self, term: Entity<TermPane>, cx: &mut Context<Self>) {
        cx.subscribe(&term, |this, pane, event, cx| {
            match event {
                TermPaneEvent::Exited => {
                    if let Some(index) = this.tabs.iter().position(|t| t.term == pane) {
                        this.dismiss_exited_tab(index, cx);
                    }
                }
            }
        })
        .detach();
    }

    /// Process already gone (`exit`). No confirm. Last tab → quit the app,
    /// same as wezterm-gui `exit_behavior = Close` + `quit_when_all_windows_are_closed`.
    fn dismiss_exited_tab(&mut self, index: usize, cx: &mut Context<Self>) {
        if index >= self.tabs.len() {
            return;
        }
        if self.remove_tab_at(index) {
            cx.quit();
            return;
        }
        self.focus_pending = true;
        cx.notify();
    }

    fn spawn_profile(&mut self, profile: &ShellProfile, window: &mut Window, cx: &mut Context<Self>) {
        self.add_tab_profile(profile, cx);
        self.request_terminal_focus(window, cx);
        cx.notify();
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
        let _ = self.remove_tab_at(index);
    }

    /// Remove `index`. `true` if that was the last tab (caller should quit).
    fn remove_tab_at(&mut self, index: usize) -> bool {
        if index >= self.tabs.len() {
            return self.tabs.is_empty();
        }
        if self.tabs.len() <= 1 {
            self.tabs.clear();
            self.active = 0;
            self.last_active = None;
            return true;
        }
        let switch = config::configuration().switch_to_last_active_tab_when_closing_tab;
        let new_active = active_after_close(
            self.active,
            self.last_active,
            index,
            self.tabs.len(),
            switch,
        );
        let prev_last = self.last_active;
        self.tabs.remove(index);
        self.active = new_active;
        self.last_active = prev_last.and_then(|i| {
            if i == index {
                return None;
            }
            let adj = if i > index { i - 1 } else { i };
            if adj == self.active {
                None
            } else {
                Some(adj)
            }
        });
        false
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
        let skip = self.tabs[index]
            .term
            .read(cx)
            .can_close_without_prompting(mux::pane::CloseReason::Tab);
        if !wants_tab_close_prompt(skip) {
            self.close_tab_at(index);
            self.request_terminal_focus(window, cx);
            cx.notify();
            return;
        }
        let title = self.tabs[index].title(cx);
        // So the dialog's previous-focus restore is AppShell, not the tab X.
        self.focus_terminal(window, cx);
        let shell = cx.entity();
        let restore = Self::dialog_restore(shell.clone());
        open_confirm(
            window,
            cx,
            "Close tab?",
            format!("🛑 Really kill tab `{title}` and all contained panes?"),
            "Close",
            true,
            move |window, cx| {
                shell.update(cx, |this, cx| {
                    this.close_tab_at(index);
                    this.request_terminal_focus(window, cx);
                    cx.notify();
                });
            },
            restore,
        );
    }

    fn confirm_quit(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let policy = config::configuration().window_close_confirmation;
        let all_skip = self.tabs.iter().all(|t| {
            t.term
                .read(cx)
                .can_close_without_prompting(mux::pane::CloseReason::Window)
        });
        if !wants_quit_prompt(policy, all_skip) {
            cx.quit();
            return;
        }
        self.focus_terminal(window, cx);
        let restore = Self::dialog_restore(cx.entity());
        open_confirm(
            window,
            cx,
            "Quit WezTerm?",
            "🛑 Really Quit WezTerm?",
            "Quit",
            true,
            |_, cx| cx.quit(),
            restore,
        );
    }

    fn open_rename_prompt(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let current = self
            .tabs
            .get(self.active)
            .map(|t| t.title(cx))
            .unwrap_or_default();
        self.focus_terminal(window, cx);
        let shell = cx.entity();
        let restore = Self::dialog_restore(shell.clone());
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
            restore,
        );
    }

    fn open_demo_confirm(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.focus_terminal(window, cx);
        let restore = Self::dialog_restore(cx.entity());
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
            restore,
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

    fn toggle_palette(
        &mut self,
        _: &ToggleCommandPalette,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
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

    fn on_palette_confirm(
        &mut self,
        _: &PaletteConfirm,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.palette.update(cx, |p, cx| p.confirm(window, cx));
    }

    fn on_new_tab(&mut self, _: &NewTab, window: &mut Window, cx: &mut Context<Self>) {
        self.add_tab(cx);
        self.request_terminal_focus(window, cx);
        cx.notify();
    }

    fn on_close_tab(&mut self, _: &CloseTab, window: &mut Window, cx: &mut Context<Self>) {
        cx.stop_propagation();
        self.confirm_close_active(window, cx);
    }

    fn on_quit(&mut self, _: &QuitPoc, window: &mut Window, cx: &mut Context<Self>) {
        cx.stop_propagation();
        self.confirm_quit(window, cx);
    }

    fn toggle_fps(&mut self, _: &ToggleFps, _: &mut Window, cx: &mut Context<Self>) {
        self.show_fps = !self.show_fps;
        cx.notify();
    }

    fn copy_selection(&mut self, window: &mut Window, cx: &mut Context<Self>, notify: bool) {
        let copied = self
            .tabs
            .get(self.active)
            .map(|tab| tab.term.update(cx, |term, cx| term.copy_selection(cx)))
            .unwrap_or(false);
        if notify {
            if copied {
                window.push_notification(Notification::info("Copied to clipboard"), cx);
            } else {
                window.push_notification(Notification::info("Nothing selected"), cx);
            }
        }
    }

    fn paste_clipboard(&mut self, window: &mut Window, cx: &mut Context<Self>, notify: bool) {
        let pasted = self
            .tabs
            .get(self.active)
            .map(|tab| {
                tab.term.update(cx, |term, cx| {
                    let ok = term.paste_clipboard(cx);
                    if ok {
                        cx.notify();
                    }
                    ok
                })
            })
            .unwrap_or(false);
        if notify && !pasted {
            window.push_notification(Notification::info("Clipboard is empty"), cx);
        }
    }

    fn on_copy(&mut self, _: &CopySelection, window: &mut Window, cx: &mut Context<Self>) {
        if self.palette_open || window.has_active_dialog(cx) {
            return;
        }
        self.copy_selection(window, cx, false);
    }

    fn on_paste(&mut self, _: &PasteClipboard, window: &mut Window, cx: &mut Context<Self>) {
        if self.palette_open || window.has_active_dialog(cx) {
            return;
        }
        self.paste_clipboard(window, cx, false);
    }

    fn on_term_key(&mut self, event: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        if self.palette_open || window.has_active_dialog(cx) {
            return;
        }
        if let Some(tab) = self.tabs.get(self.active) {
            tab.term.update(cx, |term, cx| {
                if term.key_down(event, cx) {
                    cx.stop_propagation();
                    cx.notify();
                }
            });
        }
    }
}

impl Render for AppShell {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if self.focus_pending && !self.palette_open && !window.has_active_dialog(cx) {
            self.focus_terminal(window, cx);
            self.focus_pending = false;
        }
        let active = self.active.min(self.tabs.len().saturating_sub(1));
        let term = self.tabs.get(active).map(|t| t.term.clone());
        let cfg = config::configuration();
        let tab_titles: Vec<(usize, String)> = self
            .tabs
            .iter()
            .enumerate()
            .map(|(i, t)| {
                (
                    i,
                    format_tab_title(
                        i,
                        &t.title(cx),
                        cfg.show_tab_index_in_tab_bar,
                        cfg.tab_and_split_indices_are_zero_based,
                        cfg.tab_max_width,
                    ),
                )
            })
            .collect();
        let show_tabs = show_tab_bar(
            self.tabs.len(),
            cfg.enable_tab_bar,
            cfg.hide_tab_bar_if_only_one_tab,
        );
        let palette_open = self.palette_open;
        let status_title = self
            .tabs
            .get(active)
            .map(|t| t.title(cx))
            .unwrap_or_else(|| "—".into());
        let status_line = self
            .tabs
            .get(active)
            .map(|t| t.term.read(cx).status_line())
            .unwrap_or_else(|| "no pane".into());
        // Root::render does not paint these. Without them, Ctrl+Q / tab X
        // push an AlertDialog that steals focus but never appears (038).
        let sheet_layer = Root::render_sheet_layer(window, cx);
        let dialog_layer = Root::render_dialog_layer(window, cx);
        let notification_layer = Root::render_notification_layer(window, cx);

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
            .on_action(cx.listener(Self::toggle_fps))
            .on_action(cx.listener(Self::on_copy))
            .on_action(cx.listener(Self::on_paste))
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
                            Label::new("POC chrome — mux LocalPane + wezterm-font paint")
                                .text_size(px(12.))
                                .text_color(cx.theme().muted_foreground),
                        ),
                ),
            )
            .when(show_tabs, |this| {
                this.child(
                    TabBar::new("tabs")
                        .selected_index(active)
                        .on_click(cx.listener(|this, index, window, cx| {
                            this.activate_tab(*index);
                            this.request_terminal_focus(window, cx);
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
                        .suffix({
                            let shells = self.shells.clone();
                            let plus_tip = format!(
                                "New tab — {} (Ctrl+T)",
                                shells
                                    .first()
                                    .map(|s| s.label.as_str())
                                    .unwrap_or("Command Prompt")
                            );
                            let view = cx.entity();
                            DropdownButton::new("new-tab")
                                .ghost()
                                .xsmall()
                                .button(
                                    Button::new("new-tab-plus")
                                        .icon(IconName::Plus)
                                        .ghost()
                                        .xsmall()
                                        .tooltip(plus_tip)
                                        .on_click(cx.listener(|this, _, window, cx| {
                                            this.add_tab(cx);
                                            this.request_terminal_focus(window, cx);
                                            cx.notify();
                                        })),
                                )
                                .dropdown_menu_with_anchor(Anchor::BottomLeft, move |menu, _, _| {
                                    let mut menu = menu;
                                    for profile in &shells {
                                        let profile = profile.clone();
                                        let view = view.clone();
                                        menu = menu.item(
                                            PopupMenuItem::new(profile.label.clone()).on_click(
                                                move |_, window, cx| {
                                                    view.update(cx, |this, cx| {
                                                        this.spawn_profile(&profile, window, cx);
                                                    });
                                                },
                                            ),
                                        );
                                    }
                                    menu
                                })
                        }),
                )
            })
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
                            "Ctrl+Shift+P palette  ·  Ctrl+Shift+C/V copy/paste  ·  Ctrl+Shift+F fps  ·  {}  ·  {}  ·  {}  ·  mux LocalDomain",
                            crate::mux_host::config_status(),
                            status_title,
                            status_line
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
            .children(sheet_layer)
            .children(dialog_layer)
            .children(notification_layer)
            .when(self.show_fps, |this| {
                this.child(gpui_fps::fps_monitor(window, cx))
            })
    }
}
