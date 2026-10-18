//! Side menu: the calendar list on the far left (calendar = a group in the task file). It is drawn on the
//! far left when the terminal is wide enough; otherwise `s` brings it up over the left side

use super::{Click, months::scrolled_top, planner::inner_areas, render_scrollbar};
use crate::{
    app::{App, Focus, Group},
    texts::Texts,
};
use ratatui::{
    Frame,
    layout::{Constraint, Layout, Position, Rect},
    style::Style,
    symbols::line,
    text::{Line, Span},
    widgets::{Block, BorderType},
};

/// Narrowest side menu width (incl. left and right border); the header always fits
const MIN_WIDTH: u16 = 22;
/// Widest side menu width (incl. left and right border); longer names get cut off
const MAX_WIDTH: u16 = 30;

/// Side menu width (incl. left and right border): just enough for the longest calendar,
/// e.g. 「 ☑ Payments API v2 12 」,
/// but kept between `MIN_WIDTH` and `MAX_WIDTH`
pub fn width(app: &App) -> u16 {
    let texts = app.lang.texts();
    let longest = app
        .groups
        .iter()
        // 「 ☑ 」 is 3 cells + name + count
        .map(|group| 3 + Span::raw(name(group, texts)).width() + count_text(app, group).len())
        .max()
        .unwrap_or(0);
    // Add the left and right border
    u16::try_from(longest + 2)
        .unwrap_or(MAX_WIDTH)
        .clamp(MIN_WIDTH, MAX_WIDTH)
}

/// The calendar's name; items at the top of the file, before any heading, are 「未分組」
fn name<'a>(group: &'a Group, texts: &'static Texts) -> &'a str {
    group.name.as_deref().unwrap_or(texts.ungrouped)
}

/// The number to the right of a calendar (one blank on each side), e.g. 「 12 」: how many of that
/// calendar's items have a date (counted even when unchecked)
fn count_text(app: &App, group: &Group) -> String {
    let count = app
        .items
        .iter()
        .filter(|item| item.date.is_some() && group.includes(item))
        .count();
    format!(" {count} ")
}

/// Draws the side menu: the header 「日曆」, then one calendar per row, e.g. 「 ☑ Work            12 」
///
/// Checked is `☑`, unchecked is `☐` (its items are hidden); a name that is too long (box already at
/// `MAX_WIDTH`) gets cut off. When there are too many calendars to fit, first scroll so the chosen one
/// is visible (stored in `app.side_top`) and draw a scrollbar on the right border. When the keyboard is
/// on the side menu, the border lights up and the chosen calendar gets a background color
pub fn render(frame: &mut Frame, area: Rect, app: &mut App) {
    // The header matches the other two boxes: a line of text + a rule
    let [header, rule, body] = inner_areas(area);
    let height = i64::from(body.height);
    let top = scrolled_top(
        i64::try_from(app.side_top).unwrap_or(0),
        i64::try_from(app.group_index).unwrap_or(0),
        height,
        // Only scroll when the chosen one leaves the box: clicking a visible calendar does not scroll it
        // away (so a second click hits the same one)
        0,
    );
    // Do not overscroll: the last calendar just touches the bottom
    let max = i64::try_from(app.groups.len()).unwrap_or(0) - height;
    app.side_top = usize::try_from(top.min(max).max(0)).unwrap_or(0);
    // Read-only from here on
    let app = &*app;
    let theme = &app.theme;
    let texts = app.lang.texts();
    let focused = app.focus == Focus::Side;
    let block = Block::bordered()
        .border_type(BorderType::Rounded)
        .border_style(Style::new().fg(if focused { theme.focus } else { theme.line }));
    frame.render_widget(block, area);
    frame.render_widget(
        Line::styled(texts.header_side, Style::new().fg(theme.muted)),
        header,
    );
    frame.render_widget(
        Line::styled(
            line::HORIZONTAL.repeat(usize::from(rule.width)),
            Style::new().fg(theme.line),
        ),
        rule,
    );
    let visible = app.groups.iter().enumerate().skip(app.side_top);
    for ((index, group), y) in visible.zip(body.y..body.bottom()) {
        let row = Rect::new(body.x, y, body.width, 1);
        let count = count_text(app, group);
        // Name on the left, count on the right; a long name is cut off and never covers the count
        // (digits and spaces are 1 cell wide)
        let [name_area, count_area] =
            Layout::horizontal([Constraint::Fill(1), Constraint::Length(count.len() as u16)])
                .areas(row);
        // Checked: ☑ in the `checked` color and the name in normal text color;
        // unchecked: ☐ and the name are both muted
        let (mark, mark_color, name_color) = if group.shown {
            ("☑", theme.checked, theme.text)
        } else {
            ("☐", theme.muted, theme.muted)
        };
        frame.render_widget(
            Line::from(vec![
                Span::styled(format!(" {mark} "), Style::new().fg(mark_color)),
                Span::styled(name(group, texts), Style::new().fg(name_color)),
            ]),
            name_area,
        );
        frame.render_widget(
            Line::styled(count, Style::new().fg(theme.muted)),
            count_area,
        );
        if focused && index == app.group_index {
            // Only change the background; leave the text color alone
            frame
                .buffer_mut()
                .set_style(row, Style::new().bg(theme.cursor_line));
        }
    }
    // Does not fit: draw a scrollbar on the right border next to the calendar rows (not when the box
    // has no rows at all)
    if let Ok(max) = u16::try_from(max)
        && max > 0
        && body.height > 0
    {
        let rows = Rect {
            y: body.y - 1,
            height: body.height + 2,
            ..area
        };
        let top = u16::try_from(app.side_top).unwrap_or(u16::MAX);
        render_scrollbar(frame, rows, top, max, body.height, theme);
    }
}

/// Which calendar a mouse click at (`column`, `row`) on the side menu (`area`) hits; `None` on the
/// header, the border, or the blank space below
pub fn click_at(area: Rect, app: &App, column: u16, row: u16) -> Option<Click> {
    let [_, _, body] = inner_areas(area);
    // One calendar per row, starting at the first row of `body` (if scrolled, that is calendar `side_top`)
    if !body.contains(Position::new(column, row)) {
        return None;
    }
    let index = app.side_top + usize::from(row - body.y);
    (index < app.groups.len()).then_some(Click::Group(index))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::texts::Lang;
    use jiff::civil::date;

    /// An app whose task file is `text`
    fn app_with(text: &str) -> App {
        App::new(
            Lang::ZhTw,
            date(2026, 10, 17),
            crate::tasks::parse(text).items,
        )
    }

    #[test]
    fn width_fits_the_longest_calendar_between_22_and_30() {
        // No task file, or a short name (「 ☑ Personal 1 」 needs only 14 cells): the minimum, 22
        assert_eq!(width(&app_with("")), 22);
        assert_eq!(width(&app_with("# Personal\n- a @2026-10-10\n")), 22);
        // 「 ☑ Tokyo trip 🗼🍣 1 」: each emoji is two cells wide, so 3 + 15 + 3 = 21 cells, 23 with the
        // border; the longest one sets the width
        let app = app_with("# Personal\n- a @2026-10-10\n# Tokyo trip 🗼🍣\n- b @2026-12-24\n");
        assert_eq!(width(&app), 23);
        // A two-digit count: 1 more cell
        let ten = "- x @2026-10-10\n".repeat(10);
        assert_eq!(width(&app_with(&format!("# Tokyo trip 🗼🍣\n{ten}"))), 24);
        // Longer still: the maximum, 30
        assert_eq!(
            width(&app_with("# Year-end chores 🎄 and plans for 2027\n- a\n")),
            30
        );
    }
}
