# New-tab profile menu (PowerShell first)

User asked for more than cmd.exe. UI from Windows Terminal: **Plus** opens the default shell; a **chevron** next to it opens a menu of available terminals. PowerShell is the first extra shell.

## In this slice

- Plus / Ctrl+T / palette New Tab: still **Command Prompt** (`%ComSpec%` / `cmd.exe`). Same as today. Lua `default_prog` stays unused (020).
- Chevron: gpui-component `DropdownButton` + `PopupMenu` listing:
  - Command Prompt
  - Windows PowerShell (`System32\WindowsPowerShell\v1.0\powershell.exe`, else `powershell.exe`)
  - PowerShell 7 (`pwsh.exe`) only if found under Program Files or PATH
- Spawn still mux `LocalDomain::spawn_pane` with a `CommandBuilder` from the profile. Tab title fallback is the profile label.
- Menu is shells only. No Settings / Command palette / About footer (palette parked). No WSL, Azure, VS developer prompts, lua `launch_menu`.

## Out

- Lua `default_prog` / `launch_menu`
- Ctrl+Shift+1…9 profile shortcuts (WT has them; not this slice)
- Brand icons per profile
- Monitor-move hang (026), Powerline, FPS HUD, ConPTY junk, palette, window/ cutover

Record: `docs/decisions/027-new-tab-shell-menu.json`.

**User-ok 2026-08-27** (“works well”) after 028 icons.

User 2026-08-28: chevron menu opened above the Plus. **User-ok** after `dropdown_menu_with_anchor(Anchor::TopRight)`.
