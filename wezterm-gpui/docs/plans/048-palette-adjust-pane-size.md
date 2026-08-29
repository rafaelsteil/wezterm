# Palette AdjustPaneSize (048)

User after **047 user-ok** (“everything works”): continue wiring command palette items.

## This slice

Thin `call_core` rows on the **GPUI split tree** (040), leftover from 046:

- Resize pane 1 cell Left / Right / Up / Down
- Walk up to the nearest ancestor split matching the axis (mux)
- Always move that split’s **first** child: Left/Up shrinks it, Right/Down grows it
- Step is one cell in logical pixels (`TermPane::cell_px`)
- Keep a `ResizableState` entity per split id so palette resize uses the same divider as the mouse

Zoomed pane: no-op (mux). gpui-component still clamps panels to its ~100px min.

Stay listed: charselect, copy-mode, search, launcher, WSL/domains, SpawnWindow, ActivateWindow, PaneSelect overlay, reload config, primary selection.

Needs user-try: Split H or V, then palette Resize Pane 1 cell Left/Right (divider moves one cell).

**User-ok 2026-08-28.**

Record: `docs/decisions/048-palette-adjust-pane-size.json`.
