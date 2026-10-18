//! 顏色：畫面只講「角色」（今天、週末、週數…），不講顏色名；換 theme = 整組換掉
//!
//! 一套 theme 是一個 TOML 檔（內建的在 repo 的 `themes/`，編進執行檔）

use ratatui::style::Color;
use serde::Deserialize;
#[cfg(test)]
use serde::Serialize;
use std::collections::BTreeMap;
use std::sync::LazyLock;
use toml_edit::DocumentMut;

/// 一套 theme：每個角色各配一個顏色；從 theme 檔（TOML）的 `[roles]` 讀（見 `Theme::parse`），欄位名 = 角色名。
/// `Serialize`：test 用來確認角色的名字（`ROLES`、`Theme::color`）跟欄位一模一樣
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[cfg_attr(test, derive(Serialize))]
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
pub const BUILTIN: [(&str, &str); 5] = [
    (
        "catppuccin-mocha",
        include_str!("../themes/catppuccin-mocha.toml"),
    ),
    (
        "catppuccin-latte",
        include_str!("../themes/catppuccin-latte.toml"),
    ),
    ("gruvbox-dark", include_str!("../themes/gruvbox-dark.toml")),
    ("tokyo-night", include_str!("../themes/tokyo-night.toml")),
    ("terminal", include_str!("../themes/terminal.toml")),
];

/// 沒指定時用的 theme
pub const DEFAULT: &str = "catppuccin-mocha";

/// 內建的 theme 的名字，照 `BUILTIN` 的順序
pub fn names() -> Vec<&'static str> {
    BUILTIN.iter().map(|&(name, _)| name).collect()
}

/// 每個角色的名字，照 `Theme` 欄位的順序（= theme 檔的 `[roles]`、設定檔的 `[colors]` 裡的名字；有 test 釘住）
pub const ROLES: [&str; 21] = [
    "text",
    "muted",
    "line",
    "year_line",
    "label",
    "week_number",
    "week_number_dim",
    "weekend",
    "has_item",
    "holiday",
    "today",
    "selected",
    "cursor_line",
    "focus",
    "checked",
    "notice",
    "error",
    "grid",
    "other_month",
    "help_section",
    "help_key",
];

/// terminal 的 16 色：theme 沒有色票時（例：`terminal`），設定頁換顏色就在這幾個裡面換
const ANSI: [Color; 16] = [
    Color::Black,
    Color::Red,
    Color::Green,
    Color::Yellow,
    Color::Blue,
    Color::Magenta,
    Color::Cyan,
    Color::Gray,
    Color::DarkGray,
    Color::LightRed,
    Color::LightGreen,
    Color::LightYellow,
    Color::LightBlue,
    Color::LightMagenta,
    Color::LightCyan,
    Color::White,
];

/// 每一套內建 theme 能挑的顏色（照 `BUILTIN` 的順序）：第一次用的時候讀一次 theme 檔，之後都用這一份
static CHOICES: LazyLock<Vec<Vec<(String, Color)>>> = LazyLock::new(|| {
    BUILTIN
        .iter()
        .map(|&(_, text)| read_choices(text))
        .collect()
});

/// 內建的 theme `name` 能挑的顏色：它的色票 (名字, 顏色)，照 theme 檔裡的順序；
/// 沒有色票（例：`terminal`）就是 terminal 的 16 色（名字 = 設定檔裡的寫法）；沒有這套 theme 是空的
pub fn choices(name: &str) -> &'static [(String, Color)] {
    BUILTIN
        .iter()
        .position(|(builtin, _)| *builtin == name)
        .map_or(&[], |index| CHOICES[index].as_slice())
}

/// 從 theme 檔（`text`）讀出能挑的顏色（見 `choices`）
fn read_choices(text: &str) -> Vec<(String, Color)> {
    let palette: Vec<(String, Color)> = text
        .parse::<DocumentMut>()
        .ok()
        .and_then(|doc| {
            let table = doc.get("palette")?.as_table()?;
            Some(
                table
                    .iter()
                    .filter_map(|(key, value)| {
                        Some((key.to_string(), value.as_str()?.parse().ok()?))
                    })
                    .collect(),
            )
        })
        .unwrap_or_default();
    if palette.is_empty() {
        ANSI.iter()
            .map(|&color| (color_text(color), color))
            .collect()
    } else {
        palette
    }
}

/// 顏色寫成設定檔裡的樣子：`#cdd6f4`、`236`（256 色的編號）；terminal 的顏色照 ratatui 的寫法 `LightBlue`
/// （大寫開頭：色票的名字都是小寫，theme 的色票就算也有 `cyan`，讀回來還是 terminal 的 cyan）
pub fn color_text(color: Color) -> String {
    match color {
        Color::Rgb(red, green, blue) => format!("#{red:02x}{green:02x}{blue:02x}"),
        named => named.to_string(),
    }
}

/// theme 檔（TOML）的樣子：`[palette]` 色票（名字自己取，可以不寫）、`[roles]` 每個角色用哪個顏色
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ThemeFile {
    /// 色票：名字 → 顏色（`"#rrggbb"`）；`[roles]` 可以寫這裡的名字
    #[serde(default)]
    palette: BTreeMap<String, String>,
    /// 每個角色用哪個顏色（色票的名字或顏色）；`Theme::parse` 再一個一個讀成顏色
    roles: toml::Table,
}

impl Theme {
    /// [Catppuccin Mocha](https://catppuccin.com/palette)：暗色背景用的柔和配色（跟 v1 的預設一樣）
    pub fn catppuccin_mocha() -> Self {
        Self::builtin("catppuccin-mocha").unwrap_or_else(|| unreachable!("內建的 theme"))
    }

    /// 內建的 theme 裡叫 `name` 的那一套（沒有蓋掉的顏色）；沒有這個名字是 `None`
    pub fn builtin(name: &str) -> Option<Self> {
        let (_, text) = BUILTIN.iter().find(|(builtin, _)| *builtin == name)?;
        // 內建的 theme 檔編進執行檔，test 確認每一套都讀得進來；讀不進來是寫程式的錯，不是使用者的錯
        Some(
            Self::parse(text, &BTreeMap::new())
                .unwrap_or_else(|error| unreachable!("內建的 theme {name}：{error}")),
        )
    }

    /// 內建的 theme `name`，再用 `colors`（設定檔的 `[colors]`：角色 → 顏色，或那套 theme 的色票名字）蓋掉幾個角色；
    /// 沒有這套 theme、不認識的角色、不是顏色都是錯誤（錯誤訊息說明哪裡錯）
    pub fn build(name: &str, colors: &BTreeMap<String, String>) -> Result<Self, String> {
        let (_, text) = BUILTIN
            .iter()
            .find(|(builtin, _)| *builtin == name)
            .ok_or_else(|| {
                format!(
                    "no theme named \"{name}\" (built-in: {})",
                    names().join(", ")
                )
            })?;
        // 先一個一個檢查蓋掉的顏色，錯誤訊息才說得出是哪一個
        let file: ThemeFile = toml::from_str(text).map_err(|error| error.to_string())?;
        for (role, color) in colors {
            if !ROLES.contains(&role.as_str()) {
                return Err(format!(
                    "[colors] {role}: no such role (the roles are the names under [roles] in themes/)"
                ));
            }
            if !file.palette.contains_key(color) && color.parse::<Color>().is_err() {
                return Err(format!(
                    "[colors] {role} = \"{color}\": not a color, nor a palette name of {name}"
                ));
            }
        }
        Self::parse(text, colors).map_err(|error| format!("[colors]: {}", error.message()))
    }

    /// 角色 `role`（`ROLES` 的名字）現在的顏色；沒有這個角色是 `None`。名字跟欄位一一對應，有 test 釘住
    pub fn color(&self, role: &str) -> Option<Color> {
        let color = match role {
            "text" => self.text,
            "muted" => self.muted,
            "line" => self.line,
            "year_line" => self.year_line,
            "label" => self.label,
            "week_number" => self.week_number,
            "week_number_dim" => self.week_number_dim,
            "weekend" => self.weekend,
            "has_item" => self.has_item,
            "holiday" => self.holiday,
            "today" => self.today,
            "selected" => self.selected,
            "cursor_line" => self.cursor_line,
            "focus" => self.focus,
            "checked" => self.checked,
            "notice" => self.notice,
            "error" => self.error,
            "grid" => self.grid,
            "other_month" => self.other_month,
            "help_section" => self.help_section,
            "help_key" => self.help_key,
            _ => return None,
        };
        Some(color)
    }

    /// 把 theme 檔（TOML）轉成 `Theme`：先用 `colors` 蓋掉幾個角色，`[roles]` 裡寫色票名字的再換成 `[palette]` 裡的顏色，
    /// 最後一個角色一個角色讀成顏色（`"#rrggbb"`、`"blue"` 這種 terminal 的顏色名字、256 色的編號都可以）。
    /// 少了角色、不認識的角色、不是顏色都是錯誤
    fn parse(text: &str, colors: &BTreeMap<String, String>) -> Result<Self, toml::de::Error> {
        let ThemeFile { palette, mut roles } = toml::from_str(text)?;
        for (role, color) in colors {
            roles.insert(role.clone(), toml::Value::String(color.clone()));
        }
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

    impl Theme {
        /// 沒有蓋掉的顏色，只讀 theme 檔（test 用）
        fn parse_text(text: &str) -> Result<Self, toml::de::Error> {
            Self::parse(text, &BTreeMap::new())
        }
    }

    #[test]
    fn roles_list_every_field_in_order() {
        // `ROLES` 跟 `Theme` 的欄位一模一樣（Theme 轉成 TOML 的名字和順序）
        let doc = toml_edit::ser::to_document(&Theme::catppuccin_mocha()).expect("serialize");
        let fields: Vec<&str> = doc.iter().map(|(key, _)| key).collect();
        assert_eq!(fields, ROLES);
        // `Theme::color` 每個角色都查得到、就是那個欄位的顏色：每個角色蓋成不一樣的顏色（256 色的第 0～20 號）再查
        let colors: BTreeMap<String, String> = (0..)
            .zip(ROLES)
            .map(|(index, role): (u8, _)| (role.to_string(), index.to_string()))
            .collect();
        let theme = Theme::build("terminal", &colors).expect("terminal");
        let doc = toml_edit::ser::to_document(&theme).expect("serialize");
        for (index, (role, value)) in (0..).zip(doc.iter()) {
            assert_eq!(value.as_str(), Some(index.to_string().as_str()), "{role}");
            assert_eq!(theme.color(role), Some(Color::Indexed(index)), "{role}");
        }
        assert_eq!(
            Theme::catppuccin_mocha().color("today"),
            Some(Color::Rgb(0xa6, 0xe3, 0xa1))
        );
        assert_eq!(Theme::catppuccin_mocha().color("todya"), None);
    }

    #[test]
    fn colors_override_roles_and_must_be_real_roles_and_colors() {
        let colors = |pairs: &[(&str, &str)]| -> BTreeMap<String, String> {
            pairs
                .iter()
                .map(|&(role, color)| (role.to_string(), color.to_string()))
                .collect()
        };
        // 顏色、色票的名字都可以；沒寫到的角色照 theme 的
        let theme = Theme::build(
            "catppuccin-mocha",
            &colors(&[("today", "#010203"), ("weekend", "red")]),
        )
        .expect("valid");
        assert_eq!(theme.today, Color::Rgb(1, 2, 3));
        assert_eq!(theme.weekend, Color::Rgb(0xf3, 0x8b, 0xa8));
        assert_eq!(theme.text, Theme::catppuccin_mocha().text);
        // terminal 沒有色票：red 是 terminal 的紅色
        let terminal = Theme::build("terminal", &colors(&[("weekend", "red")])).expect("valid");
        assert_eq!(terminal.weekend, Color::Red);
        // 錯誤：沒有這套 theme、角色名字打錯、不是顏色；訊息說得出是哪一個
        assert!(Theme::build("dracula", &BTreeMap::new()).is_err());
        let error =
            Theme::build("catppuccin-mocha", &colors(&[("todya", "red")])).expect_err("role");
        assert!(error.contains("todya: no such role"), "{error}");
        let error = Theme::build("terminal", &colors(&[("today", "surface0")])).expect_err("color");
        assert!(
            error.contains("today = \"surface0\": not a color, nor a palette name of terminal"),
            "{error}"
        );
    }

    #[test]
    fn choices_are_the_palette_in_file_order_or_the_16_colors() {
        assert!(choices("no-such-theme").is_empty());
        let mocha = choices("catppuccin-mocha");
        assert_eq!(mocha.len(), 13);
        assert_eq!(mocha[0], ("text".to_string(), Color::Rgb(0xcd, 0xd6, 0xf4)));
        assert_eq!(mocha[12].0, "mauve");
        let terminal = choices("terminal");
        assert_eq!(terminal.len(), 16);
        assert_eq!(terminal[12], ("LightBlue".to_string(), Color::LightBlue));
    }

    #[test]
    fn color_text_reads_back_as_the_same_color() {
        for color in [
            Color::Rgb(0xcd, 0xd6, 0xf4),
            Color::LightBlue,
            Color::DarkGray,
            Color::Indexed(236),
        ] {
            assert_eq!(color_text(color).parse::<Color>(), Ok(color), "{color:?}");
        }
        assert_eq!(color_text(Color::Rgb(0xcd, 0xd6, 0xf4)), "#cdd6f4");
        assert_eq!(color_text(Color::DarkGray), "DarkGray");
    }

    #[test]
    fn every_builtin_theme_reads() {
        for (name, text) in BUILTIN {
            assert!(Theme::parse(text, &BTreeMap::new()).is_ok(), "{name}");
        }
        assert_eq!(Theme::builtin("no-such-theme"), None);
        // 每一套都不一樣（複製貼上忘了改會抓到）
        let themes: Vec<Theme> = names()
            .iter()
            .filter_map(|name| Theme::builtin(name))
            .collect();
        assert_eq!(themes.len(), BUILTIN.len());
        for (index, theme) in themes.iter().enumerate() {
            for other in &themes[index + 1..] {
                assert_ne!(theme, other);
            }
        }
        // 每一套從原本的專案抄的色票，抽一個角色對照
        assert_eq!(
            Theme::builtin("gruvbox-dark").map(|theme| theme.today),
            Some(Color::Rgb(0xb8, 0xbb, 0x26))
        );
        assert_eq!(
            Theme::builtin("tokyo-night").map(|theme| theme.focus),
            Some(Color::Rgb(0xbb, 0x9a, 0xf7))
        );
        assert_eq!(
            Theme::builtin("catppuccin-latte").map(|theme| theme.today),
            Some(Color::Rgb(0x40, 0xa0, 0x2b))
        );
        assert_eq!(
            Theme::builtin("terminal").map(|theme| theme.text),
            Some(Color::Reset)
        );
    }

    #[test]
    fn roles_take_palette_names_or_colors() {
        // 從 mocha 改一行來試：色票名字、#rrggbb、terminal 的顏色名字、256 色的編號
        let mocha = BUILTIN[0].1;
        let with = |role: &str| mocha.replace("today = \"green\"", &format!("today = {role}"));
        assert_eq!(
            Theme::parse_text(&with("\"red\"")).map(|theme| theme.today),
            Ok(Color::Rgb(243, 139, 168))
        );
        assert_eq!(
            Theme::parse_text(&with("\"#010203\"")).map(|theme| theme.today),
            Ok(Color::Rgb(1, 2, 3))
        );
        assert_eq!(
            Theme::parse_text(&with("\"light-blue\"")).map(|theme| theme.today),
            Ok(Color::LightBlue)
        );
        assert_eq!(
            Theme::parse_text(&with("\"236\"")).map(|theme| theme.today),
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
            assert!(Theme::parse(&text, &BTreeMap::new()).is_err(), "{text}");
        }
    }
}
