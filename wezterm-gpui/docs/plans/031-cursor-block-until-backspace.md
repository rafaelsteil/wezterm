# Cursor block missing until backspace (parked)

User 2026-08-27 after **030 user-ok**: on **new tab** or **fresh launch**, the block cursor is not visible. Typing does not show it. **Backspace** makes it appear. Then:

- **Space** hides it again
- Typing still does not show it
- Backspace → type: visible for a bit, then gone; backspace shows, type hides; repeats

## Status

**Backlog. Do not start** unless the user asks. Same class as 026 monitor-move hang / 018 ConPTY junk.

## Likely cause (not proven)

Cursor is composited **into the line sprite** by walking `line.visible_cells()` and filling `cursor_bg` when `cursor_col == cell_index`. The VT cursor often sits on a column **past the last stored cell** (empty cell after the prompt, after a typed character, after space). That column is not in `visible_cells()`, so the fill never runs.

Backspace moves the cursor onto a cell that still exists → fill runs → block shows.

wezterm-gui draws the cursor as its own quad at `cursor.x * cell_width`, not by finding a cell in the line.

Secondary suspect (weaker match for space/backspace): 017 skip-layout when seqno+cursor unchanged; no blink timer, so a hidden first paint would stick until the line changes.

## Out of this parked note

- Do not add a blink timer as the first guess.
- Do not change 017 skip-layout as a guess.
- Do not start 026.

Record: `docs/decisions/031-cursor-block-until-backspace.json`.
