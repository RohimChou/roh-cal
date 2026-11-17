//! Layout 4 (`4`): the week view. 7 days in boxes two to a row (the 8th box, bottom right, stays blank),
//! each day's items one per line and cut off at the box's right edge (the day popup shows them whole).
//! The 7 days slide along with the selected day: moving past the last box slides them a day later, past
//! the first a day earlier

use super::{Click, day::popup_line, planner::day_color};
use crate::app::{App, Focus};
use jiff::{ToSpan, civil::Date};
use ratatui::{
    Frame,
    layout::{Position, Rect},
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
/// `app.week_top`), then draws each day's box
pub fn render(frame: &mut Frame, area: Rect, app: &mut App) {
    let first = first_day(app.week_top, app.selected);
    app.week_top = Some(first);
    let app = &*app;
    for (offset, area) in (0..DAYS).zip(boxes(area)) {
        render_day(frame, area, first.saturating_add(offset.days()), app);
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

/// What a click at (`column`, `row`) hits in the week view on `area`: a day's box, anywhere in it
pub fn click_at(area: Rect, app: &App, column: u16, row: u16) -> Option<Click> {
    let first = first_day(app.week_top, app.selected);
    let position = Position::new(column, row);
    (0..DAYS)
        .zip(boxes(area))
        .find(|(_, area)| area.contains(position))
        .map(|(offset, _)| Click::Day(first.saturating_add(offset.days())))
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
