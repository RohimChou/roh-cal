//! 畫面用到的所有顏色與樣式；換 theme = 換一個 `Theme`

use ratatui::style::{Color, Modifier, Style};

/// 月曆各部分的樣式
///
/// 同一格符合好幾種情況時，依序疊上去（`Style::patch`）：
/// 其他月份 → 今天 → 選取，後面的會蓋掉前面設過的顏色
#[derive(Debug, Clone, Copy)]
pub struct Theme {
    /// 不是選取日期那個月的日期
    pub other_month: Style,
    /// 今天
    pub today: Style,
    /// 選取的日期
    pub selected: Style,
    /// 左邊欄：選取那行的週數
    pub week_number: Style,
    /// 左邊欄：其他行跟選取那行差幾行
    pub row_distance: Style,
}

impl Default for Theme {
    /// 預設 theme：terminal 的 16 色
    fn default() -> Self {
        Self {
            other_month: Style::new().fg(Color::DarkGray),
            today: Style::new().add_modifier(Modifier::UNDERLINED),
            // 反白：前景色和背景色對調
            selected: Style::new().add_modifier(Modifier::REVERSED),
            week_number: Style::new().fg(Color::Yellow).add_modifier(Modifier::BOLD),
            row_distance: Style::new().fg(Color::DarkGray),
        }
    }
}
