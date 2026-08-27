# gpui-terminal (zortax)

Checkout: `D:\dev\gpui-terminal`  
HEAD: `51f0292938876c8da3de03f0139088591e3be518` (2026-01-11)  
License: MIT OR Apache-2.0  
Upstream: https://github.com/zortax/gpui-terminal

A **library** `TerminalView` for embedding a terminal in a GPUI app. ~10 Rust files. Not a WezTerm competitor.

## File map

| File | What it is | Our analogue |
|---|---|---|
| `src/view.rs` | Entity + `Render` + `canvas` + resize callback | `term_pane.rs` `TermPane` + `TermScreen` |
| `src/render.rs` | Cell metrics, bg merge, per-glyph `shape_line.paint` | `glyph_paint.rs` |
| `src/terminal.rs` | `alacritty_terminal::Term` wrapper + VTE | mux `LocalPane` / `wezterm-term` |
| `src/input.rs` | GPUI `Keystroke` → PTY bytes, APP_CURSOR | `term_pane.rs` `map_keystroke` → `Pane::key_down` |
| `src/mouse.rs` | pixel→cell, SGR 1006, selection types (unused in UI) | none yet |
| `src/box_drawing.rs` | U+2500–257F as GPUI paths | none yet (wezterm-gui has this) |
| `src/clipboard.rs` | `arboard` OSC 52 | mux `AssignClipboard` already exists |
| `src/colors.rs` | ANSI / 256 / truecolor palette | `wezterm_term::color::ColorPalette` |
| `src/event.rs` | bell / title / exit bridge | mux notifications |
| `src/main.rs` | `portable-pty` example | `mux_host.rs` |

## GPUI vintage (do not copy APIs)

`Cargo.toml`: `gpui = "0.2.2"` from **crates.io**. That is the old published crate. Our pin is Zed git `fecc3273…`. `shaped_line.paint(Point, cell_height, window, cx)` will not match our `ShapedLine::paint` (we need `TextAlign`, extra args — see tty7).

HANDOFF already forbids crates.io gpui 0.2.2.

## Paint path (confirmed on disk)

1. `measure_cell`: `text_system().shape_line("│", …)` → `cell_width = shaped.width`, `cell_height = (ascent+descent).ceil() * multiplier`.
2. `layout_row` **does** build `BatchedTextRun`s (adjacent same style).
3. `paint` **discards** those runs: `let (backgrounds, _) = self.layout_row(...)`. Text is a **third pass that `shape_line`s one character at a time**.
4. Box-drawing is a separate pass with horizontal span merging so lines do not gap.
5. `canvas` paint holds `&Term` (grid lock) for the whole paint.

Steal the **idea** (GPUI text, merged bg quads, geometry box-draw). Do not steal the per-cell `shape_line` loop; tty7 and Zed actually batch.

## Resize (Unix-shaped; re-evaluate on ConPTY)

`view.rs` `Render`: inside `canvas` paint, if `cols != term.columns() || rows != screen_lines()`, call `resize_callback(cols, rows)` then `term.resize`. The README example resizes `portable-pty` there.

Live PTY resize on layout is **normal on Unix** (SIGWINCH does not rewrite the screen). On Windows ConPTY it is the smear we hit. Their README is Linux-centric and `on_scroll` is still TODO — not a proven Windows design. Keep 013; re-eval conditions in [open-questions.md](open-questions.md) §1. Also uses `canvas()`, which we already found is `FnOnce` and blanks on resize.

## Input (useful, small)

`input.rs` is ~260 lines + tests: enter/tab/esc/arrows with `TermMode::APP_CURSOR` (`CSI` vs `SS3`), F1–F12, Ctrl+A–Z, Alt+key as ESC+key, `key_char` for shifted printable.

We already send `KeyCode` through mux (`map_keystroke`). WezTerm’s encoder owns APP_CURSOR. Use this file as a **checklist** of keys we might still drop, not as bytes to write to ConPTY ourselves.

Gaps vs us: they have no chrome-key filter; we swallow Ctrl+T/W/Q/P and Ctrl+Shift+P.

## Mouse / clipboard

`mouse.rs` documents SGR 1006 and has `Selection` types; README says mouse selection and scrollback nav are **not implemented**. Clipboard is OSC 52 via callback + `arboard`. Mux already has `AssignClipboard`.

## Steal vs skip

**Steal (ideas / small rewrite):**

- Measure cell from the text system (or keep wezterm-font metrics — see steal-list).
- Merge adjacent background quads (we already paint per-cell quads; merging is cheap).
- Box-drawing as paths with thickness rounded to device pixels (`calculate_thickness`).
- Compact key checklist + tests.

**Skip:**

- `alacritty_terminal`, `portable-pty` host, crates.io gpui, `canvas`, PTY resize callback, unused `BatchedTextRun` paint, `arboard` (use GPUI/mux clipboard).
- Linux-centric README (“X11 and Wayland”); Windows is an afterthought here.
