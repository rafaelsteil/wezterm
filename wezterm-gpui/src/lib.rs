//! GPUI proof-of-concept UI for WezTerm.
//!
//! This crate is a sibling to `wezterm-gui`. It does not replace the existing
//! window/box-model stack. See `docs/` for migration state.

// Keep `freetype-sys` in the rlib so this graph has a single `links = "freetype"` owner.
#[allow(unused_imports)]
use freetype_sys as _;

mod boxdraw;
mod commands;
mod confirm;
mod glyph_paint;
mod hello;
mod lua_ui;
mod mux_host;
mod palette;
pub mod shell;
mod shells;
mod split_layout;
mod term_pane;
mod win_zorder;

pub use hello::HelloWorld;
pub use palette::CommandPalette;
pub use shell::{AppShell, bind_keys};
