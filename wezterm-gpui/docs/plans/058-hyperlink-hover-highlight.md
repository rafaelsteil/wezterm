# Hyperlink hover underline + hand cursor (058)

034 Ctrl+click **OpenLinkAtMouseCursor** already opens the URL. User 2026-08-27: no underline / hand cursor on hover the way wezterm-gui does.

## Cause

GPUI never tracked wezterm-gui `TermWindow.current_highlight`. Mouse move only stored `last_mouse` (palette Open Link). TermScreen always set `CursorStyle::IBeam`. Line sprites have no underline decoration.

## This slice

- On mouse move: `apply_hyperlinks` + store the cell’s `Arc<Hyperlink>` (`same_hyperlink` = `Arc::ptr_eq`, like wezterm-gui).
- Overlay a 1–2px fg underline on consecutive matching cells (not baked into 017 row sprites — hover must not bust the line cache).
- `CursorStyle::PointingHand` while hovering a link; IBeam otherwise. Clear highlight when the pointer leaves the pane.
- Hover is **not** Ctrl-gated (wezterm-gui). Ctrl+click open stays 034 lua `mouse_bindings`.

Stay parked: 026, unix Attach, lua REPL, `window/` cutover, full default InputMap, powerline.

Needs user-try: hover a URL (`echo https://wezterm.org`), underline + hand cursor; leave the link, both go away; Ctrl+click still opens.

**User-ok** 2026-08-29 (hover). Plain click without Ctrl is **059**.

Record: `docs/decisions/058-hyperlink-hover-highlight.json`.
