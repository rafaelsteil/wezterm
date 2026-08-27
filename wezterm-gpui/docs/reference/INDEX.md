# External GPUI terminal references

Read this index when implementing paint, resize, input, box-drawing, or a GPU Element. Checkouts live **outside** this repo (do not path-dep them):

| Checkout | HEAD (cloned 2026-08-26) | License | Role |
|---|---|---|---|
| `D:\dev\gpui-terminal` | `51f0292938876c8da3de03f0139088591e3be518` | MIT OR Apache-2.0 | Small library. Old crates.io `gpui 0.2.2`. |
| `D:\dev\tty7` | `d4b8e331b970d4c500c3e659a42365f225b4d1cf` | Apache-2.0 | Full workbench. Zed git GPUI + custom Element. |

Third local source (already in our Cargo git cache, **GPL-3.0-or-later — read only, never copy**):

- Zed `crates/terminal_view/src/terminal_element.rs` at pin `fecc3273ed32643c2ea1b04a74c8780e2c9ffaf8`

## When implementing X, open Y

| Task | Open first | Then |
|---|---|---|
| **Term rendering quality (current)** | [rendering-quality.md](rendering-quality.md) | [steal-list.md](steal-list.md) |
| **Lua config keys** | [lua-config.md](lua-config.md) | [`docs/lua-config.json`](../lua-config.json) |
| **Command palette ops** | [command-palette.md](command-palette.md) | [`docs/command-palette.json`](../command-palette.json) |
| **Launcher ops** | [launcher.md](launcher.md) | [`docs/launcher.json`](../launcher.json) |
| **Scrollback** | [scrollback.md](scrollback.md) | wezterm-gui `set_viewport` / `scroll_by_line` |
| GPUI `text_system` vs sprites | [gpui-text-vs-sprites.md](gpui-text-vs-sprites.md) | [tty7.md](tty7.md) `element.rs` |
| GPUI `shape_line` / Element paint | [tty7.md](tty7.md) (`element.rs`) | Zed `terminal_element.rs` (ideas only) |
| Compact keyboard / OSC 52 / mouse reports | [gpui-terminal.md](gpui-terminal.md) (`input.rs`, `mouse.rs`) | wezterm-term `Pane::key_down` (we already map to that) |
| Geometry box-draw / powerline | `wezterm-gpui/src/boxdraw.rs` (023; U+2500–259F) | tty7 `src/terminal/boxdraw.rs` (Apache); gpui-terminal `src/box_drawing.rs`. Powerline later. |
| Device-pixel snap, `force_width`, content mask | tty7 `element.rs` `paint_glyphs` / `prepaint` | Zed prepaint snap (ideas only) |
| PTY resize / ConPTY | Drag: keep 013. Stable grid: 016 committed resize — [rendering-quality.md](rendering-quality.md) | [open-questions.md](open-questions.md) §1 if smear returns |
| Inline images / kitty graphics | tty7 `images.rs` | later; not this POC |
| Windows GPUI italic-fallback mojibake | tty7 `Cargo.toml` gpui-fork comments | do not take their `l0ng-ai/zed` pin |

## Do not adopt

- `alacritty_terminal` as VT core (keep mux + `wezterm-term`).
- crates.io `gpui 0.2.2` (gpui-terminal). We use Zed git via gpui-component + lockfile.
- tty7’s `l0ng-ai/zed` / `l0ng-ai/gpui-component` forks as Cargo deps.
- `canvas()` as the terminal widget (gpui-terminal). We already learned it is `FnOnce` and blanks on resize.
- PTY/`ResizePseudoConsole` on every layout (decision 013). A **committed** resize after the grid is still is 016 — not the same as live drag.
- Copying Zed `terminal_element.rs` into this MIT tree (GPL-3.0-or-later).
- tty7 workbench: `view.rs` (~494KB), daemon, SSH, agents, git sidebar, `generator.rs` (Fig completion scripts).

Copy snippets from gpui-terminal / tty7 only with their license headers kept, rewritten against `wezterm-term` cell types. Prefer rewrite over paste.

Decisions: [014](../decisions/014-external-gpui-terminals.json), [015](../decisions/015-next-is-scrollback.json), [016](../decisions/016-term-render-quality.json), [017](../decisions/017-line-sprites-and-coalesce.json), [018](../decisions/018-conpty-junk-also-wezterm-gui.json).
