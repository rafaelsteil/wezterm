# Last tab `exit` quits (033)

User after 032 (“it's better now”): exiting the **last** shell/tab should **fully close the app**, like wezterm-gui (`exit_behavior = Close`, `quit_when_all_windows_are_closed = true`). First 032 try spawned a replacement tab instead.

Last-tab **X** still uses confirm-quit (process still running). `exit` on last tab does not confirm.

Record: `docs/decisions/033-last-tab-exit-quits.json`.

User 2026-08-27: **user-ok** (“works great”).
