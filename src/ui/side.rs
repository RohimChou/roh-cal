//! Side menu: the calendar list on the far left (calendar = a group in the task file). It is drawn on the
//! far left when the terminal is wide enough; otherwise `s` brings it up over the left side

use super::{Click, check_box, months::scrolled_top, planner::inner_areas, render_scrollbar};
use crate::{
    app::{App, Focus, Group, SideRow},
    config::CalendarListSettings,
    texts::Texts,
};
use ratatui::{
    Frame,
    layout::{Constraint, Layout, Position, Rect},
    style::Style,
    symbols::line,
    text::{Line, Span},
    widgets::{Block, BorderType},
};
use std::ops::Range;

/// Side menu width (incl. left and right border): the config file's `[calendar_list] width` when it is
/// set; otherwise (auto) just enough for the longest calendar, e.g. 「 ☑ Payments API v2 12 」,
/// but at least `CalendarListSettings::MIN_WIDTH` (22) and at most `AUTO_MAX_WIDTH` (38); longer names
/// get cut off
pub fn width(app: &App) -> u16 {
    if let Some(width) = app.settings.calendar_list.width() {
        return width;
    }
    let max = CalendarListSettings::AUTO_MAX_WIDTH;
    let texts = app.lang.texts();
    let longest = app
        .groups
        .iter()
        // The indent, 「 ☑ 」 3 cells, name, count
        .map(|group| {
            indent(app, group).len()
                + 3
                + Span::raw(name(group, texts)).width()
                + count_text(app, group).len()
        })
        // The switch rows above the calendars: 「 ☐ 」 3 cells + its name + one blank
        .chain([texts.hide_all, texts.hide_done].map(|name| 3 + Span::raw(name).width() + 1))
        .max()
        .unwrap_or(0);
    // Add the left and right border
    u16::try_from(longest + 2)
        .unwrap_or(max)
        .clamp(CalendarListSettings::MIN_WIDTH, max)
}

/// The most rows the running calendars take when the archive part is below them; more of them scroll
pub const RUNNING_MAX_ROWS: u16 = 30;

/// A calendar is indented 2 cells per level in its part of the list (see `App::side_level`)
fn indent(app: &App, group: &Group) -> String {
    "  ".repeat(app.side_level(group))
}

/// The calendar's name; items at the top of the file, before any heading, are 「未分組」
fn name<'a>(group: &'a Group, texts: &'static Texts) -> &'a str {
    group.name.as_deref().unwrap_or(texts.ungrouped)
}

/// The number to the right of a calendar (one blank on each side), e.g. 「 12 」: how many of that
/// calendar's items have a date (counted even when unchecked)
fn count_text(app: &App, group: &Group) -> String {
    let count = app
        .items
        .iter()
        .filter(|item| item.date.is_some() && group.includes(item))
        .count();
    format!(" {count} ")
}

/// Where the side menu's parts go
struct Areas {
    /// The header 「日曆」
    header: Rect,
    /// The rule under the header
    rule: Rect,
    /// (When the box has room) the 「隱藏全部」 and 「隱藏做完的」 rows and a rule under them
    switches: Option<[Rect; 3]>,
    /// The running calendars' rows (every calendar's when there is no archive part)
    running: Rect,
    /// The archive part: the rule 「─ 已封存 ───」 and the archived calendars' rows under it; `None` without
    /// archived calendars or in a box too short for it (then they follow the others in `running`)
    archive: Option<[Rect; 2]>,
}

/// The side menu's parts: the header and its rule, (when the box has room) the two switches and a rule, then
/// the running calendars, at most `RUNNING_MAX_ROWS` when archived calendars follow under their own rule
fn areas(area: Rect, app: &App) -> Areas {
    let [header, rule, body] = inner_areas(area);
    // The two switches and their rule take 3 rows; the list keeps at least 1
    let (switches, list) = if body.height < 4 {
        (None, body)
    } else {
        let row = |y: u16| Rect {
            y,
            height: 1,
            ..body
        };
        let list = Rect {
            y: body.y + 3,
            height: body.height - 3,
            ..body
        };
        (Some([row(body.y), row(body.y + 1), row(body.y + 2)]), list)
    };
    let running = running_count(app);
    // The archive part needs its rule and a row, and leaves the running calendars a row
    if running == app.groups.len() || list.height < 3 {
        return Areas {
            header,
            rule,
            switches,
            running: list,
            archive: None,
        };
    }
    let height = u16::try_from(running)
        .unwrap_or(u16::MAX)
        .min(RUNNING_MAX_ROWS)
        .min(list.height - 2);
    let archive_rule = Rect {
        y: list.y + height,
        height: 1,
        ..list
    };
    let archived = Rect {
        y: archive_rule.y + 1,
        height: list.bottom() - archive_rule.y - 1,
        ..list
    };
    Areas {
        header,
        rule,
        switches,
        running: Rect { height, ..list },
        archive: Some([archive_rule, archived]),
    }
}

/// How many calendars are running (not archived): they come first in `App::groups`
fn running_count(app: &App) -> usize {
    app.groups.iter().filter(|group| !group.archived).count()
}

/// One part of the calendar list: where its rows go, which calendars it lists (indexes in `App::groups`) and
/// which of them is at its top (counted from its first one)
struct Part {
    rows: Rect,
    calendars: Range<usize>,
    top: usize,
}

/// The parts of the calendar list as drawn last: the running calendars (`App::side_top`), then the archive
/// part when there is one (`App::side_archive_top`)
fn parts(areas: &Areas, app: &App) -> Vec<Part> {
    let Some([_, archived]) = areas.archive else {
        return vec![Part {
            rows: areas.running,
            calendars: 0..app.groups.len(),
            top: app.side_top,
        }];
    };
    let running = running_count(app);
    vec![
        Part {
            rows: areas.running,
            calendars: 0..running,
            top: app.side_top,
        },
        Part {
            rows: archived,
            calendars: running..app.groups.len(),
            top: app.side_archive_top,
        },
    ]
}

/// The part's top after scrolling: when the chosen calendar is in this part, scroll only once it leaves the
/// rows (clicking a visible calendar does not scroll it away, so a second click hits the same one); on a
/// switch row the first part shows its first calendar; never scrolled past the last calendar
fn scrolled(part: &Part, chosen: SideRow, first: bool) -> usize {
    let height = i64::from(part.rows.height);
    let target = match chosen {
        SideRow::Calendar(index) if part.calendars.contains(&index) => {
            Some(index - part.calendars.start)
        }
        SideRow::HideAll | SideRow::HideDone if first => Some(0),
        _ => None,
    };
    let top = i64::try_from(part.top).unwrap_or(0);
    let top = match target {
        Some(target) => scrolled_top(top, i64::try_from(target).unwrap_or(0), height, 0),
        None => top,
    };
    // Do not overscroll: the last calendar just touches the bottom
    let max = i64::try_from(part.calendars.len()).unwrap_or(0) - height;
    usize::try_from(top.min(max).max(0)).unwrap_or(0)
}

/// Draws the side menu: the header 「日曆」, the 「 ☐ 隱藏全部」 and 「 ☐ 隱藏做完的」 switches
/// (`[calendar_list] hide_done`) and a rule, then one calendar per row, e.g. 「 ☑ Work            12 」; a
/// calendar inside another is indented 2 cells per level, e.g. 「   ☑ SFAP  4 」. Archived calendars come
/// last, under a rule 「─ 已封存 ───」 of their own; the running calendars above it take at most
/// `RUNNING_MAX_ROWS`
///
/// Checked is `☑`, unchecked is `☐` (its items are hidden); a name that is too long for the box (see
/// `width`) gets cut off. When a part has too many calendars to fit, first scroll it so the chosen one is
/// visible (stored in `app.side_top` / `app.side_archive_top`) and draw a scrollbar on the right border next
/// to it. When the keyboard is on the side menu, the border lights up and the chosen calendar gets a
/// background color
pub fn render(frame: &mut Frame, area: Rect, app: &mut App) {
    let areas = areas(area, app);
    let chosen = app.chosen_side_row();
    let scrolled: Vec<usize> = parts(&areas, app)
        .iter()
        .enumerate()
        .map(|(index, part)| scrolled(part, chosen, index == 0))
        .collect();
    app.side_top = scrolled[0];
    if let Some(&top) = scrolled.get(1) {
        app.side_archive_top = top;
    }
    // Read-only from here on
    let app = &*app;
    let theme = &app.theme;
    let texts = app.lang.texts();
    let focused = app.focus == Focus::Side;
    let block = Block::bordered()
        .border_type(BorderType::Rounded)
        .border_style(Style::new().fg(if focused { theme.focus } else { theme.line }));
    frame.render_widget(block, area);
    // The header matches the other two boxes: a line of text + a rule
    frame.render_widget(
        Line::styled(texts.header_side, Style::new().fg(theme.text)),
        areas.header,
    );
    let rule = |width: u16| {
        Line::styled(
            line::HORIZONTAL.repeat(usize::from(width)),
            Style::new().fg(theme.line),
        )
    };
    frame.render_widget(rule(areas.rule.width), areas.rule);
    for part in parts(&areas, app) {
        render_part(frame, area, app, &part);
    }
    // The archive part's rule: 「─ 已封存 ───」, the name like a title
    if let Some([archive_rule, _]) = areas.archive {
        let label = format!(" {} ", texts.archived);
        let rest = usize::from(archive_rule.width).saturating_sub(1 + Span::raw(&label).width());
        frame.render_widget(
            Line::from(vec![
                Span::styled(line::HORIZONTAL, Style::new().fg(theme.line)),
                Span::styled(label, Style::new().fg(theme.label)),
                Span::styled(line::HORIZONTAL.repeat(rest), Style::new().fg(theme.line)),
            ]),
            archive_rule,
        );
    }
    // The two switches above a rule: ☑ in the `checked` color when on, ☐ muted when off. 「隱藏全部」 is on
    // while every calendar is unchecked
    if let Some([hide_all, hide_done, switches_rule]) = areas.switches {
        let rows = [
            (hide_all, SideRow::HideAll, app.all_hidden(), texts.hide_all),
            (
                hide_done,
                SideRow::HideDone,
                app.settings.calendar_list.hide_done,
                texts.hide_done,
            ),
        ];
        for (row, switch, on, name) in rows {
            let mark_color = if on { theme.checked } else { theme.muted };
            frame.render_widget(
                Line::from(vec![
                    Span::styled(format!(" {}", check_box(on)), Style::new().fg(mark_color)),
                    Span::styled(name, Style::new().fg(theme.text)),
                ]),
                row,
            );
            if focused && app.chosen_side_row() == switch {
                frame
                    .buffer_mut()
                    .set_style(row, Style::new().bg(theme.selection));
            }
        }
        frame.render_widget(rule(switches_rule.width), switches_rule);
    }
}

/// Draws one part of the calendar list (`area` = the whole side menu, for the scrollbar on its border)
fn render_part(frame: &mut Frame, area: Rect, app: &App, part: &Part) {
    let theme = &app.theme;
    let texts = app.lang.texts();
    let focused = app.focus == Focus::Side;
    let visible = part.calendars.clone().skip(part.top);
    for (index, y) in visible.zip(part.rows.y..part.rows.bottom()) {
        let group = &app.groups[index];
        let row = Rect::new(part.rows.x, y, part.rows.width, 1);
        let count = count_text(app, group);
        // Name on the left, count on the right; a long name is cut off and never covers the count
        // (digits and spaces are 1 cell wide)
        let [name_area, count_area] =
            Layout::horizontal([Constraint::Fill(1), Constraint::Length(count.len() as u16)])
                .areas(row);
        // Checked: ☑ in the `checked` color and the name in normal text color; unchecked: ☐ and the name
        // are both muted. Only its own check counts, not the calendars around it
        let (mark_color, name_color) = if group.shown {
            (theme.checked, theme.text)
        } else {
            (theme.muted, theme.muted)
        };
        frame.render_widget(
            Line::from(vec![
                Span::raw(indent(app, group)),
                Span::styled(
                    format!(" {}", check_box(group.shown)),
                    Style::new().fg(mark_color),
                ),
                Span::styled(name(group, texts), Style::new().fg(name_color)),
            ]),
            name_area,
        );
        frame.render_widget(
            Line::styled(count, Style::new().fg(theme.muted)),
            count_area,
        );
        if focused && app.chosen_side_row() == SideRow::Calendar(index) {
            // Only change the background; leave the text color alone
            frame
                .buffer_mut()
                .set_style(row, Style::new().bg(theme.selection));
        }
    }
    // Does not fit: draw a scrollbar on the right border next to the part's rows (not when it has no rows)
    let max = part
        .calendars
        .len()
        .saturating_sub(usize::from(part.rows.height));
    if let Ok(max) = u16::try_from(max)
        && max > 0
        && part.rows.height > 0
    {
        let rows = Rect {
            y: part.rows.y - 1,
            height: part.rows.height + 2,
            ..area
        };
        let top = u16::try_from(part.top).unwrap_or(u16::MAX);
        render_scrollbar(frame, rows, top, max, part.rows.height, theme);
    }
}

/// Which calendar a mouse click at (`column`, `row`) on the side menu (`area`) hits, or which switch row;
/// `None` on the header, the border, the rules, or the blank space below the calendars
pub fn click_at(area: Rect, app: &App, column: u16, row: u16) -> Option<Click> {
    let areas = areas(area, app);
    let position = Position::new(column, row);
    if let Some([hide_all, hide_done, _]) = areas.switches {
        if hide_all.contains(position) {
            return Some(Click::HideAll);
        }
        if hide_done.contains(position) {
            return Some(Click::HideDone);
        }
    }
    // One calendar per row, starting at the part's first row (if scrolled, that is its calendar `top`)
    let part = parts(&areas, app)
        .into_iter()
        .find(|part| part.rows.contains(position))?;
    let index = part.calendars.start + part.top + usize::from(row - part.rows.y);
    (index < part.calendars.end).then_some(Click::Group(index))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::texts::Lang;
    use jiff::civil::date;

    /// An app whose task file is `text`
    fn app_with(text: &str) -> App {
        App::new(
            Lang::ZhTw,
            date(2026, 10, 17),
            crate::tasks::parse(text).items,
        )
    }

    #[test]
    fn auto_width_fits_the_longest_calendar_up_to_38() {
        // No task file, or a short name (「 ☑ Personal 1 」 needs only 14 cells): the minimum, 22
        assert_eq!(width(&app_with("")), 22);
        assert_eq!(width(&app_with("# Personal\n- a @2026-10-10\n")), 22);
        // 「 ☑ Tokyo trip 🗼🍣 1 」: each emoji is two cells wide, so 3 + 15 + 3 = 21 cells, 23 with the
        // border; the longest one sets the width
        let app = app_with("# Personal\n- a @2026-10-10\n# Tokyo trip 🗼🍣\n- b @2026-12-24\n");
        assert_eq!(width(&app), 23);
        // A two-digit count: 1 more cell
        let ten = "- x @2026-10-10\n".repeat(10);
        assert_eq!(width(&app_with(&format!("# Tokyo trip 🗼🍣\n{ten}"))), 24);
        // Longer still (「 ☑ Year-end chores 🎄 and plans for 2027 0 」 needs 43 + 2 = 45 cells): 38, cut
        let app = app_with("# Year-end chores 🎄 and plans for 2027\n- a\n");
        assert_eq!(width(&app), 38);
    }

    #[test]
    fn a_fixed_width_ignores_the_names() {
        // The config file's `[calendar_list] width`, wider or narrower than the names need
        let mut app = app_with("# Year-end chores 🎄 and plans for 2027\n- a\n");
        app.settings.calendar_list.width = Some(30);
        assert_eq!(width(&app), 30);
        app.settings.calendar_list.width = Some(50);
        assert_eq!(width(&app), 50);
        let mut app = app_with("# Personal\n- a @2026-10-10\n");
        app.settings.calendar_list.width = Some(40);
        assert_eq!(width(&app), 40);
        // Out of range in the config file: below 22 counts as 22, above 60 as 60
        app.settings.calendar_list.width = Some(10);
        assert_eq!(width(&app), 22);
        app.settings.calendar_list.width = Some(100);
        assert_eq!(width(&app), 60);
    }
}
