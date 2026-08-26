//! The FreeType FFI crate. Either WezTerm's vendored bindings or the GPUI-owned
//! `freetype-sys` wrapper (`gpui-freetype`).

#[cfg(all(feature = "vendored-freetype", feature = "sys-freetype"))]
compile_error!("enable only one of wezterm-font features: vendored-freetype, sys-freetype");

#[cfg(not(any(feature = "vendored-freetype", feature = "sys-freetype")))]
compile_error!("enable wezterm-font feature vendored-freetype or sys-freetype");

#[cfg(feature = "vendored-freetype")]
pub use ::freetype::*;

#[cfg(feature = "sys-freetype")]
pub use gpui_freetype::*;
