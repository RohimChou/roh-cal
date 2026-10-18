//! Help page (`?`): drawn over the middle of the screen, just big enough for its content (laid out like v1
//! and tuxedo). It opens as the short version: the keys that work where the keyboard is (e.g. on the tasks
//! page, the tasks page's keys); `Tab` switches to the full version: every key, the sections that do not work
//! where the keyboard is grayed out
//!
//! Keys are listed in left and right columns. When the screen is too narrow for two columns, they stack into
//! one column, top to bottom. When it is too short, the page scrolls, with a scrollbar on the right border.
//! The content lives in `Texts::help`

use crate::app::{App, Focus, Help};
use crate::texts::HelpSection;
use crate::theme::Theme;
use ratatui::{
    Frame,
    layout::{Constraint, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Clear, Padding, Paragraph},
};

/// Minimum gap between a key and its description (key column width = longest key in both columns + this)
const KEY_GAP: usize = 3;
/// Minimum gap between the two columns (the right column has its own indent: section names 2, keys 4,
/// so 1 cell is enough to keep them apart)
const GAP: usize = 1;
/// Gap between the short version's content and the right border: looks as wide as the gap between the
/// columns (`GAP` + the section-name indent of 2), so the left, middle and right margins are even;
/// the full version leaves only 1 cell
const SHORT_RIGHT_MARGIN: usize = GAP + 2;
/// Between border and content: 1 blank line at top and bottom (the section-name indent spaces the left side)
const PADDING: Padding = Padding::vertical(1);

/// Draws the help page: just big enough for its content, in the middle of the screen. When the terminal is
/// too short to fit it, drawing starts `help.scroll` lines down and a scrollbar is drawn on the right border
pub fn render(frame: &mut Frame, app: &App, help: Help) {
    let theme = &app.theme;
    let (lines, area) = layout(app, frame.area(), help.full);
    // Overscrolled (e.g. scrolled to the bottom, then the terminal got taller): stop at the last page
    let max = overflow(lines.len(), area);
    let scroll = help.scroll.min(max);
    let (title, hint) = border_lines(app, help.full);
    let block = Block::bordered()
        .border_type(BorderType::Rounded)
        .border_style(Style::new().fg(theme.focus))
        .padding(PADDING)
        .title_top(title)
        .title_bottom(hint);
    // Clear the screen underneath first so the box does not overlap the calendar's text
    super::clear_wide_left_of(frame.buffer_mut(), area);
    frame.render_widget(Clear, area);
    frame.render_widget(Paragraph::new(lines).block(block).scroll((scroll, 0)), area);
    if max > 0 {
        super::render_scrollbar(frame, area, scroll, max, visible_rows(area), theme);
    }
}

/// The help box's area (incl. border) on `screen` (a mouse click outside the box = close)
pub fn area(app: &App, screen: Rect, full: bool) -> Rect {
    layout(app, screen, full).1
}

/// How many lines the help page can scroll down at most (based on the size of `screen`); 0 when it fits
pub fn max_scroll(app: &App, screen: Rect, full: bool) -> u16 {
    let (lines, area) = layout(app, screen, full);
    overflow(lines.len(), area)
}

/// How many content lines fit in the box: minus the top and bottom border and padding
fn visible_rows(area: Rect) -> u16 {
    area.height.saturating_sub(PADDING.top + PADDING.bottom + 2)
}

/// How many of `lines` lines do not fit in the box at `area` (= the most it can scroll down)
fn overflow(lines: usize, area: Rect) -> u16 {
    u16::try_from(lines)
        .unwrap_or(u16::MAX)
        .saturating_sub(visible_rows(area))
}

/// The sections of the version shown now, (left column, right column): the full version (`full`) has
/// every section where `Texts::help` puts it; the short one only the short sections that work where the
/// keyboard is, those for that place on the left and those that work anywhere (views, system) on the right
fn sections(app: &App, full: bool) -> (Vec<HelpSection>, Vec<HelpSection>) {
    let help = app.lang.texts().help;
    if full {
        return (help.left.to_vec(), help.right.to_vec());
    }
    help.short
        .iter()
        .filter(|section| section.works_in(app.focus))
        .partition(|section| !section.places.is_empty())
}

/// The help page's content (line by line) and where the box goes (shared by `render`, `area`, `max_scroll`)
fn layout(app: &App, screen: Rect, full: bool) -> (Vec<Line<'static>>, Rect) {
    let (left, right) = sections(app, full);
    // The full version grays out the sections that do not work where the keyboard is
    let here = full.then_some(app.focus);
    let lines = arrange(&left, &right, &app.theme, screen.width, here);
    // Size: widest line + right margin + 2 for the border (also wide enough for the top and bottom border
    // text, or it gets cut off); line count + top and bottom padding + 2 for the border (shrunk to the
    // screen if larger)
    let (title, hint) = border_lines(app, full);
    let right_margin = if full { 1 } else { SHORT_RIGHT_MARGIN };
    let inside = (width(&lines) + right_margin)
        .max(title.width())
        .max(hint.width());
    let box_width = u16::try_from(inside + 2).unwrap_or(u16::MAX);
    let box_height = u16::try_from(lines.len())
        .unwrap_or(u16::MAX)
        .saturating_add(PADDING.top + PADDING.bottom + 2);
    // The help page belongs to no day and no box: put it in the middle of the whole screen
    let area = screen.centered(
        Constraint::Length(box_width.min(screen.width)),
        Constraint::Length(box_height.min(screen.height)),
    );
    (lines, area)
}

/// Text on the top and bottom border: top is `roh-cal ∙ 按鍵 ∙ 任務頁` in the short version (where the
/// keyboard is) and `roh-cal ∙ 全部按鍵，灰的這裡用不到` in the full one (app name stands out, the rest is
/// muted), bottom is the key hints
fn border_lines(app: &App, full: bool) -> (Line<'static>, Line<'static>) {
    let theme = &app.theme;
    let texts = app.lang.texts();
    let (title, hint) = if full {
        (texts.help.full_title.to_string(), texts.help.full_hint)
    } else {
        let place = texts.help_place(app.focus);
        (
            format!("{} ∙ {place}", texts.help.short_title),
            texts.help.short_hint,
        )
    };
    let title = Line::from(vec![
        Span::raw(" "),
        Span::styled(
            "roh-cal",
            Style::new().fg(theme.label).add_modifier(Modifier::BOLD),
        ),
        Span::styled(format!(" ∙ {title} "), Style::new().fg(theme.muted)),
    ]);
    let hint = Line::styled(format!(" {hint} "), Style::new().fg(theme.muted));
    (title, hint)
}

/// Lays out the two columns' sections as lines: if the width (minus the border) fits, side by side, each
/// column running down on its own; if not, one column, with the right column below the left one.
/// `here` = gray out the sections that do not work with the keyboard there
fn arrange(
    left: &[HelpSection],
    right: &[HelpSection],
    theme: &Theme,
    screen_width: u16,
    here: Option<Focus>,
) -> Vec<Line<'static>> {
    // The key column is equally wide in both columns: the longest key in either column + `KEY_GAP`
    let key_width = left
        .iter()
        .chain(right)
        .flat_map(|section| section.keys)
        .map(|&(key, _)| Span::raw(key).width())
        .max()
        .unwrap_or(0)
        + KEY_GAP;
    let left = section_blocks(left, key_width, theme, here);
    let right = section_blocks(right, key_width, theme, here);
    let block_width = |blocks: &[Vec<Line>]| blocks.iter().map(|block| width(block)).max();
    // The right column starts at cell `split`
    let split = block_width(&left).unwrap_or(0) + GAP;
    if split + block_width(&right).unwrap_or(0) + 2 <= usize::from(screen_width) {
        side_by_side(stacked(left), stacked(right), split)
    } else {
        // One column: the right column goes below the left one, with a blank line between
        let mut lines = stacked(left);
        lines.push(Line::default());
        lines.extend(stacked(right));
        lines
    }
}

/// Lays out each section of one column as lines: the section name (indent 2), then one line per key
/// (indent 4, key column `key_width` cells wide). With `here`, a section that does not work with the
/// keyboard there is all in the other-month gray (a step darker than muted text)
fn section_blocks(
    sections: &[HelpSection],
    key_width: usize,
    theme: &Theme,
    here: Option<Focus>,
) -> Vec<Vec<Line<'static>>> {
    sections
        .iter()
        .map(|section| {
            let gray = here.is_some_and(|focus| !section.works_in(focus));
            let (title_style, key_style, action_style) = if gray {
                let dim = theme.dimmed();
                (dim, dim, dim)
            } else {
                let bold = Modifier::BOLD;
                (
                    Style::new().fg(theme.help_section).add_modifier(bold),
                    Style::new().fg(theme.help_key).add_modifier(bold),
                    Style::new().fg(theme.text),
                )
            };
            // Put the color on the text, not the whole line: side by side, this line gets the other column's
            // text appended, and a whole-line color would bleed into it
            let title = Span::styled(format!("  {}", section.title), title_style);
            let mut lines = vec![Line::from(title)];
            for &(key, action) in section.keys {
                let padding = " ".repeat(key_width - Span::raw(key).width());
                lines.push(Line::from(vec![
                    Span::raw("    "),
                    Span::styled(format!("{key}{padding}"), key_style),
                    Span::styled(action, action_style),
                ]));
            }
            lines
        })
        .collect()
}

/// Joins one column's sections top to bottom, with a blank line between sections
fn stacked(blocks: Vec<Vec<Line<'static>>>) -> Vec<Line<'static>> {
    let mut lines = Vec::new();
    for (index, block) in blocks.into_iter().enumerate() {
        if index > 0 {
            lines.push(Line::default());
        }
        lines.extend(block);
    }
    lines
}

/// How many cells the widest line takes (a Chinese character counts as 2)
fn width(lines: &[Line]) -> usize {
    lines.iter().map(Line::width).max().unwrap_or(0)
}

/// Puts the two columns side by side: pad each left line with spaces to `split` cells, then append the same
/// line of the right column; the shorter column is left blank below
fn side_by_side(
    left: Vec<Line<'static>>,
    right: Vec<Line<'static>>,
    split: usize,
) -> Vec<Line<'static>> {
    let rows = left.len().max(right.len());
    let mut left = left.into_iter();
    let mut right = right.into_iter();
    (0..rows)
        .map(|_| {
            let mut line = left.next().unwrap_or_default();
            let padding = split.saturating_sub(line.width());
            line.push_span(Span::raw(" ".repeat(padding)));
            for span in right.next().unwrap_or_default() {
                line.push_span(span);
            }
            line
        })
        .collect()
}

#[cfg(test)]
mod tests;
