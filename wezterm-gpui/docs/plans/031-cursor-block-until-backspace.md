# Cursor block missing until backspace (031)

User 2026-08-27 after **030 user-ok**: on **new tab** or **fresh launch**, the block cursor is not visible. Typing does not show it. **Backspace** makes it appear. Then:

- **Space** hides it again
- Typing still does not show it
- Backspace → type: visible for a bit, then gone; backspace shows, type hides; repeats

User 2026-08-28: unparked (029 gap is now OK).

## Cause

Cursor was composited **into the line sprite** by walking `line.visible_cells()` and filling `cursor_bg` when `cursor_col == cell_index`. The VT cursor often sits on a column **past the last stored cell** (empty cell after the prompt, after a typed character, after space). That column is not in `visible_cells()`, so the fill never ran.

Backspace moves the cursor onto a cell that still exists → fill runs → block shows.

wezterm-gui draws the cursor as its own quad at `cursor.x * cell_width`, not by finding a cell in the line.

## This slice

- After cell fills, if the pane is focused, fill `cursor_bg` at `cursor.x` even when `visible_cells()` has no matching col (`cursor_block_col` / `fill_cursor_block`).
- Empty col width is 1 cell (`get_cell` miss → `unwrap_or(1)`). Same helper as the unfocused hollow outline.
- Do **not** add a blink timer. Do **not** change 017 skip-layout. Do **not** start 026.

Needs user-try: launch / new tab shows a block after the prompt; typing keeps it after the last char; space keeps it; backspace still shows it on the remaining cell.

**User-ok 2026-08-28** (“fixed”).

Record: `docs/decisions/031-cursor-block-until-backspace.json` (park) + `docs/decisions/031-cursor-fill-at-cursor-x.json`.
