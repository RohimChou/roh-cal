//! mock 1：行事曆風（參考北科的學期行事曆）
//!
//! 左邊月曆、右邊這個月的事項；一個月一大塊，跨月的那一週拆成兩半；跨年畫粗線

use crate::{
    app::App,
    calendar::{self, WeekRow},
    mock::Item,
    theme::Theme,
};
use jiff::{ToSpan, civil::Date};
use ratatui::{
    Frame,
    layout::{Constraint, Layout, Rect},
    style::{Modifier, Style},
    symbols::line,
    text::{Line, Span},
    widgets::{Block, BorderType, Paragraph},
};

/// 選取的那列離畫面上下邊緣至少留幾列，不夠才捲（像 vim 的 scrolloff）
const SCROLLOFF: usize = 2;
/// 星期一到星期日（表頭、標題用）
const WEEKDAYS: [&str; 7] = ["一", "二", "三", "四", "五", "六", "日"];

/// 畫面上的一列
enum Row<'a> {
    /// 月和月之間的線；`year` = 下一個月是 1 月（跨年），畫粗線
    Separator { year: bool },
    /// 一個月裡的一列：左邊的年 / 月、7 天、右邊的一件事，都可能沒有
    Week {
        label: Option<String>,
        days: Option<WeekRow>,
        item: Option<&'a Item>,
    },
}

/// 從 `first` 到 `last`（都是 1 號）每個月排成一列一列；一個月的高度 = 週數和事項數比較多的那個
fn rows<'a>(first: Date, last: Date, items: &'a [Item]) -> Vec<Row<'a>> {
    let mut rows = Vec::new();
    let mut month = first;
    while month <= last {
        if month != first {
            rows.push(Row::Separator {
                year: month.month() == 1,
            });
        }
        let weeks = calendar::month_rows(month);
        let month_items: Vec<&Item> = items
            .iter()
            .filter(|item| item.date.first_of_month() == month)
            .collect();
        // 1 月和最上面的第一個月先寫年份、下一列寫月份；其他月只寫月份
        let with_year = month.month() == 1 || month == first;
        for i in 0..weeks.len().max(month_items.len()) {
            let label = match (with_year, i) {
                (true, 0) => Some(format!("{}年", month.year())),
                (true, 1) | (false, 0) => Some(format!("{}月", month.month())),
                _ => None,
            };
            rows.push(Row::Week {
                label,
                days: weeks.get(i).copied(),
                item: month_items.get(i).copied(),
            });
        }
        month = month.saturating_add(1.month());
    }
    rows
}

/// 捲動後畫面最上面是第幾列：畫面有 `height` 列、現在最上面是 `top`、選取的是第 `selected` 列
///
/// 選取的那列離上下邊緣至少留 `margin` 列，不夠才捲；例：10 列、留 2 列 → 選取的只能在第 3～8 列
fn scrolled_top(top: usize, selected: usize, height: usize, margin: usize) -> usize {
    if height == 0 {
        return selected;
    }
    // 畫面太矮時，留的列數最多到一半
    let margin = margin.min((height - 1) / 2);
    if selected < top + margin {
        selected.saturating_sub(margin)
    } else if selected + margin >= top + height {
        selected + margin + 1 - height
    } else {
        top
    }
}

/// 畫整個畫面：外框（標題、操作說明）＋ 表頭 ＋ 捲動的月份
pub fn render(frame: &mut Frame, app: &mut App) {
    let theme = &app.theme;
    let selected = app.selected;
    let title = format!(
        " {}/{}/{}（{}） ",
        selected.year(),
        selected.month(),
        selected.day(),
        WEEKDAYS[selected.weekday().to_monday_zero_offset() as usize]
    );
    let block = Block::bordered()
        .border_type(BorderType::Rounded)
        .border_style(Style::new().fg(theme.line))
        .title_top(
            Line::from(" roh-cal v2 mock：行事曆 ")
                .style(Style::new().fg(theme.label).add_modifier(Modifier::BOLD)),
        )
        .title_top(
            Line::from(title)
                .style(Style::new().fg(theme.text))
                .right_aligned(),
        )
        .title_bottom(
            Line::from(" hjkl 移動  t 今天  q 離開 ").style(Style::new().fg(theme.muted)),
        );
    let inner = block.inner(frame.area());
    frame.render_widget(block, frame.area());

    // 表頭（一列字 + 一條線）固定在上面，下面的月份會捲動
    let [header_area, body_area] =
        Layout::vertical([Constraint::Length(2), Constraint::Fill(1)]).areas(inner);
    render_header(frame, header_area, theme);

    let rows = rows(app.first_month, app.last_month, &app.items);
    let selected_row = rows
        .iter()
        .position(|row| matches!(row, Row::Week { days: Some(days), .. } if days.contains(&Some(selected))))
        .unwrap_or(0);
    if app.jump_to_month {
        // 那個月的第一列：選取那列往上找到月和月之間的線為止
        app.scroll = rows[..selected_row]
            .iter()
            .rposition(|row| matches!(row, Row::Separator { .. }))
            .map_or(0, |separator| separator + 1);
        app.jump_to_month = false;
    }
    let height = usize::from(body_area.height);
    app.scroll = scrolled_top(app.scroll, selected_row, height, SCROLLOFF);
    let end = rows.len().min(app.scroll + height);
    render_body(frame, body_area, &rows[app.scroll..end], app);
}

/// 一列的六欄：年月、週數、│、7 天、│、事項
fn columns(area: Rect) -> [Rect; 6] {
    Layout::horizontal([
        Constraint::Length(6),
        Constraint::Length(4),
        Constraint::Length(1),
        Constraint::Length(22),
        Constraint::Length(1),
        Constraint::Fill(1),
    ])
    .areas(area)
}

/// 表頭：「年月  週 │ 一 二 … 日 │ 重要事項」，下面一條線
fn render_header(frame: &mut Frame, area: Rect, theme: &Theme) {
    let muted = Style::new().fg(theme.muted);
    let weekdays: Vec<Span> = WEEKDAYS
        .iter()
        .enumerate()
        .map(|(i, name)| {
            // 六、日用週末的顏色
            let style = if i >= 5 {
                Style::new().fg(theme.weekend)
            } else {
                muted
            };
            Span::styled(format!(" {name}"), style)
        })
        .collect();
    let line_style = Style::new().fg(theme.line);
    let cells: [Vec<Line>; 6] = [
        vec![Line::styled("年月", muted).right_aligned()],
        vec![Line::styled(" 週 ", muted).right_aligned()],
        vec![Line::styled(line::VERTICAL, line_style)],
        vec![Line::from(weekdays)],
        vec![Line::styled(line::VERTICAL, line_style)],
        vec![Line::styled(" 重要事項", muted)],
    ];
    let [labels, rule] = Layout::vertical([Constraint::Length(1); 2]).areas(area);
    for (cell, column) in cells.into_iter().zip(columns(labels)) {
        frame.render_widget(Paragraph::new(cell), column);
    }
    render_rule(frame, rule, false, theme);
}

/// 一條橫線（`thick` = 跨年的粗線），在兩條直線的地方畫十字
fn render_rule(frame: &mut Frame, area: Rect, thick: bool, theme: &Theme) {
    let (horizontal, cross, color) = if thick {
        (line::THICK_HORIZONTAL, "┿", theme.year_line)
    } else {
        (line::HORIZONTAL, line::CROSS, theme.line)
    };
    for (i, column) in columns(area).into_iter().enumerate() {
        let symbol = if i == 2 || i == 4 { cross } else { horizontal };
        let text = symbol.repeat(usize::from(column.width));
        frame.render_widget(Paragraph::new(text).style(Style::new().fg(color)), column);
    }
}

/// 捲動的部分：一列一列畫
fn render_body(frame: &mut Frame, area: Rect, rows: &[Row], app: &App) {
    let theme = &app.theme;
    let line_style = Style::new().fg(theme.line);
    for (y, row) in (area.y..area.bottom()).zip(rows) {
        let row_area = Rect::new(area.x, y, area.width, 1);
        let Row::Week { label, days, item } = row else {
            render_rule(
                frame,
                row_area,
                matches!(row, Row::Separator { year: true }),
                theme,
            );
            continue;
        };
        let label = label.as_deref().unwrap_or_default();
        // 週數：這一列任何一天都行（同一列一定是同一週）
        let week = days
            .and_then(|days| days.into_iter().flatten().next())
            .map(|day| format!("{:>3} ", calendar::week_of_year(day)))
            .unwrap_or_default();
        let cells = [
            Line::styled(label, Style::new().fg(theme.label)).right_aligned(),
            Line::styled(week, Style::new().fg(theme.week_number)),
            Line::styled(line::VERTICAL, line_style),
            days.map(|days| day_cells(&days, app)).unwrap_or_default(),
            Line::styled(line::VERTICAL, line_style),
            item.map(|item| item_line(item, app)).unwrap_or_default(),
        ];
        for (cell, column) in cells.into_iter().zip(columns(row_area)) {
            frame.render_widget(Paragraph::new(cell), column);
        }
    }
}

/// 7 天的日期，一天 3 格寬；顏色看是不是選取的、今天、假日、有事、週末
fn day_cells(days: &WeekRow, app: &App) -> Line<'static> {
    let theme = &app.theme;
    let spans: Vec<Span> = days
        .iter()
        .enumerate()
        .map(|(column, day)| {
            let Some(day) = *day else {
                return Span::raw("   ");
            };
            let items: Vec<&Item> = app.items.iter().filter(|item| item.date == day).collect();
            let mut style = Style::new().fg(theme.text);
            if column >= 5 {
                style = style.fg(theme.weekend);
            }
            if !items.is_empty() {
                style = style.fg(theme.has_item);
            }
            if items.iter().any(|item| item.holiday) {
                style = style.fg(theme.holiday);
            }
            if day == app.today {
                style = style.fg(theme.today).add_modifier(Modifier::BOLD);
            }
            if day == app.selected {
                style = style
                    .fg(theme.selected_fg)
                    .bg(theme.selected_bg)
                    .add_modifier(Modifier::BOLD);
            }
            Span::styled(format!("{:>3}", day.day()), style)
        })
        .collect();
    Line::from(spans)
}

/// 右邊的一件事：「10/10 國慶日」；選取那天的事整列換底色
fn item_line(item: &Item, app: &App) -> Line<'static> {
    let theme = &app.theme;
    let date = format!("{}/{}", item.date.month(), item.date.day());
    let title_color = if item.holiday {
        theme.holiday
    } else {
        theme.text
    };
    let line = Line::from(vec![
        Span::styled(format!(" {date:>5} "), Style::new().fg(theme.muted)),
        Span::styled(item.title, Style::new().fg(title_color)),
    ]);
    if item.date == app.selected {
        line.style(Style::new().bg(theme.selected_item_bg))
    } else {
        line
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use jiff::civil::date;

    /// 每列的種類，方便比對：`-` = 細線、`=` = 粗線、`w` = 有日期的列、`.` = 只有事項的列
    fn shape(rows: &[Row]) -> String {
        rows.iter()
            .map(|row| match row {
                Row::Separator { year: false } => '-',
                Row::Separator { year: true } => '=',
                Row::Week { days: Some(_), .. } => 'w',
                Row::Week { days: None, .. } => '.',
            })
            .collect()
    }

    #[test]
    fn rows_grow_with_items_and_mark_the_new_year() {
        let items = crate::mock::items();
        let rows = rows(date(2026, 10, 1), date(2027, 1, 1), &items);
        // 10 月 5 週但有 13 件事 → 13 列；11 月 6 週、12 月 5 週；跨年是粗線；1 月 5 週
        assert_eq!(shape(&rows), "wwwww........-wwwwww-wwwww=wwwww");
        // 年份：最上面的第一個月和 1 月先寫年份
        let labels: Vec<&str> = rows
            .iter()
            .filter_map(|row| match row {
                Row::Week {
                    label: Some(label), ..
                } => Some(label.as_str()),
                _ => None,
            })
            .collect();
        assert_eq!(labels, ["2026年", "10月", "11月", "12月", "2027年", "1月"]);
    }

    #[test]
    fn scrolled_top_keeps_the_margin() {
        // 10 列、留 2 列：選取的在第 3～8 列（index 2～7）就不捲
        assert_eq!(scrolled_top(0, 5, 10, 2), 0);
        assert_eq!(scrolled_top(0, 7, 10, 2), 0);
        // 往下超過：選取的那列停在 index 7
        assert_eq!(scrolled_top(0, 8, 10, 2), 1);
        assert_eq!(scrolled_top(0, 30, 10, 2), 23);
        // 往上超過：選取的那列停在 index 2；頂到最上面就是 0
        assert_eq!(scrolled_top(20, 21, 10, 2), 19);
        assert_eq!(scrolled_top(20, 1, 10, 2), 0);
        // 畫面只有 3 列：最多留 1 列
        assert_eq!(scrolled_top(0, 5, 3, 2), 4);
    }
}
