//! Draws the whole screen: the side menu on the far left (only when wide enough; when narrow, `s` shows it
//! over the left side), the main view on the right (layout 1's calendar box + agenda, layout 2's month grid,
//! or the tasks page), the key hints on the bottom row; popups (layout 2's day popup, the edit box, the help
//! page, the settings page) go on top

mod day;
mod editor;
mod help;
mod month;
mod months;
mod picker;
mod planner;
mod settings;
mod side;
mod tasks;

use crate::app::{App, Editor, Focus, Mode, View};
use crate::tasks::Item;
use crate::theme::Theme;
use jiff::civil::Date;
use ratatui::{
    Frame,
    buffer::Buffer,
    layout::{Constraint, Layout, Margin, Position, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Clear, Scrollbar, ScrollbarOrientation, ScrollbarState},
};

/// The terminal must be at least this wide to draw the side menu
const SIDE_MIN_WIDTH: u16 = 120;

/// Column of `1` in the layout keys `123` at the left end of the bottom row (`2` and `3` follow)
const VIEW_KEYS_X: u16 = 1;

/// Which layout each of the keys `1` `2` `3` switches to
const VIEW_KEYS: [View; 3] = [View::Planner, View::Month, View::Tasks];

/// How many columns layout 1's agenda is split into on the current screen (1 when it is narrow)
pub fn agenda_columns(app: &App) -> usize {
    planner::columns(areas(app.screen, app).agenda)
}

/// Where the edit box is (so a mouse click outside it can cancel); nothing while it is closed
pub fn editor_area(app: &App) -> Rect {
    app.editor().map_or(Rect::default(), |editor| {
        editor::area(app, areas(app.screen, app).main, editor)
    })
}

/// Where the editor's `#` list is and which calendar is on its first row (so a click can pick one); `None`
/// when it is closed
pub fn hash_list_area(app: &App, editor: &Editor) -> Option<(Rect, usize)> {
    editor::list_area(app, app.screen, editor, app.lang.texts().hash_list_keys)
}

/// Where layout 2's day popup (that day's items) is on screen (so a mouse click outside it can close it)
pub fn day_area(app: &App) -> Rect {
    day::area(app, areas(app.screen, app).main)
}

/// Which item in the popup a mouse click at (`column`, `row`) hits (its index in `App::items_on(selected)`)
pub fn day_item_at(app: &App, column: u16, row: u16) -> Option<usize> {
    day::item_at(app, areas(app.screen, app).main, column, row)
}

/// Where the help page box is on screen (so a mouse click outside it can close it)
pub fn help_area(app: &App, full: bool) -> Rect {
    help::area(app, app.screen, full)
}

/// How many lines the help page can scroll down at most (based on the terminal's current size)
pub fn help_max_scroll(app: &App, full: bool) -> u16 {
    help::max_scroll(app, app.screen, full)
}

/// Where the settings page box is on screen (so a mouse click outside it can close it)
pub fn settings_area(app: &App) -> Rect {
    settings::area(app, app.screen)
}

/// Where the color picker's box is on screen (so a mouse click outside it can cancel); an empty area when
/// it is not open
pub fn picker_area(app: &App) -> Rect {
    app.picker().map_or_else(Rect::default, |picker| {
        picker::area(app, app.screen, picker)
    })
}

/// What a mouse click at (`column`, `row`) hits in the color picker: a channel, and the value of the bar
/// cell when it is on the bar
pub fn picker_at(app: &App, column: u16, row: u16) -> Option<(usize, Option<u8>)> {
    picker::at(app, app.screen, column, row)
}

/// Which settings row a mouse click at (`column`, `row`) hits (its index in `app::rows()`)
pub fn settings_row_at(app: &App, column: u16, row: u16) -> Option<usize> {
    settings::row_at(app, app.screen, column, row)
}

/// The short calendar name at the right end of an item's first line in layout 2's day popup: its outermost
/// calendar (`CAIS` for `CAIS > 2603_…`); `None` outside any calendar, or in a holiday calendar (those items
/// are blue already)
fn calendar_tag(item: &Item) -> Option<&str> {
    let group = item.group.as_ref().filter(|group| !group.holiday)?;
    Some(group.parents.first().unwrap_or(&group.name))
}

/// An item's whole calendar path, e.g. `CAIS > 2603_中信_CAIS_CAMS報表轉送CAIS系統`; `None` outside any calendar
fn calendar_path(item: &Item) -> Option<String> {
    let group = item.group.as_ref()?;
    let names: Vec<&str> = group
        .parents
        .iter()
        .chain([&group.name])
        .map(String::as_str)
        .collect();
    Some(names.join(" > "))
}

/// Put in front of an in-progress (`[/]`) item's title: 🚧 (an emoji: always 2 cells wide, its own colors)
/// and a space
const IN_PROGRESS_ICON: &str = "🚧 ";

/// An item's title in `color`, the same in every view: crossed out once done; while in progress (`[/]`)
/// bold, with 🚧 in front
fn title_span(item: &Item, color: Color) -> Span<'static> {
    let style = Style::new().fg(color);
    if item.done {
        Span::styled(
            item.title.clone(),
            style.add_modifier(Modifier::CROSSED_OUT),
        )
    } else if item.in_progress {
        Span::styled(
            format!("{IN_PROGRESS_ICON}{}", item.title),
            style.add_modifier(Modifier::BOLD),
        )
    } else {
        Span::styled(item.title.clone(), style)
    }
}

/// If the cell left of a popup (`area`) holds a CJK character (2 cells wide), its
/// second half covers the popup's left border (the terminal gets the character
/// but not the border): replace it with a space before drawing the popup
fn clear_wide_left_of(buffer: &mut Buffer, area: Rect) {
    let Some(x) = area.x.checked_sub(1) else {
        return;
    };
    for y in area.top()..area.bottom() {
        if let Some(cell) = buffer.cell_mut((x, y))
            && Span::raw(cell.symbol()).width() > 1
        {
            cell.set_symbol(" ");
        }
    }
}

/// Clears a popup's `area` and a ring of one blank cell around it (only the part inside `bounds`), so the
/// box does not run into the text around it
fn clear_with_ring(frame: &mut Frame, area: Rect, bounds: Rect) {
    let ring = area.outer(Margin::new(1, 1)).intersection(bounds);
    clear_wide_left_of(frame.buffer_mut(), ring);
    frame.render_widget(Clear, ring);
}

/// When a popup (`area`) cannot show everything, draw a scrollbar on its right border: the track is the
/// border itself (not drawn separately), and the thick part = where the visible part sits in the whole page.
/// `scroll` = lines scrolled down, `max` = the most lines it can scroll, `visible` = lines visible at once
fn render_scrollbar(
    frame: &mut Frame,
    area: Rect,
    scroll: u16,
    max: u16,
    visible: u16,
    theme: &Theme,
) {
    let scrollbar = Scrollbar::new(ScrollbarOrientation::VerticalRight)
        .track_symbol(None)
        .begin_symbol(None)
        .end_symbol(None)
        .thumb_symbol("┃")
        .thumb_style(Style::new().fg(theme.label));
    // There are max + 1 positions to stop at (0..=max), and visible lines show at once
    let mut state = ScrollbarState::new(usize::from(max) + 1)
        .position(usize::from(scroll))
        .viewport_content_length(usize::from(visible));
    // Leave out the rounded corners: the scrollbar goes only on the straight part of the right border
    frame.render_stateful_widget(scrollbar, area.inner(Margin::new(0, 1)), &mut state);
}

/// Whether the terminal (`screen`) is wide enough to draw the side menu
pub fn side_fits(screen: Rect) -> bool {
    screen.width >= SIDE_MIN_WIDTH
}

/// Narrow screen with the keyboard in the side menu (after `s`): where the side menu sits over the left of
/// the main view; `None` otherwise (so a mouse click outside it can hide it)
pub fn side_overlay(app: &App) -> Option<Rect> {
    side_overlay_in(app.screen, app)
}

/// Same as `side_overlay`, for a terminal the size of `screen`
fn side_overlay_in(screen: Rect, app: &App) -> Option<Rect> {
    if side_fits(screen) || app.focus != Focus::Side {
        return None;
    }
    let main = areas(screen, app).main;
    Some(Rect {
        width: side::width(app).min(main.width),
        ..main
    })
}

/// Where each part of the screen is
struct Areas {
    /// Far left: the side menu (the list of calendars); width 0 when it is not drawn
    side: Rect,
    /// The whole block right of the side menu and above the key
    /// hints: layout 2's month grid and the tasks page are drawn here
    main: Rect,
    /// Left side of layout 1: the calendar box
    calendar: Rect,
    /// Right side of layout 1: the agenda, on the same rows as the
    /// calendar box; its header shows the selected day on the right
    agenda: Rect,
    /// Bottom row: the key hints
    hints: Rect,
}

/// Splits the whole terminal (`screen`) into parts; drawing and
/// mouse hit-testing both use this, so the two always agree
///
/// The side menu is there only when the terminal is wide enough
/// and `s` has not hidden it; its width follows the calendar names
fn areas(screen: Rect, app: &App) -> Areas {
    let [main, hints] =
        Layout::vertical([Constraint::Fill(1), Constraint::Length(1)]).areas(screen);
    let side_width = if side_fits(screen) && !app.side_hidden {
        side::width(app)
    } else {
        0
    };
    let [side, main] =
        Layout::horizontal([Constraint::Length(side_width), Constraint::Fill(1)]).areas(main);
    let [calendar, agenda] = Layout::horizontal([
        Constraint::Length(planner::CALENDAR_WIDTH),
        Constraint::Fill(1),
    ])
    .areas(main);
    Areas {
        side,
        main,
        calendar,
        agenda,
        hints,
    }
}

/// What a mouse click hit
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Click {
    /// A day on the calendar
    Day(Date),
    /// An item in the agenda: its index in `App::agenda()`
    Item(usize),
    /// A calendar in the side menu: its index in `App::groups`
    Group(usize),
    /// The 「隱藏做完的」 row at the bottom of the side menu
    HideDone,
    /// A row of the tasks page: its index in `App::tasks_rows()`; `on_marker` = the click was
    /// on `▸` / `▾` (expand / collapse)
    TaskRow { row: usize, on_marker: bool },
    /// One of the layout keys `123` at the left end of the bottom row: switch to that layout
    View(View),
}

/// What a mouse click at terminal cell (`column`, `row`) hits;
/// `None` when it is not on a day, an item or a calendar
pub fn click_at(app: &App, column: u16, row: u16) -> Option<Click> {
    let areas = areas(app.screen, app);
    // The layout keys `123` on the bottom row (not there while the keyboard is in the day popup or the
    // side menu: the bottom row has their keys instead)
    if row == areas.hints.y && !matches!(app.focus, Focus::Day | Focus::Side) {
        return column
            .checked_sub(VIEW_KEYS_X)
            .and_then(|index| VIEW_KEYS.get(usize::from(index)))
            .map(|view| Click::View(*view));
    }
    // side menu: on the far left when wide enough; when narrow, called up over the left side
    let side = side_overlay(app).unwrap_or(areas.side);
    if side.contains(Position::new(column, row)) {
        return side::click_at(side, app, column, row);
    }
    match app.view {
        View::Planner => planner::click_at(areas.calendar, areas.agenda, app, column, row),
        View::Month => month::click_at(areas.main, app, column, row),
        View::Tasks => tasks::click_at(areas.main, app, column, row),
    }
}

/// Draws the whole screen: in the middle, the side menu on the far left (only when wide enough; when narrow,
/// `s` shows it over the left side), and on the right layout 1's calendar box + agenda, layout 2's month grid
/// or the tasks page; the key hints on the bottom row; open popups (layout 2's day popup, the edit box, the
/// help page, the settings page) are drawn last, on top
pub fn render(frame: &mut Frame, app: &mut App) {
    let texts = app.lang.texts();
    let theme = &app.theme;
    let Areas {
        side: side_area,
        main: main_area,
        calendar: calendar_area,
        agenda: agenda_area,
        hints,
    } = areas(frame.area(), app);
    // Bottom row: delete confirmation > rescheduling (which item is moving, how to confirm) > notice (write
    // failed, reloaded; the next key brings the hints back) > key hints
    if let Some((item, count)) = app.confirm_delete() {
        frame.render_widget(
            Line::styled(
                texts.confirm_delete(&item.title, count),
                Style::new().fg(theme.notice),
            ),
            hints,
        );
    } else if let Some((name, count)) = app.confirm_delete_calendar() {
        frame.render_widget(
            Line::styled(
                texts.confirm_delete_calendar(name, count),
                Style::new().fg(theme.notice),
            ),
            hints,
        );
    } else if let Some(moving) = app.moving() {
        frame.render_widget(
            Line::styled(
                texts.moving(&moving.item.title),
                Style::new().fg(theme.notice),
            ),
            hints,
        );
    } else if let Some(notice) = &app.notice {
        frame.render_widget(
            Line::styled(texts.notice(notice), Style::new().fg(theme.notice)),
            hints,
        );
    } else {
        // The tasks page has its own key hints; while layout 2's popup or the calendar list has the keyboard,
        // show its keys
        let muted = Style::new().fg(theme.muted);
        let line = match app.view {
            _ if app.focus == Focus::Day => Line::styled(texts.hints_day, muted),
            _ if app.focus == Focus::Side => Line::styled(texts.hints_side, muted),
            View::Tasks => view_keys_line(app, texts.hints_tasks),
            View::Planner | View::Month => view_keys_line(app, texts.hints),
        };
        frame.render_widget(line, hints);
    }
    if side_area.width > 0 {
        side::render(frame, side_area, app);
    }
    // The side menu and the main view change `app` (they remember the scroll position); by now the `theme`
    // borrowed above is no longer used. Popups go on top of them
    match app.view {
        View::Planner => planner::render(frame, calendar_area, agenda_area, app),
        View::Month => month::render(frame, main_area, app),
        View::Tasks => tasks::render(frame, main_area, app),
    }
    // The side menu called up on a narrow screen: drawn over the left of the main view
    if let Some(area) = side_overlay_in(frame.area(), app) {
        frame.render_widget(Clear, area);
        side::render(frame, area, app);
    }
    // Layout 2's day popup: over the month grid (the edit box, help page and so on go on top of it)
    if matches!(app.view, View::Planner | View::Month) && app.focus == Focus::Day {
        day::render(frame, main_area, app);
    }
    // The edit box, help page and settings page go on top of everything
    match &app.mode {
        Mode::Editor(editor) => editor::render(frame, editor, app),
        Mode::Help(help) => help::render(frame, app, *help),
        Mode::Settings { page, .. } => {
            // First scroll so the chosen row is visible (kept in the settings page's `top` and
            // `colors_top`), then draw
            let page = settings::scrolled(app, frame.area(), *page);
            if let Mode::Settings { page: stored, .. } = &mut app.mode {
                *stored = page;
            }
            settings::render(frame, app, page);
            if let Some(picker) = app.picker() {
                picker::render(frame, app, picker);
            }
        }
        // Delete confirmation and rescheduling: shown on the bottom row (already drawn above)
        Mode::ConfirmDelete(..)
        | Mode::ConfirmDeleteCalendar { .. }
        | Mode::Moving(_)
        | Mode::Normal => {}
    }
}

/// The bottom row that starts with the layout keys, ` 123 view  ` + `hints`: the key of the layout shown now
/// is lit up (like a tab), and clicking a key switches to that layout (`click_at`)
fn view_keys_line(app: &App, hints: &'static str) -> Line<'static> {
    let theme = &app.theme;
    let muted = Style::new().fg(theme.muted);
    let mut spans = vec![Span::styled(" ".repeat(usize::from(VIEW_KEYS_X)), muted)];
    for (key, view) in ('1'..).zip(VIEW_KEYS) {
        let style = if view == app.view {
            Style::new().fg(theme.focus).add_modifier(Modifier::BOLD)
        } else {
            muted
        };
        spans.push(Span::styled(key.to_string(), style));
    }
    spans.push(Span::styled(format!(" {} ", app.lang.texts().views), muted));
    spans.push(Span::styled(hints, muted));
    Line::from(spans)
}

#[cfg(test)]
mod tests;
