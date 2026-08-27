# Scrollback (user-ok; perf later)

User 2026-08-26: wheel scrolls; typing snaps to live. Performance is questionable; do not optimize unless asked.

## Why it does not work today

[`term_pane.rs`](../../src/term_pane.rs) originally always painted `dims.physical_top`. That is the **live bottom**. Mux already had scrollback. Palette “Clear scrollback” worked; the wheel did nothing. That is fixed.

gpui-terminal `on_scroll` is still a TODO. Do not copy it. tty7 scrolls alacritty `grid.display_offset` plus `scroll_frac` animation — **different model**. WezTerm does not put the GUI viewport on the VT grid.

## Copy wezterm-gui, not alacritty

[`wezterm-gui/src/termwindow/mod.rs`](../../../wezterm-gui/src/termwindow/mod.rs):

- `pane_state.viewport: Option<StableRowIndex>`
- `None` means follow `physical_top` (pinned to bottom)
- `Some(pos)` clamped to `[scrollback_top, physical_top)`; `pos >= physical_top` clears to `None`
- `scroll_by_line` / `scroll_by_page` add to that offset
- New keys: `maybe_scroll_to_bottom_for_input` if config says so
- Paint uses `get_viewport().unwrap_or(physical_top)` as the `get_lines` start

Mouse wheel in wezterm-gui: if the program grabbed the mouse or alt-screen + alternate scroll, send `MouseEvent` (`VertWheel`) into `Pane::mouse_event` (vim/less). Else GUI-scroll.

Mux already has `is_mouse_grabbed`, `is_alt_screen_active`, `mouse_event`.

## POC slice (user-ok 2026-08-26)

In `TermPane`:

1. `viewport: Option<StableRowIndex>`. `None` follows `physical_top`.
2. `visible_lines` starts at `paint_top` (clamped). Cursor drawn only if that row is on screen.
3. Outer pane `div` `on_scroll_wheel` → `on_scroll_wheel`.
4. `is_mouse_grabbed()` or `is_alt_screen_active()` → `Pane::mouse_event` WheelUp/Down. Else GUI `scroll_by_line(-y)` (Windows GPUI: positive y is wheel away / older history).
5. `key_down` that hits the PTY sets `viewport = None`.
6. No tty7 `scroll_frac`, no scrollbar.

Do not call `terminal.resize` or ConPTY for scrolling.

## Validate

`cargo check` / user try 2026-08-26: wheel up shows history, type snaps to live. **Slow** while spinning the wheel — each frame still HarfBuzz-shapes and `paint_image`s the visible grid. Defer: dirty-line cache, batched GPUI text, or GPU atlas. No scrollbar widget yet.
