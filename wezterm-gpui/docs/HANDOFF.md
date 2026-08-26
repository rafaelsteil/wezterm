# HANDOFF — WezTerm GPUI spike

Feed this file to a **new agent session** and say continue. Do not rely on prior chat. After reading this, also open `STATE.json` (live machine status) and follow the write-back rules at the bottom.

Repo: `D:\dev\wezterm` (Windows). Branch: `raf-gpui`.

---

## What this is

Proof of concept: can WezTerm use **Zed GPUI** + **gpui-component** for UI chrome instead of the custom window/box-model stack, without dropping the existing GUI.

Goals:

1. Less UI code to maintain (only after a later cutover).
2. Richer widgets for free (dialogs, lists, inputs, later dock/tabs).

This is **not** a rewrite of the terminal renderer. Dual-stack is expected. POC shortcuts are allowed.

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
| mux LocalDomain cmd.exe | Done compile+build. **Not yet user-tried.** Spawn is `LocalDomain::spawn_pane` + `%ComSpec%`/`cmd.exe`. |
| GPUI-owned FreeType | Done compile. Isolated graph uses only `freetype-sys` 0.20.1. `wezterm-font` path-dep with `sys-freetype`. wezterm-gui still vendors `deps/freetype`. |
| wezterm-font cell paint | Done. User-tried 2026-08-26 (cmd.exe + JetBrains). Full-pane bitmap went black on shrink-then-grow; paint is now cached glyph sprites + quads. Consolas fallback if init fails. **Not the wezterm-gui GPU atlas.** |
| ConPTY live resize | Done. User-tried 2026-08-26: after decision 013 (display rewrap, no `ResizePseudoConsole` after first size) the vertical D/a column is gone. `colfill` 1 (wide) to 7 (narrow) is wrap occupancy. New cmd.exe output still wraps at the first PTY size. |

`go_nogo`: in-process embed = **no**. Continue POC = **yes** (isolated workspace). Runtime window = **yes**. CLI spawn = **yes**. Min usable chrome = **yes**. Mux cmd.exe = **user-tried**. `wezterm-font` paint = **sprite cache + display-only live resize, user-ok**. Glyph atlas Element = **not started**.

Pins:

- Zed GPUI: `fecc3273ed32643c2ea1b04a74c8780e2c9ffaf8` (lockfile)
- gpui-component: `ff3eb1128ac1058f1bb88e777744ce1237aa3b79` (`Cargo.toml` `rev`)
- rustc used: `1.97.1` `x86_64-pc-windows-msvc` (gpui-component wants 1.90+)

---

## Next steps (pick with the user unless they already said)

Default `wezterm gpui` now hosts **cmd.exe** (or `%ComSpec%`) through **mux `LocalDomain`**, the same module wezterm-gui uses for a local pane. Paint path is **wezterm-font glyph sprites** (cached GPUI `paint_image` + cell quads), with Consolas text as fallback. Not the wezterm-gui glyph atlas.

Do **not** start a `window/` cutover unless the user explicitly asks.

Reasonable continuations, smallest first:

1. Character selector overlay or palette keyboard polish. GPU atlas Element is a later slice.
2. Optional later: sync ConPTY to the window on a committed resize (mouse-up), without live-drag `ResizePseudoConsole`.
3. **Only if asked:** windowing cutover (replace `window/` with `gpui_platform`). Multi-quarter; huge blast radius.

If the user just says “continue”: pick (1). Do not start (3).

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
  src/shell.rs                   # TitleBar + tabs + mux TermPane + palette + confirm/prompt
  src/palette.rs                 # Input + filtered hardcoded commands (overlay card)
  src/confirm.rs                 # AlertDialog confirm + Dialog+Input line prompt
  src/mux_host.rs                # config + Mux + LocalDomain init; spawn cmd.exe
  src/term_pane.rs               # mux LocalPane; wezterm-font paint or Consolas fallback
  src/glyph_paint.rs             # LoadedFont rasterize → cached sprites + quads
  docs/
    HANDOFF.md                   # this file
    STATE.json                   # live phase/next/pins/findings (machine)
    help/resume.md               # short command/constraint cheat sheet
    plans/000-feasibility-spike.md
    adr/0001-use-zed-official-gpui.md
    adr/0002-isolated-cargo-workspace.md
    adr/0003-gpui-owns-freetype.md
    decisions/*.json

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
10. **Do not call `LocalPane::resize` (ConPTY) on live GPUI drag.** A few pixels of width is one column; `ResizePseudoConsole` then smears the cursor column (user: 84x20 `dir` colfill 16→12). wezterm-gui mostly avoids this via Win32 resize increments. POC: first stable size still resizes the PTY; later changes are `Pane::resize_display` (terminal rewrap only, decision 013). Never clamp 0×0 up to 8×2 (decision 012). Status `pty` vs `view` and `colfill=`.

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
- `docs/adr/` — accepted decisions (0003 = GPUI-owned FreeType)
- `docs/decisions/` — lightweight records
