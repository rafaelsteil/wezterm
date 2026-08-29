# Tab title matches new-tab dropdown (057)

User after **056**: opening a shell/domain from the chevron should set the tab title to the same string as the menu (Command Prompt, Windows PowerShell, PowerShell, WSL:Ubuntu, …).

## Cause

`TermPane::title()` used `LocalPane::get_title()`. The default pane title `"wezterm"` is replaced with the process basename (`cmd.exe`, `powershell.exe`, `wsl.exe`), so the tab bar never showed the dropdown labels. `fallback_title` was already `profile.label` but only used when the PTY title was empty/`wezterm`.

## This slice

Tab chrome uses `profile.label` (`fallback_title`). Palette **Rename tab** still wins via `ShellTab.title_override`. OSC / process titles stay out of the tab bar unless asked.

Stay parked: 026, unix Attach, lua REPL, `window/` cutover, `format-tab-title` event.

Needs user-try: Plus / chevron / WSL rows — tab text matches the menu.

Record: `docs/decisions/057-tab-title-from-dropdown.json`.
