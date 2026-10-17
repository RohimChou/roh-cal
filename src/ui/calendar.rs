//! 左邊的月曆框：表頭（年月、週、星期）固定在上面，下面一個月一大塊往下排，可以上下捲動
//!
//! 月曆的每一列有一個編號：今天那個月的第一週是第 0 列，往下 1、2、3…，往上 -1、-2…；
//! 一個月那一塊 = 每週一列 + 後面那條線

use crate::{
    app::App,
    calendar::{self, WeekRow},
    texts::Texts,
    theme::Theme,
};
use jiff::{ToSpan, civil::Date};
use ratatui::{
    Frame,
    layout::{Constraint, Layout, Position, Rect},
    style::{Color, Modifier, Style},
    symbols::line,
    text::{Line, Span},
    widgets::{Block, BorderType},
};

/// 框裡的四欄：年月、週數、│、7 天（一天 3 格，多留 1 格給之後選取日期的 `]`）
const COLUMNS: [Constraint; 4] = [
    Constraint::Length(6),
    Constraint::Length(4),
    Constraint::Length(1),
    Constraint::Length(22),
];
/// 框的寬度：四欄 + 左右框線
pub const WIDTH: u16 = 6 + 4 + 1 + 22 + 2;
/// 選取的那列離畫面上下邊緣至少留幾列，不夠才捲（像 vim 的 scrolloff）
const SCROLLOFF: i64 = 2;

/// 框裡的一列
#[derive(Debug, PartialEq)]
enum Row {
    /// 月和月之間的線；`year` = 下一個月是 1 月（跨年），畫粗線
    Separator { year: bool },
    /// 一個月裡的一週：左邊的年 / 月（可能沒有）、這一週屬於這個月的日子
    Week {
        label: Option<String>,
        days: WeekRow,
    },
}

/// 從 `first` 那個月開始一個月一個月往下排，排到至少 `skip + height` 列（最後一個月可能超出去）
///
/// 前 `skip` 列是捲到畫面上面、看不到的
fn rows(first: Date, skip: usize, height: usize, texts: &Texts) -> Vec<Row> {
    let mut rows = Vec::new();
    let mut month = first.first_of_month();
    // 畫面上已經有月份寫了年份
    let mut year_shown = false;
    while rows.len() < skip + height {
        if !rows.is_empty() {
            rows.push(Row::Separator {
                year: month.month() == 1,
            });
        }
        // 畫面上第一個從頭開始的月份和 1 月：第一列寫年份、第二列寫月份；其他月只在第一列寫月份
        let on_screen = rows.len() >= skip;
        let with_year = (on_screen && !year_shown) || month.month() == 1;
        year_shown |= on_screen;
        for (i, days) in calendar::month_rows(month).into_iter().enumerate() {
            let label = match (with_year, i) {
                (true, 0) => Some(texts.year_label(month)),
                (true, 1) | (false, 0) => Some(texts.month_label(month)),
                _ => None,
            };
            rows.push(Row::Week { label, days });
        }
        month = month.saturating_add(1.month());
    }
    rows
}

/// 一個月那一塊有幾列：每週一列，加上後面那條線
fn block_height(month: Date) -> i64 {
    i64::try_from(calendar::month_rows(month).len()).unwrap_or(0) + 1
}

/// `month` 那一塊的第一列是第幾列；`origin`（今天那個月）的第一列是第 0 列
fn block_start(origin: Date, month: Date) -> i64 {
    let mut start = 0;
    let mut current = origin.first_of_month();
    let month = month.first_of_month();
    // 往後：一路加上前面每個月的高度；往前：一路減掉
    while current < month {
        start += block_height(current);
        current = current.saturating_add(1.month());
    }
    while current > month {
        current = current.saturating_sub(1.month());
        start -= block_height(current);
    }
    start
}

/// `date` 那一週是第幾列
fn row_of(origin: Date, date: Date) -> i64 {
    // 這一週是那個月的第幾週（從 0 開始）
    let week = calendar::month_rows(date)
        .iter()
        .position(|row| row.contains(&Some(date)))
        .unwrap_or(0);
    block_start(origin, date) + i64::try_from(week).unwrap_or(0)
}

/// 第 `row` 列落在哪個月那一塊：回傳那個月（1 號）和那一塊的第一列是第幾列
fn block_at(origin: Date, row: i64) -> (Date, i64) {
    let mut month = origin.first_of_month();
    let mut start = 0;
    while row < start {
        month = month.saturating_sub(1.month());
        start -= block_height(month);
    }
    while row >= start + block_height(month) {
        start += block_height(month);
        month = month.saturating_add(1.month());
    }
    (month, start)
}

/// 捲動後畫面最上面是第幾列：畫面有 `height` 列、現在最上面是 `top`、選取的是第 `selected` 列
///
/// 選取的那列離上下邊緣至少留 `margin` 列，不夠才捲；例：10 列、留 2 列 → 選取的只能在畫面的第 3～8 列
fn scrolled_top(top: i64, selected: i64, height: i64, margin: i64) -> i64 {
    if height <= 0 {
        return selected;
    }
    // 畫面太矮時，留的列數最多到一半
    let margin = margin.min((height - 1) / 2);
    if selected < top + margin {
        selected - margin
    } else if selected + margin >= top + height {
        selected + margin + 1 - height
    } else {
        top
    }
}

/// 畫月曆框：捲到選取那天離上下邊緣至少留 `SCROLLOFF` 列，捲完的位置記在 `app.calendar_top`
pub fn render(frame: &mut Frame, area: Rect, app: &mut App) {
    let texts = app.lang.texts();
    let block = Block::bordered()
        .border_type(BorderType::Rounded)
        .border_style(Style::new().fg(app.theme.line));
    frame.render_widget(block, area);
    let [header, rule, body] = inner_areas(area);
    // 捲動：還沒捲過（剛打開、按了 `t`）就先把選取那天的那個月放在最上面
    // 要改 `app`，所以放在拿 `&app.theme` 之前（Rust 不准一邊讀一邊改）
    let origin = app.today;
    let top = app
        .calendar_top
        .unwrap_or_else(|| block_start(origin, app.selected));
    let top = scrolled_top(
        top,
        row_of(origin, app.selected),
        i64::from(body.height),
        SCROLLOFF,
    );
    app.calendar_top = Some(top);
    let theme = &app.theme;
    render_header(frame, header, texts, theme);
    render_rule(frame, rule, false, theme);
    // 從最上面那列所在的那個月開始排，跳過那個月在畫面上面的列
    let (month, start) = block_at(origin, top);
    let skip = usize::try_from(top - start).unwrap_or(0);
    let rows = rows(month, skip, usize::from(body.height), texts);
    for (y, row) in (body.y..body.bottom()).zip(rows.iter().skip(skip)) {
        let area = Rect::new(body.x, y, body.width, 1);
        match row {
            Row::Separator { year } => render_rule(frame, area, *year, theme),
            Row::Week { label, days } => render_week(frame, area, label.as_deref(), days, app),
        }
    }
}

/// 框裡面怎麼切：表頭（一列字 + 一條線）固定在上面，下面一列一列排月份；畫畫面和滑鼠換算位置都用這個
fn inner_areas(area: Rect) -> [Rect; 3] {
    Layout::vertical([
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Fill(1),
    ])
    .areas(Block::bordered().inner(area))
}

/// 滑鼠點在 terminal 的 (`column`, `row`)，點到月曆框（畫在 `area`）的哪一天；沒點在日期上回傳 `None`
///
/// 捲動的位置用上一次畫的時候記下的 `app.calendar_top`；一天佔 3 格（日期前面的空白也算那天）
pub fn date_at(area: Rect, app: &App, column: u16, row: u16) -> Option<Date> {
    let [_, _, body] = inner_areas(area);
    if !body.contains(Position::new(column, row)) {
        return None;
    }
    // 點的是第幾列 → 哪個月那一塊的第幾列（月和月之間的線那列沒有日期）
    let row = app.calendar_top? + i64::from(row - body.y);
    let (month, start) = block_at(app.today, row);
    let days = calendar::month_rows(month)
        .get(usize::try_from(row - start).ok()?)
        .copied()?;
    // 第幾天：7 天那欄的第幾個 3 格；最後多留的那格是第 8 個，沒有日期
    let [_, _, _, days_area] = Layout::horizontal(COLUMNS).areas(body);
    let weekday = usize::from(column.checked_sub(days_area.x)? / 3);
    days.get(weekday).copied().flatten()
}

/// 表頭：「年月  週 │ 一 二 … 日」；六、日用週末的顏色
fn render_header(frame: &mut Frame, area: Rect, texts: &Texts, theme: &Theme) {
    let muted = Style::new().fg(theme.muted);
    let weekdays: Vec<Span> = texts
        .weekdays
        .iter()
        .enumerate()
        .map(|(i, name)| {
            // 第 6、7 個是星期六、星期日
            let style = if i >= 5 {
                Style::new().fg(theme.weekend)
            } else {
                muted
            };
            Span::styled(format!(" {name}"), style)
        })
        .collect();
    render_cells(
        frame,
        area,
        [
            Line::styled(texts.header_month, muted).right_aligned(),
            Line::styled(texts.header_week, muted).right_aligned(),
            Line::styled(line::VERTICAL, Style::new().fg(theme.line)),
            Line::from(weekdays),
        ],
    );
}

/// 一週：左邊的年 / 月、週數、│、7 天（一天 3 格寬、日期靠右）
fn render_week(frame: &mut Frame, area: Rect, label: Option<&str>, days: &WeekRow, app: &App) {
    let theme = &app.theme;
    // 週數：這一列任何一天都行（同一列一定是同一週）
    let week = days
        .iter()
        .flatten()
        .next()
        .map(|day| format!("{:>3} ", calendar::week_of_year(*day)))
        .unwrap_or_default();
    render_cells(
        frame,
        area,
        [
            Line::styled(label.unwrap_or_default(), Style::new().fg(theme.label)).right_aligned(),
            Line::styled(week, Style::new().fg(theme.week_number)),
            Line::styled(line::VERTICAL, Style::new().fg(theme.line)),
            day_cells(days, app),
        ],
    );
}

/// 7 天的日期，一天 3 格寬、日期靠右；選取的那天前後加括號 `[17]`
///
/// 日期最多 2 位數，前面一定有空白：`[` 佔掉自己前面的空白、`]` 佔掉下一天前面的空白（星期日就佔最後多留的那格）
fn day_cells(days: &WeekRow, app: &App) -> Line<'static> {
    let bracket = Style::new()
        .fg(app.theme.selected)
        .add_modifier(Modifier::BOLD);
    let mut spans = Vec::new();
    // 上一天是選取的：這一格前面的空白先放 `]`
    let mut close = false;
    for day in days {
        let number = day.map(|day| day.day().to_string()).unwrap_or_default();
        let selected = *day == Some(app.selected);
        // 日期前面要補幾格空白，例：`17` 補 1 格、`5` 補 2 格；括號佔掉的格子要扣掉
        let mut gap = 3 - number.len();
        if close {
            spans.push(Span::styled("]", bracket));
            gap -= 1;
        }
        if selected {
            gap -= 1;
        }
        spans.push(Span::raw(" ".repeat(gap)));
        if selected {
            spans.push(Span::styled("[", bracket));
        }
        if let Some(day) = day {
            // 選取的和今天的日期加粗
            let mut style = Style::new().fg(day_color(*day, app));
            if selected || *day == app.today {
                style = style.add_modifier(Modifier::BOLD);
            }
            spans.push(Span::styled(number, style));
        }
        close = selected;
    }
    if close {
        spans.push(Span::styled("]", bracket));
    }
    Line::from(spans)
}

/// 一天的日期顏色：今天 > 週末 > 一般
fn day_color(day: Date, app: &App) -> Color {
    let theme = &app.theme;
    if day == app.today {
        theme.today
    } else if day.weekday().to_monday_zero_offset() >= 5 {
        theme.weekend
    } else {
        theme.text
    }
}

/// 橫線（`thick` = 跨年的粗線）；跟直線交會的地方畫十字
fn render_rule(frame: &mut Frame, area: Rect, thick: bool, theme: &Theme) {
    let (horizontal, cross, color) = if thick {
        (line::THICK_HORIZONTAL, "┿", theme.year_line)
    } else {
        (line::HORIZONTAL, line::CROSS, theme.line)
    };
    let style = Style::new().fg(color);
    let columns = Layout::horizontal(COLUMNS).areas::<4>(area);
    for (i, column) in columns.into_iter().enumerate() {
        // 第 3 欄是直線那欄
        let symbol = if i == 2 { cross } else { horizontal };
        let text = symbol.repeat(usize::from(column.width));
        frame.render_widget(Line::styled(text, style), column);
    }
}

/// 一列的四欄各畫一段字
fn render_cells(frame: &mut Frame, area: Rect, cells: [Line; 4]) {
    let columns = Layout::horizontal(COLUMNS).areas::<4>(area);
    for (cell, column) in cells.into_iter().zip(columns) {
        frame.render_widget(cell, column);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use jiff::civil::date;

    #[test]
    fn row_numbers_count_from_this_month() {
        // 今天是 2026/10：10 月第一列是第 0 列；每個月那一塊 = 週數 + 1 條線
        let origin = date(2026, 10, 17);
        assert_eq!(block_start(origin, date(2026, 10, 1)), 0);
        assert_eq!(block_start(origin, date(2026, 11, 20)), 6);
        assert_eq!(block_start(origin, date(2027, 1, 1)), 19);
        assert_eq!(block_start(origin, date(2027, 2, 1)), 25);
        assert_eq!(block_start(origin, date(2026, 9, 1)), -6);
        assert_eq!(block_start(origin, date(2026, 8, 1)), -13);
        assert_eq!(row_of(origin, date(2026, 10, 17)), 2);
        assert_eq!(row_of(origin, date(2026, 11, 7)), 7);
        assert_eq!(row_of(origin, date(2027, 1, 3)), 19);
        assert_eq!(row_of(origin, date(2026, 9, 26)), -3);
        // 每一塊最後一列是線：10 月的線是第 5 列、9 月的線是第 -1 列
        assert_eq!(block_at(origin, 5), (date(2026, 10, 1), 0));
        assert_eq!(block_at(origin, 6), (date(2026, 11, 1), 6));
        assert_eq!(block_at(origin, -1), (date(2026, 9, 1), -6));
        assert_eq!(block_at(origin, -6), (date(2026, 9, 1), -6));
        assert_eq!(block_at(origin, -7), (date(2026, 8, 1), -13));
    }

    #[test]
    fn block_at_finds_the_month_of_every_row() {
        // 用另一個方向核對：每一天的列，找回來都是它自己那個月；前後 5 年（每 3 天一筆，星期幾都會輪到）
        let origin = date(2026, 10, 17);
        let mut day = date(2021, 1, 1);
        while day < date(2032, 1, 1) {
            let (month, start) = block_at(origin, row_of(origin, day));
            assert_eq!(month, day.first_of_month(), "{day}");
            assert_eq!(start, block_start(origin, day), "{day}");
            day = day.saturating_add(3.days());
        }
    }

    #[test]
    fn scrolled_top_keeps_the_margin() {
        // 10 列、留 2 列：選取的在畫面的第 3～8 列（index 2～7）就不捲
        assert_eq!(scrolled_top(0, 5, 10, 2), 0);
        assert_eq!(scrolled_top(0, 2, 10, 2), 0);
        assert_eq!(scrolled_top(0, 7, 10, 2), 0);
        // 往下超過：選取的那列停在 index 7
        assert_eq!(scrolled_top(0, 8, 10, 2), 1);
        assert_eq!(scrolled_top(0, 30, 10, 2), 23);
        // 往上超過：選取的那列停在 index 2；可以捲到負的列（以前的月）
        assert_eq!(scrolled_top(20, 21, 10, 2), 19);
        assert_eq!(scrolled_top(0, 1, 10, 2), -1);
        assert_eq!(scrolled_top(0, -40, 10, 2), -42);
        // 畫面只有 3 列：最多留 1 列
        assert_eq!(scrolled_top(0, 5, 3, 2), 4);
    }
}
