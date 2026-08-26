# ADR 0003: GPUI graph owns FreeType (`freetype-sys`)

- Status: accepted
- Date: 2026-08-26

## Context

Cargo allows only one crate per `links` value in a dependency graph. WezTerm's vendored `deps/freetype` and GPUI's `freetype-sys` both use `links = "freetype"`. That is why `wezterm-gpui` is an isolated workspace (ADR 0002).

Putting `wezterm-font` into the GPUI graph still pulled vendored FreeType (HarfBuzz default features) and WezTerm `fontconfig` (`links = "fontconfig"` vs GPUI `yeslogic-fontconfig-sys`). Optional `freetype-sys` on a root-workspace crate also fails lockfile resolution: Cargo still sees both `links = "freetype"` owners even when features are mutually exclusive.

## Decision

1. **wezterm-gui / root workspace:** unchanged. `wezterm-font` default features are `vendored-freetype` + `native-fontconfig`. Vendored `deps/freetype` remains the `links = "freetype"` owner.
2. **wezterm-gpui graph:** GPUI's `freetype-sys` `=0.20.1` is the only FreeType. `wezterm-gpui` depends on it explicitly. `wezterm-font` is path-dep with `default-features = false, features = ["sys-freetype"]`.
3. **`gpui-freetype` (`deps/freetype-from-sys`):** reuses WezTerm's bindgen + fixed-point wrappers (`deps/freetype/src/lib.rs`) and does **not** depend on `freetype-sys` (would poison the root lockfile).
4. **HarfBuzz:** features `vendored-freetype` (default) vs `sys-freetype` (headers from the `freetype-sys` crate sources; no `freetype-sys` crate dep). `wezterm-font` path-deps HarfBuzz with `default-features = false` so workspace inheritance cannot re-enable vendored FreeType.
5. **Unix fontconfig:** `wezterm-font` feature `native-fontconfig` (default on for wezterm-gui). Off in wezterm-gpui so GPUI's fontconfig-sys owns `links = "fontconfig"`.

## Consequences

- Isolated workspace is still required; the two graphs must not merge while wezterm-gui vendors FreeType.
- Bindgen types are WezTerm's; the C library is freetype-sys 0.20. COLR v1 / `FT_Error_String` may be missing at runtime even if `cargo check` is green.
- Glyph atlas / GPUI `Element` is still not wired. This ADR only makes `wezterm-font` compilable in the GPUI binary.
- Cairo in the GPUI graph is WezTerm's vendored `deps/cairo` (FreeType disabled) via `[patch.crates-io]`.
