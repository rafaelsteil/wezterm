//! GPUI proof-of-concept UI for WezTerm.
//!
//! This crate is a sibling to `wezterm-gui`. It does not replace the existing
//! window/box-model stack. See `docs/` for migration state.

mod confirm;
mod hello;
mod palette;
pub mod shell;
mod term_pane;

pub use hello::HelloWorld;
pub use palette::CommandPalette;
pub use shell::{bind_keys, AppShell};
