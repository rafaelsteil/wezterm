# ADR 0002: Isolate wezterm-gpui as its own Cargo workspace

- Status: accepted
- Date: 2026-08-26

## Context

Adding `wezterm-gpui` as a root workspace member failed cargo resolution:

`zed-font-kit` → `freetype-sys ^0.20` (`links = "freetype"`) conflicts with WezTerm's vendored `deps/freetype` (`links = "freetype"`). Cargo allows only one crate per `links` value in a graph.

## Decision

- Exclude `wezterm-gpui` from the root workspace (`Cargo.toml` `exclude`).
- Give `wezterm-gpui` its own `[workspace]` and `Cargo.lock`.
- Check/run via `--manifest-path wezterm-gpui/Cargo.toml`, not `cargo check -p wezterm-gpui` from the repo root.

## Consequences

- Two lockfiles. Root `cargo check` does not compile GPUI.
- Duplicate native stacks until cutover: wezterm-gui still vendors `deps/freetype`; the GPUI graph uses only `freetype-sys` (ADR 0003). The two graphs must not merge.
- Git pins for Zed and gpui-component live in `wezterm-gpui/Cargo.toml` `rev` and `docs/STATE.json` `pins`.
