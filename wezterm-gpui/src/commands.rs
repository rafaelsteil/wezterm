//! Default command-palette catalog (Windows `compute_default_actions`).
//!
//! Source: `wezterm-gui/src/commands.rs` `CommandDef` + `compute_default_actions`.
//! Status tracker: `docs/command-palette.json`. Do not path-dep `wezterm-gui`.
//!
//! `Wired` rows run through AppShell. `Listed` rows render muted/disabled so the
//! full wezterm-gui list is visible while work is still tracking.

/// How the action is fulfilled. Palette UI uses this only for notes; `status`
/// decides whether Enter runs it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommandKind {
    /// Mux / pane / config API. GPUI is a thin call once wired.
    CallCore,
    /// GPUI window chrome (font, tabs, quit, hide, fullscreen, second window).
    GpuiWindow,
    /// Needs a GPUI overlay/modal (search, copy mode, charselect, launcher, …).
    GpuiUi,
    /// `wezterm-open-url`.
    OpenUrl,
    /// Key tables / InputMap. Not in this POC until asked.
    InputMap,
}

/// Whether the GPUI palette will execute the row today.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommandStatus {
    Wired,
    Listed,
}

#[derive(Debug, Clone, Copy)]
pub struct PaletteCommand {
    pub id: &'static str,
    pub brief: &'static str,
    pub doc: &'static str,
    pub menubar: &'static str,
    pub keys: &'static str,
    pub kind: CommandKind,
    pub status: CommandStatus,
}

impl PaletteCommand {
    pub fn is_wired(self) -> bool {
        self.status == CommandStatus::Wired
    }

    pub fn haystack(self) -> String {
        format!(
            "{}: {}. {} {}",
            self.menubar, self.brief, self.doc, self.keys
        )
    }
}

macro_rules! cmd {
    ($id:expr, $brief:expr, $doc:expr, $bar:expr, $keys:expr, $kind:ident, $status:ident) => {
        PaletteCommand {
            id: $id,
            brief: $brief,
            doc: $doc,
            menubar: $bar,
            keys: $keys,
            kind: CommandKind::$kind,
            status: CommandStatus::$status,
        }
    };
}

/// Windows default palette (`compute_default_actions` minus macOS-only).
/// Shortcuts are the CTRL+SHIFT synthesis wezterm-gui shows on Windows.
pub const PALETTE_COMMANDS: &[PaletteCommand] = &[
    // WezTerm
    cmd!(
        "ReloadConfiguration",
        "Reload configuration",
        "Reloads the configuration file",
        "WezTerm",
        "Shift-Ctrl-R",
        CallCore,
        Listed
    ),
    // Shell
    cmd!(
        "SpawnTab.CurrentPaneDomain",
        "New Tab",
        "Create a new tab in the same domain as the current pane",
        "Shell",
        "Shift-Ctrl-T",
        CallCore,
        Wired
    ),
    cmd!(
        "SpawnWindow",
        "New Window",
        "Launches the default program into a new window",
        "Shell",
        "Shift-Ctrl-N",
        GpuiWindow,
        Listed
    ),
    cmd!(
        "SplitVertical",
        "Split Vertically (Top/Bottom)",
        "Split the current pane vertically into two panes, by spawning the default program into the bottom half",
        "Shell",
        "Ctrl-Alt-Shift-'",
        CallCore,
        Wired
    ),
    cmd!(
        "SplitHorizontal",
        "Split Horizontally (Left/Right)",
        "Split the current pane horizontally into two panes, by spawning the default program into the right hand side",
        "Shell",
        "Ctrl-Alt-Shift-5",
        CallCore,
        Wired
    ),
    cmd!(
        "CloseCurrentTab.confirm",
        "Close current Tab",
        "Closes the current tab, terminating all the processes that are running in its panes.",
        "Shell",
        "Shift-Ctrl-W",
        GpuiWindow,
        Wired
    ),
    cmd!(
        "CloseCurrentPane.confirm",
        "Close current Pane",
        "Closes the current pane, terminating the processes that are running inside it.",
        "Shell",
        "",
        CallCore,
        Wired
    ),
    cmd!(
        "DetachDomain.CurrentPaneDomain",
        "Detach the domain of the active pane",
        "Detaches (disconnects from) the domain of the active pane",
        "Shell",
        "",
        CallCore,
        Listed
    ),
    cmd!(
        "ResetTerminal",
        "Reset the terminal emulation state in the current pane",
        "Reset the terminal emulation state in the current pane",
        "Shell",
        "",
        CallCore,
        Wired
    ),
    cmd!(
        "OpenLinkAtMouseCursor",
        "Open link at mouse cursor",
        "If there is no link under the mouse cursor, has no effect.",
        "Shell",
        "",
        CallCore,
        Wired
    ),
    cmd!(
        "ShowLauncher",
        "Show the launcher",
        "Shows the launcher menu",
        "Shell",
        "",
        GpuiUi,
        Listed
    ),
    // Edit
    cmd!(
        "PasteFrom.PrimarySelection",
        "Paste primary selection",
        "Pastes text from the primary selection",
        "Edit",
        "Shift-Insert",
        CallCore,
        Listed
    ),
    cmd!(
        "CopyTo.PrimarySelection",
        "Copy to primary selection",
        "Copies text to the primary selection",
        "Edit",
        "Ctrl-Insert",
        CallCore,
        Listed
    ),
    cmd!(
        "CopyTo.Clipboard",
        "Copy to clipboard",
        "Copies text to the clipboard",
        "Edit",
        "Shift-Ctrl-C",
        CallCore,
        Wired
    ),
    cmd!(
        "PasteFrom.Clipboard",
        "Paste from clipboard",
        "Pastes text from the clipboard",
        "Edit",
        "Shift-Ctrl-V",
        CallCore,
        Wired
    ),
    cmd!(
        "ClearScrollback.ScrollbackOnly",
        "Clear scrollback",
        "Clears any text that has scrolled out of the viewport of the current pane",
        "Edit",
        "Shift-Ctrl-K",
        CallCore,
        Wired
    ),
    cmd!(
        "ClearScrollback.ScrollbackAndViewport",
        "Clear the scrollback and viewport",
        "Removes all content from the screen and scrollback",
        "Edit",
        "",
        CallCore,
        Wired
    ),
    cmd!(
        "QuickSelect",
        "Enter QuickSelect mode",
        "Activates the quick selection UI for the current pane",
        "Edit",
        "Shift-Ctrl-Space",
        GpuiUi,
        Listed
    ),
    cmd!(
        "CharSelect",
        "Enter Emoji / Character selection mode",
        "Activates the character selection UI for the current pane",
        "Edit",
        "Shift-Ctrl-U",
        GpuiUi,
        Listed
    ),
    cmd!(
        "ActivateCopyMode",
        "Activate Copy Mode",
        "Enter mouse-less copy mode to select text using only the keyboard",
        "Edit",
        "Shift-Ctrl-X",
        GpuiUi,
        Listed
    ),
    cmd!(
        "ClearKeyTableStack",
        "Clear the key table stack",
        "Removes all entries from the stack",
        "Edit",
        "",
        InputMap,
        Listed
    ),
    cmd!(
        "ActivateCommandPalette",
        "Activate Command Palette",
        "Shows the command palette modal",
        "Edit",
        "Shift-Ctrl-P",
        GpuiUi,
        Wired
    ),
    // View
    cmd!(
        "DecreaseFontSize",
        "Decrease font size",
        "Scales the font size smaller by 10%",
        "View",
        "Ctrl--",
        GpuiWindow,
        Wired
    ),
    cmd!(
        "IncreaseFontSize",
        "Increase font size",
        "Scales the font size larger by 10%",
        "View",
        "Ctrl-=",
        GpuiWindow,
        Wired
    ),
    cmd!(
        "ResetFontSize",
        "Reset font size",
        "Restores the font size to match your configuration file",
        "View",
        "Ctrl-0",
        GpuiWindow,
        Wired
    ),
    cmd!(
        "ResetFontAndWindowSize",
        "Reset the window and font size",
        "Restores the original window and font size",
        "View",
        "",
        GpuiWindow,
        Wired
    ),
    cmd!(
        "ScrollByPage.Up",
        "Scroll Up One Page",
        "Scrolls the viewport up by 1 page",
        "View",
        "Shift-PageUp",
        CallCore,
        Wired
    ),
    cmd!(
        "ScrollByPage.Down",
        "Scroll Down One Page",
        "Scrolls the viewport down by 1 page",
        "View",
        "Shift-PageDown",
        CallCore,
        Wired
    ),
    cmd!(
        "ScrollToTop",
        "Scroll to the top",
        "Scrolls to the top of the viewport",
        "View",
        "",
        CallCore,
        Wired
    ),
    cmd!(
        "ScrollToBottom",
        "Scroll to the bottom",
        "Scrolls to the bottom of the viewport",
        "View",
        "",
        CallCore,
        Wired
    ),
    cmd!(
        "ToggleFullScreen",
        "Toggle full screen mode",
        "Switch between normal and full screen mode",
        "View",
        "Alt-Return",
        GpuiWindow,
        Wired
    ),
    // Window
    cmd!(
        "ToggleAlwaysOnTop",
        "Toggle always on Top",
        "Toggles the window between floating and non-floating states to stay on top of other windows.",
        "Window",
        "",
        GpuiWindow,
        Wired
    ),
    cmd!(
        "ToggleAlwaysOnBottom",
        "Toggle always on Bottom",
        "Toggles the window to remain behind all other windows.",
        "Window",
        "",
        GpuiWindow,
        Wired
    ),
    cmd!(
        "SetWindowLevel.AlwaysOnBottom",
        "Always on Bottom",
        "Set window to remain behind all other windows.",
        "Window",
        "",
        GpuiWindow,
        Wired
    ),
    cmd!(
        "SetWindowLevel.Normal",
        "Normal",
        "Set window level to normal",
        "Window",
        "",
        GpuiWindow,
        Wired
    ),
    cmd!(
        "SetWindowLevel.AlwaysOnTop",
        "Always on Top",
        "Set the window level to be on top of other windows.",
        "Window",
        "",
        GpuiWindow,
        Wired
    ),
    cmd!(
        "Hide",
        "Hide/Minimize Window",
        "Hides/Mimimizes the current window",
        "Window",
        "Shift-Ctrl-M",
        GpuiWindow,
        Wired
    ),
    cmd!(
        "Search",
        "Search pane output",
        "Enters the search mode UI for the current pane",
        "Window",
        "Shift-Ctrl-F",
        GpuiUi,
        Listed
    ),
    cmd!(
        "PaneSelect.Activate",
        "Enter Pane selection mode",
        "Activates the pane selection UI",
        "Window",
        "",
        GpuiUi,
        Listed
    ),
    cmd!(
        "PaneSelect.SwapWithActive",
        "Swap a pane with the active pane",
        "Activates the pane selection UI",
        "Window",
        "",
        GpuiUi,
        Listed
    ),
    cmd!(
        "PaneSelect.SwapWithActiveKeepFocus",
        "Swap a pane with the active pane, keeping focus",
        "Activates the pane selection UI",
        "Window",
        "",
        GpuiUi,
        Listed
    ),
    cmd!(
        "PaneSelect.MoveToNewTab",
        "Move a pane into its own tab",
        "Activates the pane selection UI",
        "Window",
        "",
        GpuiUi,
        Listed
    ),
    cmd!(
        "PaneSelect.MoveToNewWindow",
        "Move a pane into its own window",
        "Activates the pane selection UI",
        "Window",
        "",
        GpuiUi,
        Listed
    ),
    cmd!(
        "RotatePanes.Clockwise",
        "Rotate panes Clockwise",
        "Rotate panes Clockwise",
        "Window",
        "",
        CallCore,
        Wired
    ),
    cmd!(
        "RotatePanes.CounterClockwise",
        "Rotate panes CounterClockwise",
        "Rotate panes CounterClockwise",
        "Window",
        "",
        CallCore,
        Wired
    ),
    cmd!(
        "ActivateTab.0",
        "Activate 1st Tab",
        "Activates the 1st tab",
        "Window",
        "Shift-Ctrl-1",
        GpuiWindow,
        Wired
    ),
    cmd!(
        "ActivateTab.1",
        "Activate 2nd Tab",
        "Activates the 2nd tab",
        "Window",
        "Shift-Ctrl-2",
        GpuiWindow,
        Wired
    ),
    cmd!(
        "ActivateTab.2",
        "Activate 3rd Tab",
        "Activates the 3rd tab",
        "Window",
        "Shift-Ctrl-3",
        GpuiWindow,
        Wired
    ),
    cmd!(
        "ActivateTab.3",
        "Activate 4th Tab",
        "Activates the 4th tab",
        "Window",
        "Shift-Ctrl-4",
        GpuiWindow,
        Wired
    ),
    cmd!(
        "ActivateTab.4",
        "Activate 5th Tab",
        "Activates the 5th tab",
        "Window",
        "Shift-Ctrl-5",
        GpuiWindow,
        Wired
    ),
    cmd!(
        "ActivateTab.5",
        "Activate 6th Tab",
        "Activates the 6th tab",
        "Window",
        "Shift-Ctrl-6",
        GpuiWindow,
        Wired
    ),
    cmd!(
        "ActivateTab.6",
        "Activate 7th Tab",
        "Activates the 7th tab",
        "Window",
        "Shift-Ctrl-7",
        GpuiWindow,
        Wired
    ),
    cmd!(
        "ActivateTab.7",
        "Activate 8th Tab",
        "Activates the 8th tab",
        "Window",
        "Shift-Ctrl-8",
        GpuiWindow,
        Wired
    ),
    cmd!(
        "ActivateTab.-1",
        "Activate right-most tab",
        "Activates the tab on the far right",
        "Window",
        "Shift-Ctrl-9",
        GpuiWindow,
        Wired
    ),
    cmd!(
        "ActivateTabRelative.-1",
        "Activate the tab to the left",
        "Activates the tab to the left. If this is the left-most tab then cycles around and activates the right-most tab",
        "Window",
        "Shift-Ctrl-Tab",
        GpuiWindow,
        Wired
    ),
    cmd!(
        "ActivateTabRelative.1",
        "Activate the tab to the right",
        "Activates the tab to the right. If this is the right-most tab then cycles around and activates the left-most tab",
        "Window",
        "Ctrl-Tab",
        GpuiWindow,
        Wired
    ),
    cmd!(
        "ActivateWindow.0",
        "Activate 1st Window",
        "Activates the 1st window",
        "Window",
        "",
        GpuiWindow,
        Listed
    ),
    cmd!(
        "ActivateWindow.1",
        "Activate 2nd Window",
        "Activates the 2nd window",
        "Window",
        "",
        GpuiWindow,
        Listed
    ),
    cmd!(
        "ActivateWindow.2",
        "Activate 3rd Window",
        "Activates the 3rd window",
        "Window",
        "",
        GpuiWindow,
        Listed
    ),
    cmd!(
        "ActivateWindow.3",
        "Activate 4th Window",
        "Activates the 4th window",
        "Window",
        "",
        GpuiWindow,
        Listed
    ),
    cmd!(
        "ActivateWindow.4",
        "Activate 5th Window",
        "Activates the 5th window",
        "Window",
        "",
        GpuiWindow,
        Listed
    ),
    cmd!(
        "ActivateWindow.5",
        "Activate 6th Window",
        "Activates the 6th window",
        "Window",
        "",
        GpuiWindow,
        Listed
    ),
    cmd!(
        "ActivateWindow.6",
        "Activate 7th Window",
        "Activates the 7th window",
        "Window",
        "",
        GpuiWindow,
        Listed
    ),
    cmd!(
        "ActivateWindow.7",
        "Activate 8th Window",
        "Activates the 8th window",
        "Window",
        "",
        GpuiWindow,
        Listed
    ),
    cmd!(
        "ActivateWindow.8",
        "Activate 9th Window",
        "Activates the 9th window",
        "Window",
        "",
        GpuiWindow,
        Listed
    ),
    cmd!(
        "ActivateWindow.9",
        "Activate 10th Window",
        "Activates the 10th window",
        "Window",
        "",
        GpuiWindow,
        Listed
    ),
    cmd!(
        "ActivateWindowRelative.-1",
        "Activate the preceeding window",
        "Activates the preceeding window. If this is the first window then cycles around and activates last window",
        "Window",
        "",
        GpuiWindow,
        Listed
    ),
    cmd!(
        "ActivateWindowRelative.1",
        "Activate the next window",
        "Activates the next window. If this is the last window then cycles around and activates first window",
        "Window",
        "",
        GpuiWindow,
        Listed
    ),
    cmd!(
        "MoveTabRelative.-1",
        "Move tab one place to the left",
        "Rearranges the tabs so that the current tab moves one place to the left",
        "Window",
        "Shift-Ctrl-PageUp",
        GpuiWindow,
        Wired
    ),
    cmd!(
        "MoveTabRelative.1",
        "Move tab one place to the right",
        "Rearranges the tabs so that the current tab moves one place to the right",
        "Window",
        "Shift-Ctrl-PageDown",
        GpuiWindow,
        Wired
    ),
    cmd!(
        "AdjustPaneSize.Left",
        "Resize Pane 1 cell(s) to the Left",
        "Adjusts the closest split divider to the left",
        "Window",
        "Ctrl-Alt-Shift-LeftArrow",
        CallCore,
        Listed
    ),
    cmd!(
        "AdjustPaneSize.Right",
        "Resize Pane 1 cell(s) to the Right",
        "Adjusts the closest split divider to the right",
        "Window",
        "Ctrl-Alt-Shift-RightArrow",
        CallCore,
        Listed
    ),
    cmd!(
        "AdjustPaneSize.Up",
        "Resize Pane 1 cell(s) Upwards",
        "Adjusts the closest split divider towards the top",
        "Window",
        "Ctrl-Alt-Shift-UpArrow",
        CallCore,
        Listed
    ),
    cmd!(
        "AdjustPaneSize.Down",
        "Resize Pane 1 cell(s) Downwards",
        "Adjusts the closest split divider towards the bottom",
        "Window",
        "Ctrl-Alt-Shift-DownArrow",
        CallCore,
        Listed
    ),
    cmd!(
        "ActivatePaneDirection.Left",
        "Activate Pane Left",
        "Activates the pane to the left of the current pane",
        "Window",
        "Shift-Ctrl-LeftArrow",
        CallCore,
        Wired
    ),
    cmd!(
        "ActivatePaneDirection.Right",
        "Activate Pane Right",
        "Activates the pane to the right of the current pane",
        "Window",
        "Shift-Ctrl-RightArrow",
        CallCore,
        Wired
    ),
    cmd!(
        "ActivatePaneDirection.Up",
        "Activate Pane Up",
        "Activates the pane to the top of the current pane",
        "Window",
        "Shift-Ctrl-UpArrow",
        CallCore,
        Wired
    ),
    cmd!(
        "ActivatePaneDirection.Down",
        "Activate Pane Down",
        "Activates the pane to the bottom of the current pane",
        "Window",
        "Shift-Ctrl-DownArrow",
        CallCore,
        Wired
    ),
    cmd!(
        "TogglePaneZoomState",
        "Toggle Pane Zoom",
        "Toggles the zoom state for the current pane",
        "Window",
        "Shift-Ctrl-Z",
        CallCore,
        Wired
    ),
    cmd!(
        "ActivateLastTab",
        "Activate the last active tab",
        "If there was no prior active tab, has no effect.",
        "Window",
        "",
        GpuiWindow,
        Wired
    ),
    cmd!(
        "ShowTabNavigator",
        "Navigate tabs",
        "Shows the tab navigator",
        "Window",
        "",
        GpuiUi,
        Listed
    ),
    // Help
    cmd!(
        "OpenUri.docs",
        "Documentation",
        "Visit the wezterm documentation website",
        "Help",
        "",
        OpenUrl,
        Wired
    ),
    cmd!(
        "OpenUri.discussions",
        "Discuss on GitHub",
        "Visit wezterm's GitHub discussion",
        "Help",
        "",
        OpenUrl,
        Wired
    ),
    cmd!(
        "OpenUri.issues",
        "Search or report issue on GitHub",
        "Visit wezterm's GitHub issues",
        "Help",
        "",
        OpenUrl,
        Wired
    ),
    cmd!(
        "ShowDebugOverlay",
        "Show debug overlay",
        "Activates the debug overlay and Lua REPL",
        "Help",
        "Shift-Ctrl-L",
        GpuiUi,
        Listed
    ),
    // POC extras (not in Windows compute_default_actions; already wired in the shell)
    cmd!(
        "QuitApplication",
        "Quit WezTerm",
        "Quits WezTerm",
        "WezTerm",
        "Ctrl-Q",
        GpuiWindow,
        Wired
    ),
    cmd!(
        "PromptInputLine",
        "Prompt the user for a line of text",
        "Activates the prompt overlay and wait for input",
        "Edit",
        "",
        GpuiUi,
        Wired
    ),
    cmd!(
        "Confirmation",
        "Prompt the user for confirmation",
        "Activates the confirmation overlay and wait for input",
        "Edit",
        "",
        GpuiUi,
        Wired
    ),
    cmd!(
        "RenameTab",
        "Rename tab",
        "Prompt for a new title for the current tab",
        "Edit",
        "",
        GpuiUi,
        Wired
    ),
];

#[cfg(test)]
pub fn command_by_id(id: &str) -> Option<&'static PaletteCommand> {
    PALETTE_COMMANDS.iter().find(|c| c.id == id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn palette_ids_unique() {
        let mut seen = HashSet::new();
        for cmd in PALETTE_COMMANDS {
            assert!(seen.insert(cmd.id), "duplicate palette id {}", cmd.id);
        }
    }

    #[test]
    fn palette_has_windows_defaults_and_wired_subset() {
        assert!(PALETTE_COMMANDS.len() >= 80);
        let wired: Vec<_> = PALETTE_COMMANDS
            .iter()
            .filter(|c| c.is_wired())
            .map(|c| c.id)
            .collect();
        assert_eq!(wired.len(), 55);
        for id in [
            "SpawnTab.CurrentPaneDomain",
            "CopyTo.Clipboard",
            "PasteFrom.Clipboard",
            "ClearScrollback.ScrollbackOnly",
            "IncreaseFontSize",
            "ActivateCommandPalette",
            "CloseCurrentTab.confirm",
        ] {
            assert!(wired.contains(&id), "missing wired {id}");
        }
        assert!(
            PALETTE_COMMANDS
                .iter()
                .any(|c| c.id == "CharSelect" && !c.is_wired())
        );
        assert!(
            PALETTE_COMMANDS
                .iter()
                .any(|c| c.id == "ShowLauncher" && !c.is_wired())
        );
    }

    #[test]
    fn palette_wired_ids_are_dispatched() {
        for id in [
            "SpawnTab.CurrentPaneDomain",
            "CloseCurrentTab.confirm",
            "CloseCurrentPane.confirm",
            "QuitApplication",
            "IncreaseFontSize",
            "DecreaseFontSize",
            "ResetFontSize",
            "ClearScrollback.ScrollbackOnly",
            "CopyTo.Clipboard",
            "PasteFrom.Clipboard",
            "ActivateCommandPalette",
            "RenameTab",
            "PromptInputLine",
            "Confirmation",
            "ResetTerminal",
            "OpenLinkAtMouseCursor",
            "ClearScrollback.ScrollbackAndViewport",
            "ScrollByPage.Up",
            "ScrollToTop",
            "Hide",
            "ActivateTab.0",
            "ActivateTab.-1",
            "ActivateTabRelative.1",
            "MoveTabRelative.-1",
            "ActivateLastTab",
            "OpenUri.docs",
            "SplitHorizontal",
            "SplitVertical",
            "ActivatePaneDirection.Left",
            "ActivatePaneDirection.Right",
            "RotatePanes.Clockwise",
            "TogglePaneZoomState",
            "ToggleFullScreen",
            "ResetFontAndWindowSize",
            "ToggleAlwaysOnTop",
            "SetWindowLevel.Normal",
        ] {
            assert!(
                command_by_id(id).is_some_and(|c| c.is_wired()),
                "wired id missing from catalog: {id}"
            );
        }
    }
}
