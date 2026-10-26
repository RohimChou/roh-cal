//! Default locations of the files roh-cal uses
//!
//! Follows the XDG spec (the Linux convention; developer tools such as nvim and ghostty use it on macOS too),
//! not macOS's `~/Library/Application Support`

use std::ffi::OsString;
use std::path::{Path, PathBuf};

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

/// Sample task file for `--sample`: `$XDG_CACHE_HOME/roh-cal/sample.md`, or `~/.cache/roh-cal/sample.md` if
/// unset (a cache: every `--sample` writes a fresh copy)
pub fn sample_file() -> Option<PathBuf> {
    Some(
        xdg_dir("XDG_CACHE_HOME", ".cache")?
            .join("roh-cal")
            .join("sample.md"),
    )
}

/// Marker that the intro has played (it plays only on the very first run): `$XDG_STATE_HOME/roh-cal/splash-seen`,
/// or `~/.local/state/roh-cal/splash-seen` if unset (state: kept between runs, nothing worth backing up)
pub fn splash_seen_file() -> Option<PathBuf> {
    Some(
        xdg_dir("XDG_STATE_HOME", ".local/state")?
            .join("roh-cal")
            .join("splash-seen"),
    )
}

/// How the help page writes `path`, and whether the file is there. The path is where the file really is:
/// symlinks followed (the same file roh-cal writes to), a relative `--tasks` path made absolute, the home
/// folder written as `~`
pub fn shown(path: &Path) -> (String, bool) {
    let (real, exists) = match std::fs::canonicalize(path) {
        Ok(real) => (real, true),
        // Not there yet: no symlink to follow, just make it absolute
        Err(_) => (
            std::path::absolute(path).unwrap_or_else(|_| path.to_path_buf()),
            false,
        ),
    };
    // Follow symlinks in HOME too, or a followed path would not start with it
    let home = std::env::var_os("HOME")
        .map(PathBuf::from)
        .map(|home| std::fs::canonicalize(&home).unwrap_or(home));
    (with_tilde(&real, home.as_deref()), exists)
}

/// Writes `path` with the home folder `home` as `~`, e.g. `/Users/me/.config/x` → `~/.config/x`. A path
/// outside it, no home, or a home of `/` (every path would start with `~`) leaves it as it is
fn with_tilde(path: &Path, home: Option<&Path>) -> String {
    let rest = home
        .filter(|home| home.parent().is_some())
        .and_then(|home| path.strip_prefix(home).ok());
    match rest {
        Some(rest) if rest.as_os_str().is_empty() => "~".to_string(),
        Some(rest) => format!("~/{}", rest.display()),
        None => path.display().to_string(),
    }
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

    #[test]
    fn home_folder_is_written_as_a_tilde() {
        let home = Some(Path::new("/Users/me"));
        let tilde = |path: &str| with_tilde(Path::new(path), home);
        assert_eq!(
            tilde("/Users/me/.config/roh-cal/config.toml"),
            "~/.config/roh-cal/config.toml"
        );
        assert_eq!(tilde("/Users/me"), "~");
        // Only whole folder names count: /Users/meg is not inside /Users/me
        assert_eq!(tilde("/Users/meg/tasks.md"), "/Users/meg/tasks.md");
        assert_eq!(tilde("/tmp/tasks.md"), "/tmp/tasks.md");
        // No home, or a home of `/`: as it is
        assert_eq!(
            with_tilde(Path::new("/Users/me/tasks.md"), None),
            "/Users/me/tasks.md"
        );
        assert_eq!(
            with_tilde(Path::new("/Users/me/tasks.md"), Some(Path::new("/"))),
            "/Users/me/tasks.md"
        );
    }

    #[test]
    fn shown_path_is_the_real_file_and_says_whether_it_is_there() {
        let dir = crate::files::test_dir("shown");
        std::fs::create_dir_all(&dir).expect("dir");
        let real = dir.join("tasks.md");
        std::fs::write(&real, "").expect("write");
        let (path, exists) = shown(&real);
        assert!(exists);
        assert!(path.ends_with("/tasks.md"), "{path}");
        // A symlink shows the file it points to (the one roh-cal writes to)
        let link = dir.join("link.md");
        std::os::unix::fs::symlink(&real, &link).expect("symlink");
        assert_eq!(shown(&link), (path, true));
        // Not there yet: still absolute, `false`
        let (missing, exists) = shown(&dir.join("config.toml"));
        assert!(!exists);
        assert!(missing.ends_with("/config.toml"), "{missing}");
        // A relative path (e.g. `--tasks Cargo.toml`) is made absolute from the current folder
        let current = std::env::current_dir().expect("current dir");
        assert_eq!(
            shown(Path::new("Cargo.toml")),
            shown(&current.join("Cargo.toml"))
        );
        assert!(shown(Path::new("Cargo.toml")).0 != "Cargo.toml");
    }
}
