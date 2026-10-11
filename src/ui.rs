//! 畫面：每種 view 一個 module；兩種 view 共用的小東西（星期順序、週末顏色、點點、項目分組）也放這裡

pub mod compact;
pub mod day_list;
pub mod month;
pub mod settings;

use crate::app::{App, Notice, Overlay, View};
use crate::calendar::week_of_year;
use crate::tasks::Item;
use crate::theme::Theme;
use jiff::civil::{Date, Weekday};
use ratatui::{
    Frame,
    layout::Rect,
    style::Style,
    text::{Line, Span},
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

/// 依 `app.view` 畫出現在的畫面；有跳窗就疊在上面（設定頁在右下角、當天清單在月曆正中間）
pub fn render(frame: &mut Frame, app: &App) {
    match app.view {
        View::Compact => compact::render(frame, app),
        View::Month => month::render(frame, app),
    }
    match app.overlay {
        Some(Overlay::Settings(panel)) => settings::render(frame, panel, app),
        Some(Overlay::DayList(list)) => {
            let calendar = calendar_area(frame.area(), app);
            day_list::render(frame, list, app, calendar);
        }
        None => {}
    }
}

/// 月曆框在 terminal（`screen`）裡的位置：小月曆是左上角的框，大月曆是整個 terminal
fn calendar_area(screen: Rect, app: &App) -> Rect {
    match app.view {
        View::Compact => compact::area(screen, app),
        View::Month => screen,
    }
}

/// 下框線暫時換成的提示（醒目的顏色）；沒有提示回傳 `None`，下框線照常顯示操作說明
fn notice_line(app: &App) -> Option<Line<'static>> {
    let text = match app.notice.as_ref()? {
        Notice::Reloaded => app.texts.file_reloaded.to_string(),
        Notice::Failed(kind) => (app.texts.file_failed)(*kind),
        Notice::Undone(title) => (app.texts.undone)(title),
        Notice::NothingToUndo => app.texts.nothing_to_undo.to_string(),
    };
    Some(Line::styled(format!(" {text} "), app.theme.title))
}

/// 月曆（大小都是）的下框線：平常是操作說明；有提示、而且沒有跳窗蓋在上面時換成提示
/// （有跳窗時，提示顯示在跳窗的下框線）
fn calendar_bottom(app: &App) -> Line<'static> {
    let notice = if app.overlay.is_none() {
        notice_line(app)
    } else {
        None
    };
    notice
        .unwrap_or_else(|| Line::styled(format!(" {} ", app.texts.hint), app.theme.hint))
        .centered()
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

/// 左邊欄的數字（3 格寬）：選取那週（第 `selected_row` 行）顯示選取日期的週數（靠左、醒目），
/// 其他週顯示跟選取那週差幾週（靠右、淡）；像 vim 的 `number` + `relativenumber`，大小月曆共用
fn week_number_span(
    row: usize,
    selected_row: usize,
    selected: Date,
    theme: &Theme,
) -> Span<'static> {
    if row == selected_row {
        Span::styled(format!("{:<3}", week_of_year(selected)), theme.week_number)
    } else {
        let distance = row.abs_diff(selected_row);
        Span::styled(format!("{distance:>3}"), theme.row_distance)
    }
}

/// 把有日期的項目依日期分組（沒日期的不算）；每天的順序見 `sort_day`
fn items_by_day(items: &[Item]) -> HashMap<Date, Vec<&Item>> {
    let mut days: HashMap<Date, Vec<&Item>> = HashMap::new();
    for item in items {
        if let Some(date) = item.date {
            days.entry(date).or_default().push(item);
        }
    }
    for day in days.values_mut() {
        sort_day(day);
    }
    days
}

/// 某一天的項目，順序跟月曆格子裡一樣（當天清單用）
pub fn items_on(items: &[Item], date: Date) -> Vec<&Item> {
    let mut day: Vec<&Item> = items
        .iter()
        .filter(|item| item.date == Some(date))
        .collect();
    sort_day(&mut day);
    day
}

/// 一天裡的順序：有時間的先、照時間排；沒時間的排後面、照檔案順序
fn sort_day(day: &mut [&Item]) {
    // `None < Some(..)`，所以先用 is_none() 把沒時間的排到後面；stable sort 保留檔案順序
    day.sort_by_key(|item| (item.time.is_none(), item.time));
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
        // 當天清單拿到的跟月曆格子裡的一模一樣
        assert_eq!(
            items_on(&items, date(2026, 10, 12)),
            days[&date(2026, 10, 12)]
        );
        assert!(items_on(&items, date(2026, 10, 14)).is_empty());
    }
}
