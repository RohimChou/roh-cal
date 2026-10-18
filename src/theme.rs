//! 顏色：畫面只講「角色」（今天、週末、週數…），不講顏色名；換 theme = 整組換掉
//!
//! 一套 theme 是一個 TOML 檔（內建的在 repo 的 `themes/`，編進執行檔）

use ratatui::style::Color;
use serde::Deserialize;
use std::collections::BTreeMap;

/// 一套 theme：每個角色各配一個顏色；從 theme 檔（TOML）的 `[roles]` 讀（見 `Theme::parse`），欄位名 = 角色名
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Theme {
    /// 一般的字：平日的日期
    pub text: Color,
    /// 次要的字：表頭、最下面的操作說明
    pub muted: Color,
    /// 外框、欄和欄之間、月和月之間的線
    pub line: Color,
    /// 跨年的粗線
    pub year_line: Color,
    /// 標題、左邊的年 / 月
    pub label: Color,
    /// 選取那一週的週數
    pub week_number: Color,
    /// 其他週的週數（比較暗）
    pub week_number_dim: Color,
    /// 六、日的日期和表頭
    pub weekend: Color,
    /// 有還沒結束的事的日期
    pub has_item: Color,
    /// 假日（假日日曆裡的事的日子）的日期
    pub holiday: Color,
    /// 今天的日期
    pub today: Color,
    /// 選取的日期：前後的括號 `[17]`
    pub selected: Color,
    /// 月曆上選取那一週那一列、右邊選到的那件事的底色（像 vim 的 cursorline）
    pub cursor_line: Color,
    /// 鍵盤在的那個框的框線
    pub focus: Color,
    /// side menu 上勾選的日曆的 `☑`
    pub checked: Color,
    /// 最下面一列暫時換成的提示（寫檔失敗、重新載入）
    pub notice: Color,
    /// 寫錯的地方（編輯框裡存不了的原因、任務頁上寫錯格式的那一行）
    pub error: Color,
    /// 大月曆（版型 3）格子之間的淡格線
    pub grid: Color,
    /// 大月曆（版型 3）不是選取那個月的日子：日期和事都變灰
    pub other_month: Color,
    /// 說明頁的段落名（`移動`、`編輯`…）
    pub help_section: Color,
    /// 說明頁的按鍵（`hjkl`、`Space`…）
    pub help_key: Color,
}

/// 內建的 theme：(名字, theme 檔的內容)；檔案在 repo 的 `themes/`，編進執行檔
pub const BUILTIN: [(&str, &str); 1] = [(
    "catppuccin-mocha",
    include_str!("../themes/catppuccin-mocha.toml"),
)];

/// theme 檔（TOML）的樣子：`[palette]` 色票（名字自己取，可以不寫）、`[roles]` 每個角色用哪個顏色
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ThemeFile {
    #[serde(default)]
    palette: BTreeMap<String, String>,
    roles: toml::Table,
}

impl Theme {
    /// [Catppuccin Mocha](https://catppuccin.com/palette)：暗色背景用的柔和配色（跟 v1 的預設一樣）
    pub fn catppuccin_mocha() -> Self {
        Self::builtin("catppuccin-mocha").unwrap_or_else(|| unreachable!("內建的 theme"))
    }

    /// 內建的 theme 裡叫 `name` 的那一套；沒有這個名字是 `None`
    pub fn builtin(name: &str) -> Option<Self> {
        let (_, text) = BUILTIN.iter().find(|(builtin, _)| *builtin == name)?;
        // 內建的 theme 檔編進執行檔，test 確認每一套都讀得進來；讀不進來是寫程式的錯，不是使用者的錯
        Some(Self::parse(text).unwrap_or_else(|error| unreachable!("內建的 theme {name}：{error}")))
    }

    /// 把 theme 檔（TOML）轉成 `Theme`：`[roles]` 裡寫色票名字的先換成 `[palette]` 裡的顏色，
    /// 再一個角色一個角色讀成顏色（`"#rrggbb"`、`"blue"` 這種 terminal 的顏色名字、256 色的編號都可以）。
    /// 少了角色、不認識的角色、不是顏色都是錯誤
    pub fn parse(text: &str) -> Result<Self, toml::de::Error> {
        let ThemeFile { palette, mut roles } = toml::from_str(text)?;
        for (_, value) in roles.iter_mut() {
            if let Some(color) = value.as_str().and_then(|name| palette.get(name)) {
                *value = toml::Value::String(color.clone());
            }
        }
        toml::Value::Table(roles).try_into()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_builtin_theme_reads() {
        for (name, text) in BUILTIN {
            assert!(Theme::parse(text).is_ok(), "{name}");
        }
        assert_eq!(Theme::builtin("no-such-theme"), None);
    }

    #[test]
    fn roles_take_palette_names_or_colors() {
        // 從 mocha 改一行來試：色票名字、#rrggbb、terminal 的顏色名字、256 色的編號
        let mocha = BUILTIN[0].1;
        let with = |role: &str| mocha.replace("today = \"green\"", &format!("today = {role}"));
        assert_eq!(
            Theme::parse(&with("\"red\"")).map(|theme| theme.today),
            Ok(Color::Rgb(243, 139, 168))
        );
        assert_eq!(
            Theme::parse(&with("\"#010203\"")).map(|theme| theme.today),
            Ok(Color::Rgb(1, 2, 3))
        );
        assert_eq!(
            Theme::parse(&with("\"light-blue\"")).map(|theme| theme.today),
            Ok(Color::LightBlue)
        );
        assert_eq!(
            Theme::parse(&with("\"236\"")).map(|theme| theme.today),
            Ok(Color::Indexed(236))
        );
    }

    #[test]
    fn mistakes_in_a_theme_file_are_errors() {
        let mocha = BUILTIN[0].1;
        for text in [
            // 少一個角色
            mocha.replace("today = \"green\"\n", ""),
            // 角色名字打錯
            mocha.replace("today = ", "todya = "),
            // 不是顏色、也不是色票的名字
            mocha.replace("today = \"green\"", "today = \"greeen\""),
            // 不認識的表
            format!("{mocha}\n[colours]\n"),
        ] {
            assert!(Theme::parse(&text).is_err(), "{text}");
        }
    }
}
