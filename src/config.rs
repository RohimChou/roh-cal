//! 設定檔 `config.toml`：啟動時讀進來，設定頁的開關一開始就是檔案裡寫的值
//!
//! 位置見 `paths::config_file`。名字跟 `Settings` 的欄位一樣，只寫想改的就好，沒寫的用預設值；
//! 範例見 `testdata/config.toml`

use crate::settings::Settings;
use color_eyre::{Result, eyre::WrapErr};
use std::io::ErrorKind;
use std::path::Path;

/// 讀設定檔；檔案不存在就全部用預設值，讀不到或內容寫錯就回傳錯誤
pub fn load(path: &Path) -> Result<Settings> {
    let text = match std::fs::read_to_string(path) {
        Ok(text) => text,
        Err(error) if error.kind() == ErrorKind::NotFound => return Ok(Settings::default()),
        Err(error) => {
            return Err(error)
                .wrap_err_with(|| format!("cannot read config file: {}", path.display()));
        }
    };
    parse(&text).wrap_err_with(|| format!("invalid config file: {}", path.display()))
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
}
