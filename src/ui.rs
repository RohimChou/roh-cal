//! 畫面：每種 view 一個 module；兩種 view 共用的小東西（星期順序、週末顏色、點點、項目分組）也放這裡

pub mod compact;
pub mod day_list;
pub mod form;
pub mod help;
pub mod month;
pub mod settings;

use crate::app::{App, Notice, Overlay, View};
use crate::calendar::week_of_year;
use crate::tasks::Item;
use crate::theme::Theme;
use jiff::civil::{Date, Weekday};
use ratatui::{
    Frame,
    layout::{Constraint, Rect},
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

/// 依 `app.view` 畫出現在的畫面；有跳窗就疊在上面（設定頁在右下角、當天清單和表單在月曆正中間、說明頁在月曆框的左上角或畫面正中間）
pub fn render(frame: &mut Frame, app: &App) {
    match app.view {
        View::Compact => compact::render(frame, app),
        View::Month => month::render(frame, app),
    }
    match app.overlay {
        Some(Overlay::Settings(panel)) => settings::render(frame, panel, app),
        Some(Overlay::DayList(list)) => day_list::render(frame, list, app),
        Some(Overlay::Help { back, full, scroll }) => {
            // 從當天清單打開的：清單照畫，說明頁疊在上面
            if let Some(list) = back {
                day_list::render(frame, list, app);
            }
            help::render(frame, app, full, scroll);
        }
        Some(Overlay::Form(ref form)) => {
            // 不管從清單還是月曆打開，表單都直接疊在月曆上（置中在月曆框上）；底下不畫清單，
            // 兩個差不多大的框疊在一起，清單的框線會從表單上下露出來
            form::render(frame, form, app);
        }
        None => {}
    }
}

/// 現在開著的跳窗，框（含框線）在畫面上的範圍；沒有跳窗回傳 `None`（滑鼠點在框外 = 關掉跳窗）
pub fn overlay_bounds(app: &App) -> Option<Rect> {
    Some(match &app.overlay {
        Some(Overlay::Settings(panel)) => settings::bounds(*panel, app),
        Some(Overlay::DayList(list)) => day_list::bounds(*list, app),
        Some(Overlay::Help { full, .. }) => help::bounds(app, *full),
        Some(Overlay::Form(form)) => form::bounds(form, app),
        None => return None,
    })
}

/// 月曆框在 terminal（`screen`）裡的位置：小月曆是左上角的框，大月曆是整個 terminal
fn calendar_area(screen: Rect, app: &App) -> Rect {
    match app.view {
        View::Compact => compact::area(screen, app),
        View::Month => screen,
    }
}

/// 跳窗（當天清單、表單）的框最窄幾格（含框線）：刪除的問句 `delete with 2 subtasks? y = yes, other = no` 才放得下
const OVERLAY_MIN_WIDTH: u16 = 47;
/// 跳窗的框最寬幾格（含框線）：大月曆在很寬的 terminal 上才不會太長
const OVERLAY_MAX_WIDTH: u16 = 60;
/// 跳窗的框和月曆框的左右框線之間，各留幾格
const OVERLAY_MARGIN: u16 = 2;

/// 跳窗（當天清單、表單）的框（含框線）在 terminal（`screen`）的哪裡：`height` 行高，疊在月曆框正中間。
/// 寬度 = 月曆框左右各留 `OVERLAY_MARGIN` 格，但至少 `OVERLAY_MIN_WIDTH`、最多 `OVERLAY_MAX_WIDTH`；
/// 月曆框比跳窗窄時，跳窗貼齊月曆框的左邊、往右凸出去。都不會超出 terminal
fn overlay_area(screen: Rect, app: &App, height: u16) -> Rect {
    let calendar = calendar_area(screen, app);
    let width = calendar
        .width
        .saturating_sub(2 * OVERLAY_MARGIN)
        .clamp(OVERLAY_MIN_WIDTH, OVERLAY_MAX_WIDTH);
    // 置中的範圍：月曆框；月曆框比跳窗窄，就往右加寬到跟跳窗一樣寬（但不超出 terminal）
    let base = Rect {
        width: calendar.width.max(width),
        ..calendar
    }
    .intersection(screen);
    // 範圍不夠高、不夠寬就縮（Length 超過範圍會被縮到範圍的大小）
    base.centered(Constraint::Length(width), Constraint::Length(height))
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
    // 按了 V：列出接下來可以按的鍵（醒目的顏色）
    if app.view_prefix {
        return Line::styled(format!(" {} ", app.texts.view_prefix_hint), app.theme.title)
            .centered();
    }
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

/// 畫面每一行的文字，去掉尾端空白（test 用）；中文字佔 2 格，後面那格（buffer 裡是空白）跳過，接起來才是原本的字
#[cfg(test)]
pub fn rows(buffer: &ratatui::buffer::Buffer) -> Vec<String> {
    let area = buffer.area;
    (0..area.height)
        .map(|y| {
            let mut row = String::new();
            let mut x = 0;
            while x < area.width {
                let symbol = buffer[(x, y)].symbol();
                row.push_str(symbol);
                x += u16::try_from(Span::raw(symbol).width().max(1)).unwrap();
            }
            row.trim_end().to_string()
        })
        .collect()
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

    #[test]
    fn calendar_hint_fits_the_compact_view() {
        use crate::i18n::Texts;
        use ratatui::{Terminal, backend::TestBackend};
        // 小月曆的寬度是固定的：下框線的操作說明中英文都要整句看得到
        for texts in [Texts::en(), Texts::zh_tw()] {
            let mut app = App::on_date(date(2026, 10, 10), texts, Vec::new());
            let mut terminal = Terminal::new(TestBackend::new(80, 24)).unwrap();
            terminal.draw(|frame| render(frame, &app)).unwrap();
            let text = rows(terminal.backend().buffer()).join("\n");
            assert!(text.contains(texts.hint), "{}", texts.hint);
            // 按了 V：換成可以按的鍵，一樣整句看得到
            app.view_prefix = true;
            terminal.draw(|frame| render(frame, &app)).unwrap();
            let text = rows(terminal.backend().buffer()).join("\n");
            assert!(
                text.contains(texts.view_prefix_hint),
                "{}",
                texts.view_prefix_hint
            );
            assert!(!text.contains(texts.hint), "{}", texts.hint);
        }
    }

    #[test]
    fn overlays_are_at_least_47_and_at_most_60_wide() {
        use crate::i18n::Texts;
        let mut app = App::on_date(date(2026, 10, 10), Texts::en(), Vec::new());
        let screen = |width, height| Rect::new(0, 0, width, height);
        // 小月曆（框 51 寬）：左右各留 2 格 → 47 寬，x = 2
        assert_eq!(
            overlay_area(screen(80, 24), &app, 5),
            Rect::new(2, 4, 47, 5)
        );
        // 關掉週數（框 47 寬）：不再縮成 43，跟月曆框一樣寬、貼齊左邊
        app.settings.calendar.week_numbers = false;
        assert_eq!(
            overlay_area(screen(80, 24), &app, 5),
            Rect::new(0, 4, 47, 5)
        );
        // terminal 只有 40 寬：縮到 terminal 的寬度
        assert_eq!(
            overlay_area(screen(40, 24), &app, 5),
            Rect::new(0, 4, 40, 5)
        );
        // 大月曆（框 = 整個 terminal）：很寬時最多 60、置中；很窄時至少 47
        app.view = View::Month;
        assert_eq!(
            overlay_area(screen(120, 40), &app, 6),
            Rect::new(30, 17, 60, 6)
        );
        assert_eq!(
            overlay_area(screen(49, 15), &app, 5),
            Rect::new(1, 5, 47, 5)
        );
    }
}
