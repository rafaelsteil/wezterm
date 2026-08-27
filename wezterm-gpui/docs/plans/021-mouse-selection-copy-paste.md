# Mouse selection + copy/paste (GPUI pane)

GUI-side selection on `TermPane`, like wezterm-gui `TermWindow.selection`. Not stored in mux. Do not change `wezterm-gui`.

## In this slice

- Left click-drag: cell range
- Double-click: word (`selection_word_boundary`; continues across wrap)
- Triple-click: wrapped **logical** line (`Pane::get_logical_lines`, same as wezterm-gui)
- Highlight via `palette.selection_bg` / `selection_fg` in line sprites (017 cache key includes the column range)
- Copy: `Ctrl+Shift+C`, `Ctrl+Insert`, palette **Copy to clipboard**
- Paste: `Ctrl+Shift+V`, `Shift+Insert`, palette **Paste from clipboard** → `Pane::send_paste`
- Mouse-grabbed apps: PTY mouse unless SHIFT (bypass, same idea as wezterm-gui)

## Out

- Lua `mouse_bindings` (Ctrl+click hyperlinks, Ctrl+wheel)
- Rectangular / block select
- Copy-on-select
- Right-click paste / context menu
- `window/` cutover, palette resume, live lua reload

Record: `docs/decisions/021-mouse-selection-copy-paste.json`.
