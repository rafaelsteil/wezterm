# GPUI icon assets (028)

User after 027: Plus is a blank square; title-bar min/max/close are missing. Same cause: `IconName` paths (`icons/plus.svg`, `icons/window-close.svg`, …) need an `AssetSource`. gpui-component does **not** embed SVGs; Longbridge docs require `gpui-component-assets` + `with_assets`.

Native Windows caption buttons are off because `TitleBar::title_bar_options()` sets `appears_transparent: true`; GPUI `TitleBar` draws the controls itself via the same icons.

## In this slice

- Direct dep `gpui-component-assets` at the same `rev` as `gpui-component` (`ff3eb112`).
- `gpui_platform::application().with_assets(Assets)` in `main.rs`.
- Zed SHA stays `pins.gpui_rev` (crate was already a transitive dep).

## Out

- Custom SVG set / rust-embed of a subset
- Changing title-bar chrome beyond making the stock icons load
- 026 monitor-move hang

Record: `docs/decisions/028-gpui-component-icon-assets.json`.

**User-ok 2026-08-27** (“works well”). Plus / tab close / title-bar min/max/close visible.
