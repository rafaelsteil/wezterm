# Tab/shell splitting (040)

User asked to implement pane splits, then activate them from the command palette.

## wezterm-gui

- `SplitHorizontal` = left/right, new program on the **right**.
- `SplitVertical` = top/bottom, new program on the **bottom**.
- Mux `Tab` holds a binary tree (`split_and_insert`); spawn is `LocalDomain::split_pane` / `Mux::split_pane`.

## In this slice

GPUI tabs were one `TermPane` each, spawned via `LocalDomain::spawn_pane` **without** a mux `Tab`. `Mux::split_pane` needs a tab id, so the first slice keeps a **GPUI-side** tree:

- `split_layout.rs` binary tree of pane indices (H/V, 50/50 via gpui-component `h_resizable` / `v_resizable`).
- New leaf is another `spawn_pane` using the focused pane’s shell profile.
- Click emits `TermPaneEvent::Activated`; keys / copy / paste go to that leaf.
- `exit` on a split sibling removes that leaf; last pane of last tab still quits (033).
- Palette `SplitHorizontal` / `SplitVertical` are **Wired**. `CloseCurrentPane` closes the focused leaf.

Out of slice: pane-select overlay, rotate, zoom, programmatic adjust-size, mux `Tab` as source of truth.

Record: `docs/decisions/040-tab-shell-splits.json`.
