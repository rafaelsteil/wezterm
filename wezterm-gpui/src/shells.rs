//! Built-in local shell profiles for the GPUI new-tab menu.
//!
//! Plus / Ctrl+T still spawn the first profile (Command Prompt / `%ComSpec%`).
//! The chevron lists that plus Windows PowerShell, and PowerShell 7 (`pwsh`)
//! when it is installed. Lua `default_prog` / `launch_menu` stay out (020).

use std::ffi::OsString;
use std::path::PathBuf;

use portable_pty::CommandBuilder;

/// A spawnable local program shown in the new-tab dropdown.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ShellProfile {
    pub id: &'static str,
    pub label: String,
    /// `None` means `CommandBuilder::new_default_prog()` (non-Windows fallback).
    pub argv: Option<Vec<OsString>>,
}

impl ShellProfile {
    pub fn command(&self) -> CommandBuilder {
        match &self.argv {
            Some(argv) => CommandBuilder::from_argv(argv.clone()),
            None => CommandBuilder::new_default_prog(),
        }
    }
}

/// Profiles for the dropdown. First entry is Plus / Ctrl+T.
pub fn available_shells() -> Vec<ShellProfile> {
    let mut out = vec![command_prompt()];
    #[cfg(windows)]
    {
        out.push(windows_powershell());
        if let Some(pwsh) = powershell_core() {
            out.push(pwsh);
        }
    }
    out
}

pub fn default_shell() -> ShellProfile {
    available_shells()
        .into_iter()
        .next()
        .unwrap_or_else(command_prompt)
}

fn command_prompt() -> ShellProfile {
    #[cfg(windows)]
    let argv = {
        let prog = std::env::var_os("ComSpec").unwrap_or_else(|| OsString::from("cmd.exe"));
        Some(vec![prog])
    };
    #[cfg(not(windows))]
    let argv = None;
    ShellProfile {
        id: "cmd",
        label: if cfg!(windows) {
            "Command Prompt".into()
        } else {
            "Default shell".into()
        },
        argv,
    }
}

#[cfg(windows)]
fn windows_powershell() -> ShellProfile {
    let exe = system_root()
        .map(|root| {
            root.join("System32")
                .join("WindowsPowerShell")
                .join("v1.0")
                .join("powershell.exe")
        })
        .filter(|p| p.is_file())
        .unwrap_or_else(|| PathBuf::from("powershell.exe"));
    ShellProfile {
        id: "powershell",
        label: "Windows PowerShell".into(),
        argv: Some(vec![exe.into()]),
    }
}

#[cfg(windows)]
fn powershell_core() -> Option<ShellProfile> {
    let exe = pwsh_candidates().into_iter().find(|p| p.is_file())?;
    Some(ShellProfile {
        id: "pwsh",
        label: "PowerShell".into(),
        argv: Some(vec![exe.into()]),
    })
}

#[cfg(windows)]
fn system_root() -> Option<PathBuf> {
    std::env::var_os("SystemRoot").map(PathBuf::from)
}

#[cfg(windows)]
fn pwsh_candidates() -> Vec<PathBuf> {
    let mut out = Vec::new();
    if let Some(pf) = std::env::var_os("ProgramFiles") {
        let base = PathBuf::from(pf).join("PowerShell");
        out.push(base.join("7").join("pwsh.exe"));
        out.push(base.join("6").join("pwsh.exe"));
    }
    if let Some(found) = find_on_path("pwsh.exe") {
        if !out.iter().any(|p| p == &found) {
            out.push(found);
        }
    }
    out
}

#[cfg(windows)]
fn find_on_path(name: &str) -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    for dir in std::env::split_paths(&path) {
        let candidate = dir.join(name);
        if candidate.is_file() {
            return Some(candidate);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_is_command_prompt() {
        let shells = available_shells();
        assert!(!shells.is_empty());
        assert_eq!(shells[0].id, "cmd");
        assert_eq!(default_shell().id, "cmd");
    }

    #[cfg(windows)]
    #[test]
    fn windows_lists_powershell() {
        let shells = available_shells();
        assert!(
            shells.iter().any(|s| s.id == "powershell"),
            "expected Windows PowerShell in {shells:?}"
        );
        let ps = shells.iter().find(|s| s.id == "powershell").unwrap();
        assert_eq!(ps.label, "Windows PowerShell");
        let argv = ps.argv.as_ref().expect("powershell argv");
        assert!(
            argv[0]
                .to_string_lossy()
                .to_ascii_lowercase()
                .contains("powershell"),
            "argv={argv:?}"
        );
    }
}
