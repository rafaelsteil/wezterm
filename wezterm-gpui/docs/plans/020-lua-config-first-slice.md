# Load Lua config in wezterm-gpui (first slice)

Honor `~/.wezterm.lua` (usual WezTerm search path) in the GPUI POC without changing `wezterm-gui`.

## Apply now

- `font` / `font_size` via `FontConfiguration` + `AppShell.font_px`
- `color_scheme` via mux `TermConfig` → `pane.palette()`
- `scrollback_lines` via `TermConfig::scrollback_size`
- `audible_bell` on `MuxNotification::Alert { Bell }` (`Disabled` no-op; `SystemBeep` → `MessageBeep` on Windows)

## Do not apply

Tab bar, `window_decorations`, integrated title buttons, `max_fps`, scrollbar widget, `mouse_bindings`, command palette fonts, `default_prog`, live reload, `wezterm.on`.

## Implementation notes

- [`src/mux_host.rs`](../src/mux_host.rs): `common_init(..., skip_config=false)`. Still `CommandBuilder::new(%ComSpec%)`. Log path + `configuration_warnings_and_errors()`.
- [`src/glyph_paint.rs`](../src/glyph_paint.rs): `FontConfiguration::new(Some(configuration()), dpi)`; `sync_font` scale = `font_px / config.font_size`. Rebuild fonts when window DPI changes.
- Spawn still forces cmd.exe so a user `default_prog` cannot replace the POC shell.
- Windows lookup order is unchanged: exe-dir `wezterm.lua` still beats `~/.wezterm.lua`.

Record: `docs/decisions/020-lua-config-first-slice.json`.
