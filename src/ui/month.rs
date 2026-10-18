//! Layout 3: the month grid (like v1's month view): the whole area right of the side menu, 6 rows of
//! equal-height cells, each cell listing that day's items
//!
//! A row is one week, Monday to Sunday, but every month starts a new row (like layout 1's calendar): the
//! week holding Sep 30 and Oct 1 becomes two rows, each leaving the other month's days empty. With grid
//! lines, the line above a new month is in the border color and the line above a new year is thick.
//! 6 rows at a time; scrolls one row at a time only when the selected day leaves the screen (with
//! `[month] center_cursor` on in the config file, the selected row stays on row 3).
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

/// How many rows to show at once
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
    /// Without grid lines, a new year on screen still gets its line: the row it comes after (from 0); one
    /// row of height is taken for it. `None` with grid lines (they already have a line between every two
    /// rows) or when no year starts on screen
    year_line_after: Option<u16>,
}

impl Grid {
    /// Splits the inside of the box (`inner`) into cells; `lines` = whether to draw grid lines
    /// (`[month] grid_lines` in the config file); `new_year_after` = the row a new year starts after, if any
    /// (see `new_year_after`)
    /// (`[month] grid_lines` in the config file)
    ///
    /// Width: the week-number column takes 5 cells; the rest, minus 7 vertical lines, is split evenly over
    /// 7 days. The leftover cells go one each to the columns starting from Monday.
    /// Height: header 1 row + header line 1 row; the rest, minus the 5 lines between weeks, is split evenly
    /// over 6 weeks. Without grid lines, only a new year's line (if any) is subtracted
    fn new(inner: Rect, lines: bool, new_year_after: Option<u16>) -> Self {
        let gap = u16::from(lines);
        let year_line_after = new_year_after.filter(|_| !lines);
        let year_line = u16::from(year_line_after.is_some());
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
            week_height: inner
                .height
                .saturating_sub(1 + gap + (WEEKS - 1) * gap + year_line)
                / WEEKS,
            gap,
            year_line_after,
        }
    }

    /// Cells too small to draw the dates (each day needs at least 1 cell of width, each week at least
    /// 1 row); leftovers go from Monday on, so Sunday is the narrowest
    fn too_small(&self) -> bool {
        self.week_height == 0 || self.columns[7].1 == 0
    }

    /// The first row of week `week` (from 0)
    fn week_y(&self, week: u16) -> u16 {
        // Rows below the extra year line move down by its row
        let year_line = u16::from(self.year_line_after.is_some_and(|after| week > after));
        self.header_y + 1 + self.gap + week * (self.week_height + self.gap) + year_line
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

/// The first day of the row `day` is in: its Monday, or the 1st of its month when the month starts later
/// that week
fn row_start(day: Date) -> Date {
    calendar::monday_of(day).max(day.first_of_month())
}

/// The last day of the row starting on `start`: its Sunday, or the last day of the month if that comes first
fn row_end(start: Date) -> Date {
    calendar::monday_of(start)
        .saturating_add(6.days())
        .min(start.last_of_month())
}

/// The first day of the row `rows` rows after (negative: before) the row starting on `start`
fn row_after(start: Date, rows: i64) -> Date {
    let mut row = start;
    for _ in 0..rows.unsigned_abs() {
        row = if rows > 0 {
            row_end(row).saturating_add(1.day())
        } else {
            row_start(row.saturating_sub(1.day()))
        };
    }
    row
}

/// The first day of the top row on screen: starting from `top`, scroll only when the selected row leaves
/// the 6 rows (one row at a time, like v1). `center` (`[month] center_cursor` in the config file) = ignore
/// `top`; the selected row stays on row 3 of the 6 (with an even count it leans one row up, so one more
/// row shows below)
fn scrolled(top: Date, selected: Date, center: bool) -> Date {
    let row = row_start(selected);
    let last = i64::from(WEEKS - 1);
    if center {
        return row_after(row, -last / 2);
    }
    let top = row_start(top);
    if row < top {
        row
    } else if row > row_after(top, last) {
        row_after(row, -last)
    } else {
        top
    }
}

/// The top row right after opening, after `t` / `[` / `]`, or when switching to layout 3: the last row of
/// the month before `date`'s (so the end of the previous month shows)
fn month_top(date: Date) -> Date {
    row_after(date.first_of_month(), -1)
}

/// The line between two rows (with grid lines on)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Rule {
    /// Within a month: a faint grid line
    Week,
    /// Above a new month: a line in the border color
    Month,
    /// Above a new year (the row starting on that January 1st): a thick line with the year on it
    Year(Date),
}

impl Rule {
    /// The line above the row starting on `start`
    fn above(start: Date) -> Self {
        match (start.month(), start.day()) {
            (1, 1) => Self::Year(start),
            (_, 1) => Self::Month,
            _ => Self::Week,
        }
    }
}

/// Draws the month grid: a rounded box (the border lights up when the keyboard is on the calendar) with
/// the selected day in the middle of the top border; inside are the header (weekday names) + 6 weeks of cells
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
        .title_top(Line::styled(date, Style::new().fg(theme.text)).centered());
    let inner = block.inner(area);
    frame.render_widget(block, area);
    let grid = Grid::new(inner, app.settings.month.grid_lines, new_year_after(top));
    if grid.too_small() {
        return;
    }
    if app.settings.month.grid_lines {
        // The line above each row after the first: faint, the border color above a new month, thick above a
        // new year
        let rules: Vec<Rule> = rows(top)
            .skip(1)
            .take(usize::from(WEEKS - 1))
            .map(Rule::above)
            .collect();
        render_grid_lines(frame.buffer_mut(), inner, &grid, &rules, app, border);
    }
    // Without grid lines a new year still gets its thick line, on the row the grid set aside for it
    if let Some(after) = grid.year_line_after {
        let day = row_after(top, i64::from(after) + 1);
        render_year_line(frame.buffer_mut(), inner, &grid, day, app, border);
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
    for (week, start) in (0..WEEKS).zip(rows(top)) {
        let end = row_end(start);
        let monday = calendar::monday_of(start);
        let days: Vec<Date> = monday.series(1.day()).take(7).collect();
        // Week number: from the row's first day (a week split by a new month shows it on both rows); the
        // row with the selected day gets the `week_number` color, other rows `week_number_dim`.
        // With `[calendar] relative_weeks`, other rows show the difference in weeks
        let week_color = if (start..=end).contains(&app.selected) {
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
                    calendar::week_label(start, app.selected, relative)
                ),
                Style::new().fg(week_color),
            ),
            number_area,
        );
        // Only the row's own days are drawn; the other days of that week stay empty
        let cells: Vec<Rect> = (1..=7).map(|column| grid.cell(week, column)).collect();
        let lines: Vec<Vec<Line>> = days
            .iter()
            .zip(&cells)
            .map(|(&day, cell)| {
                if (start..=end).contains(&day) {
                    cell_lines(day, month, &app.items_on(day), cell.height, app)
                } else {
                    Vec::new()
                }
            })
            .collect();
        render_week(frame, &cells, &lines);
    }
}

/// The first days of the rows from `top` down (as many as asked for)
fn rows(top: Date) -> impl Iterator<Item = Date> {
    std::iter::successors(Some(top), |&start| Some(row_after(start, 1)))
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

/// Draws the grid lines: `│` between columns, `─` below the header and between rows, `┬` / `┼` at
/// crossings; horizontal lines meet the outer border as `├` / `┤`, vertical lines meet the bottom border
/// as `┴` (in the border color). `rules` = the line above each row after the first: faint (`grid`) within
/// a month, the border color (`line`) above a new month, and above a new year a thick `━` (`year_line`,
/// crossings `┿`, meeting the border as `┝` / `┥`) with the year written on it over Monday's cell
/// (`label` color; left out when Monday's cell is too narrow), like layout 1's calendar.
/// For the leftover rows (empty rows below the last row), vertical lines still run to the bottom border,
/// so the last row just looks a few rows taller and no two lines stack up
///
/// Writes cell by cell straight into the `Buffer` (no widget); positions off screen are skipped
fn render_grid_lines(
    buffer: &mut Buffer,
    inner: Rect,
    grid: &Grid,
    rules: &[Rule],
    app: &App,
    border: Style,
) {
    let theme = &app.theme;
    let faint = Style::new().fg(theme.grid);
    let mut put = |x: u16, y: u16, symbol: &str, style: Style| {
        if let Some(cell) = buffer.cell_mut(Position::new(x, y)) {
            cell.set_symbol(symbol).set_style(style);
        }
    };
    let top = grid.header_y + 1;
    for x in grid.vertical_xs() {
        for y in top..inner.bottom() {
            put(x, y, line::VERTICAL, faint);
        }
        put(x, inner.bottom(), line::HORIZONTAL_UP, border);
    }
    // The line below the header, then the one above each row after the first
    let mut lines = vec![(
        top,
        line::HORIZONTAL,
        line::HORIZONTAL_DOWN,
        faint,
        ("├", "┤"),
    )];
    // Years to write on their thick lines: (y, January 1st)
    let mut years = Vec::new();
    for (week, rule) in (0..).zip(rules) {
        let y = grid.week_y(week) + grid.week_height;
        lines.push(match rule {
            Rule::Week => (y, line::HORIZONTAL, line::CROSS, faint, ("├", "┤")),
            Rule::Month => {
                let style = Style::new().fg(theme.line);
                (y, line::HORIZONTAL, line::CROSS, style, ("├", "┤"))
            }
            Rule::Year(day) => {
                years.push((y, *day));
                let style = Style::new().fg(theme.year_line);
                (y, line::THICK_HORIZONTAL, "┿", style, ("┝", "┥"))
            }
        });
    }
    for (y, horizontal, cross, style, (left, right)) in lines {
        for x in inner.x..inner.right() {
            put(x, y, horizontal, style);
        }
        for x in grid.vertical_xs() {
            put(x, y, cross, style);
        }
        put(inner.x.wrapping_sub(1), y, left, border);
        put(inner.right(), y, right, border);
    }
    for (y, day) in years {
        write_year(buffer, grid, y, day, app);
    }
}

/// Without grid lines: the line above a new year (thick `━` in `year_line`, meeting the border as `┝` /
/// `┥`, with the year on it), on the row the grid set aside for it
fn render_year_line(
    buffer: &mut Buffer,
    inner: Rect,
    grid: &Grid,
    day: Date,
    app: &App,
    border: Style,
) {
    let Some(after) = grid.year_line_after else {
        return;
    };
    let y = grid.week_y(after) + grid.week_height;
    let style = Style::new().fg(app.theme.year_line);
    for x in inner.x..inner.right() {
        buffer.set_string(x, y, line::THICK_HORIZONTAL, style);
    }
    buffer.set_string(inner.x.wrapping_sub(1), y, "┝", border);
    buffer.set_string(inner.right(), y, "┥", border);
    write_year(buffer, grid, y, day, app);
}

/// Writes the year of `day` on the thick line at `y`, one line cell in from Monday's left edge:
/// 「━ 2027 ━━━」 (`label` color); left out when Monday's cell cannot keep a line cell on both sides
fn write_year(buffer: &mut Buffer, grid: &Grid, y: u16, day: Date, app: &App) {
    let (monday_x, monday_width) = grid.columns[1];
    let label = format!(" {} ", app.lang.texts().year_label(day));
    let width = u16::try_from(Span::raw(&label).width()).unwrap_or(u16::MAX);
    if monday_width >= width.saturating_add(2) {
        buffer.set_string(monday_x + 1, y, label, Style::new().fg(app.theme.label));
    }
}

/// Which row a new year starts after, among the rows on screen from `top` (e.g. `Some(0)` when the
/// second row starts on January 1st); `None` when no year starts on screen
fn new_year_after(top: Date) -> Option<u16> {
    rows(top)
        .skip(1)
        .take(usize::from(WEEKS - 1))
        .position(|start| matches!(Rule::above(start), Rule::Year(_)))
        .and_then(|row| u16::try_from(row).ok())
}

/// Which day a mouse click at (`column`, `row`) on the month grid (`area`) hits; `None` on the header,
/// the week-number column, grid lines, the outer border or an empty cell of a split week
///
/// The scroll position is `app.month_top`, stored at the last draw; the empty leftover rows below the
/// last week count as the last week
pub fn click_at(area: Rect, app: &App, column: u16, row: u16) -> Option<Click> {
    let inner = Block::bordered().inner(area);
    let top = app.month_top?;
    let grid = Grid::new(inner, app.settings.month.grid_lines, new_year_after(top));
    if grid.too_small() {
        return None;
    }
    let position = Position::new(column, row);
    for (week, start) in (0..WEEKS).zip(rows(top)) {
        for weekday in 0..7 {
            let mut cell = grid.cell(week, weekday + 1);
            if week == WEEKS - 1 {
                cell.height = inner.bottom() - cell.y;
            }
            if cell.contains(position) {
                // A cell left empty because its day belongs to the other month's row: nothing to click
                let day =
                    calendar::monday_of(start).saturating_add(i64::try_from(weekday).ok()?.days());
                return (start..=row_end(start))
                    .contains(&day)
                    .then_some(Click::Day(day));
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
        let grid = Grid::new(Rect::new(1, 1, 78, 28), true, None);
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
        assert_eq!(
            Grid::new(Rect::new(1, 1, 78, 27), true, None).week_height,
            3
        );
        // Too small: cannot be drawn
        assert!(Grid::new(Rect::new(1, 1, 18, 28), true, None).too_small());
        assert!(Grid::new(Rect::new(1, 1, 78, 12), true, None).too_small());
        assert!(!Grid::new(Rect::new(1, 1, 19, 13), true, None).too_small());
    }

    #[test]
    fn without_grid_lines_the_cells_touch() {
        // 78 × 28 without grid lines: day width 78 − 5 = 73, split over 7 columns = 10 remainder 3
        // → Mon, Tue, Wed get 11 cells, the others 10; columns touch
        let grid = Grid::new(Rect::new(1, 1, 78, 28), false, None);
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
        assert!(!Grid::new(Rect::new(1, 1, 78, 12), false, None).too_small());
    }

    #[test]
    fn without_grid_lines_a_new_year_takes_one_row_for_its_line() {
        // 78 × 28 without grid lines, a new year after the first row: 28 − header 1 − year line 1 = 26 →
        // 4 rows each (26 / 6 = 4 remainder 2); row 1 at y 2, the year line at y 6, row 2 at y 7, row 3 at y 11
        let grid = Grid::new(Rect::new(1, 1, 78, 28), false, Some(0));
        assert_eq!(grid.week_height, 4);
        assert_eq!([0, 1, 2].map(|week| grid.week_y(week)), [2, 7, 11]);
        // With grid lines there is already a line between every two rows: nothing changes
        assert_eq!(
            Grid::new(Rect::new(1, 1, 78, 28), true, Some(0)),
            Grid::new(Rect::new(1, 1, 78, 28), true, None)
        );
        // Rows from 12/28: the second one starts on 1/1; rows from 12/7: the fifth; rows from 9/28: none
        assert_eq!(new_year_after(date(2026, 12, 28)), Some(0));
        assert_eq!(new_year_after(date(2026, 12, 7)), Some(3));
        assert_eq!(new_year_after(date(2026, 9, 28)), None);
    }

    #[test]
    fn every_month_starts_a_new_row() {
        // 2026-10-01 is a Thursday: the week of 9/28 splits into 9/28-9/30 and 10/1-10/4
        assert_eq!(row_start(date(2026, 9, 30)), date(2026, 9, 28));
        assert_eq!(row_start(date(2026, 10, 3)), date(2026, 10, 1));
        assert_eq!(row_end(date(2026, 9, 28)), date(2026, 9, 30));
        assert_eq!(row_end(date(2026, 10, 1)), date(2026, 10, 4));
        // 11/1 is a Sunday: a row of its own between 10/26-10/31 and 11/2-11/8
        assert_eq!(row_after(date(2026, 10, 26), 1), date(2026, 11, 1));
        assert_eq!(row_after(date(2026, 11, 1), 1), date(2026, 11, 2));
        assert_eq!(row_after(date(2026, 11, 2), -2), date(2026, 10, 26));
        // 2027-02-01 is a Monday: the month starts a whole week, no extra row
        assert_eq!(row_after(date(2027, 1, 25), 1), date(2027, 2, 1));
        // The line above a row: thick before 1/1, the border color before any other 1st, faint otherwise
        assert_eq!(Rule::above(date(2027, 1, 1)), Rule::Year(date(2027, 1, 1)));
        assert_eq!(Rule::above(date(2026, 10, 1)), Rule::Month);
        assert_eq!(Rule::above(date(2026, 10, 5)), Rule::Week);
    }

    #[test]
    fn scrolling_moves_a_row_only_when_the_selection_leaves_the_six_rows() {
        // Top is 9/28: rows 9/28-30, 10/1-4, 10/5-11, 10/12-18, 10/19-25, 10/26-31; no scrolling inside them
        let top = date(2026, 9, 28);
        assert_eq!(scrolled(top, date(2026, 9, 28), false), top);
        assert_eq!(scrolled(top, date(2026, 10, 31), false), top);
        // One row further down (11/1): the top moves down one row (to 10/1); one row up (9/27, in the row
        // 9/21-27): that row becomes the top
        assert_eq!(scrolled(top, date(2026, 11, 1), false), date(2026, 10, 1));
        assert_eq!(scrolled(top, date(2026, 9, 27), false), date(2026, 9, 21));
        // A stored top that is not the start of a row counts as the row it is in
        assert_eq!(
            scrolled(date(2026, 10, 3), date(2026, 10, 17), false),
            date(2026, 10, 1)
        );
        // Centered: the selected row (10/12-18) is always row 3, so the top is two rows up (10/1)
        for top in [date(2026, 9, 21), date(2026, 10, 12), date(2027, 1, 4)] {
            assert_eq!(scrolled(top, date(2026, 10, 17), true), date(2026, 10, 1));
        }
        // Just opened: the last row of the previous month is at the top
        assert_eq!(month_top(date(2026, 10, 17)), date(2026, 9, 28));
        assert_eq!(month_top(date(2027, 1, 31)), date(2026, 12, 28));
    }
}
