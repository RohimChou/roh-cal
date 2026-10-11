//! 畫面用到的所有顏色與樣式；換 theme = 換一個 `Theme`

use ratatui::style::{Color, Modifier, Style};

/// 月曆各部分的樣式
///
/// 同一格符合好幾種情況時，依序疊上去（`Style::patch`）：
/// 一般日期 → 星期六 / 星期日 → 其他月份 → 今天 → 選取，後面的會蓋掉前面設過的顏色
/// 點點也一樣：`dots_open` / `dots_quiet` → 其他月份 → 今天 → 選取（點點才會跟著數字變暗、變色）
#[derive(Debug, Clone, Copy)]
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
    /// 星期六（星期列和日期都用）
    pub saturday: Style,
    /// 星期日（星期列和日期都用）
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
    pub month_marker: Style,
    /// 左邊欄：選取那行的週數
    pub week_number: Style,
    /// 左邊欄：其他行跟選取那行差幾行
    pub row_distance: Style,
    /// 大月曆：選取那一格的整格底色（比 terminal 底色亮一點點；其他格子不塗底）
    pub cell_selected: Style,
    /// 大月曆：選取那一格的日期數字（格子已有底色，數字只換顏色，不用藥丸）
    pub cell_selected_day: Style,
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
        let subtext0 = Color::Rgb(166, 173, 200);
        let overlay0 = Color::Rgb(108, 112, 134);
        let surface2 = Color::Rgb(88, 91, 112);
        let surface0 = Color::Rgb(49, 50, 68);
        let base = Color::Rgb(30, 30, 46);
        // 不在 Catppuccin 色票上：base 往亮調一點點，選取的格子才不會像一塊色板
        let base_lighter = Color::Rgb(40, 40, 58);
        let lavender = Color::Rgb(180, 190, 254);
        let blue = Color::Rgb(137, 180, 250);
        let red = Color::Rgb(243, 139, 168);
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
            sunday: Style::new().fg(red),
            other_month: Style::new().fg(overlay0),
            today: Style::new().fg(green).add_modifier(bold),
            selected: Style::new().fg(base).bg(mauve).add_modifier(bold),
            cursor_line: Style::new().bg(surface0),
            dots_open: Style::new().fg(peach),
            dots_quiet: Style::new().fg(subtext0),
            month_marker: Style::new().fg(lavender),
            week_number: Style::new().fg(yellow).add_modifier(bold),
            row_distance: Style::new().fg(overlay0),
            cell_selected: Style::new().bg(base_lighter),
            cell_selected_day: Style::new().fg(mauve).add_modifier(bold),
        }
    }
}
