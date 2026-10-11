//! 檔案的共用工具：安全寫檔（設定檔、任務檔都用）、test 用的暫存資料夾

use std::path::Path;

/// 安全地寫檔：先寫到旁邊的暫存檔（例：`config.toml.tmp`），再 rename 蓋過去。
/// rename 是一次完成的，寫到一半當掉也不會留下寫壞的檔案；資料夾不存在會自動建立
pub fn write_atomically(path: &Path, text: &str) -> std::io::Result<()> {
    // 檔案是 symlink（例：dotfiles 統一管理）就寫到它指向的真正檔案，symlink 才不會被換成一般檔案
    let path = std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let mut temp = path.clone().into_os_string();
    temp.push(".tmp");
    std::fs::write(&temp, text)?;
    std::fs::rename(&temp, &path)?;
    Ok(())
}

/// test 用的暫存資料夾（每次先清空）：`$TMPDIR/roh-cal-<pid>-<name>`，不會碰到真的 `~/.config`
#[cfg(test)]
pub fn test_dir(name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("roh-cal-{}-{name}", std::process::id()));
    // 上次留下的先刪掉；本來就沒有也沒關係
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
        // 資料夾還不存在：自動建立
        write_atomically(&path, "- [ ] 週會\n").expect("write");
        assert_eq!(
            std::fs::read_to_string(&path).expect("read"),
            "- [ ] 週會\n"
        );
        // 已經有檔案：整個換成新的內容
        write_atomically(&path, "- [x] 週會\n").expect("overwrite");
        assert_eq!(
            std::fs::read_to_string(&path).expect("read"),
            "- [x] 週會\n"
        );
        // 暫存檔沒有留下來
        assert!(!dir.join("roh-cal").join("tasks.md.tmp").exists());
        std::fs::remove_dir_all(&dir).expect("clean up");
    }

    #[test]
    fn write_atomically_writes_through_a_symlink() {
        let dir = test_dir("symlink");
        std::fs::create_dir_all(&dir).expect("dir");
        // dotfiles 的寫法：真正的檔案在別處，config.toml 是指過去的 symlink
        let real = dir.join("dotfiles-config.toml");
        let link = dir.join("config.toml");
        std::fs::write(&real, "[month]\ngrid_lines = true\n").expect("write");
        std::os::unix::fs::symlink(&real, &link).expect("symlink");
        write_atomically(&link, "[month]\ngrid_lines = false\n").expect("write");
        // symlink 還是 symlink，內容寫進真正的檔案
        let file_type = std::fs::symlink_metadata(&link).expect("meta").file_type();
        assert!(file_type.is_symlink());
        assert_eq!(
            std::fs::read_to_string(&real).expect("read"),
            "[month]\ngrid_lines = false\n"
        );
        std::fs::remove_dir_all(&dir).expect("clean up");
    }
}
