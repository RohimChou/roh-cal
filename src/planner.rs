//! mock 1：行事曆風（參考北科的學期行事曆）
//!
//! 左邊月曆、右邊這個月的事項；一個月一大塊，跨月的那一週拆成兩半；跨年畫粗線

use crate::{
    app::{App, Focus, Variant, is_shown},
    calendar::{self, WeekRow},
    mock::Item,
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
    widgets::{Block, BorderType, Paragraph},
};

/// 選取的那列離畫面上下邊緣至少留幾列，不夠才捲（像 vim 的 scrolloff）
const SCROLLOFF: usize = 2;

/// 畫面上的一列
enum Row<'a> {
    /// 月和月之間的線；`year` = 下一個月是 1 月（跨年），畫粗線
    Separator { year: bool },
    /// 一個月裡的一列：左邊的年 / 月、7 天、右邊每一欄的一件事，都可能沒有
    Week {
        label: Option<String>,
        days: Option<WeekRow>,
        items: Vec<Option<&'a Item>>,
    },
}

/// 從 `first` 到 `last`（都是 1 號）每個月排成一列一列
///
/// 右邊這個月的事分成 `columns` 欄排（像報紙：先排滿第一欄再排第二欄）；一個月的高度 = 週數和每欄件數比較多的那個
fn rows<'a>(
    first: Date,
    last: Date,
    items: &[&'a Item],
    columns: usize,
    texts: &Texts,
) -> Vec<Row<'a>> {
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
            .copied()
            .filter(|item| item.date.first_of_month() == month)
            .collect();
        // 1 月和最上面的第一個月先寫年份、下一列寫月份；其他月只寫月份
        let with_year = month.month() == 1 || month == first;
        let height = weeks.len().max(month_items.len().div_ceil(columns));
        for i in 0..height {
            let label = match (with_year, i) {
                (true, 0) => Some(texts.year_label(month)),
                (true, 1) | (false, 0) => Some(texts.month_label(month)),
                _ => None,
            };
            rows.push(Row::Week {
                label,
                days: weeks.get(i).copied(),
                // 第 c 欄的第 i 列 = 這個月的第 c × 高度 + i 件事
                items: (0..columns)
                    .map(|c| month_items.get(c * height + i).copied())
                    .collect(),
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

/// 左邊月曆框裡的四欄：年月、週數、│、7 天（多留 1 格給星期日的 `]`）
const CALENDAR_COLUMNS: [Constraint; 4] = [
    Constraint::Length(6),
    Constraint::Length(4),
    Constraint::Length(1),
    Constraint::Length(22),
];
/// 左邊月曆框的寬度：四欄 + 左右框線
const CALENDAR_WIDTH: u16 = 6 + 4 + 1 + 22 + 2;
/// 右邊的事分欄時，每一欄至少幾格寬
const AGENDA_COLUMN_WIDTH: usize = 28;

/// 右邊 `width` 格寬要分幾欄：每欄至少 `AGENDA_COLUMN_WIDTH` 格、最多 4 欄
fn agenda_columns(width: u16) -> usize {
    (usize::from(width) / AGENDA_COLUMN_WIDTH).clamp(1, 4)
}

/// 把右邊的一列（或整個框）切成 `count` 欄，欄和欄之間空 2 格
fn split_columns(area: Rect, count: usize) -> std::rc::Rc<[Rect]> {
    Layout::horizontal(vec![Constraint::Fill(1); count])
        .spacing(2)
        .split(area)
}
/// side menu 的寬度（含框線）
const SIDE_WIDTH: u16 = 18;
/// terminal 至少這麼寬才畫 side menu
const SIDE_MIN_WIDTH: u16 = 120;

/// 畫整個畫面：最上面一列標題、中間左右兩個框（月曆、事項）、最下面一列操作說明
///
/// 鍵盤在哪一邊，那個框的框線就亮起來
pub fn render(frame: &mut Frame, app: &mut App) {
    // side menu：夠寬、沒按 `s` 收起才畫；沒畫的時候鍵盤不能停在它上面
    app.side_shown = frame.area().width >= SIDE_MIN_WIDTH && !app.side_hidden;
    if !app.side_shown && app.focus == Focus::Side {
        app.focus = Focus::Calendar;
    }
    let theme = &app.theme;
    let texts = app.lang.texts();
    let selected = app.selected;
    let [title_area, panes_area, hints_area] = Layout::vertical([
        Constraint::Length(1),
        Constraint::Fill(1),
        Constraint::Length(1),
    ])
    .areas(frame.area());

    // 標題：左邊 app 的名字、右邊選取的日期；最下面是操作說明
    let title = Line::styled(
        texts.title,
        Style::new().fg(theme.label).add_modifier(Modifier::BOLD),
    );
    let date = Line::styled(
        format!("{} ", texts.date_title(selected)),
        Style::new().fg(theme.text),
    );
    frame.render_widget(Paragraph::new(title), title_area);
    frame.render_widget(Paragraph::new(date.right_aligned()), title_area);
    frame.render_widget(
        Paragraph::new(Line::styled(texts.hints, Style::new().fg(theme.muted))),
        hints_area,
    );

    // 左右兩個框（夠寬時最左邊再多一個 side menu）：鍵盤在的那個框線用 `focus` 的顏色
    let side_width = if app.side_shown { SIDE_WIDTH } else { 0 };
    let [side_area, calendar_area, agenda_area] = Layout::horizontal([
        Constraint::Length(side_width),
        Constraint::Length(CALENDAR_WIDTH),
        Constraint::Fill(1),
    ])
    .areas(panes_area);
    let pane = |focused: bool| {
        let color = if focused { theme.focus } else { theme.line };
        Block::bordered()
            .border_type(BorderType::Rounded)
            .border_style(Style::new().fg(color))
    };
    let calendar_block = pane(app.focus == Focus::Calendar);
    let agenda_block = pane(app.focus == Focus::Agenda);
    let calendar_inner = calendar_block.inner(calendar_area);
    let agenda_inner = agenda_block.inner(agenda_area);
    frame.render_widget(calendar_block, calendar_area);
    frame.render_widget(agenda_block, agenda_area);

    // 兩個框都是：表頭（一列字 + 一條線）固定在上面，下面的月份會捲動；兩邊的每一列對齊
    let split = |area: Rect| -> [Rect; 2] {
        Layout::vertical([Constraint::Length(2), Constraint::Fill(1)]).areas(area)
    };
    let [calendar_header, calendar_body] = split(calendar_inner);
    let [agenda_header, agenda_body] = split(agenda_inner);
    // 版型 1：右邊的事跟月份對齊，表頭和月份之間的線畫過兩個框
    // 版型 2：右邊另外畫（`render_upcoming`），表頭和線只畫在月曆框裡（右邊給寬度 0）
    let month_items = app.variant == Variant::MonthItems;
    let rows_agenda = |area: Rect| {
        if month_items {
            area
        } else {
            Rect { width: 0, ..area }
        }
    };
    render_header(
        frame,
        calendar_header,
        rows_agenda(agenda_header),
        theme,
        texts,
    );
    if app.side_shown {
        render_side(frame, side_area, app, texts);
    }

    // 只排有勾選的日曆的事
    let visible: Vec<&Item> = app
        .items
        .iter()
        .filter(|item| is_shown(&app.groups, item))
        .collect();
    // 版型 2 的事不排在月份旁邊，月份只有週數那麼高
    let row_items: &[&Item] = if month_items { &visible } else { &[] };
    let item_columns = agenda_columns(agenda_body.width);
    let rows = rows(
        app.first_month,
        app.last_month,
        row_items,
        item_columns,
        texts,
    );
    // 鍵盤在右邊時選到的那件事
    let chosen = match app.focus {
        Focus::Agenda => visible.get(app.selected_item).copied(),
        Focus::Calendar | Focus::Side => None,
    };
    // 選取的那列：鍵盤在右邊、而且那件事排在月份旁邊（版型 1），就是那件事那列；其他時候是選取那天那列
    let selected_row = chosen
        .and_then(|chosen| {
            rows.iter().position(|row| {
                matches!(row, Row::Week { items, .. } if items.iter().flatten().any(|item| std::ptr::eq(*item, chosen)))
            })
        })
        .or_else(|| {
            rows.iter().position(|row| {
                matches!(row, Row::Week { days: Some(days), .. } if days.contains(&Some(selected)))
            })
        })
        .unwrap_or(0);
    if app.jump_to_month {
        // 那個月的第一列：選取那列往上找到月和月之間的線為止
        app.scroll = rows[..selected_row]
            .iter()
            .rposition(|row| matches!(row, Row::Separator { .. }))
            .map_or(0, |separator| separator + 1);
        app.jump_to_month = false;
    }
    let height = usize::from(calendar_body.height);
    app.scroll = scrolled_top(app.scroll, selected_row, height, SCROLLOFF);
    let end = rows.len().min(app.scroll + height);
    render_body(
        frame,
        calendar_body,
        rows_agenda(agenda_body),
        &rows[app.scroll..end],
        app,
        chosen,
    );
    if !month_items {
        render_upcoming(frame, agenda_inner, app, &visible, chosen, texts);
    }
}

/// 表頭：左邊「年月  週 │ 一 二 … 日」、右邊「重要事項」，下面各一條線
fn render_header(frame: &mut Frame, calendar: Rect, agenda: Rect, theme: &Theme, texts: &Texts) {
    let muted = Style::new().fg(theme.muted);
    let weekdays: Vec<Span> = texts
        .weekdays
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
    let cells = [
        Line::styled(texts.header_month, muted).right_aligned(),
        Line::styled(texts.header_week, muted).right_aligned(),
        Line::styled(line::VERTICAL, Style::new().fg(theme.line)),
        Line::from(weekdays),
    ];
    let [calendar_labels, calendar_rule] =
        Layout::vertical([Constraint::Length(1); 2]).areas(calendar);
    let [agenda_labels, agenda_rule] = Layout::vertical([Constraint::Length(1); 2]).areas(agenda);
    for (cell, column) in cells
        .into_iter()
        .zip(Layout::horizontal(CALENDAR_COLUMNS).areas::<4>(calendar_labels))
    {
        frame.render_widget(Paragraph::new(cell), column);
    }
    frame.render_widget(
        Paragraph::new(Line::styled(texts.header_items, muted)),
        agenda_labels,
    );
    render_rule(frame, calendar_rule, agenda_rule, false, theme);
}

/// 左右兩個框裡的同一條橫線（`thick` = 跨年的粗線）；月曆框裡跟直線交會的地方畫十字
fn render_rule(frame: &mut Frame, calendar: Rect, agenda: Rect, thick: bool, theme: &Theme) {
    let (horizontal, cross, color) = if thick {
        (line::THICK_HORIZONTAL, "┿", theme.year_line)
    } else {
        (line::HORIZONTAL, line::CROSS, theme.line)
    };
    let style = Style::new().fg(color);
    let columns = Layout::horizontal(CALENDAR_COLUMNS).areas::<4>(calendar);
    for (i, column) in columns.into_iter().enumerate() {
        let symbol = if i == 2 { cross } else { horizontal };
        let text = symbol.repeat(usize::from(column.width));
        frame.render_widget(Paragraph::new(text).style(style), column);
    }
    let text = horizontal.repeat(usize::from(agenda.width));
    frame.render_widget(Paragraph::new(text).style(style), agenda);
}

/// 捲動的部分：一列一列畫，左邊月曆框和右邊事項框的同一列在同一個高度
fn render_body(
    frame: &mut Frame,
    calendar: Rect,
    agenda: Rect,
    rows: &[Row],
    app: &App,
    chosen: Option<&Item>,
) {
    let theme = &app.theme;
    for (y, row) in (calendar.y..calendar.bottom()).zip(rows) {
        let calendar_row = Rect::new(calendar.x, y, calendar.width, 1);
        let agenda_row = Rect::new(agenda.x, y, agenda.width, 1);
        let Row::Week { label, days, items } = row else {
            let thick = matches!(row, Row::Separator { year: true });
            render_rule(frame, calendar_row, agenda_row, thick, theme);
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
            Line::styled(line::VERTICAL, Style::new().fg(theme.line)),
            days.map(|days| day_cells(&days, app)).unwrap_or_default(),
        ];
        let columns = Layout::horizontal(CALENDAR_COLUMNS).areas::<4>(calendar_row);
        for (cell, column) in cells.into_iter().zip(columns) {
            frame.render_widget(Paragraph::new(cell), column);
        }
        // 右邊每一欄的事
        for (item, column) in items
            .iter()
            .zip(split_columns(agenda_row, items.len()).iter())
        {
            if let Some(item) = item {
                frame.render_widget(Paragraph::new(item_line(item, app, chosen)), *column);
            }
        }
    }
}

/// side menu：表頭「日曆」，下面每個日曆一列「☑ 工作 12」；沒勾的日曆，它的事不顯示
fn render_side(frame: &mut Frame, area: Rect, app: &App, texts: &Texts) {
    let theme = &app.theme;
    let focused = app.focus == Focus::Side;
    let block = Block::bordered()
        .border_type(BorderType::Rounded)
        .border_style(Style::new().fg(if focused { theme.focus } else { theme.line }));
    let inner = block.inner(area);
    frame.render_widget(block, area);
    // 表頭跟另外兩個框一樣：一列字 + 一條線
    let [header, rule, list] = Layout::vertical([
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Fill(1),
    ])
    .areas(inner);
    frame.render_widget(
        Paragraph::new(Line::styled(
            texts.header_side,
            Style::new().fg(theme.muted),
        )),
        header,
    );
    frame.render_widget(
        Paragraph::new(line::HORIZONTAL.repeat(usize::from(rule.width)))
            .style(Style::new().fg(theme.line)),
        rule,
    );
    let lines: Vec<Line> = app
        .groups
        .iter()
        .enumerate()
        .map(|(i, group)| {
            let count = app
                .items
                .iter()
                .filter(|item| item.group == group.name)
                .count();
            // 勾選的：紫色的 ☑、白字；沒勾的：灰色的 ☐、灰字
            let (mark, mark_color, name_color) = if group.shown {
                ("☑", theme.selected, theme.text)
            } else {
                ("☐", theme.muted, theme.muted)
            };
            let line = Line::from(vec![
                Span::styled(format!(" {mark} "), Style::new().fg(mark_color)),
                Span::styled(group.name, Style::new().fg(name_color)),
                Span::styled(format!(" {count}"), Style::new().fg(theme.muted)),
            ]);
            // 鍵盤在 side menu 時，選到的那列換底色
            if focused && i == app.selected_group {
                line.style(Style::new().bg(theme.selected_item_bg))
            } else {
                line
            }
        })
        .collect();
    frame.render_widget(Paragraph::new(lines), list);
}

/// 版型 2 的右邊：從選取那天開始，一天一段（標題 + 那天的事），框放得下幾天就列幾天
///
/// 選取那天沒有事也寫出來（「沒有事」）；之後只列有事的日子
fn render_upcoming(
    frame: &mut Frame,
    area: Rect,
    app: &App,
    visible: &[&Item],
    chosen: Option<&Item>,
    texts: &Texts,
) {
    let theme = &app.theme;
    let muted = Style::new().fg(theme.muted);
    // 表頭跟另外兩個框一樣：一列字 + 一條線
    let [header, rule, list] = Layout::vertical([
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Fill(1),
    ])
    .areas(area);
    frame.render_widget(
        Paragraph::new(Line::styled(texts.header_upcoming, muted)),
        header,
    );
    frame.render_widget(
        Paragraph::new(line::HORIZONTAL.repeat(usize::from(rule.width)))
            .style(Style::new().fg(theme.line)),
        rule,
    );

    // 要列的日子：選取那天，加上之後有事的日子（事項照日期排好）
    let mut days = vec![app.selected];
    for item in visible {
        if item.date > app.selected && days.last() != Some(&item.date) {
            days.push(item.date);
        }
    }
    // 每天一段：標題 + 那天的事
    let blocks = days.into_iter().map(|day| {
        // 那天的標題，顏色跟月曆上那天一樣
        let color = day_color(day, app);
        let mut heading = vec![Span::styled(
            format!(" {}", texts.day_heading(day)),
            Style::new().fg(color).add_modifier(Modifier::BOLD),
        )];
        if day == app.today {
            heading.push(Span::styled(texts.today, muted));
        }
        let mut lines = vec![Line::from(heading)];
        let day_items: Vec<&Item> = visible
            .iter()
            .copied()
            .filter(|item| item.date == day)
            .collect();
        if day_items.is_empty() {
            lines.push(Line::styled(format!("   {}", texts.no_items), muted));
        }
        for item in day_items {
            let mut spans = vec![Span::raw("   ")];
            if let Some(time) = item.time {
                spans.push(Span::styled(format!("{time} "), muted));
            }
            spans.push(Span::styled(item.title, item_style(item, color, theme)));
            let line = Line::from(spans);
            // 鍵盤在右邊時，選到的那件事換底色
            let is_chosen = chosen.is_some_and(|chosen| std::ptr::eq(item, chosen));
            lines.push(if is_chosen {
                line.style(Style::new().bg(theme.selected_item_bg))
            } else {
                line
            });
        }
        lines
    });

    // 像報紙一樣分欄：一段一段往下放，放不下就換下一欄（同一天不拆開）
    let columns = split_columns(list, agenda_columns(list.width));
    let height = usize::from(list.height);
    let mut blocks = blocks.peekable();
    for column in columns.iter() {
        let mut lines: Vec<Line> = Vec::new();
        // 這欄還是空的就一定放（太長就被切掉）；不然要放得下「空一列 + 這一段」
        while let Some(block) =
            blocks.next_if(|block| lines.is_empty() || lines.len() + 1 + block.len() <= height)
        {
            if !lines.is_empty() {
                lines.push(Line::default());
            }
            lines.extend(block);
        }
        frame.render_widget(Paragraph::new(lines), *column);
    }
}

/// 一件事的標題樣式：做完的變灰、劃掉；沒做完的用那天的顏色
fn item_style(item: &Item, color: Color, theme: &Theme) -> Style {
    if item.done {
        Style::new()
            .fg(theme.muted)
            .add_modifier(Modifier::CROSSED_OUT)
    } else {
        Style::new().fg(color)
    }
}

/// 一天的日期顏色：今天 > 假日 > 有沒做完的事 > 週末 > 一般（事都做完了就跟沒事一樣）
///
/// 月曆的日期和右邊事項前面的日期都用這個，兩邊的顏色才對得起來
fn day_color(day: Date, app: &App) -> Color {
    let theme = &app.theme;
    let items: Vec<&Item> = app
        .items
        .iter()
        .filter(|item| item.date == day && is_shown(&app.groups, item))
        .collect();
    if day == app.today {
        theme.today
    } else if items.iter().any(|item| item.holiday) {
        theme.holiday
    } else if items.iter().any(|item| !item.done) {
        theme.has_item
    } else if day.weekday().to_monday_zero_offset() >= 5 {
        theme.weekend
    } else {
        theme.text
    }
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

/// 右邊的一件事：「10/10 國慶日」；日期和標題的顏色都跟月曆上那天一樣
///
/// 鍵盤在月曆時，選取那天的事整列換底色；鍵盤在右邊時，選到的那件事換底色、日期加括號 `[10/17]`
fn item_line(item: &Item, app: &App, chosen: Option<&Item>) -> Line<'static> {
    let theme = &app.theme;
    let date = format!("{}/{}", item.date.month(), item.date.day());
    // `chosen` = 鍵盤在右邊時選到的那件事；鍵盤不在右邊時是 `None`
    let agenda = chosen.is_some();
    let chosen = match chosen {
        Some(chosen) => std::ptr::eq(item, chosen),
        None => item.date == app.selected,
    };
    let date_style = Style::new().fg(day_color(item.date, app));
    // 日期那段 7 格寬：「 10/17 」或「[10/17]」
    let mut spans = if agenda && chosen {
        let bracket = Style::new().fg(theme.selected).add_modifier(Modifier::BOLD);
        vec![
            Span::raw(" ".repeat(5 - date.len())),
            Span::styled("[", bracket),
            Span::styled(date, date_style.add_modifier(Modifier::BOLD)),
            Span::styled("]", bracket),
        ]
    } else {
        vec![Span::styled(format!(" {date:>5} "), date_style)]
    };
    if let Some(time) = item.time {
        spans.push(Span::styled(
            format!("{time} "),
            Style::new().fg(theme.muted),
        ));
    }
    spans.push(Span::styled(
        item.title,
        item_style(item, day_color(item.date, app), theme),
    ));
    let line = Line::from(spans);
    if chosen {
        line.style(Style::new().bg(theme.selected_item_bg))
    } else {
        line
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::texts::Lang;
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
        // 自己的資料（不用 mock 的，換假資料才不會壞）：10/1～10/6 每天一件事
        let items: Vec<Item> = (1..=6)
            .map(|day| Item {
                date: date(2026, 10, day),
                time: None,
                group: "工作",
                title: "x",
                holiday: false,
                done: false,
            })
            .collect();
        let items: Vec<&Item> = items.iter().collect();
        let rows = rows(
            date(2026, 10, 1),
            date(2027, 1, 1),
            &items,
            1,
            Lang::Zh.texts(),
        );
        // 10 月 5 週但有 6 件事 → 6 列；11 月 6 週、12 月 5 週；跨年是粗線；1 月 5 週
        assert_eq!(shape(&rows), "wwwww.-wwwwww-wwwww=wwwww");
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
    fn english_labels_have_no_suffix() {
        let rows = rows(
            date(2026, 12, 1),
            date(2027, 1, 1),
            &[],
            1,
            Lang::En.texts(),
        );
        let labels: Vec<&str> = rows
            .iter()
            .filter_map(|row| match row {
                Row::Week {
                    label: Some(label), ..
                } => Some(label.as_str()),
                _ => None,
            })
            .collect();
        assert_eq!(labels, ["2026", "Dec", "2027", "Jan"]);
    }

    #[test]
    fn selected_day_is_wrapped_in_brackets_without_moving_the_others() {
        let mut app = App::new(date(2026, 10, 17));
        let week = |day| {
            let rows = calendar::month_rows(day);
            *rows.iter().find(|row| row.contains(&Some(day))).unwrap()
        };
        let text = |app: &App, days: &WeekRow| day_cells(days, app).to_string();
        // 兩位數、在中間
        assert_eq!(
            text(&app, &week(date(2026, 10, 17))),
            " 12 13 14 15 16[17]18"
        );
        // 一位數、星期一
        app.selected = date(2026, 10, 5);
        assert_eq!(
            text(&app, &week(date(2026, 10, 5))),
            " [5] 6  7  8  9 10 11"
        );
        // 星期日：`]` 放在最後多留的那格
        app.selected = date(2026, 10, 18);
        assert_eq!(
            text(&app, &week(date(2026, 10, 18))),
            " 12 13 14 15 16 17[18]"
        );
        // 跨月那週的另一半：沒有選取的那週不加括號
        assert_eq!(
            text(&app, &week(date(2026, 11, 1))),
            "                    1"
        );
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
