# Resume protocol (agents)

## Read order

1. `wezterm-gpui/docs/HANDOFF.md` (fresh session: read this first)
2. This file
3. `wezterm-gpui/docs/STATE.json`
4. `wezterm-gpui/docs/lua-config.json` if the work is a lua key (per-key honor + stats)
5. `wezterm-gpui/docs/command-palette.json` / `docs/launcher.json` if the work is a palette or launcher op
6. Latest files in `docs/adr/` and `docs/decisions/` if a decision is in doubt
7. `docs/reference/` (rendering-quality is the current main goal; lua-config, command-palette, launcher, INDEX, open-questions, steal-list, scrollback). Checkouts `D:\dev\gpui-terminal` and `D:\dev\tty7`, not Cargo deps.
8. `docs/plans/` for the active plan (`000-feasibility-spike.md`; lua: `020-lua-config-first-slice.md` + `034-lua-config-second-slice.md`; palette/launcher: `036-command-palette-and-launcher-matrix.md`; selection: `021-mouse-selection-copy-paste.md`; box-draw: `023-boxdraw-and-cell-clip.md`; 4K dest: `024-hi-dpi-line-sprite-dest.md`; 120dpi: `025-120dpi-glyph-clip-and-dest-1to1.md`; monitor-move hang parked: `026-monitor-move-dpi-reshape-hang.md`; new-tab menu: `027-new-tab-shell-menu.md`; icons: `028-gpui-component-icon-assets.md`; cursor gap (insufficient): `029-120dpi-cursor-cell-span.md`; integer cell grid: `030-integer-cell-grid.md`; cursor until backspace parked: `031-cursor-block-until-backspace.md`; tab close/exit keys: `032-tab-close-exit-keys.md`; last tab `exit` quits: `033-last-tab-exit-quits.md`; dialog cancel/Ctrl+Q focus: `037-dialog-cancel-restores-term-focus.md`; dialog layer mount: `038-mount-root-dialog-layer.md`; skip close confirm: `039-skip-close-confirmation-for-shells.md`; tab/shell splits: `040-tab-shell-splits.md`; inactive pane HSB: `041-inactive-pane-hsb.md`; palette selection contrast: `042-palette-selection-contrast.md`; palette arrow keys: `043-palette-arrow-keeps-selection.md`; palette core/tab chrome: `044-palette-core-chrome.md`; palette arrow scroll: `045-palette-arrow-scrolls-list.md`; palette pane ops: `046-palette-pane-ops.md`; palette window chrome: `047-palette-window-chrome.md`)

## Commands

```powershell
cargo check --manifest-path wezterm-gpui/Cargo.toml
cargo run --manifest-path wezterm-gpui/Cargo.toml
cargo run --manifest-path wezterm-gpui/Cargo.toml -- --hello
cargo check -p wezterm
target\debug\wezterm.exe gpui
target\debug\wezterm.exe gpui --hello
```

`wezterm-gpui` is **not** a member of the root WezTerm workspace (`freetype` `links` conflict). Do not use `cargo check -p wezterm-gpui` from the repo root.

`--hello` is the button smoke window. Default is the app chrome shell (tabs + **mux LocalPane** + Ctrl+Shift+P palette). Plus / Ctrl+T spawn Command Prompt; the chevron lists cmd + Windows PowerShell + `pwsh` if installed (027). Paint is wezterm-font **per-line** sprites at window DPI; U+2500–259F is CPU geometry (023). Loads **`~/.wezterm.lua`** (020): font, size, color scheme, scrollback, bell. **034** tab chrome + user `mouse_bindings` **user-ok** except no hover highlight. Per-key: `docs/lua-config.json`. Palette catalog: `docs/command-palette.json` (036; unimplemented rows dimmed; Split H/V wired in 040). Launcher: `docs/launcher.json` (do not clone the overlay). Status bar shows family/size/lua file. **Ctrl+Shift+C/V** copy/paste (021). **Ctrl+Shift+F** shows the gpui-fps HUD (off by default; continuous while visible). Drag rewraps display only; ~450ms after the grid is still, one ConPTY commit (016). Status `pty` vs `view` vs `dpi`. Charselect / copy-mode / search overlays parked. ConPTY vertical junk is later polish (also in wezterm-gui). Not the wezterm-gui glyph atlas.

Also check default fonts after `wezterm-font` feature edits:

```powershell
cargo check -p wezterm-font
```

`wezterm gpui` spawns `wezterm-gpui` as a **child process** (search: `WEZTERM_GPUI`, next to `wezterm.exe`, then `wezterm-gpui/target/{debug,release}`). It does not start `wezterm-gui`.

Do not run workspace-wide `cargo build` or `cargo test` for this crate. Do not `cargo build --release`. From inside `wezterm-gpui/`, `cargo check` / `cargo run` also work.

## Constraints

- Existing WezTerm UI (`wezterm-gui`, `window`, box model, termwiz overlays) must keep working.
- GPUI owns its own event loop and windows. Do not embed GPUI views in a WezTerm HWND/NSView/X window in-process.
- POC shortcuts are allowed (hardcoded commands, separate process/window).
- Use Zed official GPUI git (`zed-industries/zed`), not gpui-ce, plus `gpui-component`.
- Pin git SHAs in `STATE.json` `pins` once a pair compiles.
- New plans go in `wezterm-gpui/docs/plans/`, not only `.cursor/plans/`.
- GPUI graph: only `freetype-sys` owns FreeType (`wezterm-font` `sys-freetype`). Do not crate-depend on `freetype-sys` from `gpui-freetype` or HarfBuzz (poisons the root lockfile).

## Write-back

After any material change, update `STATE.json` (`updated`, `current_phase`, `next`, `phases`, `pins`, `findings`, `blockers`). If a lua key changed, also update `docs/lua-config.json` (`stats`, `keys`, `backlog`); never delete key rows. Append ADRs and decisions; never delete them.

Main goal is term paint (`docs/reference/rendering-quality.md`), not chrome. 017 line sprites **user-ok**. Lua config first slice (020): font/size/scheme/scrollback/bell from `~/.wezterm.lua`; Plus still cmd.exe. **User-ok** (“works like a charm”) after BGRA + coverage blit. Mouse selection + copy/paste (021) **user-ok** including wrapped triple-click. **022** click-not-select + shell focus **user-ok**. **023** geometry box-draw **user-ok**; tight cell clip **reverted (025)**. **025** 4K 120dpi **user-ok**. **026** monitor-move hang **backlog** — do not start unless asked. **027** new-tab chevron (cmd + PowerShell) **user-ok**. **028** icon assets (`with_assets`) **user-ok**. **029** cursor `cell_span` **not user-ok**. **030** integer cell grid **user-ok**. **031** cursor missing until backspace **backlog** — do not start unless asked. **032** tab close/`exit` keys **better now**. **033** last-tab `exit` quits **user-ok**. **037** dialog cancel/Ctrl+Q/tab X focus **keep after dismiss**. **038** Root dialog layer **user-ok**. **040** tab/shell splits **user-ok**. **041** inactive pane HSB **user-ok**. **042** palette selected-row invert **user-ok**. **043** palette ↑↓ keep selection **user-ok**. **044** palette core/tab chrome **user-ok**. **045** palette ↑↓ scroll-into-view **user-ok**. **046** palette pane dir/rotate/zoom **user-ok**. **047** palette window chrome **user-ok**. **034** lua tab chrome + Ctrl+click/Ctrl+wheel **user-ok**; hover highlight **backlog**. Matrices: `docs/lua-config.json`, `docs/command-palette.json`, `docs/launcher.json`. FPS HUD (019) **off by default**, Ctrl+Shift+F; come back later (continuous vs FRAME-for-typing). ConPTY junk also in wezterm-gui (018) — do not chase in GPUI. Zed `terminal_element.rs` is GPL-3 — do not copy. Live ConPTY **on drag** stays off (013). Do not start charselect / copy-mode / search / cloned launcher / WSL domains unless asked. Do not start GPUI text unless paint feels slow again. After adding git crates, re-pin Zed with `cargo update -p gpui --precise <pins.gpui_rev>`.
