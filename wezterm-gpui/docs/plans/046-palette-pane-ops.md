# Palette pane ops (046)

User after **045 user-ok** (“all good now”): continue wiring command palette items.

## This slice

Thin `call_core` rows on the **GPUI split tree** (040), not mux `Tab`:

- Activate pane Left / Right / Up / Down (largest shared edge on equal-half geometry)
- Rotate panes Clockwise / CounterClockwise (cycle leaf identities; tree shape stays)
- Toggle pane zoom (paint only the active leaf). A new split unzooms. `unzoom_on_switch_pane` (lua, default true) unzooms before ActivatePaneDirection.

Stay listed: charselect, copy-mode, search, launcher, WSL/domains, fullscreen/always-on-top, spawn window, PaneSelect overlay, AdjustPaneSize, reload config, primary selection.

**User-ok** 2026-08-27 (“works like a charm”).

Record: `docs/decisions/046-palette-pane-ops.json`.
