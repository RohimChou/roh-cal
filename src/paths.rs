//! Default locations of the files roh-cal uses
//!
//! Follows the XDG spec (the Linux convention; developer tools such as nvim and ghostty use it on macOS too),
//! not macOS's `~/Library/Application Support`

use std::ffi::OsString;
use std::path::PathBuf;

/// Task file: `$XDG_DATA_HOME/roh-cal/tasks.md`, or `~/.local/share/roh-cal/tasks.md` if unset
pub fn tasks_file() -> Option<PathBuf> {
    Some(
        xdg_dir("XDG_DATA_HOME", ".local/share")?
            .join("roh-cal")
            .join("tasks.md"),
    )
}

/// Config file: `$XDG_CONFIG_HOME/roh-cal/config.toml`, or `~/.config/roh-cal/config.toml` if unset
pub fn config_file() -> Option<PathBuf> {
    Some(
        xdg_dir("XDG_CONFIG_HOME", ".config")?
            .join("roh-cal")
            .join("config.toml"),
    )
}

/// Reads the environment variables `var` and `HOME` and lets `resolve` pick the folder
fn xdg_dir(var: &str, fallback: &str) -> Option<PathBuf> {
    resolve(std::env::var_os(var), std::env::var_os("HOME"), fallback)
}

/// XDG folder: use the variable if it is set and absolute. If it is unset, empty, or relative (the spec says
/// to ignore those), use `~/<fallback>`.
/// Without even `HOME` it is `None` (nowhere to put the files)
fn resolve(value: Option<OsString>, home: Option<OsString>, fallback: &str) -> Option<PathBuf> {
    match value.map(PathBuf::from).filter(|dir| dir.is_absolute()) {
        Some(dir) => Some(dir),
        None => Some(PathBuf::from(home?).join(fallback)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn xdg_variable_is_used_only_when_it_is_an_absolute_path() {
        let home = || Some(OsString::from("/Users/me"));
        // Set and absolute → use it
        assert_eq!(
            resolve(Some("/tmp/data".into()), home(), ".local/share"),
            Some(PathBuf::from("/tmp/data"))
        );
        // Unset, empty, or relative → ~/.local/share
        for value in [None, Some("".into()), Some("data".into())] {
            assert_eq!(
                resolve(value.clone(), home(), ".local/share"),
                Some(PathBuf::from("/Users/me/.local/share")),
                "{value:?}"
            );
        }
        // Not even HOME
        assert_eq!(resolve(None, None, ".local/share"), None);
    }
}
