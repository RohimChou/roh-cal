//! Layout 2: the month grid (like v1's month view): the whole area right of the side menu, 6 rows of
//! equal-height cells, each cell listing that day's items
//!
//! A row is one whole week, Monday to Sunday: the week holding Sep 30 and Oct 1 is one row with days of
//! both months, and with grid lines a staircase line marks where one month ends. 6 weeks at a time; scrolls one week at a time only when the selected day leaves the screen
//! (with `[month] center_cursor` on in the config file, the selected week stays on row 3). The rows left
//! over at the bottom show the weeks after, as many as start there, the last one cut off by the border.
//! Cell sizes are computed from the current space on every draw (they follow the terminal as it resizes)

use super::{
    Click,
    planner::{day_color, in_multi_day, open_item_color},
    title_span,
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
    style::{Color, Modifier, Style},
    symbols::line,
    text::{Line, Span},
    widgets::{Block, BorderType, Clear},
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
    /// Cells taken by a grid line between columns and below the header: 1 with grid lines, 0 without
    /// (cells touch)
    gap: u16,
    /// Rows taken by the line between week k and week k + 1 (one for each two weeks drawn): with grid lines
    /// always 1; without, 1 only where a month ends between the two weeks (its edge goes there, see
    /// `render_month_edges`), else 0
    week_gaps: Vec<u16>,
    /// How many weeks are drawn: the 6, and after them in the rows left over at the bottom as many more as
    /// have room for the line before them (if any) and their first row; the last one is cut off by the
    /// bottom border
    weeks: u16,
    /// The row just below the inside of the box: cells end here at the latest
    bottom: u16,
}

impl Grid {
    /// Splits the inside of the box (`inner`) into cells for the weeks starting on `top`; `lines` = whether
    /// to draw grid lines (`[month] grid_lines` in the config file)
    ///
    /// Width: the week-number column takes 5 cells; the rest, minus 7 vertical lines, is split evenly over
    /// 7 days. The leftover cells go one each to the columns starting from Monday.
    /// Height: header 1 row + header line 1 row; the rest, minus the 5 lines between weeks, is split evenly
    /// over 6 weeks. Without grid lines only the header and the rows for a month's edge are subtracted
    fn new(inner: Rect, lines: bool, top: Date) -> Self {
        let gap = u16::from(lines);
        let gap_after = |week: u16| {
            u16::from(lines || (1..=7).any(|column| rule_under(top, week, column) != Rule::Week))
        };
        let days_width = inner.width.saturating_sub(WEEK_COLUMN_WIDTH + 7 * gap);
        let (base, extra) = (days_width / 7, days_width % 7);
        let mut columns = [(inner.x, WEEK_COLUMN_WIDTH.min(inner.width)); 8];
        let mut x = inner.x.saturating_add(WEEK_COLUMN_WIDTH + gap);
        for (weekday, column) in (0..7).zip(columns.iter_mut().skip(1)) {
            let width = base + u16::from(weekday < extra);
            *column = (x, width);
            x = x.saturating_add(width + gap);
        }
        let week_gaps: Vec<u16> = (0..WEEKS - 1).map(gap_after).collect();
        let lines_between: u16 = week_gaps.iter().sum();
        let mut grid = Self {
            columns,
            header_y: inner.y,
            week_height: inner.height.saturating_sub(1 + gap + lines_between) / WEEKS,
            gap,
            week_gaps,
            weeks: WEEKS,
            bottom: inner.bottom(),
        };
        // The rows left over at the bottom: the weeks after, as long as one more starts above the bottom
        while grid.week_height > 0 {
            grid.week_gaps.push(gap_after(grid.weeks - 1));
            if grid.week_y(grid.weeks) >= grid.bottom {
                grid.week_gaps.pop();
                break;
            }
            grid.weeks += 1;
        }
        grid
    }

    /// Cells too small to draw the dates (each day needs at least 1 cell of width, each week at least
    /// 1 row); leftovers go from Monday on, so Sunday is the narrowest
    fn too_small(&self) -> bool {
        self.week_height == 0 || self.columns[7].1 == 0
    }

    /// The first row of week `week` (from 0)
    fn week_y(&self, week: u16) -> u16 {
        let lines: u16 = self.week_gaps.iter().take(usize::from(week)).sum();
        self.header_y + 1 + self.gap + week * self.week_height + lines
    }

    /// The cell at week `week`, column `column` (0 = week-number column, 1 = Monday … 7 = Sunday); the
    /// cut-off week's cells end at the bottom of the box
    fn cell(&self, week: u16, column: usize) -> Rect {
        let (x, width) = self.columns[column];
        let y = self.week_y(week);
        Rect::new(
            x,
            y,
            width,
            self.week_height.min(self.bottom.saturating_sub(y)),
        )
    }

    /// The last week drawn: the rows left below it count as part of it
    fn last_week(&self) -> u16 {
        self.weeks - 1
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
    let last = i64::from(WEEKS - 1);
    if center {
        return week.saturating_sub((last / 2).weeks());
    }
    let top = calendar::monday_of(top);
    if week < top {
        week
    } else if week > top.saturating_add(last.weeks()) {
        week.saturating_sub(last.weeks())
    } else {
        top
    }
}

/// The top week right after opening, after `t` / `[` / `]`, or when switching to layout 2: the week with
/// the 1st of `date`'s month (a month takes at most 6 weeks, so all of it shows)
fn month_top(date: Date) -> Date {
    calendar::monday_of(date.first_of_month())
}

/// Draws the month grid: a rounded box (the border lights up when the keyboard is on the calendar) with
/// the selected day in the middle of the top border; inside are the header (weekday names) + 6 weeks of cells
///
/// First scroll so the selected week is visible (stored in `app.month_top`), then draw; after the wheel
/// (`app.month_free`) the 6 weeks stay put until the selection changes
pub fn render(frame: &mut Frame, area: Rect, app: &mut App) {
    let top = app.month_top.unwrap_or_else(|| month_top(app.selected));
    let top = if app.month_free == Some(app.selected) {
        top
    } else {
        app.month_free = None;
        scrolled(top, app.selected, app.settings.month.center_cursor)
    };
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
    let grid = Grid::new(inner, app.settings.month.grid_lines, top);
    if grid.too_small() {
        return;
    }
    if app.settings.month.grid_lines {
        render_grid_lines(frame.buffer_mut(), inner, &grid, top, app, border);
    }

    // Header, in the normal text color: the week-number column says 「週」; each day's name sits at the far
    // left of its column, lined up with the dates below; Saturday and Sunday use the weekend color
    let normal = Style::new().fg(theme.text);
    let header = |column: usize| {
        let (x, width) = grid.columns[column];
        Rect::new(x, grid.header_y, width, 1)
    };
    frame.render_widget(
        Line::styled(format!(" {}", texts.header_week), normal),
        header(0),
    );
    for (column, name) in (1..).zip(texts.weekdays) {
        let style = if column >= 6 {
            Style::new().fg(theme.weekend)
        } else {
            normal
        };
        frame.render_widget(Line::styled(format!(" {name}"), style), header(column));
    }

    // The selected month: days of other months are grayed out
    let month = app.selected.first_of_month();
    for week in 0..grid.weeks {
        let monday = top.saturating_add(i64::from(week).weeks());
        let days: Vec<Date> = monday.series(1.day()).take(7).collect();
        // Week number: from a day of that week in the selected month (only differs for the week across the
        // new year), from Monday if it has none; the selected week gets the `week_number` color, other
        // weeks `week_number_dim`. With `[calendar] relative_weeks`, other weeks show the difference in weeks
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
        // The week with a 1st: its month under the week number, and in January the year below that
        if let Some(&first) = days.iter().find(|day| day.day() == 1) {
            let column = grid.cell(week, 0);
            let mut labels = vec![texts.month_label(first)];
            if first.month() == 1 {
                labels.push(first.year().to_string());
            }
            for (label, y) in labels.into_iter().zip(column.y + 1..column.bottom()) {
                frame.render_widget(
                    Line::styled(week_column_text(&label), Style::new().fg(theme.label)),
                    Rect {
                        y,
                        height: 1,
                        ..column
                    },
                );
            }
        }
        let cells: Vec<Rect> = (1..=7).map(|column| grid.cell(week, column)).collect();
        // The selected day's cell gets a background, top-left to bottom-right (like v1)
        let selected = days.iter().position(|&day| day == app.selected);
        if let Some(column) = selected {
            frame
                .buffer_mut()
                .set_style(cells[column], Style::new().bg(theme.selection));
        }
        let (bars, slots) = layout_row(&days, app);
        // Laid out as a whole week even when cut off at the bottom (the cut-off part just does not show)
        let lines: Vec<Vec<Line>> = days
            .iter()
            .zip(&slots)
            .map(|(&day, slots)| cell_lines(day, month, slots, grid.week_height, app))
            .collect();
        render_week(frame, &cells, &lines, &days, selected);
        let shown: Vec<usize> = slots
            .iter()
            .map(|slots| shown_lines(slots.len(), grid.week_height))
            .collect();
        render_bars(frame, &cells, &bars, &shown, &days, app);
    }
    if !app.settings.month.grid_lines {
        render_month_edges(frame.buffer_mut(), inner, &grid, top, app, border);
    }
}

/// Text for the week-number column, lined up with the week numbers 「  42 」: right-aligned to end in the
/// column's 4th cell (e.g. 「 Nov」, 「 1月」); wider than 3 cells starts right after the first blank
/// (「 11月」, 「 2027」)
fn week_column_text(text: &str) -> String {
    let width = Span::raw(text).width();
    format!(" {}{text}", " ".repeat(3usize.saturating_sub(width)))
}

/// What one line of a cell, below the date, holds
#[derive(Debug, Clone, Copy)]
enum Slot<'a> {
    /// A multi-day item's bar (drawn across its days afterwards, see `render_bars`)
    Bar(&'a Item),
    /// A one-day item
    Item(&'a Item),
    /// Nothing: the line is kept free because a bar on another day of the row uses it
    Empty,
}

/// A multi-day item's bar in one row: the line it is on (`lane`; 0 = the first line under the date) and the
/// columns it covers (0 = Monday)
struct Bar<'a> {
    item: &'a Item,
    lane: usize,
    first: usize,
    last: usize,
    /// The item starts on the bar's first day (not a row before): the time is written there
    starts: bool,
}

/// Lays out one row's items (`days` = Monday to Sunday): every multi-day item covering some of the days
/// gets a line of its own on all of them (`Bar::lane`), so its days line up into one bar; the earlier it
/// starts in the row, the higher its line, and a line is used again once free. Each day's one-day items
/// then fill that day's free lines from the top (in `App::items_on` order: in progress first), then go below
///
/// Returns the bars and, for each day, what each line under its date holds
fn layout_row<'a>(days: &[Date], app: &'a App) -> (Vec<Bar<'a>>, Vec<Vec<Slot<'a>>>) {
    let on_day: Vec<Vec<&Item>> = days.iter().map(|&day| app.items_on(day)).collect();
    let mut bars: Vec<Bar> = Vec::new();
    for (column, items) in on_day.iter().enumerate() {
        for &item in items {
            if item.end.is_none() || bars.iter().any(|bar| std::ptr::eq(bar.item, item)) {
                continue;
            }
            let last = (column..on_day.len())
                .take_while(|&next| on_day[next].iter().any(|&other| std::ptr::eq(other, item)))
                .last()
                .unwrap_or(column);
            let lane = (0..)
                .find(|&lane| {
                    !bars
                        .iter()
                        .any(|bar| bar.lane == lane && bar.first <= last && column <= bar.last)
                })
                .unwrap_or(0);
            bars.push(Bar {
                item,
                lane,
                first: column,
                last,
                starts: item.date == Some(days[column]),
            });
        }
    }
    let slots = on_day
        .iter()
        .enumerate()
        .map(|(column, items)| {
            let mut slots: Vec<Slot> = Vec::new();
            for bar in bars
                .iter()
                .filter(|bar| bar.first <= column && column <= bar.last)
            {
                if slots.len() <= bar.lane {
                    slots.resize(bar.lane + 1, Slot::Empty);
                }
                slots[bar.lane] = Slot::Bar(bar.item);
            }
            for &item in items.iter().filter(|item| item.end.is_none()) {
                match slots.iter_mut().find(|slot| matches!(slot, Slot::Empty)) {
                    Some(slot) => *slot = Slot::Item(item),
                    None => slots.push(Slot::Item(item)),
                }
            }
            slots
        })
        .collect();
    (bars, slots)
}

/// How many of a cell's `slots` lines under the date it shows, `rows` tall: all if they fit; otherwise one
/// fewer, and `+N` goes on the last line; none in a 1-line cell (dots instead)
fn shown_lines(slots: usize, rows: u16) -> usize {
    let available = usize::from(rows).saturating_sub(1);
    if slots <= available {
        slots
    } else {
        available.saturating_sub(1)
    }
}

/// Draws the row's bars over its cells: each bar is one band on its line in the `range` background, from
/// its first day in the row to its last (over the grid lines between), with the title written once at the
/// start; a day whose cell cannot show that line (it shows `+N` there instead) cuts the band, and so does
/// a new month (the title is written again after it). A band outside the selected month has gray text
fn render_bars(
    frame: &mut Frame,
    cells: &[Rect],
    bars: &[Bar],
    shown: &[usize],
    days: &[Date],
    app: &App,
) {
    for bar in bars {
        let Ok(lane) = u16::try_from(bar.lane) else {
            continue;
        };
        let mut column = bar.first;
        while column <= bar.last {
            if bar.lane >= shown[column] {
                column += 1;
                continue;
            }
            // A run of days in one month that all show the bar's line
            let first = column;
            let first_month = days[first].first_of_month();
            while column < bar.last
                && bar.lane < shown[column + 1]
                && days[column + 1].first_of_month() == first_month
            {
                column += 1;
            }
            let (from, to) = (cells[first], cells[column]);
            let area = Rect::new(from.x, from.y + 1 + lane, to.right() - from.x, 1);
            // The cut-off week at the bottom: that line is below the box
            if area.y >= from.bottom() {
                column += 1;
                continue;
            }
            let time = bar.starts && first == bar.first;
            // Clear first: the grid lines under the band would otherwise still show through
            frame.render_widget(Clear, area);
            frame.render_widget(bar_line(bar.item, time, app), area);
            column += 1;
        }
    }
}

/// A multi-day item's band: 「 09:00 New Year trip」 on the `range` background (the whole band, set on the
/// `Line`); `time` = write the time (only where the item starts). The title is colored like layout 1's
/// agenda (`open_item_color`: a priority's or a holiday's color for the whole band's text, otherwise the
/// normal color with the time muted), gray once the item is over (struck through when a done task); other
/// months look the same (the grid lines between months already set them apart)
fn bar_line(item: &Item, time: bool, app: &App) -> Line<'static> {
    let theme = &app.theme;
    let (color, time_color) = item_colors(item, app);
    let mut spans = vec![Span::raw(" ")];
    if let Some(at) = item.time.filter(|_| time) {
        spans.push(Span::styled(
            format!("{} ", at.strftime("%H:%M")),
            Style::new().fg(time_color),
        ));
    }
    spans.push(title_span(item, color));
    Line::from(spans).style(Style::new().bg(theme.range))
}

/// Draws the 7 cells of one week (`days`, Monday to Sunday), line by line; `selected` = the column
/// (0 = Monday) of the selected day's cell, if it is in this row
///
/// Text below the first line (the date) that is too long **borrows to the right** from neighbor cells
/// (like a spreadsheet): if the neighbor has nothing on that line, its width is borrowed (drawing over
/// the grid line between), and so on until a cell with something on that line, the selected day's cell
/// (its background stays clean), a new month, or Sunday; anything longer is cut off
fn render_week(
    frame: &mut Frame,
    cells: &[Rect],
    lines: &[Vec<Line>],
    days: &[Date],
    selected: Option<usize>,
) {
    for (column, (cell, cell_lines)) in cells.iter().zip(lines).enumerate() {
        for (row, (line, y)) in cell_lines.iter().zip(cell.y..cell.bottom()).enumerate() {
            let mut right = cell.right();
            if row > 0 {
                for (next_column, (next, next_lines)) in
                    cells.iter().zip(lines).enumerate().skip(column + 1)
                {
                    if next_lines.len() > row
                        || selected == Some(next_column)
                        || days[next_column].first_of_month() != days[column].first_of_month()
                    {
                        break;
                    }
                    right = next.right();
                }
            }
            frame.render_widget(line, Rect::new(cell.x, y, right - cell.x, 1));
        }
    }
}

/// The lines of one cell: the date first (see `date_line`), then one line per slot (see `layout_row`):
/// a one-day item, e.g. 「 14:00 Code review」; a bar's line and a free line are left empty here (the
/// bars are drawn over them afterwards)
///
/// - List all slots if they fit; otherwise list one fewer and put `+N` on the last line
///   (N = the number of items not listed; free lines do not count)
/// - Cell only 1 line tall (very short terminal): no items; dots after the date, one per item (at most 4)
///
/// Each piece of text carries its color on its `Span` (not `Line::styled`), so when a long title borrows
/// the empty neighbor cells to the right, the neighbor's grid line does not get colored too
fn cell_lines(day: Date, month: Date, slots: &[Slot], rows: u16, app: &App) -> Vec<Line<'static>> {
    let theme = &app.theme;
    let mut first = date_line(day, month, app);
    let items: Vec<&Item> = slots
        .iter()
        .filter_map(|slot| match slot {
            Slot::Bar(item) | Slot::Item(item) => Some(*item),
            Slot::Empty => None,
        })
        .collect();
    if rows <= 1 {
        // Dots: the day's color if some item is not over yet, muted if all are over
        let color = if items.iter().any(|item| item.is_open(app.today)) {
            day_color(day, app)
        } else {
            theme.muted
        };
        first.push_span(Span::styled(dots(items.len()), Style::new().fg(color)));
        return vec![first];
    }
    let shown = shown_lines(slots.len(), rows);
    let mut lines = vec![first];
    lines.extend(slots[..shown].iter().map(|slot| match slot {
        Slot::Item(item) => item_line(item, app),
        Slot::Bar(_) | Slot::Empty => Line::default(),
    }));
    if shown < slots.len() {
        let color = theme.muted;
        let hidden = slots[shown..]
            .iter()
            .filter(|slot| !matches!(slot, Slot::Empty))
            .count();
        lines.push(Line::from(Span::styled(
            format!(" +{hidden}"),
            Style::new().fg(color),
        )));
    }
    lines
}

/// (title color, time color) of an item in the month grid: not over yet like layout 1's agenda
/// (`open_item_color`: a priority or a holiday colors the time too, otherwise it is muted); over gray. Other
/// months look the same
fn item_colors(item: &Item, app: &App) -> (Color, Color) {
    let theme = &app.theme;
    if item.is_open(app.today) {
        match open_item_color(item, theme) {
            (color, true) => (color, color),
            (color, false) => (color, theme.muted),
        }
    } else {
        (theme.muted, theme.muted)
    }
}

/// One item in a cell: 「 14:00 標題」 (time only if it has one; not for an in-progress item, whose 🚧
/// would leave no room for the title in a narrow cell: the day popup shows its time)
///
/// Same as layout 1's agenda: items not over yet are in the normal text color with the time muted, an
/// item with a priority or a holiday all in that color (`open_item_color`); items that are over are gray
/// for the whole line (done tasks also get a struck-through title); other months look the same
fn item_line(item: &Item, app: &App) -> Line<'static> {
    let (color, time_color) = item_colors(item, app);
    let mut spans = vec![Span::styled(" ", Style::new().fg(color))];
    if let Some(time) = item.time.filter(|_| !item.in_progress) {
        spans.push(Span::styled(
            format!("{} ", time.strftime("%H:%M")),
            Style::new().fg(time_color),
        ));
    }
    spans.push(title_span(item, color));
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

/// A cell's first line: the date on the left, with the month after the 1st, e.g. 「 1 10月」 (the selected
/// day has no brackets; its cell has a background instead, see `render`)
///
/// Color: same as layout 1's calendar (`day_color`: today, a day off, normal), other months too (not
/// grayed out: the grid lines between months already set them apart). Dates in the selected month are bold;
/// days covered by a multi-day item are underlined
fn date_line(day: Date, month: Date, app: &App) -> Line<'static> {
    let theme = &app.theme;
    let other_month = day.first_of_month() != month;
    let color = day_color(day, app);
    let mut number = Style::new().fg(color);
    if !other_month || day == app.today {
        number = number.add_modifier(Modifier::BOLD);
    }
    // Underline days covered by a multi-day item (same as layout 1's calendar)
    if in_multi_day(day, app) {
        number = number.add_modifier(Modifier::UNDERLINED);
    }
    let mut spans = vec![
        Span::raw(" "),
        Span::styled(day.day().to_string(), number),
        Span::raw(" "),
    ];
    if day.day() == 1 {
        spans.push(Span::styled(
            app.lang.texts().month_label(day),
            Style::new().fg(theme.label),
        ));
    }
    Line::from(spans)
}

/// A stretch of grid line, by the two days it lies between
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Rule {
    /// Two days of the same month: faint
    Week,
    /// A month's last day and the next month's 1st: in the border color (like the line between months in
    /// layout 1)
    Month,
    /// Dec 31st and Jan 1st: thick, in the year-line color
    Year,
}

impl Rule {
    /// The stretch between `day` and `next` (the day right of it, or the same weekday a week later)
    fn between(day: Date, next: Date) -> Self {
        if day.year() != next.year() {
            Self::Year
        } else if day.month() != next.month() {
            Self::Month
        } else {
            Self::Week
        }
    }

    /// Drawn with thick box-drawing characters
    fn heavy(self) -> bool {
        self == Self::Year
    }

    fn style(self, app: &App) -> Style {
        Style::new().fg(match self {
            Self::Week => app.theme.grid,
            Self::Month => app.theme.line,
            Self::Year => app.theme.year_line,
        })
    }
}

/// The day on weekday `weekday` (0 = Monday) of week `week` (from 0) of the weeks starting on `top`
fn day_at(top: Date, week: u16, weekday: usize) -> Date {
    top.saturating_add((i64::from(week) * 7 + i64::try_from(weekday).unwrap_or(0)).days())
}

/// The stretch of line left of weekday `weekday` (1-6 = Tuesday-Sunday) in week `week`: between that day
/// and the day before
fn rule_left_of(top: Date, week: u16, weekday: usize) -> Rule {
    Rule::between(day_at(top, week, weekday - 1), day_at(top, week, weekday))
}

/// The stretch under grid column `column` (0 = the week-number column, which goes with Monday; 1-7 =
/// Monday-Sunday) of week `week`, above the next week
fn rule_under(top: Date, week: u16, column: usize) -> Rule {
    let weekday = column.max(1) - 1;
    Rule::between(day_at(top, week, weekday), day_at(top, week + 1, weekday))
}

/// The box-drawing character where four stretches of line meet, each thin or thick (`true`)
fn cross(up: bool, down: bool, left: bool, right: bool) -> &'static str {
    match (up, down, left, right) {
        (false, false, false, false) => "┼",
        (false, false, true, false) => "┽",
        (false, false, false, true) => "┾",
        (false, false, true, true) => "┿",
        (true, false, false, false) => "╀",
        (false, true, false, false) => "╁",
        (true, true, false, false) => "╂",
        (true, false, true, false) => "╃",
        (true, false, false, true) => "╄",
        (false, true, true, false) => "╅",
        (false, true, false, true) => "╆",
        (true, false, true, true) => "╇",
        (false, true, true, true) => "╈",
        (true, true, true, false) => "╉",
        (true, true, false, true) => "╊",
        (true, true, true, true) => "╋",
    }
}

/// Draws the grid lines: `│` between columns, `─` below the header and between weeks, crossings where they
/// meet; horizontal lines meet the outer border as `├` / `┤`, vertical lines meet the bottom border as `┴`
/// (in the border color). The weeks start on `top` (a Monday)
///
/// Each stretch of line is drawn by the two days it lies between (`Rule`): faint within a month, in the
/// border color where a month ends, thick (`━` `┃`) where a year ends. So a month's edge is a staircase
/// like the outline of a paper calendar's month: the vertical stretch between the last day and the 1st,
/// then the line under the last week's days. The stretch under the week-number column goes with Monday's;
/// a crossing takes the strongest color of its four stretches.
/// For the leftover rows (empty rows below the last week), vertical lines still run to the bottom border,
/// so the last week just looks a few rows taller and no two lines stack up
///
/// Writes cell by cell straight into the `Buffer` (no widget); positions off screen are skipped
fn render_grid_lines(
    buffer: &mut Buffer,
    inner: Rect,
    grid: &Grid,
    top: Date,
    app: &App,
    border: Style,
) {
    let mut put = |x: u16, y: u16, symbol: &str, style: Style| {
        if let Some(cell) = buffer.cell_mut(Position::new(x, y)) {
            cell.set_symbol(symbol).set_style(style);
        }
    };
    // Vertical line `line` (0 = right of the week-number column, 1-6 = right of Monday-Saturday) in week
    // `week`
    let vertical = |week: u16, line: usize| {
        if line == 0 {
            Rule::Week
        } else {
            rule_left_of(top, week, line)
        }
    };
    let under = |week: u16, column: usize| rule_under(top, week, column);
    let xs: Vec<u16> = grid.vertical_xs().collect();
    let header_line = grid.header_y + 1;
    for (line, &x) in xs.iter().enumerate() {
        for week in 0..grid.weeks {
            let rule = vertical(week, line);
            let symbol = if rule.heavy() {
                line::THICK_VERTICAL
            } else {
                line::VERTICAL
            };
            let from = grid.week_y(week);
            let to = if week == grid.last_week() {
                inner.bottom()
            } else {
                from + grid.week_height
            };
            for y in from..to {
                put(x, y, symbol, rule.style(app));
            }
        }
        let last = vertical(grid.last_week(), line);
        put(
            x,
            inner.bottom(),
            if last.heavy() { "┸" } else { "┴" },
            border,
        );
        // The line below the header: faint, its crossings point down
        let below = vertical(0, line);
        put(
            x,
            header_line,
            if below.heavy() { "┰" } else { "┬" },
            below.style(app),
        );
    }
    for x in inner.x..inner.right() {
        if !xs.contains(&x) {
            put(x, header_line, line::HORIZONTAL, Rule::Week.style(app));
        }
    }
    put(
        inner.x.wrapping_sub(1),
        header_line,
        line::VERTICAL_RIGHT,
        border,
    );
    put(inner.right(), header_line, line::VERTICAL_LEFT, border);
    // The lines between weeks
    for week in 0..grid.last_week() {
        let y = grid.week_y(week) + grid.week_height;
        for (column, &(x, width)) in grid.columns.iter().enumerate() {
            let rule = under(week, column);
            let symbol = if rule.heavy() {
                line::THICK_HORIZONTAL
            } else {
                line::HORIZONTAL
            };
            for x in x..x + width {
                put(x, y, symbol, rule.style(app));
            }
        }
        for (line, &x) in xs.iter().enumerate() {
            let rules = [
                vertical(week, line),
                vertical(week + 1, line),
                under(week, line),
                under(week, line + 1),
            ];
            let [up, down, left, right] = rules.map(Rule::heavy);
            let strongest = rules.into_iter().max().unwrap_or(Rule::Week);
            put(x, y, cross(up, down, left, right), strongest.style(app));
        }
        let left = if under(week, 0).heavy() { "┝" } else { "├" };
        let right = if under(week, 7).heavy() { "┥" } else { "┤" };
        put(inner.x.wrapping_sub(1), y, left, border);
        put(inner.right(), y, right, border);
    }
}

/// The box-drawing character for a month's edge without grid lines, by which of its arms are there
/// (up, down, left, right); thick when `heavy`. A month's edge only bends at right angles, so this is a
/// corner (or, at most, a crossing)
fn edge_char(up: bool, down: bool, left: bool, right: bool, heavy: bool) -> &'static str {
    let [thin, thick] = match (up, down, left, right) {
        (false, true, false, true) => ["┌", "┏"],
        (false, true, true, false) => ["┐", "┓"],
        (true, false, false, true) => ["└", "┗"],
        (true, false, true, false) => ["┘", "┛"],
        (true, true, false, true) => ["├", "┣"],
        (true, true, true, false) => ["┤", "┫"],
        (false, true, true, true) => ["┬", "┳"],
        (true, false, true, true) => ["┴", "┻"],
        (true, true, true, true) => ["┼", "╋"],
        (_, _, false, false) => ["│", "┃"],
        _ => ["─", "━"],
    };
    if heavy { thick } else { thin }
}

/// Without grid lines: only where a month ends, its edge, the same staircase as `render_grid_lines` (in
/// the border color, thick where a year ends); no other lines. The vertical stretch between the month's
/// last day and the 1st goes in the first column of the 1st's cell (always blank: every line in a cell
/// starts with a space), the horizontal stretches on the row the grid left between the two weeks
/// (`Grid::week_gaps`); corners where they meet, and `├` `┤` `┴` where they meet the outer border.
/// Drawn after the cells, over that first column
fn render_month_edges(
    buffer: &mut Buffer,
    inner: Rect,
    grid: &Grid,
    top: Date,
    app: &App,
    border: Style,
) {
    let mut put = |x: u16, y: u16, symbol: &str, style: Style| {
        if let Some(cell) = buffer.cell_mut(Position::new(x, y)) {
            cell.set_symbol(symbol).set_style(style);
        }
    };
    let edge = |rule: Rule| rule != Rule::Week;
    // The vertical stretches, through the whole week (the last week down to the bottom border)
    for week in 0..grid.weeks {
        for weekday in 1..7 {
            let rule = rule_left_of(top, week, weekday);
            if !edge(rule) {
                continue;
            }
            let x = grid.columns[weekday + 1].0;
            let from = grid.week_y(week);
            let last = week == grid.last_week();
            let to = if last {
                inner.bottom()
            } else {
                from + grid.week_height
            };
            let symbol = if rule.heavy() {
                line::THICK_VERTICAL
            } else {
                line::VERTICAL
            };
            for y in from..to {
                put(x, y, symbol, rule.style(app));
            }
            if last {
                put(
                    x,
                    inner.bottom(),
                    if rule.heavy() { "┸" } else { "┴" },
                    border,
                );
            }
        }
    }
    // The horizontal stretches, on the row between two weeks
    for week in 0..grid.last_week() {
        if grid.week_gaps[usize::from(week)] == 0 {
            continue;
        }
        let y = grid.week_y(week) + grid.week_height;
        for (column, &(x, width)) in grid.columns.iter().enumerate() {
            let rule = rule_under(top, week, column);
            if !edge(rule) {
                continue;
            }
            let symbol = if rule.heavy() {
                line::THICK_HORIZONTAL
            } else {
                line::HORIZONTAL
            };
            for x in x..x + width {
                put(x, y, symbol, rule.style(app));
            }
        }
        // Where a vertical stretch meets this row
        for weekday in 1..7 {
            let rules = [
                rule_left_of(top, week, weekday),
                rule_left_of(top, week + 1, weekday),
                rule_under(top, week, weekday),
                rule_under(top, week, weekday + 1),
            ];
            let [up, down, left, right] = rules.map(edge);
            if !up && !down {
                continue;
            }
            let strongest = rules.into_iter().max().unwrap_or(Rule::Week);
            put(
                grid.columns[weekday + 1].0,
                y,
                edge_char(up, down, left, right, strongest.heavy()),
                strongest.style(app),
            );
        }
        let (left, right) = (rule_under(top, week, 0), rule_under(top, week, 7));
        if edge(left) {
            let symbol = if left.heavy() { "┝" } else { "├" };
            put(inner.x.wrapping_sub(1), y, symbol, border);
        }
        if edge(right) {
            let symbol = if right.heavy() { "┥" } else { "┤" };
            put(inner.right(), y, symbol, border);
        }
    }
}

/// Which day a mouse click at (`column`, `row`) on the month grid (`area`) hits; `None` on the header,
/// the week-number column, grid lines or the outer border
///
/// The scroll position is `app.month_top`, stored at the last draw; the empty leftover rows below the
/// last week count as the last week
pub fn click_at(area: Rect, app: &App, column: u16, row: u16) -> Option<Click> {
    let inner = Block::bordered().inner(area);
    let top = app.month_top?;
    let grid = Grid::new(inner, app.settings.month.grid_lines, top);
    if grid.too_small() {
        return None;
    }
    let position = Position::new(column, row);
    for week in 0..grid.weeks {
        for weekday in 0..7 {
            let mut cell = grid.cell(week, weekday + 1);
            if week == grid.last_week() {
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
mod tests;
