//! Writing files: write a temp file first, then rename it, so a crash mid-write never leaves a broken file

use std::path::Path;

/// Writes a file safely: first to a temp file beside it (e.g. `tasks.md.tmp`), then renames that over it;
/// a rename happens in one step, so a crash mid-write never leaves a broken file.
/// Creates the folder if it does not exist
pub fn write_atomically(path: &Path, text: &str) -> std::io::Result<()> {
    // If the file is a symlink (e.g. managed by dotfiles or a notes vault), write to the real file it points
    // to, so the symlink is not replaced by a plain file
    let path = std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let mut temp = path.clone().into_os_string();
    temp.push(".tmp");
    std::fs::write(&temp, text)?;
    std::fs::rename(&temp, &path)
}

/// Temp folder for tests (emptied first each time): `$TMPDIR/roh-cal-<pid>-<name>`,
/// so tests never touch the real task file
#[cfg(test)]
pub fn test_dir(name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("roh-cal-{}-{name}", std::process::id()));
    // Delete what the last run left behind; fine if there is nothing
    let _ = std::fs::remove_dir_all(&dir);
    dir
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn write_atomically_creates_the_folder_and_leaves_no_temp_file() {
        let dir = test_dir("write");
        let path = dir.join("roh-cal").join("tasks.md");
        // The folder does not exist yet: it is created
        write_atomically(&path, "- [ ] 週會\n").expect("write");
        assert_eq!(
            std::fs::read_to_string(&path).expect("read"),
            "- [ ] 週會\n"
        );
        // The file already exists: its whole content is replaced
        write_atomically(&path, "- [x] 週會\n").expect("overwrite");
        assert_eq!(
            std::fs::read_to_string(&path).expect("read"),
            "- [x] 週會\n"
        );
        // No temp file is left behind
        assert!(!dir.join("roh-cal").join("tasks.md.tmp").exists());
        std::fs::remove_dir_all(&dir).expect("clean up");
    }

    #[test]
    fn write_atomically_writes_through_a_symlink() {
        let dir = test_dir("symlink");
        std::fs::create_dir_all(&dir).expect("dir");
        // The real file lives elsewhere (e.g. a notes vault); tasks.md is a symlink to it
        let real = dir.join("vault-tasks.md");
        let link = dir.join("tasks.md");
        std::fs::write(&real, "- [ ] 週會\n").expect("write");
        std::os::unix::fs::symlink(&real, &link).expect("symlink");
        write_atomically(&link, "- [x] 週會\n").expect("write");
        // The symlink stays a symlink; the content goes into the real file
        let file_type = std::fs::symlink_metadata(&link).expect("meta").file_type();
        assert!(file_type.is_symlink());
        assert_eq!(
            std::fs::read_to_string(&real).expect("read"),
            "- [x] 週會\n"
        );
        std::fs::remove_dir_all(&dir).expect("clean up");
    }
}
