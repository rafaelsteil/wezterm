# Palette core + tab chrome (044)

User after **043 user-ok** (“All good”): continue wiring command palette items.

## This slice

Thin `call_core` / `gpui_window` / `open_url` rows that do **not** need a new overlay:

- Scroll: page up/down, top, bottom; clear scrollback+viewport
- Reset terminal (RIS)
- Open link at last mouse cell
- Help URLs (docs / discussions / issues)
- Hide/minimize
- Activate tab 1–8 / last / relative; last-active tab; move tab left/right

Stay listed: charselect, copy-mode, search, launcher, WSL/domains, fullscreen/always-on-top, spawn window, pane rotate/adjust/zoom/select, reload config, primary selection.

**User-ok** 2026-08-27 (“all of these work”).

Record: `docs/decisions/044-palette-core-chrome.json`.
