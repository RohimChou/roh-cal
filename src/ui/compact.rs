//! compact view：固定大小的小月曆（8 週），畫在 terminal 左上角

use crate::app::{App, View};
use crate::calendar::monday_of;
use crate::i18n::Texts;
use crate::tasks::Item;
use crate::theme::Theme;
use crate::ui::{WEEKDAYS, item_dots, week_number_span, weekend_style};
use jiff::{ToSpan, civil::Date};
use ratatui::{
    Frame,
    layout::Rect,
    text::{Line, Span},
    widgets::{Block, BorderType, Padding, Paragraph},
};
use std::collections::HashMap;

/// compact view 固定顯示幾週
const WEEKS: i64 = View::Compact.weeks();
/// 框線和內容之間：左右各空幾格、上下各空幾行
const PADDING: Padding = Padding::symmetric(2, 1);
/// 月份標記標在每個月幾號的那一行；15 號一定在一個月的中間幾行
const MARKER_DAY: i8 = 15;
/// 右邊月份標記的寬度：「 11 月」= 空白 1 + 月份 2 + 空白 1 + 「月」2（中文字佔 2 格）
const MONTH_LABEL_WIDTH: u16 = 6;
/// 左邊欄（週數 / 相對週數）的寬度：數字 3 + 空白 1
const WEEK_NUMBERS_WIDTH: u16 = 4;

/// 左邊欄的寬度：設定頁的 `week_numbers` 關掉時是 0（整欄拿掉）
fn left_width(week_numbers: bool) -> u16 {
    if week_numbers { WEEK_NUMBERS_WIDTH } else { 0 }
}

/// 月曆框的寬度：左右框線 2 + 左右 padding + 左邊欄 + 7 天 × 每格 5 + 月份標記
fn calendar_width(week_numbers: bool) -> u16 {
    2 + PADDING.left + PADDING.right + left_width(week_numbers) + 7 * 5 + MONTH_LABEL_WIDTH
}
/// 月曆框的高度：上下框線 2 + 上下 padding + 星期列 1 + WEEKS 週
const CALENDAR_HEIGHT: u16 = 2 + PADDING.top + PADDING.bottom + 1 + WEEKS as u16;

/// 畫出 compact view：剛好包住月曆的圓角框，年月在上邊線、操作說明在下邊線
pub fn render(frame: &mut Frame, app: &App) {
    let month = app.selected.first_of_month();
    let title = format!(" {} ", (app.texts.title)(month.year(), month.month()));
    let hint = format!(" {} ", app.texts.hint);
    let block = Block::bordered()
        // 圓角框線 ╭─╮
        .border_type(BorderType::Rounded)
        .border_style(app.theme.border)
        .padding(PADDING)
        // 上邊線中間：選取日期的年月
        .title_top(Line::styled(title, app.theme.title).centered())
        // 下邊線中間：操作說明
        .title_bottom(Line::styled(hint, app.theme.hint).centered());

    let area = area(frame.area(), app);
    let days = summarize_days(&app.items);
    let lines = calendar_lines(
        app.top,
        app.selected,
        app.today,
        &days,
        &app.theme,
        &app.texts,
        app.settings.calendar.week_numbers,
    );
    frame.render_widget(Paragraph::new(lines).block(block), area);
}

/// 月曆框在 terminal（`screen`）裡的位置：左上角，剛好包住月曆的大小（terminal 比較小時就用 terminal 的大小）
pub fn area(screen: Rect, app: &App) -> Rect {
    Rect {
        width: screen
            .width
            .min(calendar_width(app.settings.calendar.week_numbers)),
        height: screen.height.min(CALENDAR_HEIGHT),
        ..screen
    }
}

/// 最上面那週是 `top` 時，滑鼠點在 terminal 的 (`column`, `row`) 點到哪一天；沒點在日期格子上回傳 `None`
///
/// 月曆框固定畫在 terminal 左上角（見 `render`），例：預設排版（有左邊欄）時
/// `x` 7～11 是星期一、12～16 是星期二…；`y` 3 是第 1 週、4 是第 2 週…；左邊欄關掉時 `x` 全部往左 4 格
pub fn date_at(app: &App, column: u16, row: u16) -> Option<Date> {
    let top = app.top;
    // 第 1 週那行的 y：框線 1 + 上 padding + 星期列 1
    let first_row = 1 + PADDING.top + 1;
    // 星期一那格的 x：框線 1 + 左 padding + 左邊欄
    let first_column = 1 + PADDING.left + left_width(app.settings.calendar.week_numbers);
    // 比第一格還上面 / 左邊：checked_sub 得到 None，`?` 直接回傳 None
    let week = i64::from(row.checked_sub(first_row)?);
    let weekday = i64::from(column.checked_sub(first_column)? / 5);
    if week >= WEEKS || weekday >= 7 {
        return None;
    }
    top.checked_add((week * 7 + weekday).days()).ok()
}

/// 組出月曆：星期列，接著從 `top` 那週開始一週一行，共 `WEEKS` 週
///
/// 每行最左邊像 vim 的 relativenumber：選取的那行顯示選取日期的週數，其他行顯示跟選取那行差幾行
/// （`week_numbers` = false 時整欄拿掉）。
/// 每行最右邊：這週有某個月的 `MARKER_DAY` 號，就標出那個月（例：「11 月」），看起來像標在月份中間。
/// 顏色都來自 `theme`：週末、其他月份、今天、選取各有自己的樣式（疊的順序見 `Theme`）
///
/// 左邊欄 4 格，每格日期 5 個字元寬、靠右對齊在星期名稱下面，例：選 2026-10-10
/// ```text
///      MON  TUE  WED  THU  FRI  SAT  SUN
///   2   21   22   23   24   25   26   27    ← 9 月，灰色
///   1   28   29   30    1    2    3    4    ← 28–30 灰色
/// 41     5    6    7    8    9   10   11    ← 選取的那週：第 41 週
///   1   12   13   14   15   16   17   18
///  ...
/// ```
fn calendar_lines(
    top: Date,
    selected: Date,
    today: Date,
    days: &HashMap<Date, DaySummary>,
    theme: &Theme,
    texts: &Texts,
    week_numbers: bool,
) -> Vec<Line<'static>> {
    let month = selected.first_of_month();
    // 畫面上每一行的星期一
    let mondays: Vec<Date> = top.series(1.week()).take(WEEKS as usize).collect();
    // 選取的那週在第幾行（0 = 最上面）；選取一定在畫面上，找不到只是保險
    let selected_row = mondays
        .iter()
        .position(|&monday| monday == monday_of(selected))
        .unwrap_or(0);

    // 星期列：前面空出左邊欄的寬度，每個星期名稱佔 5 格（靠右對齊 4 格 + 1 個空白）
    let mut header = Line::from(" ".repeat(usize::from(left_width(week_numbers))));
    for (name, weekday) in texts.weekdays.into_iter().zip(WEEKDAYS) {
        let style = theme.weekday_header.patch(weekend_style(weekday, theme));
        let padding = " ".repeat(4_usize.saturating_sub(display_width(name)));
        header.push_span(Span::styled(format!("{padding}{name} "), style));
    }
    let mut lines = vec![header];
    for (row, monday) in mondays.into_iter().enumerate() {
        let mut week = Line::default();
        if row == selected_row {
            // 選取那行整行墊底色；每個日期自己的顏色會疊在上面
            week.style = theme.cursor_line;
        }
        // 左邊欄（設定頁可以關）：選取那行顯示選取日期的週數（靠左），其他行顯示差幾行（靠右）
        if week_numbers {
            week.push_span(week_number_span(row, selected_row, selected, theme));
            week.push_span(" ");
        }

        // 這週如果有某個月的 MARKER_DAY 號，記下是幾月，排完 7 天後標在最右邊
        let mut label_month = None;
        // 這一週的 7 天（只有 9999-12-31 這種日期上限才會不滿 7 天）
        for date in monday.series(1.day()).take(7) {
            if date.day() == MARKER_DAY {
                label_month = Some(date.month());
            }
            // 依序疊上 style（順序見 `Theme` 的說明）；數字和點點都疊同樣的「其他月份 → 今天 → 選取」，
            // 點點才會跟著數字變暗 / 變綠 / 在選取的藥丸裡變深色
            let mut number_style = theme.day.patch(weekend_style(date.weekday(), theme));
            let day = days.get(&date).copied().unwrap_or_default();
            // 還有沒完成的 → 醒目的點；都完成了 → 安靜的點
            let mut dots_style = if day.has_open_task {
                theme.dots_open
            } else {
                theme.dots_quiet
            };
            let layers = [
                (date.first_of_month() != month, theme.other_month),
                (date == today, theme.today),
                (date == selected, theme.selected),
            ];
            for (applies, layer) in layers {
                if applies {
                    number_style = number_style.patch(layer);
                    dots_style = dots_style.patch(layer);
                }
            }
            // 一格 5 字元 = 1 個空白 + 「空白 + 2 位數日期」+ 有幾個項目的點點（沒有就是空白）
            // 後面兩段都套 style，選取的背景色才不會只蓋住數字
            week.push_span(" ");
            week.push_span(Span::styled(format!(" {:>2}", date.day()), number_style));
            week.push_span(Span::styled(item_dots(day.count), dots_style));
        }
        let label = match label_month {
            Some(month) => format!(" {}", (texts.month_label)(month)),
            None => String::new(),
        };
        // 補空白到同樣寬度，每行才一樣寬（cursorline 的底色才會一樣長）
        let width = usize::from(MONTH_LABEL_WIDTH);
        let padding = " ".repeat(width.saturating_sub(display_width(&label)));
        week.push_span(Span::styled(label, theme.month_label));
        week.push_span(padding);
        lines.push(week);
    }
    lines
}

/// 某一天的項目摘要，決定點點畫幾個、用什麼顏色
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
struct DaySummary {
    /// 那天一共幾個項目
    count: usize,
    /// 有沒有還沒完成的任務
    has_open_task: bool,
}

/// 把項目依日期整理成每天的摘要；沒有日期的項目不算
/// （每次畫面重畫時算一次；項目很少，很快）
fn summarize_days(items: &[Item]) -> HashMap<Date, DaySummary> {
    let mut days: HashMap<Date, DaySummary> = HashMap::new();
    for item in items {
        let Some(date) = item.date else {
            continue;
        };
        let day = days.entry(date).or_default();
        day.count += 1;
        if !item.done {
            day.has_open_task = true;
        }
    }
    days
}

/// 文字在 terminal 上佔幾格：英文字母 1 格、中文字 2 格，例：「11 月」佔 5 格
fn display_width(text: &str) -> usize {
    Span::raw(text).width()
}

#[cfg(test)]
mod tests {
    use super::*;
    use jiff::civil::date;

    #[test]
    fn calendar_always_has_weekday_header_plus_fixed_weeks() {
        let app = App::on_date(date(2027, 2, 1), Texts::en(), Vec::new());
        let lines = calendar_lines(
            app.top,
            app.selected,
            app.today,
            &HashMap::new(),
            &app.theme,
            &app.texts,
            true,
        );
        assert_eq!(lines.len(), 1 + usize::try_from(WEEKS).unwrap_or(0));
        // 每一行一樣寬：左邊欄 4 + 7 天 × 5 + 月份標記
        let width = 4 + 7 * 5 + usize::from(MONTH_LABEL_WIDTH);
        assert!(lines[1..].iter().all(|line| line.width() == width));
    }

    #[test]
    fn summarize_days_counts_items_and_open_tasks() {
        let text = "- [ ] a @2026-10-12\n- 週會 @2026-10-12\n- [x] 國慶日 @2026-10-10\n\
                    - [x] 完成了 @2026-10-05\n- [ ] 沒日期";
        let days = summarize_days(&crate::tasks::parse(text).items);
        let summary = |count, has_open_task| DaySummary {
            count,
            has_open_task,
        };
        // 有沒完成的 → 醒目（沒 checkbox 的也算未完成）；都完成了 → 安靜；沒日期的不算
        assert_eq!(days[&date(2026, 10, 12)], summary(2, true));
        assert_eq!(days[&date(2026, 10, 10)], summary(1, false));
        assert_eq!(days[&date(2026, 10, 5)], summary(1, false));
        assert_eq!(days.len(), 3);
    }

    #[test]
    fn dates_show_one_to_four_dots_by_item_count() {
        let app = App::on_date(date(2026, 10, 10), Texts::en(), Vec::new());
        // 10/12 有 1 個、10/13 有 2 個、10/14 有 3 個、10/15 有 4 個、10/16 有 9 個
        let day = |count| DaySummary {
            count,
            has_open_task: false,
        };
        let days = HashMap::from([
            (date(2026, 10, 12), day(1)),
            (date(2026, 10, 13), day(2)),
            (date(2026, 10, 14), day(3)),
            (date(2026, 10, 15), day(4)),
            (date(2026, 10, 16), day(9)),
        ]);
        let lines = calendar_lines(
            app.top,
            app.selected,
            app.today,
            &days,
            &app.theme,
            &app.texts,
            true,
        );
        // 10/12 那週在第 4 行（9/21、9/28、10/5、10/12）
        let row: String = lines[4]
            .spans
            .iter()
            .map(|span| span.content.as_ref())
            .collect();
        for expected in [" 12⠂", " 13⠒", " 14⠖", " 15⠶", " 16⠶", " 17 "] {
            assert!(row.contains(expected), "{expected} in {row}");
        }
        // 點點一定佔 1 格，每行一樣寬
        assert_eq!(
            display_width(&row),
            4 + 7 * 5 + usize::from(MONTH_LABEL_WIDTH)
        );
    }

    #[test]
    fn date_at_maps_screen_position_to_date() {
        // 一打開：最上面那週從 9/21（星期一）開始
        let app = App::on_date(date(2026, 10, 10), Texts::en(), Vec::new());
        // 第 1 週星期一那格的頭、尾
        assert_eq!(date_at(&app, 7, 3), Some(date(2026, 9, 21)));
        assert_eq!(date_at(&app, 11, 3), Some(date(2026, 9, 21)));
        // 第 3 週星期六（x = 7 + 5 × 5）
        assert_eq!(date_at(&app, 32, 5), Some(date(2026, 10, 10)));
        // 最後一週星期日那格的尾
        assert_eq!(date_at(&app, 41, 10), Some(date(2026, 11, 15)));
        // 不是日期格子：框線、左邊欄、星期列、月份標記、最後一週下面
        for (column, row) in [(0, 0), (4, 5), (10, 2), (42, 5), (10, 11)] {
            assert_eq!(date_at(&app, column, row), None, "({column}, {row})");
        }
    }

    #[test]
    fn week_numbers_off_removes_the_left_column() {
        let mut app = App::on_date(date(2026, 10, 10), Texts::en(), Vec::new());
        let lines = calendar_lines(
            app.top,
            app.selected,
            app.today,
            &HashMap::new(),
            &app.theme,
            &app.texts,
            false,
        );
        // 每一週少了左邊欄的 4 格：7 天 × 5 + 月份標記；星期列也少 4 格
        let width = 7 * 5 + usize::from(MONTH_LABEL_WIDTH);
        assert!(lines[1..].iter().all(|line| line.width() == width));
        assert_eq!(lines[0].width(), 7 * 5);
        // 選取那週（第 3 週，`lines[3]`）不再以週數「41」開頭；cursorline 的底色還在
        assert!(!lines[3].to_string().starts_with("41"));
        assert_eq!(lines[3].style, app.theme.cursor_line);
        // 滑鼠：星期一那格整個往左 4 格（x = 3～7）
        app.settings.calendar.week_numbers = false;
        assert_eq!(date_at(&app, 3, 3), Some(date(2026, 9, 21)));
        assert_eq!(date_at(&app, 7, 3), Some(date(2026, 9, 21)));
        assert_eq!(date_at(&app, 8, 3), Some(date(2026, 9, 22)));
        assert_eq!(date_at(&app, 2, 3), None);
        assert_eq!(calendar_width(false), calendar_width(true) - 4);
    }

    #[test]
    fn month_labels_fit_their_column() {
        for texts in [Texts::en(), Texts::zh_tw()] {
            for month in 1..=12 {
                let label = format!(" {}", (texts.month_label)(month));
                assert!(
                    display_width(&label) <= usize::from(MONTH_LABEL_WIDTH),
                    "{label}"
                );
            }
        }
    }
}
