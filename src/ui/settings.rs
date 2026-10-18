//! Settings page (`,`): a box over the middle of the screen that lists the settings the way the config
//! file looks: top-level ones (`theme`) first, then one section per table (`[calendar]`, `[month]`,
//! `[colors]`); the bottom line explains what the chosen row does. Settings that do nothing on the current
//! view (e.g. the month grid's on layout 1) are dimmed, and choosing one says below which views it is for
//!
//! ```text
//! ╭ roh-cal ∙ 設定 ─────────────────────────╮
//! │  theme              nord                │  ← the chosen row gets a background color
//! │  [calendar]                             │  ← no blank lines: the section names stand out by color
//! │    relative_weeks   off                 │
//! │  ...                                    │
//! │  [colors]                               │
//! │    text             ██ text             ┃  ← swatch = the role's current color, then its palette name;
//! │  ...                                    │     at most `COLOR_ROWS` colors at once, the section scrolls
//! │                                         │     inside to the chosen one (scrollbar beside it); the
//! │                                         │     whole list scrolls too when the screen is short
//! │  顏色：l 換下一套 theme，h 換上一套     │  ← the description stays at the bottom and does not scroll
//! │                                         │  ← for a dimmed row: 「這個畫面用不到：2 大月曆」
//! ╰ jk 選  hl 改  r 還原  R 全部  Esc 關閉 ──╯
//! ```

use crate::app::{App, Row, SettingsPage, rows};
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
/// Between border and content: none (the section-name indent spaces the left side; no blank rows, so the
/// box stays short)
const PADDING: Padding = Padding::ZERO;
/// Gap between the content and the right border (same as the section-name indent on the left)
const RIGHT_MARGIN: usize = 2;
/// The swatch before the value on color rows: 2 blank cells with that color as background, then 1 blank
const SWATCH: &str = "  ";
/// Fixed lines below the list: a blank line, the description, and the line saying which views a dimmed
/// row is for (blank otherwise)
const FOOTER: u16 = 3;
/// When scrolling, the minimum lines between the chosen row and the top or bottom edge
const SCROLLOFF: i64 = 1;
/// The most colors `[colors]` shows at once; the rest scroll inside the section
pub const COLOR_ROWS: usize = 10;

/// One line in the list: a section name or a setting row (an index into `rows()`)
enum Entry {
    /// A section name like `[calendar]`
    Section(&'static str),
    /// A setting row: an index into `rows()`
    Row(usize),
}

/// Where the colors start in `rows()` (they are the last rows) and how many there are
fn colors() -> (usize, usize) {
    let rows = rows();
    let first = rows
        .iter()
        .position(|row| matches!(row, Row::Color(_)))
        .unwrap_or(rows.len());
    (first, rows.len() - first)
}

/// What each line of the list is, top to bottom: top-level rows first (no section name); each section
/// is its name + its rows. `[colors]` lists only `COLOR_ROWS` colors, from the `colors_top`-th
fn entries(colors_top: usize) -> Vec<Entry> {
    let rows = rows();
    let (first_color, _) = colors();
    let window = first_color + colors_top..first_color + colors_top + COLOR_ROWS;
    let mut entries = Vec::new();
    for (index, row) in rows.iter().enumerate() {
        let new_section = index == 0 || rows[index - 1].section() != row.section();
        if new_section && let Some(section) = row.section() {
            entries.push(Entry::Section(section));
        }
        if index < first_color || window.contains(&index) {
            entries.push(Entry::Row(index));
        }
    }
    entries
}

/// Which list line row `row` (an index into `rows()`) is on; `None` for a color scrolled out of
/// `[colors]`
fn line_of(row: usize, colors_top: usize) -> Option<usize> {
    entries(colors_top)
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
    let height = u16::try_from(entries(0).len())
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

/// Where to scroll, only when needed and never past the end: first `[colors]` (`page.colors_top`), so a
/// chosen color is among the colors shown, at least `SCROLLOFF` from the section's ends; then the whole
/// list (`page.top`), so the chosen row is on screen, at least `SCROLLOFF` lines from the box's edges
pub fn scrolled(app: &App, screen: Rect, page: SettingsPage) -> SettingsPage {
    let to_i64 = |n: usize| i64::try_from(n).unwrap_or(0);
    let to_usize = |n: i64| usize::try_from(n.max(0)).unwrap_or(0);
    let scroll = |top: usize, line: usize, height: i64, lines: usize| {
        let top = super::months::scrolled_top(to_i64(top), to_i64(line), height, SCROLLOFF);
        to_usize(top.min(to_i64(lines) - height))
    };
    let (first_color, count) = colors();
    let colors_top = match page.row.checked_sub(first_color) {
        Some(color) => scroll(page.colors_top, color, to_i64(COLOR_ROWS), count),
        None => page.colors_top.min(count.saturating_sub(COLOR_ROWS)),
    };
    let height = i64::from(list_area(area(app, screen)).height);
    let line = line_of(page.row, colors_top).unwrap_or(0);
    let top = scroll(page.top, line, height, entries(colors_top).len());
    SettingsPage {
        top,
        colors_top,
        ..page
    }
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
    let lines: Vec<Line> = entries(page.colors_top)
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
                let (name_style, mut value_style) = if applies {
                    (Style::new().fg(theme.help_key), Style::new().fg(theme.text))
                } else {
                    (theme.dimmed(), theme.dimmed())
                };
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
                    name_style,
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
    if let Some(y) = row_y(list, page, page.row) {
        for x in list.left()..list.right() {
            let cell = &mut frame.buffer_mut()[(x, y)];
            if cell.bg == ratatui::style::Color::Reset {
                cell.set_bg(theme.selection);
            }
        }
    }
    // Does not fit: draw a scrollbar on the right border
    let to_u16 = |n: usize| u16::try_from(n).unwrap_or(u16::MAX);
    let max = to_u16(entries(page.colors_top).len()).saturating_sub(list.height);
    if max > 0 {
        super::render_scrollbar(frame, area, scroll.min(max), max, list.height, theme);
    } else {
        // More colors than `[colors]` shows: a scrollbar on the right border beside them (the area
        // passed is one row bigger each way, standing in for the corners `render_scrollbar` leaves out)
        let (first_color, count) = colors();
        let first = row_y(list, page, first_color + page.colors_top);
        if let Some(first) = first.filter(|_| count > COLOR_ROWS) {
            let beside = Rect {
                y: first - 1,
                height: to_u16(COLOR_ROWS) + 2,
                ..area
            };
            let max = to_u16(count - COLOR_ROWS);
            let scroll = to_u16(page.colors_top);
            super::render_scrollbar(frame, beside, scroll, max, to_u16(COLOR_ROWS), theme);
        }
    }
}

/// Which screen row of the list area (`list`, drawn from line `page.top`, `[colors]` from its
/// `page.colors_top`-th color) row `row` (an index into `rows()`) is drawn on; `None` if scrolled out
fn row_y(list: Rect, page: SettingsPage, row: usize) -> Option<u16> {
    let line = line_of(row, page.colors_top)?.checked_sub(page.top)?;
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
    (0..rows().len()).find(|&index| row_y(list, page, index) == Some(row))
}

#[cfg(test)]
mod tests;
