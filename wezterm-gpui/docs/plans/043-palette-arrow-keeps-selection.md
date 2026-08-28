# Palette arrow keys keep selection (043)

User after **042 user-ok** (“Confirmed colors work”): ↑↓ in the command palette almost immediately jumps back to the first row, as if the list were rebuilt and lost state.

## Cause

`CommandPalette` `observe`d the search `InputState` and zeroed `selected` on every notify. `InputState` notifies on caret blink (~500ms) and on `MoveUp`/`MoveDown` (the focused search field also binds those keys). AppShell’s `PaletteMoveUp`/`Down` did move the row, then the observer reset it to 0.

## In this slice

Subscribe to `InputEvent::Change` only. Typing still resets the highlight to the first match. Arrow keys and blink do not.

**User-ok** 2026-08-27 (“All good”).

Record: `docs/decisions/043-palette-arrow-keeps-selection.json`.
