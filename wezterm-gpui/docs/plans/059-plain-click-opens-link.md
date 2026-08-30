# Plain click opens hovered link (059)

User after **058** hover OK: wezterm-gui opens a URL with a **plain left click**. GPUI required Ctrl+click.

## Cause

034 only honored **user** `mouse_bindings`. The lua file adds Ctrl+click `OpenLinkAtMouseCursor` (and Ctrl+down `Nop`) but does **not** set `disable_default_mouse_bindings`. wezterm-gui’s default InputMap still binds unmodified / SHIFT Left Up to `CompleteSelectionOrOpenLinkAtMouseCursor`: empty selection → open `current_highlight`.

## This slice

- Unmodified (and SHIFT) left-up with **no** selection opens the hovered link (`current_highlight`, else hit cell).
- Drag-select still does not open (selection range is Some).
- Ctrl+click still opens via lua.
- Copy-on-select stays Ctrl+Shift+C (do not wire the copy half of CompleteSelection unless asked).

Stay parked: 026, unix Attach, lua REPL, `window/` cutover. **060** honors `disable_default_mouse_bindings`.

Needs user-try: hover a URL, **click** (no Ctrl) opens it; drag-select still selects.

Record: `docs/decisions/059-plain-click-opens-link.json`.
