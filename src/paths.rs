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

/// 設定檔：`$XDG_CONFIG_HOME/roh-cal/config-v2.toml`，沒設就是 `~/.config/roh-cal/config-v2.toml`
///
/// 跟 v1 的 `config.toml` 放在同一個資料夾、但不同檔名：兩版的開關名字不一樣（v1 不認識的名字算錯誤），
/// 用同一個檔案會互相弄壞；v2 取代 v1 時再改回 `config.toml`
pub fn config_file() -> Option<PathBuf> {
    Some(
        xdg_dir("XDG_CONFIG_HOME", ".config")?
            .join("roh-cal")
            .join("config-v2.toml"),
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
            resolve(Some("/tmp/data".into()), home(), ".local/share"),
            Some(PathBuf::from("/tmp/data"))
        );
        // 沒設、空字串、相對路徑 → ~/.local/share
        for value in [None, Some("".into()), Some("data".into())] {
            assert_eq!(
                resolve(value.clone(), home(), ".local/share"),
                Some(PathBuf::from("/Users/me/.local/share")),
                "{value:?}"
            );
        }
        // 連 HOME 都沒有
        assert_eq!(resolve(None, None, ".local/share"), None);
    }
}
