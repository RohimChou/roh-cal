//! Layout 2's day popup: sits in the middle of the month grid, one item per row (same style as layout 1's
//! agenda; a title too long for the box wraps onto the next lines), with the selected day on the top border
//! and some blank space inside the border when there is room

use super::{
    calendar_path, calendar_tag, clear_with_ring,
    months::scrolled_to_range,
    planner::{item_line, secondary_color},
    render_scrollbar,
    tasks::wrap,
};
use crate::app::App;
use crate::tasks::Item;
use crate::theme::Theme;
use ratatui::{
    Frame,
    layout::{Margin, Position, Rect},
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Block, BorderType},
};

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

/// One item's lines at `width` cells (one blank cell kept at the end): its date and time, then its title,
/// and its calendar's short name at the right end of the first line; a title that does not fit wraps onto
/// the next lines, which start under the title (and leave the calendar's cells free too)
fn item_lines(item: &Item, app: &App, chosen: bool, width: u16) -> Vec<Line<'static>> {
    let mut spans = item_line(item, app, chosen).spans;
    // The last span is the title, the ones before it the date (and time)
    let title = spans.pop().into_iter().collect::<Vec<_>>();
    let prefix_width = Line::from(spans.clone()).width();
    let tag = tag_span(item, app);
    let tag_width = tag.as_ref().map_or(0, Span::width);
    let room = usize::from(width).saturating_sub(prefix_width + 1 + tag_width);
    let mut wrapped = wrap(&title, room).into_iter();
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
    lines
}

/// How many lines each of the day's items takes at `width` cells (see `item_lines`)
fn heights(app: &App, width: u16) -> Vec<usize> {
    app.items_on(app.selected)
        .iter()
        .map(|item| item_lines(item, app, false, width).len())
        .collect()
}

/// The popup's area (see `area`) and where the items go inside it: the border taken off, and the padding
/// too, each way only when the box still fits on the month grid with it (too many lines to fit = no rows of
/// padding, so as many lines as possible show)
fn layout(app: &App, main: Rect) -> (Rect, Rect) {
    let items = app.items_on(app.selected);
    // The widest line: date, time, title and the calendar at the right end
    let widest = items
        .iter()
        .map(|item| {
            item_line(item, app, false).width()
                + tag_span(item, app).as_ref().map_or(0, Span::width)
        })
        .max()
        .unwrap_or(0);
    let to_u16 = |n: usize| u16::try_from(n).unwrap_or(u16::MAX);
    // Left and right border + one blank cell at the end
    let bare_width = to_u16(widest + 3);
    let horizontal = if bare_width + 2 * PADDING.horizontal <= main.width {
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
        .min(main.width);
    // Lines: the items wrapped at the width inside the border and padding
    let lines: usize = heights(app, width.saturating_sub(2 + 2 * horizontal))
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

/// Where each of the day's items starts, counting lines from the first item's first line, and the total
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
/// then the items line by line. The chosen item's date gets brackets `[10/17]` and its lines the selection
/// background from border to border (same as layout 1's agenda). When the lines do not fit, the right border
/// has a scrollbar
pub fn render(frame: &mut Frame, main: Rect, app: &mut App) {
    let (area, inner) = layout(app, main);
    let heights = heights(app, inner.width);
    let (starts, total) = starts(&heights);
    let visible = i64::from(inner.height);
    let to_i64 = |n: usize| i64::try_from(n).unwrap_or(i64::MAX);
    if let (Some(&first), Some(&height)) = (starts.get(app.day_index), heights.get(app.day_index)) {
        let top = scrolled_to_range(
            to_i64(app.day_top),
            to_i64(first),
            to_i64(first + height) - 1,
            visible,
            0,
        );
        // Do not overscroll: the last line just touches the bottom
        let max = to_i64(total) - visible;
        app.day_top = usize::try_from(top.min(max).max(0)).unwrap_or(0);
    }
    // Read-only from here on
    let app = &*app;
    let theme = &app.theme;
    let (date, counts) = top_titles(app);
    let mut block = Block::bordered()
        .border_type(BorderType::Rounded)
        .border_style(Style::new().fg(theme.focus))
        .title_top(date);
    if let Some(counts) = counts {
        block = block.title_top(counts);
    }
    // Bottom border: the chosen item's whole calendar path, in the calendar headings' color (as on the tasks
    // page); cut off at the right if the box is narrower
    if let Some(path) = app
        .items_on(app.selected)
        .get(app.day_index)
        .and_then(|item| calendar_path(item))
    {
        block = block.title_bottom(Line::styled(
            format!(" {path} "),
            Style::new().fg(theme.label),
        ));
    }
    clear_with_ring(frame, area, main);
    frame.render_widget(block, area);
    for (index, (item, start)) in app.items_on(app.selected).iter().zip(&starts).enumerate() {
        let chosen = index == app.day_index;
        for (part, line) in item_lines(item, app, chosen, inner.width)
            .iter()
            .enumerate()
        {
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
        }
    }
    if total > usize::from(inner.height) {
        let to_u16 = |n: usize| u16::try_from(n).unwrap_or(u16::MAX);
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

/// Which popup item the mouse click at (`column`, `row`) hits (its index in `items_on(selected)`; any line
/// of a wrapped item); `None` on the border, in the padding, outside the box, or below the last item
pub fn item_at(app: &App, main: Rect, column: u16, row: u16) -> Option<usize> {
    let (_, inner) = layout(app, main);
    if !inner.contains(Position::new(column, row)) {
        return None;
    }
    let line = app.day_top + usize::from(row - inner.y);
    let heights = heights(app, inner.width);
    let (starts, _) = starts(&heights);
    starts
        .iter()
        .zip(&heights)
        .position(|(&start, &height)| (start..start + height).contains(&line))
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
