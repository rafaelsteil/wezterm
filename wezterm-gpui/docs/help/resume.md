# Resume protocol (agents)

## Read order

1. `wezterm-gpui/docs/HANDOFF.md` (fresh session: read this first)
2. This file
3. `wezterm-gpui/docs/STATE.json`
4. Latest files in `docs/adr/` and `docs/decisions/` if a decision is in doubt
5. `docs/reference/` (rendering-quality is the current main goal; INDEX, open-questions, steal-list, scrollback). Checkouts `D:\dev\gpui-terminal` and `D:\dev\tty7`, not Cargo deps.
6. `docs/plans/` for the active plan (`000-feasibility-spike.md`; lua: `020-lua-config-first-slice.md`)

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

`--hello` is the button smoke window. Default is the app chrome shell (tabs + **mux LocalPane cmd.exe** + Ctrl+Shift+P palette). Paint is wezterm-font **per-line** sprites at window DPI. Loads **`~/.wezterm.lua`** (020): font, size, color scheme, scrollback, bell. Status bar shows family/size/lua file. **Ctrl+Shift+F** shows the gpui-fps HUD (off by default; continuous while visible). Drag rewraps display only; ~450ms after the grid is still, one ConPTY commit (016). Status `pty` vs `view` vs `dpi`. Palette/charselect parked. ConPTY vertical junk is later polish (also in wezterm-gui). Not the wezterm-gui glyph atlas.

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

After any material change, update `STATE.json` (`updated`, `current_phase`, `next`, `phases`, `pins`, `findings`, `blockers`). Append ADRs and decisions; never delete them.

Main goal is term paint (`docs/reference/rendering-quality.md`), not chrome. 120dpi user-ok. 017 line sprites **user-ok**. Lua config first slice (020): font/size/scheme/scrollback/bell from `~/.wezterm.lua`; still cmd.exe. **User-ok** (“works like a charm”) after BGRA + coverage blit. FPS HUD (019) **off by default**, Ctrl+Shift+F; come back later (continuous vs FRAME-for-typing). ConPTY junk also in wezterm-gui (018) — do not chase in GPUI. Zed `terminal_element.rs` is GPL-3 — do not copy. Live ConPTY **on drag** stays off (013). Do not resume palette/charselect. Do not start GPUI text unless paint feels slow again. After adding git crates, re-pin Zed with `cargo update -p gpui --precise <pins.gpui_rev>`.
