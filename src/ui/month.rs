//! 版型 3：大月曆（照 v1 的 month view）：side menu 右邊整塊，6 週等高的格子
//!
//! 一次 6 週；選取的日期移出畫面才一週一週捲。格子的大小每次畫都從現在的空間算（terminal 拉大縮小就跟著變）

use super::{Click, planner::day_color};
use crate::{
    app::{App, Focus},
    calendar,
};
use jiff::{ToSpan, civil::Date};
use ratatui::{
    Frame,
    buffer::Buffer,
    layout::{Position, Rect},
    style::{Modifier, Style},
    symbols::line,
    text::{Line, Span},
    widgets::{Block, BorderType},
};

/// 一次顯示幾週
const WEEKS: u16 = 6;
/// 左邊週數欄的寬度：「  42 」
const WEEK_COLUMN_WIDTH: u16 = 5;

/// 大月曆的格子在畫面上的位置（不含格線）；畫畫面和滑鼠換算位置都用這個，兩邊才對得起來
#[derive(Debug, PartialEq)]
struct Grid {
    /// 週數欄和 7 天每一欄的 (x, 寬度)：`[週數欄, 一, 二, …, 日]`；欄和欄之間各留 1 格畫直線
    columns: [(u16, u16); 8],
    /// 表頭（星期幾）那一列；下一列是表頭的線
    header_y: u16,
    /// 每週幾列高：6 週一樣高，除不盡的零頭留在最下面
    week_height: u16,
}

impl Grid {
    /// 把框裡面（`inner`）切成格子
    ///
    /// 寬：週數欄 5 格，剩下的扣掉 7 條直線、7 天平分；除不盡的零頭從星期一開始一欄多 1 格。
    /// 高：表頭 1 列 + 表頭的線 1 列，剩下的扣掉週和週之間 5 條線、6 週平分
    fn new(inner: Rect) -> Self {
        let days_width = inner.width.saturating_sub(WEEK_COLUMN_WIDTH + 7);
        let (base, extra) = (days_width / 7, days_width % 7);
        let mut columns = [(inner.x, WEEK_COLUMN_WIDTH.min(inner.width)); 8];
        let mut x = inner.x.saturating_add(WEEK_COLUMN_WIDTH + 1);
        for (weekday, column) in (0..7).zip(columns.iter_mut().skip(1)) {
            let width = base + u16::from(weekday < extra);
            *column = (x, width);
            x = x.saturating_add(width + 1);
        }
        Self {
            columns,
            header_y: inner.y,
            week_height: inner.height.saturating_sub(2 + (WEEKS - 1)) / WEEKS,
        }
    }

    /// 格子太小、畫不出日期（每天至少要 1 格寬、每週至少 1 列）；零頭從星期一開始分，所以星期日最窄
    fn too_small(&self) -> bool {
        self.week_height == 0 || self.columns[7].1 == 0
    }

    /// 第 `week` 週（從 0 開始）的第一列
    fn week_y(&self, week: u16) -> u16 {
        self.header_y + 2 + week * (self.week_height + 1)
    }

    /// 第 `week` 週、第 `column` 欄（0 = 週數欄、1 = 星期一 … 7 = 星期日）的格子
    fn cell(&self, week: u16, column: usize) -> Rect {
        let (x, width) = self.columns[column];
        Rect::new(x, self.week_y(week), width, self.week_height)
    }

    /// 直線的 x：週數欄右邊、星期一到星期六的右邊
    fn vertical_xs(&self) -> impl Iterator<Item = u16> + '_ {
        self.columns[..7].iter().map(|&(x, width)| x + width)
    }
}

/// 畫面最上面那週的星期一：從 `top` 開始，選取的那週移出 6 週才捲（一週一週捲，跟 v1 一樣）
fn scrolled(top: Date, selected: Date) -> Date {
    let week = calendar::monday_of(selected);
    let last = top.saturating_add(i64::from(WEEKS - 1).weeks());
    if week < top {
        week
    } else if week > last {
        week.saturating_sub(i64::from(WEEKS - 1).weeks())
    } else {
        top
    }
}

/// 剛打開、按了 `t` / `[` / `]`、換到版型 3 時，最上面那週：`date` 那個月 1 號的前一週（看得到上個月月底）
fn month_top(date: Date) -> Date {
    calendar::monday_of(date.first_of_month()).saturating_sub(1.week())
}

/// 畫大月曆：圓角框（鍵盤在月曆時框線亮起來），右上角寫選取的日期；裡面是表頭（星期幾）+ 6 週的格子
///
/// 先捲到選取的那週看得到（記在 `app.month_top`），再畫
pub fn render(frame: &mut Frame, area: Rect, app: &mut App) {
    let top = scrolled(
        app.month_top.unwrap_or_else(|| month_top(app.selected)),
        app.selected,
    );
    app.month_top = Some(top);
    // 接下來只讀
    let app = &*app;
    let theme = &app.theme;
    let texts = app.lang.texts();
    let border = Style::new().fg(if app.focus == Focus::Calendar {
        theme.focus
    } else {
        theme.line
    });
    let date = format!(" {} ", texts.date_title(app.selected));
    let block = Block::bordered()
        .border_type(BorderType::Rounded)
        .border_style(border)
        .title_top(Line::styled(date, Style::new().fg(theme.text)).right_aligned());
    let inner = block.inner(area);
    frame.render_widget(block, area);
    let grid = Grid::new(inner);
    if grid.too_small() {
        return;
    }
    render_grid_lines(frame.buffer_mut(), inner, &grid, theme.grid, border);

    // 表頭：週數欄寫「週」，每一天的名字放在那欄的最左邊，跟下面的日期對齊；六、日用週末的顏色
    let muted = Style::new().fg(theme.muted);
    let header = |column: usize| {
        let (x, width) = grid.columns[column];
        Rect::new(x, grid.header_y, width, 1)
    };
    frame.render_widget(
        Line::styled(format!(" {}", texts.header_week), muted),
        header(0),
    );
    for (column, name) in (1..).zip(texts.weekdays) {
        let style = if column >= 6 {
            Style::new().fg(theme.weekend)
        } else {
            muted
        };
        frame.render_widget(Line::styled(format!(" {name}"), style), header(column));
    }

    // 選取的那個月：其他月份的日子變灰
    let month = app.selected.first_of_month();
    for week in 0..WEEKS {
        let monday = top.saturating_add(i64::from(week).weeks());
        let days: Vec<Date> = monday.series(1.day()).take(7).collect();
        // 週數：那週裡屬於選取那個月的日子（跨年那週才看得出差別）；選取那週亮黃、其他週暗黃
        let numbered = days
            .iter()
            .find(|day| day.first_of_month() == month)
            .copied()
            .unwrap_or(monday);
        let week_color = if days.contains(&app.selected) {
            theme.week_number
        } else {
            theme.week_number_dim
        };
        let number_area = Rect {
            height: 1,
            ..grid.cell(week, 0)
        };
        frame.render_widget(
            Line::styled(
                format!(" {:>3} ", calendar::week_of_year(numbered)),
                Style::new().fg(week_color),
            ),
            number_area,
        );
        for (column, &day) in (1..).zip(&days) {
            let cell = grid.cell(week, column);
            frame.render_widget(date_line(day, month, app), Rect { height: 1, ..cell });
        }
    }
}

/// 格子的第一行：日期靠左（選取的加括號 `[17]`），1 號後面寫月份，例：「 1 10月」
///
/// 顏色：今天 > 其他月份（灰）> 跟版型 1 的月曆一樣（有沒做完的事橘、週末藍）；選取那個月的日期加粗
fn date_line(day: Date, month: Date, app: &App) -> Line<'static> {
    let theme = &app.theme;
    let other_month = day.first_of_month() != month;
    let color = if day == app.today {
        theme.today
    } else if other_month {
        theme.other_month
    } else {
        day_color(day, app)
    };
    let mut number = Style::new().fg(color);
    if !other_month || day == app.today {
        number = number.add_modifier(Modifier::BOLD);
    }
    let bracket = Style::new().fg(theme.selected).add_modifier(Modifier::BOLD);
    let (open, close) = if day == app.selected {
        (Span::styled("[", bracket), Span::styled("]", bracket))
    } else {
        (Span::raw(" "), Span::raw(" "))
    };
    let mut spans = vec![open, Span::styled(day.day().to_string(), number), close];
    if day.day() == 1 {
        let label = if other_month {
            theme.other_month
        } else {
            theme.label
        };
        spans.push(Span::styled(
            app.lang.texts().month_label(day),
            Style::new().fg(label),
        ));
    }
    Line::from(spans)
}

/// 畫淡淡的格線：欄和欄之間 `│`、表頭下面和週和週之間 `─`、交叉的地方 `┬` / `┼`；
/// 橫線碰到外框接成 `├` / `┤`、直線碰到下框接成 `┴`（外框的顏色）。
/// 除不盡的零頭（最後一週下面空的列）直線照樣畫到下框，看起來是最後一週多幾列空白，不會有兩條線疊在一起
///
/// 直接一格一格寫進 `Buffer`（不用 widget）；超出畫面的位置略過
fn render_grid_lines(
    buffer: &mut Buffer,
    inner: Rect,
    grid: &Grid,
    color: ratatui::style::Color,
    border: Style,
) {
    let style = Style::new().fg(color);
    let mut put = |x: u16, y: u16, symbol: &str, style: Style| {
        if let Some(cell) = buffer.cell_mut(Position::new(x, y)) {
            cell.set_symbol(symbol).set_style(style);
        }
    };
    let top = grid.header_y + 1;
    for x in grid.vertical_xs() {
        for y in top..inner.bottom() {
            put(x, y, line::VERTICAL, style);
        }
        put(x, inner.bottom(), line::HORIZONTAL_UP, border);
    }
    // 表頭下面那條、週和週之間的 5 條
    let mut rules = vec![(top, line::HORIZONTAL_DOWN)];
    rules.extend((0..WEEKS - 1).map(|week| (grid.week_y(week) + grid.week_height, line::CROSS)));
    for (y, cross) in rules {
        for x in inner.x..inner.right() {
            put(x, y, line::HORIZONTAL, style);
        }
        for x in grid.vertical_xs() {
            put(x, y, cross, style);
        }
        put(inner.x.wrapping_sub(1), y, line::VERTICAL_RIGHT, border);
        put(inner.right(), y, line::VERTICAL_LEFT, border);
    }
}

/// 滑鼠點在大月曆（`area`）的 (`column`, `row`) 點到哪一天；點在表頭、週數欄、格線、外框上回傳 `None`
///
/// 捲動的位置用上一次畫的時候記下的 `app.month_top`；最後一週下面空的那幾列（除不盡的零頭）也算最後一週
pub fn click_at(area: Rect, app: &App, column: u16, row: u16) -> Option<Click> {
    let inner = Block::bordered().inner(area);
    let grid = Grid::new(inner);
    if grid.too_small() {
        return None;
    }
    let top = app.month_top?;
    let position = Position::new(column, row);
    for week in 0..WEEKS {
        for weekday in 0..7 {
            let mut cell = grid.cell(week, weekday + 1);
            if week == WEEKS - 1 {
                cell.height = inner.bottom() - cell.y;
            }
            if cell.contains(position) {
                let days = i64::from(week) * 7 + i64::try_from(weekday).ok()?;
                return Some(Click::Day(top.saturating_add(days.days())));
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use jiff::civil::date;

    #[test]
    fn grid_splits_the_days_evenly_and_the_weeks_equally() {
        // 框裡面 78 × 28（80 × 30 的框）：天的寬度 78 − 5 − 7 = 66，平分 7 欄 = 9 餘 3 → 一、二、三 10 格，其他 9 格
        let grid = Grid::new(Rect::new(1, 1, 78, 28));
        let widths: Vec<u16> = grid.columns.iter().map(|&(_, width)| width).collect();
        assert_eq!(widths, [5, 10, 10, 10, 9, 9, 9, 9]);
        // 欄和欄之間各留 1 格直線：星期一從 1 + 5 + 1 = 7 開始，星期日最後一格貼著框裡面的右邊（78）
        assert_eq!(grid.columns[1].0, 7);
        let (x, width) = grid.columns[7];
        assert_eq!(x + width, 1 + 78);
        // 高：28 − 表頭 2 − 線 5 = 21，6 週各 3 列；第一週從 1 + 2 = 3 開始，下一週隔一條線
        assert_eq!(grid.week_height, 3);
        assert_eq!(grid.week_y(0), 3);
        assert_eq!(grid.week_y(1), 7);
        // 除不盡：27 列 → 20 ÷ 6 = 3 餘 2，零頭留在最下面
        assert_eq!(Grid::new(Rect::new(1, 1, 78, 27)).week_height, 3);
        // 太小：畫不出來
        assert!(Grid::new(Rect::new(1, 1, 18, 28)).too_small());
        assert!(Grid::new(Rect::new(1, 1, 78, 12)).too_small());
        assert!(!Grid::new(Rect::new(1, 1, 19, 13)).too_small());
    }

    #[test]
    fn scrolling_moves_a_week_only_when_the_selection_leaves_the_six_weeks() {
        // 最上面是 9/21：選取的日期在 9/21 ～ 11/1 這 6 週裡就不捲
        let top = date(2026, 9, 21);
        assert_eq!(scrolled(top, date(2026, 9, 21)), top);
        assert_eq!(scrolled(top, date(2026, 11, 1)), top);
        // 往下一週：最上面也往下一週；往上一週：選取那週變成最上面
        assert_eq!(scrolled(top, date(2026, 11, 2)), date(2026, 9, 28));
        assert_eq!(scrolled(top, date(2026, 9, 20)), date(2026, 9, 14));
        // 剛打開：10 月 1 號那週的前一週在最上面
        assert_eq!(month_top(date(2026, 10, 17)), date(2026, 9, 21));
        assert_eq!(month_top(date(2027, 1, 31)), date(2026, 12, 21));
    }
}
