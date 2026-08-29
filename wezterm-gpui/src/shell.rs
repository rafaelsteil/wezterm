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
use mux::Mux;
use mux::tab::TabId;
use mux::window::WindowId as MuxWindowId;
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
        OpenSearch,
        CopySelection,
        PasteClipboard,
        SendPtyTab,
        SendPtyShiftTab,
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
        KeyBinding::new("ctrl-shift-f", OpenSearch, None),
        KeyBinding::new("ctrl-shift-c", CopySelection, Some(APP_CONTEXT)),
        KeyBinding::new("ctrl-shift-v", PasteClipboard, Some(APP_CONTEXT)),
        KeyBinding::new("ctrl-insert", CopySelection, Some(APP_CONTEXT)),
        KeyBinding::new("shift-insert", PasteClipboard, Some(APP_CONTEXT)),
        // Deeper than gpui-component Root's `tab` → focus_next (054).
        KeyBinding::new("tab", SendPtyTab, Some(APP_CONTEXT)),
        KeyBinding::new("shift-tab", SendPtyShiftTab, Some(APP_CONTEXT)),
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
    /// Mux tab for the Domain::spawn leaf. Split siblings stay orphan panes (040).
    mux_tab: Option<TabId>,
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

#[derive(Clone, Copy, PartialEq, Eq)]
enum PaneSelectMode {
    Activate,
    Swap,
    SwapKeep,
    MoveTab,
    MoveWindow,
}

struct PaneSelectState {
    mode: PaneSelectMode,
    typed: String,
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
    /// gpui-fps HUD. Off by default (019). Palette ToggleFpsHud only (050).
    show_fps: bool,
    search_open: bool,
    search_query: String,
    search_case: bool,
    search_current: usize,
    pane_select: Option<PaneSelectState>,
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
    mux_window_id: MuxWindowId,
    /// Mux workspace tag for this HWND (not the client-wide active name).
    workspace: String,
    /// HWND hidden because this mux window is not in the active workspace.
    workspace_hidden: bool,
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
                    this.apply_command(&id, window, cx);
                    if !this.workspace_hidden
                        && !this.palette_open
                        && !window.has_active_dialog(cx)
                    {
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
                    if this.picker_kind.is_none()
                        && !this.palette_open
                        && !this.workspace_hidden
                    {
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
        let shells = crate::mux_host::launch_profiles();
        window.focus(&focus_handle, cx);
        window.activate_window();
        let default = shells.first().cloned().unwrap_or_else(crate::shells::default_shell);
        let mux_window_id = crate::mux_host::new_mux_window().unwrap_or_else(|err| {
            eprintln!("wezterm-gpui mux window: {err:#}");
            0
        });
        let workspace = crate::mux_host::workspace_of(mux_window_id)
            .unwrap_or_else(crate::mux_host::active_workspace);
        let first = if let Some(pane) = initial {
            ShellTab {
                title_override: None,
                layout: SplitLayout::leaf(pane),
                split_states: HashMap::new(),
                mux_tab: None,
            }
        } else {
            Self::new_tab(font_px, focus_handle.clone(), &default, mux_window_id, cx)
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
            search_open: false,
            search_query: String::new(),
            search_case: true,
            search_current: 0,
            pane_select: None,
            focus_pending: true,
            shells,
            next_split_id: 1,
            original_size: launch_content_size(window),
            window_level: WindowZOrder::Normal,
            mux_window_id,
            workspace,
            workspace_hidden: false,
        };
        if let Some(term) = first_term {
            this.watch_pane(term, window, cx);
        }
        // OS caption X / Alt+F4 posts WM_CLOSE. Default GPUI then
        // DestroyWindow without close_self_or_quit, so a hidden workspace
        // HWND stays and the process is a ghost. Intercept like Zed.
        let shell = cx.entity().downgrade();
        window.on_window_should_close(cx, move |window, cx| {
            shell
                .update(cx, |this, cx| {
                    this.confirm_close_window(window, cx);
                    false
                })
                .unwrap_or(true)
        });
        this
    }

    pub fn focus_terminal(&self, window: &mut Window, cx: &mut Context<Self>) {
        if self.workspace_hidden {
            return;
        }
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
        if let Some(name) = id.strip_prefix("domain:") {
            self.spawn_named_domain(name, window, cx);
            return;
        }
        if id == "workspace:create" {
            self.create_workspace(window, cx);
            return;
        }
        if let Some(name) = id.strip_prefix("workspace:switch:") {
            self.switch_workspace(name, window, cx);
            return;
        }
        if id == "workspace:relative:1" {
            self.switch_workspace_relative(1, window, cx);
            return;
        }
        if id == "workspace:relative:-1" {
            self.switch_workspace_relative(-1, window, cx);
            return;
        }
        match id {
            "SpawnTab.CurrentPaneDomain" => {
                self.add_tab(window, cx);
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
            "ActivateCommandPalette" => self.open_palette(window, cx),
            "ReloadConfiguration" => self.reload_configuration(window, cx),
            "SpawnWindow" => {
                self.spawn_window(window, cx);
            }
            "DetachDomain.CurrentPaneDomain" => {
                window.push_notification(
                    Notification::info("The local domain cannot be detached"),
                    cx,
                );
            }
            "ShowLauncher" => self.open_picker(PickerKind::Launcher, window, cx),
            "PasteFrom.PrimarySelection" => self.paste_clipboard(window, cx, true),
            "CopyTo.PrimarySelection" => self.copy_selection(window, cx, true),
            "QuickSelect" => self.open_quick_select(window, cx),
            "CharSelect" => self.open_picker(PickerKind::CharSelect, window, cx),
            "ActivateCopyMode" => self.enter_copy_mode(cx),
            "ClearKeyTableStack" => {
                window.push_notification(Notification::info("Key table stack is empty"), cx);
            }
            "Search" => self.open_search(window, cx),
            "ShowTabNavigator" => self.open_picker(PickerKind::TabNav, window, cx),
            "ShowDebugOverlay" => self.show_debug_overlay(window, cx),
            "PaneSelect.Activate" => self.open_pane_select(PaneSelectMode::Activate, window, cx),
            "PaneSelect.SwapWithActive" => {
                self.open_pane_select(PaneSelectMode::Swap, window, cx)
            }
            "PaneSelect.SwapWithActiveKeepFocus" => {
                self.open_pane_select(PaneSelectMode::SwapKeep, window, cx)
            }
            "PaneSelect.MoveToNewTab" => {
                self.open_pane_select(PaneSelectMode::MoveTab, window, cx)
            }
            "PaneSelect.MoveToNewWindow" => {
                self.open_pane_select(PaneSelectMode::MoveWindow, window, cx)
            }
            "ToggleFpsHud" => self.toggle_fps_hud(cx),
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
        mux_window: MuxWindowId,
        cx: &mut Context<Self>,
    ) -> ShellTab {
        match crate::mux_host::spawn_tab_in_window(
            mux_window,
            profile.domain.as_deref(),
            profile.command(),
        ) {
            Ok((tab_id, pane)) => {
                let term = cx.new(|cx| {
                    TermPane::from_pane(font_px, shell_focus, profile, pane, cx)
                });
                ShellTab {
                    title_override: None,
                    layout: SplitLayout::leaf(term),
                    split_states: HashMap::new(),
                    mux_tab: Some(tab_id),
                }
            }
            Err(err) => {
                eprintln!("wezterm-gpui Domain::spawn: {err:#}");
                let term = cx.new(|cx| TermPane::spawn(font_px, shell_focus, profile, cx));
                ShellTab {
                    title_override: None,
                    layout: SplitLayout::leaf(term),
                    split_states: HashMap::new(),
                    mux_tab: None,
                }
            }
        }
    }

    fn add_tab(&mut self, window: &Window, cx: &mut Context<Self>) {
        let profile = self.default_profile();
        self.add_tab_profile(&profile, window, cx);
    }

    fn add_tab_profile(
        &mut self,
        profile: &ShellProfile,
        window: &Window,
        cx: &mut Context<Self>,
    ) {
        let tab = Self::new_tab(
            self.font_px,
            self.focus_handle.clone(),
            profile,
            self.mux_window_id,
            cx,
        );
        let term = tab.layout.active_pane().cloned();
        if !self.tabs.is_empty() {
            self.last_active = Some(self.active);
        }
        self.tabs.push(tab);
        self.active = self.tabs.len() - 1;
        if let Some(term) = term {
            self.watch_pane(term, window, cx);
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

    fn watch_pane(&mut self, term: Entity<TermPane>, window: &Window, cx: &mut Context<Self>) {
        cx.subscribe_in(&term, window, |this, pane, event, window, cx| {
            match event {
                TermPaneEvent::Exited => {
                    if let Some(index) = this.tabs.iter().position(|t| t.layout.contains(&pane)) {
                        this.dismiss_exited_pane(index, pane.clone(), window, cx);
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

    /// Process already gone (`exit`). No confirm. Last pane of last tab closes
    /// this HWND; `cx.quit()` only when it was the last window (033 + 052).
    fn dismiss_exited_pane(
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
        if empty && self.remove_tab_at(tab_index, cx, true) {
            self.close_self_or_quit(window, cx);
            return;
        }
        self.sync_pane_focus(cx);
        self.focus_pending = true;
        cx.notify();
    }

    fn spawn_profile(&mut self, profile: &ShellProfile, window: &mut Window, cx: &mut Context<Self>) {
        self.add_tab_profile(profile, window, cx);
        self.request_terminal_focus(window, cx);
        cx.notify();
    }

    fn spawn_named_domain(&mut self, name: &str, window: &mut Window, cx: &mut Context<Self>) {
        self.spawn_profile(&ShellProfile::mux_domain(name), window, cx);
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

    fn close_tab_at(&mut self, index: usize, cx: &App) {
        if self.tabs.len() <= 1 || index >= self.tabs.len() {
            return;
        }
        let _ = self.remove_tab_at(index, cx, true);
    }

    fn release_tab_mux(tab: &ShellTab, cx: &App) {
        let Some(mux) = Mux::try_get() else {
            return;
        };
        if let Some(id) = tab.mux_tab {
            mux.remove_tab(id);
        }
        for pane in tab.layout.panes() {
            if let Some(pid) = pane.read(cx).pane_id() {
                mux.remove_pane(pid);
            }
        }
    }

    /// Remove `index`. `true` if that was the last tab (caller should quit).
    fn remove_tab_at(&mut self, index: usize, cx: &App, kill_mux: bool) -> bool {
        if index >= self.tabs.len() {
            return self.tabs.is_empty();
        }
        if kill_mux {
            if let Some(tab) = self.tabs.get(index) {
                Self::release_tab_mux(tab, cx);
            }
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
            self.confirm_close_window(window, cx);
            return;
        }
        let skip = self.tabs[index].can_close_without_prompting(mux::pane::CloseReason::Tab, cx);
        if !wants_tab_close_prompt(skip) {
            self.close_tab_at(index, cx);
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
                    this.close_tab_at(index, cx);
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
        self.watch_pane(term, window, cx);
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
        if self.pane_select.is_some() {
            self.pane_select = None;
        }
        self.with_active_term(cx, |term, _| term.exit_quick_select());
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
                subtitle: profile
                    .domain
                    .as_ref()
                    .map(|d| format!("domain `{d}`"))
                    .unwrap_or_else(|| "Spawn".into()),
            });
        }
        for profile in crate::mux_host::spawnable_domain_profiles() {
            if self.shells.iter().any(|s| s.domain.as_deref() == Some(profile.id.as_str())) {
                continue;
            }
            items.push(PickerItem {
                id: format!("domain:{}", profile.id),
                title: format!("New Tab (domain `{}`)", profile.id),
                subtitle: "Mux domain".into(),
            });
        }
        items.push(PickerItem {
            id: "launch:window".into(),
            title: "New Window".into(),
            subtitle: "Spawn".into(),
        });
        let current = self.workspace.clone();
        for name in crate::workspaces::known_names(cx) {
            if name == current {
                continue;
            }
            items.push(PickerItem {
                id: format!("workspace:switch:{name}"),
                title: format!("Switch to workspace: `{name}`"),
                subtitle: "Window | Workspace".into(),
            });
        }
        items.push(PickerItem {
            id: "workspace:create".into(),
            title: "Create new Workspace".into(),
            subtitle: format!("current is `{current}`"),
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
        if id.starts_with("workspace:") {
            self.apply_command(id, window, cx);
            return;
        }
        if let Some(name) = id.strip_prefix("domain:") {
            self.spawn_named_domain(name, window, cx);
            return;
        }
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
            self.spawn_window(window, cx);
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
        let domain = spawn_domain_name(&cmd);
        let argv = if domain.is_some()
            && cmd.args.as_ref().map(|a| a.is_empty()).unwrap_or(true)
        {
            None
        } else {
            builder_argv(&builder)
        };
        let profile = ShellProfile {
            id: "launch_menu".into(),
            label,
            argv,
            domain,
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
            let _ = self.remove_tab_at(tab_index, cx, false);
        }
        let tab = ShellTab {
            title_override: None,
            layout: SplitLayout::leaf(pane.clone()),
            split_states: HashMap::new(),
            mux_tab: None,
        };
        self.tabs.push(tab);
        self.last_active = Some(self.active.min(self.tabs.len().saturating_sub(1)));
        self.active = self.tabs.len() - 1;
        self.watch_pane(pane, window, cx);
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
            self.spawn_window(window, cx);
            return;
        }
        let Some(pane) = tab.layout.panes().get(i).cloned() else {
            return;
        };
        let tab_index = self.active;
        let extracted = self.tabs[tab_index].layout.extract_pane(&pane);
        self.tabs[tab_index].retain_split_states();
        let Some((pane, empty)) = extracted else {
            self.spawn_window(window, cx);
            return;
        };
        if empty {
            let _ = self.remove_tab_at(tab_index, cx, false);
        }
        let opts = app_window_options_offset(window, cx);
        if let Ok(handle) = cx.open_window(opts, move |window, cx| {
            let view = cx.new(|cx| AppShell::build(window, cx, Some(pane.clone())));
            let root = cx.new(|cx| Root::new(view.clone(), window, cx).bg(cx.theme().background));
            view.update(cx, |shell, cx| shell.focus_terminal(window, cx));
            root
        }) {
            focus_opened_window(handle, cx);
        }
        self.sync_pane_focus(cx);
        cx.notify();
    }

    fn spawn_window(&mut self, window: &mut Window, cx: &mut Context<Self>) -> bool {
        let opts = app_window_options_offset(window, cx);
        if let Ok(handle) = open_app_shell(opts, cx) {
            focus_opened_window(handle, cx);
            true
        } else {
            false
        }
    }

    fn create_workspace(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.workspace_hidden {
            return;
        }
        if !crate::workspaces::try_begin_spawn() {
            return;
        }
        let previous = self.workspace.clone();
        let name = crate::mux_host::generate_workspace_name();
        crate::mux_host::set_active_workspace(&name);
        self.hide_then_spawn_workspace_window(previous, window, cx);
    }

    fn hide_then_spawn_workspace_window(
        &mut self,
        previous: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.workspace_hidden = true;
        crate::win_zorder::set_hidden(window, true);
        crate::workspaces::hide_all(cx);
        let opts = app_window_options_offset(window, cx);
        cx.spawn(async move |this, cx| {
            let _spawn = crate::workspaces::SpawnGuard;
            let ok = cx.update(|cx| open_app_shell(opts, cx).is_ok());
            if !ok {
                this.update_in(cx, |this, window, cx| {
                    crate::mux_host::set_active_workspace(&previous);
                    this.workspace_hidden = false;
                    crate::win_zorder::set_hidden(window, false);
                    crate::workspaces::show_workspace(&previous, cx);
                })
                .ok();
            }
        })
        .detach();
    }

    fn switch_workspace(&mut self, name: &str, window: &mut Window, cx: &mut Context<Self>) {
        if self.workspace_hidden && self.workspace != name {
            return;
        }
        crate::mux_host::set_active_workspace(name);
        if crate::workspaces::has_workspace(name, cx) {
            self.workspace_hidden = self.workspace != name;
            crate::workspaces::show_workspace(name, cx);
        } else if self.workspace_hidden || !crate::workspaces::try_begin_spawn() {
            return;
        } else {
            self.hide_then_spawn_workspace_window(self.workspace.clone(), window, cx);
        }
    }

    fn switch_workspace_relative(
        &mut self,
        delta: isize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let names = crate::workspaces::known_names(cx);
        if names.len() < 2 {
            return;
        }
        let current = self.workspace.clone();
        let idx = names.iter().position(|w| *w == current).unwrap_or(0);
        let n = names.len() as isize;
        let new_idx = (idx as isize + delta).rem_euclid(n) as usize;
        if let Some(w) = names.get(new_idx) {
            if w != &current {
                self.switch_workspace(w, window, cx);
            }
        }
    }

    fn activate_window_at(&mut self, n: usize, window: &mut Window, cx: &mut Context<Self>) {
        let Some(h) = cx.windows().get(n).copied() else {
            return;
        };
        if h == window.window_handle() {
            window.activate_window();
            crate::win_zorder::bring_to_front(window);
            return;
        }
        cx.spawn(async move |_, cx| {
            let _ = cx.update(|cx| {
                h.update(cx, |_, window, _| {
                    window.activate_window();
                    crate::win_zorder::bring_to_front(window);
                })
            });
        })
        .detach();
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
        self.close_search(cx);
        self.pane_select = None;
        self.with_active_term(cx, |term, cx| {
            term.exit_quick_select();
            term.enter_copy_mode();
            cx.notify();
        });
        cx.notify();
    }

    fn open_search(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if crate::find::PICKER_SEARCH_QUICKSELECT_PANESELECT {
            self.open_picker(PickerKind::Search, window, cx);
            return;
        }
        if self.search_open {
            self.close_search(cx);
            return;
        }
        self.palette_open = false;
        self.picker_kind = None;
        self.pane_select = None;
        let seed = self
            .tabs
            .get(self.active)
            .and_then(|t| t.layout.active_pane())
            .map(|term| term.read(cx).selection_plain_text())
            .unwrap_or_default();
        self.search_query = if seed.contains('\n') {
            String::new()
        } else {
            seed
        };
        self.search_case = true;
        self.search_current = 0;
        self.search_open = true;
        self.with_active_term(cx, |term, _| term.exit_quick_select());
        self.sync_search(cx);
        cx.notify();
    }

    fn close_search(&mut self, cx: &mut Context<Self>) {
        self.search_open = false;
        self.with_active_term(cx, |term, cx| {
            term.clear_search();
            cx.notify();
        });
        cx.notify();
    }

    fn sync_search(&mut self, cx: &mut Context<Self>) {
        if !self.search_open {
            return;
        }
        let query = self.search_query.clone();
        let case = self.search_case;
        let mut current = self.search_current;
        self.with_active_term(cx, |term, cx| {
            term.set_search(&query, case, current);
            if let Some((_, shown, total, _)) = term.search_status() {
                current = if total == 0 { 0 } else { shown.saturating_sub(1) };
            }
            cx.notify();
        });
        self.search_current = current;
    }

    fn search_step(&mut self, delta: isize, cx: &mut Context<Self>) {
        let mut current = self.search_current;
        self.with_active_term(cx, |term, cx| {
            term.search_step(delta);
            if let Some((_, shown, total, _)) = term.search_status() {
                current = if total == 0 { 0 } else { shown.saturating_sub(1) };
            }
            cx.notify();
        });
        self.search_current = current;
        cx.notify();
    }

    fn search_key(
        &mut self,
        event: &KeyDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        let ks = &event.keystroke;
        let key = ks.key.as_str();
        // Palette / Search toggles must reach their actions (051 Copy was
        // swallowed here because any Control key returned true).
        if ks.modifiers.control && ks.modifiers.shift && matches!(key, "f" | "p") {
            return false;
        }
        if ks.modifiers.control && !ks.modifiers.shift && key == "p" {
            return false;
        }
        if (ks.modifiers.control && ks.modifiers.shift && key == "c")
            || (ks.modifiers.control && key == "insert")
        {
            self.copy_selection(window, cx, false);
            return true;
        }
        if (ks.modifiers.control && ks.modifiers.shift && key == "v")
            || (ks.modifiers.shift && key == "insert")
        {
            return false;
        }
        if key == "escape" {
            self.close_search(cx);
            return true;
        }
        if key == "enter" || key == "return" || key == "down" {
            let delta = if ks.modifiers.shift || key == "up" {
                -1
            } else {
                1
            };
            self.search_step(delta, cx);
            return true;
        }
        if key == "up" {
            self.search_step(-1, cx);
            return true;
        }
        if key == "backspace" {
            self.search_query.pop();
            self.search_current = 0;
            self.sync_search(cx);
            cx.notify();
            return true;
        }
        if ks.modifiers.control && key == "r" {
            self.search_case = !self.search_case;
            self.search_current = 0;
            self.sync_search(cx);
            cx.notify();
            return true;
        }
        if ks.modifiers.control || ks.modifiers.alt || ks.modifiers.platform {
            return true;
        }
        let ch = ks
            .key_char
            .as_deref()
            .and_then(|s| s.chars().next())
            .or_else(|| key.chars().next().filter(|_| key.len() == 1));
        let ch = if key == "space" {
            Some(' ')
        } else {
            ch.filter(|c| !c.is_control())
        };
        if let Some(ch) = ch {
            self.search_query.push(ch);
            self.search_current = 0;
            self.sync_search(cx);
            cx.notify();
        }
        true
    }

    fn open_quick_select(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if crate::find::PICKER_SEARCH_QUICKSELECT_PANESELECT {
            self.open_picker(PickerKind::QuickSelect, window, cx);
            return;
        }
        self.close_search(cx);
        self.pane_select = None;
        self.with_active_term(cx, |term, cx| {
            term.enter_quick_select();
            cx.notify();
        });
        cx.notify();
    }

    fn open_pane_select(
        &mut self,
        mode: PaneSelectMode,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if crate::find::PICKER_SEARCH_QUICKSELECT_PANESELECT {
            let kind = match mode {
                PaneSelectMode::Activate => PickerKind::PaneSelectActivate,
                PaneSelectMode::Swap => PickerKind::PaneSelectSwap,
                PaneSelectMode::SwapKeep => PickerKind::PaneSelectSwapKeep,
                PaneSelectMode::MoveTab => PickerKind::PaneSelectMoveTab,
                PaneSelectMode::MoveWindow => PickerKind::PaneSelectMoveWindow,
            };
            self.open_picker(kind, window, cx);
            return;
        }
        self.close_search(cx);
        self.with_active_term(cx, |term, _| term.exit_quick_select());
        if let Some(tab) = self.tabs.get_mut(self.active) {
            tab.layout.unzoom();
        }
        self.pane_select = Some(PaneSelectState {
            mode,
            typed: String::new(),
        });
        cx.notify();
    }

    fn pane_select_key(
        &mut self,
        event: &KeyDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        let ks = &event.keystroke;
        let key = ks.key.as_str();
        if key == "escape" {
            self.pane_select = None;
            cx.notify();
            return true;
        }
        if key == "backspace" {
            if let Some(ps) = &mut self.pane_select {
                ps.typed.pop();
            }
            cx.notify();
            return true;
        }
        if ks.modifiers.control || ks.modifiers.alt || ks.modifiers.platform {
            return true;
        }
        let ch = ks
            .key_char
            .as_deref()
            .and_then(|s| s.chars().next())
            .or_else(|| key.chars().next().filter(|_| key.len() == 1));
        let Some(ch) = ch.filter(|c| c.is_ascii_alphanumeric()) else {
            return true;
        };
        let Some(ps) = self.pane_select.as_mut() else {
            return true;
        };
        ps.typed.push(ch.to_ascii_lowercase());
        let typed = ps.typed.clone();
        let mode = ps.mode;
        let n = self
            .tabs
            .get(self.active)
            .map(|t| t.layout.pane_count())
            .unwrap_or(0);
        let labels = crate::find::compute_labels_for_alphabet(&crate::find::alphabet(), n);
        if let Some(i) = labels.iter().position(|l| *l == typed) {
            self.pane_select = None;
            let id = format!("pane:{i}");
            match mode {
                PaneSelectMode::Activate => self.pane_select_activate(&id, cx),
                PaneSelectMode::Swap => self.pane_select_swap(&id, false, cx),
                PaneSelectMode::SwapKeep => self.pane_select_swap(&id, true, cx),
                PaneSelectMode::MoveTab => self.pane_select_move_tab(&id, window, cx),
                PaneSelectMode::MoveWindow => self.pane_select_move_window(&id, window, cx),
            }
            return true;
        }
        if !labels.iter().any(|l| l.starts_with(&typed)) {
            if let Some(ps) = &mut self.pane_select {
                ps.typed.clear();
            }
        }
        cx.notify();
        true
    }

    fn show_debug_overlay(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if crate::find::PICKER_SEARCH_QUICKSELECT_PANESELECT {
            self.open_picker(PickerKind::Debug, window, cx);
            return;
        }
        window.push_notification(
            Notification::info("Debug overlay Lua REPL is not implemented yet"),
            cx,
        );
    }

    fn toggle_fps_hud(&mut self, cx: &mut Context<Self>) {
        self.show_fps = !self.show_fps;
        cx.notify();
    }

    fn reload_configuration(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        config::reload();
        crate::mux_host::register_configured_domains();
        self.shells = crate::mux_host::launch_profiles();
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
        if empty && self.remove_tab_at(tab_index, cx, true) {
            self.close_self_or_quit(window, cx);
            return;
        }
        self.sync_pane_focus(cx);
        self.request_terminal_focus(window, cx);
        cx.notify();
    }

    /// Last tab of this HWND. Other windows stay up (052). Last HWND still
    /// quits, same as wezterm-gui `quit_when_all_windows_are_closed`.
    /// If this was the last HWND in the workspace, switch to another
    /// workspace that still has mux windows (055).
    fn close_self_or_quit(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let mux_window = self.mux_window_id;
        let workspace = self.workspace.clone();
        for tab in &self.tabs {
            Self::release_tab_mux(tab, cx);
        }
        self.tabs.clear();
        crate::workspaces::unregister(mux_window, cx);
        crate::mux_host::kill_mux_window(mux_window);
        let others = crate::workspaces::remaining(cx);
        if let Some((fallback, _)) = others.iter().find(|(w, _)| w != &workspace) {
            crate::mux_host::set_active_workspace(fallback);
            crate::workspaces::show_workspace(fallback, cx);
            window.remove_window();
        } else if others.iter().any(|(w, _)| w == &workspace) {
            close_this_window_or_quit(window, cx);
        } else {
            cx.quit();
        }
    }

    fn confirm_close_window(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let policy = config::configuration().window_close_confirmation;
        let all_skip = self
            .tabs
            .iter()
            .all(|t| t.can_close_without_prompting(mux::pane::CloseReason::Window, cx));
        if !wants_quit_prompt(policy, all_skip) {
            self.close_self_or_quit(window, cx);
            return;
        }
        let last_hwnd = crate::workspaces::is_last_hwnd(window, cx);
        let (title, message, button) = if last_hwnd {
            ("Quit WezTerm?", "🛑 Really Quit WezTerm?", "Quit")
        } else {
            (
                "Close window?",
                "🛑 Really close this window and all contained tabs?",
                "Close",
            )
        };
        self.focus_terminal(window, cx);
        let shell = cx.entity();
        let restore = Self::dialog_restore(shell.clone());
        open_confirm(
            window,
            cx,
            title,
            message,
            button,
            true,
            move |window, cx| {
                shell.update(cx, |this, cx| {
                    this.close_self_or_quit(window, cx);
                });
            },
            restore,
        );
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
        self.pane_select = None;
        self.with_active_term(cx, |term, _| term.exit_quick_select());
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
        self.add_tab(window, cx);
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

    fn on_open_search(&mut self, _: &OpenSearch, window: &mut Window, cx: &mut Context<Self>) {
        self.open_search(window, cx);
    }

    fn copy_selection(&mut self, window: &mut Window, cx: &mut Context<Self>, notify: bool) {
        let mut copied = self
            .tabs
            .get(self.active)
            .and_then(|tab| tab.layout.active_pane())
            .map(|term| term.update(cx, |term, cx| term.copy_selection(cx)))
            .unwrap_or(false);
        if !copied && self.search_open && !self.search_query.is_empty() {
            cx.write_to_clipboard(ClipboardItem::new_string(self.search_query.clone()));
            copied = true;
        }
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

    fn on_send_pty_tab(&mut self, _: &SendPtyTab, window: &mut Window, cx: &mut Context<Self>) {
        self.send_pty_tab(false, window, cx);
    }

    fn on_send_pty_shift_tab(
        &mut self,
        _: &SendPtyShiftTab,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.send_pty_tab(true, window, cx);
    }

    fn send_pty_tab(&mut self, shift: bool, window: &mut Window, cx: &mut Context<Self>) {
        // Dialogs stay in APP_CONTEXT (unlike palette). Let Root Tab cycle
        // OK/Cancel. Search / pane-select swallow Tab instead of focus_next.
        if window.has_active_dialog(cx) {
            cx.propagate();
            return;
        }
        if self.overlay_open() || self.search_open || self.pane_select.is_some() {
            return;
        }
        if let Some(term) = self
            .tabs
            .get(self.active)
            .and_then(|tab| tab.layout.active_pane())
            .cloned()
        {
            term.update(cx, |term, cx| {
                if term.send_tab(shift, cx) {
                    cx.notify();
                }
            });
            self.request_terminal_focus(window, cx);
        }
    }

    fn on_term_key(&mut self, event: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        if window.has_active_dialog(cx) {
            return;
        }
        if self.palette_open || self.picker_kind.is_some() {
            return;
        }
        if self.search_open {
            if self.search_key(event, window, cx) {
                cx.stop_propagation();
            }
            return;
        }
        if self.pane_select.is_some() {
            if self.pane_select_key(event, window, cx) {
                cx.stop_propagation();
            }
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
        crate::workspaces::touch(window, self.mux_window_id, &self.workspace, cx);
        let active_ws = crate::mux_host::active_workspace();
        if self.workspace != active_ws {
            crate::win_zorder::set_hidden(window, true);
            self.workspace_hidden = true;
        } else if self.workspace_hidden {
            self.workspace_hidden = false;
            self.request_terminal_focus(window, cx);
        }
        if !self.workspace_hidden {
            crate::workspaces::set_current_view(&self.workspace, cx);
        }
        let workspace = self.workspace.clone();
        if self.focus_pending
            && !self.workspace_hidden
            && !self.overlay_open()
            && !window.has_active_dialog(cx)
        {
            self.focus_terminal(window, cx);
            self.focus_pending = false;
        }
        let active = self.active.min(self.tabs.len().saturating_sub(1));
        let pane_body = self.tabs.get(active).map(|t| {
            let pane_labels = self.pane_select.as_ref().map(|_| {
                crate::find::compute_labels_for_alphabet(
                    &crate::find::alphabet(),
                    t.layout.pane_count(),
                )
            });
            if t.layout.is_zoomed() {
                render_split_tree(
                    &LayoutNode::leaf(t.layout.active_index()),
                    t.layout.panes(),
                    &t.split_states,
                    pane_labels.as_deref(),
                )
            } else {
                render_split_tree(
                    t.layout.root(),
                    t.layout.panes(),
                    &t.split_states,
                    pane_labels.as_deref(),
                )
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
        let search_bar = if self.search_open {
            let (query, shown, total, case) = self
                .tabs
                .get(active)
                .and_then(|t| t.layout.active_pane())
                .and_then(|term| term.read(cx).search_status())
                .unwrap_or_else(|| {
                    (
                        self.search_query.clone(),
                        0,
                        0,
                        self.search_case,
                    )
                });
            let mode = if case {
                "case-sensitive"
            } else {
                "ignore-case"
            };
            Some(format!("Search: {query} ({shown}/{total} matches. {mode})"))
        } else {
            None
        };
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
            .on_action(cx.listener(Self::on_open_search))
            .on_action(cx.listener(Self::on_copy))
            .on_action(cx.listener(Self::on_paste))
            .on_action(cx.listener(Self::on_send_pty_tab))
            .on_action(cx.listener(Self::on_send_pty_shift_tab))
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
                            Label::new(format!("WezTerm GPUI · {workspace}"))
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
                                            this.add_tab(window, cx);
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
            .when_some(search_bar, |this, text| {
                this.child(
                    div()
                        .id("search-bar")
                        .w_full()
                        .px_3()
                        .py_1()
                        .bg(rgb(0xe8e8e8))
                        .child(
                            Label::new(text)
                                .text_size(px(13.))
                                .text_color(rgb(0x111111)),
                        ),
                )
            })
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
                            "Ctrl+Shift+P palette  ·  Ctrl+Shift+C/V copy/paste  ·  Ctrl+Shift+F search  ·  {}  ·  {}  ·  {}  ·  ws `{workspace}`  ·  mux LocalDomain",
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

fn app_window_options_offset(from: &Window, cx: &App) -> WindowOptions {
    let current = from.bounds();
    let size = if current.size.width > px(1.) && current.size.height > px(1.) {
        current.size
    } else {
        size(px(980.), px(640.))
    };
    let origin = cascade_origin(from);
    app_window_options_at(
        Some(Bounds { origin, size }),
        cx,
    )
}

/// Pick a top-left that no visible HWND already occupies (050/051).
fn cascade_origin(from: &Window) -> Point<Pixels> {
    let gpui = from.bounds().origin;
    let step = 48.0;
    let slack = 24.0;
    let Some((x0, y0)) = crate::win_zorder::hwnd_origin(from) else {
        return point(gpui.x + px(step), gpui.y + px(step));
    };
    let scale = f32::from(from.scale_factor()).max(0.5);
    let step_px = (step * scale).round().max(1.0) as i32;
    let slack_px = (slack * scale).round().max(1.0) as i32;
    let occupied = crate::win_zorder::visible_hwnd_origins();
    let (x, y) = cascade_screen_origin((x0, y0), &occupied, step_px, slack_px);
    let dx = (x - x0) as f32 / scale;
    let dy = (y - y0) as f32 / scale;
    point(gpui.x + px(dx), gpui.y + px(dy))
}

fn cascade_screen_origin(
    start: (i32, i32),
    occupied: &[(i32, i32)],
    step: i32,
    slack: i32,
) -> (i32, i32) {
    let mut x = start.0.saturating_add(step);
    let mut y = start.1.saturating_add(step);
    for _ in 0..32 {
        if !occupied.iter().any(|(ox, oy)| {
            ox.abs_diff(x) <= slack as u32 && oy.abs_diff(y) <= slack as u32
        }) {
            return (x, y);
        }
        x = x.saturating_add(step);
        y = y.saturating_add(step);
    }
    (x, y)
}

/// Last pane/tab of this HWND. Other GPUI windows stay; last HWND quits.
fn close_this_window_or_quit(window: &mut Window, cx: &mut App) {
    if cx.windows().len() > 1 {
        window.remove_window();
    } else {
        cx.quit();
    }
}

fn focus_opened_window(handle: WindowHandle<Root>, cx: &mut Context<AppShell>) {
    cx.spawn(async move |_, cx| {
        let _ = cx.update(|cx| {
            handle.update(cx, |_, window, _| {
                window.activate_window();
                crate::win_zorder::bring_to_front(window);
            })
        });
    })
    .detach();
}

fn open_app_shell(
    opts: WindowOptions,
    cx: &mut App,
) -> Result<WindowHandle<Root>, anyhow::Error> {
    cx.open_window(opts, |window, cx| {
        let view = cx.new(|cx| AppShell::new(window, cx));
        let root = cx.new(|cx| Root::new(view.clone(), window, cx).bg(cx.theme().background));
        view.update(cx, |shell, cx| shell.focus_terminal(window, cx));
        root
    })
    .map_err(|err| anyhow::anyhow!("{err:?}"))
}

fn app_window_options_at(bounds: Option<Bounds<Pixels>>, cx: &App) -> WindowOptions {
    WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(bounds.unwrap_or_else(|| {
            Bounds::centered(None, size(px(980.), px(640.)), cx)
        }))),
        titlebar: Some(TitlebarOptions {
            title: Some("WezTerm GPUI".into()),
            ..TitleBar::title_bar_options()
        }),
        ..Default::default()
    }
}

fn spawn_domain_name(cmd: &config::keyassignment::SpawnCommand) -> Option<String> {
    match &cmd.domain {
        config::keyassignment::SpawnTabDomain::DomainName(name) => Some(name.clone()),
        _ => None,
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
    pane_labels: Option<&[String]>,
) -> AnyElement {
    match node {
        LayoutNode::Leaf(i) => {
            let Some(pane) = panes.get(*i) else {
                return div().into_any_element();
            };
            let label = pane_labels.and_then(|labels| labels.get(*i)).cloned();
            div()
                .relative()
                .size_full()
                .min_h_0()
                .overflow_hidden()
                .child(pane.clone())
                .when_some(label, |this, lab| {
                    this.child(
                        div()
                            .absolute()
                            .inset_0()
                            .flex()
                            .items_center()
                            .justify_center()
                            .child(
                                div()
                                    .w(px(56.))
                                    .h(px(56.))
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .rounded_lg()
                                    .bg(gpui::black().opacity(0.82))
                                    .child(
                                        div()
                                            .text_size(px(28.))
                                            .line_height(px(28.))
                                            .font_weight(FontWeight::BOLD)
                                            .text_color(gpui::white())
                                            .child(lab),
                                    ),
                            ),
                    )
                })
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
                .child(render_split_tree(first, panes, states, pane_labels))
                .child(render_split_tree(second, panes, states, pane_labels))
                .into_any_element()
        }
    }
}

#[cfg(test)]
mod cascade_tests {
    use super::cascade_screen_origin;

    #[test]
    fn first_slot_free() {
        assert_eq!(
            cascade_screen_origin((100, 100), &[(100, 100)], 48, 24),
            (148, 148)
        );
    }

    #[test]
    fn skips_occupied_cascade() {
        assert_eq!(
            cascade_screen_origin((100, 100), &[(100, 100), (148, 148)], 48, 24),
            (196, 196)
        );
    }
}
