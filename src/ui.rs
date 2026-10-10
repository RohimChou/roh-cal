//! 畫面：每種 view 一個 module；兩種 view 共用的小東西（星期順序、週末顏色）也放這裡

pub mod compact;
pub mod month;

use crate::app::{App, View};
use crate::theme::Theme;
use jiff::civil::Weekday;
use ratatui::{Frame, style::Style};

/// 一週的 7 天，照畫面上由左到右的順序（星期一開始）
const WEEKDAYS: [Weekday; 7] = [
    Weekday::Monday,
    Weekday::Tuesday,
    Weekday::Wednesday,
    Weekday::Thursday,
    Weekday::Friday,
    Weekday::Saturday,
    Weekday::Sunday,
];

/// 依 `app.view` 畫出現在的畫面
pub fn render(frame: &mut Frame, app: &App) {
    match app.view {
        View::Compact => compact::render(frame, app),
        View::Month => month::render(frame, app),
    }
}

/// 星期六、星期日的樣式；平日不另外上色（回傳空的 `Style`，疊上去不會改變任何東西）
fn weekend_style(weekday: Weekday, theme: &Theme) -> Style {
    match weekday {
        Weekday::Saturday => theme.saturday,
        Weekday::Sunday => theme.sunday,
        _ => Style::new(),
    }
}
