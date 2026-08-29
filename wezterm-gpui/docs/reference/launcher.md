# Launcher matrix (GPUI)

**Source of truth:** [`docs/launcher.json`](../launcher.json).

Human index only. Palette overlap lives in [`command-palette.json`](../command-palette.json), not here.

wezterm-gui **Launcher** is a termwiz overlay (`wezterm-gui/src/overlay/launcher.rs`). Right-click the tab-bar **Plus** (`ShowLauncher`). Default flags mix:

1. lua `launch_menu`
2. mux domains (New Tab / Attach)
3. workspaces
4. **all** `CommandDef`s
5. **all** InputMap key assignments

That dump is why the menu feels like a second command palette. **Do not clone it.** Unique work is domain/workspace/`launch_menu` spawn — those are mux calls once the domains exist.

GPUI already has the 027 Plus+chevron (cmd / PowerShell / **053 WSL**). **049:** `ShowLauncher` / `ShowTabNavigator` are a searchable `Picker` (shells, lua `launch_menu`, splits, new window, tabs). **053** adds mux domain rows. **055** adds workspace create/switch. Not a termwiz dump of COMMANDS+KEY_ASSIGNMENTS.

## Screenshot buckets (2026-08-27)

| Row | Kind | GPUI |
|---|---|---|
| New Tab (domain `local`) | `call_core` | Wired as Plus / palette New Tab |
| New Tab (domain `WSL:…`) | `call_core` | **053 user-ok** — `LocalDomain::new_wsl` |
| Attach domain `unix` | `call_core` | `needs_mux` — Unix domain |
| Create new Workspace | `call_core` | **055 user-ok** — hide/show HWNDs |
| Reload configuration | `call_core` | Wired (049) |
| New Tab / New Window / splits / copy / font / scroll / … | overlap | See command-palette.json (049 all Wired) |
| lua `launch_menu` | `dynamic` | Wired in ShowLauncher Picker (049) |

## Do not start unless asked

- A GPUI replica of the termwiz COMMANDS+KEY_ASSIGNMENTS dump
- A GPUI replica of the termwiz COMMANDS+KEY_ASSIGNMENTS dump
- Unix domain attach (`wezterm-client`)
- `RenameWorkspace` / lua `window:set_workspace` / `SwitchToWorkspace.spawn`
- `augment-command-palette`
