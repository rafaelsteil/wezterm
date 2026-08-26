//! GPUI proof-of-concept UI for WezTerm.
//!
//! This crate is a sibling to `wezterm-gui`. It does not replace the existing
//! window/box-model stack. See `docs/` for migration state.

// Linked with `sys-freetype` (GPUI's `freetype-sys`). Glyph atlas is not wired yet.
#[allow(unused_imports)]
use wezterm_font as _;
#[allow(unused_imports)]
use freetype_sys as _;

mod confirm;
mod hello;
mod mux_host;
mod palette;
pub mod shell;
mod term_pane;

pub use hello::HelloWorld;
pub use palette::CommandPalette;
pub use shell::{bind_keys, AppShell};
