# Terminal rendering quality (current main goal)

Palette / charselect / chrome polish is **parked**. The POC is only as useful as cmd.exe looking right **and feeling usable**.

Decisions: [016](../decisions/016-term-render-quality.json), [017](../decisions/017-line-sprites-and-coalesce.json), [018](../decisions/018-conpty-junk-also-wezterm-gui.json).

## User-try

**016 (2026-08-26):** 120dpi looks correct. Vertical junk also in official wezterm-gui → later polish (018). Paint was too slow (`dir /a`, scroll, especially typing).

**017 (2026-08-26):** “yah, much better now.” Line sprites + mux coalesce are good enough for now. Do not start a GPUI `text_system` rewrite unless lag comes back.

**020 (2026-08-27):** Lua font/scheme **user-ok** (“works like a charm”). Brown pane was RGBA vs GPUI BGRA; thin Cascadia was `fg × alpha²`. Coverage blit matches wezterm-gui well enough.

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

## Still wrong / next (smallest first)

1. Visual leftovers vs wezterm-gui: geometry box-draw, per-cell clip, selection/mouse if those get in the way.
2. GPUI `text_system` spike only if paint feels slow again — [gpui-text-vs-sprites.md](gpui-text-vs-sprites.md). Do not go back to per-glyph `paint_image`.
3. **Come back later: measure with gpui-fps** (Ctrl+Shift+F). Stock HUD is *continuous* (how fast we can paint; window never idles). For typing/`dir` lag, watch **FRAME** with continuous *off* — that toggle is **not wired yet**. Hide HUD to idle.
4. ConPTY vertical junk: **later**, with wezterm-gui (018).

## Feedback that helps

FPS HUD is **off** until Ctrl+Shift+F. Screenshots of “looks wrong” vs wezterm-gui still help. Palette notes and isolated ConPTY-junk hunts are not useful for a while.

## Do not

- `ResizePseudoConsole` on every layout (013).
- Viewport-sized `RenderImage` every frame (010).
- Path-dep gpui-terminal / tty7; copy Zed GPL `terminal_element.rs`.
- Start charselect / palette as “continue”.
- Spend a session on ConPTY smear that wezterm-gui also shows (018).
- Rewrite paint onto GPUI text unless the user says it is slow again.
