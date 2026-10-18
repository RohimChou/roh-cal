//! Layout 2's day popup: sits in the middle of the month grid, one item per row (same style as layout 1's
//! agenda), with the selected day on the top border

use super::{clear_wide_left_of, months::scrolled_top, planner::item_line, render_scrollbar};
use crate::app::App;
use ratatui::{
    Frame,
    layout::{Position, Rect},
    style::Style,
    text::Line,
    widgets::{Block, BorderType, Clear},
};

/// Narrowest popup width (incl. border): enough room for the date on the top border
const MIN_WIDTH: u16 = 30;

/// The popup's area (incl. border) on the month grid (`main`): centered. Width follows the longest item
/// (at least `MIN_WIDTH`); height = that day's item count + top and bottom border; neither exceeds the month
/// grid. Used both for drawing and for mapping mouse positions
pub fn area(app: &App, main: Rect) -> Rect {
    let items = app.items_on(app.selected);
    let widest = items
        .iter()
        .map(|item| item_line(item, app, false).width())
        .max()
        .unwrap_or(0);
    // Left and right border + one blank cell at the end
    let width = u16::try_from(widest + 3)
        .unwrap_or(u16::MAX)
        .max(MIN_WIDTH)
        .min(main.width);
    let height = u16::try_from(items.len() + 2)
        .unwrap_or(u16::MAX)
        .min(main.height);
    Rect::new(
        main.x + (main.width - width) / 2,
        main.y + (main.height - height) / 2,
        width,
        height,
    )
}

/// Draws the popup: first scroll so the chosen item is visible (only scroll when it falls outside the box, so
/// clicking a visible item does not scroll it away; stored in `app.day_top`), clear the month grid underneath,
/// then one item per row. The chosen item's date gets brackets `[10/17]` (same as layout 1's agenda). When the
/// items do not fit, the right border has a scrollbar
pub fn render(frame: &mut Frame, main: Rect, app: &mut App) {
    let area = area(app, main);
    let visible = i64::from(Block::bordered().inner(area).height);
    let top = scrolled_top(
        i64::try_from(app.day_top).unwrap_or(0),
        i64::try_from(app.day_index).unwrap_or(0),
        visible,
        0,
    );
    // Do not overscroll: the last item just touches the bottom
    let max = i64::try_from(app.items_on(app.selected).len()).unwrap_or(0) - visible;
    app.day_top = usize::try_from(top.min(max).max(0)).unwrap_or(0);
    // Read-only from here on
    let app = &*app;
    let theme = &app.theme;
    let texts = app.lang.texts();
    let block = Block::bordered()
        .border_type(BorderType::Rounded)
        .border_style(Style::new().fg(theme.focus))
        .title_top(Line::styled(
            format!(" {} ", texts.date_title(app.selected)),
            Style::new().fg(theme.text),
        ));
    let inner = block.inner(area);
    clear_wide_left_of(frame.buffer_mut(), area);
    frame.render_widget(Clear, area);
    frame.render_widget(block, area);
    let items = app.items_on(app.selected);
    let visible = usize::from(inner.height);
    let top = app.day_top;
    let rows = items.iter().enumerate().skip(top);
    for ((index, item), y) in rows.zip(inner.y..inner.bottom()) {
        frame.render_widget(
            item_line(item, app, index == app.day_index),
            Rect {
                y,
                height: 1,
                ..inner
            },
        );
    }
    if items.len() > visible {
        let to_u16 = |n: usize| u16::try_from(n).unwrap_or(u16::MAX);
        render_scrollbar(
            frame,
            area,
            to_u16(top),
            to_u16(items.len() - visible),
            inner.height,
            theme,
        );
    }
}

/// Which popup item the mouse click at (`column`, `row`) hits (its index in `items_on(selected)`);
/// `None` on the border, outside the box, or below the last item
pub fn item_at(app: &App, main: Rect, column: u16, row: u16) -> Option<usize> {
    let inner = Block::bordered().inner(area(app, main));
    if !inner.contains(Position::new(column, row)) {
        return None;
    }
    let index = app.day_top + usize::from(row - inner.y);
    (index < app.items_on(app.selected).len()).then_some(index)
}
