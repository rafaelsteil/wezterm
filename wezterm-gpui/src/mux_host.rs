//! Mux bootstrap for the GPUI POC.
//!
//! wezterm-gui hosts shells via `mux::domain::LocalDomain::spawn_pane` (ConPTY +
//! `wezterm-term` inside `LocalPane`). This module uses that same path so the
//! GPUI chrome is not a second, homemade PTY host.
//!
//! Still not `wezterm-gui` or `window/`. Paint is wezterm-font sprites
//! in `glyph_paint.rs` (Consolas text fallback in `term_pane.rs`).

use std::ffi::OsString;
use std::sync::{Arc, OnceLock};

use anyhow::Context as _;
use mux::domain::{Domain, LocalDomain};
use mux::pane::Pane;
use mux::Mux;
use portable_pty::CommandBuilder;
use wezterm_term::TerminalSize;

static INIT: OnceLock<Result<(), String>> = OnceLock::new();

pub fn ensure_init() -> anyhow::Result<()> {
    INIT.get_or_init(|| init_inner().map_err(|err| format!("{err:#}")))
        .as_ref()
        .map(|_| ())
        .map_err(|err| anyhow::anyhow!("{err}"))
}

fn init_inner() -> anyhow::Result<()> {
    config::designate_this_as_the_main_thread();
    // Skip wezterm.lua so a user `default_prog` cannot replace cmd.exe.
    // Disable the mux SSH agent: it creates runtime-dir symlinks we do not need.
    config::common_init(
        None,
        &[("mux_enable_ssh_agent".to_string(), "false".to_string())],
        true,
    )
    .context("config::common_init")?;

    // Mux PTY threads call `promise::spawn::spawn_into_main_thread`. GPUI owns
    // GetMessage; we must not take `window/`'s spawn queue. A dedicated
    // SimpleExecutor thread is the mux "main" for those notifications.
    let exec = promise::spawn::SimpleExecutor::new();
    std::thread::Builder::new()
        .name("wezterm-gpui-mux".into())
        .spawn(move || loop {
            if exec.tick().is_err() {
                break;
            }
        })
        .context("mux promise executor thread")?;

    let domain: Arc<dyn Domain> = Arc::new(LocalDomain::new("local").context("LocalDomain::new")?);
    let mux = Arc::new(Mux::new(Some(Arc::clone(&domain))));
    Mux::set_mux(&mux);
    Ok(())
}

/// Spawn Windows `cmd.exe` (or `%ComSpec%`) through mux `LocalDomain`.
///
/// This is the same host wezterm-gui uses for a local pane: `LocalDomain`
/// opens ConPTY, builds `wezterm-term::Terminal`, wraps `LocalPane`, and the
/// mux PTY reader thread feeds parsed actions into that pane.
pub fn spawn_cmd_exe(size: TerminalSize) -> anyhow::Result<Arc<dyn Pane>> {
    ensure_init()?;
    let domain = Mux::get().default_domain();
    let prog = std::env::var_os("ComSpec").unwrap_or_else(|| OsString::from("cmd.exe"));
    let cmd = CommandBuilder::new(prog);
    promise::spawn::block_on(domain.spawn_pane(size, Some(cmd), None))
        .context("LocalDomain::spawn_pane cmd.exe")
}
