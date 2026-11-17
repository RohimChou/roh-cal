//! Layout 4 (`4`): the week view. 7 days in boxes two to a row, each day's items one per line and cut off
//! at the box's right edge (the day popup shows them whole); the 8th box (bottom right) is a small
//! calendar. The 7 days slide along with the selected day: moving past the last box slides them a day
//! later, past the first a day earlier

use super::{
    Click,
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

/// Draws the week view on `area`: first slides the 7 days so the selected day is among them (kept in
/// `app.week_top`), then draws each day's box and the small calendar
pub fn render(frame: &mut Frame, area: Rect, app: &mut App) {
    let first = first_day(app.week_top, app.selected);
    app.week_top = Some(first);
    let app = &*app;
    let boxes = boxes(area);
    for (offset, &area) in (0..DAYS).zip(&boxes) {
        render_day(frame, area, first.saturating_add(offset.days()), app);
    }
    if let Some(&area) = boxes.get(7) {
        render_calendar(frame, area, first, app);
    }
}

/// One day's box: its date on the top border (`今天` after today's), its items one per line like the day
/// popup's (check box, dates of a multi-day item, time, title) in `App::items_on` order, cut off one cell
/// before the right border. Items that do not fit: the last line says how many more there are. The
/// selected day's border is in the `focus` color (unless the side menu has the keyboard)
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
    let items = app.items_on(day);
    let shown = if items.len() <= rows {
        items.len()
    } else {
        rows - 1
    };
    for (item, y) in items.iter().take(shown).zip(inner.y..) {
        frame.render_widget(popup_line(item, app), Rect::new(inner.x, y, width, 1));
    }
    if shown < items.len() {
        let more = Line::styled(
            texts.week_more(items.len() - shown),
            Style::new().fg(theme.muted),
        )
        .right_aligned();
        let y = inner.bottom() - 1;
        frame.render_widget(more, Rect::new(inner.x, y, width, 1));
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

/// What a click at (`column`, `row`) hits in the week view on `area`: a day's box (anywhere in it), or a
/// date in the small calendar (the blanks before and after it count, as in layout 1)
pub fn click_at(area: Rect, app: &App, column: u16, row: u16) -> Option<Click> {
    let first = first_day(app.week_top, app.selected);
    let position = Position::new(column, row);
    let boxes = boxes(area);
    if let Some((offset, _)) = (0..DAYS)
        .zip(&boxes)
        .find(|(_, area)| area.contains(position))
    {
        return Some(Click::Day(first.saturating_add(offset.days())));
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
