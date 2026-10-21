//! Layout 2's day popup: sits in the middle of the month grid, one item per row (same colors as layout 1's
//! agenda, but each item starts with a check box, or a dot for an event, and only a multi-day item writes
//! its dates: the top border already says which day; a title too long for the box wraps onto the next
//! lines, under the title), with the selected day on the top border and some blank space inside the border
//! when there is room

use super::{
    calendar_path, calendar_tag, check_box, clear_with_ring, item_marker,
    months::scrolled_to_range,
    planner::{date_text, item_color, secondary_color, time_and_title},
    render_scrollbar,
    tasks::wrap,
};
use crate::app::{App, TextSpot};
use crate::tasks::{Heading, Item};
use crate::theme::Theme;
use ratatui::{
    Frame,
    layout::{Margin, Position, Rect},
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Block, BorderType},
};

/// Cells kept free left and right of the box at its widest (like the editor), so it does not touch what is
/// beside the month grid
const SIDE_MARGIN: u16 = 2;

/// Below this month-grid width the box may take the whole width (no `SIDE_MARGIN`)
const NARROW: u16 = 30;

/// Narrowest popup width (incl. border): enough room for the date on the top border
const MIN_WIDTH: u16 = 30;

/// The top border's two titles: the selected day on the left; on the right, how many of its tasks are not
/// done yet and done, e.g. 「 未完成 3 / 已完成 2 」 (`None` when the day has no tasks). The open count's color
/// says how much is left (`open_color`); the done count is always green (`today`, green in most themes)
fn top_titles(app: &App) -> (Line<'static>, Option<Line<'static>>) {
    let theme = &app.theme;
    let texts = app.lang.texts();
    let date = Line::styled(
        format!(" {} ", texts.date_title(app.selected)),
        Style::new().fg(theme.text),
    );
    let (open, done) = app.day_counts(app.selected);
    let counts = (open + done > 0).then(|| {
        Line::from(vec![
            Span::styled(
                format!(" {}", texts.day_open(open)),
                Style::new().fg(open_color(open, open + done, theme)),
            ),
            Span::styled(" / ", Style::new().fg(theme.muted)),
            Span::styled(
                format!("{} ", texts.day_done(done)),
                Style::new().fg(theme.today),
            ),
        ])
        .right_aligned()
    });
    (date, counts)
}

/// The color of a day's open count, by how much of its `total` tasks is left (the priority colors): more
/// than half red, a third or more orange, more than a fifth yellow, otherwise the normal text color
fn open_color(open: usize, total: usize, theme: &Theme) -> Color {
    if open * 2 > total {
        theme.priority_high
    } else if open * 3 >= total {
        theme.priority_medium
    } else if open * 5 > total {
        theme.priority_low
    } else {
        theme.text
    }
}

/// The switch at the right end of the bottom border, e.g. 「 ☑ By calendar 」: ☑ in the `checked` color
/// while grouped by calendar (`[day_popup] by_calendar`), ☐ muted when not (like the calendar list's
/// switches); `c` or a click flips it
fn switch_line(app: &App) -> Line<'static> {
    let theme = &app.theme;
    let by_calendar = app.settings.day_popup.by_calendar;
    let color = if by_calendar {
        theme.checked
    } else {
        theme.muted
    };
    Line::from(vec![
        Span::styled(
            format!(" {}", check_box(by_calendar)),
            Style::new().fg(color),
        ),
        Span::styled(
            format!("{} ", app.lang.texts().by_calendar),
            Style::new().fg(theme.text),
        ),
    ])
    .right_aligned()
}

/// Where the switch is (see `switch_line`): the right end of the bottom border, next to the corner
pub fn switch_area(app: &App, main: Rect) -> Rect {
    let area = area(app, main);
    let width = u16::try_from(switch_line(app).width())
        .unwrap_or(u16::MAX)
        .min(area.width.saturating_sub(2));
    Rect::new(
        area.right().saturating_sub(1 + width),
        area.bottom().saturating_sub(1),
        width,
        1,
    )
}

/// Blank space between the border and the items when the month grid has room for it: one row above and
/// below, one cell left and right
const PADDING: Margin = Margin::new(1, 1);

/// The popup's area (incl. border) on the month grid (`main`): centered. Width follows the longest item
/// (at least `MIN_WIDTH`); height = how many lines that day's items take + top and bottom border; neither
/// exceeds the month grid. Used both for drawing and for mapping mouse positions
pub fn area(app: &App, main: Rect) -> Rect {
    layout(app, main).0
}

/// An item's calendar at the right end of its first line, with the blank cell before it, in the time's
/// color (see `calendar_tag`)
fn tag_span(item: &Item, app: &App) -> Option<Span<'static>> {
    calendar_tag(item).map(|tag| {
        Span::styled(
            format!(" {tag}"),
            Style::new().fg(secondary_color(item, app)),
        )
    })
}

/// One item's line before wrapping: its `item_marker` (colored like the time), its dates only when it spans
/// several days, its time, then its title, e.g. 「 ☐ 17:30 Buy groceries」, 「 ∙ 10/16~18 Parents in town」
fn popup_line(item: &Item, app: &App) -> Line<'static> {
    let mut spans = vec![
        Span::raw(" "),
        Span::styled(
            item_marker(item),
            Style::new().fg(secondary_color(item, app)),
        ),
    ];
    if item.end.is_some() {
        spans.push(Span::styled(
            format!("{} ", date_text(item)),
            Style::new().fg(item_color(item, app)),
        ));
    }
    spans.extend(time_and_title(item, app));
    Line::from(spans)
}

/// One item's lines at `width` cells (one blank cell kept at the end): see `popup_line`, moved in and with
/// its calendar at the right end of the first line as `indent_and_tag` says; a title that does not fit wraps
/// onto the next lines, which start under the title (and leave the calendar's cells free too). Also where
/// each line shows which title characters (`title_cells`)
fn item_lines(item: &Item, app: &App, width: u16) -> (Vec<Line<'static>>, Vec<Vec<TitleCell>>) {
    let (indent, tag) = indent_and_tag(item, app);
    let mut spans = popup_line(item, app).spans;
    spans.insert(0, Span::raw(" ".repeat(indent)));
    // The last span is the title, the ones before it the indent, dates (and time)
    let title = spans.pop().into_iter().collect::<Vec<_>>();
    let prefix_width = Line::from(spans.clone()).width();
    let tag_width = tag.as_ref().map_or(0, Span::width);
    let room = usize::from(width).saturating_sub(prefix_width + 1 + tag_width);
    let wrapped = wrap(&title, room);
    let cells = title_cells(item, &title, &wrapped, prefix_width);
    let mut wrapped = wrapped.into_iter();
    let mut first = Line::from(
        spans
            .into_iter()
            .chain(wrapped.next().unwrap_or_default())
            .collect::<Vec<_>>(),
    );
    if let Some(tag) = tag {
        // Blank cells up to the calendar, which ends one cell before the edge
        let pad = usize::from(width).saturating_sub(first.width() + tag_width + 1);
        first.spans.push(Span::raw(" ".repeat(pad)));
        first.spans.push(tag);
    }
    let mut lines = vec![first];
    lines.extend(wrapped.map(|line| {
        Line::from(
            std::iter::once(Span::raw(" ".repeat(prefix_width)))
                .chain(line)
                .collect::<Vec<_>>(),
        )
    }));
    (lines, cells)
}

/// One title character on a popup line (for dragging over text): where it starts (cells from the start of
/// the line), how many cells wide it is, and which character of the item's title it is (`TextSpot::char`)
#[derive(Debug, Clone, Copy)]
struct TitleCell {
    x: usize,
    width: usize,
    char: usize,
}

/// Which title characters each of an item's lines shows, and where: `wrapped` = `title` (the title as drawn,
/// see `title_span`) cut into lines by `wrap`, each starting `x` cells into its line. 🚧 in front of an
/// in-progress title is not a title character
fn title_cells(
    item: &Item,
    title: &[Span<'static>],
    wrapped: &[Vec<Span<'static>>],
    x: usize,
) -> Vec<Vec<TitleCell>> {
    let shown: Vec<char> = title.iter().flat_map(|span| span.content.chars()).collect();
    // How many characters come before the title itself (🚧 and a space)
    let icon = shown.len().saturating_sub(item.title.chars().count());
    // Which character of `shown` comes next
    let mut next = 0;
    wrapped
        .iter()
        .map(|line| {
            let mut at = x;
            let mut cells = Vec::new();
            for c in line.iter().flat_map(|span| span.content.chars()) {
                // `wrap` only leaves out spaces (where it breaks a line): skip those
                while shown.get(next).is_some_and(|&each| each != c) {
                    next += 1;
                }
                let width = Span::raw(c.to_string()).width();
                if let Some(char) = next.checked_sub(icon) {
                    cells.push(TitleCell { x: at, width, char });
                }
                at += width;
                next += 1;
            }
            cells
        })
        .collect()
}

/// Grouped by calendar (`[day_popup] by_calendar`): an item in a calendar starts 2 cells in, under its
/// calendar's heading, and has nothing at the right end. Not grouped: no indent, its outermost calendar at
/// the right end (`tag_span`)
fn indent_and_tag(item: &Item, app: &App) -> (usize, Option<Span<'static>>) {
    if !app.settings.day_popup.by_calendar {
        return (0, tag_span(item, app));
    }
    (if item.group.is_some() { 2 } else { 0 }, None)
}

/// The line over a group of items when grouped by calendar: their outermost calendar, in the calendar
/// headings' color (as on the tasks page), e.g. 「 CAIS」
fn heading_line(name: &str, app: &App) -> Line<'static> {
    Line::styled(format!(" {name}"), Style::new().fg(app.theme.label))
}

/// One piece of the popup's lines: one item's (`item` = its index in `App::day_items`; `cells` = where each
/// line shows which title characters), or a calendar's heading (`item` = `None`, no `cells`)
struct Piece {
    item: Option<usize>,
    lines: Vec<Line<'static>>,
    cells: Vec<Vec<TitleCell>>,
}

/// The popup's lines at `width` cells, top to bottom: the day's items in `App::day_items` order; grouped by
/// calendar, a heading before the first item of each outermost calendar (none for items outside any
/// calendar, which come first)
fn pieces(app: &App, width: u16) -> Vec<Piece> {
    let mut pieces = Vec::new();
    let mut calendar = None;
    for (index, item) in app.day_items().into_iter().enumerate() {
        let outermost = item.group.as_ref().map(Heading::outermost);
        if app.settings.day_popup.by_calendar
            && let Some(name) = outermost
            && outermost != calendar
        {
            pieces.push(Piece {
                item: None,
                lines: vec![heading_line(name, app)],
                cells: Vec::new(),
            });
        }
        calendar = outermost;
        let (lines, cells) = item_lines(item, app, width);
        pieces.push(Piece {
            item: Some(index),
            lines,
            cells,
        });
    }
    pieces
}

/// How many lines each piece takes
fn heights(pieces: &[Piece]) -> Vec<usize> {
    pieces.iter().map(|piece| piece.lines.len()).collect()
}

/// The popup's area (see `area`) and where the items go inside it: the border taken off, and the padding
/// too, each way only when the box still fits on the month grid with it (too many lines to fit = no rows of
/// padding, so as many lines as possible show)
fn layout(app: &App, main: Rect) -> (Rect, Rect) {
    let grouped = app.settings.day_popup.by_calendar;
    // The widest line: indent, dates, time, title and the calendar at the right end; or a heading
    let widest = app
        .day_items()
        .iter()
        .map(|item| {
            let (indent, tag) = indent_and_tag(item, app);
            let line = indent + popup_line(item, app).width() + tag.as_ref().map_or(0, Span::width);
            let heading = item
                .group
                .as_ref()
                .filter(|_| grouped)
                .map_or(0, |group| heading_line(group.outermost(), app).width());
            line.max(heading)
        })
        .max()
        .unwrap_or(0);
    let to_u16 = |n: usize| u16::try_from(n).unwrap_or(u16::MAX);
    // Left and right border + one blank cell at the end
    let bare_width = to_u16(widest + 3);
    // The widest the box gets: `SIDE_MARGIN` cells free on each side, unless the month grid is narrow
    let room = if main.width >= NARROW {
        main.width - 2 * SIDE_MARGIN
    } else {
        main.width
    };
    let horizontal = if bare_width + 2 * PADDING.horizontal <= room {
        PADDING.horizontal
    } else {
        0
    };
    // The top border fits both titles, with at least one line cell between them
    let (date, counts) = top_titles(app);
    let titles = to_u16(date.width() + counts.as_ref().map_or(0, |counts| counts.width() + 1) + 2);
    let width = (bare_width + 2 * horizontal)
        .max(MIN_WIDTH)
        .max(titles)
        .min(room);
    // Lines: the items wrapped at the width inside the border and padding
    let lines: usize = heights(&pieces(app, width.saturating_sub(2 + 2 * horizontal)))
        .iter()
        .sum();
    let bare_height = to_u16(lines + 2);
    let vertical = if bare_height + 2 * PADDING.vertical <= main.height {
        PADDING.vertical
    } else {
        0
    };
    let height = (bare_height + 2 * vertical).min(main.height);
    let area = Rect::new(
        main.x + (main.width - width) / 2,
        main.y + (main.height - height) / 2,
        width,
        height,
    );
    (
        area,
        Block::bordered()
            .inner(area)
            .inner(Margin::new(horizontal, vertical)),
    )
}

/// Where each piece starts, counting lines from the first piece's first line, and the total
fn starts(heights: &[usize]) -> (Vec<usize>, usize) {
    let mut total = 0;
    let starts = heights
        .iter()
        .map(|height| {
            let start = total;
            total += height;
            start
        })
        .collect();
    (starts, total)
}

/// Draws the popup: first scroll so the chosen item's lines are all visible (only scroll when they fall
/// outside the box, so clicking a visible item does not scroll it away; stored in `app.day_top`, counted in
/// lines), clear the month grid underneath and one blank cell around the box (as far as the month grid goes),
/// then the items line by line. The chosen item's lines get the selection background from border to border
/// (same as layout 1's agenda), and title characters dragged over with the mouse (`App::day_selection`) the
/// `text_selection` background, in the normal text color. When the lines do not fit, the right border has a
/// scrollbar
pub fn render(frame: &mut Frame, main: Rect, app: &mut App) {
    let (area, inner) = layout(app, main);
    let pieces = pieces(app, inner.width);
    let heights = heights(&pieces);
    let (starts, total) = starts(&heights);
    let visible = i64::from(inner.height);
    let to_i64 = |n: usize| i64::try_from(n).unwrap_or(i64::MAX);
    if let Some(chosen) = pieces
        .iter()
        .position(|piece| piece.item == Some(app.day_index))
    {
        let last = starts[chosen] + heights[chosen] - 1;
        // The heading right above comes into view with the item, when both fit
        let first = match chosen.checked_sub(1) {
            Some(above)
                if pieces[above].item.is_none() && heights[chosen] < usize::from(inner.height) =>
            {
                starts[above]
            }
            _ => starts[chosen],
        };
        let top = scrolled_to_range(to_i64(app.day_top), to_i64(first), to_i64(last), visible, 0);
        // Do not overscroll: the last line just touches the bottom
        let max = to_i64(total) - visible;
        app.day_top = usize::try_from(top.min(max).max(0)).unwrap_or(0);
    }
    // Read-only from here on
    let app = &*app;
    let theme = &app.theme;
    let to_u16 = |n: usize| u16::try_from(n).unwrap_or(u16::MAX);
    let selection = app.day_selection();
    let (date, counts) = top_titles(app);
    let mut block = Block::bordered()
        .border_type(BorderType::Rounded)
        .border_style(Style::new().fg(theme.focus))
        .title_top(date);
    if let Some(counts) = counts {
        block = block.title_top(counts);
    }
    let switch = switch_line(app);
    let switch_width = u16::try_from(switch.width()).unwrap_or(u16::MAX);
    block = block.title_bottom(switch);
    clear_with_ring(frame, area, main);
    frame.render_widget(block, area);
    // Bottom border, left: the chosen item's whole calendar path, in the calendar headings' color (as on
    // the tasks page); cut off at the right so a line cell stays before the switch
    if let Some(path) = app
        .day_items()
        .get(app.day_index)
        .and_then(|item| calendar_path(item))
    {
        // Colored by its span, not the line: a line's style would also color the border cells it covers
        let path = Line::from(Span::styled(
            format!(" {path} "),
            Style::new().fg(theme.label),
        ));
        let room = area.width.saturating_sub(2 + switch_width + 1);
        let width = u16::try_from(path.width()).unwrap_or(u16::MAX).min(room);
        frame.render_widget(
            path,
            Rect::new(area.x + 1, area.bottom().saturating_sub(1), width, 1),
        );
    }
    for (piece, start) in pieces.iter().zip(&starts) {
        let chosen = piece.item == Some(app.day_index);
        for (part, line) in piece.lines.iter().enumerate() {
            // Above the top of the box: skip; below its bottom: done
            let Some(offset) = (start + part).checked_sub(app.day_top) else {
                continue;
            };
            let Some(y) = u16::try_from(offset)
                .ok()
                .map(|offset| inner.y + offset)
                .filter(|&y| y < inner.bottom())
            else {
                break;
            };
            let row = Rect {
                y,
                height: 1,
                ..inner
            };
            frame.render_widget(line, row);
            // The chosen item: the selection background from border to border (as in layout 1's agenda)
            if chosen {
                let bar = Rect {
                    x: area.x + 1,
                    width: area.width.saturating_sub(2),
                    ..row
                };
                frame
                    .buffer_mut()
                    .set_style(bar, Style::new().bg(theme.selection));
            }
            // Title characters dragged over with the mouse: their own background, in the normal text color (a
            // priority's red would be hard to read on it)
            if let Some((first, last)) = selection
                && let Some(item) = piece.item
            {
                for cell in piece.cells.get(part).into_iter().flatten() {
                    if (first..=last).contains(&TextSpot {
                        item,
                        char: cell.char,
                    }) {
                        let x = inner.x.saturating_add(to_u16(cell.x));
                        let area = Rect::new(x, y, to_u16(cell.width), 1).intersection(inner);
                        frame
                            .buffer_mut()
                            .set_style(area, Style::new().fg(theme.text).bg(theme.text_selection));
                    }
                }
            }
        }
    }
    if total > usize::from(inner.height) {
        render_scrollbar(
            frame,
            area,
            to_u16(app.day_top),
            to_u16(total - usize::from(inner.height)),
            inner.height,
            theme,
        );
    }
}

/// Which popup item the mouse click at (`column`, `row`) hits (its index in `App::day_items`; any line of a
/// wrapped item); `None` on a calendar's heading, the border, in the padding, outside the box, or below the
/// last item
pub fn item_at(app: &App, main: Rect, column: u16, row: u16) -> Option<usize> {
    let (_, inner) = layout(app, main);
    if !inner.contains(Position::new(column, row)) {
        return None;
    }
    let pieces = pieces(app, inner.width);
    piece_at(&pieces, app.day_top + usize::from(row - inner.y)).and_then(|(piece, _)| piece.item)
}

/// Which title character is at (`column`, `row`), as a `TextSpot`: the one under it; left or right of the
/// title on that line (e.g. on the check box, past the end), that line's first or last one. `None` on a
/// calendar's name, a blank row, the border, or outside the box; with `clamp`, a spot outside the lines
/// counts as the nearest cell inside them (dragging past the box's edge)
pub fn text_at(app: &App, main: Rect, column: u16, row: u16, clamp: bool) -> Option<TextSpot> {
    let (_, inner) = layout(app, main);
    if inner.is_empty() {
        return None;
    }
    let (column, row) = if clamp {
        (
            column.clamp(inner.x, inner.right() - 1),
            row.clamp(inner.y, inner.bottom() - 1),
        )
    } else {
        (column, row)
    };
    if !inner.contains(Position::new(column, row)) {
        return None;
    }
    let pieces = pieces(app, inner.width);
    let (piece, part) = piece_at(&pieces, app.day_top + usize::from(row - inner.y))?;
    let cells = piece.cells.get(part)?;
    let x = usize::from(column - inner.x);
    let cell = cells
        .iter()
        .find(|cell| x < cell.x + cell.width)
        .or(cells.last())?;
    Some(TextSpot {
        item: piece.item?,
        char: cell.char,
    })
}

/// The piece that shows popup line `line` (counting from the first piece's first line), and which of its
/// lines that is; `None` below the last piece
fn piece_at(pieces: &[Piece], line: usize) -> Option<(&Piece, usize)> {
    let (starts, _) = starts(&heights(pieces));
    pieces.iter().zip(starts).find_map(|(piece, start)| {
        let part = line.checked_sub(start)?;
        (part < piece.lines.len()).then_some((piece, part))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_open_count_turns_red_orange_yellow_by_how_much_is_left() {
        let theme = Theme::default();
        // More than half left: red (4 of 6); exactly half or a third: orange (3 of 6, 2 of 6)
        assert_eq!(open_color(4, 6, &theme), theme.priority_high);
        assert_eq!(open_color(3, 6, &theme), theme.priority_medium);
        assert_eq!(open_color(2, 6, &theme), theme.priority_medium);
        // Less than a third, more than a fifth: yellow (2 of 8 = a quarter, 2 of 7)
        assert_eq!(open_color(2, 8, &theme), theme.priority_low);
        assert_eq!(open_color(2, 7, &theme), theme.priority_low);
        // A fifth or less: the normal text color (2 of 10, 1 of 10); none left too
        assert_eq!(open_color(2, 10, &theme), theme.text);
        assert_eq!(open_color(1, 10, &theme), theme.text);
        assert_eq!(open_color(0, 4, &theme), theme.text);
    }
}
