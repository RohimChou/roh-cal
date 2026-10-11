//! 畫面：每種 view 一個 module；兩種 view 共用的小東西（星期順序、週末顏色、點點、項目分組）也放這裡

pub mod compact;
pub mod month;

use crate::app::{App, View};
use crate::tasks::Item;
use crate::theme::Theme;
use jiff::civil::{Date, Weekday};
use ratatui::{
    Frame,
    layout::Rect,
    style::Style,
    text::{Line, Span},
    widgets::{Block, BorderType, Clear, Paragraph},
};
use std::collections::HashMap;

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

/// 依 `app.view` 畫出現在的畫面；debug 選色器開著就疊在右下角
pub fn render(frame: &mut Frame, app: &App) {
    match app.view {
        View::Compact => compact::render(frame, app),
        View::Month => month::render(frame, app),
    }
    if let Some(picker) = app.picker {
        render_picker(frame, picker, &app.theme);
    }
}

/// debug 選色器的小框（右下角）：欄位、顏色名和 RGB、一段用那個顏色畫的範例字
fn render_picker(frame: &mut Frame, picker: crate::theme::Picker, theme: &Theme) {
    let (name, color) = picker.color();
    let rgb = match color {
        ratatui::style::Color::Rgb(r, g, b) => format!("({r}, {g}, {b})"),
        other => format!("{other:?}"),
    };
    let lines = vec![
        Line::from(format!(
            " field  {}   (Tab / Shift+Tab)",
            picker.field_name()
        )),
        Line::from(format!(" color  {name} {rgb}   (j / k)")),
        Line::from(vec![
            Span::raw(" sample "),
            Span::styled("14:00 範例標題 sample", Style::new().fg(color)),
        ]),
    ];
    let block = Block::bordered()
        .border_type(BorderType::Rounded)
        .border_style(theme.border)
        .title_top(Line::styled(" colors (Option+q / Esc) ", theme.title));
    let width = 44;
    let height = 5;
    let area = frame.area();
    let overlay = Rect {
        x: area.right().saturating_sub(width),
        y: area.bottom().saturating_sub(height),
        width: width.min(area.width),
        height: height.min(area.height),
    };
    // 先清掉底下的畫面，框才不會跟月曆的字疊在一起
    frame.render_widget(Clear, overlay);
    frame.render_widget(Paragraph::new(lines).block(block), overlay);
}

/// 星期六、星期日的樣式；平日不另外上色（回傳空的 `Style`，疊上去不會改變任何東西）
fn weekend_style(weekday: Weekday, theme: &Theme) -> Style {
    match weekday {
        Weekday::Saturday => theme.saturday,
        Weekday::Sunday => theme.sunday,
        _ => Style::new(),
    }
}

/// 那天有幾個項目，就畫幾個點（最多 4 個，4 個以上都畫 4 個）
///
/// 用點字（Braille）字元：一個字元就能放好幾個點，而且一定佔 1 格寬
/// （`•` 這類字在開了 CJK 寬字元的 terminal 會變 2 格，月曆會歪）
fn item_dots(count: usize) -> &'static str {
    match count {
        0 => " ",
        1 => "⠂",
        2 => "⠒",
        3 => "⠖",
        _ => "⠶",
    }
}

/// 把有日期的項目依日期分組（沒日期的不算）
///
/// 每天的順序：有時間的先、照時間排；沒時間的排後面、照檔案順序
fn items_by_day(items: &[Item]) -> HashMap<Date, Vec<&Item>> {
    let mut days: HashMap<Date, Vec<&Item>> = HashMap::new();
    for item in items {
        if let Some(date) = item.date {
            days.entry(date).or_default().push(item);
        }
    }
    for day in days.values_mut() {
        // `None < Some(..)`，所以先用 is_none() 把沒時間的排到後面；stable sort 保留檔案順序
        day.sort_by_key(|item| (item.time.is_none(), item.time));
    }
    days
}

#[cfg(test)]
mod tests {
    use super::*;
    use jiff::civil::date;

    #[test]
    fn items_by_day_puts_timed_items_first_in_time_order() {
        let text = "- [ ] 下午 @2026-10-12 14:00\n- 沒時間 A @2026-10-12\n- [x] 早上 @2026-10-12 09:00\n\
                    - 沒時間 B @2026-10-12\n- [ ] 沒日期\n- 別天 @2026-10-13";
        let items = crate::tasks::parse(text).items;
        let days = items_by_day(&items);
        let titles: Vec<&str> = days[&date(2026, 10, 12)]
            .iter()
            .map(|item| item.title.as_str())
            .collect();
        assert_eq!(titles, ["早上", "下午", "沒時間 A", "沒時間 B"]);
        assert_eq!(days[&date(2026, 10, 13)].len(), 1);
        assert_eq!(days.len(), 2);
    }
}
