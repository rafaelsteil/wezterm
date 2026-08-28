# Close this window, not the app; search copy (052)

User after **051**:

1. `exit` / last tab of one HWND closed **every** GPUI window (`cx.quit()`). Close this HWND when others exist; quit only when it was the last window (`quit_when_all_windows_are_closed`). Ctrl+Q still quits all.
2. Copy during Search still failed: `open_palette` cleared search before palette Copy, and `search_key` swallowed Ctrl+Shift+C.

Stay parked: 026, 031, `window/` cutover, debug lua REPL.

Needs user-try: two windows, `exit` in one (other stays); Ctrl+Shift+F then Ctrl+Shift+C / palette Copy.

Record: `docs/decisions/052-close-window-not-app-search-copy.json`.
