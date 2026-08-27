# HANDOFF — WezTerm GPUI spike

Feed this file to a **new agent session** and say continue. Do not rely on prior chat. After reading this, also open `STATE.json` (live machine status) and follow the write-back rules at the bottom.

Repo: `D:\dev\wezterm` (Windows). Branch: `raf-gpui`.

---

## What this is

Proof of concept: can WezTerm use **Zed GPUI** + **gpui-component** for UI chrome instead of the custom window/box-model stack, without dropping the existing GUI.

Goals:

1. **Terminal/shell rendering that is actually usable** (current main goal; decision 016). Chrome does not matter until cmd.exe looks right.
2. Later: less UI code to maintain (only after a cutover) and richer widgets.

Dual-stack is expected. POC shortcuts are allowed. Palette / charselect are **parked**.

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

`go_nogo`: in-process embed = **no**. Continue POC = **yes**. Runtime window = **yes**. CLI spawn = **yes**. Min usable chrome = **yes**. Mux cmd.exe = **user-tried**. Paint = **line sprites user-ok (017) + 120dpi (016)**. FPS HUD = **wired, off by default (019)**. Lua config = **user-ok (020; Cascadia + Dracula)**. Glyph atlas Element = **not started**. Scrollback = **user-ok**. Palette = **parked**. ConPTY junk = **shared with wezterm-gui, polish later**.

Pins:

- Zed GPUI: `fecc3273ed32643c2ea1b04a74c8780e2c9ffaf8` (lockfile)
- gpui-component: `ff3eb1128ac1058f1bb88e777744ce1237aa3b79` (`Cargo.toml` `rev`)
- rustc used: `1.97.1` `x86_64-pc-windows-msvc` (gpui-component wants 1.90+)

---

## Next steps (pick with the user unless they already said)

Default `wezterm gpui` hosts **cmd.exe** through mux. Paint is **one cached GPUI image per line** (wezterm-font composite, 017) at window DPI — **user-ok** (“much better”). Live drag rewraps display only; ~450ms later one ConPTY `resize`. Loads **`~/.wezterm.lua`** for font/size/scheme/scrollback/bell (020) — **user-ok** (“works like a charm”). **Main goal still rendering** vs wezterm-gui (looks, not chrome).

Do **not** start a `window/` cutover unless the user explicitly asks.
Do **not** start character selector / palette keyboard — user parked those.
Do **not** investigate ConPTY vertical junk in this POC — also happens in official wezterm-gui (018).
Do **not** start a GPUI `text_system` rewrite unless paint feels slow again.
Do **not** expand lua to tab-bar / decorations / mouse bindings / live reload unless asked.

Workstream: `docs/reference/rendering-quality.md`. Lua: `docs/plans/020-lua-config-first-slice.md`.

Reasonable continuations, smallest first:

1. Visual leftovers vs wezterm-gui: geometry box-draw, per-cell clip, selection/mouse if those get in the way.
2. Come back later: FPS HUD (Ctrl+Shift+F). Continuous = sustain rate; for typing lag we still want a non-continuous FRAME mode (not wired).
3. GPUI `text_system` spike only if they ask for more speed (`docs/reference/gpui-text-vs-sprites.md`).
4. **Only if asked:** windowing cutover.

If the user just says “continue”: pick (1) from whatever still looks wrong. Do not start (4). Do not paste Zed `terminal_element.rs` (GPL-3). Do not resume palette/charselect.

---

## Architecture (why GPUI cannot eat WezTerm incrementally in-process)

WezTerm has **three** UI layers:

| Layer | Where | GPUI role |
|---|---|---|
| Native windowing | `window/` (Win32, Cocoa, X11, Wayland, EGL/WGL) | Replaced only at full cutover by `gpui_platform` |
| Box model chrome | `wezterm-gui/src/termwindow/box_model.rs` + fancy tab bar, window buttons, `Modal`s | Natural gpui-component target |
| Box-model modals | `palette.rs`, `charselect.rs`, `paneselect.rs` | First incremental replacements (sibling windows today) |
| Termwiz overlays | `wezterm-gui/src/overlay/` (launcher, copy, debug, confirm, …) | Dialog/Input (confirm+prompt POC done; rest later) |
| Terminal cells | glyph cache, glium + optional wgpu 25 | Custom GPUI `Element` later. **POC now:** mux `LocalPane` (cmd.exe) + wezterm-font sprites in `glyph_paint.rs` |

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
  src/shell.rs                   # TitleBar + tabs + mux TermPane + palette + gpui-fps HUD
  src/palette.rs                 # Input + filtered hardcoded commands (overlay card)
  src/confirm.rs                 # AlertDialog confirm + Dialog+Input line prompt
  src/mux_host.rs                # config + Mux + LocalDomain; load lua; spawn cmd.exe
  src/term_pane.rs               # mux LocalPane; wezterm-font paint or Consolas fallback
  src/glyph_paint.rs             # wezterm-font → cached per-line RenderImages (017)
  docs/
    HANDOFF.md                   # this file
    STATE.json                   # live phase/next/pins/findings (machine)
    help/resume.md               # short command/constraint cheat sheet
    plans/000-feasibility-spike.md
    plans/020-lua-config-first-slice.md
    adr/0001-use-zed-official-gpui.md
    adr/0002-isolated-cargo-workspace.md
    adr/0003-gpui-owns-freetype.md
    decisions/*.json
    reference/INDEX.md           # gpui-terminal + tty7 (D:\dev checkouts; not path deps)
    reference/rendering-quality.md  # current main goal (decision 016)
    reference/open-questions.md  # live PTY resize + GPUI text: re-eval later
    reference/scrollback.md      # viewport slice (decision 015; user-ok)

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
13. **Lua config (020).** `common_init` used to skip `wezterm.lua` so `default_prog` could not replace cmd.exe. Now the file loads; spawn still forces `%ComSpec%`. On Windows, `wezterm.lua` next to the exe still wins over `~/.wezterm.lua`. Unknown lua fields reject the whole file (same as wezterm-gui) → defaults. Live reload / `wezterm.on` / chrome keys are not this slice.
14. **GPUI `RenderImage` is BGRA.** Line sprites must swap R/B before `Frame::new`. RGBA upload made Dracula `#282a36` look brown (`#362a28`). `gpui::rgb(0xRRGGBB)` for quads is already correct.
15. **Do not premultiply glyph coverage then multiply alpha again.** That is `fg * alpha²` and Cascadia looks thin vs wezterm-gui. Cache FreeType coverage; blit `sRGB_fg * linear_a + bg * (1-a)` like the glyph shader.

---

## Session protocol

1. Read this file, then `wezterm-gpui/docs/STATE.json`.
2. Treat `STATE.json` `current_phase`, `next`, `blockers`, `pins` as live (this HANDOFF can lag; if they disagree, **STATE wins**, then update this file if the story changed).
3. After material work: update `STATE.json`; append `docs/adr/` or `docs/decisions/`; if the narrative of “where we are / what’s next” changed, update **this HANDOFF.md** so the next fresh session stays accurate.
4. Never delete findings, decisions, or ADRs.
5. Do not change default `wezterm-gui` / `window` behavior.

Docs index:

- `docs/STATE.json` — machine tracker
- `docs/help/resume.md` — short commands
- `docs/plans/000-feasibility-spike.md` — original feasibility plan (blast radius, effort)
- `docs/plans/020-lua-config-first-slice.md` — load wezterm.lua (font/size/scheme/scrollback/bell)
- `docs/adr/` — accepted decisions (0003 = GPUI-owned FreeType)
- `docs/decisions/` — 016 paint quality; 017 line sprites; 018 ConPTY junk later; 019 gpui-fps HUD; 020 lua config first slice
- `docs/reference/` — rendering-quality (current), gpui-terminal / tty7, steal-list, open-questions, scrollback
