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

GPUI already has the 027 Plus+chevron (cmd / PowerShell). `ShowLauncher` is dimmed in the palette.

## Screenshot buckets (2026-08-27)

| Row | Kind | GPUI |
|---|---|---|
| New Tab (domain `local`) | `call_core` | Wired as Plus / palette New Tab |
| New Tab (domain `WSL:…`) | `call_core` | `needs_mux` — register WslDomain |
| Attach domain `unix` | `call_core` | `needs_mux` — Unix domain |
| Create new Workspace | `call_core` | `needs_mux` |
| Reload configuration | `call_core` | Listed (live reload parked) |
| New Tab / New Window / splits / copy / font / scroll / … | overlap | See command-palette.json |

## Do not start unless asked

- A GPUI replica of the termwiz launcher list
- Registering WSL / unix domains
- Workspaces
- Honoring lua `launch_menu`
