# Palette arrow keys scroll the list (045)

User after **044 user-ok** (“all of these work”): ↑↓ in the command palette moves the highlight, but the list stays on the first page. Mouse-wheel scrolling works.

## Cause

The command list is a `div` with `.overflow_y_scroll()`. Wheel events change the overflow offset. Arrow keys only updated `selected` and `cx.notify()`, so the selected row could sit below the viewport.

## In this slice

Track a GPUI `ScrollHandle` on `#command-list`. After `move_sel` (and after query Change / reopen reset to row 0), call `scroll_to_item(selected)`. Prepaint scrolls the minimal amount so that child is fully visible (`ScrollStrategy::FirstVisible`). Immediate children of the tracked div must be the command rows.

**User-ok** 2026-08-27 (“all good now”).

Record: `docs/decisions/045-palette-arrow-scrolls-list.json`.
