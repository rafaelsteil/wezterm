# Honor `window_decorations` (062)

[wezterm.org `window_decorations`](https://wezterm.org/config/lua/config/window_decorations.html). User lua: `INTEGRATED_BUTTONS|RESIZE` plus `integrated_title_button_alignment = "Right"`.

034 treated this as already matched because 028 TitleBar always drew Hide/Maximize/Close on a **separate** row above the TabBar, with `appears_transparent: true` and always-resizable. That is not the lua flags.

## wezterm-gui (Windows)

`no_native_title_bar` = decorations are `RESIZE` only **or** contain `INTEGRATED_BUTTONS`.

| lua | Native caption | Resize | Buttons |
|---|---|---|---|
| `TITLE\|RESIZE` (default) | yes | yes | OS caption |
| `TITLE` | yes | no | OS caption |
| `RESIZE` | no | yes | none |
| `NONE` | no | no | none |
| `INTEGRATED_BUTTONS\|RESIZE` | no | yes | min/max/close in the **tab bar** |

## This slice

- `WindowOptions`: `appears_transparent` / `app_owns_titlebar_drag` from native vs client chrome; `is_resizable` from `RESIZE`. Load lua before the first HWND (`mux_host::ensure_init` in `app_window_options`).
- `TITLE` without `INTEGRATED_BUTTONS`: native OS caption (icon, name, min/max/close). GPUI Windows never sets `WS_CAPTION` (Zed is always client-decorated), so after HWND create we apply wezterm-gui’s style (`WS_CAPTION|WS_SYSMENU|WS_MINIMIZEBOX|WS_MAXIMIZEBOX`, plus `WS_THICKFRAME` if `RESIZE`). **No** GPUI TitleBar; TabBar below if shown.
- `INTEGRATED_BUTTONS`: client chrome; TabBar is a child of TitleBar (one row: tabs + plus + Hide/Maximize/Close). If the tab bar is hidden, keep a thin TitleBar so buttons remain.
- Empty tab-bar space (between tabs and plus) is always a window-drag region (`WindowControlArea::Drag`), including `NONE` / `RESIZE` with no caption. Matches wezterm-gui `TabBarItem::None`.
- `RESIZE` only / `NONE`: client chrome, no TitleBar, no window buttons.

Parked: `integrated_title_button_alignment` Left (TitleBar controls stay on the right on Windows), custom `integrated_title_buttons` set/order, `integrated_title_button_style` / `_color`, macOS-only flags.

Stay parked: 026, unix Attach, lua REPL, `window/` cutover.

**User-ok** 2026-08-29 (“confirmed as working”). TITLE native caption (after `WS_CAPTION`); INTEGRATED_BUTTONS one row; tab-bar empty space drags including `NONE`.

Record: `docs/decisions/062-window-decorations.json`.
