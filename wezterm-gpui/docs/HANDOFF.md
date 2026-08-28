# HANDOFF — WezTerm GPUI spike

Feed this file to a **new agent session** and say continue. Do not rely on prior chat. After reading this, also open `STATE.json` (live machine status) and follow the write-back rules at the bottom.

Repo: `D:\dev\wezterm` (Windows). Branch: `raf-gpui`.

---

## What this is

Proof of concept: can WezTerm use **Zed GPUI** + **gpui-component** for UI chrome instead of the custom window/box-model stack, without dropping the existing GUI.

Goals:

1. **Terminal/shell rendering that is actually usable** (current main goal; decision 016). Chrome does not matter until cmd.exe looks right.
2. Later: less UI code to maintain (only after a cutover) and richer widgets.

Dual-stack is expected. POC shortcuts are allowed. Palette **listing** is in (036; unimplemented rows dimmed). Charselect / copy-mode / search overlays stay **parked**.

---

## Hard constraints (do not violate)

- **Keep existing UI working.** Do not remove or break `wezterm-gui`, `window/`, box model, or termwiz overlays. Default WezTerm behavior stays the current GUI.
- **Do not share event loops in-process.** GPUI owns `GetMessage` / `NSApplication::run` / calloop. WezTerm already owns the same via `GuiFrontEnd::run_forever` → `Connection::run_message_loop()`. Two loops cannot coexist on one thread. No `from_hwnd` / embed-GPUI-in-TermWindow.
- **Zed official GPUI**, not gpui-ce, not crates.io `gpui` 0.2.2. Widgets from [gpui-component](https://longbridge.github.io/gpui-component/docs/getting-started).
- **Isolated Cargo workspace.** Never add `wezterm-gpui` to the root workspace members. It is in root `exclude`.
- **Do not put `rev` on `gpui` / `gpui_platform` in `wezterm-gpui/Cargo.toml`.** That duplicates the `gpui` crate against gpui-component’s unpinned Zed git (`Styled`/`Entity` type mismatch). Pin gpui-component with `rev`; pin Zed SHA in `wezterm-gpui/Cargo.lock` + `STATE.json` `pins`.
- **Rust validation:** `cargo check` scoped to the crate. No workspace `cargo test`, no `cargo build --release`, no `cargo clean`. Full `cargo build` only when a binary is needed (run window / `wezterm.exe`).
- New plans, ADRs, decisions live under `wezterm-gpui/docs/`, not only `.cursor/plans/`.

---

## Progress (done)

| Slice | Status |
|---|---|
| Docs tree + Cursor rule | Done |
| Isolated crate + Zed GPUI + gpui-component compile | Done (`cargo check --manifest-path wezterm-gpui/Cargo.toml`) |
| Hello window (button) | Done; **HWND verified** via `wezterm gpui --hello` title `WezTerm GPUI - Hello` |
| Command palette replica | Done; overlay in the app shell (`Ctrl+Shift+P`). Hardcoded commands; some mutate POC chrome (tabs, font size, quit). User ran commands from it. |
| `wezterm gpui` child-process launch | Done; `cargo build -p wezterm` + E2E spawn of isolated `wezterm-gpui.exe`. Default `wezterm-gui` not started. |
| App chrome shell | Done; **user-tried and approved** (`target\debug\wezterm.exe gpui`). TitleBar + TabBar + palette overlay. |
| Confirm / prompt dialog | Done; gpui-component `AlertDialog` + `Dialog`+`Input`. Close tab / last-tab-as-quit / Ctrl+Q. |
| Live PTY in chrome | Superseded by mux `LocalPane` (same crates, homemade host). |
| mux LocalDomain cmd.exe | Done. **User-tried** 2026-08-26. Spawn is `LocalDomain::spawn_pane` + `%ComSpec%`/`cmd.exe`. |
| GPUI-owned FreeType | Done compile. Isolated graph uses only `freetype-sys` 0.20.1. `wezterm-font` path-dep with `sys-freetype`. wezterm-gui still vendors `deps/freetype`. |
| wezterm-font cell paint | Done. User-tried 2026-08-26 (cmd.exe + JetBrains). Viewport bitmap went black on shrink-grow (010). Per-glyph `paint_image` was too slow (user 2026-08-26). Now **cached per-line** sprites (017) + DPI/snap (016). Consolas fallback. **Not the wezterm-gui GPU atlas.** |
| ConPTY live resize | Display-only drag (013) + 450ms committed ConPTY (016). Vertical junk **also happens in official wezterm-gui** — later polish (018), not a GPUI-only bug. |
| External GPUI terminals | Done (docs only). Cloned `D:\dev\gpui-terminal` and `D:\dev\tty7`. Notes in `docs/reference/`. No path deps. |
| Scrollback viewport | Done. **User-tried** 2026-08-26 (decision 015): wheel history, typing snaps to live. No scrollbar widget. |
| Term paint quality (016) | DPI **user-ok** at 120dpi. Device-pixel snap + committed ConPTY in. |
| Paint lag (017) | Done. **User-tried** 2026-08-26: “much better now.” Line sprites + mux coalesce + seqno skip. Still debug vs official release; not the wezterm-gui GPU atlas. |
| gpui-fps HUD (019) | Wired, **off by default**. **Ctrl+Shift+F** shows it. Continuous while visible. Come back later to measure (and maybe add a non-continuous mode). Click HUD to compact. |
| Lua config (020) | First slice: load `~/.wezterm.lua`. Font/size/scheme/scrollback/bell. Still forces `cmd.exe`. **User-ok** 2026-08-27 (“works like a charm”) after BGRA swap + coverage blit. |
| Mouse selection + copy/paste (021) | **User-ok** 2026-08-27. Drag/copy/paste. Wrapped triple-click after `get_logical_lines` (“works”). **022 user-ok:** click is not a 1-col selection; typing works on launch (no right-click needed). |
| Geometry box-draw + per-cell clip (023) | **User-ok** 2026-08-27 (`echo` / `tree`) for box-draw. Tight cell clip **reverted (025)**. Powerline triangles out. |
| 4K line-sprite dest (024) | **Not user-ok.** Horizontal slivers first; retry at 120dpi was vertical glyph slivers (`<DIR>` → `<CIF>`). 1080p 96dpi OK. |
| 120dpi clip + dest 1:1 (025) | **User-ok** 2026-08-27 on 4K 120dpi (96dpi still OK). Tight cell clip off; dest 1:1. |
| Monitor-move hang (026) | **Backlog.** 2–3s hang when dragging between monitors. Do not start unless asked. |
| New-tab shell menu (027) | **User-ok** 2026-08-27 (“works well”). Plus / Ctrl+T Command Prompt; chevron lists cmd + Windows PowerShell + `pwsh` if installed. |
| GPUI icon assets (028) | **User-ok** 2026-08-27 (“works well”). `gpui-component-assets` + `with_assets`. Plus / tab close / title-bar min/max/close. |
| 120dpi cursor gap (029) | **Not user-ok.** `cell_span` only changed fill width; gap persisted. |
| Integer cell grid (030) | **User-ok** 2026-08-27 (“works great now”). Ceil cell size + glyphs step `num_cells * cell_w`. |
| Cursor missing until backspace (031) | **Backlog.** Block cursor hidden on launch/new tab; typing no; backspace yes; space hides. Do not start unless asked. |
| Tab close / `exit` kills typing (032) | **Better now.** Close tab on pane death; restore AppShell focus. |
| Last tab `exit` quits (033) | **User-ok** 2026-08-27 (“works great”). Last-tab process exit calls `cx.quit()`. |
| Dialog cancel / Ctrl+Q / tab X (037) | Wired restore after dismiss. User: **no dialog shown** (038). |
| Mount Root dialog layer (038) | **User-ok** 2026-08-27 (“great, that works now”). |
| Skip close confirm for shells (039) | Wired. cmd.exe / skip list does not prompt; AlertDialog kept for stateful panes. Needs user-try. |
| Tab/shell splits (040) | **User-ok** 2026-08-27 (“works great”). Palette Split H/V. |
| Inactive pane HSB (041) | **User-ok** 2026-08-27 (“perfect”). lua `inactive_pane_hsb` + hollow unfocused cursor. |
| Palette selection contrast (042) | **User-ok** 2026-08-27 (“Confirmed colors work”). Selected row inverts lua `command_palette_fg/bg`. |
| Palette arrow keys keep selection (043) | **User-ok** 2026-08-27 (“All good”). Subscribe `InputEvent::Change` instead of observing `InputState`. |
| Palette core + tab chrome (044) | **User-ok** 2026-08-27 (“all of these work”). 41 catalog rows (scroll, reset, open link, Help URLs, minimize, ActivateTab/MoveTab). Overlays still dimmed. |
| Palette arrow keys scroll the list (045) | **User-ok** 2026-08-27 (“all good now”). `ScrollHandle` on `#command-list`; `scroll_to_item(selected)` after ↑↓. |
| Palette pane ops (046) | **User-ok** 2026-08-27 (“works like a charm”). ActivatePaneDirection / RotatePanes / TogglePaneZoom on the GPUI split tree. |
| Palette window chrome (047) | **User-ok** 2026-08-27 (“everything works”). ToggleFullScreen, ResetFontAndWindowSize, AlwaysOnTop/Bottom (Win32 HWND_TOPMOST). |
| Lua config second slice (034) | **User-ok** 2026-08-27 (tab chrome + Ctrl+click open + Ctrl+wheel). **Partial:** no hyperlink hover highlight (backlog). Fancy tab bar / decorations: no visible change. `adjust_window_size` not tested. Matrix: `docs/lua-config.json`. |
| Command palette + launcher matrix (036) | Tracking JSON + full Windows default list in the GPUI palette. Wired rows run; the rest render **dimmed**. Launcher overlay **not cloned**. Matrices: `docs/command-palette.json`, `docs/launcher.json`. |

`go_nogo`: in-process embed = **no**. Continue POC = **yes**. Runtime window = **yes**. CLI spawn = **yes**. Min usable chrome = **yes**. Mux cmd.exe = **user-tried**. Paint = **line sprites user-ok (017)**; **025 4K 120dpi user-ok**. FPS HUD = **wired, off by default (019)**. Lua config = **user-ok (020; Cascadia + Dracula)**; **034 user-ok** except hover highlight. Selection/copy/paste = **user-ok (021, including wrap)**. 022 click/focus = **user-ok**. 023 box-draw = **user-ok** (tight clip reverted). 026 monitor-move hang = **backlog**. **027 shell menu = user-ok.** **028 icons = user-ok.** **029 cursor gap = not user-ok.** **030 integer cell grid = user-ok.** **031 cursor until backspace = backlog.** **032 tab close/exit keys = better now.** **033 last-tab `exit` quits = user-ok.** **037 dialog cancel focus = keep after dismiss.** **038 dialog layer mounted = user-ok.** **039 skip close confirm for skip-listed shells = needs user-try.** **040 splits = user-ok.** **041 inactive pane HSB = user-ok.** **042 palette selection invert = user-ok.** **043 palette ↑↓ keep selection = user-ok.** **044 palette core/tab chrome = user-ok (41 wired).** **045 palette ↑↓ scroll-into-view = user-ok.** **046 palette pane dir/rotate/zoom = user-ok.** **047 palette window chrome = user-ok.** Glyph atlas Element = **not started**. Scrollback = **user-ok**. Palette = **036 listing in (dimmed NYI)**; Split H/V **wired (040)**; charselect/copy-mode/search **parked**. ConPTY junk = **shared with wezterm-gui, polish later**.

Pins:

- Zed GPUI: `fecc3273ed32643c2ea1b04a74c8780e2c9ffaf8` (lockfile)
- gpui-component: `ff3eb1128ac1058f1bb88e777744ce1237aa3b79` (`Cargo.toml` `rev`)
- rustc used: `1.97.1` `x86_64-pc-windows-msvc` (gpui-component wants 1.90+)

---

## Next steps (pick with the user unless they already said)

Default `wezterm gpui` hosts **cmd.exe** through mux (Plus / Ctrl+T). Chevron next to Plus lists Command Prompt, Windows PowerShell, and `pwsh` if installed (027) — **user-ok**. Icons need `gpui-component-assets` (028) — **user-ok**. Paint is **one cached GPUI image per line** (wezterm-font composite, 017) at window DPI — **user-ok** (“much better”). Live drag rewraps display only; ~450ms later one ConPTY `resize`. Loads **`~/.wezterm.lua`** for font/size/scheme/scrollback/bell (020) — **user-ok** (“works like a charm”). **034** tab chrome + that file’s `mouse_bindings` — **user-ok** except no hyperlink hover highlight (backlog). Per-key matrix: **`docs/lua-config.json`**. Mouse **selection + copy/paste** (021) **user-ok** including wrapped triple-click. **023 user-ok:** box-draw (`echo` / `tree`); tight cell clip **reverted (025)**. **025 user-ok:** 4K 120dpi glyphs. **029 not user-ok** (`cell_span`). **030 user-ok:** integer cell grid (“works great now”). **026 backlog:** 2–3s hang when moving the window between monitors. **031 backlog:** block cursor missing until backspace (launch/new tab; space hides). **032:** tab X / `exit` left typing dead — **better now**. **033 user-ok:** last-tab `exit` quits the app (“works great”). **037:** Ctrl+Q / tab X **Cancel** still killed typing (032 only restored after confirm-close OK). **038:** Ctrl+Q showed **no dialog** — AppShell never painted `Root::render_dialog_layer` — **user-ok**. **039:** close confirm is not default wezterm — skip `cmd.exe` / skip list; keep AlertDialog. **Needs user-try.** **040 user-ok:** tab/shell splits via palette Split H/V (“works great”). **041 user-ok:** inactive pane HSB + hollow cursor (“perfect”). **042 user-ok:** palette selected row inverts lua fg/bg (“Confirmed colors work”). **043 user-ok:** ↑↓ in the palette keep the highlight (“All good”). **044 user-ok:** more palette rows run (scroll, reset, tabs, Help, minimize) (“all of these work”). **045 user-ok:** ↑↓ in the palette scroll the list into view (“all good now”). **046 user-ok:** palette Activate Pane / Rotate / Toggle Zoom on splits (“works like a charm”). **047 user-ok:** Toggle full screen / Always on Top / Reset font+window size (“everything works”). **Do not start** 026/031 unless asked.

Do **not** start a `window/` cutover unless the user explicitly asks.
Do **not** start character selector / copy mode / search overlay / a cloned launcher unless asked. Palette **listing** is 036; 047 wired 55 rows (fullscreen/z-order/reset size); Split H/V **wired (040)**.
Do **not** investigate ConPTY vertical junk in this POC — also happens in official wezterm-gui (018).
Do **not** investigate the monitor-move hang (026) unless asked.
Do **not** investigate the missing cursor until backspace (031) unless asked.
Do **not** start a GPUI `text_system` rewrite unless paint feels slow again.
Do **not** expand lua to `enable_scroll_bar` / `max_fps` / palette fonts / live reload / `default_prog` / `launch_menu` / hyperlink hover highlight unless asked.

Workstream: `docs/reference/rendering-quality.md`. Lua matrix: `docs/lua-config.json` + `docs/reference/lua-config.md`. Palette matrix: `docs/command-palette.json` + `docs/reference/command-palette.md`. Launcher matrix: `docs/launcher.json` + `docs/reference/launcher.md`. Lua slices: `docs/plans/020-lua-config-first-slice.md` + `docs/plans/034-lua-config-second-slice.md`. Selection: `docs/plans/021-mouse-selection-copy-paste.md`. Box-draw: `docs/plans/023-boxdraw-and-cell-clip.md`. 4K dest: `docs/plans/024-hi-dpi-line-sprite-dest.md`. 120dpi clip/dest: `docs/plans/025-120dpi-glyph-clip-and-dest-1to1.md`. Monitor-move hang (parked): `docs/plans/026-monitor-move-dpi-reshape-hang.md`. New-tab menu: `docs/plans/027-new-tab-shell-menu.md`. Icons: `docs/plans/028-gpui-component-icon-assets.md`. Cursor gap (insufficient): `docs/plans/029-120dpi-cursor-cell-span.md`. Integer cell grid: `docs/plans/030-integer-cell-grid.md`. Cursor until backspace (parked): `docs/plans/031-cursor-block-until-backspace.md`. Tab close/exit keys: `docs/plans/032-tab-close-exit-keys.md`. Last tab `exit` quits: `docs/plans/033-last-tab-exit-quits.md`. Palette+launcher matrix: `docs/plans/036-command-palette-and-launcher-matrix.md`. Dialog cancel/Ctrl+Q focus: `docs/plans/037-dialog-cancel-restores-term-focus.md`. Dialog layer mount: `docs/plans/038-mount-root-dialog-layer.md`. Skip close confirm: `docs/plans/039-skip-close-confirmation-for-shells.md`. Tab/shell splits: `docs/plans/040-tab-shell-splits.md`. Inactive pane HSB: `docs/plans/041-inactive-pane-hsb.md`. Palette selection contrast: `docs/plans/042-palette-selection-contrast.md`. Palette arrow keys: `docs/plans/043-palette-arrow-keeps-selection.md`. Palette core/tab chrome: `docs/plans/044-palette-core-chrome.md`. Palette arrow scroll: `docs/plans/045-palette-arrow-scrolls-list.md`. Palette pane ops: `docs/plans/046-palette-pane-ops.md`. Palette window chrome: `docs/plans/047-palette-window-chrome.md`.

Reasonable continuations, smallest first:

1. **039** skip-confirm still needs a try if not done. Wait for the next palette/bug list; do not invent a slice.
2. Come back later: FPS HUD (Ctrl+Shift+F). Continuous = sustain rate; for typing lag we still want a non-continuous FRAME mode (not wired).
3. Come back later: monitor-move hang (026) — 2–3s when dragging between 96dpi and 120dpi monitors.
4. Come back later: cursor missing until backspace (031) — launch/new tab; space hides it.
5. GPUI `text_system` spike only if they ask for more speed (`docs/reference/gpui-text-vs-sprites.md`).
6. **Only if asked:** windowing cutover.

If the user just says “continue”: wait for the next bug list; do not invent a new slice; do **not** start 026, 031, charselect, copy-mode, search, or hyperlink hover.

---

## Architecture (why GPUI cannot eat WezTerm incrementally in-process)

WezTerm has **three** UI layers:

| Layer | Where | GPUI role |
|---|---|---|
| Native windowing | `window/` (Win32, Cocoa, X11, Wayland, EGL/WGL) | Replaced only at full cutover by `gpui_platform` |
| Box model chrome | `wezterm-gui/src/termwindow/box_model.rs` + fancy tab bar, window buttons, `Modal`s | Natural gpui-component target |
| Box-model modals | `palette.rs`, `charselect.rs`, `paneselect.rs` | First incremental replacements (sibling windows today) |
| Termwiz overlays | `wezterm-gui/src/overlay/` (launcher, copy, debug, confirm, …) | Dialog/Input (confirm+prompt POC done; rest later) |
| Terminal cells | glyph cache, glium + optional wgpu 25 | Custom GPUI `Element` later. **POC now:** mux `LocalPane` (cmd / PowerShell) + wezterm-font sprites in `glyph_paint.rs` |

`use_box_model_render` is an experimental pane paint path. Ignore it as a migration vehicle.

GPUI is an **application framework**: `gpui_platform::application().run` takes the native loop and `cx.open_window` creates native windows. Embedding a GPUI view in a WezTerm window needs a custom `Platform` (new backend), not a flag.

Until cutover: GPUI UIs = **sibling process/window**. `wezterm gpui` is that pattern.

---

## Tree

```
wezterm-gpui/                    # OWN Cargo workspace (excluded from root)
  Cargo.toml                     # wezterm-font sys-freetype + explicit freetype-sys
  Cargo.lock                     # pins Zed SHA; commit this
  src/main.rs                    # --hello vs default app shell; sets window title
  src/lib.rs
  src/hello.rs                   # Button smoke view
  src/shell.rs                   # TitleBar + tabs + Plus/chevron shell menu + mux TermPane + palette + gpui-fps HUD
  src/commands.rs                # Windows default palette catalog (036); wired vs listed
  src/palette.rs                 # Input + filtered catalog; dimmed NYI rows; 042 selected invert; 043 Change-only reset; 045 ScrollHandle
  src/split_layout.rs            # GPUI pane tree (040); 046 dir/rotate/zoom
  src/win_zorder.rs              # 047 Win32 HWND_TOPMOST / HWND_BOTTOM
  src/confirm.rs                 # AlertDialog confirm + Dialog+Input line prompt
  src/mux_host.rs                # config + Mux + LocalDomain; load lua; spawn CommandBuilder
  src/lua_ui.rs                  # tab title / last-active helpers (034)
  src/shells.rs                  # cmd / Windows PowerShell / optional pwsh profiles (027)
  src/term_pane.rs               # mux LocalPane; wezterm-font paint or Consolas fallback; GUI selection (021)
  src/glyph_paint.rs             # wezterm-font → cached per-line RenderImages (017); selection tint; 025 dest 1:1; 029 cell_span; 030 integer grid
  src/boxdraw.rs                 # U+2500–259F CPU geometry into the row bitmap (023)
  docs/
    HANDOFF.md                   # this file
    STATE.json                   # live phase/next/pins/findings (machine)
    lua-config.json              # per-key lua honor + stats (035)
    command-palette.json         # per-command palette honor (036)
    launcher.json                # launcher sources + unique ops (036)
    help/resume.md               # short command/constraint cheat sheet
    plans/000-feasibility-spike.md
    plans/020-lua-config-first-slice.md
    plans/021-mouse-selection-copy-paste.md
    plans/023-boxdraw-and-cell-clip.md
    plans/024-hi-dpi-line-sprite-dest.md
    plans/025-120dpi-glyph-clip-and-dest-1to1.md
    plans/026-monitor-move-dpi-reshape-hang.md
    plans/027-new-tab-shell-menu.md
    plans/028-gpui-component-icon-assets.md
    plans/029-120dpi-cursor-cell-span.md
    plans/030-integer-cell-grid.md
    plans/031-cursor-block-until-backspace.md
    plans/032-tab-close-exit-keys.md
    plans/033-last-tab-exit-quits.md
    plans/034-lua-config-second-slice.md
    plans/036-command-palette-and-launcher-matrix.md
    plans/037-dialog-cancel-restores-term-focus.md
    plans/038-mount-root-dialog-layer.md
    plans/039-skip-close-confirmation-for-shells.md
    plans/040-tab-shell-splits.md
    plans/041-inactive-pane-hsb.md
    plans/042-palette-selection-contrast.md
    plans/043-palette-arrow-keeps-selection.md
    plans/044-palette-core-chrome.md
    plans/045-palette-arrow-scrolls-list.md
    plans/046-palette-pane-ops.md
    plans/047-palette-window-chrome.md
    adr/0001-use-zed-official-gpui.md
    adr/0002-isolated-cargo-workspace.md
    adr/0003-gpui-owns-freetype.md
    decisions/*.json
    reference/INDEX.md           # gpui-terminal + tty7 (D:\dev checkouts; not path deps)
    reference/rendering-quality.md  # current main goal (decision 016)
    reference/open-questions.md  # live PTY resize + GPUI text: re-eval later
    reference/scrollback.md      # viewport slice (decision 015; user-ok)
    reference/lua-config.md      # human index for lua-config.json
    reference/command-palette.md # human index for command-palette.json
    reference/launcher.md        # human index for launcher.json

deps/freetype-from-sys/          # WezTerm FT bindgen; no links=; GPUI owns C lib

wezterm/src/gpui_launch.rs       # `wezterm gpui` → spawn wezterm-gpui
wezterm/src/main.rs              # SubCommand::Gpui (does NOT delegate_to_gui)

.cursor/rules/gpui-migration.mdc
.cursor/rules/rust-fast-development.mdc

Root Cargo.toml exclude includes "wezterm-gpui"
.gitignore has /wezterm-gpui/target/
```

Legacy palette (untouched): `wezterm-gui/src/termwindow/palette.rs`.
Legacy confirm/prompt (untouched): `wezterm-gui/src/overlay/confirm.rs`, `overlay/prompt.rs`.

---

## Commands

```powershell
# GPUI crate (isolated — NOT cargo check -p wezterm-gpui)
cargo check --manifest-path wezterm-gpui/Cargo.toml
cargo run --manifest-path wezterm-gpui/Cargo.toml
cargo run --manifest-path wezterm-gpui/Cargo.toml -- --hello

# CLI launcher (root workspace)
cargo check -p wezterm
# needs a linked wezterm.exe:
cargo build -p wezterm
target\debug\wezterm.exe gpui
target\debug\wezterm.exe gpui --hello
```

Verified launch on Windows: `target\debug\wezterm.exe gpui` (user-approved). Rebuild `wezterm-gpui` after UI edits or that exe is stale.

Binary lookup for `wezterm gpui`: env `WEZTERM_GPUI`, then next to `wezterm.exe`, then `wezterm-gpui/target/{debug,release}/wezterm-gpui.exe` relative to exe/cwd. Isolated target dir is `wezterm-gpui/target/debug/`, **not** repo-root `target/debug/`.

---

## Pitfalls already paid for

1. **`links = "freetype"`** — root workspace cannot contain both WezTerm `deps/freetype` and GPUI `freetype-sys`. Isolated workspace is mandatory. Inside *this* graph, only `freetype-sys` may own FreeType (`wezterm-font` feature `sys-freetype`). Do not depend on `freetype-sys` from `gpui-freetype` or HarfBuzz: Cargo would then lock it next to vendored FT in the **root** lockfile.
2. **`rev` on `gpui` git dep** — two `gpui` crates in one graph. Never again.
3. **gpui-component has no Command palette widget** — POC uses `Input` + clickable rows. Notifications: `WindowExt` + `Notification::info`.
4. **Unpinned Zed git vs gpui-component git** will drift. After a successful compile, copy SHAs into `STATE.json` `pins` and keep `Cargo.lock`.
5. First GPUI resolve is huge (~844 crates). Incremental `cargo check` after that is fast.
6. **Dialog builders are `Fn`**, re-run each render. Capture titles/callbacks via `Clone`/`Rc`. `Dialog` does not auto-footer from `button_props`; `AlertDialog` does. Prompt uses `DialogFooter` + `DialogClose` + `DialogAction`.
7. **Path-dep `mux` is OK** from the isolated workspace (pulls config/lua). Path-dep **`wezterm-font` with `sys-freetype` + `vendor-jetbrains`** (ADR 0003). `sys-freetype` alone has no bundled faces; default config is JetBrains Mono, so paint then fails with `metrics_for_idx: there is no font with idx=0` and falls back to Consolas. Still do **not** path-dep `wezterm-gui` or `window`. Mux PTY threads use `promise::spawn`; GPUI owns the native loop, so the POC runs `promise::spawn::SimpleExecutor` on a side thread — not `window/`'s spawn queue.
8. **HarfBuzz `workspace = true` + `default-features = false` is ignored.** `wezterm-font` must path-dep `deps/harfbuzz` with `default-features = false` or vendored FT re-enters the GPUI graph.
9. **Unix `fontconfig` `links`** — WezTerm `deps/fontconfig` vs GPUI `yeslogic-fontconfig-sys`. Keep `native-fontconfig` off in wezterm-gpui.
10. **Do not call `LocalPane::resize` (ConPTY) on live GPUI drag.** A few pixels of width is one column; `ResizePseudoConsole` smears the cursor column. wezterm-gui mostly avoids this via Win32 cell-sized resize increments. POC: first valid pane size still resizes the PTY; later **drag** is `Pane::resize_display` (decision 013). After the view grid is unchanged ~450ms, **one** committed ConPTY `resize` (decision 016) so new output can wrap at the new width. Skip sub-2-cell bounds (decision 012). Status shows `pty` vs `view` vs `dpi`.
11. **External GPUI terminals** — `D:\dev\gpui-terminal` and `D:\dev\tty7`. Notes: `docs/reference/` (including open-questions, GPUI-text vs sprites, scrollback). Do not path-dep, do not switch to `alacritty_terminal`. Zed `terminal_element.rs` is GPL-3: ideas only.
12. **Adding a gpui-component git crate re-resolves unpinned Zed.** `gpui-fps` at the same `rev` as `gpui-component` is fine, but `cargo check` walked Zed to a newer SHA. Pin back with `cargo update -p gpui --precise fecc3273ed32643c2ea1b04a74c8780e2c9ffaf8` (the `STATE.json` `pins.gpui_rev`). Do not put `rev` on the `gpui` dep (pitfall 2).
13. **Lua config (020 + 034).** `common_init` used to skip `wezterm.lua` so `default_prog` could not replace cmd.exe. Now the file loads; Plus / Ctrl+T still spawn Command Prompt. Chevron can spawn PowerShell (027). On Windows, `wezterm.lua` next to the exe still wins over `~/.wezterm.lua`. Unknown lua fields reject the whole file (same as wezterm-gui) → defaults. 034 honors tab chrome + **user** `mouse_bindings` only (`OpenLinkAtMouseCursor`, `Nop`, `ScrollByPage`) — not wezterm-gui’s default InputMap. Ctrl+click **opens** but has **no hover underline** (backlog; do not start unless asked). Per-key status: `docs/lua-config.json`. Live reload / `wezterm.on` / `launch_menu` / scrollbar / `max_fps` / palette fonts stay out until asked. Open URL uses `wezterm-open-url` on a side thread (no `open-uri` event).
14. **GPUI `RenderImage` is BGRA.** Line sprites must swap R/B before `Frame::new`. RGBA upload made Dracula `#282a36` look brown (`#362a28`). `gpui::rgb(0xRRGGBB)` for quads is already correct.
15. **Do not premultiply glyph coverage then multiply alpha again.** That is `fg * alpha²` and Cascadia looks thin vs wezterm-gui. Cache FreeType coverage; blit `sRGB_fg * linear_a + bg * (1-a)` like the glyph shader.
16. **TermScreen left-click `stop_propagation` vs AppShell `track_focus`.** Swallowing the press before the shell focuses means typing stays dead until a right-click (unhandled, bubbles). Focus AppShell from the pane press (and at window open). Cell selection: a MouseMove on the press with start==end paints a 1-col box; keep range None until the hit cell leaves origin (wezterm-gui).
17. **Box-draw is CPU geometry in the line sprite, not GPUI `Path`.** wezterm-gui `customglyph.rs` is tiny-skia + the GPU atlas; do not path-dep `wezterm-gui`. tty7 `boxdraw.rs` is Apache — rewrite into pixels. Honor `custom_block_glyphs`. Powerline (U+E0B0) is not this slice.
18. **4K line sprites (024).** `paint_image(pane, row_dest)` is object-fit. Dest must be `image_device_px / window.scale_factor()` and passed as both args, or high-DPI rows paint as horizontal slivers. Other (1x) screens can look fine.
19. **120dpi vertical glyph slivers (025).** Tight 023 clip to `cell_w` cut FreeType LCD/bearings at scale 1.25 (`<DIR>` → `<CIF>`); 96dpi hid it. Clip is row-bitmap only. Also lock dest origin in device px so `snap_bounds` width equals the bitmap (independent origin/size snaps skip columns at 1.25).
20. **Monitor-move hang (026, backlog).** 2–3s when dragging between 96dpi and 120dpi. `sync_font` drops glyph+row caches on DPI change, then every visible line is reshaped on the UI thread. Do not start unless asked.
21. **New-tab menu is gpui-component `DropdownButton`, not a custom overlay.** Plus is `shells[0]` (Command Prompt). Chevron `PopupMenu` items call `spawn_profile`. Do not honor lua `launch_menu` / `default_prog` unless asked. WT Settings/About footer and Ctrl+Shift+1…9 shortcuts are out.
22. **gpui-component does not embed icon SVGs.** `IconName` is a path (`icons/plus.svg`). Without `gpui-component-assets` + `application().with_assets(Assets)`, Plus, tab close, dropdown caret, and TitleBar min/max/close are empty. Native Windows caption is off (`appears_transparent`). Same git `rev` as gpui-component.
23. **120dpi cursor 1px gap (029, not sufficient).** Cell bg/cursor used `round(col*cell_w)+round(cell_w)`. At 1.25 those snaps can skip a pixel. `cell_span` abuts fills but does **not** move the cursor’s left edge. User: problem persists.
24. **Integer cell grid (030).** wezterm-gui `RenderMetrics` ceils cell size; glyphs step `num_cells * cell_width`, not HarfBuzz `x_advance`. Fractional metrics + accumulated advance sit left of `round(col*cell_w)`. Dest 1:1 (025) still holds after ceil (bitmap size changes; dest = image px / scale). Do not skip ceil to “protect” dest size.
25. **Cursor missing until backspace (031, backlog).** Block cursor hidden on launch/new tab; typing does not show it; backspace does; space hides. Cursor fill walks `visible_cells()`; VT cursor often sits past the last stored cell. wezterm-gui paints a cursor quad at `cursor.x`. Do not start unless asked.
26. **Tab close / `exit` kills typing (032).** After tab X, dialog restores a destroyed Close-button focus handle so AppShell never sees keys (Plus still clicks). After `exit`, the ShellTab kept a mux-removed pane; `key_down` wrote ConPTY. Close the tab on `TermPaneEvent::Exited`. `request_terminal_focus` after close/new tab. Mux subscriber `try_send`. User: better now.
28. **Palette catalog is a copy of `CommandDef`, not a wezterm-gui path-dep.** `PALETTE_COMMANDS` in `wezterm-gpui/src/commands.rs` matches Windows `compute_default_actions`. Status lives in `docs/command-palette.json`. Launcher is a termwiz overlay that also dumps those commands; unique ops (WSL/unix/workspaces) are `docs/launcher.json`. Do not clone the overlay. Do not honor `launch_menu` / `augment-command-palette` unless asked.
29. **Dialog Cancel / Ctrl+Q still killed typing after 032 (037).** 032 only called `request_terminal_focus` after confirm-close **OK**. Cancel restores the tab Close button; `confirm_quit` had no restore. Focus AppShell before `open_confirm`. Restore on `on_ok` **and** `on_cancel` (AlertDialog `.on_close` is wiped by `build_surface`). Delayed retry 400ms with `spawn_in` + `focus_terminal`.
30. **Root does not paint dialogs (038).** `open_alert_dialog` pushes `active_dialogs` and focuses them; `Root::render` does not draw that list. AppShell must call `Root::render_dialog_layer` (plus sheet/notification). Without it, Ctrl+Q / tab X look like no-ops while `has_active_dialog` stays true and typing dies.
31. **Close confirm is skip-listed for cmd.exe (039).** wezterm `CloseCurrentTab { confirm: true }` still does not prompt for `cmd.exe` / PowerShell (`skip_close_confirmation_for_processes_named`). Keep AlertDialog; skip when `can_close_without_prompting`. Ctrl+Q matches window-close, not `QuitApplication` always-prompt.
32. **Splits are a GPUI tree, not mux Tab (040).** POC `spawn_pane` does not put panes in a mux `Tab`, so `Mux::split_pane` cannot run. Binary tree of `TermPane` + `h_resizable`/`v_resizable`. Palette Split H/V wired. Click focuses the leaf. **User-ok** (“works great”).
33. **Inactive pane HSB is CPU on line sprites (041).** wezterm-gui `inactive_pane_hsb` (default sat 0.9 / bri 0.8) is a GPU HSV multiply. GPUI applies the same transform after compositing the 017 row bitmap. Unfocused cursor is a hollow `cursor_border` outline. **User-ok** (“perfect”).
34. **Palette selected row inverts lua fg/bg (042).** GPUI used `theme.accent` @ 0.22; on this theme accent ≈ foreground so the highlight vanished. wezterm-gui inverts `command_palette_fg_color` / `command_palette_bg_color`. Honor those keys; selected row is a solid invert. **User-ok** (“Confirmed colors work”).
35. **Palette ↑↓ must not observe InputState (043).** Input notifies on caret blink and arrow-key cursor moves. Observing it zeroed `selected` after every AppShell `PaletteMoveUp`/`Down`. Subscribe to `InputEvent::Change` so typing still resets to the first match. **User-ok** (“All good”).
36. **044 palette wiring is thin call_core + tab chrome, not overlays.** Scroll/reset/open-link/Help/minimize/ActivateTab/MoveTab. **User-ok** (“all of these work”). Charselect, copy-mode, search, launcher, WSL, fullscreen, SpawnWindow, pane rotate/zoom stay listed.
37. **Palette ↑↓ must scroll the overflow list (045).** Wheel works because `.overflow_y_scroll` handles it. Arrow keys only mutated `selected`, so the highlight walked off the first page. Track `ScrollHandle` on `#command-list` and `scroll_to_item(selected)` after `move_sel` / query reset. **User-ok** (“all good now”).
38. **046 pane ops are GPUI-tree, not mux Tab.** ActivatePaneDirection uses equal-half neighbor geometry. Rotate cycles leaf identities. Zoom paints only the active leaf. AdjustPaneSize stays listed. **User-ok** (“works like a charm”).
39. **047 window chrome is GPUI fullscreen + Win32 z-order.** `toggle_fullscreen` is GPUI. AlwaysOnTop uses `HWND_TOPMOST` (GPUI has no set_window_level; wezterm-gui is a no-op on Windows). Reset font+size restores lua font and launch content size. **User-ok** (“everything works”).

---

## Session protocol

1. Read this file, then `wezterm-gpui/docs/STATE.json`. For lua keys also `docs/lua-config.json`. For palette/launcher ops `docs/command-palette.json` / `docs/launcher.json`.
2. Treat `STATE.json` `current_phase`, `next`, `blockers`, `pins` as live (this HANDOFF can lag; if they disagree, **STATE wins**, then update this file if the story changed). Lua per-key status: **`docs/lua-config.json` wins**. Palette ops: **`docs/command-palette.json` wins**. Launcher: **`docs/launcher.json` wins**.
3. After material work: update `STATE.json`; if a lua key changed, update `docs/lua-config.json`; if a palette/launcher op changed, update those JSON files (`stats` / rows / `backlog`); append `docs/adr/` or `docs/decisions/`; if the narrative of “where we are / what’s next” changed, update **this HANDOFF.md** so the next fresh session stays accurate.
4. Never delete findings, decisions, ADRs, lua-config, command-palette, or launcher rows.
5. Do not change default `wezterm-gui` / `window` behavior.

Docs index:

- `docs/STATE.json` — machine tracker
- `docs/lua-config.json` — per-key lua honor + stats (do not duplicate long lists elsewhere)
- `docs/command-palette.json` — per-command palette honor + kind (036)
- `docs/launcher.json` — launcher sources + unique ops (036)
- `docs/help/resume.md` — short commands
- `docs/plans/000-feasibility-spike.md` — original feasibility plan (blast radius, effort)
- `docs/plans/020-lua-config-first-slice.md` — load wezterm.lua (font/size/scheme/scrollback/bell)
- `docs/plans/021-mouse-selection-copy-paste.md` — drag-select + clipboard
- `docs/plans/023-boxdraw-and-cell-clip.md` — geometry U+2500–259F (tight cell clip reverted in 025)
- `docs/plans/024-hi-dpi-line-sprite-dest.md` — 4K line-sprite dest / scale_factor
- `docs/plans/025-120dpi-glyph-clip-and-dest-1to1.md` — drop tight clip + dest 1:1 at 120dpi (**user-ok**)
- `docs/plans/026-monitor-move-dpi-reshape-hang.md` — parked: 2–3s hang moving between monitors
- `docs/plans/027-new-tab-shell-menu.md` — Plus = cmd; chevron lists PowerShell (**user-ok**)
- `docs/plans/028-gpui-component-icon-assets.md` — `with_assets` so Plus / window controls paint (**user-ok**)
- `docs/plans/029-120dpi-cursor-cell-span.md` — abutting cell fills (**not user-ok**; gap persisted)
- `docs/plans/030-integer-cell-grid.md` — ceil cell size + `num_cells * cell_w` glyphs (**user-ok**)
- `docs/plans/031-cursor-block-until-backspace.md` — parked: block cursor missing until backspace
- `docs/plans/032-tab-close-exit-keys.md` — tab X / `exit` left typing dead (**better now**)
- `docs/plans/033-last-tab-exit-quits.md` — last-tab `exit` quits like wezterm (**user-ok**)
- `docs/plans/034-lua-config-second-slice.md` — remaining ~/.wezterm.lua chrome + mouse_bindings (**user-ok**; hover highlight backlog)
- `docs/plans/036-command-palette-and-launcher-matrix.md` — classify palette/launcher ops; dimmed NYI list
- `docs/plans/037-dialog-cancel-restores-term-focus.md` — Ctrl+Q / tab X Cancel left typing dead (032 leftover)
- `docs/plans/038-mount-root-dialog-layer.md` — AppShell must paint `Root::render_dialog_layer` (**user-ok**)
- `docs/plans/039-skip-close-confirmation-for-shells.md` — skip close confirm for cmd.exe / skip list
- `docs/plans/040-tab-shell-splits.md` — palette Split H/V; GPUI tree + spawn_pane (**user-ok**)
- `docs/plans/041-inactive-pane-hsb.md` — lua `inactive_pane_hsb` + hollow unfocused cursor (**user-ok**)
- `docs/plans/042-palette-selection-contrast.md` — selected row inverts lua palette fg/bg (**user-ok**)
- `docs/plans/043-palette-arrow-keeps-selection.md` — ↑↓ keep selected row (`InputEvent::Change`) (**user-ok**)
- `docs/plans/044-palette-core-chrome.md` — scroll/reset/tabs/Help/minimize (41 wired) (**user-ok**)
- `docs/plans/045-palette-arrow-scrolls-list.md` — ↑↓ scroll selected row into view (`ScrollHandle`) (**user-ok**)
- `docs/plans/046-palette-pane-ops.md` — ActivatePaneDirection / RotatePanes / TogglePaneZoom (**user-ok**)
- `docs/plans/047-palette-window-chrome.md` — fullscreen / z-order / reset size (**user-ok**)
- `docs/adr/` — accepted decisions (0003 = GPUI-owned FreeType)
- `docs/decisions/` — 016 paint quality; 017 line sprites; 018 ConPTY junk later; 019 gpui-fps HUD; 020 lua config first slice; 021 mouse selection + copy/paste; 022 click ≠ selection + term focus; 023 box-draw + cell clip; 024 4K line-sprite dest; 025 120dpi clip+dest; 026 monitor-move hang backlog; 027 new-tab shell menu; 028 icon assets; 029 cursor cell span; 030 integer cell grid; 031 cursor until backspace backlog; 032 tab close/exit keys; 033 last-tab `exit` quits; 034 lua second slice; 035 lua-config.json matrix; 036 palette+launcher matrices; 037 dialog cancel restores term focus; 038 mount Root dialog layer; 039 skip close confirm for shells; 040 tab/shell splits; 041 inactive pane HSB; 042 palette selection contrast; 043 palette arrow keys keep selection; 044 palette core/tab chrome; 045 palette arrow keys scroll the list; 046 palette pane ops; 047 palette window chrome
- `docs/reference/` — rendering-quality (current), lua-config, command-palette, launcher, gpui-terminal / tty7, steal-list, open-questions, scrollback
