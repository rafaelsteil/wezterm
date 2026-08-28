//! Pure helpers for lua chrome keys (034). No GPUI.

/// Tab label: optional `"N: "` prefix, then title, clipped to `tab_max_width` cells.
pub fn format_tab_title(
    index: usize,
    title: &str,
    show_index: bool,
    zero_based: bool,
    max_width: usize,
) -> String {
    let mut s = String::new();
    if show_index {
        let n = if zero_based { index } else { index + 1 };
        s.push_str(&format!("{n}: "));
    }
    s.push_str(title);
    let max = max_width.max(1);
    let chars: Vec<char> = s.chars().collect();
    if chars.len() <= max {
        s
    } else if max == 1 {
        "…".into()
    } else {
        chars.into_iter().take(max - 1).collect::<String>() + "…"
    }
}

pub fn show_tab_bar(tab_count: usize, enable_tab_bar: bool, hide_if_only_one: bool) -> bool {
    enable_tab_bar && !(hide_if_only_one && tab_count <= 1)
}

/// CloseCurrentTab { confirm: true } still skips when the pane is in
/// `skip_close_confirmation_for_processes_named` (cmd.exe, powershell, …).
pub fn wants_tab_close_prompt(can_close_without_prompting: bool) -> bool {
    !can_close_without_prompting
}

pub fn enable_scroll_bar() -> bool {
    config::configuration().enable_scroll_bar
}

/// Ctrl+Q / last-tab X. wezterm-gui `QuitApplication` always prompts on
/// `AlwaysPrompt`; window-close does not if every pane is skip-listed.
/// POC matches window-close so a default cmd.exe session does not prompt.
pub fn wants_quit_prompt(
    policy: config::WindowCloseConfirmation,
    all_can_close_without_prompting: bool,
) -> bool {
    match policy {
        config::WindowCloseConfirmation::NeverPrompt => false,
        config::WindowCloseConfirmation::AlwaysPrompt => !all_can_close_without_prompting,
    }
}

/// Active index after removing `closed` from `tab_count_before` tabs.
pub fn active_after_close(
    active: usize,
    last_active: Option<usize>,
    closed: usize,
    tab_count_before: usize,
    switch_to_last: bool,
) -> usize {
    let remaining = tab_count_before.saturating_sub(1);
    if remaining == 0 {
        return 0;
    }
    let adj = |i: usize| if i > closed { i - 1 } else { i };
    if closed == active && switch_to_last {
        if let Some(prev) = last_active.filter(|&i| i != closed && i < tab_count_before) {
            return adj(prev).min(remaining - 1);
        }
    }
    if active >= remaining {
        remaining - 1
    } else if active > closed {
        active - 1
    } else {
        active
    }
}

/// wezterm-gui `ActivateTab(n)`: negative indexes from the end (`-1` = last).
pub fn tab_index_from_assignment(n: isize, len: usize) -> Option<usize> {
    if len == 0 {
        return None;
    }
    let idx = if n < 0 {
        len.saturating_sub(n.unsigned_abs())
    } else {
        n as usize
    };
    (idx < len).then_some(idx)
}

/// wezterm-gui `ActivateTabRelative` with wrap.
pub fn tab_index_relative(active: usize, delta: isize, len: usize) -> Option<usize> {
    if len == 0 {
        return None;
    }
    Some((active as isize + delta).rem_euclid(len as isize) as usize)
}

/// wezterm-gui `MoveTabRelative`: clamp, no wrap.
pub fn tab_index_move_relative(active: usize, delta: isize, len: usize) -> Option<usize> {
    if len == 0 {
        return None;
    }
    let tab = active as isize + delta;
    Some(if tab < 0 {
        0
    } else if tab >= len as isize {
        len - 1
    } else {
        tab as usize
    })
}

/// Remap `last_active` after moving the tab at `from` to `to` (remove then insert).
pub fn remap_last_after_move(last: Option<usize>, from: usize, to: usize) -> Option<usize> {
    let last = last?;
    if last == from {
        return Some(to);
    }
    Some(if from < to {
        if last > from && last <= to {
            last - 1
        } else {
            last
        }
    } else if to < from {
        if last >= to && last < from {
            last + 1
        } else {
            last
        }
    } else {
        last
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn title_no_index_truncates() {
        assert_eq!(
            format_tab_title(0, "abcdefghijklmnopqrstuvwxyz0123456789", false, false, 32),
            "abcdefghijklmnopqrstuvwxyz01234…"
        );
        assert_eq!(format_tab_title(0, "cmd", false, false, 32), "cmd");
    }

    #[test]
    fn title_index_one_based() {
        assert_eq!(
            format_tab_title(0, "Command Prompt", true, false, 32),
            "1: Command Prompt"
        );
        assert_eq!(
            format_tab_title(1, "Windows PowerShell", true, true, 32),
            "1: Windows PowerShell"
        );
    }

    #[test]
    fn hide_tab_bar_matches_lua() {
        assert!(show_tab_bar(1, true, false));
        assert!(!show_tab_bar(1, true, true));
        assert!(show_tab_bar(2, true, true));
        assert!(!show_tab_bar(2, false, false));
    }

    #[test]
    fn last_active_when_closing_active() {
        // [A,B,C] active C last A → close C → A
        assert_eq!(active_after_close(2, Some(0), 2, 3, true), 0);
        // [A,B,C] active A last C → close A → C (index 1 after removal)
        assert_eq!(active_after_close(0, Some(2), 0, 3, true), 1);
        // [A,B,C] active B last A → close B → A
        assert_eq!(active_after_close(1, Some(0), 1, 3, true), 0);
        // without the flag, closing last tab selects the new last
        assert_eq!(active_after_close(2, Some(0), 2, 3, false), 1);
        // closing a non-active tab left of active
        assert_eq!(active_after_close(2, Some(0), 0, 3, true), 1);
    }

    #[test]
    fn close_prompt_skips_stateless_shells() {
        assert!(!wants_tab_close_prompt(true));
        assert!(wants_tab_close_prompt(false));
        assert!(!wants_quit_prompt(
            config::WindowCloseConfirmation::NeverPrompt,
            false
        ));
        assert!(!wants_quit_prompt(
            config::WindowCloseConfirmation::AlwaysPrompt,
            true
        ));
        assert!(wants_quit_prompt(
            config::WindowCloseConfirmation::AlwaysPrompt,
            false
        ));
    }

    #[test]
    fn tab_index_assignment_and_relative() {
        assert_eq!(tab_index_from_assignment(0, 3), Some(0));
        assert_eq!(tab_index_from_assignment(2, 3), Some(2));
        assert_eq!(tab_index_from_assignment(3, 3), None);
        assert_eq!(tab_index_from_assignment(-1, 3), Some(2));
        assert_eq!(tab_index_from_assignment(-2, 3), Some(1));
        assert_eq!(tab_index_relative(0, -1, 3), Some(2));
        assert_eq!(tab_index_relative(2, 1, 3), Some(0));
        assert_eq!(tab_index_move_relative(0, -1, 3), Some(0));
        assert_eq!(tab_index_move_relative(2, 1, 3), Some(2));
        assert_eq!(tab_index_move_relative(0, 1, 3), Some(1));
        assert_eq!(remap_last_after_move(Some(1), 0, 2), Some(0));
        assert_eq!(remap_last_after_move(Some(0), 0, 2), Some(2));
    }
}
