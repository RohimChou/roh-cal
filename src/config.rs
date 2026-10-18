//! 設定檔：啟動時讀進來（位置見 `paths::config_file`），設定頁（`,`）關掉時把改過的存回去
//!
//! 只寫想改的就好，沒寫的用預設值；名字打錯、型別不對都算錯誤（啟動時印出哪一行錯，不進 TUI；`[colors]` 寫錯是說哪個角色哪個值）。
//! 存檔只改有變的那幾行，其他行和註解一字不動（手寫的設定檔不會被蓋掉）。範例見 `testdata/config.toml`

use crate::files;
use crate::theme::{self, Theme};
use color_eyre::{
    Result,
    eyre::{WrapErr, bail},
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::io::ErrorKind;
use std::path::Path;
use toml_edit::{DocumentMut, Item, Table, TableLike};

/// 執行時可以改的設定 = 設定檔的內容，結構一模一樣：`theme` = 最上面的 `theme = "..."`、`calendar` = `[calendar]`、
/// `month` = `[month]`、`colors` = `[colors]`
///
/// 設定檔裡的名字就是欄位名（例：`[month]` 底下寫 `grid_lines = false`）。`serde` 的 `default`：沒寫的欄位用
/// `Default` 的值；`deny_unknown_fields`：不認識的名字算錯誤，打錯字才不會被默默忽略。`Serialize`：存檔時反過來轉成 TOML。
/// 有 `String`（會配置記憶體），所以不是 `Copy`，要複製一份得明確寫 `.clone()`
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct Settings {
    /// 用哪一套內建的 theme（`theme::BUILTIN` 的名字），例：`catppuccin-latte`
    pub theme: String,
    /// `[calendar]`：版型 1、3 的月曆共用的開關
    pub calendar: CalendarSettings,
    /// `[month]`：大月曆（版型 3）的開關
    pub month: MonthSettings,
    /// `[colors]`：蓋掉 theme 的幾個角色（角色 → 顏色，例：`today = "#a6e3a1"`）；沒寫的照 theme。
    /// 設定頁只寫跟 theme 不一樣的（還原就拿掉那一行）
    pub colors: BTreeMap<String, String>,
}

impl Settings {
    /// 換成叫 `name` 的 theme。`[colors]` 蓋掉的顏色先照現在的 theme 寫成顏色本身（例：色票名字 `surface0` →
    /// `#313244`、terminal 的 `cyan` → `Cyan`）：新的 theme 不一定有那個色票名字、或同名的是別的顏色，換過去還是同一個顏色
    pub fn set_theme(&mut self, name: &str) {
        if let Ok(current) = Theme::build(&self.theme, &self.colors) {
            for (role, color) in &mut self.colors {
                if let Some(resolved) = current.color(role) {
                    *color = theme::color_text(resolved);
                }
            }
        }
        self.theme = name.to_string();
    }
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            theme: theme::DEFAULT.to_string(),
            calendar: CalendarSettings::default(),
            month: MonthSettings::default(),
            colors: BTreeMap::new(),
        }
    }
}

/// 月曆共用的開關（設定檔的 `[calendar]`）；預設全部關（`bool` 的預設值是 `false`）
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct CalendarSettings {
    /// 週數欄：選取那週寫週數，其他週寫跟選取那週差幾週（像 vim 的 relativenumber）
    pub relative_weeks: bool,
}

/// 大月曆的開關（設定檔的 `[month]`）；預設值在 `Default`
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
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
    let text =
        read(path).wrap_err_with(|| format!("cannot read config file: {}", path.display()))?;
    parse(&text).wrap_err_with(|| format!("invalid config file: {}", path.display()))
}

/// 把設定頁改過的存回設定檔：`before`（打開設定頁那時候）到 `after`（現在）改了的值才寫，其他照檔案原本的
/// （`--theme` 這種只用這一次的、在別處改過的都不會被蓋掉）；沒有要改的就不寫；資料夾不存在會自動建立。
/// 寫出來的檔案讀不進來（例：檔案在別處改壞了、theme 和 `[colors]` 對不上）就不存、回傳 `InvalidData`，
/// 原本的內容留著讓使用者自己修，下次才打得開
pub fn save(path: &Path, before: &Settings, after: &Settings) -> std::io::Result<()> {
    let invalid =
        |error: color_eyre::Report| std::io::Error::new(ErrorKind::InvalidData, error.to_string());
    let text = read(path)?;
    let new_text = updated(&text, before, after).map_err(invalid)?;
    // 只是打開設定頁看看、沒改東西，檔案就不碰
    if new_text == text {
        return Ok(());
    }
    parse(&new_text).map_err(invalid)?;
    files::write_atomically(path, &new_text)
}

/// 讀設定檔的文字；檔案不存在就當作空的（空的 = 全部用預設值）
fn read(path: &Path) -> std::io::Result<String> {
    match std::fs::read_to_string(path) {
        Err(error) if error.kind() == ErrorKind::NotFound => Ok(String::new()),
        result => result,
    }
}

/// 把設定檔的內容轉成 `Settings`：沒寫的欄位用預設值；不認識的名字、值的型別不對都是錯誤（錯誤訊息會指出第幾行哪裡錯）；
/// 沒有這套 theme、`[colors]` 的角色打錯或不是顏色也是錯誤
fn parse(text: &str) -> Result<Settings> {
    let settings: Settings = toml::from_str(text)?;
    if let Err(error) = Theme::build(&settings.theme, &settings.colors) {
        bail!(error);
    }
    Ok(settings)
}

/// 設定檔的新內容：`before` 到 `after` 改了的值寫進 `text`：有那一行就換值，沒有就加在同一個表（例：`[month]`）
/// 的後面（檔案結尾的註解還是留在最後面）；還原成 theme 的顏色整行拿掉；其他行、註解、空白一字不動
fn updated(text: &str, before: &Settings, after: &Settings) -> Result<String> {
    // 保留格式的 TOML（toml_edit）：改一個值，其他內容原封不動
    let mut doc: DocumentMut = text.parse()?;
    // 改之前、改之後都轉成 TOML，一個名字一個名字比
    let before = toml_edit::ser::to_document(before)?;
    let after = toml_edit::ser::to_document(after)?;
    merge(doc.as_table_mut(), before.as_table(), after.as_table());
    Ok(doc.to_string())
}

/// 把 `wanted`（改之後）裡跟 `current`（改之前）不一樣的值寫進 `doc`；遇到表（例：`[month]`）就進到裡面一層一樣做；
/// `current` 有、`wanted` 沒有的（例：還原成 theme 的顏色）從 `doc` 拿掉
///
/// `dyn TableLike`：`[month]` 這種表和 `{ a = 1 }` 這種寫在一行的表都可以傳進來
fn merge(doc: &mut dyn TableLike, current: &dyn TableLike, wanted: &dyn TableLike) {
    for (key, item) in wanted.iter() {
        // 兩邊是同一個 serializer 轉出來的，印出來一樣就是值一樣（整個表沒變也直接跳過，不會多出空的 `[month]`）
        if current.get(key).map(ToString::to_string) == Some(item.to_string()) {
            continue;
        }
        if let Some(wanted_table) = item.as_table_like() {
            // 表：檔案裡還沒有就先建一個空的 `[key]`，再進到裡面比
            if doc.get(key).and_then(Item::as_table_like).is_none() {
                doc.insert(key, Item::Table(Table::new()));
            }
            // current 和 wanted 是同一個 struct 轉出來的，wanted 這裡是表，current 這裡也一定是表
            if let (Some(doc_table), Some(current_table)) = (
                doc.get_mut(key).and_then(Item::as_table_like_mut),
                current.get(key).and_then(Item::as_table_like),
            ) {
                merge(doc_table, current_table, wanted_table);
            }
            // 整個表都沒東西了（例：顏色全部還原）→ 連 `[colors]` 這行也拿掉
            if wanted_table.is_empty() {
                doc.remove(key);
            }
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
            // 沒有這一行：加在同一個表的後面、檔案結尾的註解前面
            _ => {
                doc.insert(key, item.clone());
            }
        }
    }
    // 檔案有、要存的沒有（例：`r` 還原成 theme 的顏色）→ 整行拿掉（那行上面的註解也一起）
    let stale: Vec<String> = current
        .iter()
        .map(|(key, _)| key.to_string())
        .filter(|key| wanted.get(key).is_none())
        .collect();
    for key in stale {
        doc.remove(&key);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 跟檔案現在的值比（改之前 = 檔案裡的值），算出新的內容（test 用）
    fn updated_file(text: &str, settings: &Settings) -> Result<String> {
        updated(text, &parse(text)?, settings)
    }

    #[test]
    fn save_writes_only_what_changed_on_the_page() {
        let dir = files::test_dir("config-only-changes");
        std::fs::create_dir_all(&dir).expect("dir");
        let path = dir.join("config-v2.toml");
        // 打開 app 之後，檔案在別處改了格線；這次用 `--theme tokyo-night` 打開（檔案裡是 gruvbox-dark）
        std::fs::write(
            &path,
            "theme = \"gruvbox-dark\"\n[month]\ngrid_lines = false\n",
        )
        .expect("write");
        let before = Settings {
            theme: "tokyo-night".to_string(),
            ..Settings::default()
        };
        // 設定頁只打開了相對週數：只寫那一行，theme、格線照檔案的
        let mut after = before.clone();
        after.calendar.relative_weeks = true;
        save(&path, &before, &after).expect("save");
        assert_eq!(
            std::fs::read_to_string(&path).expect("read"),
            "theme = \"gruvbox-dark\"\n[month]\ngrid_lines = false\n\n[calendar]\nrelative_weeks = true\n"
        );
        // 沒改東西：不寫
        save(&path, &after, &after).expect("nothing changed");
        std::fs::remove_dir_all(&dir).expect("clean up");
    }

    #[test]
    fn save_refuses_to_write_a_file_that_would_not_load() {
        let dir = files::test_dir("config-would-break");
        std::fs::create_dir_all(&dir).expect("dir");
        let path = dir.join("config-v2.toml");
        // 檔案在別處加了 gruvbox 色票的名字；設定頁把 theme 換成 mocha：寫出來 mocha 沒有 light1，下次打不開 → 不寫
        let text = "theme = \"gruvbox-dark\"\n[colors]\ntoday = \"light1\"\n";
        std::fs::write(&path, text).expect("write");
        let before = Settings {
            theme: "gruvbox-dark".to_string(),
            ..Settings::default()
        };
        let after = Settings::default();
        let error = save(&path, &before, &after).expect_err("would not load");
        assert_eq!(error.kind(), ErrorKind::InvalidData);
        assert_eq!(std::fs::read_to_string(&path).expect("read"), text);
        std::fs::remove_dir_all(&dir).expect("clean up");
    }

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
            // 沒有這套 theme
            "theme = \"dracula\"",
            // 角色名字打錯、不是顏色、terminal 沒有 catppuccin 的色票名字
            "[colors]\ntodya = \"red\"",
            "[colors]\ntoday = \"redd\"",
            "theme = \"terminal\"\n[colors]\ntoday = \"surface0\"",
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
                theme: "catppuccin-latte".to_string(),
                calendar: CalendarSettings {
                    relative_weeks: true
                },
                month: MonthSettings {
                    grid_lines: false,
                    item_icons: false,
                    center_cursor: true,
                },
                colors: BTreeMap::from([("weekend".to_string(), "red".to_string())]),
            }
        );
        // 檔案不存在 → 預設值
        let missing = load(Path::new("testdata/no-such-config.toml")).expect("missing file");
        assert_eq!(missing, Settings::default());
        // 存在但讀不到（是資料夾）→ 錯誤，不是預設值
        assert!(load(Path::new("testdata")).is_err());
    }

    /// 預設值只改 `[month]` 的一部分（test 用），例：`with_month(|month| month.grid_lines = false)`
    fn with_month(change: impl FnOnce(&mut MonthSettings)) -> Settings {
        let mut settings = Settings::default();
        change(&mut settings.month);
        settings
    }

    #[test]
    fn updated_changes_only_what_differs_and_keeps_comments() {
        let text =
            "# 我的設定\n\n[month]\n# 格線\ngrid_lines = true   # 太花了\ncenter_cursor = true\n";
        // grid_lines 改成 false（有那一行：只換值，兩個註解都留著）；
        // item_icons 改成 false（沒有那一行：加在 [month] 的最後）；center_cursor 沒變（不動）
        let settings = with_month(|month| {
            month.grid_lines = false;
            month.item_icons = false;
            month.center_cursor = true;
        });
        assert_eq!(
            updated_file(text, &settings).expect("valid file"),
            "# 我的設定\n\n[month]\n# 格線\ngrid_lines = false   # 太花了\ncenter_cursor = true\nitem_icons = false\n"
        );
    }

    #[test]
    fn updated_creates_a_table_the_file_does_not_have() {
        let mut settings = Settings::default();
        settings.calendar.relative_weeks = true;
        assert_eq!(
            updated_file("[month]\ngrid_lines = true\n", &settings).expect("valid file"),
            "[month]\ngrid_lines = true\n\n[calendar]\nrelative_weeks = true\n"
        );
        assert_eq!(
            updated_file("", &settings).expect("empty file"),
            "[calendar]\nrelative_weeks = true\n"
        );
    }

    #[test]
    fn updated_adds_new_lines_before_the_comments_at_the_end() {
        // 檔案結尾的註解（例：先註解掉的設定）留在最後面，新加的那行放在它們前面
        let text = "[month]\ngrid_lines = true\n# item_icons = true\n";
        let settings = with_month(|month| month.center_cursor = true);
        assert_eq!(
            updated_file(text, &settings).expect("valid file"),
            "[month]\ngrid_lines = true\ncenter_cursor = true\n# item_icons = true\n"
        );
    }

    #[test]
    fn updated_puts_the_theme_above_the_tables() {
        // theme 寫在最上層：TOML 規定要在所有 `[表]` 前面，不然會被當成最後那個表的
        let settings = Settings {
            theme: "catppuccin-latte".to_string(),
            ..Settings::default()
        };
        let text = "[month]\ngrid_lines = true\n";
        let new_text = updated_file(text, &settings).expect("valid file");
        assert_eq!(
            new_text,
            "theme = \"catppuccin-latte\"\n[month]\ngrid_lines = true\n"
        );
        assert_eq!(parse(&new_text).expect("reads back"), settings);
    }

    #[test]
    fn updated_writes_changed_colors_and_removes_the_reset_ones() {
        let text = "[month]\ngrid_lines = false\n\n[colors]\n# 太淡了\nmuted = \"#010203\"\nweekend = \"#f38ba8\"   # 紅\n";
        // 讀進來 → muted 還原（拿掉）、新增 today、weekend 不變
        let mut settings = parse(text).expect("valid");
        settings.colors.remove("muted");
        settings
            .colors
            .insert("today".to_string(), "#ffffff".to_string());
        assert_eq!(
            updated_file(text, &settings).expect("valid"),
            "[month]\ngrid_lines = false\n\n[colors]\nweekend = \"#f38ba8\"   # 紅\ntoday = \"#ffffff\"\n"
        );
    }

    #[test]
    fn updated_removes_the_colors_table_when_every_color_is_reset() {
        let text = "[month]\ngrid_lines = false\n\n[colors]\nweekend = \"#f38ba8\"\n";
        let mut settings = parse(text).expect("valid");
        settings.colors.clear();
        assert_eq!(
            updated_file(text, &settings).expect("valid"),
            "[month]\ngrid_lines = false\n"
        );
        // 檔案沒有 [colors]、也沒有改過的顏色 → 不會多出空的 [colors]
        assert_eq!(
            updated_file("[month]\n", &Settings::default()).expect("valid"),
            "[month]\n"
        );
    }

    #[test]
    fn updated_leaves_the_text_alone_when_nothing_changed() {
        // 檔案裡寫的跟預設一樣的值也不會被拿掉（grid_lines 預設 true）
        let text = "# 註解\n[month]\ngrid_lines = true\n";
        assert_eq!(
            updated_file(text, &Settings::default()).expect("valid"),
            text
        );
        // 空檔案 + 全部預設 → 還是空的（不會把預設值都寫進去，也不會多出空的表）
        assert_eq!(updated_file("", &Settings::default()).expect("empty"), "");
    }

    #[test]
    fn save_creates_the_folder_and_load_reads_it_back() {
        let dir = files::test_dir("config-save");
        let path = dir.join("roh-cal").join("config-v2.toml");
        // 檔案不存在、開關都是預設：沒有要改的 → 什麼都不寫，連資料夾都不建
        save(&path, &Settings::default(), &Settings::default()).expect("nothing to save");
        assert!(!dir.exists());
        let settings = with_month(|month| month.center_cursor = true);
        save(&path, &Settings::default(), &settings).expect("save");
        assert_eq!(load(&path).expect("load"), settings);
        // 只寫有變的那一個；暫存檔沒有留下來
        assert_eq!(
            std::fs::read_to_string(&path).expect("read"),
            "[month]\ncenter_cursor = true\n"
        );
        assert!(!dir.join("roh-cal").join("config-v2.toml.tmp").exists());
        std::fs::remove_dir_all(&dir).expect("clean up");
    }

    #[test]
    fn save_refuses_to_touch_a_broken_file() {
        let dir = files::test_dir("config-broken");
        std::fs::create_dir_all(&dir).expect("dir");
        let path = dir.join("config-v2.toml");
        std::fs::write(&path, "[month]\ngrid_line = false\n").expect("write");
        // 名字打錯的檔案（在別處改壞的）：設定頁改了東西也不存，原本的內容留著讓使用者自己修
        let mut changed = Settings::default();
        changed.month.item_icons = false;
        let error = save(&path, &Settings::default(), &changed).expect_err("broken file");
        assert_eq!(error.kind(), ErrorKind::InvalidData);
        // 沒改東西：不碰檔案，也不算錯
        save(&path, &changed, &changed).expect("nothing changed");
        assert_eq!(
            std::fs::read_to_string(&path).expect("read"),
            "[month]\ngrid_line = false\n"
        );
        std::fs::remove_dir_all(&dir).expect("clean up");
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
