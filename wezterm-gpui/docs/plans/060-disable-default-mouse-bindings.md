# Honor `disable_default_mouse_bindings` (060)

User after **059**: GPUI still opened a URL on a **plain click** even with `disable_default_mouse_bindings = true`. wezterm-gui then only highlights (058 hover) and does not open.

## Cause

059 hardcoded the default InputMap `CompleteSelectionOrOpenLinkAtMouseCursor` on unmodified / SHIFT Left Up. It treated `disable_default_mouse_bindings` as parked. wezterm-gui skips that default when the flag is true; user `mouse_bindings` (Ctrl+click `OpenLinkAtMouseCursor`) still apply.

## This slice

- When `disable_default_mouse_bindings` is true, a plain click does **not** open.
- Ctrl+click from user lua still opens.
- Hover underline + hand cursor (058) stay.
- 021 selection stays hardcoded (not a full default InputMap).

Stay parked: 026, unix Attach, lua REPL, `window/` cutover, remaining default mouse map.

Needs user-try: with the flag true, hover still underlines; **click** does not open; **Ctrl+click** still opens.

**User-ok** 2026-08-29 (“confirmed as working”).

Record: `docs/decisions/060-disable-default-mouse-bindings.json`.
