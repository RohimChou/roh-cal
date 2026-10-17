//! 左邊的月曆框：表頭（年月、週、星期）固定在上面，下面從這個月開始，一個月一大塊往下排

use crate::{
    app::App,
    calendar::{self, WeekRow},
    texts::Texts,
};
use jiff::{ToSpan, civil::Date};
use ratatui::{
    Frame,
    layout::{Constraint, Layout, Rect},
    symbols::line,
    text::Line,
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

/// 畫月曆框：最上面是今天那個月
pub fn render(frame: &mut Frame, area: Rect, app: &App) {
    let texts = app.lang.texts();
    let block = Block::bordered().border_type(BorderType::Rounded);
    let inner = block.inner(area);
    frame.render_widget(block, area);
    // 表頭（一列字 + 一條線）固定在上面，下面一列一列排月份
    let [header, rule, body] = Layout::vertical([
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Fill(1),
    ])
    .areas(inner);
    render_header(frame, header, texts);
    render_rule(frame, rule, false);
    let rows = rows(app.today, usize::from(body.height), texts);
    for (y, row) in (body.y..body.bottom()).zip(&rows) {
        let area = Rect::new(body.x, y, body.width, 1);
        match row {
            Row::Separator { year } => render_rule(frame, area, *year),
            Row::Week { label, days } => render_week(frame, area, label.as_deref(), days),
        }
    }
}

/// 表頭：「年月  週 │ 一 二 … 日」
fn render_header(frame: &mut Frame, area: Rect, texts: &Texts) {
    let weekdays: String = texts
        .weekdays
        .iter()
        .map(|name| format!(" {name}"))
        .collect();
    render_cells(
        frame,
        area,
        [
            Line::from(texts.header_month).right_aligned(),
            Line::from(texts.header_week).right_aligned(),
            Line::from(line::VERTICAL),
            Line::from(weekdays),
        ],
    );
}

/// 一週：左邊的年 / 月、週數、│、7 天（一天 3 格寬、日期靠右）
fn render_week(frame: &mut Frame, area: Rect, label: Option<&str>, days: &WeekRow) {
    // 週數：這一列任何一天都行（同一列一定是同一週）
    let week = days
        .iter()
        .flatten()
        .next()
        .map(|day| format!("{:>3} ", calendar::week_of_year(*day)))
        .unwrap_or_default();
    let days: String = days
        .iter()
        .map(|day| match day {
            Some(day) => format!("{:>3}", day.day()),
            None => "   ".to_string(),
        })
        .collect();
    render_cells(
        frame,
        area,
        [
            Line::from(label.unwrap_or_default()).right_aligned(),
            Line::from(week),
            Line::from(line::VERTICAL),
            Line::from(days),
        ],
    );
}

/// 橫線（`thick` = 跨年的粗線）；跟直線交會的地方畫十字
fn render_rule(frame: &mut Frame, area: Rect, thick: bool) {
    let (horizontal, cross) = if thick {
        (line::THICK_HORIZONTAL, "┿")
    } else {
        (line::HORIZONTAL, line::CROSS)
    };
    let columns = Layout::horizontal(COLUMNS).areas::<4>(area);
    for (i, column) in columns.into_iter().enumerate() {
        // 第 3 欄是直線那欄
        let symbol = if i == 2 { cross } else { horizontal };
        frame.render_widget(symbol.repeat(usize::from(column.width)), column);
    }
}

/// 一列的四欄各畫一段字
fn render_cells(frame: &mut Frame, area: Rect, cells: [Line; 4]) {
    let columns = Layout::horizontal(COLUMNS).areas::<4>(area);
    for (cell, column) in cells.into_iter().zip(columns) {
        frame.render_widget(cell, column);
    }
}
