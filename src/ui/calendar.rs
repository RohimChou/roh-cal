//! 左邊的月曆框：表頭（年月、週、星期）固定在上面，下面從這個月開始，一個月一大塊往下排

use crate::{
    app::App,
    calendar::{self, WeekRow},
    texts::Texts,
    theme::Theme,
};
use jiff::{ToSpan, civil::Date};
use ratatui::{
    Frame,
    layout::{Constraint, Layout, Rect},
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

/// 從 `first` 那個月開始一個月一個月往下排，排到至少 `height` 列（最後一個月可能超出去）
fn rows(first: Date, height: usize, texts: &Texts) -> Vec<Row> {
    let mut rows = Vec::new();
    let mut month = first.first_of_month();
    while rows.len() < height {
        let top = rows.is_empty();
        if !top {
            rows.push(Row::Separator {
                year: month.month() == 1,
            });
        }
        // 最上面那個月和 1 月：第一列寫年份、第二列寫月份；其他月只在第一列寫月份
        let with_year = top || month.month() == 1;
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

/// 畫月曆框：最上面是選取那天的那個月
pub fn render(frame: &mut Frame, area: Rect, app: &App) {
    let texts = app.lang.texts();
    let theme = &app.theme;
    let block = Block::bordered()
        .border_type(BorderType::Rounded)
        .border_style(Style::new().fg(theme.line));
    let inner = block.inner(area);
    frame.render_widget(block, area);
    // 表頭（一列字 + 一條線）固定在上面，下面一列一列排月份
    let [header, rule, body] = Layout::vertical([
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Fill(1),
    ])
    .areas(inner);
    render_header(frame, header, texts, theme);
    render_rule(frame, rule, false, theme);
    let rows = rows(app.selected, usize::from(body.height), texts);
    for (y, row) in (body.y..body.bottom()).zip(&rows) {
        let area = Rect::new(body.x, y, body.width, 1);
        match row {
            Row::Separator { year } => render_rule(frame, area, *year, theme),
            Row::Week { label, days } => render_week(frame, area, label.as_deref(), days, app),
        }
    }
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
