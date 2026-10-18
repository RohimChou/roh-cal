//! One-line editor: a box in the middle of the main view (like the day popup), with some blank space inside
//! the border when there is room; the `#` list of calendars opens right under the typed text

use crate::app::{App, Editor, Reading, Target};
use crate::edit::Place;
use ratatui::{
    Frame,
    layout::{Margin, Position, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Clear, Paragraph},
};

/// Editor height: top border, the typing row, a blank row, the check-result row, bottom border
const HEIGHT: u16 = 5;

/// The widest the editor gets: a usual task line fits with room to spare; a longer one scrolls
const MAX_WIDTH: u16 = 80;

/// Below this terminal width the editor takes the whole width (no margins, no blank cells inside)
const NARROW: u16 = 30;

/// Blank space between the border and the text when there is room: one row above and below (when the
/// terminal is tall enough), one cell left and right (when it is at least `NARROW` wide)
const PADDING: Margin = Margin::new(1, 1);

/// The most calendars the `#` list shows at once (more: it scrolls)
const LIST_ROWS: usize = 8;

/// The editor's area on the main view (`main`: right of the side menu, above the key-hint row): in the
/// middle both ways, like the day popup; 2 cells from each side, at most `MAX_WIDTH` wide.
/// Used for drawing and for the mouse (a click outside the box = cancel)
pub fn area(main: Rect) -> Rect {
    layout(main).0
}

/// The editor's area (see `area`) and where the text goes inside it: the border taken off, and the
/// padding too when there is room for it
fn layout(main: Rect) -> (Rect, Rect) {
    let rows = main.height;
    let wide = main.width >= NARROW;
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
        (main.width - 4).min(MAX_WIDTH)
    } else {
        main.width
    };
    let area = Rect::new(
        main.x + (main.width - width) / 2,
        main.y + (rows - height) / 2,
        width,
        height,
    );
    (area, Block::bordered().inner(area).inner(padding))
}

/// Where the typed text goes: the first row inside the box, after one blank cell
fn input_area(inner: Rect) -> Rect {
    Rect {
        x: inner.x + 1,
        width: inner.width.saturating_sub(1),
        height: inner.height.min(1),
        ..inner
    }
}

/// How many cells the typed text is scrolled left (so the cursor stays visible, with one cell to spare)
fn input_scroll(editor: &Editor, input_area: Rect) -> usize {
    editor
        .input
        .visual_scroll(usize::from(input_area.width).saturating_sub(1))
}

/// The `#` list's box and which calendar is on its first row; `None` when the list is closed (or there is
/// no room for even one row). The box is right under the typed text with its left border under the `#`
/// (moved left when it would go past the right edge), as wide as the longest calendar or the keys on its
/// bottom border, at most `LIST_ROWS` calendars tall (it may cover the key hints at the bottom); it
/// scrolls so the chosen calendar shows
pub fn list_area(app: &App, screen: Rect, editor: &Editor, keys: &str) -> Option<(Rect, usize)> {
    let found = editor.suggestions()?;
    let (start, _) = editor.hash_word()?;
    let input_area = input_area(layout(super::areas(screen, app).main).1);
    // Where the `#` is on screen: the cells before it, less what the typed text is scrolled
    let before: String = editor.input.value().chars().take(start).collect();
    let column = Span::raw(before)
        .width()
        .saturating_sub(input_scroll(editor, input_area));
    let column = input_area.x + u16::try_from(column).unwrap_or(u16::MAX);
    let names = editor.calendar_names();
    let widest = found
        .iter()
        .map(|found| Span::raw(&names[found.index]).width() + 4)
        .chain([Span::raw(keys).width() + 2])
        .max()
        .unwrap_or(0);
    let width = u16::try_from(widest).unwrap_or(u16::MAX).min(screen.width);
    let y = input_area.y + 1;
    let rows = u16::try_from(found.len().min(LIST_ROWS)).unwrap_or(0);
    let height = (rows + 2).min(screen.bottom().saturating_sub(y));
    if height < 3 {
        return None;
    }
    let x = column
        .saturating_sub(1)
        .min(screen.right().saturating_sub(width));
    let index = editor.hash_list.index.min(found.len() - 1);
    let top = (index + 1).saturating_sub(usize::from(height - 2));
    Some((Rect::new(x, y, width, height), top))
}

/// Draws the editor: the top border says 「編輯事項 ∙ its calendar」 when editing an item (or 「新增 ∙ the
/// calendar it goes into」, 「新增日曆」); row 1 is the typed text
/// (cursor at the typing position); after a blank row, row 3 shows live what the line will be read as (red text when something
/// is wrong, saying what is wrong and that it cannot be saved); the bottom border shows how to save
pub fn render(frame: &mut Frame, editor: &Editor, app: &App) {
    let theme = &app.theme;
    let texts = app.lang.texts();
    let (area, inner) = layout(super::areas(frame.area(), app).main);
    let block = Block::bordered()
        .border_type(BorderType::Rounded)
        .border_style(Style::new().fg(theme.focus))
        .title_top(Line::styled(
            match &editor.target {
                // Editing an item: the calendar it is in (or moves to), e.g. 「 編輯事項 ∙ Work > SFAP 」
                Target::Edit { .. } if let Some(path) = editor.calendar() => {
                    format!("{}∙ {} ", texts.edit_title(), path.join(" > "))
                }
                Target::Edit { .. } => texts.edit_title().to_string(),
                // Adding an item: which calendar it goes into, e.g. 「 新增 ∙ Work > SFAP 」
                Target::Add(_) if let Some(path) = editor.calendar() => {
                    format!("{}∙ {} ", texts.add_title(false), path.join(" > "))
                }
                Target::Add(place) => texts
                    .add_title(matches!(place, Place::ChildOf(_)))
                    .to_string(),
                Target::AddCalendar { .. } => texts.add_calendar_title.to_string(),
                Target::RenameCalendar { .. } => texts.rename_calendar_title.to_string(),
            },
            Style::new().fg(theme.label),
        ))
        .title_bottom(Line::styled(
            texts.editor_keys(!editor.hash_list.calendars.is_empty()),
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
    let input_area = input_area(inner);
    let scroll = input_scroll(editor, input_area);
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
    // Check result: what it reads as (muted), or what is wrong (red); a blank row between it and the typed
    // text (left out when the terminal is too short for it)
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
                y: inner.y + 2.min(inner.height - 1),
                height: 1,
                ..inner
            },
        );
    }
    render_list(frame, editor, app);
}

/// Draws the `#` list over the editor (see `list_area`): one calendar per row, its full path; the characters
/// that match what was typed in the focus color and bold, the chosen row on the selection background; the
/// keys on the bottom border
fn render_list(frame: &mut Frame, editor: &Editor, app: &App) {
    let theme = &app.theme;
    let keys = app.lang.texts().hash_list_keys;
    let (Some((area, top)), Some(found)) = (
        list_area(app, frame.area(), editor, keys),
        editor.suggestions(),
    ) else {
        return;
    };
    let names = editor.calendar_names();
    let index = editor.hash_list.index.min(found.len() - 1);
    super::clear_wide_left_of(frame.buffer_mut(), area);
    frame.render_widget(Clear, area);
    let block = Block::bordered()
        .border_type(BorderType::Rounded)
        .border_style(Style::new().fg(theme.focus))
        .title_bottom(Line::styled(keys, Style::new().fg(theme.muted)));
    let inner = block.inner(area);
    frame.render_widget(block, area);
    for (row, (at, found)) in found
        .iter()
        .enumerate()
        .skip(top)
        .take(usize::from(inner.height))
        .enumerate()
    {
        let mut spans = vec![Span::raw(" ")];
        for (position, character) in names[found.index].chars().enumerate() {
            let style = if found.positions.contains(&position) {
                Style::new().fg(theme.focus).add_modifier(Modifier::BOLD)
            } else {
                Style::new().fg(theme.text)
            };
            spans.push(Span::styled(character.to_string(), style));
        }
        // The chosen row: the background across the whole row, not only under the name
        let row_style = if at == index {
            Style::new().bg(theme.selection)
        } else {
            Style::new()
        };
        frame.render_widget(
            Paragraph::new(Line::from(spans)).style(row_style),
            Rect {
                y: inner.y + u16::try_from(row).unwrap_or(0),
                height: 1,
                ..inner
            },
        );
    }
}
