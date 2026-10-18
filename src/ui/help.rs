//! Help page (`?`): drawn over the middle of the screen, just big enough for its content (laid out like v1
//! and tuxedo). It opens as the short version; `Tab` switches to the full version
//!
//! Keys are listed in left and right columns. When the screen is too narrow for two columns, they stack into
//! one column, top to bottom. When it is too short, the page scrolls, with a scrollbar on the right border.
//! The content lives in `Texts::help_short` and `Texts::help_full`

use crate::app::{App, Help};
use crate::texts::{HelpPage, HelpSection};
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
    let (title, hint) = border_lines(page(app, help.full), theme);
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

/// The version shown now: the short one, or the full one when `full`
fn page(app: &App, full: bool) -> HelpPage {
    let texts = app.lang.texts();
    if full {
        texts.help_full
    } else {
        texts.help_short
    }
}

/// The help page's content (line by line) and where the box goes (shared by `render`, `area`, `max_scroll`)
fn layout(app: &App, screen: Rect, full: bool) -> (Vec<Line<'static>>, Rect) {
    let help = page(app, full);
    // The short version aligns section names horizontally. The full version has many sections of very different
    // lengths, so each column runs down on its own (aligning would add many blank lines)
    let lines = arrange(help.left, help.right, &app.theme, screen.width, !full);
    // Size: widest line + right margin + 2 for the border (also wide enough for the top and bottom border
    // text, or it gets cut off); line count + top and bottom padding + 2 for the border (shrunk to the
    // screen if larger)
    let (title, hint) = border_lines(help, &app.theme);
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

/// Text on the top and bottom border: top is `roh-cal ∙ 按鍵` (app name stands out, the rest is muted),
/// bottom is the key hints
fn border_lines(help: HelpPage, theme: &Theme) -> (Line<'static>, Line<'static>) {
    let title = Line::from(vec![
        Span::raw(" "),
        Span::styled(
            "roh-cal",
            Style::new().fg(theme.label).add_modifier(Modifier::BOLD),
        ),
        Span::styled(format!(" ∙ {} ", help.title), Style::new().fg(theme.muted)),
    ]);
    let hint = Line::styled(format!(" {} ", help.hint), Style::new().fg(theme.muted));
    (title, hint)
}

/// Lays out the two columns' sections as lines: if the width (minus the border) fits, side by side (with
/// `align`, the section names at the same position share a line); if not, one column, with the right column
/// below the left one
fn arrange(
    left: &[HelpSection],
    right: &[HelpSection],
    theme: &Theme,
    screen_width: u16,
    align: bool,
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
    let left = section_blocks(left, key_width, theme);
    let right = section_blocks(right, key_width, theme);
    let block_width = |blocks: &[Vec<Line>]| blocks.iter().map(|block| width(block)).max();
    // The right column starts at cell `split`
    let split = block_width(&left).unwrap_or(0) + GAP;
    if split + block_width(&right).unwrap_or(0) + 2 <= usize::from(screen_width) {
        let (left, right) = if align {
            aligned(left, right)
        } else {
            (stacked(left), stacked(right))
        };
        side_by_side(left, right, split)
    } else {
        // One column: the right column goes below the left one, with a blank line between
        let mut lines = stacked(left);
        lines.push(Line::default());
        lines.extend(stacked(right));
        lines
    }
}

/// Lays out each section of one column as lines: the section name (indent 2), then one line per key
/// (indent 4, key column `key_width` cells wide)
fn section_blocks(
    sections: &[HelpSection],
    key_width: usize,
    theme: &Theme,
) -> Vec<Vec<Line<'static>>> {
    let bold = Modifier::BOLD;
    sections
        .iter()
        .map(|section| {
            // Put the color on the text, not the whole line: side by side, this line gets the other column's
            // text appended, and a whole-line color would bleed into it
            let title = Span::styled(
                format!("  {}", section.title),
                Style::new().fg(theme.help_section).add_modifier(bold),
            );
            let mut lines = vec![Line::from(title)];
            for &(key, action) in section.keys {
                let padding = " ".repeat(key_width - Span::raw(key).width());
                lines.push(Line::from(vec![
                    Span::raw("    "),
                    Span::styled(
                        format!("{key}{padding}"),
                        Style::new().fg(theme.help_key).add_modifier(bold),
                    ),
                    Span::styled(action, Style::new().fg(theme.text)),
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

/// Aligns the two columns section by section: of the two sections at the same position, the shorter one
/// gets blank lines below so the next section names share a line; a blank line between sections
fn aligned(
    left: Vec<Vec<Line<'static>>>,
    right: Vec<Vec<Line<'static>>>,
) -> (Vec<Line<'static>>, Vec<Line<'static>>) {
    let count = left.len().max(right.len());
    let (mut left, mut right) = (left.into_iter(), right.into_iter());
    let (mut left_lines, mut right_lines) = (Vec::new(), Vec::new());
    for index in 0..count {
        let mut left_block = left.next().unwrap_or_default();
        let mut right_block = right.next().unwrap_or_default();
        let height = left_block.len().max(right_block.len());
        left_block.resize(height, Line::default());
        right_block.resize(height, Line::default());
        if index > 0 {
            left_lines.push(Line::default());
            right_lines.push(Line::default());
        }
        left_lines.extend(left_block);
        right_lines.extend(right_block);
    }
    (left_lines, right_lines)
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
mod tests {
    use super::*;
    use crate::app::Mode;
    use crate::texts::Lang;
    use jiff::civil::date;
    use ratatui::{Terminal, backend::TestBackend, buffer::Buffer};

    /// Opens the help page (`help`) and draws the whole `width` × `height` screen once (layout 1 underneath);
    /// returns the box's area and the text of each row in the box
    fn draw(lang: Lang, width: u16, height: u16, help: Help) -> (Rect, Vec<String>, Buffer) {
        let mut app = App::new(lang, date(2026, 10, 17), Vec::new());
        app.screen = Rect::new(0, 0, width, height);
        app.mode = Mode::Help(help);
        let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
        terminal
            .draw(|frame| crate::ui::render(frame, &mut app))
            .unwrap();
        let buffer = terminal.backend().buffer().clone();
        let area = area(&app, app.screen, help.full);
        (area, crate::ui::tests::area_rows(&buffer, area), buffer)
    }

    #[test]
    fn short_page_puts_two_columns_in_the_middle_of_the_screen() {
        // Expected box computed by a separate Python script: key column = longest key (Space, [ / ],
        // 5 cells) + 3; the right column starts at the left column's widest line + 1; section names aligned
        // (編輯 has 6 keys, so 移動 gets 2 blank lines below)
        let (area, rows, buffer) = draw(Lang::ZhTw, 70, 23, Help::default());
        // When centering does not divide evenly, the extra cell goes on the left (ratatui's `Flex::Center`):
        // (70 − 49) / 2 = 10.5 → 11
        assert_eq!(area, Rect::new(11, 3, 49, 17));
        assert_eq!(
            rows,
            [
                "╭ roh-cal ∙ 按鍵 ───────────────────────────────╮",
                "│                                               │",
                "│  移動                        編輯             │",
                "│    hjkl    移動                Space   打勾   │",
                "│    [ / ]   上個月 / 下個月     a       新增   │",
                "│    t       今天                e       編輯   │",
                "│    Tab     換框                r       改期   │",
                "│                                d       刪除   │",
                "│                                u       復原   │",
                "│                                               │",
                "│  畫面                        系統             │",
                "│    1       月曆 + 事項         ?       說明   │",
                "│    3       大月曆              ,       設定   │",
                "│    4       任務頁              q       離開   │",
                "│    s       日曆清單                           │",
                "│                                               │",
                "╰ Tab 全部按鍵  ?/q/Esc 關閉 ───────────────────╯",
            ]
        );
        // Colors: the border is lit (keyboard focus is here); section names, keys and descriptions each have
        // their own color
        let theme = Theme::catppuccin_mocha();
        assert_eq!(buffer[(11, 3)].fg, theme.focus);
        assert_eq!(buffer[(14, 5)].fg, theme.help_section);
        assert_eq!(buffer[(16, 6)].fg, theme.help_key);
        assert_eq!(buffer[(24, 6)].fg, theme.text);
        // Outside the box is still the calendar underneath (layout 1's top-left corner)
        assert_eq!(buffer[(0, 0)].symbol(), "╭");
    }

    #[test]
    fn a_narrow_screen_stacks_the_columns() {
        // 44 cells cannot fit two columns (left 28 + right 16 + border 2 = 46): the right column goes below the
        // left one, with a blank line between
        let (area, rows, _) = draw(Lang::ZhTw, 44, 30, Help::default());
        assert_eq!(area, Rect::new(6, 1, 32, 28));
        assert_eq!(
            rows,
            [
                "╭ roh-cal ∙ 按鍵 ──────────────╮",
                "│                              │",
                "│  移動                        │",
                "│    hjkl    移動              │",
                "│    [ / ]   上個月 / 下個月   │",
                "│    t       今天              │",
                "│    Tab     換框              │",
                "│                              │",
                "│  畫面                        │",
                "│    1       月曆 + 事項       │",
                "│    3       大月曆            │",
                "│    4       任務頁            │",
                "│    s       日曆清單          │",
                "│                              │",
                "│  編輯                        │",
                "│    Space   打勾              │",
                "│    a       新增              │",
                "│    e       編輯              │",
                "│    r       改期              │",
                "│    d       刪除              │",
                "│    u       復原              │",
                "│                              │",
                "│  系統                        │",
                "│    ?       說明              │",
                "│    ,       設定              │",
                "│    q       離開              │",
                "│                              │",
                "╰ Tab 全部按鍵  ?/q/Esc 關閉 ──╯",
            ]
        );
    }

    #[test]
    fn a_short_screen_scrolls_with_a_bar_on_the_right_border() {
        // 44 × 20: one column of 24 lines; the box is at most 20 tall and holds 16 lines → can scroll 8 lines;
        // the scrollbar is drawn on the right border
        let (area, rows, _) = draw(Lang::ZhTw, 44, 20, Help::default());
        assert_eq!(area, Rect::new(6, 0, 32, 20));
        let bar = |rows: &[String]| -> Vec<usize> {
            rows.iter()
                .enumerate()
                .filter(|(_, row)| row.ends_with('┃'))
                .map(|(y, _)| y)
                .collect()
        };
        // At the top: the first section name is visible and the scrollbar is at the top
        // (starting on the row below the rounded corner)
        assert_eq!(rows[2], "│  移動                        ┃");
        assert_eq!(bar(&rows).first(), Some(&1));
        // Scrolled to the bottom (overscrolling also stops at the last page): the last key is visible and
        // the scrollbar is at the bottom
        let scrolled = Help {
            full: false,
            scroll: 99,
        };
        let (_, rows, _) = draw(Lang::ZhTw, 44, 20, scrolled);
        assert_eq!(
            rows[17].trim_end_matches(['│', '┃', ' ']),
            "│    q       離開"
        );
        assert_eq!(bar(&rows).last(), Some(&18));
    }

    #[test]
    fn full_page_lists_every_section_side_by_side_without_aligning() {
        for lang in [Lang::En, Lang::ZhTw] {
            let page = lang.texts().help_full;
            let (area, rows, _) = draw(
                lang,
                100,
                38,
                Help {
                    full: true,
                    scroll: 0,
                },
            );
            // The longer column (left) is 32 lines + padding 2 + border 2: it fits, no scrolling
            assert_eq!(area.height, 36, "{lang:?}");
            let text = rows.join("\n");
            for section in page.left.iter().chain(page.right) {
                assert!(text.contains(section.title), "{}", section.title);
            }
            // Side by side: the first section of both columns is on the same row
            let row_of = |text: &str| rows.iter().position(|row| row.contains(text));
            assert_eq!(row_of(page.left[0].title), row_of(page.right[0].title));
            // Section names not aligned: the left column's second section follows right after its first
            // (one blank line between), without waiting for the right column's longer first section
            assert_eq!(
                row_of(page.left[1].title),
                row_of(page.left[0].title).map(|row| row + page.left[0].keys.len() + 2),
                "{lang:?}"
            );
            // The full version leaves only 1 cell on the right: between the right column's widest line
            // (a description) and the right border
            let widest = page
                .right
                .iter()
                .flat_map(|section| section.keys)
                .max_by_key(|(_, action)| Span::raw(*action).width())
                .map(|(_, action)| *action)
                .unwrap();
            let row = &rows[row_of(widest).unwrap()];
            assert_eq!(
                &row[row.find(widest).unwrap() + widest.len()..],
                " │",
                "{lang:?}"
            );
        }
    }
}
