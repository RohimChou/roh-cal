//! One-line editor: drawn at the bottom (above the key hints), as wide as the whole terminal

use crate::app::{App, Editor, Target};
use crate::edit::Place;
use ratatui::{
    Frame,
    layout::{Position, Rect},
    style::Style,
    text::Line,
    widgets::{Block, BorderType, Clear, Paragraph},
};

/// Editor height: top border, the typing row, the check-result row, bottom border
const HEIGHT: u16 = 4;

/// The editor's area: above the key hints at the bottom, as wide as the terminal. Used for drawing and for
/// the mouse (a click outside the box = cancel)
pub fn area(screen: Rect) -> Rect {
    let height = HEIGHT.min(screen.height.saturating_sub(1));
    Rect::new(
        screen.x,
        screen.bottom().saturating_sub(1 + height),
        screen.width,
        height,
    )
}

/// Draws the editor: the top border shows which line is being edited (or 「新增」); row 1 is the typed text
/// (cursor at the typing position); row 2 shows live what the line will be read as (red text when something
/// is wrong, saying what is wrong and that it cannot be saved); the bottom border shows how to save
pub fn render(frame: &mut Frame, editor: &Editor, app: &App) {
    let theme = &app.theme;
    let texts = app.lang.texts();
    let area = area(frame.area());
    let block = Block::bordered()
        .border_type(BorderType::Rounded)
        .border_style(Style::new().fg(theme.focus))
        .title_top(Line::styled(
            match &editor.target {
                Target::Edit(item) => texts.edit_title(item.line),
                Target::Add(place) => texts
                    .add_title(matches!(place, Place::ChildOf(_)))
                    .to_string(),
            },
            Style::new().fg(theme.label),
        ))
        .title_bottom(Line::styled(
            texts.editor_keys(),
            Style::new().fg(theme.muted),
        ));
    let inner = block.inner(area);
    // Clear the screen underneath first, then draw the box
    frame.render_widget(Clear, area);
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
            Ok(item) => Line::styled(texts.preview(&item), Style::new().fg(theme.muted)),
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
