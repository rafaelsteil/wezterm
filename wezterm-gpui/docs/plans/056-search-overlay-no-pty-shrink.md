# Search overlay must not shrink the PTY (056)

User after **055** / vim: Ctrl+Shift+F search UI makes the terminal jump and duplicate.

## Symptoms

1. Editing and scrolling in vim work.
2. Opening the search bar eats the first visible row for ~1s; content jumps up as if a line was deleted, then redraws.
3. Typing in the search box duplicates some on-screen content (highlights still work). Closing search only redraws correctly after another key (Esc, arrows).

## Causes

1. The search bar was a **flex sibling** under `term-host`. That shrinks the pane by ~1 row → `resize_display` immediately, then a **450ms ConPTY `resize`**. Vim’s alt-screen loses a row, then full-redraws.
2. `set_search` jumped to `hits[0]` from a **top-down** scan (`scrollback_top` first). Typing reset `current` to 0, so the viewport jumped to an old vim frame. That looked like duplicated content.
3. `clear_search` did not restore the pre-search viewport. Live snap waited for `key_down` (`viewport = None`).

wezterm-gui paints search on the **last terminal row** (`compute_search_row`); PTY size stays put. Results are reversed **newest-first**.

## This slice

| Fix | How |
|---|---|
| No PTY shrink | Search bar is `absolute().bottom_0()` on `term-host` (covers last row). Fixed 26px, nowrap. |
| No scrollback jump on type | Reverse hits (newest first). `pick_current_hit` keeps a visible match. |
| Close redraws | `viewport_on_open` restored in `clear_search`. |

Stay parked: 026, unix Attach, lua REPL, `window/` cutover, 049 Picker search path.

**User-ok 2026-08-29** (“Fixed”).

Record: `docs/decisions/056-search-overlay-no-pty-shrink.json`.
