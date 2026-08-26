# ADR 0001: Use Zed official GPUI and gpui-component; keep existing UI

- Status: accepted
- Date: 2026-08-26

## Context

WezTerm's GUI is a custom window crate plus a box-model chrome layer and termwiz overlays. The goal is less UI code and more widget capability via GPUI. gpui-ce was considered as a Zed fork; gpui-component is developed against Zed git GPUI.

## Decision

1. Depend on **Zed official GPUI** (`git = https://github.com/zed-industries/zed`), not gpui-ce and not crates.io `gpui` 0.2.2.
2. Use **gpui-component** (`git = https://github.com/longbridge/gpui-component`) for widgets.
3. **Do not drop** the existing `wezterm-gui` / `window` / box-model / overlay stacks. Build a sibling crate (`wezterm-gpui`) and replace surfaces one at a time.
4. Do not share event loops in-process. GPUI windows are sibling windows or a sibling process until a later windowing cutover.

## Consequences

- Dual-stack until cutover means more code, not less.
- Must pin git SHAs; unpinned Zed + gpui-component HEAD will drift.
- In-window overlays (true embedding) are out of scope until a custom GPUI `Platform` or a full `window/` replacement.
