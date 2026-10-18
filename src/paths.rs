//! roh-cal 用到的檔案的預設位置
//!
//! 照 XDG 規範放（Linux 的慣例；macOS 上 nvim、ghostty 這類開發工具也這樣放），
//! 不用 macOS 的 `~/Library/Application Support`

use std::ffi::OsString;
use std::path::PathBuf;

/// 任務檔：`$XDG_DATA_HOME/roh-cal/tasks.md`，沒設就是 `~/.local/share/roh-cal/tasks.md`
pub fn tasks_file() -> Option<PathBuf> {
    Some(
        xdg_dir("XDG_DATA_HOME", ".local/share")?
            .join("roh-cal")
            .join("tasks.md"),
    )
}

/// 設定檔：`$XDG_CONFIG_HOME/roh-cal/config-v1.toml`，沒設就是 `~/.config/roh-cal/config-v1.toml`
/// （`config.toml` 給 v2 用；兩版的設定名字不一樣，共用一個檔會互相讀不了）
pub fn config_file() -> Option<PathBuf> {
    Some(
        xdg_dir("XDG_CONFIG_HOME", ".config")?
            .join("roh-cal")
            .join("config-v1.toml"),
    )
}

/// 讀環境變數 `var` 和 `HOME`，交給 `resolve` 決定用哪個資料夾
fn xdg_dir(var: &str, fallback: &str) -> Option<PathBuf> {
    resolve(std::env::var_os(var), std::env::var_os("HOME"), fallback)
}

/// XDG 資料夾：環境變數有設、而且是絕對路徑就用它；沒設、空字串、相對路徑（規範說要忽略）都改用 `~/<fallback>`。
/// 連 `HOME` 都沒有就是 `None`（找不到放檔案的地方）
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
        // 有設、絕對路徑 → 用它
        assert_eq!(
            resolve(Some("/tmp/conf".into()), home(), ".config"),
            Some(PathBuf::from("/tmp/conf"))
        );
        // 沒設、空字串、相對路徑 → ~/.config
        for value in [None, Some("".into()), Some("conf".into())] {
            assert_eq!(
                resolve(value.clone(), home(), ".config"),
                Some(PathBuf::from("/Users/me/.config")),
                "{value:?}"
            );
        }
        // 連 HOME 都沒有 → 找不到地方放
        assert_eq!(resolve(None, None, ".config"), None);
    }
}
