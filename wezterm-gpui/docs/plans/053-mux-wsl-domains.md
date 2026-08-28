# Mux WSL (and exec) domains (053)

User after **052** (skip remaining overlay try-list): launch other shells **registered on the machine**, especially WSL distros.

## This slice

Register mux domains the same way wezterm-gui does in `update_mux_domains` for **LocalDomain** hosts:

| Source | How |
|---|---|
| `config.wsl_domains()` / `WslDomain::default_domains()` (`wsl -l -v`) | `LocalDomain::new_wsl` + `mux.add_domain` |
| lua `exec_domains` | `LocalDomain::new_exec_domain` |

Spawn is `domain.spawn_pane(size, None, None)` so WSL builds `wsl.exe --distribution …` (not wrapping cmd.exe).

Surfaces:

- Chevron (027) lists WSL after cmd / PowerShell / pwsh. Plus / Ctrl+T stay Command Prompt.
- ShowLauncher Picker: `New Tab (domain \`WSL:…\`)`.
- Command palette: dynamic `New Tab (Domain WSL:…)` rows.

Do **not** path-dep `wezterm-mux-server-impl` (unix/SSH/TLS clients). Stay parked: unix Attach, workspaces, 026, 031, `window/` cutover.

Needs user-try: chevron WSL distro, palette “New Tab (Domain WSL:…)”, ShowLauncher domain row.

Record: `docs/decisions/053-mux-wsl-domains.json`.
