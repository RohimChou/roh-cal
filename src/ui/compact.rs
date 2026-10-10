//! compact view：固定大小的小月曆（8 週），畫在 terminal 左上角

use crate::app::{App, WEEKS};
use crate::calendar::{monday_of, week_of_year};
use crate::i18n::Texts;
use crate::theme::Theme;
use jiff::{
    ToSpan,
    civil::{Date, Weekday},
};
use ratatui::{
    Frame,
    layout::Rect,
    style::Style,
    text::{Line, Span},
    widgets::{Block, BorderType, Padding, Paragraph},
};

/// 框線和內容之間：左右各空幾格、上下各空幾行
const PADDING: Padding = Padding::symmetric(2, 1);
/// 月份標記標在每個月幾號的那一行；15 號一定在一個月的中間幾行
const MARKER_DAY: i8 = 15;
/// 右邊月份標記的寬度：「 11 月」= 空白 1 + 月份 2 + 空白 1 + 「月」2（中文字佔 2 格）
const MONTH_MARKER_WIDTH: u16 = 6;
/// 月曆框的寬度：左右框線 2 + 左右 padding + 左邊欄 4 + 7 天 × 每格 5 + 月份標記
const CALENDAR_WIDTH: u16 = 2 + PADDING.left + PADDING.right + 4 + 7 * 5 + MONTH_MARKER_WIDTH;
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

    // 框只畫到剛好包住月曆的大小（terminal 比較小時就用 terminal 的大小）
    let area = frame.area();
    let area = Rect {
        width: area.width.min(CALENDAR_WIDTH),
        height: area.height.min(CALENDAR_HEIGHT),
        ..area
    };
    let lines = calendar_lines(app.top, app.selected, app.today, &app.theme, &app.texts);
    frame.render_widget(Paragraph::new(lines).block(block), area);
}

/// 最上面那週是 `top` 時，滑鼠點在 terminal 的 (`column`, `row`) 點到哪一天；沒點在日期格子上回傳 `None`
///
/// 月曆框固定畫在 terminal 左上角（見 `render`），例：預設排版時
/// `x` 7～11 是星期一、12～16 是星期二…；`y` 3 是第 1 週、4 是第 2 週…
pub fn date_at(top: Date, column: u16, row: u16) -> Option<Date> {
    // 第 1 週那行的 y：框線 1 + 上 padding + 星期列 1
    let first_row = 1 + PADDING.top + 1;
    // 星期一那格的 x：框線 1 + 左 padding + 左邊欄 4
    let first_column = 1 + PADDING.left + 4;
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
/// 每行最左邊像 vim 的 relativenumber：選取的那行顯示選取日期的週數，其他行顯示跟選取那行差幾行。
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
    theme: &Theme,
    texts: &Texts,
) -> Vec<Line<'static>> {
    let month = selected.first_of_month();
    // 畫面上每一行的星期一
    let mondays: Vec<Date> = top.series(1.week()).take(WEEKS as usize).collect();
    // 選取的那週在第幾行（0 = 最上面）；選取一定在畫面上，找不到只是保險
    let selected_row = mondays
        .iter()
        .position(|&monday| monday == monday_of(selected))
        .unwrap_or(0);

    // 星期列：前面空出左邊欄的 4 格，每個星期名稱佔 5 格（靠右對齊 4 格 + 1 個空白）
    let mut header = Line::from("    ");
    let weekdays = [
        Weekday::Monday,
        Weekday::Tuesday,
        Weekday::Wednesday,
        Weekday::Thursday,
        Weekday::Friday,
        Weekday::Saturday,
        Weekday::Sunday,
    ];
    for (name, weekday) in texts.weekdays.into_iter().zip(weekdays) {
        let style = theme.weekday_header.patch(weekend_style(weekday, theme));
        let padding = " ".repeat(4_usize.saturating_sub(display_width(name)));
        header.push_span(Span::styled(format!("{padding}{name} "), style));
    }
    let mut lines = vec![header];
    for (row, monday) in mondays.into_iter().enumerate() {
        let mut week = Line::default();
        // 左邊欄：選取那行顯示選取日期的週數（靠左），其他行顯示差幾行（靠右）
        if row == selected_row {
            // 選取那行整行墊底色；每個日期自己的顏色會疊在上面
            week.style = theme.cursor_line;
            let week_number = format!("{:<3}", week_of_year(selected));
            week.push_span(Span::styled(week_number, theme.week_number));
        } else {
            let distance = row.abs_diff(selected_row);
            week.push_span(Span::styled(format!("{distance:>3}"), theme.row_distance));
        }
        week.push_span(" ");

        // 這週如果有某個月的 MARKER_DAY 號，記下是幾月，排完 7 天後標在最右邊
        let mut marker_month = None;
        // 這一週的 7 天（只有 9999-12-31 這種日期上限才會不滿 7 天）
        for date in monday.series(1.day()).take(7) {
            if date.day() == MARKER_DAY {
                marker_month = Some(date.month());
            }
            // 依序疊上 style（順序見 `Theme` 的說明）
            let mut style = theme.day.patch(weekend_style(date.weekday(), theme));
            if date.first_of_month() != month {
                style = style.patch(theme.other_month);
            }
            if date == today {
                style = style.patch(theme.today);
            }
            if date == selected {
                style = style.patch(theme.selected);
            }
            // 一格 5 字元 = 1 個空白 + 「空白 + 2 位數日期 + 空白」
            // 後面那段套 style，選取的背景色才不會只蓋住數字
            week.push_span(" ");
            week.push_span(Span::styled(format!(" {:>2} ", date.day()), style));
        }
        let marker = match marker_month {
            Some(month) => format!(" {}", (texts.month_marker)(month)),
            None => String::new(),
        };
        // 補空白到同樣寬度，每行才一樣寬（cursorline 的底色才會一樣長）
        let width = usize::from(MONTH_MARKER_WIDTH);
        let padding = " ".repeat(width.saturating_sub(display_width(&marker)));
        week.push_span(Span::styled(marker, theme.month_marker));
        week.push_span(padding);
        lines.push(week);
    }
    lines
}

/// 星期六、星期日的樣式；平日不另外上色（回傳空的 `Style`，疊上去不會改變任何東西）
fn weekend_style(weekday: Weekday, theme: &Theme) -> Style {
    match weekday {
        Weekday::Saturday => theme.saturday,
        Weekday::Sunday => theme.sunday,
        _ => Style::new(),
    }
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
        let app = App::on_date(date(2027, 2, 1), Texts::en());
        let lines = calendar_lines(app.top, app.selected, app.today, &app.theme, &app.texts);
        assert_eq!(lines.len(), 1 + usize::try_from(WEEKS).unwrap_or(0));
        // 每一行一樣寬：左邊欄 4 + 7 天 × 5 + 月份標記
        let width = 4 + 7 * 5 + usize::from(MONTH_MARKER_WIDTH);
        assert!(lines[1..].iter().all(|line| line.width() == width));
    }

    #[test]
    fn date_at_maps_screen_position_to_date() {
        // 一打開：最上面那週從 9/21（星期一）開始
        let app = App::on_date(date(2026, 10, 10), Texts::en());
        // 第 1 週星期一那格的頭、尾
        assert_eq!(date_at(app.top, 7, 3), Some(date(2026, 9, 21)));
        assert_eq!(date_at(app.top, 11, 3), Some(date(2026, 9, 21)));
        // 第 3 週星期六（x = 7 + 5 × 5）
        assert_eq!(date_at(app.top, 32, 5), Some(date(2026, 10, 10)));
        // 最後一週星期日那格的尾
        assert_eq!(date_at(app.top, 41, 10), Some(date(2026, 11, 15)));
        // 不是日期格子：框線、左邊欄、星期列、月份標記、最後一週下面
        for (column, row) in [(0, 0), (4, 5), (10, 2), (42, 5), (10, 11)] {
            assert_eq!(date_at(app.top, column, row), None, "({column}, {row})");
        }
    }

    #[test]
    fn month_markers_fit_their_column() {
        for texts in [Texts::en(), Texts::zh_tw()] {
            for month in 1..=12 {
                let marker = format!(" {}", (texts.month_marker)(month));
                assert!(
                    display_width(&marker) <= usize::from(MONTH_MARKER_WIDTH),
                    "{marker}"
                );
            }
        }
    }
}
