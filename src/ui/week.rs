//! Layout 4 (`4`): the week view. 7 days in boxes two to a row, each day's items one per line and cut off
//! at the box's right edge (the day popup shows them whole); the 8th box (bottom right) is a small
//! calendar. The 7 days slide along with the selected day: moving past the last box slides them a day
//! later, past the first a day earlier

use super::{
    Click, cell_items,
    day::popup_line,
    planner::{self, COLUMN_WIDTHS, DAY_WIDTH, day_color},
};
use crate::{
    app::{App, Focus},
    calendar::{self, WeekRow},
};
use jiff::{ToSpan, civil::Date};
use ratatui::{
    Frame,
    layout::{Margin, Position, Rect},
    style::{Modifier, Style},
    symbols::line,
    text::{Line, Span},
    widgets::{Block, BorderType},
};

/// Days on screen
const DAYS: i64 = 7;
/// Rows of boxes
const ROWS: u16 = 4;
/// Boxes in a row
const COLUMNS: u16 = 2;
/// Cells left of the small calendar's dates: year / month, week number, │
const DATES_X: u16 = COLUMN_WIDTHS[0] + COLUMN_WIDTHS[1] + COLUMN_WIDTHS[2];

/// The first of the 7 days on screen: from `top` (`App::week_top`; not set = the selected day), slid just
/// enough that `selected` is inside: after the last day = it becomes the last, before the first = the first
fn first_day(top: Option<Date>, selected: Date) -> Date {
    let top = top.unwrap_or(selected);
    let last = top.saturating_add((DAYS - 1).days());
    if selected < top {
        selected
    } else if selected > last {
        selected.saturating_sub((DAYS - 1).days())
    } else {
        top
    }
}

/// Which of a box's `count` items its `rows` lines show from item `top` on: (how many, whether the first line
/// says how many more are above, whether the last line says how many more are below). All fit: all of them.
/// From the top: one line fewer, the last saying how many more. Scrolled down (`top` > 0): a line on top,
/// and one at the bottom only if the rest still does not fit (so it always stands for 2 or more).
/// `notes` = false (the chosen item's box with fewer than 3 lines): no such lines, only items
fn window(count: usize, rows: usize, top: usize, notes: bool) -> (usize, bool, bool) {
    if count <= rows {
        (count, false, false)
    } else if !notes {
        (rows.min(count - top), false, false)
    } else if top == 0 {
        (rows - 1, false, true)
    } else if count - top < rows {
        (count - top, true, false)
    } else {
        (rows - 2, true, true)
    }
}

/// The first item shown in the chosen item's box: from `top` (where it was), scrolled just enough that item
/// `chosen` shows (see `window`); never 1, since a line saying 「1 more above」 can be that item instead
fn scroll_top(count: usize, rows: usize, top: usize, chosen: usize, notes: bool) -> usize {
    if count <= rows {
        return 0;
    }
    let mut top = top.min(chosen);
    while top < chosen && chosen >= top + window(count, rows, top, notes).0 {
        top += 1;
    }
    if notes && top == 1 { 0 } else { top }
}

/// How `day`'s box (`rows` lines, `count` items) shows its items: (first item shown, `window`'s three). Only
/// the selected day's box scrolls, while its items have the keyboard (`App::cell_top`)
fn box_window(app: &App, day: Date, count: usize, rows: usize) -> (usize, usize, bool, bool) {
    let chosen_box = app.focus == Focus::Cell && day == app.selected;
    let notes = !chosen_box || rows >= 3;
    let top = app
        .cell_top
        .filter(|&(top_day, _)| chosen_box && top_day == day)
        .map_or(0, |(_, top)| top);
    let (shown, above, below) = window(count, rows, top, notes);
    (top, shown, above, below)
}

/// `total` cells split into `parts` evenly, the leftover one each to the first parts
fn split(total: u16, parts: u16) -> impl Iterator<Item = (u16, u16)> {
    let mut start = 0;
    (0..parts).map(move |part| {
        let length = total / parts + u16::from(part < total % parts);
        let piece = (start, length);
        start += length;
        piece
    })
}

/// The 8 boxes on `area`, row by row (left, then right): 4 rows of 2, the first 7 for the days on screen.
/// The rows share the height evenly and the two boxes the width; leftover rows go to the top rows, a
/// leftover cell to the left box
fn boxes(area: Rect) -> Vec<Rect> {
    let columns: Vec<(u16, u16)> = split(area.width, COLUMNS).collect();
    split(area.height, ROWS)
        .flat_map(|(y, height)| {
            columns
                .iter()
                .map(move |&(x, width)| Rect::new(area.x + x, area.y + y, width, height))
        })
        .collect()
}

/// Whether every day's box on `area` has at least 1 line inside for items
pub fn shows_items(area: Rect) -> bool {
    boxes(area).iter().take(7).all(|area| area.height > 2)
}

/// Draws the week view on `area`: first slides the 7 days so the selected day is among them (kept in
/// `app.week_top`) and, while a box's items have the keyboard, scrolls that box to the chosen item (kept in
/// `app.cell_top`); then draws each day's box and the small calendar
pub fn render(frame: &mut Frame, area: Rect, app: &mut App) {
    let first = first_day(app.week_top, app.selected);
    app.week_top = Some(first);
    let boxes = boxes(area);
    // Where the chosen item's box was scrolled (on another day or with the keyboard elsewhere: the top)
    let before = app
        .cell_top
        .take()
        .filter(|&(day, _)| day == app.selected)
        .map_or(0, |(_, top)| top);
    if app.focus == Focus::Cell {
        let offset = usize::try_from((app.selected - first).get_days()).unwrap_or(0);
        let rows = boxes
            .get(offset)
            .map_or(0, |area| usize::from(area.height.saturating_sub(2)));
        let count = app.cell_items().len();
        let top = scroll_top(count, rows, before, app.cell_index, rows >= 3);
        app.cell_top = Some((app.selected, top));
    }
    let app = &*app;
    for (offset, &area) in (0..DAYS).zip(&boxes) {
        render_day(frame, area, first.saturating_add(offset.days()), app);
    }
    if let Some(&area) = boxes.get(7) {
        render_calendar(frame, area, first, app);
    }
}

/// One day's box: its date on the top border (`今天` after today's), its items one per line like the day
/// popup's (check box, dates of a multi-day item, time, title) in `App::items_on` order, cut off one cell
/// before the right border. Items that do not fit: the last line says how many more there are (`window`);
/// while the box's items have the keyboard, it scrolls to the chosen one, which gets the `selection`
/// background, and its lines say how many are above / below. The selected day's border is in the `focus`
/// color (unless the side menu has the keyboard)
fn render_day(frame: &mut Frame, area: Rect, day: Date, app: &App) {
    let theme = &app.theme;
    let texts = app.lang.texts();
    let selected = day == app.selected;
    let border = if selected && app.focus != Focus::Side {
        theme.focus
    } else {
        theme.line
    };
    let mut date = Style::new().fg(day_color(day, app));
    if selected || day == app.today {
        date = date.add_modifier(Modifier::BOLD);
    }
    let mut title = vec![
        Span::styled(line::HORIZONTAL, Style::new().fg(border)),
        Span::styled(format!(" {} ", texts.week_day_title(day)), date),
    ];
    if day == app.today {
        title.push(Span::styled(
            format!("{} ", texts.week_today()),
            Style::new().fg(theme.today),
        ));
    }
    let block = Block::bordered()
        .border_type(BorderType::Rounded)
        .border_style(Style::new().fg(border))
        .title_top(Line::from(title));
    let inner = block.inner(area);
    frame.render_widget(block, area);
    let rows = usize::from(inner.height);
    if rows == 0 {
        return;
    }
    // One blank cell kept before the right border
    let width = inner.width.saturating_sub(1);
    let items = cell_items(app, day);
    let (top, shown, above, below) = box_window(app, day, items.len(), rows);
    let chosen = (app.focus == Focus::Cell && selected).then_some(app.cell_index);
    let note = |text: String, y: u16| {
        let line = Line::styled(text, Style::new().fg(theme.muted)).right_aligned();
        (line, Rect::new(inner.x, y, width, 1))
    };
    if above {
        let (line, area) = note(texts.week_above(top), inner.y);
        frame.render_widget(line, area);
    }
    let first_y = inner.y + u16::from(above);
    for ((index, item), y) in items
        .iter()
        .enumerate()
        .skip(top)
        .take(shown)
        .zip(first_y..)
    {
        frame.render_widget(popup_line(item, app), Rect::new(inner.x, y, width, 1));
        if chosen == Some(index) {
            let row = Rect::new(inner.x, y, inner.width, 1);
            frame
                .buffer_mut()
                .set_style(row, Style::new().bg(theme.selection));
        }
    }
    if below {
        let hidden = items.len() - top - shown;
        let text = if chosen.is_some() {
            texts.week_below(hidden)
        } else {
            texts.week_more(hidden)
        };
        let (line, area) = note(text, inner.bottom() - 1);
        frame.render_widget(line, area);
    }
}

/// Draws one piece of text in each of the small calendar's four columns on a row (`area`), at layout 1's
/// column widths from the left; whatever does not fit in `area` is cut off at its right edge
fn render_columns(frame: &mut Frame, area: Rect, cells: [Line; 4]) {
    let mut x = area.x;
    for (cell, width) in cells.into_iter().zip(COLUMN_WIDTHS) {
        let column = Rect::new(x, area.y, width, area.height).intersection(area);
        frame.render_widget(cell, column);
        x = x.saturating_add(width);
    }
}

/// The first day of the small calendar's top week: the week the 7 days start in
fn calendar_top(first: Date, app: &App) -> Date {
    calendar::week_start(first, app.first_day_of_week())
}

/// The small calendar in the 8th box (no title): layout 1's header (the year of the first day on screen,
/// 週, the weekdays), then the weeks one after another from the week the 7 days start in, as many as fit,
/// without breaks between months (a month's name on the top week and on the week with its 1st). Dates,
/// dots and the selected day's `[17]` as in layout 1 (`planner::day_cells`); the 7 days on screen get the
/// `selection` background, and the weeks they are in the bright week-number color
fn render_calendar(frame: &mut Frame, area: Rect, first: Date, app: &App) {
    let theme = &app.theme;
    let texts = app.lang.texts();
    let block = Block::bordered()
        .border_type(BorderType::Rounded)
        .border_style(Style::new().fg(theme.line));
    let inner = block.inner(area);
    frame.render_widget(block, area);
    if inner.height == 0 {
        return;
    }
    render_columns(
        frame,
        Rect { height: 1, ..inner },
        planner::header_cells(app, first),
    );
    let week_first = app.first_day_of_week();
    let relative = app.settings.calendar.relative_weeks;
    let shown = first..=first.saturating_add((DAYS - 1).days());
    for (week, y) in (0..).zip(inner.y + 1..inner.bottom()) {
        let start = calendar_top(first, app).saturating_add(i64::from(week).weeks());
        let days: [Date; 7] = std::array::from_fn(|day| start.saturating_add((day as i64).days()));
        let row: WeekRow = days.map(Some);
        // A month's name: on the week with its 1st, and on the top week
        let label = days
            .iter()
            .find(|day| day.day() == 1)
            .or((week == 0).then_some(&first))
            .map(|&day| texts.month_label(day))
            .unwrap_or_default();
        // The week number from 1/1 when the week has it (that week is week 1), else from its first day
        let numbered = days
            .iter()
            .copied()
            .find(|day| day.month() == 1 && day.day() == 1)
            .unwrap_or(start);
        let number = calendar::week_label(numbered, app.selected, relative, week_first);
        let number_color = if days.iter().any(|day| shown.contains(day)) {
            theme.week_number
        } else {
            theme.week_number_dim
        };
        let area = Rect {
            y,
            height: 1,
            ..inner
        };
        render_columns(
            frame,
            area,
            [
                Line::styled(label, Style::new().fg(theme.label)).right_aligned(),
                Line::styled(format!("{number:>3} "), Style::new().fg(number_color)),
                Line::styled(line::VERTICAL, Style::new().fg(theme.line)),
                planner::day_cells(&row, app),
            ],
        );
        // The 7 days on screen: the background, DAY_WIDTH cells each
        for (column, day) in (0..).zip(&days) {
            if shown.contains(day) {
                let x = area.x + DATES_X + column * DAY_WIDTH;
                let cells = Rect::new(x, y, DAY_WIDTH, 1).intersection(area);
                frame
                    .buffer_mut()
                    .set_style(cells, Style::new().bg(theme.selection));
            }
        }
    }
}

/// What a click at (`column`, `row`) hits in the week view on `area`: an item's line in a box (as it was
/// last drawn: scrolled), a line there saying how many more (the box's day, to open its popup), elsewhere in
/// a box its day, or a date in the small calendar (the blanks before and after it count, as in layout 1)
pub fn click_at(area: Rect, app: &App, column: u16, row: u16) -> Option<Click> {
    let first = first_day(app.week_top, app.selected);
    let position = Position::new(column, row);
    let boxes = boxes(area);
    if let Some((offset, area)) = (0..DAYS)
        .zip(&boxes)
        .find(|(_, area)| area.contains(position))
    {
        let day = first.saturating_add(offset.days());
        let inner = area.inner(Margin::new(1, 1));
        if inner.contains(position) {
            let count = cell_items(app, day).len();
            let rows = usize::from(inner.height);
            let (top, shown, above, below) = box_window(app, day, count, rows);
            let line = usize::from(row - inner.y);
            if (above && line == 0) || (below && line + 1 == rows) {
                return Some(Click::More(day));
            }
            if let Some(index) = line.checked_sub(usize::from(above))
                && index < shown
            {
                return Some(Click::CellItem {
                    day,
                    index: top + index,
                });
            }
        }
        return Some(Click::Day(day));
    }
    // The small calendar: below its header, right of the │
    let inner = boxes.get(7)?.inner(Margin::new(1, 1));
    if !inner.contains(position) || row == inner.y || column < inner.x + DATES_X {
        return None;
    }
    let weekday = (column - inner.x - DATES_X) / DAY_WIDTH;
    if weekday >= 7 {
        return None;
    }
    let week = row - inner.y - 1;
    let day = calendar_top(first, app)
        .saturating_add(i64::from(week).weeks())
        .saturating_add(i64::from(weekday).days());
    Some(Click::Day(day))
}

#[cfg(test)]
mod tests {
    use super::*;
    use jiff::civil::date;

    #[test]
    fn the_seven_days_slide_just_enough_to_keep_the_selected_day() {
        let top = Some(date(2026, 10, 17));
        // Not set yet: from the selected day
        assert_eq!(first_day(None, date(2026, 10, 20)), date(2026, 10, 20));
        // Inside (the first box, the last box): they stay
        assert_eq!(first_day(top, date(2026, 10, 17)), date(2026, 10, 17));
        assert_eq!(first_day(top, date(2026, 10, 23)), date(2026, 10, 17));
        // A day past the last box: a day later, the selected day in the last box
        assert_eq!(first_day(top, date(2026, 10, 24)), date(2026, 10, 18));
        // A day before the first box: a day earlier, the selected day in the first box
        assert_eq!(first_day(top, date(2026, 10, 16)), date(2026, 10, 16));
        // Far away (e.g. the day popup's `l` many times, or a reschedule): it lands on the near edge
        assert_eq!(first_day(top, date(2026, 12, 1)), date(2026, 11, 25));
    }

    #[test]
    fn a_box_shows_what_fits_and_says_how_many_more() {
        // 4 items in 6 lines: all
        assert_eq!(window(4, 6, 0, true), (4, false, false));
        // 35 in 6 from the top: 5, and the last line says how many more
        assert_eq!(window(35, 6, 0, true), (5, false, true));
        // Scrolled to item 2: a line on top, 4 items, a line at the bottom
        assert_eq!(window(35, 6, 2, true), (4, true, true));
        // 8 in 6 from item 3: the 5 left fit under the top line, no bottom line
        assert_eq!(window(8, 6, 3, true), (5, true, false));
        // A 1-line box from the top: only the line saying how many
        assert_eq!(window(35, 1, 0, true), (0, false, true));
        // No such lines (the chosen item's box, 2 lines): items only
        assert_eq!(window(35, 2, 0, false), (2, false, false));
        assert_eq!(window(35, 2, 34, false), (1, false, false));
    }

    #[test]
    fn the_chosen_items_box_scrolls_just_enough() {
        // All fit: never scrolled
        assert_eq!(scroll_top(4, 6, 3, 2, true), 0);
        // From the top, items 0-4 show: 4 stays, 5 needs item 2 on top (from 1, items 1-4 show)
        assert_eq!(scroll_top(35, 6, 0, 4, true), 0);
        assert_eq!(scroll_top(35, 6, 0, 5, true), 2);
        // Still in view: it stays; above the top: the chosen item becomes the top one
        assert_eq!(scroll_top(35, 6, 2, 3, true), 2);
        assert_eq!(scroll_top(35, 6, 5, 3, true), 3);
        // Never 1: item 1 shows from the top as well, without a line saying 「1 more above」
        assert_eq!(scroll_top(35, 6, 3, 1, true), 0);
        // 8 in 6, the last chosen: from item 3 the rest fit under the top line
        assert_eq!(scroll_top(8, 6, 0, 7, true), 3);
        // Without such lines (2 lines): just enough for the chosen item to be the bottom one
        assert_eq!(scroll_top(35, 2, 0, 3, false), 2);
    }

    #[test]
    fn the_boxes_share_the_room_evenly_leftovers_first() {
        // 81 × 30: rows 8, 8, 7, 7; the left box 41 wide, the right 40
        let boxes = boxes(Rect::new(3, 1, 81, 30));
        assert_eq!(boxes.len(), 8);
        assert_eq!(boxes[0], Rect::new(3, 1, 41, 8));
        assert_eq!(boxes[1], Rect::new(44, 1, 40, 8));
        assert_eq!(boxes[2], Rect::new(3, 9, 41, 8));
        assert_eq!(boxes[5], Rect::new(44, 17, 40, 7));
        assert_eq!(boxes[7], Rect::new(44, 24, 40, 7));
    }
}
