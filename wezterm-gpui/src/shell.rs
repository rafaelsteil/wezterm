//! Sibling-window app chrome. Shells are mux `LocalPane`; paint prefers wezterm-font.

use std::collections::HashMap;
use std::ffi::OsString;

use gpui::prelude::FluentBuilder;
use gpui::*;
use gpui_component::{
    ActiveTheme, IconName, ResizableState, Root, Sizable, TitleBar, WindowExt, h_resizable,
    v_resizable,
    button::*,
    label::Label,
    menu::PopupMenuItem,
    notification::Notification,
    tab::{Tab, TabBar},
};
use portable_pty::CommandBuilder;

use crate::confirm::{open_confirm, open_line_prompt};
use crate::lua_ui::{
    active_after_close, format_tab_title, remap_last_after_move, show_tab_bar,
    tab_index_from_assignment, tab_index_move_relative, tab_index_relative, wants_quit_prompt,
    wants_tab_close_prompt,
};
use crate::palette::{CommandPalette, PaletteEvent};
use crate::picker::{char_select_items, Picker, PickerEvent, PickerItem};
use crate::shells::ShellProfile;
use crate::split_layout::{LayoutNode, PaneDir, SplitAxis, SplitLayout};
use crate::term_pane::{TermPane, TermPaneEvent};
use crate::win_zorder::WindowZOrder;

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
    layout: SplitLayout<Entity<TermPane>>,
    /// One gpui-component divider state per split node (AdjustPaneSize).
    split_states: HashMap<u64, Entity<ResizableState>>,
}

impl ShellTab {
    fn title(&self, cx: &App) -> String {
        self.title_override.clone().unwrap_or_else(|| {
            self.layout
                .active_pane()
                .map(|term| term.read(cx).title())
                .unwrap_or_else(|| "—".into())
        })
    }

    fn can_close_without_prompting(&self, reason: mux::pane::CloseReason, cx: &App) -> bool {
        self.layout
            .panes()
            .iter()
            .all(|term| term.read(cx).can_close_without_prompting(reason))
    }

    fn retain_split_states(&mut self) {
        let mut ids = Vec::new();
        self.layout.root().collect_split_ids(&mut ids);
        self.split_states.retain(|id, _| ids.contains(id));
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum PickerKind {
    Launcher,
    TabNav,
    PaneSelectActivate,
    PaneSelectSwap,
    PaneSelectSwapKeep,
    PaneSelectMoveTab,
    PaneSelectMoveWindow,
    CharSelect,
    Search,
    QuickSelect,
    Debug,
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
    picker: Entity<Picker>,
    picker_kind: Option<PickerKind>,
    /// gpui-fps HUD. Off by default (019). Ctrl+Shift+F toggles.
    /// While visible the stock monitor is continuous (sustain FPS).
    show_fps: bool,
    /// Root wraps us after `new`; focus once on first paint so keys work
    /// without a right-click.
    focus_pending: bool,
    /// Plus / Ctrl+T uses `shells[0]`; the chevron lists all of them.
    shells: Vec<ShellProfile>,
    /// Unique ElementId for each GPUI split group.
    next_split_id: u64,
    /// Launch content size; ResetFontAndWindowSize restores this (047).
    original_size: Size<Pixels>,
    window_level: WindowZOrder,
}

impl Focusable for AppShell {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl AppShell {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        Self::build(window, cx, None)
    }

    fn build(
        window: &mut Window,
        cx: &mut Context<Self>,
        initial: Option<Entity<TermPane>>,
    ) -> Self {
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

        let picker = cx.new(|cx| Picker::new(window, cx));
        cx.subscribe_in(&picker, window, |this, _, event, window, cx| {
            match event {
                PickerEvent::Confirmed(id) => {
                    let kind = this.picker_kind.take();
                    this.apply_picker(kind, id.clone(), window, cx);
                    if this.picker_kind.is_none() && !this.palette_open {
                        this.request_terminal_focus(window, cx);
                    }
                }
                PickerEvent::Dismissed => {
                    this.picker_kind = None;
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
        let first = if let Some(pane) = initial {
            ShellTab {
                title_override: None,
                layout: SplitLayout::leaf(pane),
                split_states: HashMap::new(),
            }
        } else {
            Self::new_tab(font_px, focus_handle.clone(), &default, cx)
        };
        let first_term = first.layout.active_pane().cloned();
        let mut this = Self {
            focus_handle: focus_handle.clone(),
            tabs: vec![first],
            active: 0,
            last_active: None,
            font_px,
            palette,
            palette_open: false,
            picker,
            picker_kind: None,
            show_fps: false,
            focus_pending: true,
            shells,
            next_split_id: 1,
            original_size: launch_content_size(window),
            window_level: WindowZOrder::Normal,
        };
        if let Some(term) = first_term {
            this.watch_pane(term, cx);
        }
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
        if !self.overlay_open() && !window.has_active_dialog(cx) {
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
            "SplitHorizontal" => self.split_active(SplitAxis::Horizontal, window, cx),
            "SplitVertical" => self.split_active(SplitAxis::Vertical, window, cx),
            "CloseCurrentTab.confirm" => self.confirm_close_active(window, cx),
            "CloseCurrentPane.confirm" => self.confirm_close_active_pane(window, cx),
            "QuitApplication" => self.confirm_quit(window, cx),
            "IncreaseFontSize" => self.bump_font(1., cx),
            "DecreaseFontSize" => self.bump_font(-1., cx),
            "ResetFontSize" => self.set_font(crate::mux_host::config_font_size(), cx),
            "ResetFontAndWindowSize" => self.reset_font_and_window_size(window, cx),
            "ToggleFullScreen" => window.toggle_fullscreen(),
            "ToggleAlwaysOnTop" => {
                self.set_window_level(self.window_level.toggle_top(), window);
            }
            "ToggleAlwaysOnBottom" => {
                self.set_window_level(self.window_level.toggle_bottom(), window);
            }
            "ClearScrollback.ScrollbackOnly" => {
                self.with_active_term(cx, |term, cx| {
                    term.clear_scrollback();
                    cx.notify();
                });
            }
            "ClearScrollback.ScrollbackAndViewport" => {
                self.with_active_term(cx, |term, cx| {
                    term.clear_scrollback_and_viewport();
                    cx.notify();
                });
            }
            "ResetTerminal" => {
                self.with_active_term(cx, |term, cx| {
                    term.reset_terminal();
                    cx.notify();
                });
            }
            "ScrollByPage.Up" => {
                self.with_active_term(cx, |term, cx| {
                    term.scroll_by_page(-1.0);
                    cx.notify();
                });
            }
            "ScrollByPage.Down" => {
                self.with_active_term(cx, |term, cx| {
                    term.scroll_by_page(1.0);
                    cx.notify();
                });
            }
            "ScrollToTop" => {
                self.with_active_term(cx, |term, cx| {
                    term.scroll_to_top();
                    cx.notify();
                });
            }
            "ScrollToBottom" => {
                self.with_active_term(cx, |term, cx| {
                    term.scroll_to_bottom();
                    cx.notify();
                });
            }
            "OpenLinkAtMouseCursor" => {
                self.with_active_term(cx, |term, _| {
                    term.open_link_at_mouse_cursor();
                });
            }
            "OpenUri.docs" => wezterm_open_url::open_url("https://wezterm.org/"),
            "OpenUri.discussions" => {
                wezterm_open_url::open_url("https://github.com/wezterm/wezterm/discussions/")
            }
            "OpenUri.issues" => {
                wezterm_open_url::open_url("https://github.com/wezterm/wezterm/issues/")
            }
            "Hide" => window.minimize_window(),
            "TogglePaneZoomState" => self.toggle_pane_zoom(cx),
            "ActivateLastTab" => {
                if let Some(i) = self.last_active {
                    self.activate_tab(i, cx);
                }
            }
            "CopyTo.Clipboard" => {
                self.copy_selection(window, cx, true);
            }
            "PasteFrom.Clipboard" => {
                self.paste_clipboard(window, cx, true);
            }
            "ActivateCommandPalette" => self.palette_open = true,
            "ReloadConfiguration" => self.reload_configuration(window, cx),
            "SpawnWindow" => self.spawn_window(cx),
            "DetachDomain.CurrentPaneDomain" => {
                window.push_notification(
                    Notification::info("The local domain cannot be detached"),
                    cx,
                );
            }
            "ShowLauncher" => self.open_picker(PickerKind::Launcher, window, cx),
            "PasteFrom.PrimarySelection" => self.paste_clipboard(window, cx, true),
            "CopyTo.PrimarySelection" => self.copy_selection(window, cx, true),
            "QuickSelect" => self.open_picker(PickerKind::QuickSelect, window, cx),
            "CharSelect" => self.open_picker(PickerKind::CharSelect, window, cx),
            "ActivateCopyMode" => self.enter_copy_mode(cx),
            "ClearKeyTableStack" => {
                window.push_notification(Notification::info("Key table stack is empty"), cx);
            }
            "Search" => self.open_picker(PickerKind::Search, window, cx),
            "ShowTabNavigator" => self.open_picker(PickerKind::TabNav, window, cx),
            "ShowDebugOverlay" => self.open_picker(PickerKind::Debug, window, cx),
            "PaneSelect.Activate" => {
                self.open_picker(PickerKind::PaneSelectActivate, window, cx)
            }
            "PaneSelect.SwapWithActive" => {
                self.open_picker(PickerKind::PaneSelectSwap, window, cx)
            }
            "PaneSelect.SwapWithActiveKeepFocus" => {
                self.open_picker(PickerKind::PaneSelectSwapKeep, window, cx)
            }
            "PaneSelect.MoveToNewTab" => {
                self.open_picker(PickerKind::PaneSelectMoveTab, window, cx)
            }
            "PaneSelect.MoveToNewWindow" => {
                self.open_picker(PickerKind::PaneSelectMoveWindow, window, cx)
            }
            "RenameTab" | "PromptInputLine" => {
                self.open_rename_prompt(window, cx);
            }
            "Confirmation" => self.open_demo_confirm(window, cx),
            id => {
                if let Some(n) = id.strip_prefix("ActivateTabRelative.") {
                    if let Ok(delta) = n.parse::<isize>() {
                        self.activate_tab_relative(delta, cx);
                    }
                } else if let Some(n) = id.strip_prefix("ActivateTab.") {
                    if let Ok(n) = n.parse::<isize>() {
                        if let Some(i) = tab_index_from_assignment(n, self.tabs.len()) {
                            self.activate_tab(i, cx);
                        }
                    }
                } else if let Some(n) = id.strip_prefix("MoveTabRelative.") {
                    if let Ok(delta) = n.parse::<isize>() {
                        self.move_tab_relative(delta, cx);
                    }
                } else if let Some(dir) = id.strip_prefix("ActivatePaneDirection.") {
                    if let Some(dir) = PaneDir::from_palette_suffix(dir) {
                        self.activate_pane_dir(dir, cx);
                    }
                } else if let Some(rot) = id.strip_prefix("RotatePanes.") {
                    if rot == "Clockwise" {
                        self.rotate_panes(true, cx);
                    } else if rot == "CounterClockwise" {
                        self.rotate_panes(false, cx);
                    }
                } else if let Some(n) = id.strip_prefix("ActivateWindowRelative.") {
                    if let Ok(delta) = n.parse::<isize>() {
                        self.activate_window_relative(delta, window, cx);
                    }
                } else if let Some(n) = id.strip_prefix("ActivateWindow.") {
                    if let Ok(n) = n.parse::<usize>() {
                        self.activate_window_at(n, window, cx);
                    }
                } else if let Some(dir) = id.strip_prefix("AdjustPaneSize.") {
                    if let Some(dir) = PaneDir::from_palette_suffix(dir) {
                        self.adjust_pane_size(dir, window, cx);
                    }
                } else if let Some(level) = WindowZOrder::from_palette_id(id) {
                    self.set_window_level(level, window);
                }
            }
        }
    }

    fn with_active_term(
        &mut self,
        cx: &mut Context<Self>,
        f: impl FnOnce(&mut TermPane, &mut Context<TermPane>),
    ) {
        let Some(term) = self
            .tabs
            .get(self.active)
            .and_then(|tab| tab.layout.active_pane())
            .cloned()
        else {
            return;
        };
        term.update(cx, f);
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
            layout: SplitLayout::leaf(term),
            split_states: HashMap::new(),
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
        let term = tab.layout.active_pane().cloned();
        if !self.tabs.is_empty() {
            self.last_active = Some(self.active);
        }
        self.tabs.push(tab);
        self.active = self.tabs.len() - 1;
        if let Some(term) = term {
            self.watch_pane(term, cx);
        }
        self.sync_pane_focus(cx);
    }

    fn activate_tab(&mut self, index: usize, cx: &mut Context<Self>) {
        if index >= self.tabs.len() || index == self.active {
            return;
        }
        self.last_active = Some(self.active);
        self.active = index;
        self.sync_pane_focus(cx);
    }

    fn activate_tab_relative(&mut self, delta: isize, cx: &mut Context<Self>) {
        if let Some(i) = tab_index_relative(self.active, delta, self.tabs.len()) {
            self.activate_tab(i, cx);
        }
    }

    fn move_tab_relative(&mut self, delta: isize, cx: &mut Context<Self>) {
        let n = self.tabs.len();
        let Some(dest) = tab_index_move_relative(self.active, delta, n) else {
            return;
        };
        if dest == self.active {
            return;
        }
        let from = self.active;
        let tab = self.tabs.remove(from);
        self.tabs.insert(dest, tab);
        self.last_active = remap_last_after_move(self.last_active, from, dest);
        self.active = dest;
        self.sync_pane_focus(cx);
    }

    fn sync_pane_focus(&self, cx: &mut Context<Self>) {
        let Some(tab) = self.tabs.get(self.active) else {
            return;
        };
        let active = tab.layout.active_index();
        for (i, pane) in tab.layout.panes().iter().enumerate() {
            let focused = i == active;
            pane.update(cx, |term, cx| {
                if term.set_focused(focused) {
                    cx.notify();
                }
            });
        }
    }

    fn watch_pane(&mut self, term: Entity<TermPane>, cx: &mut Context<Self>) {
        cx.subscribe(&term, |this, pane, event, cx| {
            match event {
                TermPaneEvent::Exited => {
                    if let Some(index) = this.tabs.iter().position(|t| t.layout.contains(&pane)) {
                        this.dismiss_exited_pane(index, pane, cx);
                    }
                }
                TermPaneEvent::Activated => {
                    for tab in &mut this.tabs {
                        if tab.layout.set_active_pane(&pane) {
                            this.sync_pane_focus(cx);
                            cx.notify();
                            break;
                        }
                    }
                }
            }
        })
        .detach();
    }

    /// Process already gone (`exit`). No confirm. Last pane of last tab → quit
    /// the app, same as wezterm-gui `exit_behavior = Close` +
    /// `quit_when_all_windows_are_closed`. A split sibling just goes away.
    fn dismiss_exited_pane(
        &mut self,
        tab_index: usize,
        pane: Entity<TermPane>,
        cx: &mut Context<Self>,
    ) {
        let Some(tab) = self.tabs.get_mut(tab_index) else {
            return;
        };
        let empty = tab.layout.remove_pane(&pane);
        tab.retain_split_states();
        if empty && self.remove_tab_at(tab_index) {
            cx.quit();
            return;
        }
        self.sync_pane_focus(cx);
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
            for term in tab.layout.panes() {
                term.update(cx, |term, cx| {
                    term.set_font_px(font_px);
                    cx.notify();
                });
            }
        }
    }

    fn reset_font_and_window_size(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if window.is_fullscreen() {
            window.toggle_fullscreen();
        }
        self.set_font(crate::mux_host::config_font_size(), cx);
        window.resize(self.original_size);
    }

    fn set_window_level(&mut self, level: WindowZOrder, window: &Window) {
        self.window_level = level;
        crate::win_zorder::apply(window, level);
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
        let skip = self.tabs[index].can_close_without_prompting(mux::pane::CloseReason::Tab, cx);
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

    fn split_active(&mut self, axis: SplitAxis, window: &mut Window, cx: &mut Context<Self>) {
        let Some(src) = self
            .tabs
            .get(self.active)
            .and_then(|tab| tab.layout.active_pane())
            .cloned()
        else {
            return;
        };
        let profile = src.read(cx).profile().clone();
        let term = cx.new(|cx| {
            TermPane::spawn(self.font_px, self.focus_handle.clone(), &profile, cx)
        });
        let id = self.next_split_id;
        self.next_split_id += 1;
        let state = cx.new(|_| ResizableState::default());
        if let Some(tab) = self.tabs.get_mut(self.active) {
            tab.layout.split(axis, term.clone(), id);
            tab.split_states.insert(id, state);
        }
        self.watch_pane(term, cx);
        self.sync_pane_focus(cx);
        self.request_terminal_focus(window, cx);
        cx.notify();
    }

    fn activate_pane_dir(&mut self, dir: PaneDir, cx: &mut Context<Self>) {
        {
            let Some(tab) = self.tabs.get_mut(self.active) else {
                return;
            };
            if tab.layout.is_zoomed() {
                if !config::configuration().unzoom_on_switch_pane {
                    return;
                }
                tab.layout.unzoom();
            }
            let _ = tab.layout.activate_direction(dir);
        }
        self.sync_pane_focus(cx);
        cx.notify();
    }

    fn rotate_panes(&mut self, clockwise: bool, cx: &mut Context<Self>) {
        if let Some(tab) = self.tabs.get_mut(self.active) {
            tab.layout.rotate(clockwise);
        }
        self.sync_pane_focus(cx);
        cx.notify();
    }

    fn toggle_pane_zoom(&mut self, cx: &mut Context<Self>) {
        if let Some(tab) = self.tabs.get_mut(self.active) {
            tab.layout.toggle_zoom();
        }
        self.sync_pane_focus(cx);
        cx.notify();
    }

    fn adjust_pane_size(&mut self, dir: PaneDir, window: &mut Window, cx: &mut Context<Self>) {
        let Some(tab) = self.tabs.get(self.active) else {
            return;
        };
        if tab.layout.is_zoomed() {
            return;
        }
        let Some(split_id) = tab.layout.ancestor_split(dir.split_axis()) else {
            return;
        };
        let Some(state) = tab.split_states.get(&split_id).cloned() else {
            return;
        };
        let Some(term) = tab.layout.active_pane().cloned() else {
            return;
        };
        let (cell_w, cell_h) = term.read(cx).cell_px();
        let step = match dir.split_axis() {
            SplitAxis::Horizontal => cell_w,
            SplitAxis::Vertical => cell_h,
        };
        let delta = px(step * dir.first_child_delta_sign());
        state.update(cx, |state, cx| {
            let Some(&current) = state.sizes().first() else {
                return;
            };
            state.resize_panel(0, current + delta, window, cx);
        });
        cx.notify();
    }

    fn overlay_open(&self) -> bool {
        self.palette_open || self.picker_kind.is_some()
    }

    fn close_overlay(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.palette_open {
            self.close_palette(window, cx);
        }
        if self.picker_kind.is_some() {
            self.close_picker(window, cx);
        }
    }

    fn close_picker(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.picker_kind.is_none() {
            return;
        }
        self.picker.update(cx, |p, cx| p.dismiss(cx));
        self.picker_kind = None;
        window.focus(&self.focus_handle, cx);
        cx.notify();
    }

    fn open_picker(&mut self, kind: PickerKind, window: &mut Window, cx: &mut Context<Self>) {
        self.palette_open = false;
        let items = self.picker_items(kind, cx);
        let title = match kind {
            PickerKind::Launcher => "Launcher",
            PickerKind::TabNav => "Tab Navigator",
            PickerKind::PaneSelectActivate => "Select pane",
            PickerKind::PaneSelectSwap => "Swap with pane",
            PickerKind::PaneSelectSwapKeep => "Swap with pane (keep focus)",
            PickerKind::PaneSelectMoveTab => "Move pane to new tab",
            PickerKind::PaneSelectMoveWindow => "Move pane to new window",
            PickerKind::CharSelect => "Character / Emoji",
            PickerKind::Search => "Search pane output",
            PickerKind::QuickSelect => "Quick Select",
            PickerKind::Debug => "Debug overlay",
        };
        self.picker_kind = Some(kind);
        self.picker.update(cx, |picker, cx| {
            picker.open(title, items, window, cx);
        });
        cx.notify();
    }

    fn picker_items(&self, kind: PickerKind, cx: &App) -> Vec<PickerItem> {
        match kind {
            PickerKind::Launcher => self.launcher_items(cx),
            PickerKind::TabNav => self
                .tabs
                .iter()
                .enumerate()
                .map(|(i, t)| PickerItem {
                    id: format!("tab:{i}"),
                    title: t.title(cx),
                    subtitle: format!("Tab {}", i + 1),
                })
                .collect(),
            PickerKind::PaneSelectActivate
            | PickerKind::PaneSelectSwap
            | PickerKind::PaneSelectSwapKeep
            | PickerKind::PaneSelectMoveTab
            | PickerKind::PaneSelectMoveWindow => self.pane_items(cx),
            PickerKind::CharSelect => char_select_items(),
            PickerKind::Search => {
                let q = ""; // filled by picker filter on titles; seed with recent lines
                self.search_seed_items(cx, q)
            }
            PickerKind::QuickSelect => self.quick_select_items(cx),
            PickerKind::Debug => {
                let dump = self
                    .tabs
                    .get(self.active)
                    .and_then(|t| t.layout.active_pane())
                    .map(|p| p.read(cx).debug_dump())
                    .unwrap_or_else(|| "no pane".into());
                vec![
                    PickerItem {
                        id: "debug:copy".into(),
                        title: "Copy debug dump to clipboard".into(),
                        subtitle: dump.lines().next().unwrap_or("").to_string(),
                    },
                    PickerItem {
                        id: "debug:dump".into(),
                        title: dump,
                        subtitle: crate::mux_host::config_status(),
                    },
                ]
            }
        }
    }

    fn launcher_items(&self, cx: &App) -> Vec<PickerItem> {
        let mut items = Vec::new();
        for (i, profile) in self.shells.iter().enumerate() {
            items.push(PickerItem {
                id: format!("launch:tab:{i}"),
                title: format!("New Tab — {}", profile.label),
                subtitle: "Spawn".into(),
            });
        }
        items.push(PickerItem {
            id: "launch:window".into(),
            title: "New Window".into(),
            subtitle: "Spawn".into(),
        });
        items.push(PickerItem {
            id: "launch:splith".into(),
            title: "Split Horizontally".into(),
            subtitle: "Current tab".into(),
        });
        items.push(PickerItem {
            id: "launch:splitv".into(),
            title: "Split Vertically".into(),
            subtitle: "Current tab".into(),
        });
        for (i, t) in self.tabs.iter().enumerate() {
            items.push(PickerItem {
                id: format!("tab:{i}"),
                title: format!("Activate tab: {}", t.title(cx)),
                subtitle: format!("Tab {}", i + 1),
            });
        }
        let cfg = config::configuration();
        for (i, cmd) in cfg.launch_menu.iter().enumerate() {
            let label = cmd
                .label_for_palette()
                .unwrap_or_else(|| format!("launch_menu #{i}"));
            items.push(PickerItem {
                id: format!("launch:menu:{i}"),
                title: format!("{label} (New Tab)"),
                subtitle: "lua launch_menu".into(),
            });
        }
        items
    }

    fn pane_items(&self, cx: &App) -> Vec<PickerItem> {
        let Some(tab) = self.tabs.get(self.active) else {
            return Vec::new();
        };
        let alphabet = "123456789abcdefghijklmnopqrstuvwxyz";
        tab.layout
            .panes()
            .iter()
            .enumerate()
            .map(|(i, pane)| {
                let letter = alphabet.chars().nth(i).unwrap_or('?');
                let active = if i == tab.layout.active_index() {
                    " (active)"
                } else {
                    ""
                };
                PickerItem {
                    id: format!("pane:{i}"),
                    title: format!("{letter}: {}{active}", pane.read(cx).title()),
                    subtitle: format!("Pane {}", i + 1),
                }
            })
            .collect()
    }

    fn search_seed_items(&self, cx: &App, _q: &str) -> Vec<PickerItem> {
        let Some(term) = self
            .tabs
            .get(self.active)
            .and_then(|t| t.layout.active_pane())
        else {
            return Vec::new();
        };
        let text = term.read(cx).visible_plain_text();
        text.lines()
            .enumerate()
            .filter(|(_, line)| !line.trim().is_empty())
            .take(200)
            .map(|(i, line)| PickerItem {
                id: format!("search:{i}:{line}"),
                title: line.to_string(),
                subtitle: format!("Visible line {}", i + 1),
            })
            .collect()
    }

    fn quick_select_items(&self, cx: &App) -> Vec<PickerItem> {
        let Some(term) = self
            .tabs
            .get(self.active)
            .and_then(|t| t.layout.active_pane())
        else {
            return Vec::new();
        };
        let text = term.read(cx).visible_plain_text();
        let mut seen = HashMap::<String, ()>::new();
        let mut items = Vec::new();
        for token in text.split_whitespace() {
            let t = token.trim_matches(|c: char| {
                matches!(c, '"' | '\'' | '`' | ',' | ';' | ')' | '(' | '[' | ']')
            });
            if t.len() < 4 || seen.contains_key(t) {
                continue;
            }
            seen.insert(t.to_string(), ());
            items.push(PickerItem {
                id: format!("qsel:{t}"),
                title: t.to_string(),
                subtitle: "Copy".into(),
            });
            if items.len() >= 80 {
                break;
            }
        }
        items
    }

    fn apply_picker(
        &mut self,
        kind: Option<PickerKind>,
        id: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match kind {
            Some(PickerKind::CharSelect) => {
                if let Some(ch) = id.strip_prefix("char:") {
                    self.with_active_term(cx, |term, cx| {
                        if term.paste_text(ch) {
                            cx.notify();
                        }
                    });
                }
            }
            Some(PickerKind::QuickSelect) => {
                if let Some(text) = id.strip_prefix("qsel:") {
                    cx.write_to_clipboard(ClipboardItem::new_string(text.to_string()));
                    window.push_notification(Notification::info("Copied"), cx);
                }
            }
            Some(PickerKind::Search) => {
                if let Some(rest) = id.strip_prefix("search:") {
                    let line = rest.splitn(2, ':').nth(1).unwrap_or(rest);
                    self.with_active_term(cx, |term, cx| {
                        let hits = term.search_hits(line, 1);
                        if let Some((row, _)) = hits.first() {
                            term.jump_to_row(*row);
                            cx.notify();
                        }
                    });
                }
            }
            Some(PickerKind::Debug) => {
                if id == "debug:copy" || id == "debug:dump" {
                    let dump = self
                        .tabs
                        .get(self.active)
                        .and_then(|t| t.layout.active_pane())
                        .map(|p| p.read(cx).debug_dump())
                        .unwrap_or_default();
                    cx.write_to_clipboard(ClipboardItem::new_string(dump));
                    window.push_notification(Notification::info("Copied debug dump"), cx);
                }
            }
            Some(PickerKind::TabNav) | Some(PickerKind::Launcher) => {
                self.apply_launcher_id(&id, window, cx);
            }
            Some(PickerKind::PaneSelectActivate) => self.pane_select_activate(&id, cx),
            Some(PickerKind::PaneSelectSwap) => self.pane_select_swap(&id, false, cx),
            Some(PickerKind::PaneSelectSwapKeep) => self.pane_select_swap(&id, true, cx),
            Some(PickerKind::PaneSelectMoveTab) => self.pane_select_move_tab(&id, window, cx),
            Some(PickerKind::PaneSelectMoveWindow) => {
                self.pane_select_move_window(&id, window, cx)
            }
            None => {}
        }
    }

    fn apply_launcher_id(&mut self, id: &str, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(n) = id.strip_prefix("tab:") {
            if let Ok(i) = n.parse::<usize>() {
                self.activate_tab(i, cx);
            }
            return;
        }
        if let Some(n) = id.strip_prefix("launch:tab:") {
            if let Ok(i) = n.parse::<usize>() {
                if let Some(profile) = self.shells.get(i).cloned() {
                    self.spawn_profile(&profile, window, cx);
                }
            }
            return;
        }
        if id == "launch:window" {
            self.spawn_window(cx);
            return;
        }
        if id == "launch:splith" {
            self.split_active(SplitAxis::Horizontal, window, cx);
            return;
        }
        if id == "launch:splitv" {
            self.split_active(SplitAxis::Vertical, window, cx);
            return;
        }
        if let Some(n) = id.strip_prefix("launch:menu:") {
            if let Ok(i) = n.parse::<usize>() {
                self.spawn_launch_menu_item(i, window, cx);
            }
        }
    }

    fn spawn_launch_menu_item(&mut self, index: usize, window: &mut Window, cx: &mut Context<Self>) {
        let cfg = config::configuration();
        let Some(cmd) = cfg.launch_menu.get(index).cloned() else {
            return;
        };
        let builder = spawn_command_builder(&cmd);
        let label = cmd
            .label_for_palette()
            .unwrap_or_else(|| "launch_menu".into());
        let profile = ShellProfile {
            id: "launch_menu",
            label,
            argv: builder_argv(&builder),
        };
        self.spawn_profile(&profile, window, cx);
    }

    fn pane_index(id: &str) -> Option<usize> {
        id.strip_prefix("pane:")?.parse().ok()
    }

    fn pane_select_activate(&mut self, id: &str, cx: &mut Context<Self>) {
        let Some(i) = Self::pane_index(id) else {
            return;
        };
        if let Some(tab) = self.tabs.get_mut(self.active) {
            if let Some(pane) = tab.layout.panes().get(i).cloned() {
                let _ = tab.layout.set_active_pane(&pane);
            }
        }
        self.sync_pane_focus(cx);
        cx.notify();
    }

    fn pane_select_swap(&mut self, id: &str, keep_focus: bool, cx: &mut Context<Self>) {
        let Some(i) = Self::pane_index(id) else {
            return;
        };
        if let Some(tab) = self.tabs.get_mut(self.active) {
            tab.layout.swap_active_with(i, keep_focus);
        }
        self.sync_pane_focus(cx);
        cx.notify();
    }

    fn pane_select_move_tab(&mut self, id: &str, window: &mut Window, cx: &mut Context<Self>) {
        let Some(i) = Self::pane_index(id) else {
            return;
        };
        let Some(tab) = self.tabs.get(self.active) else {
            return;
        };
        if tab.layout.pane_count() < 2 {
            window.push_notification(Notification::info("Need more than one pane"), cx);
            return;
        }
        let Some(pane) = tab.layout.panes().get(i).cloned() else {
            return;
        };
        let tab_index = self.active;
        let extracted = self.tabs[tab_index].layout.extract_pane(&pane);
        self.tabs[tab_index].retain_split_states();
        let Some((pane, empty)) = extracted else {
            return;
        };
        if empty {
            let _ = self.remove_tab_at(tab_index);
        }
        let tab = ShellTab {
            title_override: None,
            layout: SplitLayout::leaf(pane.clone()),
            split_states: HashMap::new(),
        };
        self.tabs.push(tab);
        self.last_active = Some(self.active.min(self.tabs.len().saturating_sub(1)));
        self.active = self.tabs.len() - 1;
        self.watch_pane(pane, cx);
        self.sync_pane_focus(cx);
        self.request_terminal_focus(window, cx);
        cx.notify();
    }

    fn pane_select_move_window(&mut self, id: &str, window: &mut Window, cx: &mut Context<Self>) {
        let Some(i) = Self::pane_index(id) else {
            return;
        };
        let Some(tab) = self.tabs.get(self.active) else {
            return;
        };
        if tab.layout.pane_count() < 2 && self.tabs.len() < 2 {
            self.spawn_window(cx);
            return;
        }
        let Some(pane) = tab.layout.panes().get(i).cloned() else {
            return;
        };
        let tab_index = self.active;
        let extracted = self.tabs[tab_index].layout.extract_pane(&pane);
        self.tabs[tab_index].retain_split_states();
        let Some((pane, empty)) = extracted else {
            self.spawn_window(cx);
            return;
        };
        if empty {
            let _ = self.remove_tab_at(tab_index);
        }
        let opts = app_window_options(cx);
        let _ = cx.open_window(opts, move |window, cx| {
            let view = cx.new(|cx| AppShell::build(window, cx, Some(pane.clone())));
            let root = cx.new(|cx| Root::new(view.clone(), window, cx).bg(cx.theme().background));
            view.update(cx, |shell, cx| shell.focus_terminal(window, cx));
            root
        });
        self.sync_pane_focus(cx);
        self.request_terminal_focus(window, cx);
        cx.notify();
    }

    fn spawn_window(&mut self, cx: &mut Context<Self>) {
        let opts = app_window_options(cx);
        let _ = cx.open_window(opts, |window, cx| {
            let view = cx.new(|cx| AppShell::new(window, cx));
            let root = cx.new(|cx| Root::new(view.clone(), window, cx).bg(cx.theme().background));
            view.update(cx, |shell, cx| shell.focus_terminal(window, cx));
            root
        });
    }

    fn activate_window_at(&mut self, n: usize, _window: &mut Window, cx: &mut Context<Self>) {
        if let Some(h) = cx.windows().get(n).copied() {
            let _ = h.update(cx, |_, w, _| {
                w.activate_window();
            });
        }
    }

    fn activate_window_relative(
        &mut self,
        delta: isize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let windows = cx.windows();
        let mine = window.window_handle();
        let Some(idx) = windows.iter().position(|h| *h == mine) else {
            return;
        };
        let n = windows.len() as isize;
        if n == 0 {
            return;
        }
        let next = (idx as isize + delta).rem_euclid(n) as usize;
        self.activate_window_at(next, window, cx);
    }

    fn enter_copy_mode(&mut self, cx: &mut Context<Self>) {
        self.with_active_term(cx, |term, cx| {
            term.enter_copy_mode();
            cx.notify();
        });
        cx.notify();
    }

    fn reload_configuration(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        config::reload();
        let font_px = crate::mux_host::config_font_size();
        self.font_px = font_px;
        for tab in &self.tabs {
            for term in tab.layout.panes() {
                term.update(cx, |term, cx| {
                    term.reload_from_config(font_px);
                    cx.notify();
                });
            }
        }
        window.push_notification(Notification::info("Reloaded configuration"), cx);
        cx.notify();
    }

    fn confirm_close_active_pane(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(tab) = self.tabs.get(self.active) else {
            return;
        };
        if tab.layout.pane_count() <= 1 {
            self.confirm_close_tab_at(self.active, window, cx);
            return;
        }
        let Some(pane) = tab.layout.active_pane().cloned() else {
            return;
        };
        let skip = pane
            .read(cx)
            .can_close_without_prompting(mux::pane::CloseReason::Pane);
        if !wants_tab_close_prompt(skip) {
            self.close_pane_in_tab(self.active, pane, window, cx);
            return;
        }
        self.focus_terminal(window, cx);
        let shell = cx.entity();
        let restore = Self::dialog_restore(shell.clone());
        let tab_index = self.active;
        open_confirm(
            window,
            cx,
            "Close pane?",
            "🛑 Really kill this pane?",
            "Close",
            true,
            move |window, cx| {
                shell.update(cx, |this, cx| {
                    this.close_pane_in_tab(tab_index, pane.clone(), window, cx);
                });
            },
            restore,
        );
    }

    fn close_pane_in_tab(
        &mut self,
        tab_index: usize,
        pane: Entity<TermPane>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(tab) = self.tabs.get_mut(tab_index) else {
            return;
        };
        let empty = tab.layout.remove_pane(&pane);
        tab.retain_split_states();
        if empty && self.remove_tab_at(tab_index) {
            cx.quit();
            return;
        }
        self.sync_pane_focus(cx);
        self.request_terminal_focus(window, cx);
        cx.notify();
    }

    fn confirm_quit(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let policy = config::configuration().window_close_confirmation;
        let all_skip = self
            .tabs
            .iter()
            .all(|t| t.can_close_without_prompting(mux::pane::CloseReason::Window, cx));
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
        self.picker_kind = None;
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
        if self.overlay_open() {
            self.close_overlay(window, cx);
        } else {
            self.open_palette(window, cx);
        }
    }

    fn on_close_palette(&mut self, _: &ClosePalette, window: &mut Window, cx: &mut Context<Self>) {
        if self.overlay_open() {
            self.close_overlay(window, cx);
        }
    }

    fn on_palette_up(&mut self, _: &PaletteMoveUp, _: &mut Window, cx: &mut Context<Self>) {
        if self.picker_kind.is_some() {
            self.picker.update(cx, |p, cx| p.move_sel(-1, cx));
        } else if self.palette_open {
            self.palette.update(cx, |p, cx| p.move_sel(-1, cx));
        }
    }

    fn on_palette_down(&mut self, _: &PaletteMoveDown, _: &mut Window, cx: &mut Context<Self>) {
        if self.picker_kind.is_some() {
            self.picker.update(cx, |p, cx| p.move_sel(1, cx));
        } else if self.palette_open {
            self.palette.update(cx, |p, cx| p.move_sel(1, cx));
        }
    }

    fn on_palette_confirm(
        &mut self,
        _: &PaletteConfirm,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.picker_kind.is_some() {
            self.picker.update(cx, |p, cx| p.confirm(cx));
        } else if self.palette_open {
            self.palette.update(cx, |p, cx| p.confirm(window, cx));
        }
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
            .and_then(|tab| tab.layout.active_pane())
            .map(|term| term.update(cx, |term, cx| term.copy_selection(cx)))
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
            .and_then(|tab| tab.layout.active_pane())
            .map(|term| {
                term.update(cx, |term, cx| {
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
        if self.overlay_open() || window.has_active_dialog(cx) {
            return;
        }
        self.copy_selection(window, cx, false);
    }

    fn on_paste(&mut self, _: &PasteClipboard, window: &mut Window, cx: &mut Context<Self>) {
        if self.overlay_open() || window.has_active_dialog(cx) {
            return;
        }
        self.paste_clipboard(window, cx, false);
    }

    fn on_term_key(&mut self, event: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        if self.overlay_open() || window.has_active_dialog(cx) {
            return;
        }
        if let Some(term) = self
            .tabs
            .get(self.active)
            .and_then(|tab| tab.layout.active_pane())
            .cloned()
        {
            term.update(cx, |term, cx| {
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
        if self.focus_pending && !self.overlay_open() && !window.has_active_dialog(cx) {
            self.focus_terminal(window, cx);
            self.focus_pending = false;
        }
        let active = self.active.min(self.tabs.len().saturating_sub(1));
        let pane_body = self.tabs.get(active).map(|t| {
            if t.layout.is_zoomed() {
                render_split_tree(
                    &LayoutNode::leaf(t.layout.active_index()),
                    t.layout.panes(),
                    &t.split_states,
                )
            } else {
                render_split_tree(t.layout.root(), t.layout.panes(), &t.split_states)
            }
        });
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
        let overlay_open = self.overlay_open();
        let palette_open = self.palette_open;
        let picker_open = self.picker_kind.is_some();
        let status_title = self
            .tabs
            .get(active)
            .map(|t| t.title(cx))
            .unwrap_or_else(|| "—".into());
        let status_line = self
            .tabs
            .get(active)
            .and_then(|t| t.layout.active_pane())
            .map(|term| term.read(cx).status_line())
            .unwrap_or_else(|| "no pane".into());
        // Root::render does not paint these. Without them, Ctrl+Q / tab X
        // push an AlertDialog that steals focus but never appears (038).
        let sheet_layer = Root::render_sheet_layer(window, cx);
        let dialog_layer = Root::render_dialog_layer(window, cx);
        let notification_layer = Root::render_notification_layer(window, cx);

        div()
            .id("app-shell")
            .track_focus(&self.focus_handle)
            .key_context(if overlay_open {
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
                            this.activate_tab(*index, cx);
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
                    .when_some(pane_body, |this, body| this.child(body)),
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
                this.child(overlay_scrim(
                    "palette-scrim",
                    cx.listener(|this, _, window, cx| {
                        this.close_palette(window, cx);
                    }),
                    self.palette.clone(),
                ))
            })
            .when(picker_open, |this| {
                this.child(overlay_scrim(
                    "picker-scrim",
                    cx.listener(|this, _, window, cx| {
                        this.close_picker(window, cx);
                    }),
                    self.picker.clone(),
                ))
            })
            .children(sheet_layer)
            .children(dialog_layer)
            .children(notification_layer)
            .when(self.show_fps, |this| {
                this.child(gpui_fps::fps_monitor(window, cx))
            })
    }
}

/// Content size at first AppShell paint. Matches `main.rs` windowed bounds
/// when the platform has not reported a size yet.
fn launch_content_size(window: &Window) -> Size<Pixels> {
    let measured = window.bounds().size;
    if measured.width > px(1.) && measured.height > px(1.) {
        measured
    } else {
        size(px(980.), px(640.))
    }
}

fn overlay_scrim(
    id: &'static str,
    on_scrim: impl Fn(&MouseDownEvent, &mut Window, &mut App) + 'static,
    child: impl IntoElement,
) -> impl IntoElement {
    div()
        .id(id)
        .absolute()
        .inset_0()
        .flex()
        .justify_center()
        .pt(px(72.))
        .bg(gpui::black().opacity(0.45))
        .on_mouse_down(MouseButton::Left, on_scrim)
        .child(
            div()
                .on_mouse_down(MouseButton::Left, |_, _, cx| {
                    cx.stop_propagation();
                })
                .child(child),
        )
}

fn app_window_options(cx: &App) -> WindowOptions {
    WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(Bounds::centered(
            None,
            size(px(980.), px(640.)),
            cx,
        ))),
        titlebar: Some(TitlebarOptions {
            title: Some("WezTerm GPUI".into()),
            ..TitleBar::title_bar_options()
        }),
        ..Default::default()
    }
}

fn spawn_command_builder(cmd: &config::keyassignment::SpawnCommand) -> CommandBuilder {
    let mut builder = match &cmd.args {
        Some(args) if !args.is_empty() => {
            CommandBuilder::from_argv(args.iter().map(OsString::from).collect())
        }
        _ => CommandBuilder::new_default_prog(),
    };
    if let Some(cwd) = &cmd.cwd {
        builder.cwd(cwd);
    }
    for (k, v) in &cmd.set_environment_variables {
        builder.env(k, v);
    }
    builder
}

fn builder_argv(builder: &CommandBuilder) -> Option<Vec<OsString>> {
    let argv = builder.get_argv();
    if argv.is_empty() {
        None
    } else {
        Some(argv.clone())
    }
}

fn render_split_tree(
    node: &LayoutNode,
    panes: &[Entity<TermPane>],
    states: &HashMap<u64, Entity<ResizableState>>,
) -> AnyElement {
    match node {
        LayoutNode::Leaf(i) => {
            let Some(pane) = panes.get(*i) else {
                return div().into_any_element();
            };
            div()
                .size_full()
                .min_h_0()
                .overflow_hidden()
                .child(pane.clone())
                .into_any_element()
        }
        LayoutNode::Split {
            axis,
            id,
            first,
            second,
        } => {
            let group = match axis {
                SplitAxis::Horizontal => h_resizable(("pane-split", *id)),
                SplitAxis::Vertical => v_resizable(("pane-split", *id)),
            };
            let group = if let Some(state) = states.get(id) {
                group.with_state(state)
            } else {
                group
            };
            group
                .child(render_split_tree(first, panes, states))
                .child(render_split_tree(second, panes, states))
                .into_any_element()
        }
    }
}
