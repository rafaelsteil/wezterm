# Open questions (re-evaluate later)

Decision 013 and 009/010 stay until a **new** decision replaces them. These notes exist because the other GPUI terminals do things that look forbidden in our steal-list — they have reasons, and some of those reasons do not apply on Windows ConPTY / wezterm-font.

## 1. Live PTY resize on every layout

**Today:** first valid pane size calls `Pane::resize` (ConPTY + terminal). Later **drag** layouts call `resize_display` only. After cols/rows are unchanged ~450ms, one committed `Pane::resize` (decision 016). User-ok so far for drag (no D/a); committed path **needs user try**.

**Why the others live-resize and it “works”:**

| Project | What they actually do | Why it can look fine |
|---|---|---|
| gpui-terminal | `canvas` paint → `pty.resize` whenever floor(px/cell) changes | README is Linux/Wayland. Unix `TIOCSWINSZ`+SIGWINCH does **not** rewrite the screen. Windows ConPTY *does* (`ResizePseudoConsole` reflow). Scrollback nav is still TODO — they may never have dogfooded Windows resize. |
| tty7 | Always `ClientMsg::Resize` to a **daemon**; local grid reflow is **deferred until a size echo** so old-width bytes are not parsed into a new-width grid | They *know* live resize during an output burst garbles the pane. The echo is a protocol fix we do not have. They still must live-resize because editors/`less`/pixel-aware apps need `ws_col`/`ws_xpixel`. |
| Zed | Coalesce pixel-only `set_size`; still PTY-resize when **cols/rows** change | Comment: avoid SIGWINCH flicker. Same Unix-friendly PTY. They also **clamp** `<2` cells **up** to 2 — we must not copy that on ConPTY (decision 012). |
| wezterm-gui | Live `LocalPane::resize` | Win32 resize increments snap to **cell pixels**, so floor(px/cell) barely oscillates. GPUI windows are free-pixel. |

So “it works for them” is mostly **different PTY + different window snapping + (tty7) a drain/echo**, not proof that ConPTY can take a col change per drag frame.

**Re-evaluate when any of these is true:**

1. GPUI window (or pane) snaps to cell increments, like wezterm-gui.
2. We have a **committed** ConPTY sync (mouse-up / `WM_EXITSIZEMOVE`) and want to try *also* live-syncing with a stronger guard than decision 011’s 100ms debounce.
3. We can detect ConPTY drain / “resize echo” (tty7 pattern) so display reflow waits for the new geometry’s first output.
4. We try the same binary on Unix (no ConPTY) where live `pty.resize` is the normal path.
5. New cmd.exe output wrapping at the **first** PTY size becomes the user-visible bug (it already is, for new output after shrink).

Until then: keep 013 **for drag**. 016 is the “new output wrapping at first PTY size” experiment (item 5 above), not live-per-frame ConPTY. Do not copy gpui-terminal’s callback-in-paint.

## 2. GPUI `text_system` vs wezterm-font sprites

**Today:** CPU raster via `LoadedFont` → cached `paint_image` sprites + `paint_quad` backgrounds (decisions 009/010). Consolas `div` fallback.

Deeper than “they use `shape_line`”:

**Sprites (keep) win if we care about WezTerm glyph identity.** Same HarfBuzz, fallback chain, color/COLR, ligatures, `vendor-jetbrains`. Metrics match wezterm-gui. We already paid `freetype-sys` (ADR 0003). Cost: one GPUI image blit per visible glyph, CPU raster on cache miss, atlas pressure if we churn `ImageId`s (010), cell clip is pane-wide not per-run.

**GPUI text wins if we care about fighting GPUI less.** tty7/Zed batch runs, `force_width`, GPU atlas owned by GPUI — that *is* the “GPU atlas Element” slice without a custom atlas. Metrics come from the same `Window` that paints (`M` / `advance('m')`), so grid and glyphs cannot disagree. Cost: DirectWrite/font-kit ≠ wezterm-font; ligatures usually **off** in those apps; tty7 already hit Windows italic-fallback **mojibake** and forked GPUI; emoji overflow needs `seg_budget` (they cite WezTerm); dual font stacks until a `window/` cutover.

**Hybrid is a trap** (GPUI for ASCII, sprites for CJK/emoji): two metrics, two clip rules.

**How to dig for real (when we pick this, not now):**

1. Spike a second paint path behind a flag: snapshot `wezterm-term` cells → tty7-style `shape_line` batches (Apache rewrite, not Zed GPL). Keep sprites as fallback.
2. On Windows: italic + CJK fallback, `dir` box lines, JetBrains ligatures, color emoji, shrink-then-grow (atlas). Compare to sprites.
3. Only then decide. Do not undo ADR 0003 just to drop FreeType; wezterm-gui still needs it.

See [gpui-text-vs-sprites.md](gpui-text-vs-sprites.md).

## 3. Scrollback (done)

User-tried 2026-08-26. Sketch: [scrollback.md](scrollback.md).

## 4. Paint performance (017 in; still compare to wezterm-gui)

User 2026-08-26: slower than wezterm-gui on `dir /a`, scroll, typing. POC was per-glyph `paint_image` + HarfBuzz every visible line every mux notify.

017: one `RenderImage` per unique line (cache by `compute_shape_hash` + cursor), drain `PaneOutput`, skip layout if seqno unchanged. **User-ok 2026-08-26** (“much better now”).

GPUI `text_system` (§2) stays the remaining big lever if lag returns. Official wezterm-gui is also a **release** binary; this spike is debug unless someone asks for `--release`.
