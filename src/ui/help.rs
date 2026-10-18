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
                let dim = Style::new().fg(theme.other_month);
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
mod tests {
    use super::*;
    use crate::app::Mode;
    use crate::texts::Lang;
    use jiff::civil::date;
    use ratatui::{Terminal, backend::TestBackend, buffer::Buffer};

    /// Opens the help page (`help`) and draws the whole `width` × `height` screen once (layout 1 underneath,
    /// the keyboard on the calendar); returns the box's area and the text of each row in the box
    fn draw(lang: Lang, width: u16, height: u16, help: Help) -> (Rect, Vec<String>, Buffer) {
        draw_in(Focus::Calendar, lang, width, height, help)
    }

    /// Same as `draw`, with the keyboard in `focus`
    fn draw_in(
        focus: Focus,
        lang: Lang,
        width: u16,
        height: u16,
        help: Help,
    ) -> (Rect, Vec<String>, Buffer) {
        let mut app = App::new(lang, date(2026, 10, 17), Vec::new());
        app.focus = focus;
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
        // Expected box computed by a separate Python script: the keyboard is on the calendar, so the
        // calendar's keys on the left, the views and system keys (they work anywhere) on the right, each
        // column running down on its own; key column = longest key (Enter, [ / ], 5 cells) + 3; the right
        // column starts at the left column's widest line + 1; the top border says where the keyboard is
        let (area, rows, buffer) = draw(Lang::ZhTw, 70, 23, Help::default());
        // When centering does not divide evenly, the extra cell goes on the left (ratatui's `Flex::Center`):
        // (70 − 56) / 2 = 7, (23 − 15) / 2 = 4
        assert_eq!(area, Rect::new(7, 4, 56, 15));
        assert_eq!(
            rows,
            [
                "╭ roh-cal ∙ 按鍵 ∙ 月曆 ───────────────────────────────╮",
                "│                                                      │",
                "│  月曆                        畫面                    │",
                "│    hjkl    移動                1       月曆 + 事項   │",
                "│    [ / ]   上個月 / 下個月     2       大月曆        │",
                "│    t       今天                3       任務頁        │",
                "│    Enter   那天的事            s       日曆清單      │",
                "│    a       在那天新增          Tab     換框          │",
                "│                                                      │",
                "│                              系統                    │",
                "│                                ?       說明          │",
                "│                                ,       設定          │",
                "│                                q       離開          │",
                "│                                                      │",
                "╰ Tab 全部按鍵  ?/q/Esc 關閉 ──────────────────────────╯",
            ]
        );
        // Colors: the border is lit (keyboard focus is here); section names, keys and descriptions each have
        // their own color
        let theme = Theme::catppuccin_mocha();
        assert_eq!(buffer[(7, 4)].fg, theme.focus);
        assert_eq!(buffer[(10, 6)].fg, theme.help_section);
        assert_eq!(buffer[(12, 7)].fg, theme.help_key);
        assert_eq!(buffer[(20, 7)].fg, theme.text);
        // Outside the box is still the calendar underneath (layout 1's top-left corner)
        assert_eq!(buffer[(0, 0)].symbol(), "╭");
    }

    #[test]
    fn short_page_lists_the_keys_for_where_the_keyboard_is() {
        // Expected boxes computed by the same separate Python script: the tasks page, the calendar list, the
        // agenda (the day popup shares the agenda's keys)
        let (area, rows, _) = draw_in(Focus::Tasks, Lang::ZhTw, 70, 23, Help::default());
        assert_eq!(area, Rect::new(6, 4, 58, 15));
        assert_eq!(
            rows,
            [
                "╭ roh-cal ∙ 按鍵 ∙ 任務頁 ───────────────────────────────╮",
                "│                                                        │",
                "│  任務頁                        畫面                    │",
                "│    j / k   選                    1       月曆 + 事項   │",
                "│    l / h   展開 / 收起           2       大月曆        │",
                "│    Space   打勾                  3       任務頁        │",
                "│    e       編輯                  s       日曆清單      │",
                "│    a / A   新增 / 新增子任務     Tab     換框          │",
                "│    r       改期                                        │",
                "│    d       刪除                系統                    │",
                "│    u       復原                  ?       說明          │",
                "│                                  ,       設定          │",
                "│                                  q       離開          │",
                "│                                                        │",
                "╰ Tab 全部按鍵  ?/q/Esc 關閉 ────────────────────────────╯",
            ]
        );
        let (_, rows, _) = draw_in(Focus::Side, Lang::ZhTw, 70, 23, Help::default());
        assert_eq!(
            rows,
            [
                "╭ roh-cal ∙ 按鍵 ∙ 日曆清單 ───────────────────────────╮",
                "│                                                      │",
                "│  日曆清單                    畫面                    │",
                "│    j / k   選                  1       月曆 + 事項   │",
                "│    Space   勾選                2       大月曆        │",
                "│    a       新增日曆            3       任務頁        │",
                "│    e       改名                s       日曆清單      │",
                "│    d       刪除                Tab     換框          │",
                "│    L / H   往內 / 往外一層                           │",
                "│                              系統                    │",
                "│                                ?       說明          │",
                "│                                ,       設定          │",
                "│                                q       離開          │",
                "│                                                      │",
                "╰ Tab 全部按鍵  ?/q/Esc 關閉 ──────────────────────────╯",
            ]
        );
        let (_, rows, _) = draw_in(Focus::Agenda, Lang::ZhTw, 70, 23, Help::default());
        assert_eq!(
            rows,
            [
                "╭ roh-cal ∙ 按鍵 ∙ 事項 ─────────────────────────────────╮",
                "│                                                        │",
                "│  事項                          畫面                    │",
                "│    j / k   選                    1       月曆 + 事項   │",
                "│    Space   打勾                  2       大月曆        │",
                "│    e       編輯                  3       任務頁        │",
                "│    a / A   新增 / 新增子任務     s       日曆清單      │",
                "│    r       改期                  Tab     換框          │",
                "│    d       刪除                                        │",
                "│    u       復原                系統                    │",
                "│                                  ?       說明          │",
                "│                                  ,       設定          │",
                "│                                  q       離開          │",
                "│                                                        │",
                "╰ Tab 全部按鍵  ?/q/Esc 關閉 ────────────────────────────╯",
            ]
        );
        let (_, rows, _) = draw_in(Focus::Day, Lang::ZhTw, 70, 23, Help::default());
        assert!(
            rows[0].starts_with("╭ roh-cal ∙ 按鍵 ∙ 那天的事 "),
            "{}",
            rows[0]
        );
        assert!(rows[2].starts_with("│  事項 "), "{}", rows[2]);
    }

    #[test]
    fn a_narrow_screen_stacks_the_columns() {
        // 44 cells cannot fit two columns (left 27 + 1 + right 23 + border 2 = 53): the right column goes below
        // the left one, with a blank line between
        let (area, rows, _) = draw(Lang::ZhTw, 44, 30, Help::default());
        assert_eq!(area, Rect::new(6, 4, 32, 22));
        assert_eq!(
            rows,
            [
                "╭ roh-cal ∙ 按鍵 ∙ 月曆 ───────╮",
                "│                              │",
                "│  月曆                        │",
                "│    hjkl    移動              │",
                "│    [ / ]   上個月 / 下個月   │",
                "│    t       今天              │",
                "│    Enter   那天的事          │",
                "│    a       在那天新增        │",
                "│                              │",
                "│  畫面                        │",
                "│    1       月曆 + 事項       │",
                "│    2       大月曆            │",
                "│    3       任務頁            │",
                "│    s       日曆清單          │",
                "│    Tab     換框              │",
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
        // 44 × 20: one column of 18 lines; the box is at most 20 tall and holds 16 lines → can scroll 2 lines;
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
        assert_eq!(rows[2], "│  月曆                        ┃");
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
            let page = lang.texts().help;
            let (area, rows, _) = draw(
                lang,
                100,
                38,
                Help {
                    full: true,
                    scroll: 0,
                },
            );
            // The longer column (left) is 33 lines + padding 2 + border 2: it fits, no scrolling
            assert_eq!(area.height, 37, "{lang:?}");
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

    #[test]
    fn full_page_grays_out_the_sections_that_do_not_work_here() {
        let theme = Theme::catppuccin_mocha();
        // The color of the piece of text that is exactly `text` (a section name with its indent, a key, a
        // description) on the full page, with the keyboard in `focus`
        let color_of = |focus: Focus, text: &str| {
            let mut app = App::new(Lang::ZhTw, date(2026, 10, 17), Vec::new());
            app.focus = focus;
            let (lines, _) = layout(&app, Rect::new(0, 0, 100, 38), true);
            lines
                .iter()
                .flat_map(|line| line.spans.iter())
                .find(|span| span.content.trim_end() == text)
                .and_then(|span| span.style.fg)
                .expect(text)
        };
        // Keyboard on the calendar: the calendar's keys and the ones that work anywhere are lit; the tasks
        // page's and the calendar list's are dimmed, keys and descriptions too
        let calendar = Focus::Calendar;
        assert_eq!(color_of(calendar, "  月曆"), theme.help_section);
        assert_eq!(color_of(calendar, "  畫面"), theme.help_section);
        assert_eq!(color_of(calendar, "  編輯框"), theme.help_section);
        assert_eq!(color_of(calendar, "  任務頁"), theme.other_month);
        assert_eq!(color_of(calendar, "  日曆清單"), theme.other_month);
        assert_eq!(color_of(calendar, "+ / -"), theme.other_month);
        assert_eq!(color_of(calendar, "全部展開 / 全部收起"), theme.other_month);
        // On the tasks page: the other way round for the calendar and the tasks page
        assert_eq!(color_of(Focus::Tasks, "  任務頁"), theme.help_section);
        assert_eq!(color_of(Focus::Tasks, "+ / -"), theme.help_key);
        assert_eq!(color_of(Focus::Tasks, "  月曆"), theme.other_month);
        assert_eq!(
            color_of(Focus::Tasks, "  事項、跳窗、任務頁"),
            theme.help_section
        );
        // The top border says what gray means
        let (_, rows, _) = draw(
            Lang::ZhTw,
            100,
            38,
            Help {
                full: true,
                scroll: 0,
            },
        );
        assert!(
            rows[0].starts_with("╭ roh-cal ∙ 全部按鍵，灰的這裡用不到 "),
            "{}",
            rows[0]
        );
    }
}
