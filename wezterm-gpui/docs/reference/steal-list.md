# Steal list (ranked against our current pain)

**Main goal is term paint** (016/017): [rendering-quality.md](rendering-quality.md). Palette parked. ConPTY junk also in wezterm-gui (018). Scrollback user-ok (015). 013 still forbids ConPTY **on drag**.

Checkouts: `D:\dev\gpui-terminal`, `D:\dev\tty7`. Zed `terminal_element.rs` is **GPL-3.0-or-later** — ideas only, no paste.

## Ranked

### 0. Scrollback (done)

User-ok. Still no scrollbar. Perf while scrolling is an open question (§4 in [open-questions.md](open-questions.md)), not this item.

### 1. GPUI `text_system` vs wezterm-font sprites (re-evaluate; not this slice)

They use `shape_line` because GPUI then owns the glyph atlas — a real reason. Keep sprites until a Windows flag spike (italic CJK, emoji, shrink-grow) says otherwise. Table and spike: [gpui-text-vs-sprites.md](gpui-text-vs-sprites.md).

### 2. Device-pixel metrics in `prepaint` (016: dpi + snap done; cell-from-GPUI-text not)

Ours now: `dpi ≈ 96 * window.scale_factor()`; snap quad edges / sprite origins to device pixels.

Still not: measure cell width from GPUI `shape_line("M")` / `advance('m')`. Sprites stay on wezterm-font metrics. Revisit only if grid and glyphs disagree at a given DPI.

### 3. Snapshot then paint (correctness, small)

tty7 copies `RenderCell[]` then drops the VT lock before GPU.

Ours: `try_glyph_paint` copies `get_lines` then returns `TermPaint`; `paint_term` runs **outside** `term.update`. Good enough. Still: `get_lines` + HarfBuzz shape hold pane/font work on the GPUI thread every frame. Later: snapshot attrs+codepoints only, shape on the paint copy.

gpui-terminal holds `&Term` during the entire `canvas` paint — do not copy that.

### 4. Batched runs + `force_width` + per-run mask (clip done for sprites)

tty7 segments rows, `force_width = cell_width`, clips each run. gpui-terminal *documents* batching then paints **one `shape_line` per character** (`layout_row` text_runs discarded).

Sprites: tight `[col, col+num_cells)` clip (023) **reverted in 025** — at 120dpi it cut LCD/bearings. Row bitmap still clips the line. Padded overflow-only clip later if smear returns. `force_width` is a GPUI-text concern; not this path.

### 5. Box-drawing as geometry (023 **finished**)

gpui-terminal `box_drawing.rs` and tty7 `boxdraw.rs` (Apache). Thickness rounded to device pixels; tty7 also covers U+2580 blocks and powerline triangles with an anti-aliased closing-edge quad.

**023 finished:** U+2500–259F is CPU geometry in the line sprite (`src/boxdraw.rs`), not font glyphs. wezterm-gui `customglyph.rs` / tiny-skia not imported. Powerline (U+E0B0…) still font sprites — later slice if nerd-font prompts look wrong, not 023 leftover.

### 6. Input checklist (when vim/less break)

Keep `Pane::key_down`. Use gpui-terminal `input.rs` tests as a key matrix; tty7 `input.rs` when kitty keyboard / Option-as-Meta / IME show up. Do not write raw CSI in the GPUI crate.

### 7. Mouse / selection / hitbox

tty7: `insert_hitbox` in prepaint, mouse events on the Element. gpui-terminal mouse reports exist but selection is unfinished. Defer until chrome+paint is boring.

### 8. Resize policy (keep 013; re-evaluate — they have reasons)

Live PTY resize is how a real terminal tells vim/`less` the new size. tty7 invented **resize-echo** because doing it naively garbles output. gpui-terminal’s Unix SIGWINCH path is not ConPTY. wezterm-gui lives-resizes because the HWND snaps to cells.

**Now:** display-only live rewrap (013) **plus** one ConPTY after the grid is still ~450ms (016). **Later:** if that smear comes back, mouse-up / cell-snap / echo — not per-frame ConPTY. [open-questions.md](open-questions.md) §1.

Zed **clamps** width `< 2 cells` **up** to 2 cells. We skip those bounds (012). Do not copy Zed’s clamp.

## Copy rules

- Apache-2.0 / MIT-OR-Apache files: keep copyright + license header on copied files; rewrite types to `wezterm-term`.
- Zed GPL: read, describe, reimplement. Never paste.
- No new Cargo path deps on the checkouts.
- No paint rewrite or live-ConPTY until those open questions are picked. Scrollback does not need either.
