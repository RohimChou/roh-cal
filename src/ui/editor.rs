//! One-line editor: a box in the upper middle of the screen, with some blank space inside the border when
//! there is room

use crate::app::{App, Editor, Reading, Target};
use crate::edit::Place;
use ratatui::{
    Frame,
    layout::{Margin, Position, Rect},
    style::Style,
    text::Line,
    widgets::{Block, BorderType, Paragraph},
};

/// Editor height: top border, the typing row, the check-result row, bottom border
const HEIGHT: u16 = 4;

/// The widest the editor gets: a usual task line fits with room to spare; a longer one scrolls
const MAX_WIDTH: u16 = 80;

/// Below this terminal width the editor takes the whole width (no margins, no blank cells inside)
const NARROW: u16 = 30;

/// Blank space between the border and the text when there is room: one row above and below (when the
/// terminal is tall enough), one cell left and right (when it is at least `NARROW` wide)
const PADDING: Margin = Margin::new(1, 1);

/// The editor's area: centered left to right, 2 cells from each side, at most `MAX_WIDTH` wide; in the
/// upper middle top to bottom (a third of the free rows above it, two thirds below; the key-hint row
/// at the bottom does not count). Used for drawing and for the mouse (a click outside the box = cancel)
pub fn area(screen: Rect) -> Rect {
    layout(screen).0
}

/// The editor's area (see `area`) and where the text goes inside it: the border taken off, and the
/// padding too when there is room for it
fn layout(screen: Rect) -> (Rect, Rect) {
    let rows = screen.height.saturating_sub(1);
    let wide = screen.width >= NARROW;
    let padding = Margin::new(
        if wide { PADDING.horizontal } else { 0 },
        if rows >= HEIGHT + 2 * PADDING.vertical {
            PADDING.vertical
        } else {
            0
        },
    );
    let height = (HEIGHT + 2 * padding.vertical).min(rows);
    let width = if wide {
        (screen.width - 4).min(MAX_WIDTH)
    } else {
        screen.width
    };
    let area = Rect::new(
        screen.x + (screen.width - width) / 2,
        screen.y + (rows - height) / 3,
        width,
        height,
    );
    (area, Block::bordered().inner(area).inner(padding))
}

/// Draws the editor: the top border shows which line is being edited (or 「新增」, 「新增日曆」); row 1 is the
/// typed text
/// (cursor at the typing position); row 2 shows live what the line will be read as (red text when something
/// is wrong, saying what is wrong and that it cannot be saved); the bottom border shows how to save
pub fn render(frame: &mut Frame, editor: &Editor, app: &App) {
    let theme = &app.theme;
    let texts = app.lang.texts();
    let (area, inner) = layout(frame.area());
    let block = Block::bordered()
        .border_type(BorderType::Rounded)
        .border_style(Style::new().fg(theme.focus))
        .title_top(Line::styled(
            match &editor.target {
                Target::Edit(item) => texts.edit_title(item.line),
                Target::Add(place) => texts
                    .add_title(matches!(place, Place::ChildOf(_)))
                    .to_string(),
                Target::AddCalendar { .. } => texts.add_calendar_title.to_string(),
                Target::RenameCalendar { .. } => texts.rename_calendar_title.to_string(),
            },
            Style::new().fg(theme.label),
        ))
        .title_bottom(Line::styled(
            texts.editor_keys(),
            Style::new().fg(theme.muted),
        ));
    // Clear the screen underneath and one blank cell around the box first (not the key-hint row at the
    // bottom), then draw the box
    let screen = frame.area();
    let bounds = Rect {
        height: screen.height.saturating_sub(1),
        ..screen
    };
    super::clear_with_ring(frame, area, bounds);
    frame.render_widget(block, area);
    if inner.height == 0 {
        return;
    }
    // Typed text: scrolls left when too long so the cursor stays visible (one blank cell in front)
    let input_area = Rect {
        x: inner.x + 1,
        width: inner.width.saturating_sub(1),
        height: 1,
        ..inner
    };
    let width = usize::from(input_area.width);
    let scroll = editor.input.visual_scroll(width.saturating_sub(1));
    frame.render_widget(
        Paragraph::new(editor.input.value())
            .style(Style::new().fg(theme.text))
            .scroll((0, u16::try_from(scroll).unwrap_or(u16::MAX))),
        input_area,
    );
    let cursor = editor.input.visual_cursor().saturating_sub(scroll);
    frame.set_cursor_position(Position::new(
        input_area.x + u16::try_from(cursor).unwrap_or(0),
        input_area.y,
    ));
    // Check result: what it reads as (muted), or what is wrong (red)
    if inner.height > 1 {
        let check = match editor.check() {
            Ok(Reading::Item(item)) => {
                Line::styled(texts.preview(&item), Style::new().fg(theme.muted))
            }
            Ok(Reading::Calendar(heading)) => Line::styled(
                texts.calendar_preview(&heading),
                Style::new().fg(theme.muted),
            ),
            Err(problem) => Line::styled(texts.problem(&problem), Style::new().fg(theme.error)),
        };
        frame.render_widget(
            check,
            Rect {
                y: inner.y + 1,
                height: 1,
                ..inner
            },
        );
    }
}
