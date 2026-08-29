# 120dpi cursor gap (cell fill snap)

User 2026-08-27: on 4K @ **120dpi**, a 1px dark sliver sits between the last character and the block cursor. 1080p @ **96dpi** and official wezterm-gui have no gap.

Cause: cell backgrounds (cursor included) used `x = round(col * cell_w)` and `w = round(cell_w)` independently. At scale 1.25 those snaps can skip a pixel (`round((col+1)*w)` > `round(col*w)+round(w)`). The last letter’s cell is pane-bg (not filled), so the skip shows as a gap before the white cursor.

Same class of bug as 025 dest origin/size.

## In this slice

- Cell fills (cursor, selection, per-cell bg) and box-draw X use `cell_span`: width = `round((col+n)*cell_w) - round(col*cell_w)` so neighbors abut.
- Do **not** ceil `cell_w` to integer (wezterm-gui `RenderMetrics` does; that would change dest size / 025). **Revised in 030:** ceil is required; dest 1:1 still holds.

User 2026-08-27: **not user-ok**. Gap persisted. Follow-up: `docs/plans/030-integer-cell-grid.md`.

**User-ok 2026-08-28** (gap closed after 030).

## Out

- Integer cell metrics / `cell_width` lua
- Cursor shape (bar/underline)
- 026 monitor-move hang

Record: `docs/decisions/029-120dpi-cursor-cell-span.json`.
