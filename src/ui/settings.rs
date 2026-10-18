//! Settings page (`,`): a box over the middle of the screen that lists the settings the way the config
//! file looks: top-level ones (`theme`) first, then one section per table (`[calendar]`, `[month]`,
//! `[colors]`); the bottom line explains what the chosen row does. Settings that do nothing on the current
//! view (e.g. the month grid's on layout 1) are dimmed, and choosing one says below which views it is for
//!
//! ```text
//! ╭ roh-cal ∙ 設定 ─────────────────────────╮
//! │                                         │
//! │  theme              catppuccin-mocha    │  ← the chosen row gets a background color
//! │                                         │
//! │  [calendar]                             │
//! │    relative_weeks   off                 │
//! │  ...                                    │
//! │  [colors]                               │
//! │    text             ██ text             │  ← swatch = the role's current color, then its palette name
//! │  ...                                    ┃  ← scrolls when it does not fit (the chosen row is
//! │                                         │     always visible), scrollbar on the right border
//! │  顏色：l 換下一套 theme，h 換上一套     │  ← the description stays at the bottom and does not scroll
//! │                                         │  ← for a dimmed row: 「這個畫面用不到：2 大月曆」
//! │                                         │
//! ╰ jk 選  hl 改  r 還原  R 全部還原  Esc 關閉 ╯
//! ```

use crate::app::{App, SettingsPage, rows};
use crate::theme;
use ratatui::{
    Frame,
    layout::{Constraint, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Clear, Padding, Paragraph},
};

/// Minimum gap between a name and its value (name column width = longest name + this)
const NAME_GAP: usize = 3;
/// Between border and content: 1 blank line at top and bottom (the section-name indent spaces the left side)
const PADDING: Padding = Padding::vertical(1);
/// Gap between the content and the right border (same as the section-name indent on the left)
const RIGHT_MARGIN: usize = 2;
/// The swatch before the value on color rows: 2 blank cells with that color as background, then 1 blank
const SWATCH: &str = "  ";
/// Fixed lines below the list: a blank line, the description, and the line saying which views a dimmed
/// row is for (blank otherwise)
const FOOTER: u16 = 3;
/// When scrolling, the minimum lines between the chosen row and the top or bottom edge
const SCROLLOFF: i64 = 1;

/// One line in the list: a section name, a setting row (an index into `rows()`), or a blank line
enum Entry {
    /// A section name like `[calendar]`
    Section(&'static str),
    /// A setting row: an index into `rows()`
    Row(usize),
    /// A blank line
    Blank,
}

/// What each line of the list is, top to bottom: top-level rows first (no section name); each section
/// is its name + its rows, with a blank line between sections
fn entries() -> Vec<Entry> {
    let rows = rows();
    let mut entries = Vec::new();
    for (index, row) in rows.iter().enumerate() {
        let new_section = index == 0 || rows[index - 1].section() != row.section();
        if new_section {
            if index > 0 {
                entries.push(Entry::Blank);
            }
            if let Some(section) = row.section() {
                entries.push(Entry::Section(section));
            }
        }
        entries.push(Entry::Row(index));
    }
    entries
}

/// Which list line row `row` (an index into `rows()`) is on
fn line_of(row: usize) -> Option<usize> {
    entries()
        .iter()
        .position(|entry| matches!(entry, Entry::Row(index) if *index == row))
}

/// Width of the name column: the longest name + `NAME_GAP`
fn name_width() -> usize {
    rows().iter().map(|row| row.name().len()).max().unwrap_or(0) + NAME_GAP
}

/// The widest the value column gets: a switch's `off`, the longest theme name, or swatch + the longest
/// color name (counting every theme's palette)
fn value_width() -> usize {
    let names = theme::names();
    let colors = names
        .iter()
        .flat_map(|name| theme::choices(name))
        .map(|(name, _)| SWATCH.len() + 1 + name.len());
    names
        .iter()
        .map(|name| name.len())
        .chain(colors)
        .chain(["off".len()])
        .max()
        .unwrap_or(0)
}

/// The box's area: just big enough for the content (sized for the longest description, so the box does
/// not resize when another row is chosen), in the middle of `screen`;
/// if taller than the screen, it shrinks to the screen height (the list can scroll)
pub fn area(app: &App, screen: Rect) -> Rect {
    let texts = app.lang.texts();
    // One row: indent 4 + name column + value
    let row_width = 4 + name_width() + value_width();
    let description_width = rows()
        .iter()
        .flat_map(|&row| {
            let elsewhere = row.views().map(|views| texts.setting_elsewhere(views));
            [texts.setting_description(row).to_string()]
                .into_iter()
                .chain(elsewhere)
        })
        .map(|text| Span::raw(&text).width() + 2)
        .max()
        .unwrap_or(0);
    // The bottom border fits the key hints and also the question shown after `R`
    let hint_width = [texts.settings_hint, texts.settings_reset_all]
        .map(|hint| Span::raw(hint).width() + 2)
        .into_iter()
        .max()
        .unwrap_or(0);
    let inside = (row_width.max(description_width) + RIGHT_MARGIN)
        .max(title(app).width())
        .max(hint_width);
    let width = u16::try_from(inside + 2).unwrap_or(u16::MAX);
    let height = u16::try_from(entries().len())
        .unwrap_or(u16::MAX)
        .saturating_add(FOOTER + PADDING.top + PADDING.bottom + 2);
    screen.centered(
        Constraint::Length(width.min(screen.width)),
        Constraint::Length(height.min(screen.height)),
    )
}

/// The inside of the box (minus border and padding)
fn inner(area: Rect) -> Rect {
    Block::bordered().padding(PADDING).inner(area)
}

/// The list area: the inside of the box minus the fixed blank line and description at the bottom
fn list_area(area: Rect) -> Rect {
    let inner = inner(area);
    Rect {
        height: inner.height.saturating_sub(FOOTER),
        ..inner
    }
}

/// Where to scroll: starting from `page.top`, keep the chosen row at least `SCROLLOFF` lines from the
/// top and bottom edges, scrolling only when needed; never overscroll
pub fn scrolled(app: &App, screen: Rect, page: SettingsPage) -> usize {
    let height = i64::from(list_area(area(app, screen)).height);
    let line = line_of(page.row).unwrap_or(0);
    let top = super::months::scrolled_top(
        i64::try_from(page.top).unwrap_or(0),
        i64::try_from(line).unwrap_or(0),
        height,
        SCROLLOFF,
    );
    let max = i64::try_from(entries().len()).unwrap_or(0) - height;
    usize::try_from(top.min(max).max(0)).unwrap_or(0)
}

/// The top border: `roh-cal ∙ 設定` (the app name stands out, the rest is muted)
fn title(app: &App) -> Line<'static> {
    let theme = &app.theme;
    Line::from(vec![
        Span::raw(" "),
        Span::styled(
            "roh-cal",
            Style::new().fg(theme.label).add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            format!(" ∙ {} ", app.lang.texts().settings_title),
            Style::new().fg(theme.muted),
        ),
    ])
}

/// Text on the bottom border: normally the key hints; while asking whether to reset everything
/// (`page.confirm_reset`), the question instead
fn hint_text(app: &App, page: SettingsPage) -> &'static str {
    let texts = app.lang.texts();
    if page.confirm_reset {
        texts.settings_reset_all
    } else {
        texts.settings_hint
    }
}

/// Draws the settings page: each row is "name value" (top-level indented 2, table rows indented 4,
/// values aligned in one column; colors have a swatch in front; rows that do nothing on the current view
/// are dimmed to the other-month gray, a step darker than muted text), the chosen row gets a background color; drawing starts at line `page.top` (scrolled first
/// with `scrolled`); the bottom says what the chosen row does (and, when it is dimmed, which views it is
/// for)
pub fn render(frame: &mut Frame, app: &App, page: SettingsPage) {
    let theme = &app.theme;
    let texts = app.lang.texts();
    let area = area(app, frame.area());
    let hint_style = if page.confirm_reset {
        Style::new().fg(theme.notice)
    } else {
        Style::new().fg(theme.muted)
    };
    let block = Block::bordered()
        .border_type(BorderType::Rounded)
        .border_style(Style::new().fg(theme.focus))
        .padding(PADDING)
        .title_top(title(app))
        .title_bottom(Line::styled(
            format!(" {} ", hint_text(app, page)),
            hint_style,
        ));
    let rows = rows();
    let chosen = rows[page.row.min(rows.len() - 1)];
    let lines: Vec<Line> = entries()
        .iter()
        .map(|entry| match *entry {
            Entry::Section(name) => Line::styled(
                format!("  [{name}]"),
                Style::new()
                    .fg(theme.help_section)
                    .add_modifier(Modifier::BOLD),
            ),
            Entry::Row(index) => {
                let row = rows[index];
                let applies = row.applies_to(app.view);
                let (name_color, value_color) = if applies {
                    (theme.help_key, theme.text)
                } else {
                    (theme.other_month, theme.other_month)
                };
                let mut value_style = Style::new().fg(value_color);
                if index == page.row {
                    value_style = value_style.add_modifier(Modifier::BOLD);
                }
                // Table rows: 2 more indent, a name column 2 narrower, so the values line up
                let (indent, width) = match row.section() {
                    Some(_) => ("    ", name_width()),
                    None => ("  ", name_width() + 2),
                };
                let mut spans = vec![Span::styled(
                    format!("{indent}{:<width$}", row.name()),
                    Style::new().fg(name_color),
                )];
                // Colors: draw the swatch first (background = the role's current color; `Reset` =
                // the terminal's default color, which looks empty)
                if let Some(color) = row.color(&app.theme) {
                    spans.push(Span::styled(SWATCH, Style::new().bg(color)));
                    spans.push(Span::raw(" "));
                }
                spans.push(Span::styled(
                    row.value(&app.settings, &app.theme),
                    value_style,
                ));
                Line::from(spans)
            }
            Entry::Blank => Line::default(),
        })
        .collect();
    let list = list_area(area);
    let scroll = u16::try_from(page.top).unwrap_or(u16::MAX);
    // Clear the screen underneath first so the box does not overlap the calendar's text
    super::clear_wide_left_of(frame.buffer_mut(), area);
    frame.render_widget(Clear, area);
    frame.render_widget(block, area);
    frame.render_widget(Paragraph::new(lines).scroll((scroll, 0)), list);
    // The description stays near the bottom inside the box; below it, for a row that does nothing here,
    // which views it is for
    let bottom = inner(area).bottom();
    frame.render_widget(
        Line::styled(
            format!("  {}", texts.setting_description(chosen)),
            Style::new().fg(theme.muted),
        ),
        Rect {
            y: bottom.saturating_sub(2),
            height: 1,
            ..list
        },
    );
    if let Some(views) = chosen.views().filter(|_| !chosen.applies_to(app.view)) {
        frame.render_widget(
            Line::styled(
                format!("  {}", texts.setting_elsewhere(views)),
                Style::new().fg(theme.notice),
            ),
            Rect {
                y: bottom.saturating_sub(1),
                height: 1,
                ..list
            },
        );
    }
    // The whole chosen row gets a background color (as wide as the inside of the box;
    // the swatch keeps its own background)
    if let Some(y) = row_y(list, page.top, page.row) {
        for x in list.left()..list.right() {
            let cell = &mut frame.buffer_mut()[(x, y)];
            if cell.bg == ratatui::style::Color::Reset {
                cell.set_bg(theme.cursor_line);
            }
        }
    }
    // Does not fit: draw a scrollbar on the right border
    let max = u16::try_from(entries().len())
        .unwrap_or(u16::MAX)
        .saturating_sub(list.height);
    if max > 0 {
        super::render_scrollbar(frame, area, scroll.min(max), max, list.height, theme);
    }
}

/// Which screen row of the list area (`list`, drawn from line `top`) row `row` (an index into `rows()`)
/// is drawn on; `None` if scrolled out
fn row_y(list: Rect, top: usize, row: usize) -> Option<u16> {
    let line = line_of(row)?.checked_sub(top)?;
    let y = list.y.checked_add(u16::try_from(line).ok()?)?;
    (y < list.bottom()).then_some(y)
}

/// Which settings row (an index into `rows()`) a mouse click at (`column`, `row`) hits; `None` on a
/// section name, blank line, the description or the border
pub fn row_at(app: &App, screen: Rect, column: u16, row: u16) -> Option<usize> {
    let page = app.settings_page()?;
    let list = list_area(area(app, screen));
    if column < list.x || column >= list.right() {
        return None;
    }
    (0..rows().len()).find(|&index| row_y(list, page.top, index) == Some(row))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::{Mode, Row};
    use crate::texts::Lang;
    use crate::theme::Theme;
    use jiff::civil::date;
    use ratatui::{
        Terminal,
        backend::TestBackend,
        buffer::{Buffer, Cell},
        style::Color,
    };

    // Where things are, counted from the box's top-left corner: list line L is on row 2 + L (border,
    // blank row of padding); names of table rows start at cell 5 (border + indent 4), the top-level theme
    // and the section names at cell 3 (border + indent 2), values at cell 23 (border + indent 2 + name
    // column 20); the last rows of the box are the description, the line below it, the padding and the
    // bottom border
    const NAME: u16 = 5;
    const TOP_NAME: u16 = 3;
    const VALUE: u16 = 23;

    /// The row (from the box's top) of the settings row `row`
    fn y_of(row: Row) -> u16 {
        let index = rows().iter().position(|&each| each == row).expect("row");
        2 + u16::try_from(line_of(index).expect("line")).expect("fits")
    }

    /// The index of `row` in `rows()`
    fn index_of(row: Row) -> usize {
        rows().iter().position(|&each| each == row).expect("row")
    }

    /// The cell `dx` right and `dy` down from the box's top-left corner
    fn cell(buffer: &Buffer, area: Rect, dx: u16, dy: u16) -> &Cell {
        &buffer[(area.x + dx, area.y + dy)]
    }

    /// Opens the settings page (`page`) and draws a 70 × `height` screen once; returns the app (with
    /// `settings_page.top` already scrolled), the box's area,
    /// the text of each row in the box (the scrollbar `┃` on the right border turned back into `│`),
    /// and the whole screen
    fn draw(page: SettingsPage, height: u16) -> (App, Rect, Vec<String>, Buffer) {
        draw_app(
            App::new(Lang::ZhTw, date(2026, 10, 17), Vec::new()),
            page,
            height,
        )
    }

    /// Same as `draw`, but with the given `app` (e.g. with the theme already changed)
    fn draw_app(mut app: App, page: SettingsPage, height: u16) -> (App, Rect, Vec<String>, Buffer) {
        app.screen = Rect::new(0, 0, 70, height);
        app.mode = Mode::Settings {
            page,
            opened: app.settings.clone(),
        };
        let mut terminal = Terminal::new(TestBackend::new(70, height)).unwrap();
        terminal
            .draw(|frame| crate::ui::render(frame, &mut app))
            .unwrap();
        let buffer = terminal.backend().buffer().clone();
        let area = area(&app, app.screen);
        let rows = crate::ui::tests::area_rows(&buffer, area)
            .iter()
            .map(|row| row.replace('┃', "│"))
            .collect();
        (app, area, rows, buffer)
    }

    /// The row a page is opened on, choosing `row`
    fn choosing(row: usize) -> SettingsPage {
        SettingsPage {
            row,
            ..SettingsPage::default()
        }
    }

    #[test]
    fn settings_list_theme_switches_and_colors_like_the_config_file() {
        // Expected box computed by a separate Python script (each role's palette name taken from
        // [roles] in themes/catppuccin-mocha.toml):
        // name column = longest name (week_number_dim, 15 cells) + 3; the top-level theme has 2 less
        // indent and a name column 2 wider, so all values share one column;
        // the width is the larger of the longest description and the key hints on the bottom border
        // (42 cells + a blank on each side)
        let (_, area, rows, buffer) = draw(SettingsPage::default(), 44);
        assert_eq!(area, Rect::new(12, 2, 46, 41));
        assert_eq!(
            rows,
            [
                "╭ roh-cal ∙ 設定 ────────────────────────────╮",
                "│                                            │",
                "│  theme               catppuccin-mocha      │",
                "│                                            │",
                "│  [calendar]                                │",
                "│    relative_weeks    off                   │",
                "│                                            │",
                "│  [month]                                   │",
                "│    grid_lines        on                    │",
                "│    center_cursor     off                   │",
                "│                                            │",
                "│  [calendar_list]                           │",
                "│    max_width         36                    │",
                "│                                            │",
                "│  [colors]                                  │",
                "│    text                 text               │",
                "│    muted                subtext0           │",
                "│    line                 surface2           │",
                "│    year_line            lavender           │",
                "│    label                lavender           │",
                "│    week_number          yellow             │",
                "│    week_number_dim      dim_yellow         │",
                "│    weekend              blue               │",
                "│    has_item             peach              │",
                "│    today                green              │",
                "│    selected             red                │",
                "│    cursor_line          surface0           │",
                "│    focus                mauve              │",
                "│    checked              mauve              │",
                "│    notice               yellow             │",
                "│    error                red                │",
                "│    grid                 surface0           │",
                "│    other_month          overlay0           │",
                "│    range                surface1           │",
                "│    help_section         blue               │",
                "│    help_key             peach              │",
                "│                                            │",
                "│  顏色：l 換下一套 theme，h 換上一套        │",
                "│                                            │",
                "│                                            │",
                "╰ jk 選  hl 改  r 還原  R 全部還原  Esc 關閉 ╯",
            ]
        );
        // The chosen row (the first row, theme) has a background from the left to the right inside edge
        // and a bold value; other rows have no background
        let theme = Theme::catppuccin_mocha();
        let at = |dx, dy| cell(&buffer, area, dx, dy);
        let right = area.width - 2;
        assert_eq!(at(1, y_of(Row::Theme)).bg, theme.cursor_line);
        assert_eq!(at(right, y_of(Row::Theme)).bg, theme.cursor_line);
        assert!(
            at(VALUE, y_of(Row::Theme))
                .modifier
                .contains(Modifier::BOLD)
        );
        assert_eq!(at(VALUE, y_of(Row::RelativeWeeks)).symbol(), "o");
        assert_eq!(at(1, y_of(Row::RelativeWeeks)).bg, Color::Reset);
        assert_eq!(at(TOP_NAME, y_of(Row::Theme)).fg, theme.help_key);
        assert_eq!(
            at(TOP_NAME, y_of(Row::RelativeWeeks) - 1).fg,
            theme.help_section
        );
        // The width row's value is in the same column
        assert_eq!(at(VALUE, y_of(Row::ListWidth)).symbol(), "3");
        // Swatch: on the today row, the 2 cells of the value column have today's color, followed by the
        // name
        let today = y_of(Row::Color("today"));
        assert_eq!(at(VALUE, today).bg, theme.today);
        assert_eq!(at(VALUE + 1, today).bg, theme.today);
        assert_eq!(at(VALUE + 2, today).bg, Color::Reset);
    }

    #[test]
    fn the_chosen_color_row_keeps_its_swatch() {
        // Choosing the text row: the whole row has a background, but the swatch keeps the text color
        let text = Row::Color("text");
        let (_, area, rows, buffer) = draw(choosing(index_of(text)), 44);
        let theme = Theme::catppuccin_mocha();
        let at = |dx, dy| cell(&buffer, area, dx, dy);
        assert_eq!(at(2, y_of(text)).bg, theme.cursor_line);
        assert_eq!(at(VALUE, y_of(text)).bg, theme.text);
        assert_eq!(at(VALUE + 3, y_of(text)).bg, theme.cursor_line);
        assert_eq!(
            rows[rows.len() - 4],
            "│  顏色：l 換色票的下一個，r 回到 theme 的   │"
        );
    }

    #[test]
    fn a_reset_color_gets_no_swatch() {
        // The terminal theme's text is the terminal's default text color (Reset): the 2 swatch cells
        // look empty, followed by Reset
        let mut app = App::new(Lang::ZhTw, date(2026, 10, 17), Vec::new());
        app.settings.set_theme("terminal");
        app.apply_theme();
        let (_, area, rows, buffer) = draw_app(app, SettingsPage::default(), 44);
        let at = |dx, dy| cell(&buffer, area, dx, dy);
        let text = y_of(Row::Color("text"));
        assert_eq!(
            rows[usize::from(text)],
            "│    text                 Reset              │"
        );
        assert_eq!(at(VALUE, text).bg, Color::Reset);
        assert_eq!(at(VALUE + 1, text).bg, Color::Reset);
        // Other roles still have a swatch (muted = dark-gray)
        assert_eq!(at(VALUE, y_of(Row::Color("muted"))).bg, Color::DarkGray);
    }

    #[test]
    fn a_short_screen_scrolls_to_the_chosen_row() {
        // 70 × 23: the box is at most 23 tall and the list holds 16 lines (23 − border 2 − padding 2 −
        // the 3 lines below the list), so it scrolls; choosing the last row (help_key) → scrolled to the
        // end (its 16 last lines, all color rows), and the description is still at the bottom
        let last = rows().len() - 1;
        let (app, area, rows, buffer) = draw(choosing(last), 23);
        assert_eq!(
            app.settings_page().map(|page| page.top),
            Some(entries().len() - 16)
        );
        assert_eq!(area, Rect::new(12, 0, 46, 23));
        assert_eq!(
            rows,
            [
                "╭ roh-cal ∙ 設定 ────────────────────────────╮",
                "│                                            │",
                "│    week_number          yellow             │",
                "│    week_number_dim      dim_yellow         │",
                "│    weekend              blue               │",
                "│    has_item             peach              │",
                "│    today                green              │",
                "│    selected             red                │",
                "│    cursor_line          surface0           │",
                "│    focus                mauve              │",
                "│    checked              mauve              │",
                "│    notice               yellow             │",
                "│    error                red                │",
                "│    grid                 surface0           │",
                "│    other_month          overlay0           │",
                "│    range                surface1           │",
                "│    help_section         blue               │",
                "│    help_key             peach              │",
                "│                                            │",
                "│  顏色：l 換色票的下一個，r 回到 theme 的   │",
                "│                                            │",
                "│                                            │",
                "╰ jk 選  hl 改  r 還原  R 全部還原  Esc 關閉 ╯",
            ]
        );
        // The scrollbar is on the bottom part of the right border
        assert_eq!(cell(&buffer, area, 45, 21).symbol(), "┃");
        assert_eq!(cell(&buffer, area, 45, 1).symbol(), "│");
        // Clicking the bottom list row: still help_key; the top one is the 16th row from the end: mapped
        // using the scroll position
        assert_eq!(row_at(&app, app.screen, 20, 17), Some(last));
        assert_eq!(row_at(&app, app.screen, 20, 2), Some(last + 1 - 16));
    }

    #[test]
    fn r_asks_on_the_bottom_border() {
        let (_, _, rows, _) = draw(
            SettingsPage {
                row: index_of(Row::CenterCursor),
                confirm_reset: true,
                ..SettingsPage::default()
            },
            44,
        );
        assert_eq!(
            rows[rows.len() - 4],
            "│  大月曆：選取的那週固定在正中間            │"
        );
        assert_eq!(
            rows[rows.len() - 1],
            "╰ 全部還原成預設？y 還原，其他鍵取消 ────────╯"
        );
    }

    #[test]
    fn clicks_map_to_the_rows() {
        let (app, area, _, _) = draw(SettingsPage::default(), 44);
        let click = |dx: u16, dy: u16| row_at(&app, app.screen, area.x + dx, area.y + dy);
        // Any cell of a row inside the box, the left and right ends too
        let last = Row::Color("help_key");
        assert_eq!(click(8, y_of(Row::Theme)), Some(0));
        assert_eq!(click(8, y_of(Row::RelativeWeeks)), Some(1));
        assert_eq!(click(1, y_of(Row::GridLines)), Some(2));
        assert_eq!(
            click(8, y_of(Row::ListWidth)),
            Some(index_of(Row::ListWidth))
        );
        assert_eq!(click(area.width - 2, y_of(last)), Some(index_of(last)));
        // Section names, blank lines, the description, the line below it and the border do not count
        let height = area.height;
        for (dx, dy) in [
            (8, 1),
            (8, y_of(Row::Theme) + 1),
            (8, y_of(Row::RelativeWeeks) - 1),
            (8, y_of(Row::ListWidth) - 1),
            (8, y_of(Row::Color("text")) - 1),
            (8, height - 4),
            (8, height - 3),
            (0, y_of(Row::Theme)),
            (area.width - 1, y_of(Row::Theme)),
            (8, 0),
        ] {
            assert_eq!(click(dx, dy), None, "({dx}, {dy})");
        }
    }

    #[test]
    fn settings_that_do_nothing_here_are_dimmed_and_say_where_they_apply() {
        // Layout 1: the month grid's grid_lines and center_cursor are dimmed (the other-month gray,
        // darker than muted text), relative_weeks is not; choosing grid_lines says below the
        // description which view it is for
        let theme = Theme::catppuccin_mocha();
        let grid_lines = index_of(Row::GridLines);
        let (_, area, rows, buffer) = draw(choosing(grid_lines), 44);
        let at = |dx, dy| cell(&buffer, area, dx, dy);
        assert_eq!(at(NAME, y_of(Row::GridLines)).fg, theme.other_month);
        assert_eq!(at(VALUE, y_of(Row::GridLines)).fg, theme.other_month);
        assert_eq!(at(NAME, y_of(Row::CenterCursor)).fg, theme.other_month);
        assert_eq!(at(NAME, y_of(Row::RelativeWeeks)).fg, theme.help_key);
        let below = rows.len() - 3;
        assert_eq!(
            rows[below - 1].trim_end_matches([' ', '│']),
            "│  大月曆：格子之間畫淡淡的格線"
        );
        assert_eq!(
            rows[below].trim_end_matches([' ', '│']),
            "│  這個畫面用不到：2 大月曆"
        );
        assert_eq!(at(TOP_NAME, area.height - 3).fg, theme.notice);
        // Layout 2: the same row is a normal one, nothing below the description
        let mut app = App::new(Lang::ZhTw, date(2026, 10, 17), Vec::new());
        app.view = crate::app::View::Month;
        let (_, area, rows, buffer) = draw_app(app, choosing(grid_lines), 44);
        assert_eq!(
            cell(&buffer, area, NAME, y_of(Row::GridLines)).fg,
            theme.help_key
        );
        assert_eq!(rows[rows.len() - 3].trim_end_matches([' ', '│']), "");
        // Tasks page: relative_weeks is for layouts 1 and 2; theme is for every view
        let mut app = App::new(Lang::ZhTw, date(2026, 10, 17), Vec::new());
        app.view = crate::app::View::Tasks;
        let (_, area, rows, buffer) = draw_app(app, choosing(index_of(Row::RelativeWeeks)), 44);
        let at = |dx, dy| cell(&buffer, area, dx, dy);
        assert_eq!(at(NAME, y_of(Row::RelativeWeeks)).fg, theme.other_month);
        assert_eq!(at(TOP_NAME, y_of(Row::Theme)).fg, theme.help_key);
        assert_eq!(
            rows[rows.len() - 3].trim_end_matches([' ', '│']),
            "│  這個畫面用不到：1 月曆 + 事項、2 大月曆"
        );
    }
}
