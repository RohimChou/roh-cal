//! Layout 3: the month grid (like v1's month view): the whole area right of the side menu, 6 weeks of
//! equal-height cells, each cell listing that day's items
//!
//! 6 weeks at a time; scrolls one week at a time only when the selected day leaves the screen (with
//! `[month] center_cursor` on in the config file, the selected week stays on row 3).
//! Cell sizes are computed from the current space on every draw (they follow the terminal as it resizes)

use super::{
    Click,
    planner::{day_color, in_multi_day},
};
use crate::{
    app::{App, Focus},
    calendar,
    tasks::Item,
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

/// How many weeks to show at once
const WEEKS: u16 = 6;
/// Width of the week-number column on the left: 「  42 」
const WEEK_COLUMN_WIDTH: u16 = 5;

/// Where the month grid's cells are on screen (grid lines excluded). Used both for drawing and for mapping
/// mouse positions, so the two always agree
#[derive(Debug, PartialEq)]
struct Grid {
    /// (x, width) of the week-number column and each of the 7 days: `[週數欄, 一, 二, …, 日]`; with grid
    /// lines, 1 cell is left between columns for the vertical line
    columns: [(u16, u16); 8],
    /// The header row (weekday names); with grid lines, the next row is the header's line
    header_y: u16,
    /// Rows per week: all 6 weeks are equally tall; the leftover rows stay at the bottom
    week_height: u16,
    /// Cells taken by a grid line: 1 with grid lines (between columns, below the header, between weeks),
    /// 0 without (cells touch)
    gap: u16,
}

impl Grid {
    /// Splits the inside of the box (`inner`) into cells; `lines` = whether to draw grid lines
    /// (`[month] grid_lines` in the config file)
    ///
    /// Width: the week-number column takes 5 cells; the rest, minus 7 vertical lines, is split evenly over
    /// 7 days. The leftover cells go one each to the columns starting from Monday.
    /// Height: header 1 row + header line 1 row; the rest, minus the 5 lines between weeks, is split evenly
    /// over 6 weeks. Without grid lines, no lines are subtracted
    fn new(inner: Rect, lines: bool) -> Self {
        let gap = u16::from(lines);
        let days_width = inner.width.saturating_sub(WEEK_COLUMN_WIDTH + 7 * gap);
        let (base, extra) = (days_width / 7, days_width % 7);
        let mut columns = [(inner.x, WEEK_COLUMN_WIDTH.min(inner.width)); 8];
        let mut x = inner.x.saturating_add(WEEK_COLUMN_WIDTH + gap);
        for (weekday, column) in (0..7).zip(columns.iter_mut().skip(1)) {
            let width = base + u16::from(weekday < extra);
            *column = (x, width);
            x = x.saturating_add(width + gap);
        }
        Self {
            columns,
            header_y: inner.y,
            week_height: inner.height.saturating_sub(1 + gap + (WEEKS - 1) * gap) / WEEKS,
            gap,
        }
    }

    /// Cells too small to draw the dates (each day needs at least 1 cell of width, each week at least
    /// 1 row); leftovers go from Monday on, so Sunday is the narrowest
    fn too_small(&self) -> bool {
        self.week_height == 0 || self.columns[7].1 == 0
    }

    /// The first row of week `week` (from 0)
    fn week_y(&self, week: u16) -> u16 {
        self.header_y + 1 + self.gap + week * (self.week_height + self.gap)
    }

    /// The cell at week `week`, column `column` (0 = week-number column, 1 = Monday … 7 = Sunday)
    fn cell(&self, week: u16, column: usize) -> Rect {
        let (x, width) = self.columns[column];
        Rect::new(x, self.week_y(week), width, self.week_height)
    }

    /// x of the vertical lines: right of the week-number column and right of Monday through Saturday
    fn vertical_xs(&self) -> impl Iterator<Item = u16> + '_ {
        self.columns[..7].iter().map(|&(x, width)| x + width)
    }
}

/// Monday of the top week on screen: starting from `top`, scroll only when the selected week leaves the
/// 6 weeks (one week at a time, like v1). `center` (`[month] center_cursor` in the config file) = ignore
/// `top`; the selected week stays on row 3 of the 6 (with an even count it leans one row up, so one more
/// week shows below)
fn scrolled(top: Date, selected: Date, center: bool) -> Date {
    let week = calendar::monday_of(selected);
    if center {
        return week.saturating_sub(i64::from((WEEKS - 1) / 2).weeks());
    }
    let last = top.saturating_add(i64::from(WEEKS - 1).weeks());
    if week < top {
        week
    } else if week > last {
        week.saturating_sub(i64::from(WEEKS - 1).weeks())
    } else {
        top
    }
}

/// The top week right after opening, after `t` / `[` / `]`, or when switching to layout 3: the week
/// before the one with the 1st of `date`'s month (so the end of the previous month shows)
fn month_top(date: Date) -> Date {
    calendar::monday_of(date.first_of_month()).saturating_sub(1.week())
}

/// Draws the month grid: a rounded box (the border lights up when the keyboard is on the calendar) with
/// the selected day in the top-right corner; inside are the header (weekday names) + 6 weeks of cells
///
/// First scroll so the selected week is visible (stored in `app.month_top`), then draw
pub fn render(frame: &mut Frame, area: Rect, app: &mut App) {
    let top = scrolled(
        app.month_top.unwrap_or_else(|| month_top(app.selected)),
        app.selected,
        app.settings.month.center_cursor,
    );
    app.month_top = Some(top);
    // Read-only from here on
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
    let grid = Grid::new(inner, app.settings.month.grid_lines);
    if grid.too_small() {
        return;
    }
    if app.settings.month.grid_lines {
        render_grid_lines(frame.buffer_mut(), inner, &grid, theme.grid, border);
    }

    // Header: the week-number column says 「週」; each day's name sits at the far left of its column, lined
    // up with the dates below; Saturday and Sunday use the weekend color
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

    // The selected month: days of other months are grayed out
    let month = app.selected.first_of_month();
    for week in 0..WEEKS {
        let monday = top.saturating_add(i64::from(week).weeks());
        let days: Vec<Date> = monday.series(1.day()).take(7).collect();
        // Week number: from a day of that week inside the selected month (only differs for the week spanning
        // the new year); the selected week gets the `week_number` color, other weeks `week_number_dim`.
        // With `[calendar] relative_weeks`, other weeks show the difference in weeks
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
        let relative = app.settings.calendar.relative_weeks;
        frame.render_widget(
            Line::styled(
                format!(
                    " {:>3} ",
                    calendar::week_label(numbered, app.selected, relative)
                ),
                Style::new().fg(week_color),
            ),
            number_area,
        );
        let cells: Vec<Rect> = (1..=7).map(|column| grid.cell(week, column)).collect();
        let lines: Vec<Vec<Line>> = days
            .iter()
            .zip(&cells)
            .map(|(&day, cell)| cell_lines(day, month, &app.items_on(day), cell.height, app))
            .collect();
        render_week(frame, &cells, &lines);
    }
}

/// Draws the 7 cells of one week, line by line
///
/// Text below the first line (the date) that is too long **borrows to the right** from neighbor cells
/// (like a spreadsheet): if the neighbor has nothing on that line, its width is borrowed (drawing over
/// the grid line between), and so on until a cell with something on that line, or Sunday; anything
/// longer is cut off
fn render_week(frame: &mut Frame, cells: &[Rect], lines: &[Vec<Line>]) {
    for (column, (cell, cell_lines)) in cells.iter().zip(lines).enumerate() {
        for (row, (line, y)) in cell_lines.iter().zip(cell.y..cell.bottom()).enumerate() {
            let mut right = cell.right();
            if row > 0 {
                for (next, next_lines) in cells.iter().zip(lines).skip(column + 1) {
                    if next_lines.len() > row {
                        break;
                    }
                    right = next.right();
                }
            }
            frame.render_widget(line, Rect::new(cell.x, y, right - cell.x, 1));
        }
    }
}

/// The lines of one cell: the date first (see `date_line`), then one item per line,
/// e.g. 「 ∙ 14:00 Code review」
///
/// - List all items if they fit; otherwise list one fewer and put `+N` on the last line
///   (N = the number of items not listed)
/// - Cell only 1 line tall (very short terminal): no items; dots after the date, one per item (at most 4)
///
/// Each piece of text carries its color on its `Span` (not `Line::styled`), so when a long title borrows
/// the empty neighbor cells to the right, the neighbor's grid line does not get colored too
fn cell_lines(day: Date, month: Date, items: &[&Item], rows: u16, app: &App) -> Vec<Line<'static>> {
    let theme = &app.theme;
    let other_month = day.first_of_month() != month;
    let mut first = date_line(day, month, app);
    let rows = usize::from(rows);
    if rows <= 1 {
        // Dots: the day's color if some item is not over yet, muted if all are over; gray for other months
        let color = if other_month {
            theme.other_month
        } else if items.iter().any(|item| item.is_open(app.today)) {
            day_color(day, app)
        } else {
            theme.muted
        };
        first.push_span(Span::styled(dots(items.len()), Style::new().fg(color)));
        return vec![first];
    }
    let available = rows - 1;
    let shown = if items.len() <= available {
        items.len()
    } else {
        available - 1
    };
    let mut lines = vec![first];
    lines.extend(
        items[..shown]
            .iter()
            .map(|item| item_line(item, other_month, app)),
    );
    if shown < items.len() {
        let color = if other_month {
            theme.other_month
        } else {
            theme.muted
        };
        lines.push(Line::from(Span::styled(
            format!(" +{}", items.len() - shown),
            Style::new().fg(color),
        )));
    }
    lines
}

/// One item in a cell: 「 ∙ 14:00 標題」 (time only if it has one; with `[month] item_icons = false` in
/// the config file there is no dot: 「 14:00 標題」)
///
/// Same as layout 1's agenda: items not over yet use that day's calendar color, with the time muted;
/// items that are over are gray for the whole line (done tasks also get a struck-through title);
/// items from other months are gray for the whole line
fn item_line(item: &Item, other_month: bool, app: &App) -> Line<'static> {
    let theme = &app.theme;
    let color = if other_month {
        theme.other_month
    } else if item.is_open(app.today) {
        item.date.map_or(theme.text, |date| day_color(date, app))
    } else {
        theme.muted
    };
    let time_color = if other_month {
        theme.other_month
    } else {
        theme.muted
    };
    let icon = if app.settings.month.item_icons {
        " ∙ "
    } else {
        " "
    };
    let mut spans = vec![Span::styled(icon, Style::new().fg(color))];
    if let Some(time) = item.time {
        spans.push(Span::styled(
            format!("{} ", time.strftime("%H:%M")),
            Style::new().fg(time_color),
        ));
    }
    let mut title = Style::new().fg(color);
    if item.done {
        title = title.add_modifier(Modifier::CROSSED_OUT);
    }
    spans.push(Span::styled(item.title.clone(), title));
    Line::from(spans)
}

/// One dot per item that day (at most 4): `⠂` `⠒` `⠖` `⠶` (these braille symbols are all 1 cell wide)
fn dots(count: usize) -> &'static str {
    match count {
        0 => "",
        1 => "⠂",
        2 => "⠒",
        3 => "⠖",
        _ => "⠶",
    }
}

/// A cell's first line: the date on the left (the selected one in brackets `[17]`), with the month after
/// the 1st, e.g. 「 1 10月」
///
/// Color: today > other months (gray) > same as layout 1's calendar (`day_color`: holiday, unfinished
/// items, weekend). Dates in the selected month are bold;
/// days covered by a multi-day item are underlined
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
    // Underline days covered by a multi-day item (same as layout 1's calendar)
    if in_multi_day(day, app) {
        number = number.add_modifier(Modifier::UNDERLINED);
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

/// Draws faint grid lines: `│` between columns, `─` below the header and between weeks, `┬` / `┼` at
/// crossings; horizontal lines meet the outer border as `├` / `┤`, vertical lines meet the bottom border
/// as `┴` (in the border color).
/// For the leftover rows (empty rows below the last week), vertical lines still run to the bottom border,
/// so the last week just looks a few rows taller and no two lines stack up
///
/// Writes cell by cell straight into the `Buffer` (no widget); positions off screen are skipped
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
    // The line below the header and the 5 between weeks
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

/// Which day a mouse click at (`column`, `row`) on the month grid (`area`) hits; `None` on the header,
/// the week-number column, grid lines or the outer border
///
/// The scroll position is `app.month_top`, stored at the last draw; the empty leftover rows below the
/// last week count as the last week
pub fn click_at(area: Rect, app: &App, column: u16, row: u16) -> Option<Click> {
    let inner = Block::bordered().inner(area);
    let grid = Grid::new(inner, app.settings.month.grid_lines);
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
        // Inside 78 × 28 (an 80 × 30 box): day width 78 − 5 − 7 = 66, split over 7 columns = 9 remainder 3
        // → Mon, Tue, Wed get 10 cells, the others 9
        let grid = Grid::new(Rect::new(1, 1, 78, 28), true);
        let widths: Vec<u16> = grid.columns.iter().map(|&(_, width)| width).collect();
        assert_eq!(widths, [5, 10, 10, 10, 9, 9, 9, 9]);
        // 1 cell for a vertical line between columns: Monday starts at 1 + 5 + 1 = 7, Sunday's last cell
        // touches the right edge of the inside (78)
        assert_eq!(grid.columns[1].0, 7);
        let (x, width) = grid.columns[7];
        assert_eq!(x + width, 1 + 78);
        // Height: 28 − header 2 − lines 5 = 21, 3 rows for each of 6 weeks; week 1 starts at 1 + 2 = 3,
        // the next week one line further
        assert_eq!(grid.week_height, 3);
        assert_eq!(grid.week_y(0), 3);
        assert_eq!(grid.week_y(1), 7);
        // Not evenly divisible: 27 rows → 20 ÷ 6 = 3 remainder 2; the leftover stays at the bottom
        assert_eq!(Grid::new(Rect::new(1, 1, 78, 27), true).week_height, 3);
        // Too small: cannot be drawn
        assert!(Grid::new(Rect::new(1, 1, 18, 28), true).too_small());
        assert!(Grid::new(Rect::new(1, 1, 78, 12), true).too_small());
        assert!(!Grid::new(Rect::new(1, 1, 19, 13), true).too_small());
    }

    #[test]
    fn without_grid_lines_the_cells_touch() {
        // 78 × 28 without grid lines: day width 78 − 5 = 73, split over 7 columns = 10 remainder 3
        // → Mon, Tue, Wed get 11 cells, the others 10; columns touch
        let grid = Grid::new(Rect::new(1, 1, 78, 28), false);
        let widths: Vec<u16> = grid.columns.iter().map(|&(_, width)| width).collect();
        assert_eq!(widths, [5, 11, 11, 11, 10, 10, 10, 10]);
        assert_eq!(grid.columns[1].0, 1 + 5);
        let (x, width) = grid.columns[7];
        assert_eq!(x + width, 1 + 78);
        // Height: 28 − header 1 = 27, 4 rows for each of 6 weeks; week 1 sits right below the header,
        // the next week right after
        assert_eq!(grid.week_height, 4);
        assert_eq!(grid.week_y(0), 2);
        assert_eq!(grid.week_y(1), 6);
        // A height too short with grid lines can be drawn without them: 12 − 1 = 11 → 1 row per week
        assert!(!Grid::new(Rect::new(1, 1, 78, 12), false).too_small());
    }

    #[test]
    fn scrolling_moves_a_week_only_when_the_selection_leaves_the_six_weeks() {
        // Top is 9/21: no scrolling while the selected day is within the 6 weeks 9/21 to 11/1
        let top = date(2026, 9, 21);
        assert_eq!(scrolled(top, date(2026, 9, 21), false), top);
        assert_eq!(scrolled(top, date(2026, 11, 1), false), top);
        // One week further down: the top moves down a week too; one week up: the selected week becomes the top
        assert_eq!(scrolled(top, date(2026, 11, 2), false), date(2026, 9, 28));
        assert_eq!(scrolled(top, date(2026, 9, 20), false), date(2026, 9, 14));
        // Centered: the selected week (10/12) is always on row 3, whatever the previous top was
        for top in [date(2026, 9, 21), date(2026, 10, 12), date(2027, 1, 4)] {
            assert_eq!(scrolled(top, date(2026, 10, 17), true), date(2026, 9, 28));
        }
        // Just opened: the week before the one with Oct 1 is at the top
        assert_eq!(month_top(date(2026, 10, 17)), date(2026, 9, 21));
        assert_eq!(month_top(date(2027, 1, 31)), date(2026, 12, 21));
    }
}
