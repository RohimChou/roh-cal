//! 畫面用到的所有顏色與樣式；換 theme = 換一個 `Theme`

use ratatui::style::{Color, Modifier, Style};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Catppuccin Mocha 全部 26 色（名字, 顏色），設定頁換色用；順序照官方色票
pub const PALETTE: [(&str, Color); 26] = [
    ("rosewater", Color::Rgb(245, 224, 220)),
    ("flamingo", Color::Rgb(242, 205, 205)),
    ("pink", Color::Rgb(245, 194, 231)),
    ("mauve", Color::Rgb(203, 166, 247)),
    ("red", Color::Rgb(243, 139, 168)),
    ("maroon", Color::Rgb(235, 160, 172)),
    ("peach", Color::Rgb(250, 179, 135)),
    ("yellow", Color::Rgb(249, 226, 175)),
    ("green", Color::Rgb(166, 227, 161)),
    ("teal", Color::Rgb(148, 226, 213)),
    ("sky", Color::Rgb(137, 220, 235)),
    ("sapphire", Color::Rgb(116, 199, 236)),
    ("blue", Color::Rgb(137, 180, 250)),
    ("lavender", Color::Rgb(180, 190, 254)),
    ("text", Color::Rgb(205, 214, 244)),
    ("subtext1", Color::Rgb(186, 194, 222)),
    ("subtext0", Color::Rgb(166, 173, 200)),
    ("overlay2", Color::Rgb(147, 153, 178)),
    ("overlay1", Color::Rgb(127, 132, 156)),
    ("overlay0", Color::Rgb(108, 112, 134)),
    ("surface2", Color::Rgb(88, 91, 112)),
    ("surface1", Color::Rgb(69, 71, 90)),
    ("surface0", Color::Rgb(49, 50, 68)),
    ("base", Color::Rgb(30, 30, 46)),
    ("mantle", Color::Rgb(24, 24, 37)),
    ("crust", Color::Rgb(17, 17, 27)),
];

/// 月曆各部分的樣式
///
/// 同一格符合好幾種情況時，依序疊上去（`Style::patch`）：
/// 一般日期 → 星期六 / 星期日 → 其他月份 → 今天 → 選取，後面的會蓋掉前面設過的顏色
/// 點點也一樣：`dots_open` / `dots_quiet` → 其他月份 → 今天 → 選取（點點才會跟著數字變暗、變色）
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Theme {
    /// 框線
    pub border: Style,
    /// 上邊線的年月標題
    pub title: Style,
    /// 下邊線的操作說明
    pub hint: Style,
    /// 星期列（MON、TUE…）
    pub weekday_header: Style,
    /// 一般日期
    pub day: Style,
    /// 星期六（星期列和日期都用，小 / 大月曆共用）
    pub saturday: Style,
    /// 星期日（同上；預設跟星期六同色，分成兩個欄位是讓想分開的人可以分）
    pub sunday: Style,
    /// 不是選取日期那個月的日期
    pub other_month: Style,
    /// 今天
    pub today: Style,
    /// 選取的日期
    pub selected: Style,
    /// 選取的那一整行（像 vim 的 cursorline），墊在這行所有日期底下
    pub cursor_line: Style,
    /// 數字旁邊的點點（1～4 個）：那天還有沒完成的任務
    pub dots_open: Style,
    /// 數字旁邊的點點：那天的項目都完成了（比較安靜）
    pub dots_quiet: Style,
    /// 右邊的月份標記（每個月 15 號那行的「11 月」）
    pub month_label: Style,
    /// 左邊欄：選取那行的週數
    pub week_number: Style,
    /// 左邊欄：其他行跟選取那行差幾行
    pub row_distance: Style,
    /// 大月曆：選取那一格的整格底色（其他格子不塗底，日期數字也不換色）
    pub selected_bg: Style,
    /// 大月曆：本月日期數字額外疊的樣式（粗體；格子大，數字要跳出來。其他月份不疊）
    pub cell_day: Style,
    /// 大月曆格子裡的項目：還沒完成的（`14:00 標題`，不加符號）
    pub item: Style,
    /// 大月曆格子裡的項目：完成的（整行變灰 + 劃線；不支援劃線的 terminal 只看到變灰）
    pub item_done: Style,
    /// 大月曆格子放不下時最後一行的 `+N`（項目的時間跟著項目自己的顏色，不用這個）
    pub more_items: Style,
    /// 大月曆格子之間淡淡的格線
    pub grid_lines: Style,
    /// 說明頁（`?`）的段落名，例：`CALENDAR`
    pub help_section: Style,
    /// 說明頁的按鍵那欄，例：`Enter / o`
    pub help_key: Style,
}

impl Default for Theme {
    /// 預設 theme：Catppuccin Mocha
    fn default() -> Self {
        Self::catppuccin_mocha()
    }
}

impl Theme {
    /// [Catppuccin Mocha](https://catppuccin.com/palette)：暗色背景用的柔和配色
    pub fn catppuccin_mocha() -> Self {
        // 色票：https://github.com/catppuccin/palette 的 mocha
        let text = Color::Rgb(205, 214, 244);
        let subtext1 = Color::Rgb(186, 194, 222);
        let subtext0 = Color::Rgb(166, 173, 200);
        let overlay0 = Color::Rgb(108, 112, 134);
        let surface2 = Color::Rgb(88, 91, 112);
        let surface1 = Color::Rgb(69, 71, 90);
        let surface0 = Color::Rgb(49, 50, 68);
        let base = Color::Rgb(30, 30, 46);
        let lavender = Color::Rgb(180, 190, 254);
        let blue = Color::Rgb(137, 180, 250);
        let green = Color::Rgb(166, 227, 161);
        let yellow = Color::Rgb(249, 226, 175);
        let mauve = Color::Rgb(203, 166, 247);
        let peach = Color::Rgb(250, 179, 135);

        let bold = Modifier::BOLD;
        Self {
            border: Style::new().fg(surface2),
            title: Style::new().fg(lavender).add_modifier(bold),
            hint: Style::new().fg(subtext0),
            weekday_header: Style::new().fg(subtext0),
            day: Style::new().fg(text),
            saturday: Style::new().fg(blue),
            sunday: Style::new().fg(blue),
            other_month: Style::new().fg(overlay0),
            today: Style::new().fg(green).add_modifier(bold),
            selected: Style::new().fg(base).bg(mauve).add_modifier(bold),
            cursor_line: Style::new().bg(surface0),
            dots_open: Style::new().fg(peach),
            dots_quiet: Style::new().fg(subtext0),
            month_label: Style::new().fg(lavender),
            week_number: Style::new().fg(yellow).add_modifier(bold),
            row_distance: Style::new().fg(overlay0),
            selected_bg: Style::new().bg(surface1),
            cell_day: Style::new().add_modifier(bold),
            item: Style::new().fg(subtext1),
            item_done: Style::new()
                .fg(overlay0)
                .add_modifier(Modifier::CROSSED_OUT),
            more_items: Style::new().fg(subtext0),
            grid_lines: Style::new().fg(surface0),
            help_section: Style::new().fg(blue).add_modifier(bold),
            help_key: Style::new().fg(peach).add_modifier(bold),
        }
    }

    /// 預設 theme，再換上設定檔 `[colors]` 裡改過的顏色（例：`sunday = "#f38ba8"`）
    pub fn with_colors(colors: &BTreeMap<ColorField, HexColor>) -> Self {
        let mut theme = Self::default();
        for (&field, &color) in colors {
            field.set_color(&mut theme, color.into());
        }
        theme
    }
}

/// `Theme` 裡可以換色的欄位（設定頁一個欄位一列，順序見 `settings::ROWS`）
///
/// 每個欄位的名字、對到 `Theme` 哪個欄位、調底色還是文字色，都只寫在這裡；
/// 新增一個欄位時，`match` 少寫一個 compiler 就會報錯。
/// 設定檔 `[colors]` 的名字由 serde 從 variant 名轉成 snake_case（`MoreItems` → `more_items`），跟 `name()` 一樣（有 test 釘住）；
/// `Ord`：當 map 的 key 時照宣告的順序排
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ColorField {
    Item,
    ItemDone,
    MoreItems,
    GridLines,
    SelectedBg,
    Day,
    OtherMonth,
    Today,
    Title,
    MonthLabel,
    WeekdayHeader,
    Saturday,
    Sunday,
}

impl ColorField {
    /// 名字 = `Theme` 的欄位名（設定頁顯示的就是它）
    pub fn name(self) -> &'static str {
        match self {
            Self::Item => "item",
            Self::ItemDone => "item_done",
            Self::MoreItems => "more_items",
            Self::GridLines => "grid_lines",
            Self::SelectedBg => "selected_bg",
            Self::Day => "day",
            Self::OtherMonth => "other_month",
            Self::Today => "today",
            Self::Title => "title",
            Self::MonthLabel => "month_label",
            Self::WeekdayHeader => "weekday_header",
            Self::Saturday => "saturday",
            Self::Sunday => "sunday",
        }
    }

    /// 現在的顏色（底色或文字色，看 `uses_bg`）
    pub fn color(self, theme: &Theme) -> Option<Color> {
        let style = self.style(theme);
        if self.uses_bg() { style.bg } else { style.fg }
    }

    /// 換顏色（底色或文字色，看 `uses_bg`）；粗體、劃線這些不動
    pub fn set_color(self, theme: &mut Theme, color: Color) {
        let style = self.style_mut(theme);
        *style = if self.uses_bg() {
            style.bg(color)
        } else {
            style.fg(color)
        };
    }

    /// 調的是底色（整格塗色的那種）還是文字色
    fn uses_bg(self) -> bool {
        self == Self::SelectedBg
    }

    /// 這個欄位現在的樣式
    fn style(self, theme: &Theme) -> Style {
        // `style_mut` 要 `&mut`；Theme 是 Copy，複製一份來讀就好
        let mut theme = *theme;
        *self.style_mut(&mut theme)
    }

    /// 這個欄位在 `theme` 裡的位置（要改它就透過這裡）
    fn style_mut(self, theme: &mut Theme) -> &mut Style {
        match self {
            Self::Item => &mut theme.item,
            Self::ItemDone => &mut theme.item_done,
            Self::MoreItems => &mut theme.more_items,
            Self::GridLines => &mut theme.grid_lines,
            Self::SelectedBg => &mut theme.selected_bg,
            Self::Day => &mut theme.day,
            Self::OtherMonth => &mut theme.other_month,
            Self::Today => &mut theme.today,
            Self::Title => &mut theme.title,
            Self::MonthLabel => &mut theme.month_label,
            Self::WeekdayHeader => &mut theme.weekday_header,
            Self::Saturday => &mut theme.saturday,
            Self::Sunday => &mut theme.sunday,
        }
    }
}

/// 設定檔裡的顏色：`"#rrggbb"`（例：`"#f38ba8"`），跟 theme 無關，換哪個 theme 都看得懂
///
/// `try_from` / `into`：serde 讀寫時先轉成字串，再用下面的 `TryFrom` / `From` 跟 `HexColor` 互轉；
/// 格式不對（`"blue"`、`"#12345"`）讀檔時就是錯誤
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(try_from = "String", into = "String")]
pub struct HexColor(pub u8, pub u8, pub u8);

impl HexColor {
    /// RGB 顏色才換得成色碼；terminal 的 16 色（`Color::Blue` 這種）沒有固定的色碼，回傳 `None`
    pub fn from_color(color: Color) -> Option<Self> {
        match color {
            Color::Rgb(red, green, blue) => Some(Self(red, green, blue)),
            _ => None,
        }
    }
}

impl TryFrom<String> for HexColor {
    type Error = String;

    /// `"#f38ba8"` → `HexColor(243, 139, 168)`；大小寫都可以
    fn try_from(text: String) -> Result<Self, String> {
        let digits = text
            .strip_prefix('#')
            .filter(|digits| digits.len() == 6 && digits.chars().all(|c| c.is_ascii_hexdigit()))
            .ok_or_else(|| format!("invalid color \"{text}\": expected \"#rrggbb\""))?;
        // 上面確認過是 6 個 16 進位數字，這裡一定換得成功
        let byte = |start: usize| u8::from_str_radix(&digits[start..start + 2], 16).unwrap_or(0);
        Ok(Self(byte(0), byte(2), byte(4)))
    }
}

impl From<HexColor> for String {
    /// `HexColor(243, 139, 168)` → `"#f38ba8"`（小寫）
    fn from(color: HexColor) -> Self {
        format!("#{:02x}{:02x}{:02x}", color.0, color.1, color.2)
    }
}

impl From<HexColor> for Color {
    fn from(color: HexColor) -> Self {
        Color::Rgb(color.0, color.1, color.2)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hex_color_reads_and_writes_rrggbb() {
        // f3 = 243、8b = 139、a8 = 168（Catppuccin 的 red）
        let red = HexColor::try_from("#f38ba8".to_string()).expect("valid");
        assert_eq!(red, HexColor(243, 139, 168));
        assert_eq!(HexColor::try_from("#F38BA8".to_string()), Ok(red));
        assert_eq!(String::from(red), "#f38ba8");
        assert_eq!(Color::from(red), Color::Rgb(243, 139, 168));
        for text in ["blue", "f38ba8", "#f38ba", "#f38ba8a", "#gggggg", ""] {
            assert!(HexColor::try_from(text.to_string()).is_err(), "{text}");
        }
    }

    #[test]
    fn with_colors_changes_only_the_listed_fields() {
        let colors = BTreeMap::from([(ColorField::Sunday, HexColor(1, 2, 3))]);
        let theme = Theme::with_colors(&colors);
        assert_eq!(theme.sunday.fg, Some(Color::Rgb(1, 2, 3)));
        // 沒列到的跟預設一樣（星期六預設也是 blue，不跟著星期日變）
        assert_eq!(theme.saturday, Theme::default().saturday);
        assert_eq!(
            Theme::with_colors(&BTreeMap::new()).item,
            Theme::default().item
        );
    }
}
