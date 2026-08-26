//! Spawn `wezterm-gpui` as a child process. Does not share WezTerm's event loop.

use anyhow::{anyhow, Context};
use clap::Parser;
use std::ffi::OsString;
use std::path::PathBuf;
use std::process::Command;

#[derive(Debug, Parser, Clone)]
pub struct GpuiCommand {
    /// Open the hello/button smoke window instead of the command palette replica.
    #[arg(long)]
    pub hello: bool,

    /// Extra arguments forwarded to wezterm-gpui.
    #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
    pub args: Vec<OsString>,
}

pub fn run(cmd: GpuiCommand) -> anyhow::Result<()> {
    let exe = find_wezterm_gpui().context("could not find wezterm-gpui binary")?;
    let mut child = Command::new(&exe);
    if cmd.hello {
        child.arg("--hello");
    }
    child.args(&cmd.args);

    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        let err = child.exec();
        return Err(anyhow!("failed to exec {exe:?}: {err:?}"));
    }

    #[cfg(windows)]
    {
        let status = child
            .spawn()
            .with_context(|| format!("failed to spawn {exe:?}"))?
            .wait()?;
        std::process::exit(status.code().unwrap_or(1));
    }
}

fn find_wezterm_gpui() -> anyhow::Result<PathBuf> {
    if let Some(explicit) = std::env::var_os("WEZTERM_GPUI") {
        let path = PathBuf::from(explicit);
        if path.exists() {
            return Ok(path);
        }
        return Err(anyhow!(
            "WEZTERM_GPUI is set to {} but that path does not exist",
            path.display()
        ));
    }

    let name = if cfg!(windows) {
        "wezterm-gpui.exe"
    } else {
        "wezterm-gpui"
    };

    let mut candidates: Vec<PathBuf> = Vec::new();

    if let Ok(current) = std::env::current_exe() {
        if let Some(dir) = current.parent() {
            candidates.push(dir.join(name));
            // Root workspace target/debug vs isolated wezterm-gpui/target/debug.
            candidates.push(
                dir.join("..")
                    .join("..")
                    .join("wezterm-gpui")
                    .join("target")
                    .join("debug")
                    .join(name),
            );
            candidates.push(
                dir.join("..")
                    .join("..")
                    .join("wezterm-gpui")
                    .join("target")
                    .join("release")
                    .join(name),
            );
        }
    }

    if let Ok(cwd) = std::env::current_dir() {
        candidates.push(cwd.join("wezterm-gpui").join("target").join("debug").join(name));
        candidates.push(
            cwd.join("wezterm-gpui")
                .join("target")
                .join("release")
                .join(name),
        );
        candidates.push(cwd.join("target").join("debug").join(name));
    }

    for path in &candidates {
        if path.exists() {
            return Ok(std::fs::canonicalize(path).unwrap_or_else(|_| path.clone()));
        }
    }

    // Last resort: hope it is on PATH.
    Ok(PathBuf::from(name))
}
