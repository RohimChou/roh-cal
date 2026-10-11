//! 設定檔 `config.toml`：啟動時讀進來，設定頁關掉時把改過的開關存回去
//!
//! 位置見 `paths::config_file`。名字跟 `Settings` 的欄位一樣，只寫想改的就好，沒寫的用預設值；
//! 範例見 `testdata/config.toml`。存檔只改有變的那幾行，其他行和註解一字不動（手寫的設定檔不會被蓋掉）

use crate::settings::Settings;
use color_eyre::{Result, eyre::WrapErr};
use std::io::ErrorKind;
use std::path::Path;
use toml_edit::{DocumentMut, Item};

/// 讀設定檔；檔案不存在就全部用預設值，讀不到或內容寫錯就回傳錯誤
pub fn load(path: &Path) -> Result<Settings> {
    let text = read(path)?;
    parse(&text).wrap_err_with(|| format!("invalid config file: {}", path.display()))
}

/// 把開關存回設定檔：跟檔案不一樣的值才改，沒有要改的就不寫；資料夾不存在會自動建立
pub fn save(path: &Path, settings: Settings) -> Result<()> {
    let text = read(path)?;
    let new_text = updated(&text, settings)
        .wrap_err_with(|| format!("invalid config file: {}", path.display()))?;
    // 只是打開設定頁看看、沒改東西，檔案就不碰
    if new_text == text {
        return Ok(());
    }
    write_atomically(path, &new_text)
        .wrap_err_with(|| format!("cannot write config file: {}", path.display()))
}

/// 讀設定檔的文字；檔案不存在就當作空的（空的 = 全部用預設值）
fn read(path: &Path) -> Result<String> {
    match std::fs::read_to_string(path) {
        Ok(text) => Ok(text),
        Err(error) if error.kind() == ErrorKind::NotFound => Ok(String::new()),
        Err(error) => {
            Err(error).wrap_err_with(|| format!("cannot read config file: {}", path.display()))
        }
    }
}

/// 把設定檔的內容轉成 `Settings`：沒寫的欄位用預設值；不認識的名字、值的型別不對都是錯誤
/// （錯誤訊息會指出第幾行哪裡錯）
fn parse(text: &str) -> Result<Settings> {
    Ok(toml::from_str(text)?)
}

/// 設定檔的新內容：值跟 `text` 現在不一樣的開關改掉，沒有那一行就加在其他開關的後面
/// （檔案結尾的註解還是留在最後面）；
/// 其他行、註解、空白一字不動
fn updated(text: &str, settings: Settings) -> Result<String> {
    // 保留格式的 TOML（toml_edit）：改一個值，其他內容原封不動
    let mut doc: DocumentMut = text.parse()?;
    // 檔案現在的值（沒寫的 = 預設值）和要存的值都轉成 TOML，一個名字一個名字比
    let current = toml_edit::ser::to_document(&parse(text)?)?;
    let wanted = toml_edit::ser::to_document(&settings)?;
    for (key, item) in wanted.iter() {
        // 兩邊是同一個 serializer 轉出來的，印出來一樣就是值一樣
        if current.get(key).map(ToString::to_string) == Some(item.to_string()) {
            continue;
        }
        match (
            doc.get_mut(key).and_then(Item::as_value_mut),
            item.as_value(),
        ) {
            // 檔案裡有這一行：只換值，行尾註解（存在舊值的 decor 裡）搬到新值上
            (Some(old), Some(new)) => {
                let decor = old.decor().clone();
                *old = new.clone();
                *old.decor_mut() = decor;
            }
            // 沒有這一行：加在其他開關的後面、檔案結尾的註解前面
            _ => doc[key] = item.clone(),
        }
    }
    Ok(doc.to_string())
}

/// 安全地寫檔：先寫到旁邊的暫存檔（`config.toml.tmp`），再 rename 蓋過去。
/// rename 是一次完成的，寫到一半當掉也不會留下寫壞的設定檔
fn write_atomically(path: &Path, text: &str) -> Result<()> {
    // 設定檔是 symlink（例：dotfiles 統一管理）就寫到它指向的真正檔案，symlink 才不會被換成一般檔案
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
    fn missing_fields_use_the_defaults() {
        assert_eq!(parse("").expect("empty file"), Settings::default());
        let settings = parse("grid_lines = false").expect("one field");
        assert_eq!(
            settings,
            Settings {
                grid_lines: false,
                ..Settings::default()
            }
        );
    }

    #[test]
    fn mistakes_are_errors() {
        for text in [
            // 名字打錯（少一個 s）
            "grid_line = false",
            // 型別不對：要 true / false，不是字串
            "grid_lines = \"no\"",
            // TOML 語法錯：等號後面沒有值
            "grid_lines =",
        ] {
            assert!(parse(text).is_err(), "{text}");
        }
    }

    #[test]
    fn load_reads_the_sample_and_treats_a_missing_file_as_defaults() {
        // 範例檔：把三個開關都改成跟預設相反
        let settings = load(Path::new("testdata/config.toml")).expect("sample file");
        assert_eq!(
            settings,
            Settings {
                grid_lines: false,
                item_icons: false,
                center_cursor: false,
            }
        );
        // 檔案不存在 → 預設值
        let missing = load(Path::new("testdata/no-such-config.toml")).expect("missing file");
        assert_eq!(missing, Settings::default());
        // 存在但讀不到（是資料夾）→ 錯誤，不是預設值
        assert!(load(Path::new("testdata")).is_err());
    }

    #[test]
    fn updated_changes_only_what_differs_and_keeps_comments() {
        let text = "# 我的設定\n\n# 格線\ngrid_lines = false   # 太花了\ncenter_cursor = true\n";
        // grid_lines 改回 true（有那一行：只換值，兩個註解都留著）；
        // item_icons 改成 false（沒有那一行：加在最後）；center_cursor 沒變（不動）
        let settings = Settings {
            grid_lines: true,
            item_icons: false,
            center_cursor: true,
        };
        assert_eq!(
            updated(text, settings).expect("valid file"),
            "# 我的設定\n\n# 格線\ngrid_lines = true   # 太花了\ncenter_cursor = true\nitem_icons = false\n"
        );
    }

    #[test]
    fn updated_adds_new_lines_before_the_comments_at_the_end() {
        // 檔案結尾的註解（例：先註解掉的設定）留在最後面，新加的那行放在它們前面
        let text = "grid_lines = true\n# item_icons = true\n";
        let settings = Settings {
            center_cursor: false,
            ..Settings::default()
        };
        assert_eq!(
            updated(text, settings).expect("valid file"),
            "grid_lines = true\ncenter_cursor = false\n# item_icons = true\n"
        );
    }

    #[test]
    fn updated_leaves_the_text_alone_when_nothing_changed() {
        // 檔案裡寫的跟預設一樣的值也不會被拿掉
        let text = "# 註解\ngrid_lines = true\n";
        assert_eq!(updated(text, Settings::default()).expect("valid"), text);
        // 空檔案 + 全部預設 → 還是空的（不會把預設值都寫進去）
        assert_eq!(updated("", Settings::default()).expect("empty"), "");
    }

    #[test]
    fn save_creates_the_folder_and_load_reads_it_back() {
        let dir = test_dir("save");
        let path = dir.join("roh-cal").join("config.toml");
        // 檔案不存在、開關都是預設：沒有要改的 → 什麼都不寫，連資料夾都不建
        save(&path, Settings::default()).expect("nothing to save");
        assert!(!dir.exists());
        let settings = Settings {
            center_cursor: false,
            ..Settings::default()
        };
        save(&path, settings).expect("save");
        assert_eq!(load(&path).expect("load"), settings);
        // 只寫有變的那一個；暫存檔沒有留下來
        assert_eq!(
            std::fs::read_to_string(&path).expect("read"),
            "center_cursor = false\n"
        );
        assert!(!dir.join("roh-cal").join("config.toml.tmp").exists());
        std::fs::remove_dir_all(&dir).expect("clean up");
    }

    #[test]
    fn save_refuses_to_touch_a_broken_file() {
        let dir = test_dir("broken");
        std::fs::create_dir_all(&dir).expect("dir");
        let path = dir.join("config.toml");
        std::fs::write(&path, "grid_line = false\n").expect("write");
        // 名字打錯的檔案：不存，原本的內容留著讓使用者自己修
        assert!(save(&path, Settings::default()).is_err());
        assert_eq!(
            std::fs::read_to_string(&path).expect("read"),
            "grid_line = false\n"
        );
        std::fs::remove_dir_all(&dir).expect("clean up");
    }

    #[test]
    fn save_writes_through_a_symlink() {
        let dir = test_dir("symlink");
        std::fs::create_dir_all(&dir).expect("dir");
        // dotfiles 的寫法：真正的檔案在別處，config.toml 是指過去的 symlink
        let real = dir.join("dotfiles-config.toml");
        let link = dir.join("config.toml");
        std::fs::write(&real, "grid_lines = true\n").expect("write");
        std::os::unix::fs::symlink(&real, &link).expect("symlink");
        let settings = Settings {
            grid_lines: false,
            ..Settings::default()
        };
        save(&link, settings).expect("save");
        // symlink 還是 symlink，內容寫進真正的檔案
        let file_type = std::fs::symlink_metadata(&link).expect("meta").file_type();
        assert!(file_type.is_symlink());
        assert_eq!(
            std::fs::read_to_string(&real).expect("read"),
            "grid_lines = false\n"
        );
        std::fs::remove_dir_all(&dir).expect("clean up");
    }
}
