# Skip close confirmation for stateless shells (039)

User 2026-08-27 after **038 user-ok** (dialog now shows): asking for close confirmation is **not** default wezterm. Keep the AlertDialog implementation; default to not prompting.

## wezterm-gui

- `CloseCurrentTab { confirm: true }` still skips if `Pane::can_close_without_prompting` — default `skip_close_confirmation_for_processes_named` includes `cmd.exe`, `powershell.exe`, `pwsh.exe`.
- Window close: `window_close_confirmation` default `AlwaysPrompt`, but also skips when every pane is skip-listed.
- `QuitApplication` always prompts on `AlwaysPrompt`. POC matches **window-close** so a default cmd.exe session does not prompt on Ctrl+Q either.

Dialog stays for a stateful process (vim, ssh, …) and for lua `window_close_confirmation = "AlwaysPrompt"` when the skip list does not cover the pane. Palette “Confirmation” demo unchanged.

## In this slice

- Tab X / Ctrl+W: prompt only if `!can_close_without_prompting(Tab)`.
- Ctrl+Q / last-tab X: `NeverPrompt` → quit; `AlwaysPrompt` → prompt only if some pane is stateful.

Record: `docs/decisions/039-skip-close-confirmation-for-shells.json`.

**User-ok 2026-08-28.**
