//! 設定檔：啟動時讀進來（位置見 `paths::config_file`）
//!
//! 只寫想改的就好，沒寫的用預設值；名字打錯、型別不對都算錯誤（啟動時印出哪一行錯，不進 TUI）。
//! 範例見 `testdata/config.toml`

use color_eyre::{Result, eyre::WrapErr};
use serde::Deserialize;
use std::io::ErrorKind;
use std::path::Path;

/// 執行時可以改的設定 = 設定檔的內容，結構一模一樣：`calendar` = `[calendar]`、`month` = `[month]`
///
/// 設定檔裡的名字就是欄位名（例：`[month]` 底下寫 `grid_lines = false`）。`serde` 的 `default`：沒寫的欄位用
/// `Default` 的值；`deny_unknown_fields`：不認識的名字算錯誤，打錯字才不會被默默忽略
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Settings {
    /// `[calendar]`：版型 1、3 的月曆共用的開關
    pub calendar: CalendarSettings,
    /// `[month]`：大月曆（版型 3）的開關
    pub month: MonthSettings,
}

/// 月曆共用的開關（設定檔的 `[calendar]`）；預設全部關（`bool` 的預設值是 `false`）
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct CalendarSettings {
    /// 週數欄：選取那週寫週數，其他週寫跟選取那週差幾週（像 vim 的 relativenumber）
    pub relative_weeks: bool,
}

/// 大月曆的開關（設定檔的 `[month]`）；預設值在 `Default`
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct MonthSettings {
    /// 格子之間畫淡淡的格線（關掉可以多 1 列 1 格放內容）
    pub grid_lines: bool,
    /// 格子裡每件事前面加一個點（`∙ 14:00 週會`）
    pub item_icons: bool,
    /// 選取的那週固定在 6 週的正中間（像 vim 的 `scrolloff=999`）；關掉就一週一週捲，移出 6 週才捲
    pub center_cursor: bool,
}

impl Default for MonthSettings {
    fn default() -> Self {
        Self {
            grid_lines: true,
            item_icons: true,
            center_cursor: false,
        }
    }
}

/// 讀設定檔；檔案不存在就全部用預設值，讀不到或內容寫錯就回傳錯誤
pub fn load(path: &Path) -> Result<Settings> {
    let text = read(path)?;
    parse(&text).wrap_err_with(|| format!("invalid config file: {}", path.display()))
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_fields_use_the_defaults() {
        assert_eq!(parse("").expect("empty file"), Settings::default());
        assert_eq!(
            parse("[month]\n").expect("empty table"),
            Settings::default()
        );
        let settings = parse("[month]\ngrid_lines = false").expect("one field");
        assert_eq!(
            settings.month,
            MonthSettings {
                grid_lines: false,
                ..MonthSettings::default()
            }
        );
        assert_eq!(settings.calendar, CalendarSettings::default());
    }

    #[test]
    fn mistakes_are_errors() {
        for text in [
            // 名字打錯（少一個 s）
            "[month]\ngrid_line = false",
            // 型別不對：要 true / false，不是字串
            "[month]\ngrid_lines = \"no\"",
            // TOML 語法錯：等號後面沒有值
            "[month]\ngrid_lines =",
            // 開關要放在 [month] 底下，不能寫在最上層
            "grid_lines = false",
            // 表的名字打錯
            "[mnth]\ngrid_lines = false",
            // v1 的名字（v1 的 week_numbers = true 是相對週數，v2 改叫 relative_weeks）
            "[calendar]\nweek_numbers = true",
        ] {
            assert!(parse(text).is_err(), "{text}");
        }
    }

    #[test]
    fn load_reads_the_sample_and_treats_a_missing_file_as_defaults() {
        // 範例檔：每個開關都寫成跟預設相反
        let settings = load(Path::new("testdata/config.toml")).expect("sample file");
        assert_eq!(
            settings,
            Settings {
                calendar: CalendarSettings {
                    relative_weeks: true
                },
                month: MonthSettings {
                    grid_lines: false,
                    item_icons: false,
                    center_cursor: true,
                },
            }
        );
        // 檔案不存在 → 預設值
        let missing = load(Path::new("testdata/no-such-config.toml")).expect("missing file");
        assert_eq!(missing, Settings::default());
        // 存在但讀不到（是資料夾）→ 錯誤，不是預設值
        assert!(load(Path::new("testdata")).is_err());
    }

    #[test]
    fn the_error_names_the_file_and_the_line() {
        let dir = crate::files::test_dir("config-error");
        std::fs::create_dir_all(&dir).expect("dir");
        let path = dir.join("config.toml");
        std::fs::write(&path, "[month]\n\ngrid_lines = 1\n").expect("write");
        let error = format!("{:?}", load(&path).expect_err("wrong type"));
        assert!(error.contains("invalid config file"), "{error}");
        assert!(error.contains("config.toml"), "{error}");
        // toml 的錯誤訊息會指出第 3 行
        assert!(error.contains("line 3"), "{error}");
        std::fs::remove_dir_all(&dir).expect("clean up");
    }
}
