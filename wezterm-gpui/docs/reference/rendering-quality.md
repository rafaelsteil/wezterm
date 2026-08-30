# Terminal rendering quality (current main goal)

Palette / charselect / chrome polish is **parked**. The POC is only as useful as cmd.exe looking right **and feeling usable**.

Decisions: [016](../decisions/016-term-render-quality.json), [017](../decisions/017-line-sprites-and-coalesce.json), [018](../decisions/018-conpty-junk-also-wezterm-gui.json).

## User-try

**016 (2026-08-26):** 120dpi looks correct. Vertical junk also in official wezterm-gui → later polish (018). Paint was too slow (`dir /a`, scroll, especially typing).

**017 (2026-08-26):** “yah, much better now.” Line sprites + mux coalesce are good enough for now. Do not start a GPUI `text_system` rewrite unless lag comes back.

**020 (2026-08-27):** Lua font/scheme **user-ok** (“works like a charm”). Brown pane was RGBA vs GPUI BGRA; thin Cascadia was `fg × alpha²`. Coverage blit matches wezterm-gui well enough.

**021 (2026-08-27):** Selection/copy/paste **user-ok**. Wrapped triple-click first missed continuation (physical row); `get_logical_lines` then **user-ok** (“works”).

**023 (2026-08-27):** Geometry box-draw **user-ok** (`echo` / `tree`). **Finished** 2026-08-29 (tight clip stays reverted in 025; powerline is not 023).

**024 (2026-08-27):** 4K main monitor shredded each row into horizontal slivers; other screen OK. Dest is now image device px / `scale_factor`. **Not user-ok** — retry showed vertical glyph slivers at 120dpi (025).

**025 (2026-08-27):** 4K @ 120dpi **user-ok** after dropping tight cell clip and locking dest 1:1. 96dpi still OK.

**029 (user-ok 2026-08-28):** 4K @ 120dpi block cursor gap. `cell_span` alone did not close it; **030** ceil + `num_cells * cell_w` did.

**030 (2026-08-27):** Integer cell grid **user-ok** (“works great now”). Ceil + `num_cells * cell_w` closed the 120dpi cursor gap.

**026 (parked):** Moving the window between monitors hangs **2–3s** (looks like DPI reshape/redraw). Backlog; do not start unless asked.

**031 (user-ok 2026-08-28):** Fill `cursor_bg` at `cursor.x` even when `visible_cells()` has no matching col (“fixed”).

**032 (2026-08-27):** Tab X / `exit` left typing dead. **Better now.**

**033 (2026-08-27):** Last-tab `exit` quits the app like wezterm-gui. **User-ok** (“works great”).

Fairness note: this POC is a **debug** `wezterm-gpui.exe`. Official wezterm-gui is **release**.

## Done

| Item | Status |
|---|---|
| DPI from `scale_factor` | User-ok at 120dpi |
| Device-pixel snap | In |
| Committed ConPTY after ~450ms still grid | In (drag still 013) |
| Per-line cached `RenderImage` | **User-ok** (017) |
| Drain mux `PaneOutput` before `notify` | **User-ok** (017) |
| Skip layout when pane seqno/viewport/cursor unchanged | **User-ok** (017) |
| gpui-fps HUD | Wired (019), **off by default**. Ctrl+Shift+F. Come back later. |
| Lua `font` / `color_scheme` | **User-ok** (020; BGRA + coverage blit) |
| Mouse selection + copy/paste | **User-ok** (021), including wrapped triple-click |
| Geometry box-draw U+2500–259F | **Finished** (023; echo/tree **user-ok**) |
| Per-cell glyph clip | **Reverted (025)** — 120dpi cut LCD/bearings; row-bitmap clip only |
| 4K line-sprite dest | **User-ok** with 025 (dest 1:1 + clip off) |
| 120dpi cursor 1px gap | **User-ok** (030 integer cell grid). 029 `cell_span` alone was not enough |
| Cursor missing until backspace | **User-ok (031)** — fill at `cursor.x` even past last stored cell |
| Tab close / `exit` kills typing | **User-ok** (032 better now; 033 last-tab `exit` quits) |

## Still wrong / next (smallest first)

1. **049** remaining palette catalog (charselect, copy mode, launcher, reload) if not tried.
2. **Backlog: monitor-move hang (026)** — 2–3s when dragging between 96dpi and 120dpi monitors. Do not start unless asked.
3. GPUI `text_system` spike only if paint feels slow again — [gpui-text-vs-sprites.md](gpui-text-vs-sprites.md). Do not go back to per-glyph `paint_image`.
4. **Come back later: measure with gpui-fps** (palette Toggle FPS HUD). Stock HUD is *continuous* (how fast we can paint; window never idles). For typing/`dir` lag, watch **FRAME** with continuous *off* — that toggle is **not wired yet**. Hide HUD to idle.
5. ConPTY vertical junk: **later**, with wezterm-gui (018).
6. Powerline triangles (U+E0B0…) only if nerd-font prompts look wrong (not 023).

## Feedback that helps

FPS HUD is **off** until Ctrl+Shift+F. Screenshots of “looks wrong” vs wezterm-gui still help. Palette notes and isolated ConPTY-junk hunts are not useful for a while.

## Do not

- `ResizePseudoConsole` on every layout (013).
- Viewport-sized `RenderImage` every frame (010).
- Path-dep gpui-terminal / tty7; copy Zed GPL `terminal_element.rs`.
- Start charselect / palette as “continue”.
- Spend a session on ConPTY smear that wezterm-gui also shows (018).
- Rewrite paint onto GPUI text unless the user says it is slow again.
