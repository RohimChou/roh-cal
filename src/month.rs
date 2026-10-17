//! 版型 3：大月曆（照 v1 的 month view），填滿 side menu 右邊的空間，每天一個大格子，格子裡列當天的事
//!
//! 一次 6 週；選取的日期移出畫面才一週一週捲。格子大小每次重畫都從現在的空間算

use crate::{
    app::{App, Focus},
    calendar,
    items::Item,
    planner::{day_color, item_style},
    texts::Texts,
};
use jiff::{ToSpan, civil::Date};
use ratatui::{
    Frame,
    layout::{Constraint, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Paragraph},
};

/// 一次顯示幾週
const WEEKS: i64 = 6;
/// 左邊週數欄的寬度：數字 3 + 空白 2
const WEEK_NUMBER_WIDTH: u16 = 5;

/// 捲動：剛打開、按 `t`、換到這個版型時，選取那個月的 1 號那週放最上面；之後選取的日期移出 6 週才一週一週捲
pub fn scroll(app: &mut App) {
    if app.jump_to_month {
        app.month_top = calendar::monday_of(app.selected.first_of_month());
        app.jump_to_month = false;
    }
    let week = calendar::monday_of(app.selected);
    let last = app.month_top.saturating_add((WEEKS - 1).weeks());
    if week < app.month_top {
        app.month_top = week;
    } else if week > last {
        app.month_top = week.saturating_sub((WEEKS - 1).weeks());
    }
}

/// 畫大月曆：圓角框（鍵盤在月曆時框線亮起來），裡面是星期列 + 6 週，每週 7 個等寬等高的格子
pub fn render(frame: &mut Frame, area: Rect, app: &App, visible: &[&Item], texts: &Texts) {
    let theme = &app.theme;
    let month = app.selected.first_of_month();
    let color = if app.focus == Focus::Calendar {
        theme.focus
    } else {
        theme.line
    };
    let block = Block::bordered()
        .border_type(BorderType::Rounded)
        .border_style(Style::new().fg(color))
        .title_top(
            Line::styled(
                format!(" {} ", texts.month_title(month)),
                Style::new().fg(theme.label).add_modifier(Modifier::BOLD),
            )
            .centered(),
        );
    let inner = block.inner(area);
    frame.render_widget(block, area);
    let (header, weeks) = grid(inner);

    // 星期列：每個名稱放在自己那欄的最左邊，跟下面的日期對齊；六、日用週末的顏色
    for (i, (cell, name)) in header[1..].iter().zip(texts.weekdays).enumerate() {
        let color = if i >= 5 { theme.weekend } else { theme.muted };
        frame.render_widget(
            Paragraph::new(format!(" {name}")).style(Style::new().fg(color)),
            *cell,
        );
    }

    for (row, cells) in weeks.iter().enumerate() {
        let monday = app.month_top.saturating_add((row as i64).weeks());
        // 左邊欄：週數，跟日期同一行
        let number = Line::styled(
            format!("{:>3}", calendar::week_of_year(monday)),
            Style::new().fg(theme.week_number),
        );
        frame.render_widget(
            Paragraph::new(number),
            Rect {
                height: 1,
                ..cells[0]
            },
        );
        let days: Vec<Date> = monday.series(1.day()).take(7).collect();
        let lines: Vec<Vec<Line>> = days
            .iter()
            .map(|&day| {
                let day_items: Vec<&Item> = visible
                    .iter()
                    .copied()
                    .filter(|item| item.date == day)
                    .collect();
                cell_lines(day, month, &day_items, cells[1].height, app, texts)
            })
            .collect();
        render_week(frame, &cells[1..], &days, &lines, app);
    }
}

/// 切格子：回傳（星期列, 每週），每列是 8 格：週數欄 + 7 天
///
/// 每週的高度 = (框裡面的高度 − 星期列 1) ÷ 6，取整數；除不盡的零頭留在最底下當空白（高低才會一樣）
fn grid(inner: Rect) -> ([Rect; 8], Vec<[Rect; 8]>) {
    let week_height = inner.height.saturating_sub(1) / WEEKS as u16;
    let mut rows = vec![Constraint::Length(1)];
    rows.extend(std::iter::repeat_n(
        Constraint::Length(week_height),
        WEEKS as usize,
    ));
    rows.push(Constraint::Fill(1));
    let rows = Layout::vertical(rows).split(inner);
    let mut columns = vec![Constraint::Length(WEEK_NUMBER_WIDTH)];
    columns.extend(std::iter::repeat_n(Constraint::Fill(1), 7));
    let split = |row: Rect| -> [Rect; 8] { Layout::horizontal(columns.clone()).areas(row) };
    let weeks = (0..WEEKS as usize)
        .map(|row| split(rows[1 + row]))
        .collect();
    (split(rows[0]), weeks)
}

/// 畫一週的 7 格：選取的那格先塗底色，再一行一行畫文字
///
/// 文字太長時可以**往右延伸**到隔壁格（像試算表的溢出）：隔壁格同一行沒有東西就借用它的寬度，
/// 一路借到遇見有內容的格子、選取的格子、或這週最右邊為止；再長就截斷
fn render_week(frame: &mut Frame, cells: &[Rect], days: &[Date], lines: &[Vec<Line>], app: &App) {
    for (&cell, &day) in cells.iter().zip(days) {
        if day == app.selected {
            frame.render_widget(
                Paragraph::new("").style(Style::new().bg(app.theme.selected_item_bg)),
                cell,
            );
        }
    }
    for (column, (&cell, cell_lines)) in cells.iter().zip(lines).enumerate() {
        for (row, line) in cell_lines.iter().enumerate() {
            // 第一行是日期，不延伸；其他行往右看隔壁格這一行是不是空的
            let mut width = cell.width;
            if row > 0 {
                for next in column + 1..cells.len() {
                    let free = lines[next].len() <= row && days[next] != app.selected;
                    if !free {
                        break;
                    }
                    width = cells[next].right() - cell.x;
                }
            }
            let area = Rect {
                y: cell.y + row as u16,
                width,
                height: 1,
                ..cell
            };
            frame.render_widget(Paragraph::new(line.clone()), area);
        }
    }
}

/// 一格的內容：第一行日期（選取的加括號 `[17]`、1 號後面寫月份），下面一行一件事；放不下就少列一件、最後一行寫 `+N`
///
/// 不是這個月的日子：日期和事都變灰
fn cell_lines(
    day: Date,
    month: Date,
    items: &[&Item],
    rows: u16,
    app: &App,
    texts: &Texts,
) -> Vec<Line<'static>> {
    let theme = &app.theme;
    let other_month = day.first_of_month() != month;
    let color = if other_month {
        theme.muted
    } else {
        day_color(day, app)
    };
    let mut number = Style::new().fg(color);
    if !other_month {
        number = number.add_modifier(Modifier::BOLD);
    }
    let bracket = Style::new().fg(theme.selected).add_modifier(Modifier::BOLD);
    let (open, close) = if day == app.selected {
        (Span::styled("[", bracket), Span::styled("]", bracket))
    } else {
        (Span::raw(" "), Span::raw(" "))
    };
    let mut first = Line::from(vec![
        open,
        Span::styled(day.day().to_string(), number),
        close,
    ]);
    if day.day() == 1 {
        let label_color = if other_month {
            theme.muted
        } else {
            theme.label
        };
        first.push_span(Span::styled(
            texts.month_label(day),
            Style::new().fg(label_color),
        ));
    }
    let rows = usize::from(rows);
    if rows <= 1 {
        return vec![first];
    }
    let available = rows - 1;
    let shown = if items.len() <= available {
        items.len()
    } else {
        available - 1
    };
    let mut lines = vec![first];
    for item in &items[..shown] {
        let mut text = String::from(" ");
        if let Some(time) = item.time {
            text.push_str(&format!("{} ", time.strftime("%H:%M")));
        }
        text.push_str(&item.title);
        let style = if other_month {
            item_style(item, theme.muted, theme)
        } else {
            item_style(item, color, theme)
        };
        lines.push(Line::styled(text, style));
    }
    if shown < items.len() {
        lines.push(Line::styled(
            format!(" +{}", items.len() - shown),
            Style::new().fg(theme.muted),
        ));
    }
    lines
}
