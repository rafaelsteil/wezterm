# tty7 (l0ng-ai)

Checkout: `D:\dev\tty7`  
HEAD: `d4b8e331b970d4c500c3e659a42365f225b4d1cf` (2026-08-26)  
License: Apache-2.0  
Upstream: https://github.com/l0ng-ai/tty7

A **full terminal workbench** (daemon, SSH, agents, git). Useful slice for us is `src/terminal/` paint/input, not the product.

## GPUI pins (do not take)

From `Cargo.toml`:

- Declared: `zed-industries/zed` rev `1d217ee39d381ac101b7cf49d3d22451ac1093fe`
- **Patched** to `github.com/l0ng-ai/zed` branch `tty7` (same trick we use: `[patch]` so gpui-component does not duplicate the crate)
- `gpui-component` from `l0ng-ai/gpui-component` branch `tty7`, not longbridge upstream
- VT: their fork of Zed’s `alacritty_terminal` (`l0ng-ai/alacritty`)

Our pins stay Zed `fecc3273…` + longbridge gpui-component `ff3eb112…`.

### Windows GPUI patch to remember (do not fork yet)

Their comment on the Zed patch, item 2:

> gpui's Windows backend rasterizes a font-fallback run with the face DirectWrite actually shaped it with, instead of re-deriving one from the face's family/weight/style. The round trip mapped DirectWrite's italic to oblique, so italic CJK — which every pane reaches through the fallback chain — drew a *different* face's outlines at the shaped glyph indices. Every character rendered as an unrelated character (mojibake).

If we switch paint to GPUI `text_system` on Windows, this is a known landmine. They also patch IME (`prefers_ime_for_printable_keys` takes the keystroke) and resvg 0.47.

## File map (`src/terminal/` only)

| File | Size (approx) | Use for us |
|---|---|---|
| `element.rs` | 129 KB | **Primary.** Custom `Element`, snapshot, batched `shape_line`, mask, snap |
| `boxdraw.rs` | 27 KB | Geometry U+2500–259F + device-pixel snap |
| `input.rs` | 36 KB | Kitty keyboard + DECCKM + Option-as-Meta + IME |
| `images.rs` | 37 KB | Kitty graphics → GPUI `RenderImage` (BGRA swap note) |
| `size.rs` | tiny | `TermSize` cols/rows |
| `remote.rs` | 199 KB | Client↔daemon PTY; `resize` + resize-echo |
| `view.rs` | 494 KB | Workbench chrome — **skip** except `set_grid_size` |
| `scrollbar.rs`, `search.rs`, `smart_select.rs` | — | Later chrome, not paint POC |
| `generator.rs` | — | Fig completion scripts; **not** a GPU path |
| `parked_cursor.rs` | — | IME anchor when TUIs hide the hardware cursor |

## Element paint (confirmed)

Same shape as our `TermScreen` (real `Element`, not `canvas`):

1. **`prepaint`**: `shape_line("M")` → `cell_width`; `line_height = round(font_size * mul)`; `cols/rows = floor(bounds / cell)`; `view.set_grid_size(..., window.scale_factor())`; `insert_hitbox`.
2. **`build_grid`**: lock alacritty `Term`, copy visible cells into `RenderCell[]` (fg/bg/bold/italic/underline/selection), drop the lock.
3. **`paint`**: `with_content_mask(bounds)` then:
   - merged background quads
   - selection / search washes
   - `paint_glyphs` (segmented runs)
   - special underlines (device-px snap)
   - kitty images
   - cursor

`paint_glyphs` is the high-value routine:

- Segment a row into runs / wide / box-draw / powerline / grapheme clusters.
- Box-draw and powerline: `paint_quad` / `paint_path`, not fonts.
- Else `shape_line(text, font_size, run, force_width)` with `force_width = Some(cell_width)` (or 2× for wide).
- Fallback glyphs that overflow: `fit_scale` then reshape smaller; clip with **per-run** `ContentMask`.
- `FontFeatures::disable_ligatures()` on the terminal face.
- Emoji budget comment **cites WezTerm**: extra slack when the next cell is blank (`seg_budget`).

Device snap: `snap = |v| (v * scale).round() / scale` on underline and box-draw edges.

## Resize (not our policy)

`TerminalView::set_grid_size` always calls `terminal.resize(TermSize, cell_w_device, cell_h_device)`.

`RemoteTerminal::resize` (`remote.rs`):

- Early-out only if **cols/rows and device cell size** match (DPI move between 1×/2× still resizes so `ws_xpixel` stays honest).
- Sends `ClientMsg::Resize` to **their daemon**.
- If the daemon **echoes** size, they **defer local grid reflow** until the echo, so old-width bytes are not parsed into a new-width grid.

That is a protocol-level fix for the same class of bug as ConPTY smear — evidence they **need** live resize (editors, `ws_xpixel`) *and* that naive live resize is unsafe. We do not own a daemon. Keep 013 until a re-eval condition in [open-questions.md](open-questions.md) §1 (mouse-up ConPTY, cell-snap, echo/drain, Unix).

They report cell size in **device pixels** (`logical * scale_factor`, rounded) for pixel-aware children — same as kitty/ghostty. We hardcode `dpi: 96` in `TerminalSize`.

## Input

Full kitty keyboard (`DISAMBIGUATE_ESC_CODES`, report-all-keys) plus DECCKM. Option-as-Meta reshapes `key_char`. IME deferral on macOS.

We should keep mux `Pane::key_down` (wezterm-term encodes modes). Use tty7 as a **gap list** when vim/less/kitty apps misbehave, not as a second encoder.

## Images

`images.rs` documents GPUI atlas expecting **BGRA**; they swap R↔B once at ingest because `image` crate `RgbaImage` gets swapped again inside GPUI. Relevant if we keep `paint_image` sprites (`glyph_paint.rs`).

## Steal vs skip

**Steal (rewrite against wezterm-term):**

- Snapshot cells, drop emulator lock, then GPU.
- Batched `shape_line` + `force_width` + per-run mask (if we move to GPUI text).
- Box-draw / powerline as geometry with scale snap.
- Hitbox + mouse on the Element (when we add selection).
- Device-pixel cell metrics and `ws_xpixel` when we sync PTY size again.
- Ligature-off for the terminal face.

**Skip:**

- Daemon/SSH/agents/git/`view.rs` chrome.
- `alacritty_terminal` and their alacritty fork (emoji width / kitty stack panic — wezterm-term already has its own Unicode width).
- Their GPUI/gpui-component forks as dependencies.
- Live PTY resize every prepaint **until** the open-question conditions land. Their `on_scroll` / `display_offset` is the wrong scroll model for us ([scrollback.md](scrollback.md)).
- Completion generators, parked-cursor scanner (unless IME on Windows becomes a bug).
