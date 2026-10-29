//! The confirm box: before checking off a task, deleting an item or a calendar, or resetting every setting,
//! a small box in the middle of the screen asks first (like yazi's)
//!
//! ```text
//! ╭────────── 刪除？ ──────────╮
//! │                            │
//! │  Write the design doc      │  ← what it is about (wraps), in the text color
//! │  和底下 2 件子任務         │  ← what goes with it, gray
//! │                            │
//! │────────────────────────────│
//! │   [Y] 刪除      (N) 取消   │  ← the buttons, each in the middle of its half; clicking one = its key
//! ╰────────────────────────────╯
//! ```

use crate::app::App;
use crate::texts::Confirm;
use ratatui::{
    Frame,
    layout::{Constraint, Margin, Position, Rect},
    style::Style,
    text::{Line, Span},
    widgets::{Block, BorderType},
};

/// The narrowest the inside of the box gets
const MIN_WIDTH: u16 = 30;
/// The widest the inside of the box gets; a longer subject wraps
const MAX_WIDTH: u16 = 50;
/// Blank cells left and right of the subject and the detail
const INDENT: u16 = 2;
/// Rows that are always there: the borders, a blank row above and below the subject, the line above the
/// buttons and the buttons
const FIXED_ROWS: u16 = 6;

/// What the confirm box asks now: checking off a task, deleting an item or a calendar, resetting every
/// setting (`R` on the settings page); `None` while it is not asking
pub fn question(app: &App) -> Option<Confirm> {
    let texts = app.lang.texts();
    if let Some(item) = app.confirm_check() {
        Some(texts.confirm_check(&item.title))
    } else if let Some((item, count)) = app.confirm_delete() {
        Some(texts.confirm_delete(&item.title, count))
    } else if let Some((name, count)) = app.confirm_delete_calendar() {
        Some(texts.confirm_delete_calendar(name, count))
    } else {
        app.settings_page()
            .filter(|page| page.confirm_reset)
            .map(|_| texts.confirm_reset_all())
    }
}

/// The two buttons' words: `[Y] 刪除` and `(N) 取消`
fn buttons(app: &App, question: &Confirm) -> [String; 2] {
    [
        format!("[Y] {}", question.yes),
        format!("(N) {}", app.lang.texts().confirm_no),
    ]
}

/// The box's area and the subject's lines (wrapped to fit): as wide as the widest of the question, the
/// subject, the detail and the buttons (`MIN_WIDTH`..=`MAX_WIDTH` inside, 2 cells from each side of
/// `screen` at most), in the middle of `screen`
fn layout(app: &App, screen: Rect, question: &Confirm) -> (Rect, Vec<Line<'static>>) {
    let width = |text: &str| u16::try_from(Span::raw(text).width()).unwrap_or(u16::MAX);
    let [yes, no] = buttons(app, question);
    let wanted = [
        // The question with a blank and at least one `─` on each side
        width(question.question) + 4,
        width(&question.subject) + 2 * INDENT,
        question.detail.as_deref().map_or(0, width) + 2 * INDENT,
        // Each button in its half, with a blank on each side
        2 * (width(&yes).max(width(&no)) + 2),
    ]
    .into_iter()
    .max()
    .unwrap_or(0)
    .clamp(MIN_WIDTH, MAX_WIDTH);
    let inside = wanted.min(screen.width.saturating_sub(6));
    let subject = super::tasks::wrap(
        &[Span::raw(question.subject.clone())],
        usize::from(inside.saturating_sub(2 * INDENT)),
    )
    .into_iter()
    .map(Line::from)
    .collect::<Vec<_>>();
    let rows = u16::try_from(subject.len()).unwrap_or(u16::MAX)
        + u16::from(question.detail.is_some())
        + FIXED_ROWS;
    let area = screen.centered(
        Constraint::Length((inside + 2).min(screen.width)),
        Constraint::Length(rows.min(screen.height)),
    );
    (area, subject)
}

/// Where the two buttons are in the box at `area` (yes, no): each word in the middle of its half of the
/// bottom row inside, with a blank on each side (so it is easier to click)
fn button_areas(app: &App, area: Rect, question: &Confirm) -> [Rect; 2] {
    let row = Rect::new(
        area.x + 1,
        area.bottom().saturating_sub(2),
        area.width.saturating_sub(2),
        1,
    );
    // An odd width: the right half gets the extra cell
    let left = row.width / 2;
    let halves = [
        Rect { width: left, ..row },
        Rect {
            x: row.x + left,
            width: row.width - left,
            ..row
        },
    ];
    let words = buttons(app, question);
    std::array::from_fn(|index| {
        let half = halves[index];
        let width = u16::try_from(Span::raw(words[index].as_str()).width()).unwrap_or(u16::MAX) + 2;
        Rect::new(
            half.x + half.width.saturating_sub(width) / 2,
            half.y,
            width,
            1,
        )
        .intersection(half)
    })
}

/// Draws the confirm box over everything else
pub fn render(frame: &mut Frame, app: &App, question: &Confirm) {
    let theme = &app.theme;
    let (area, subject) = layout(app, frame.area(), question);
    let block = Block::bordered()
        .border_type(BorderType::Rounded)
        .border_style(Style::new().fg(theme.focus))
        .title_top(
            Line::styled(
                format!(" {} ", question.question),
                Style::new().fg(theme.notice),
            )
            .centered(),
        );
    super::clear_with_ring(frame, area, frame.area());
    frame.render_widget(block, area);
    let inside = area.inner(Margin::new(1, 1));
    let text = inside.inner(Margin::new(INDENT, 0));
    // The subject from the second row inside (below a blank one), then the detail
    let mut y = inside.y + 1;
    for line in subject {
        frame.render_widget(line.style(Style::new().fg(theme.text)), Rect { y, ..text });
        y += 1;
    }
    if let Some(detail) = &question.detail {
        frame.render_widget(
            Line::styled(detail.clone(), Style::new().fg(theme.muted)),
            Rect { y, ..text },
        );
    }
    // The line above the buttons, in the border's color
    let line_y = area.bottom().saturating_sub(3);
    frame.render_widget(
        Line::styled(
            "─".repeat(usize::from(inside.width)),
            Style::new().fg(theme.focus),
        ),
        Rect {
            y: line_y,
            ..inside
        },
    );
    // The buttons: the key in the help page's key color, the word in the text color
    for (word, button) in buttons(app, question)
        .into_iter()
        .zip(button_areas(app, area, question))
    {
        let (key, rest) = word.split_at(3);
        frame.render_widget(
            Line::from(vec![
                Span::raw(" "),
                Span::styled(key.to_string(), Style::new().fg(theme.help_key)),
                Span::styled(rest.to_string(), Style::new().fg(theme.text)),
            ]),
            button,
        );
    }
}

/// What a click at (`column`, `row`) means while the confirm box is open: `Some(true)` = on the yes button,
/// `Some(false)` = on the no button or outside the box (cancel), `None` = elsewhere in the box (nothing)
pub fn click(app: &App, screen: Rect, question: &Confirm, column: u16, row: u16) -> Option<bool> {
    let (area, _) = layout(app, screen, question);
    let position = Position::new(column, row);
    let [yes, no] = button_areas(app, area, question);
    if yes.contains(position) {
        Some(true)
    } else if no.contains(position) || !area.contains(position) {
        Some(false)
    } else {
        None
    }
}

#[cfg(test)]
mod tests;
