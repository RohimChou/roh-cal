//! 版型 1：左邊的月曆框和右邊的事項框，兩邊同一列對齊；表頭固定在上面，下面一個月一大塊往下排，可以上下捲動
//!
//! 每個月佔幾列、列的編號、捲動到哪：見 `months.rs`

use super::{
    Click,
    months::{Label, Months, Row},
};
use crate::{
    app::{App, Focus},
    calendar::{self, WeekRow},
    tasks::Item,
    texts::Texts,
    theme::Theme,
};
use jiff::civil::Date;
use ratatui::{
    Frame,
    layout::{Constraint, Layout, Position, Rect},
    style::{Color, Modifier, Style},
    symbols::line,
    text::{Line, Span},
    widgets::{Block, BorderType},
};
use std::rc::Rc;

/// 月曆框裡的四欄：年月、週數、│、7 天（一天 3 格，多留 1 格給選取日期的 `]`）
const COLUMNS: [Constraint; 4] = [
    Constraint::Length(6),
    Constraint::Length(4),
    Constraint::Length(1),
    Constraint::Length(22),
];
/// 月曆框的寬度：四欄 + 左右框線
pub const CALENDAR_WIDTH: u16 = 6 + 4 + 1 + 22 + 2;
/// 選取的那列離畫面上下邊緣至少留幾列，不夠才捲（像 vim 的 scrolloff）
const SCROLLOFF: i64 = 2;
/// 右邊的事分欄時，每一欄至少幾格寬
const AGENDA_COLUMN_WIDTH: u16 = 28;
/// 右邊最多分幾欄
const MAX_AGENDA_COLUMNS: u16 = 4;
/// 右邊欄和欄之間空幾格
const AGENDA_GAP: u16 = 2;

/// 右邊 `width` 格寬要分幾欄：每欄至少 `AGENDA_COLUMN_WIDTH` 格（欄和欄之間空 `AGENDA_GAP` 格）、最多 `MAX_AGENDA_COLUMNS` 欄
fn agenda_columns(width: u16) -> usize {
    usize::from(
        ((width + AGENDA_GAP) / (AGENDA_COLUMN_WIDTH + AGENDA_GAP)).clamp(1, MAX_AGENDA_COLUMNS),
    )
}

/// 把右邊的一列切成 `count` 欄，像報紙一樣
fn split_columns(area: Rect, count: usize) -> Rc<[Rect]> {
    Layout::horizontal(vec![Constraint::Fill(1); count])
        .spacing(AGENDA_GAP)
        .split(area)
}

/// 這一週裡第一個屬於這個月的日子；週數、年 / 月都用它算
fn first_day(days: &WeekRow) -> Option<Date> {
    days.iter().flatten().next().copied()
}

/// 畫左邊的月曆框（`calendar`）和右邊的事項框（`agenda`）
///
/// 捲到選取那天離上下邊緣至少留 `SCROLLOFF` 列，捲完的位置記在 `app.calendar_top`
pub fn render(frame: &mut Frame, calendar: Rect, agenda: Rect, app: &mut App) {
    let [calendar_header, calendar_rule, calendar_body] = inner_areas(calendar);
    let [agenda_header, agenda_rule, agenda_body] = inner_areas(agenda);
    let columns = agenda_columns(agenda_body.width);
    // 捲動：還沒捲過（剛打開、按了 `t`）就先把選取那天的那個月放在最上面
    // 要改 `app`，所以先算好（`Months` 借著 `app.items`，Rust 不准一邊借著讀一邊改）
    let top = {
        let months = Months::new(app, columns);
        let top = app
            .calendar_top
            .unwrap_or_else(|| months.block_start(app.selected));
        months.scrolled_to(
            top,
            app.selected,
            app.chosen(),
            i64::from(calendar_body.height),
            SCROLLOFF,
        )
    };
    app.calendar_top = Some(top);
    // 接下來只讀
    let app = &*app;
    let months = Months::new(app, columns);
    let texts = app.lang.texts();
    let theme = &app.theme;
    // 鍵盤在的那個框，框線用 `focus` 的顏色
    for (area, focus) in [(calendar, Focus::Calendar), (agenda, Focus::Agenda)] {
        let color = if app.focus == focus {
            theme.focus
        } else {
            theme.line
        };
        let block = Block::bordered()
            .border_type(BorderType::Rounded)
            .border_style(Style::new().fg(color));
        frame.render_widget(block, area);
    }
    let chosen = app.chosen();

    // 畫面上看得到的列（事很多的月份，週會黏在畫面最上面，見 `Months::visible_rows`）
    let visible = months.visible_rows(top, usize::from(calendar_body.height));

    // 表頭：畫面上沒有 1 月寫的年份時，年月欄改寫最上面那週的年份，提醒現在是哪一年
    let year_on_screen = visible.iter().any(|row| {
        matches!(
            row,
            Row::Week {
                label: Some(Label::Year),
                ..
            }
        )
    });
    let year_hint = if year_on_screen {
        None
    } else {
        visible.iter().find_map(|row| match row {
            Row::Week {
                days: Some(days), ..
            } => first_day(days),
            _ => None,
        })
    };
    render_header(frame, calendar_header, texts, theme, year_hint);
    // 右邊的表頭：左邊「重要事項」、右邊選取的日期（後面留一格空白）
    // 兩段畫在同一列，日期的顏色放在 `Span` 上：`Line::styled` 的顏色會套到整列，會蓋掉先畫的
    frame.render_widget(
        Line::styled(texts.header_items, Style::new().fg(theme.muted)),
        agenda_header,
    );
    let date = format!("{} ", texts.date_title(app.selected));
    frame.render_widget(
        Line::from(Span::styled(date, Style::new().fg(theme.text))).right_aligned(),
        agenda_header,
    );
    render_rule(frame, calendar_rule, agenda_rule, false, theme);

    // 一列一列畫，左右兩個框的同一列在同一個高度
    for (y, row) in (calendar_body.y..calendar_body.bottom()).zip(&visible) {
        let calendar_row = Rect::new(calendar_body.x, y, calendar_body.width, 1);
        let agenda_row = Rect::new(agenda_body.x, y, agenda_body.width, 1);
        match row {
            Row::Separator { year } => render_rule(frame, calendar_row, agenda_row, *year, theme),
            Row::Week { label, days, items } => {
                render_week(frame, calendar_row, *label, days.as_ref(), app);
                for (item, area) in items
                    .iter()
                    .zip(split_columns(agenda_row, items.len()).iter())
                {
                    if let Some(item) = item {
                        // 鍵盤在右邊時選到的那件事：日期加括號、墊底色
                        let is_chosen = chosen.is_some_and(|chosen| std::ptr::eq(chosen, *item));
                        // 鍵盤在月曆時：選取那天的事墊底色（跟月曆上那週一樣）
                        let on_selected_day =
                            app.focus == Focus::Calendar && item.date == Some(app.selected);
                        frame.render_widget(item_line(item, app, is_chosen), *area);
                        if is_chosen || on_selected_day {
                            frame
                                .buffer_mut()
                                .set_style(*area, Style::new().bg(theme.cursor_line));
                        }
                    }
                }
            }
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

/// 滑鼠點在 terminal 的 (`column`, `row`) 點到什麼：左邊月曆框（`calendar`）的一天，或右邊事項框（`agenda`）的一件事；
/// 沒點在日期或事上回傳 `None`
///
/// 捲動的位置用上一次畫的時候記下的 `app.calendar_top`；月曆一天佔 3 格（日期前面的空白也算那天）
pub fn click_at(calendar: Rect, agenda: Rect, app: &App, column: u16, row: u16) -> Option<Click> {
    let [_, _, calendar_body] = inner_areas(calendar);
    let [_, _, agenda_body] = inner_areas(agenda);
    let months = Months::new(app, agenda_columns(agenda_body.width));
    let position = Position::new(column, row);
    // 畫面上的每一列，跟畫畫面時一樣（含黏在上面的週）；左右兩個框同一列對齊
    let rows = months.visible_rows(app.calendar_top?, usize::from(calendar_body.height));
    if calendar_body.contains(position) {
        // 點的是哪一列（月和月之間的線、只有事的列都沒有日期）
        let Row::Week {
            days: Some(days), ..
        } = rows.get(usize::from(row - calendar_body.y))?
        else {
            return None;
        };
        // 第幾天：7 天那欄的第幾個 3 格；最後多留的那格是第 8 個，沒有日期
        let [_, _, _, days_area] = Layout::horizontal(COLUMNS).areas(calendar_body);
        let weekday = usize::from(column.checked_sub(days_area.x)? / 3);
        days.get(weekday).copied().flatten().map(Click::Day)
    } else if agenda_body.contains(position) {
        let Row::Week { items, .. } = rows.get(usize::from(row - agenda_body.y))? else {
            return None;
        };
        // 點在第幾欄（欄和欄之間的空白不算）
        let agenda_row = Rect::new(agenda_body.x, row, agenda_body.width, 1);
        let index = split_columns(agenda_row, items.len())
            .iter()
            .position(|area| area.contains(position))?;
        let item = items.get(index).copied().flatten()?;
        // 那件事在 `App::agenda()` 裡的位置
        app.agenda()
            .iter()
            .position(|other| std::ptr::eq(*other, item))
            .map(Click::Item)
    } else {
        None
    }
}

/// 表頭：「年月  週 │ 一 二 … 日」；六、日用週末的顏色
///
/// `year_hint` 有值（畫面上看不到年份）：「年月」改寫那天的年份，例：「2026年  週」
fn render_header(
    frame: &mut Frame,
    area: Rect,
    texts: &Texts,
    theme: &Theme,
    year_hint: Option<Date>,
) {
    let muted = Style::new().fg(theme.muted);
    let month_header =
        year_hint.map_or(texts.header_month.to_string(), |day| texts.year_label(day));
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
            Line::styled(month_header, muted).right_aligned(),
            Line::styled(texts.header_week, muted).right_aligned(),
            Line::styled(line::VERTICAL, Style::new().fg(theme.line)),
            Line::from(weekdays),
        ],
    );
}

/// 月曆框的一列：左邊的年 / 月、週數、│、7 天（一天 3 格寬、日期靠右）；只有事的列只畫 │
///
/// 選取的那天在這一列：週數用亮的顏色、整列墊上 `cursor_line` 的底色；其他列的週數比較暗
fn render_week(
    frame: &mut Frame,
    area: Rect,
    label: Option<Label>,
    days: Option<&WeekRow>,
    app: &App,
) {
    let theme = &app.theme;
    let texts = app.lang.texts();
    let day = days.and_then(first_day);
    let label = match (label, day) {
        (Some(Label::Year), Some(day)) => texts.year_label(day),
        (Some(Label::Month), Some(day)) => texts.month_label(day),
        _ => String::new(),
    };
    // 選取的那天在這一列：週數亮一點、整列墊底色
    let selected_week = days.is_some_and(|days| days.contains(&Some(app.selected)));
    // 週數：這一列任何一天都行（同一列一定是同一週）
    let week = day
        .map(|day| format!("{:>3} ", calendar::week_of_year(day)))
        .unwrap_or_default();
    let week_color = if selected_week {
        theme.week_number
    } else {
        theme.week_number_dim
    };
    render_cells(
        frame,
        area,
        [
            Line::styled(label, Style::new().fg(theme.label)).right_aligned(),
            Line::styled(week, Style::new().fg(week_color)),
            Line::styled(line::VERTICAL, Style::new().fg(theme.line)),
            days.map(|days| day_cells(days, app)).unwrap_or_default(),
        ],
    );
    if selected_week {
        // 只換底色，字的顏色不動
        frame
            .buffer_mut()
            .set_style(area, Style::new().bg(theme.cursor_line));
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

/// 一天的日期顏色：今天 > 有沒做完的事 > 週末 > 一般（事都做完了就跟沒事一樣）
fn day_color(day: Date, app: &App) -> Color {
    let theme = &app.theme;
    if day == app.today {
        theme.today
    } else if app
        .items
        .iter()
        .any(|item| item.date == Some(day) && !item.done)
    {
        theme.has_item
    } else if day.weekday().to_monday_zero_offset() >= 5 {
        theme.weekend
    } else {
        theme.text
    }
}

/// 右邊的一件事：「 10/16 17:30 交週報」（日期 5 格靠右、有時間才寫時間）
///
/// 沒做完的：日期和標題用那天在月曆上的顏色，兩邊對得起來；時間淡色。做完的：整行變灰、標題劃線。
/// `chosen` = 鍵盤在右邊時選到的這件：日期加括號 `[10/17]`、加粗
fn item_line(item: &Item, app: &App, chosen: bool) -> Line<'static> {
    let theme = &app.theme;
    let color = if item.done {
        theme.muted
    } else {
        item.date.map_or(theme.text, |date| day_color(date, app))
    };
    let date = item
        .date
        .map(|date| format!("{}/{}", date.month(), date.day()))
        .unwrap_or_default();
    // 日期那段 7 格寬：「 10/17 」；選到的是「[10/17]」（日期比較短時，括號前面補空白）
    let mut spans = if chosen {
        let bracket = Style::new().fg(theme.selected).add_modifier(Modifier::BOLD);
        vec![
            Span::raw(" ".repeat(5_usize.saturating_sub(date.len()))),
            Span::styled("[", bracket),
            Span::styled(date, Style::new().fg(color).add_modifier(Modifier::BOLD)),
            Span::styled("]", bracket),
        ]
    } else {
        vec![Span::styled(format!(" {date:>5} "), Style::new().fg(color))]
    };
    if let Some(time) = item.time {
        spans.push(Span::styled(
            format!("{} ", time.strftime("%H:%M")),
            Style::new().fg(theme.muted),
        ));
    }
    let mut title_style = Style::new().fg(color);
    if item.done {
        title_style = title_style.add_modifier(Modifier::CROSSED_OUT);
    }
    spans.push(Span::styled(item.title.clone(), title_style));
    Line::from(spans)
}

/// 左右兩個框裡的同一條橫線（`thick` = 跨年的粗線）；月曆框裡跟直線交會的地方畫十字
fn render_rule(frame: &mut Frame, calendar: Rect, agenda: Rect, thick: bool, theme: &Theme) {
    let (horizontal, cross, color) = if thick {
        (line::THICK_HORIZONTAL, "┿", theme.year_line)
    } else {
        (line::HORIZONTAL, line::CROSS, theme.line)
    };
    let style = Style::new().fg(color);
    let columns = Layout::horizontal(COLUMNS).areas::<4>(calendar);
    for (i, column) in columns.into_iter().enumerate() {
        // 第 3 欄是直線那欄
        let symbol = if i == 2 { cross } else { horizontal };
        let text = symbol.repeat(usize::from(column.width));
        frame.render_widget(Line::styled(text, style), column);
    }
    let text = horizontal.repeat(usize::from(agenda.width));
    frame.render_widget(Line::styled(text, style), agenda);
}

/// 月曆框一列的四欄各畫一段字
fn render_cells(frame: &mut Frame, area: Rect, cells: [Line; 4]) {
    let columns = Layout::horizontal(COLUMNS).areas::<4>(area);
    for (cell, column) in cells.into_iter().zip(columns) {
        frame.render_widget(cell, column);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn agenda_columns_are_at_least_28_wide() {
        // 欄和欄之間空 2 格：2 欄要 28 + 2 + 28 = 58 格
        assert_eq!(agenda_columns(0), 1);
        assert_eq!(agenda_columns(57), 1);
        assert_eq!(agenda_columns(58), 2);
        assert_eq!(agenda_columns(88), 3);
        assert_eq!(agenda_columns(118), 4);
        assert_eq!(agenda_columns(500), 4);
    }
}
