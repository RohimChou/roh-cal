//! 版型 1：左邊的月曆框和右邊的事項框，兩邊同一列對齊；表頭固定在上面，下面一個月一大塊往下排，可以上下捲動
//!
//! 一個月那一塊 = 週數和這個月的事排成幾列（分欄之後）比較多的那個，再加上後面那條線；
//! 每一列有一個編號：今天那個月的第一列是第 0 列，往下 1、2、3…，往上 -1、-2…

use crate::{
    app::App,
    calendar::{self, WeekRow},
    tasks::Item,
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
use std::rc::Rc;

/// 月曆框裡的四欄：年月、週數、│、7 天（一天 3 格，多留 1 格給選取日期的 `]`）
const COLUMNS: [Constraint; 4] = [
    Constraint::Length(6),
    Constraint::Length(4),
    Constraint::Length(1),
    Constraint::Length(22),
];
/// 月曆框的寬度：四欄 + 左右框線
pub const WIDTH: u16 = 6 + 4 + 1 + 22 + 2;
/// 選取的那列離畫面上下邊緣至少留幾列，不夠才捲（像 vim 的 scrolloff）
const SCROLLOFF: i64 = 2;
/// 右邊的事分欄時，每一欄至少幾格寬
const AGENDA_COLUMN_WIDTH: u16 = 28;
/// 右邊最多分幾欄
const MAX_AGENDA_COLUMNS: u16 = 4;
/// 右邊欄和欄之間空幾格
const AGENDA_GAP: u16 = 2;

/// 左邊欄寫什麼
#[derive(Debug, Clone, Copy, PartialEq)]
enum Label {
    /// 年份，例：`2027年`
    Year,
    /// 月份，例：`10月`
    Month,
}

/// 畫面上的一列（左右兩個框的同一列）
#[derive(Debug, PartialEq)]
enum Row<'a> {
    /// 月和月之間的線；`year` = 下一個月是 1 月（跨年），畫粗線
    Separator { year: bool },
    /// 一個月裡的一列：左邊的年 / 月（可能沒有）、這一週屬於這個月的日子、右邊每一欄的一件事（可能沒有）
    ///
    /// 這個月的事比週數多時，後面幾列只有右邊的事，`days` 是 `None`
    Week {
        label: Option<Label>,
        days: Option<WeekRow>,
        items: Vec<Option<&'a Item>>,
    },
}

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

/// 排版要用的資料：哪個月是第 0 列、有日期的事（照日期、時間排好）、右邊分幾欄
struct Months<'a> {
    /// 第 0 列所在的月（今天那個月）
    origin: Date,
    /// 有日期的事，照日期、時間排好（同一天沒有時間的在前；日期、時間一樣的照檔案裡的順序）
    items: Vec<&'a Item>,
    /// 右邊的事分幾欄
    columns: usize,
}

impl<'a> Months<'a> {
    fn new(app: &'a App, columns: usize) -> Self {
        // 沒有日期的事先不顯示（月曆上沒有位置放）
        let mut items: Vec<&Item> = app
            .items
            .iter()
            .filter(|item| item.date.is_some())
            .collect();
        // sort_by_key 是 stable sort：key 一樣的保持原本的順序；時間的 None 排在 Some 前面
        items.sort_by_key(|item| (item.date, item.time));
        Self {
            origin: app.today,
            items,
            columns,
        }
    }

    /// `month` 那個月的事（`items` 已經照日期排好，所以是連續的一段）
    fn month_items(&self, month: Date) -> &[&'a Item] {
        let first = Some(month.first_of_month());
        let last = Some(month.last_of_month());
        let start = self.items.partition_point(|item| item.date < first);
        let end = self.items.partition_point(|item| item.date <= last);
        &self.items[start..end]
    }

    /// 一個月那一塊有幾列（不算後面那條線）：週數和事排成幾列，比較多的那個
    fn month_height(&self, month: Date) -> usize {
        let weeks = calendar::month_rows(month).len();
        weeks.max(self.month_items(month).len().div_ceil(self.columns))
    }

    /// 一個月那一塊有幾列，加上後面那條線
    fn block_height(&self, month: Date) -> i64 {
        i64::try_from(self.month_height(month)).unwrap_or(0) + 1
    }

    /// `month` 那一塊的第一列是第幾列
    fn block_start(&self, month: Date) -> i64 {
        let mut start = 0;
        let mut current = self.origin.first_of_month();
        let month = month.first_of_month();
        // 往後：一路加上前面每個月的高度；往前：一路減掉
        while current < month {
            start += self.block_height(current);
            current = current.saturating_add(1.month());
        }
        while current > month {
            current = current.saturating_sub(1.month());
            start -= self.block_height(current);
        }
        start
    }

    /// `date` 那一週是第幾列
    fn row_of(&self, date: Date) -> i64 {
        // 這一週是那個月的第幾週（從 0 開始）；週一定排在那個月最上面幾列
        let week = calendar::month_rows(date)
            .iter()
            .position(|row| row.contains(&Some(date)))
            .unwrap_or(0);
        self.block_start(date) + i64::try_from(week).unwrap_or(0)
    }

    /// 第 `row` 列落在哪個月那一塊：回傳那個月（1 號）和那一塊的第一列是第幾列
    fn block_at(&self, row: i64) -> (Date, i64) {
        let mut month = self.origin.first_of_month();
        let mut start = 0;
        while row < start {
            month = month.saturating_sub(1.month());
            start -= self.block_height(month);
        }
        while row >= start + self.block_height(month) {
            start += self.block_height(month);
            month = month.saturating_add(1.month());
        }
        (month, start)
    }

    /// 從 `first` 那個月開始一個月一個月往下排，排到至少 `height` 列（最後一個月可能超出去）
    fn rows(&self, first: Date, height: usize) -> Vec<Row<'a>> {
        let mut rows = Vec::new();
        let mut month = first.first_of_month();
        while rows.len() < height {
            if !rows.is_empty() {
                rows.push(Row::Separator {
                    year: month.month() == 1,
                });
            }
            let weeks = calendar::month_rows(month);
            let items = self.month_items(month);
            let month_height = self.month_height(month);
            // 1 月：第一列寫年份、第二列寫月份；其他月只在第一列寫月份
            let january = month.month() == 1;
            for i in 0..month_height {
                let label = match (january, i) {
                    (true, 0) => Some(Label::Year),
                    (true, 1) | (false, 0) => Some(Label::Month),
                    _ => None,
                };
                rows.push(Row::Week {
                    label,
                    days: weeks.get(i).copied(),
                    // 先排滿第一欄再排第二欄：第 c 欄的第 i 列 = 這個月的第 c × 高度 + i 件事
                    items: (0..self.columns)
                        .map(|c| items.get(c * month_height + i).copied())
                        .collect(),
                });
            }
            month = month.saturating_add(1.month());
        }
        rows
    }

    /// 第 `row` 列是什麼（滑鼠點的時候用）
    fn row_at(&self, row: i64) -> Option<Row<'a>> {
        let (month, start) = self.block_at(row);
        let skip = usize::try_from(row - start).ok()?;
        self.rows(month, skip + 1).into_iter().nth(skip)
    }
}

/// 這一週裡第一個屬於這個月的日子；週數、年 / 月都用它算
fn first_day(days: &WeekRow) -> Option<Date> {
    days.iter().flatten().next().copied()
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
        scrolled_top(
            top,
            months.row_of(app.selected),
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
    for area in [calendar, agenda] {
        let block = Block::bordered()
            .border_type(BorderType::Rounded)
            .border_style(Style::new().fg(theme.line));
        frame.render_widget(block, area);
    }

    // 從最上面那列所在的那個月開始排，跳過那個月在畫面上面的列
    let (month, start) = months.block_at(top);
    let skip = usize::try_from(top - start).unwrap_or(0);
    let height = usize::from(calendar_body.height);
    let rows = months.rows(month, skip + height);
    let visible = &rows[skip..skip + height];

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
    frame.render_widget(
        Line::styled(texts.header_items, Style::new().fg(theme.muted)),
        agenda_header,
    );
    render_rule(frame, calendar_rule, agenda_rule, false, theme);

    // 一列一列畫，左右兩個框的同一列在同一個高度
    for (y, row) in (calendar_body.y..calendar_body.bottom()).zip(visible) {
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
                        frame.render_widget(item_line(item, app), *area);
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

/// 滑鼠點在 terminal 的 (`column`, `row`) 點到哪一天：左邊月曆框（`calendar`）的日期，或右邊事項框（`agenda`）的一件事那天；
/// 沒點在日期或事上回傳 `None`
///
/// 捲動的位置用上一次畫的時候記下的 `app.calendar_top`；月曆一天佔 3 格（日期前面的空白也算那天）
pub fn date_at(calendar: Rect, agenda: Rect, app: &App, column: u16, row: u16) -> Option<Date> {
    let [_, _, calendar_body] = inner_areas(calendar);
    let [_, _, agenda_body] = inner_areas(agenda);
    let months = Months::new(app, agenda_columns(agenda_body.width));
    let position = Position::new(column, row);
    if calendar_body.contains(position) {
        // 點的是哪一列（月和月之間的線、只有事的列都沒有日期）
        let Row::Week {
            days: Some(days), ..
        } = months.row_at(app.calendar_top? + i64::from(row - calendar_body.y))?
        else {
            return None;
        };
        // 第幾天：7 天那欄的第幾個 3 格；最後多留的那格是第 8 個，沒有日期
        let [_, _, _, days_area] = Layout::horizontal(COLUMNS).areas(calendar_body);
        let weekday = usize::from(column.checked_sub(days_area.x)? / 3);
        days.get(weekday).copied().flatten()
    } else if agenda_body.contains(position) {
        let Row::Week { items, .. } =
            months.row_at(app.calendar_top? + i64::from(row - agenda_body.y))?
        else {
            return None;
        };
        // 點在第幾欄（欄和欄之間的空白不算）
        let agenda_row = Rect::new(agenda_body.x, row, agenda_body.width, 1);
        let index = split_columns(agenda_row, items.len())
            .iter()
            .position(|area| area.contains(position))?;
        items.get(index).copied().flatten()?.date
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
/// 選取的那天在這一列：整列墊上 `cursor_line` 的底色
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
    // 週數：這一列任何一天都行（同一列一定是同一週）
    let week = day
        .map(|day| format!("{:>3} ", calendar::week_of_year(day)))
        .unwrap_or_default();
    render_cells(
        frame,
        area,
        [
            Line::styled(label, Style::new().fg(theme.label)).right_aligned(),
            Line::styled(week, Style::new().fg(theme.week_number)),
            Line::styled(line::VERTICAL, Style::new().fg(theme.line)),
            days.map(|days| day_cells(days, app)).unwrap_or_default(),
        ],
    );
    if days.is_some_and(|days| days.contains(&Some(app.selected))) {
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
/// 日期和標題用那天在月曆上的顏色，兩邊對得起來；時間淡色；做完的標題變灰、劃線
fn item_line(item: &Item, app: &App) -> Line<'static> {
    let theme = &app.theme;
    let color = item.date.map_or(theme.text, |date| day_color(date, app));
    let date = item
        .date
        .map(|date| format!("{}/{}", date.month(), date.day()))
        .unwrap_or_default();
    let mut spans = vec![Span::styled(format!(" {date:>5} "), Style::new().fg(color))];
    if let Some(time) = item.time {
        spans.push(Span::styled(
            format!("{} ", time.strftime("%H:%M")),
            Style::new().fg(theme.muted),
        ));
    }
    let title_style = if item.done {
        Style::new()
            .fg(theme.muted)
            .add_modifier(Modifier::CROSSED_OUT)
    } else {
        Style::new().fg(color)
    };
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
    use crate::texts::Lang;
    use jiff::civil::date;

    /// 今天 2026/10/17、任務檔是 `text` 的 app
    fn app_with(text: &str) -> App {
        App::new(
            Lang::En,
            date(2026, 10, 17),
            crate::tasks::parse(text).items,
        )
    }

    #[test]
    fn row_numbers_count_from_this_month() {
        // 今天是 2026/10：10 月第一列是第 0 列；沒有事時，每個月那一塊 = 週數 + 1 條線
        let app = app_with("");
        let months = Months::new(&app, 1);
        assert_eq!(months.block_start(date(2026, 10, 1)), 0);
        assert_eq!(months.block_start(date(2026, 11, 20)), 6);
        assert_eq!(months.block_start(date(2027, 1, 1)), 19);
        assert_eq!(months.block_start(date(2027, 2, 1)), 25);
        assert_eq!(months.block_start(date(2026, 9, 1)), -6);
        assert_eq!(months.block_start(date(2026, 8, 1)), -13);
        assert_eq!(months.row_of(date(2026, 10, 17)), 2);
        assert_eq!(months.row_of(date(2026, 11, 7)), 7);
        assert_eq!(months.row_of(date(2027, 1, 3)), 19);
        assert_eq!(months.row_of(date(2026, 9, 26)), -3);
        // 每一塊最後一列是線：10 月的線是第 5 列、9 月的線是第 -1 列
        assert_eq!(months.block_at(5), (date(2026, 10, 1), 0));
        assert_eq!(months.block_at(6), (date(2026, 11, 1), 6));
        assert_eq!(months.block_at(-1), (date(2026, 9, 1), -6));
        assert_eq!(months.block_at(-6), (date(2026, 9, 1), -6));
        assert_eq!(months.block_at(-7), (date(2026, 8, 1), -13));
    }

    #[test]
    fn months_grow_with_their_items() {
        // 10 月 5 週、7 件事：一欄要 7 列，兩欄要 4 列（比 5 週少，所以還是 5 列）；9 月的事不算在 10 月
        let text = (1..=7)
            .map(|day| format!("- [ ] x @2026-10-{day:02}\n"))
            .collect::<String>()
            + "- [ ] 九月 @2026-09-30\n- [ ] 沒日期\n";
        let app = app_with(&text);
        let one = Months::new(&app, 1);
        assert_eq!(one.month_items(date(2026, 10, 1)).len(), 7);
        assert_eq!(one.block_height(date(2026, 10, 1)), 8);
        assert_eq!(one.block_start(date(2026, 11, 1)), 8);
        let two = Months::new(&app, 2);
        assert_eq!(two.block_height(date(2026, 10, 1)), 6);
        // 選取的那週還是在那個月最上面幾列
        assert_eq!(one.row_of(date(2026, 11, 7)), 9);
    }

    #[test]
    fn items_are_sorted_by_date_then_time_untimed_first() {
        // 同一天：沒時間的在前、有時間的照時間；日期時間一樣的照檔案順序
        let app = app_with(
            "- b @2026-10-16 17:30\n- a @2026-10-16 09:00\n- c @2026-10-16\n- d @2026-10-15\n\
             - e @2026-10-16 09:00\n",
        );
        let months = Months::new(&app, 1);
        let titles: Vec<&str> = months
            .month_items(date(2026, 10, 1))
            .iter()
            .map(|item| item.title.as_str())
            .collect();
        assert_eq!(titles, ["d", "c", "a", "e", "b"]);
    }

    #[test]
    fn rows_fill_the_first_column_before_the_second() {
        // 10 月 7 件事、兩欄：高度 5（週數比較多），第一欄 5 件、第二欄 2 件
        let text = (1..=7)
            .map(|day| format!("- [ ] {day} @2026-10-{day:02}\n"))
            .collect::<String>();
        let app = app_with(&text);
        let months = Months::new(&app, 2);
        let rows = months.rows(date(2026, 10, 1), 6);
        let titles: Vec<[&str; 2]> = rows[..5]
            .iter()
            .map(|row| match row {
                Row::Week { items, .. } => [0, 1].map(|c| items[c].map_or("", |item| &item.title)),
                Row::Separator { .. } => panic!("10 月的 5 列裡不該有線"),
            })
            .collect();
        assert_eq!(
            titles,
            [["1", "6"], ["2", "7"], ["3", ""], ["4", ""], ["5", ""]]
        );
        assert_eq!(rows[5], Row::Separator { year: false });
    }

    /// 每列的種類，方便比對：`-` = 細線、`=` = 粗線、`w` = 有日期的列、`.` = 只有事的列
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
    fn months_with_more_items_than_weeks_get_rows_without_dates() {
        // 一欄：10 月 7 件事 → 5 週 + 2 列只有事；11 月沒事 → 6 週；12 月 → 5 週；跨年粗線；
        // 要 22 列時，最後的 1 月也整個排完（5 週）
        let text = (1..=7)
            .map(|day| format!("- [ ] x @2026-10-{day:02}\n"))
            .collect::<String>();
        let app = app_with(&text);
        let months = Months::new(&app, 1);
        assert_eq!(
            shape(&months.rows(date(2026, 10, 1), 22)),
            "wwwww..-wwwwww-wwwww=wwwww"
        );
    }

    #[test]
    fn block_at_finds_the_month_of_every_row() {
        // 用另一個方向核對：每一天的列，找回來都是它自己那個月；前後 5 年（每 3 天一筆，星期幾都會輪到）
        // 用範例任務檔，有些月比週數高
        let app = app_with(include_str!("../../testdata/tasks.md"));
        let months = Months::new(&app, 1);
        let mut day = date(2021, 1, 1);
        while day < date(2032, 1, 1) {
            let (month, start) = months.block_at(months.row_of(day));
            assert_eq!(month, day.first_of_month(), "{day}");
            assert_eq!(start, months.block_start(day), "{day}");
            day = day.saturating_add(3.days());
        }
    }

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
