//! Draws the whole screen: the side menu on the far left (only when wide enough; when narrow, `s` shows it
//! over the left side), the main view on the right (layout 1's calendar box + agenda, layout 3's month grid,
//! or the tasks page), the key hints on the bottom row; popups (layout 3's day popup, the edit box, the help
//! page, the settings page) go on top

mod day;
mod editor;
mod help;
mod month;
mod months;
mod planner;
mod settings;
mod side;
mod tasks;

use crate::app::{App, Focus, Mode, SettingsPage, View};
use crate::theme::Theme;
use jiff::civil::Date;
use ratatui::{
    Frame,
    buffer::Buffer,
    layout::{Constraint, Layout, Margin, Position, Rect},
    style::Style,
    text::{Line, Span},
    widgets::{Clear, Scrollbar, ScrollbarOrientation, ScrollbarState},
};

/// The terminal must be at least this wide to draw the side menu
const SIDE_MIN_WIDTH: u16 = 120;

/// Where the edit box is (so a mouse click outside it can cancel)
pub fn editor_area(screen: Rect) -> Rect {
    editor::area(screen)
}

/// Where layout 3's day popup (that day's items) is on screen (so a mouse click outside it can close it)
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

/// Which settings row a mouse click at (`column`, `row`) hits (its index in `app::rows()`)
pub fn settings_row_at(app: &App, column: u16, row: u16) -> Option<usize> {
    settings::row_at(app, app.screen, column, row)
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
    /// hints: layout 3's month grid and the tasks page are drawn here
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
    /// A row of the tasks page: its index in `App::tasks_rows()`; `on_marker` = the click was
    /// on `▸` / `▾` (expand / collapse)
    TaskRow { row: usize, on_marker: bool },
}

/// What a mouse click at terminal cell (`column`, `row`) hits;
/// `None` when it is not on a day, an item or a calendar
pub fn click_at(app: &App, column: u16, row: u16) -> Option<Click> {
    let areas = areas(app.screen, app);
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
/// `s` shows it over the left side), and on the right layout 1's calendar box + agenda, layout 3's month grid
/// or the tasks page; the key hints on the bottom row; open popups (layout 3's day popup, the edit box, the
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
        // The tasks page has its own key hints; while layout 3's popup or the calendar list has the keyboard,
        // show its keys
        let hint_text = match app.view {
            _ if app.focus == Focus::Day => texts.hints_day,
            _ if app.focus == Focus::Side => texts.hints_side,
            View::Tasks => texts.hints_tasks,
            View::Planner | View::Month => texts.hints,
        };
        frame.render_widget(Line::styled(hint_text, Style::new().fg(theme.muted)), hints);
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
    // Layout 3's day popup: over the month grid (the edit box, help page and so on go on top of it)
    if app.view == View::Month && app.focus == Focus::Day {
        day::render(frame, main_area, app);
    }
    // The edit box, help page and settings page go on top of everything
    match &app.mode {
        Mode::Editor(editor) => editor::render(frame, editor, app),
        Mode::Help(help) => help::render(frame, app, *help),
        Mode::Settings { page, .. } => {
            // First scroll so the chosen row is visible (kept in the settings page's `top`), then draw
            let page = SettingsPage {
                top: settings::scrolled(app, frame.area(), *page),
                ..*page
            };
            if let Mode::Settings { page: stored, .. } = &mut app.mode {
                *stored = page;
            }
            settings::render(frame, app, page);
        }
        // Delete confirmation and rescheduling: shown on the bottom row (already drawn above)
        Mode::ConfirmDelete(..)
        | Mode::ConfirmDeleteCalendar { .. }
        | Mode::Moving(_)
        | Mode::Normal => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::Focus;
    use crate::texts::Lang;
    use crate::theme::Theme;
    use jiff::{
        ToSpan,
        civil::{Date, date},
    };
    use ratatui::{
        Terminal,
        backend::TestBackend,
        buffer::Buffer,
        style::{Color, Modifier},
        text::Span,
    };

    /// Draws one `width` × `height` screen in `lang`: today is 2026/10/17, the selected day is `selected`
    fn draw(lang: Lang, width: u16, height: u16, selected: Date) -> Terminal<TestBackend> {
        let mut app = App::new(lang, date(2026, 10, 17), Vec::new());
        app.selected = selected;
        draw_app(&mut app, width, height)
    }

    /// Which day a mouse click at (`column`, `row`) hits: a date on the calendar gives that day, an item in
    /// the agenda gives that item's day
    fn date_at(app: &App, column: u16, row: u16) -> Option<Date> {
        match click_at(app, column, row)? {
            Click::Day(date) => Some(date),
            Click::Item(index) => Some(app.agenda()[index].date),
            Click::Group(_) | Click::TaskRow { .. } => None,
        }
    }

    /// Draws one `width` × `height` screen with the current `app`
    /// (the scroll position is kept in `app` for the next draw)
    fn draw_app(app: &mut App, width: u16, height: u16) -> Terminal<TestBackend> {
        let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
        terminal.draw(|frame| render(frame, app)).unwrap();
        terminal
    }

    /// The text of each screen row, trailing spaces removed (see `area_rows`)
    fn text_rows(buffer: &Buffer) -> Vec<String> {
        area_rows(buffer, buffer.area)
            .iter()
            .map(|row| row.trim_end().to_string())
            .collect()
    }

    /// The text of each row in `area`; a CJK character takes 2 cells, and the
    /// second one (a space in the buffer) is skipped so the joined text matches
    /// the original. The help page and settings page tests use this too
    pub(super) fn area_rows(buffer: &Buffer, area: Rect) -> Vec<String> {
        (area.top()..area.bottom())
            .map(|y| {
                let mut row = String::new();
                let mut x = area.left();
                while x < area.right() {
                    let symbol = buffer[(x, y)].symbol();
                    row.push_str(symbol);
                    x += u16::try_from(Span::raw(symbol).width().max(1)).unwrap();
                }
                row
            })
            .collect()
    }

    #[test]
    fn calendar_from_this_month_down_with_a_thick_line_between_years() {
        // Only January shows its year (its month name goes on the next row); the header's corner shows the
        // top week's year (2026年); the line between years is thick; today, which is selected, gets brackets
        let terminal = draw(Lang::ZhTw, 35, 26, date(2026, 10, 17));
        assert_eq!(
            text_rows(terminal.backend().buffer()),
            [
                "╭─────────────────────────────────╮",
                "│2026年 週 │ 一 二 三 四 五 六 日 │",
                "│──────────┼──────────────────────│",
                "│  10月 40 │           1  2  3  4 │",
                "│       41 │  5  6  7  8  9 10 11 │",
                "│       42 │ 12 13 14 15 16[17]18 │",
                "│       43 │ 19 20 21 22 23 24 25 │",
                "│       44 │ 26 27 28 29 30 31    │",
                "│──────────┼──────────────────────│",
                "│  11月 44 │                    1 │",
                "│       45 │  2  3  4  5  6  7  8 │",
                "│       46 │  9 10 11 12 13 14 15 │",
                "│       47 │ 16 17 18 19 20 21 22 │",
                "│       48 │ 23 24 25 26 27 28 29 │",
                "│       49 │ 30                   │",
                "│──────────┼──────────────────────│",
                "│  12月 49 │     1  2  3  4  5  6 │",
                "│       50 │  7  8  9 10 11 12 13 │",
                "│       51 │ 14 15 16 17 18 19 20 │",
                "│       52 │ 21 22 23 24 25 26 27 │",
                "│       53 │ 28 29 30 31          │",
                "│━━━━━━━━━━┿━━━━━━━━━━━━━━━━━━━━━━│",
                "│2027年  1 │              1  2  3 │",
                "│   1月  2 │  4  5  6  7  8  9 10 │",
                "╰─────────────────────────────────╯",
                " hjkl 移動  Tab 切換  [] 換月  t 今",
            ]
        );
    }

    #[test]
    fn calendar_in_english() {
        let terminal = draw(Lang::En, 35, 11, date(2026, 10, 17));
        assert_eq!(
            text_rows(terminal.backend().buffer()),
            [
                "╭─────────────────────────────────╮",
                "│  2026 Wk │ Mo Tu We Th Fr Sa Su │",
                "│──────────┼──────────────────────│",
                "│   Oct 40 │           1  2  3  4 │",
                "│       41 │  5  6  7  8  9 10 11 │",
                "│       42 │ 12 13 14 15 16[17]18 │",
                "│       43 │ 19 20 21 22 23 24 25 │",
                "│       44 │ 26 27 28 29 30 31    │",
                "│──────────┼──────────────────────│",
                "╰─────────────────────────────────╯",
                " hjkl move  Tab pane  [] month  t t",
            ]
        );
    }

    #[test]
    fn selection_brackets_and_the_month_on_top() {
        // Another month selected: that month goes on top; no January on
        // screen, so the header shows the year; brackets hug a one-digit date
        let terminal = draw(Lang::En, 35, 8, date(2026, 11, 2));
        assert_eq!(
            text_rows(terminal.backend().buffer()),
            [
                "╭─────────────────────────────────╮",
                "│  2026 Wk │ Mo Tu We Th Fr Sa Su │",
                "│──────────┼──────────────────────│",
                "│   Nov 44 │                    1 │",
                "│       45 │ [2] 3  4  5  6  7  8 │",
                "│       46 │  9 10 11 12 13 14 15 │",
                "╰─────────────────────────────────╯",
                " hjkl move  Tab pane  [] month  t t",
            ]
        );
        // Sunday: `]` goes in the extra cell at the end; the calendar has only 3 rows and must keep 1 row of
        // margin, so it scrolls up one more row (the thick year line)
        let terminal = draw(Lang::ZhTw, 35, 8, date(2027, 1, 3));
        assert_eq!(
            text_rows(terminal.backend().buffer()),
            [
                "╭─────────────────────────────────╮",
                "│2027年 週 │ 一 二 三 四 五 六 日 │",
                "│──────────┼──────────────────────│",
                "│━━━━━━━━━━┿━━━━━━━━━━━━━━━━━━━━━━│",
                "│2027年  1 │              1  2 [3]│",
                "│   1月  2 │  4  5  6  7  8  9 10 │",
                "╰─────────────────────────────────╯",
                " hjkl 移動  Tab 切換  [] 換月  t 今",
            ]
        );
    }

    #[test]
    fn calendar_scrolls_down_only_when_the_selection_gets_close_to_the_bottom() {
        // The calendar has 6 rows: 10/17 → 10/24 → 10/31 → 11/7 going down; the selected row keeps at least 2
        // rows from the bottom edge and scrolls only when it can't
        let mut app = App::new(Lang::En, date(2026, 10, 17), Vec::new());
        draw_app(&mut app, 35, 11);
        for day in [date(2026, 10, 24), date(2026, 10, 31), date(2026, 11, 7)] {
            app.selected = day;
            draw_app(&mut app, 35, 11);
        }
        // October's first 4 rows have scrolled off the top;
        // no January on screen, so the header shows the year
        assert_eq!(
            text_rows(draw_app(&mut app, 35, 11).backend().buffer()),
            [
                "╭─────────────────────────────────╮",
                "│  2026 Wk │ Mo Tu We Th Fr Sa Su │",
                "│──────────┼──────────────────────│",
                "│       44 │ 26 27 28 29 30 31    │",
                "│──────────┼──────────────────────│",
                "│   Nov 44 │                    1 │",
                "│       45 │  2  3  4  5  6 [7] 8 │",
                "│       46 │  9 10 11 12 13 14 15 │",
                "│       47 │ 16 17 18 19 20 21 22 │",
                "╰─────────────────────────────────╯",
                " hjkl move  Tab pane  [] month  t t",
            ]
        );
        // One week back: the selected row keeps at least 2 rows from the top edge, so it scrolls up 2 rows
        app.selected = date(2026, 10, 31);
        assert_eq!(
            text_rows(draw_app(&mut app, 35, 11).backend().buffer()),
            [
                "╭─────────────────────────────────╮",
                "│  2026 Wk │ Mo Tu We Th Fr Sa Su │",
                "│──────────┼──────────────────────│",
                "│       42 │ 12 13 14 15 16 17 18 │",
                "│       43 │ 19 20 21 22 23 24 25 │",
                "│       44 │ 26 27 28 29 30[31]   │",
                "│──────────┼──────────────────────│",
                "│   Nov 44 │                    1 │",
                "│       45 │  2  3  4  5  6  7  8 │",
                "╰─────────────────────────────────╯",
                " hjkl move  Tab pane  [] month  t t",
            ]
        );
    }

    #[test]
    fn calendar_scrolls_up_into_earlier_months() {
        // From 10/17 one week up to 10/10: only 1 row is left above,
        // so it scrolls up 1 row and shows the line after September
        let mut app = App::new(Lang::ZhTw, date(2026, 10, 17), Vec::new());
        draw_app(&mut app, 35, 11);
        app.selected = date(2026, 10, 10);
        assert_eq!(
            text_rows(draw_app(&mut app, 35, 11).backend().buffer()),
            [
                "╭─────────────────────────────────╮",
                "│2026年 週 │ 一 二 三 四 五 六 日 │",
                "│──────────┼──────────────────────│",
                "│──────────┼──────────────────────│",
                "│  10月 40 │           1  2  3  4 │",
                "│       41 │  5  6  7  8  9[10]11 │",
                "│       42 │ 12 13 14 15 16 17 18 │",
                "│       43 │ 19 20 21 22 23 24 25 │",
                "│       44 │ 26 27 28 29 30 31    │",
                "╰─────────────────────────────────╯",
                " hjkl 移動  Tab 切換  [] 換月  t 今",
            ]
        );
    }

    #[test]
    fn selection_always_stays_two_rows_from_the_edges() {
        // Day by day 40 days forward, week by week 120 weeks forward, then 240 weeks back: every draw checks
        // that the brackets are on screen, at least 2 rows from the top and bottom edges
        let mut app = App::new(Lang::En, date(2026, 10, 17), Vec::new());
        let steps = std::iter::repeat_n(1, 40)
            .chain(std::iter::repeat_n(7, 120))
            .chain(std::iter::repeat_n(-7, 240));
        for step in steps {
            app.selected = app.selected.saturating_add(step.days());
            let rows = text_rows(draw_app(&mut app, 40, 13).backend().buffer());
            // Calendar rows: above are the border, the header and the
            // header line; below are the border and the key hints
            let body = &rows[3..rows.len() - 2];
            let selected = format!("[{}]", app.selected.day());
            let y = body
                .iter()
                .position(|row| row.contains(&selected))
                .unwrap_or_else(|| panic!("{} is not on screen: {body:#?}", app.selected));
            assert!(y >= 2 && y + 2 < body.len(), "{}: on row {y}", app.selected);
        }
    }

    #[test]
    fn click_positions_map_to_the_dates_on_screen() {
        // 40 × 26, October on top (screen: see
        // `calendar_from_this_month_down_with_a_thick_line_between_years`)
        let mut app = App::new(Lang::ZhTw, date(2026, 10, 17), Vec::new());
        app.screen = Rect::new(0, 0, 40, 26);
        draw_app(&mut app, 40, 26);
        // A day is 3 cells wide, and the space before it counts as that day: the 16th is x 24-26
        assert_eq!(date_at(&app, 24, 5), Some(date(2026, 10, 16)));
        assert_eq!(date_at(&app, 26, 5), Some(date(2026, 10, 16)));
        assert_eq!(date_at(&app, 27, 5), Some(date(2026, 10, 17)));
        assert_eq!(date_at(&app, 32, 5), Some(date(2026, 10, 18)));
        // 11/1 is in the last cell of November's first row; 12/31 is a Thursday
        assert_eq!(date_at(&app, 31, 9), Some(date(2026, 11, 1)));
        assert_eq!(date_at(&app, 22, 20), Some(date(2026, 12, 31)));
        // No date: the extra cell at the end, the blanks before 10/1, the line between months, the year/month
        // column, the week-number column, the header, the border, the key hints, the blank space on the right
        for (x, y) in [
            (33, 5),
            (13, 3),
            (20, 8),
            (5, 5),
            (9, 5),
            (20, 1),
            (0, 5),
            (20, 25),
            (38, 5),
        ] {
            assert_eq!(date_at(&app, x, y), None, "({x}, {y})");
        }
    }

    #[test]
    fn click_positions_follow_the_scroll() {
        // Scrolled to 11/7 (screen: see
        // `calendar_scrolls_down_only_when_the_selection_gets_close_to_the_bottom`)
        let mut app = App::new(Lang::En, date(2026, 10, 17), Vec::new());
        app.screen = Rect::new(0, 0, 40, 11);
        draw_app(&mut app, 40, 11);
        for day in [date(2026, 10, 24), date(2026, 10, 31), date(2026, 11, 7)] {
            app.selected = day;
            draw_app(&mut app, 40, 11);
        }
        // The top row is the week of 10/26, then a line, then 11/1, then the week of 11/2
        assert_eq!(date_at(&app, 13, 3), Some(date(2026, 10, 26)));
        assert_eq!(date_at(&app, 13, 4), None);
        assert_eq!(date_at(&app, 31, 5), Some(date(2026, 11, 1)));
        assert_eq!(date_at(&app, 13, 6), Some(date(2026, 11, 2)));
    }

    /// Task file for the agenda tests (`tools/expected_screen.py`
    /// reads this same file for the expected screens)
    const AGENDA_TASKS: &str = include_str!("../testdata/agenda.md");

    /// An app with today 2026/10/17, today selected, and `AGENDA_TASKS` as the task file
    fn agenda_app(lang: Lang) -> App {
        App::new(
            lang,
            date(2026, 10, 17),
            crate::tasks::parse(AGENDA_TASKS).items,
        )
    }

    #[test]
    fn the_side_menu_needs_120_columns() {
        let mut app = App::new(Lang::ZhTw, date(2026, 10, 17), Vec::new());
        assert_eq!(areas(Rect::new(0, 0, 119, 20), &app).side.width, 0);
        assert_eq!(areas(Rect::new(0, 0, 120, 20), &app).side.width, 22);
        // `s` hides it: the calendar moves back to the far left
        app.side_hidden = true;
        let hidden = areas(Rect::new(0, 0, 120, 20), &app);
        assert_eq!((hidden.side.width, hidden.calendar.x), (0, 0));
    }

    #[test]
    fn a_one_row_terminal_with_the_side_menu_does_not_panic() {
        // Only 1 row (all of it for the key hints): the side menu is 0 rows tall; whether it is on the left
        // when wide or over the left after s when narrow, nothing may panic
        let text = "# Work\n- a @2026-10-10\n";
        for (width, focus) in [(121, Focus::Calendar), (60, Focus::Side)] {
            let mut app = App::new(
                Lang::ZhTw,
                date(2026, 10, 17),
                crate::tasks::parse(text).items,
            );
            app.focus = focus;
            draw_app(&mut app, width, 1);
        }
    }

    #[test]
    fn a_long_calendar_list_scrolls_to_the_chosen_one() {
        // 121 × 10: the side menu fits 5 calendars (y 3-7), and there are 12 (c01-c12, 1 item each)
        let text: String = (1..=12)
            .map(|n| format!("# c{n:02}\n- a @2026-10-{n:02}\n"))
            .collect();
        let mut app = App::new(
            Lang::ZhTw,
            date(2026, 10, 17),
            crate::tasks::parse(&text).items,
        );
        app.screen = Rect::new(0, 0, 121, 10);
        app.focus = Focus::Side;
        let names = |app: &mut App| {
            let terminal = draw_app(app, 121, 10);
            let buffer = terminal.backend().buffer();
            let rows = area_rows(buffer, Rect::new(0, 3, 21, 5));
            let thumb: Vec<u16> = (3..8)
                .filter(|&y| buffer[(21, y)].symbol() == "┃")
                .collect();
            // Split each row 「│ ☑ c01 … 1」 on whitespace; the 3rd part is the name
            let names = rows
                .iter()
                .map(|row| row.split_whitespace().nth(2).unwrap_or("").to_string());
            (names.collect::<Vec<_>>(), thumb)
        };
        // Choose the 1st: no scroll, the scrollbar is at the top
        let (rows, thumb) = names(&mut app);
        assert_eq!(rows, ["c01", "c02", "c03", "c04", "c05"]);
        assert_eq!(thumb.first(), Some(&3));
        // Choose the 9th: scroll only once it leaves the box,
        // just enough to put it on the last row (c05 on top)
        app.group_index = 8;
        assert_eq!(names(&mut app).0, ["c05", "c06", "c07", "c08", "c09"]);
        // The last one: the scrollbar is at the bottom
        app.group_index = 11;
        let (rows, thumb) = names(&mut app);
        assert_eq!(rows, ["c08", "c09", "c10", "c11", "c12"]);
        assert_eq!(thumb.last(), Some(&7));
        // Back to the 8th: still visible, no scroll; the 6th: scroll so it is just on the first row (c06 on
        // top); clicking the first row = c06
        app.group_index = 7;
        assert_eq!(names(&mut app).0, ["c08", "c09", "c10", "c11", "c12"]);
        app.group_index = 5;
        assert_eq!(names(&mut app).0, ["c06", "c07", "c08", "c09", "c10"]);
        assert_eq!(click_at(&app, 5, 3), Some(Click::Group(5)));
        // Click the last row (c10): it gets chosen and does not scroll away; clicking the same
        // spot again = the same calendar, which unchecks it
        let click = |app: &mut App| {
            use ratatui::crossterm::event::{
                KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
            };
            app.handle_mouse(MouseEvent {
                kind: MouseEventKind::Down(MouseButton::Left),
                column: 5,
                row: 7,
                modifiers: KeyModifiers::NONE,
            });
        };
        click(&mut app);
        assert_eq!(app.group_index, 9);
        assert_eq!(names(&mut app).0, ["c06", "c07", "c08", "c09", "c10"]);
        click(&mut app);
        assert!(!app.groups[9].shown);
    }

    #[test]
    fn a_narrow_side_menu_covers_the_left_of_the_screen() {
        // 60 × 14, keyboard in the side menu (after s): the side menu is 22 cells
        // wide, over the calendar's left side, and the calendar does not shrink
        // (x 22 is still the calendar's top border); the number = how many items in
        // that calendar have a date (c in Personal has no date, so it does not count)
        let text = "# Work\n- a @2026-10-10\n# Personal\n- b @2026-10-11\n- c\n";
        let mut app = App::new(
            Lang::ZhTw,
            date(2026, 10, 17),
            crate::tasks::parse(text).items,
        );
        app.focus = Focus::Side;
        let terminal = draw_app(&mut app, 60, 14);
        let buffer = terminal.backend().buffer();
        assert_eq!(
            area_rows(buffer, Rect::new(0, 0, 22, 6)),
            [
                "╭────────────────────╮",
                "│ 日曆               │",
                "│────────────────────│",
                "│ ☑ Work           1 │",
                "│ ☑ Personal       1 │",
                "│                    │",
            ]
        );
        assert_eq!(buffer[(21, 13 - 1)].symbol(), "╯");
        assert_eq!(buffer[(22, 0)].symbol(), "─");
        // Keyboard in the side menu: the key hints are the calendar list's
        assert_eq!(
            text_rows(buffer)[13].trim_end(),
            " jk 移動  Space 勾選  a 新增日曆  e 改名  d 刪除  Esc 回去"
        );
        // Keyboard not in the side menu: not drawn; the key hints have s even when narrow (60 cells wide,
        // the end is cut off)
        app.focus = Focus::Calendar;
        let terminal = draw_app(&mut app, 60, 14);
        assert_eq!(terminal.backend().buffer()[(0, 3)].symbol(), "│");
        assert_ne!(terminal.backend().buffer()[(4, 3)].symbol(), "W");
        assert!(
            text_rows(terminal.backend().buffer())[13]
                .starts_with(" hjkl 移動  Tab 切換  [] 換月  t 今天  s 日曆")
        );
    }

    #[test]
    fn side_menu_lists_the_calendars_when_wide() {
        // 121 cells wide: the calendars are listed on the far left (the task file's groups, in order of
        // appearance; items before any heading are 「未分組」), the number is how many items have a date
        // (Work has one item without a date); the last name is too long even at the 36-cell max width
        // (`[calendar_list] max_width`'s default), so it is cut; the key hints gain an `s`. The agenda has room for one column only and October's 8 items
        // do not fit: it scrolls 1 row, leaving one row after the selected day's (10/17) items
        let mut app = agenda_app(Lang::ZhTw);
        let terminal = draw_app(&mut app, 121, 14);
        assert_eq!(
            text_rows(terminal.backend().buffer()),
            [
                "╭──────────────────────────────────╮╭─────────────────────────────────╮╭────────────────────────────────────────────────╮",
                "│ 日曆                             ││2026年 週 │ 一 二 三 四 五 六 日 ││ 重要事項                      2026/10/17（六） │",
                "│──────────────────────────────────││──────────┼──────────────────────││────────────────────────────────────────────────│",
                "│ ☑ 未分組                       2 ││  10月 40 │           1  2  3  4 ││ 10/10 Farmers market                           │",
                "│ ☑ Personal                     5 ││       41 │  5  6  7  8  9 10 11 ││ 10/15 Fix the login bug                        ┃",
                "│ ☑ Work                         1 ││       42 │ 12 13 14 15 16[17]18 ││ 10/16 17:30 Send the weekly update             ┃",
                "│   ☑ Year-end odds and ends and 2 ││       43 │ 19 20 21 22 23 24 25 ││ 10/17 Dinner with friends                      ┃",
                "│                                  ││       44 │ 26 27 28 29 30 31    ││ 10/17 08:00 Coffee chat                        ┃",
                "│                                  ││          │                      ││ 10/18 07:00 Go hiking                          │",
                "│                                  ││──────────┼──────────────────────││──────────────────────────────────── 還有 1 件 ─│",
                "│                                  ││  11月 44 │                    1 ││  11/3 09:30 Release v2.3                       │",
                "│                                  ││       45 │  2  3  4  5  6  7  8 ││                                                │",
                "╰──────────────────────────────────╯╰─────────────────────────────────╯╰────────────────────────────────────────────────╯",
                " hjkl 移動  Tab 切換  [] 換月  t 今天  s 日曆  ? 說明  , 設定  q 離開",
            ]
        );
        // ☑ uses the `checked` color, names are normal text, numbers
        // and the header are muted, the border is a normal line
        let theme = Theme::catppuccin_mocha();
        let buffer = terminal.backend().buffer();
        let cell = |x: u16, y: u16| (buffer[(x, y)].symbol(), buffer[(x, y)].fg);
        assert_eq!(cell(2, 3), ("☑", theme.checked));
        assert_eq!(cell(4, 4), ("P", theme.text));
        assert_eq!(cell(33, 4), ("5", theme.muted));
        assert_eq!(cell(2, 1), ("日", theme.muted));
        assert_eq!(cell(0, 0), ("╭", theme.line));
        // `s` hides it: the calendar moves back to the far left,
        // and the key hints still have `s` (press it again to open)
        app.side_hidden = true;
        assert_eq!(
            text_rows(draw_app(&mut app, 121, 14).backend().buffer()),
            [
                "╭─────────────────────────────────╮╭────────────────────────────────────────────────────────────────────────────────────╮",
                "│2026年 週 │ 一 二 三 四 五 六 日 ││ 重要事項                                                          2026/10/17（六） │",
                "│──────────┼──────────────────────││────────────────────────────────────────────────────────────────────────────────────│",
                "│  10月 40 │           1  2  3  4 ││  10/5 Pay the phone bill                   10/17 08:00 Coffee chat                 │",
                "│       41 │  5  6  7  8  9 10 11 ││ 10/10 Farmers market                       10/18 07:00 Go hiking                   │",
                "│       42 │ 12 13 14 15 16[17]18 ││ 10/15 Fix the login bug                    10/20 A really long title that just keep│",
                "│       43 │ 19 20 21 22 23 24 25 ││ 10/16 17:30 Send the weekly update                                                 │",
                "│       44 │ 26 27 28 29 30 31    ││ 10/17 Dinner with friends                                                          │",
                "│──────────┼──────────────────────││────────────────────────────────────────────────────────────────────────────────────│",
                "│  11月 44 │                    1 ││  11/3 09:30 Release v2.3                                                           │",
                "│       45 │  2  3  4  5  6  7  8 ││                                                                                    │",
                "│       46 │  9 10 11 12 13 14 15 ││                                                                                    │",
                "╰─────────────────────────────────╯╰────────────────────────────────────────────────────────────────────────────────────╯",
                " hjkl 移動  Tab 切換  [] 換月  t 今天  s 日曆  ? 說明  , 設定  q 離開",
            ]
        );
    }

    #[test]
    fn unchecked_calendars_hide_their_items() {
        // Personal unchecked: ☐ and the name are muted, the number still counts; its 5 items are not listed,
        // and 10/5 and 10/10 are no longer orange
        let mut app = agenda_app(Lang::En);
        app.groups[1].shown = false;
        let terminal = draw_app(&mut app, 121, 14);
        assert_eq!(
            text_rows(terminal.backend().buffer()),
            [
                "╭──────────────────────────────────╮╭─────────────────────────────────╮╭────────────────────────────────────────────────╮",
                "│ Calendars                        ││  2026 Wk │ Mo Tu We Th Fr Sa Su ││ Agenda                        Sat Oct 17, 2026 │",
                "│──────────────────────────────────││──────────┼──────────────────────││────────────────────────────────────────────────│",
                "│ ☑ Ungrouped                    2 ││   Oct 40 │           1  2  3  4 ││ 10/15 Fix the login bug                        │",
                "│ ☐ Personal                     5 ││       41 │  5  6  7  8  9 10 11 ││ 10/16 17:30 Send the weekly update             │",
                "│ ☑ Work                         1 ││       42 │ 12 13 14 15 16[17]18 ││ 10/20 A really long title that just keeps going│",
                "│   ☑ Year-end odds and ends and 2 ││       43 │ 19 20 21 22 23 24 25 ││                                                │",
                "│                                  ││       44 │ 26 27 28 29 30 31    ││                                                │",
                "│                                  ││──────────┼──────────────────────││────────────────────────────────────────────────│",
                "│                                  ││   Nov 44 │                    1 ││  11/3 09:30 Release v2.3                       │",
                "│                                  ││       45 │  2  3  4  5  6  7  8 ││                                                │",
                "│                                  ││       46 │  9 10 11 12 13 14 15 ││                                                │",
                "╰──────────────────────────────────╯╰─────────────────────────────────╯╰────────────────────────────────────────────────╯",
                " hjkl move  Tab pane  [] month  t today  s calendars  ? help  , settings  q quit",
            ]
        );
        let theme = Theme::catppuccin_mocha();
        let buffer = terminal.backend().buffer();
        let cell = |x: u16, y: u16| (buffer[(x, y)].symbol(), buffer[(x, y)].fg);
        assert_eq!(cell(2, 4), ("☐", theme.muted));
        assert_eq!(cell(4, 4), ("P", theme.muted));
        assert_eq!(cell(2, 3), ("☑", theme.checked));
        // 10/5 (Monday) and 10/10 (Saturday) go back to their
        // usual colors; 10/16 is an ungrouped item, still orange
        assert_eq!(cell(50, 4), ("5", theme.text));
        assert_eq!(cell(65, 4), ("0", theme.weekend));
        assert_eq!(cell(62, 5), ("6", theme.has_item));
    }

    #[test]
    fn a_calendar_inside_an_unchecked_one_is_gray() {
        // Work unchecked: SFAP inside it keeps its ☑ but is gray like Work (it is hidden too); Personal is not
        let mut app = App::new(Lang::En, date(2026, 10, 17), Vec::new());
        app.set_items(crate::tasks::parse(
            "# Work\n## SFAP\n- a @2026-10-10\n# Personal\n- b @2026-10-11\n",
        ));
        app.groups[0].shown = false;
        let terminal = draw_app(&mut app, 121, 14);
        let buffer = terminal.backend().buffer();
        assert_eq!(
            area_rows(buffer, Rect::new(0, 3, 22, 3)),
            [
                "│ ☐ Work           0 │",
                "│   ☑ SFAP         1 │",
                "│ ☑ Personal       1 │",
            ]
        );
        let theme = Theme::catppuccin_mocha();
        let cell = |x: u16, y: u16| (buffer[(x, y)].symbol(), buffer[(x, y)].fg);
        assert_eq!(cell(4, 4), ("☑", theme.muted));
        assert_eq!(cell(6, 4), ("S", theme.muted));
        assert_eq!(cell(2, 5), ("☑", theme.checked));
        assert_eq!(cell(4, 5), ("P", theme.text));
    }

    #[test]
    fn side_menu_focus_lights_its_border_and_the_chosen_calendar() {
        // Keyboard in the side menu, the 2nd one chosen (Personal): the side menu's border is lit, the
        // calendar's is not; that whole row gets the highlight background, text colors unchanged
        let mut app = agenda_app(Lang::ZhTw);
        app.focus = Focus::Side;
        app.group_index = 1;
        let terminal = draw_app(&mut app, 121, 14);
        let theme = Theme::catppuccin_mocha();
        let buffer = terminal.backend().buffer();
        assert_eq!(buffer[(0, 0)].fg, theme.focus);
        assert_eq!(buffer[(36, 0)].fg, theme.line);
        for x in 1..35 {
            assert_eq!(buffer[(x, 4)].bg, theme.cursor_line, "x = {x}");
            assert_ne!(buffer[(x, 3)].bg, theme.cursor_line, "x = {x}");
        }
        assert_eq!(
            (buffer[(2, 4)].symbol(), buffer[(2, 4)].fg),
            ("☑", theme.checked)
        );
    }

    #[test]
    fn click_positions_in_the_side_menu() {
        // 121 × 14 (screen: see `side_menu_lists_the_calendars_when_wide`):
        // calendars start at y 3, one per row
        let mut app = agenda_app(Lang::ZhTw);
        app.screen = Rect::new(0, 0, 121, 14);
        draw_app(&mut app, 121, 14);
        assert_eq!(click_at(&app, 5, 3), Some(Click::Group(0)));
        assert_eq!(click_at(&app, 1, 6), Some(Click::Group(3)));
        assert_eq!(click_at(&app, 20, 4), Some(Click::Group(1)));
        // No calendar: the blank space below, the header, the
        // header line, the left and right borders, the key hints
        for (x, y) in [(5, 7), (5, 1), (5, 2), (0, 3), (35, 3), (5, 13)] {
            assert_eq!(click_at(&app, x, y), None, "({x}, {y})");
        }
        // The calendar moved 36 cells right: the 16th is at x 60-62
        assert_eq!(date_at(&app, 60, 5), Some(date(2026, 10, 16)));
        assert_eq!(date_at(&app, 63, 5), Some(date(2026, 10, 17)));
    }

    #[test]
    fn agenda_lists_each_months_items_next_to_it() {
        // One agenda column: October's 8 items outnumber its 5 weeks, so the month grows 3 rows down; sorted
        // by date and time (on the same day, items without a time first); items without a date are not
        // listed; titles that are too long are cut
        let mut app = agenda_app(Lang::ZhTw);
        assert_eq!(
            text_rows(draw_app(&mut app, 70, 23).backend().buffer()),
            [
                "╭─────────────────────────────────╮╭─────────────────────────────────╮",
                "│2026年 週 │ 一 二 三 四 五 六 日 ││ 重要事項       2026/10/17（六） │",
                "│──────────┼──────────────────────││─────────────────────────────────│",
                "│  10月 40 │           1  2  3  4 ││  10/5 Pay the phone bill        │",
                "│       41 │  5  6  7  8  9 10 11 ││ 10/10 Farmers market            │",
                "│       42 │ 12 13 14 15 16[17]18 ││ 10/15 Fix the login bug         │",
                "│       43 │ 19 20 21 22 23 24 25 ││ 10/16 17:30 Send the weekly upda│",
                "│       44 │ 26 27 28 29 30 31    ││ 10/17 Dinner with friends       │",
                "│          │                      ││ 10/17 08:00 Coffee chat         │",
                "│          │                      ││ 10/18 07:00 Go hiking           │",
                "│          │                      ││ 10/20 A really long title that j│",
                "│──────────┼──────────────────────││─────────────────────────────────│",
                "│  11月 44 │                    1 ││  11/3 09:30 Release v2.3        │",
                "│       45 │  2  3  4  5  6  7  8 ││                                 │",
                "│       46 │  9 10 11 12 13 14 15 ││                                 │",
                "│       47 │ 16 17 18 19 20 21 22 ││                                 │",
                "│       48 │ 23 24 25 26 27 28 29 ││                                 │",
                "│       49 │ 30                   ││                                 │",
                "│──────────┼──────────────────────││─────────────────────────────────│",
                "│  12月 49 │     1  2  3  4  5  6 ││ 12/31 23:00 New Year's Eve count│",
                "│       50 │  7  8  9 10 11 12 13 ││                                 │",
                "╰─────────────────────────────────╯╰─────────────────────────────────╯",
                " hjkl 移動  Tab 切換  [] 換月  t 今天  s 日曆  ? 說明  , 設定  q 離開",
            ]
        );
    }

    #[test]
    fn agenda_flows_into_columns_when_wide() {
        // Agenda 58 cells wide: 2 columns, filled like a newspaper,
        // the first column full (October's 5 rows) before the second
        let mut app = agenda_app(Lang::En);
        assert_eq!(
            text_rows(draw_app(&mut app, 95, 15).backend().buffer()),
            [
                "╭─────────────────────────────────╮╭──────────────────────────────────────────────────────────╮",
                "│  2026 Wk │ Mo Tu We Th Fr Sa Su ││ Agenda                                  Sat Oct 17, 2026 │",
                "│──────────┼──────────────────────││──────────────────────────────────────────────────────────│",
                "│   Oct 40 │           1  2  3  4 ││  10/5 Pay the phone bill      10/17 08:00 Coffee chat    │",
                "│       41 │  5  6  7  8  9 10 11 ││ 10/10 Farmers market          10/18 07:00 Go hiking      │",
                "│       42 │ 12 13 14 15 16[17]18 ││ 10/15 Fix the login bug       10/20 A really long title t│",
                "│       43 │ 19 20 21 22 23 24 25 ││ 10/16 17:30 Send the weekly                              │",
                "│       44 │ 26 27 28 29 30 31    ││ 10/17 Dinner with friends                                │",
                "│──────────┼──────────────────────││──────────────────────────────────────────────────────────│",
                "│   Nov 44 │                    1 ││  11/3 09:30 Release v2.3                                 │",
                "│       45 │  2  3  4  5  6  7  8 ││                                                          │",
                "│       46 │  9 10 11 12 13 14 15 ││                                                          │",
                "│       47 │ 16 17 18 19 20 21 22 ││                                                          │",
                "╰─────────────────────────────────╯╰──────────────────────────────────────────────────────────╯",
                " hjkl move  Tab pane  [] month  t today  s calendars  ? help  , settings  q quit",
            ]
        );
    }

    #[test]
    fn clicking_an_item_selects_its_day() {
        // One column (screen: see `agenda_lists_each_months_items_next_to_it`):
        // the inside of the agenda box starts at x 36
        let mut app = agenda_app(Lang::ZhTw);
        app.screen = Rect::new(0, 0, 70, 23);
        draw_app(&mut app, 70, 23);
        assert_eq!(date_at(&app, 40, 3), Some(date(2026, 10, 5)));
        assert_eq!(date_at(&app, 68, 8), Some(date(2026, 10, 17)));
        assert_eq!(date_at(&app, 40, 12), Some(date(2026, 11, 3)));
        // Agenda rows without an item, and the header; on the
        // left, the rows that exist only for items have no date
        assert_eq!(date_at(&app, 40, 13), None);
        assert_eq!(date_at(&app, 40, 1), None);
        assert_eq!(date_at(&app, 20, 8), None);
        // Which agenda item was clicked (in `App::agenda()` order)
        assert_eq!(click_at(&app, 40, 3), Some(Click::Item(0)));
        assert_eq!(click_at(&app, 20, 5), Some(Click::Day(date(2026, 10, 14))));
        // Two columns (screen: see `agenda_flows_into_columns_when_wide`): the second column starts
        // at x 36 + 28 + 2 = 66; the 2 blank cells between do not count
        let mut app = agenda_app(Lang::En);
        app.screen = Rect::new(0, 0, 95, 15);
        draw_app(&mut app, 95, 15);
        assert_eq!(date_at(&app, 70, 3), Some(date(2026, 10, 17)));
        assert_eq!(date_at(&app, 63, 3), Some(date(2026, 10, 5)));
        assert_eq!(date_at(&app, 64, 3), None);
        assert_eq!(date_at(&app, 65, 3), None);
        assert_eq!(date_at(&app, 66, 3), Some(date(2026, 10, 17)));
    }

    #[test]
    fn the_selected_day_and_its_items_stay_on_screen_with_tall_months() {
        // In the sample task file some months are taller than their weeks (October has
        // many items): keyboard in the calendar, day by day 120 days forward, 240 days
        // back; every draw checks: the selected day's brackets are visible on the
        // calendar; if that day has items, its first item is visible in the agenda
        let mut app = App::new(
            Lang::En,
            date(2026, 10, 17),
            crate::tasks::parse(include_str!("../testdata/tasks.md")).items,
        );
        app.selected = date(2026, 9, 1);
        let steps = std::iter::repeat_n(1, 120).chain(std::iter::repeat_n(-1, 240));
        for step in steps {
            app.selected = app.selected.saturating_add(step.days());
            let rows = text_rows(draw_app(&mut app, 70, 13).backend().buffer());
            let body = &rows[3..rows.len() - 2];
            // In each row, left of `││` is the calendar box and right of it the agenda
            let (left, right): (Vec<&str>, Vec<&str>) = body
                .iter()
                .map(|row| row.split_once("││").unwrap_or((row, "")))
                .unzip();
            let day = format!("[{}]", app.selected.day());
            assert!(
                left.iter().any(|row| row.contains(&day)),
                "{}: {day} is not on the calendar: {body:#?}",
                app.selected
            );
            // The day's first item (the middle days of a multi-day item
            // count too): the agenda shows its start day, e.g. ` 10/28~11/3`
            let first = app
                .agenda()
                .iter()
                .find(|entry| entry.is_on(app.selected))
                .and_then(|entry| entry.item.date);
            if let Some(first) = first {
                let date = format!(" {}/{}", first.month(), first.day());
                assert!(
                    right.iter().any(|row| row.contains(&date)),
                    "{}: the day's items are not in the agenda: {body:#?}",
                    app.selected
                );
            }
        }
    }

    #[test]
    fn item_colors_follow_their_day() {
        // Screen: see `agenda_lists_each_months_items_next_to_it`;
        // the inside of the agenda box starts at x 36
        let mut app = agenda_app(Lang::ZhTw);
        let terminal = draw_app(&mut app, 70, 23);
        let buffer = terminal.backend().buffer();
        let theme = Theme::catppuccin_mocha();
        // The (x, y) cell: symbol, foreground color, whether it is struck through
        let cell = |x: u16, y: u16| {
            let cell = &buffer[(x, y)];
            (
                cell.symbol(),
                cell.fg,
                cell.modifier.contains(Modifier::CROSSED_OUT),
            )
        };
        // Agenda header: 「重要事項」 muted, the selected day on
        // the right in the normal color (2026/10/17 starts at x 52)
        assert_eq!(cell(37, 1), ("重", theme.muted, false));
        assert_eq!(cell(52, 1), ("2", theme.text, false));
        // 10/5 has an unfinished item: date and title are orange, same as on the calendar
        assert_eq!(cell(38, 3), ("1", theme.has_item, false));
        assert_eq!(cell(43, 3), ("P", theme.has_item, false));
        // 10/15's finished item: the whole line is gray (the date too), the title struck through
        assert_eq!(cell(37, 5), ("1", theme.muted, false));
        assert_eq!(cell(43, 5), ("F", theme.muted, true));
        // 10/17 today: green; the time is muted
        assert_eq!(cell(43, 7), ("D", theme.today, false));
        assert_eq!(cell(43, 8), ("0", theme.muted, false));
        assert_eq!(cell(49, 8), ("C", theme.today, false));
        // 10/18 (Sun) has an unfinished item: orange wins over the weekend color
        assert_eq!(cell(49, 9), ("G", theme.has_item, false));
    }

    #[test]
    fn the_selected_week_row_has_a_background() {
        // The week of 10/17 is at y 5: that whole row in the calendar box (x 1-33) gets the highlight
        // background; the rows above and below, and the same row in the agenda, do not
        let mut app = agenda_app(Lang::ZhTw);
        let theme = Theme::catppuccin_mocha();
        let bg = |terminal: &Terminal<TestBackend>, x: u16, y: u16| {
            terminal.backend().buffer()[(x, y)].bg
        };
        let terminal = draw_app(&mut app, 70, 23);
        for x in 1..=33 {
            assert_eq!(bg(&terminal, x, 5), theme.cursor_line, "x {x}");
        }
        assert_eq!(bg(&terminal, 0, 5), Color::Reset);
        assert_eq!(bg(&terminal, 20, 4), Color::Reset);
        assert_eq!(bg(&terminal, 20, 6), Color::Reset);
        assert_eq!(bg(&terminal, 40, 5), Color::Reset);
        // The week that crosses months is split into two rows: with 10/31 selected, only the October half
        // (y 7) gets the highlight, the November half (y 12) does not
        app.selected = date(2026, 10, 31);
        let terminal = draw_app(&mut app, 70, 23);
        assert_eq!(bg(&terminal, 20, 7), theme.cursor_line);
        assert_eq!(bg(&terminal, 20, 12), Color::Reset);
    }

    #[test]
    fn agenda_focus_brackets_the_chosen_item() {
        // Keyboard in the agenda, the 6th item chosen (10/17 08:00 Coffee chat): its date gets brackets
        let mut app = agenda_app(Lang::ZhTw);
        app.focus = Focus::Agenda;
        app.agenda_index = 5;
        let terminal = draw_app(&mut app, 70, 23);
        assert_eq!(
            text_rows(terminal.backend().buffer()),
            [
                "╭─────────────────────────────────╮╭─────────────────────────────────╮",
                "│2026年 週 │ 一 二 三 四 五 六 日 ││ 重要事項       2026/10/17（六） │",
                "│──────────┼──────────────────────││─────────────────────────────────│",
                "│  10月 40 │           1  2  3  4 ││  10/5 Pay the phone bill        │",
                "│       41 │  5  6  7  8  9 10 11 ││ 10/10 Farmers market            │",
                "│       42 │ 12 13 14 15 16[17]18 ││ 10/15 Fix the login bug         │",
                "│       43 │ 19 20 21 22 23 24 25 ││ 10/16 17:30 Send the weekly upda│",
                "│       44 │ 26 27 28 29 30 31    ││ 10/17 Dinner with friends       │",
                "│          │                      ││[10/17]08:00 Coffee chat         │",
                "│          │                      ││ 10/18 07:00 Go hiking           │",
                "│          │                      ││ 10/20 A really long title that j│",
                "│──────────┼──────────────────────││─────────────────────────────────│",
                "│  11月 44 │                    1 ││  11/3 09:30 Release v2.3        │",
                "│       45 │  2  3  4  5  6  7  8 ││                                 │",
                "│       46 │  9 10 11 12 13 14 15 ││                                 │",
                "│       47 │ 16 17 18 19 20 21 22 ││                                 │",
                "│       48 │ 23 24 25 26 27 28 29 ││                                 │",
                "│       49 │ 30                   ││                                 │",
                "│──────────┼──────────────────────││─────────────────────────────────│",
                "│  12月 49 │     1  2  3  4  5  6 ││ 12/31 23:00 New Year's Eve count│",
                "│       50 │  7  8  9 10 11 12 13 ││                                 │",
                "╰─────────────────────────────────╯╰─────────────────────────────────╯",
                " hjkl 移動  Tab 切換  [] 換月  t 今天  s 日曆  ? 說明  , 設定  q 離開",
            ]
        );
        let buffer = terminal.backend().buffer();
        let theme = Theme::catppuccin_mocha();
        // The agenda's border lights up, and the calendar box's goes back to a normal line
        assert_eq!(buffer[(35, 0)].fg, theme.focus);
        assert_eq!(buffer[(0, 0)].fg, theme.line);
        // The brackets are in the selected color; the chosen item gets the highlight across the whole column
        // (x 36-68), the items above and below do not
        assert_eq!(
            (buffer[(36, 8)].symbol(), buffer[(36, 8)].fg),
            ("[", theme.selected)
        );
        assert_eq!(
            (buffer[(42, 8)].symbol(), buffer[(42, 8)].fg),
            ("]", theme.selected)
        );
        // The second half of a wide (CJK) character is not sent to the terminal, so skip it (on a real
        // terminal the background covers both cells)
        let mut x = 36;
        while x <= 68 {
            assert_eq!(buffer[(x, 8)].bg, theme.cursor_line, "x {x}");
            x += u16::try_from(Span::raw(buffer[(x, 8)].symbol()).width().max(1)).unwrap();
        }
        assert_eq!(buffer[(40, 7)].bg, Color::Reset);
        assert_eq!(buffer[(40, 9)].bg, Color::Reset);
    }

    #[test]
    fn the_chosen_item_is_always_on_screen() {
        // Sample task file: October has far more items than weeks (a month does not fit, so the agenda
        // scrolls inside it); choose items one by one going down, then going up; the chosen item (date in
        // brackets) and the selected day on the calendar both stay on screen
        let mut app = App::new(
            Lang::En,
            date(2026, 10, 17),
            crate::tasks::parse(include_str!("../testdata/tasks.md")).items,
        );
        app.focus = Focus::Agenda;
        let count = app.agenda().len();
        for index in (0..count).chain((0..count).rev()) {
            app.agenda_index = index;
            app.selected = app.agenda()[index].date;
            let first = app.agenda()[index].item.date.expect("has a date");
            let rows = text_rows(draw_app(&mut app, 70, 13).backend().buffer());
            let body = &rows[3..rows.len() - 2];
            // The chosen item's date gets brackets; for a multi-day
            // item the brackets hold the range, e.g. `[10/28~11/3]`
            let chosen = format!("[{}/{}", first.month(), first.day());
            assert!(
                body.iter().any(|row| row.contains(&chosen)),
                "item {index} is not on screen: {body:#?}"
            );
            // The selected day is also visible in the calendar box (left of `││` in each row)
            let day = format!("[{}]", app.selected.day());
            assert!(
                body.iter().any(|row| row
                    .split("││")
                    .next()
                    .is_some_and(|left| left.contains(&day))),
                "item {index}: {day} is not on the calendar: {body:#?}"
            );
        }
    }

    #[test]
    fn a_capped_month_scrolls_inside_while_choosing_items() {
        // 6 calendar rows → a month gets at most 3 rows, but at least its number of weeks (October has 5):
        // only 5 of October's 8 items fit. Keyboard in the agenda, the 8th item chosen (10/20): October's
        // agenda scrolls 3 rows, the calendar does not move
        let mut app = agenda_app(Lang::ZhTw);
        app.screen = Rect::new(0, 0, 70, 11);
        app.focus = Focus::Agenda;
        app.agenda_index = 7;
        app.selected = date(2026, 10, 20);
        let terminal = draw_app(&mut app, 70, 11);
        assert_eq!(
            text_rows(terminal.backend().buffer()),
            [
                "╭─────────────────────────────────╮╭─────────────────────────────────╮",
                "│2026年 週 │ 一 二 三 四 五 六 日 ││ 重要事項       2026/10/20（二） │",
                "│──────────┼──────────────────────││─────────────────────────────────│",
                "│  10月 40 │           1  2  3  4 ││ 10/16 17:30 Send the weekly upda│",
                "│       41 │  5  6  7  8  9 10 11 ││ 10/17 Dinner with friends       │",
                "│       42 │ 12 13 14 15 16 17 18 ││ 10/17 08:00 Coffee chat         ┃",
                "│       43 │ 19[20]21 22 23 24 25 ││ 10/18 07:00 Go hiking           ┃",
                "│       44 │ 26 27 28 29 30 31    ││[10/20]A really long title that j┃",
                "│──────────┼──────────────────────││─────────────────────────────────│",
                "╰─────────────────────────────────╯╰─────────────────────────────────╯",
                " hjkl 移動  Tab 切換  [] 換月  t 今天  s 日曆  ? 說明  , 設定  q 離開",
            ]
        );
        assert_eq!(app.agenda_window, Some((date(2026, 10, 1), 3)));
        assert_eq!(app.calendar_top, Some(0));
        // The scrollbar thumb has the same color as the border (keyboard in the agenda: focus color)
        let theme = Theme::catppuccin_mocha();
        let cell = &terminal.backend().buffer()[(69, 5)];
        assert_eq!((cell.symbol(), cell.fg), ("┃", theme.focus));
        // Clicks match the screen: the agenda's first row is the 4th item (10/16 Send the weekly update)
        assert_eq!(click_at(&app, 40, 3), Some(Click::Item(3)));
        // Back to the 1st item (10/5): the agenda scrolls back to the top
        app.agenda_index = 0;
        app.selected = date(2026, 10, 5);
        draw_app(&mut app, 70, 11);
        assert_eq!(app.agenda_window, Some((date(2026, 10, 1), 0)));
    }

    #[test]
    fn the_agenda_follows_the_calendar() {
        // Keyboard in the calendar, only 5 of October's items fit: select 10/20, and the agenda scrolls until
        // the 10/20 item is visible; the calendar does not move
        let mut app = agenda_app(Lang::ZhTw);
        draw_app(&mut app, 70, 11);
        app.selected = date(2026, 10, 20);
        let terminal = draw_app(&mut app, 70, 11);
        assert_eq!(
            text_rows(terminal.backend().buffer()),
            [
                "╭─────────────────────────────────╮╭─────────────────────────────────╮",
                "│2026年 週 │ 一 二 三 四 五 六 日 ││ 重要事項       2026/10/20（二） │",
                "│──────────┼──────────────────────││─────────────────────────────────│",
                "│  10月 40 │           1  2  3  4 ││ 10/16 17:30 Send the weekly upda│",
                "│       41 │  5  6  7  8  9 10 11 ││ 10/17 Dinner with friends       │",
                "│       42 │ 12 13 14 15 16 17 18 ││ 10/17 08:00 Coffee chat         ┃",
                "│       43 │ 19[20]21 22 23 24 25 ││ 10/18 07:00 Go hiking           ┃",
                "│       44 │ 26 27 28 29 30 31    ││ 10/20 A really long title that j┃",
                "│──────────┼──────────────────────││─────────────────────────────────│",
                "╰─────────────────────────────────╯╰─────────────────────────────────╯",
                " hjkl 移動  Tab 切換  [] 換月  t 今天  s 日曆  ? 說明  , 設定  q 離開",
            ]
        );
        assert_eq!(app.agenda_window, Some((date(2026, 10, 1), 3)));
        assert_eq!(app.calendar_top, Some(0));
        // That day's items get the highlight background (like
        // its week on the calendar); other days' items do not
        let buffer = terminal.backend().buffer();
        let theme = Theme::catppuccin_mocha();
        assert_eq!(buffer[(40, 7)].bg, theme.cursor_line);
        assert_eq!(buffer[(40, 6)].bg, Color::Reset);
        // Select 10/19, which has no items: the agenda does not scroll (no jumping up and down)
        app.selected = date(2026, 10, 19);
        draw_app(&mut app, 70, 11);
        assert_eq!(app.agenda_window, Some((date(2026, 10, 1), 3)));
        // Select 10/5: the agenda scrolls back to the top
        app.selected = date(2026, 10, 5);
        draw_app(&mut app, 70, 11);
        assert_eq!(app.agenda_window, Some((date(2026, 10, 1), 0)));
    }

    #[test]
    fn the_year_in_the_corner_is_the_top_weeks() {
        // 6 calendar rows: select 12/28, then go one week down to 2027/1/4; the top of the screen is still
        // the December week → the top-left corner shows 2026年
        let mut app = App::new(Lang::ZhTw, date(2026, 10, 17), Vec::new());
        app.selected = date(2026, 12, 28);
        draw_app(&mut app, 35, 11);
        app.selected = date(2027, 1, 4);
        let rows = text_rows(draw_app(&mut app, 35, 11).backend().buffer());
        assert_eq!(rows[1], "│2026年 週 │ 一 二 三 四 五 六 日 │");
        assert_eq!(rows[3], "│       53 │ 28 29 30 31          │");
    }

    #[test]
    fn a_busy_month_takes_at_most_two_thirds() {
        // Sample task file, 70 × 24: the calendar has 19 rows and October has many items → it
        // takes 19 × 2 / 3 = 12 rows, and a few weeks of November show below
        let mut app = App::new(
            Lang::ZhTw,
            date(2026, 10, 17),
            crate::tasks::parse(include_str!("../testdata/tasks.md")).items,
        );
        let rows = text_rows(draw_app(&mut app, 70, 24).backend().buffer());
        let body = &rows[3..rows.len() - 2];
        assert_eq!(body.len(), 19);
        let separator = body.iter().position(|row| row.contains('┼'));
        assert_eq!(separator, Some(12));
        assert!(body[13].contains("11月"));
    }

    #[test]
    fn a_hint_tells_how_many_more_items_a_month_has() {
        // Just opened (today 10/17 selected), only 5 of October's items fit: the agenda scrolls 2 rows to
        // show 10/17's items, and the line under October says 「還有 1 件」 (the 10/20 item)
        let mut app = agenda_app(Lang::ZhTw);
        let terminal = draw_app(&mut app, 70, 11);
        assert_eq!(
            text_rows(terminal.backend().buffer()),
            [
                "╭─────────────────────────────────╮╭─────────────────────────────────╮",
                "│2026年 週 │ 一 二 三 四 五 六 日 ││ 重要事項       2026/10/17（六） │",
                "│──────────┼──────────────────────││─────────────────────────────────│",
                "│  10月 40 │           1  2  3  4 ││ 10/15 Fix the login bug         │",
                "│       41 │  5  6  7  8  9 10 11 ││ 10/16 17:30 Send the weekly upda┃",
                "│       42 │ 12 13 14 15 16[17]18 ││ 10/17 Dinner with friends       ┃",
                "│       43 │ 19 20 21 22 23 24 25 ││ 10/17 08:00 Coffee chat         ┃",
                "│       44 │ 26 27 28 29 30 31    ││ 10/18 07:00 Go hiking           │",
                "│──────────┼──────────────────────││───────────────────── 還有 1 件 ─│",
                "╰─────────────────────────────────╯╰─────────────────────────────────╯",
                " hjkl 移動  Tab 切換  [] 換月  t 今天  s 日曆  ? 說明  , 設定  q 離開",
            ]
        );
        // The hint is muted; the line on both sides of it keeps the
        // line color (「還」 is at x 58, the line's last cell at x 68)
        let buffer = terminal.backend().buffer();
        let theme = Theme::catppuccin_mocha();
        assert_eq!(
            (buffer[(58, 8)].symbol(), buffer[(58, 8)].fg),
            ("還", theme.muted)
        );
        assert_eq!(
            (buffer[(40, 8)].symbol(), buffer[(40, 8)].fg),
            ("─", theme.line)
        );
        assert_eq!(
            (buffer[(68, 8)].symbol(), buffer[(68, 8)].fg),
            ("─", theme.line)
        );
    }

    #[test]
    fn the_editor_shows_the_line_and_what_it_reads_as() {
        use ratatui::crossterm::event::{KeyCode, KeyEvent};
        let mut app = App::new(
            Lang::ZhTw,
            date(2026, 10, 17),
            // 🎂 is on purpose: 2 cells wide, so the cursor position below counts cells, not chars or bytes
            crate::tasks::parse("- [ ] Pick up the cake 🎂 @2026-10-16 17:30\n").items,
        );
        app.handle_key(KeyEvent::from(KeyCode::Tab));
        app.handle_key(KeyEvent::from(KeyCode::Char('e')));
        // 70 × 12: the edit box sits just above the key hints
        // at the bottom (y 7-10), as wide as the whole terminal
        let mut terminal = draw_app(&mut app, 70, 12);
        let rows = text_rows(terminal.backend().buffer());
        assert!(rows[7].starts_with("╭ 編輯第 1 行 ─"), "{}", rows[7]);
        assert!(rows[7].ends_with("─╮"));
        assert_eq!(
            rows[8].trim_end_matches([' ', '│']),
            "│ - [ ] Pick up the cake 🎂 @2026-10-16 17:30"
        );
        assert_eq!(
            rows[9].trim_end_matches([' ', '│']),
            "│ 任務 · 2026/10/16（五） 17:30 · 「Pick up the cake 🎂」"
        );
        assert!(
            rows[10].starts_with("╰ Enter 存檔  Esc 取消 ─"),
            "{}",
            rows[10]
        );
        // The cursor is right after the typed text, at
        // border 1 + space 1 + 「- [ ] Pick up the cake 🎂 @2026-10-16 17:30」 43 cells (42 chars)
        assert_eq!(
            terminal.get_cursor_position().expect("cursor"),
            Position::new(45, 8)
        );
        // What the line parses as is shown muted; change it to a time
        // that does not exist: that row becomes a red explanation
        let theme = Theme::catppuccin_mocha();
        assert_eq!(terminal.backend().buffer()[(2, 9)].fg, theme.muted);
        for code in [
            KeyCode::Backspace,
            KeyCode::Backspace,
            KeyCode::Char('9'),
            KeyCode::Char('9'),
        ] {
            app.handle_key(KeyEvent::from(code));
        }
        let terminal = draw_app(&mut app, 70, 12);
        let rows = text_rows(terminal.backend().buffer());
        assert_eq!(
            rows[9].trim_end_matches([' ', '│']),
            "│ 17:99：沒有這個時間"
        );
        assert_eq!(terminal.backend().buffer()[(2, 9)].fg, theme.error);
    }

    #[test]
    fn the_bottom_line_says_what_is_being_moved() {
        let mut app = App::new(
            Lang::ZhTw,
            date(2026, 10, 17),
            crate::tasks::parse("- [ ] Send the weekly update @2026-10-16\n").items,
        );
        app.focus = Focus::Agenda;
        app.handle_key(ratatui::crossterm::event::KeyEvent::from(
            ratatui::crossterm::event::KeyCode::Char('r'),
        ));
        let rows = text_rows(draw_app(&mut app, 70, 11).backend().buffer());
        assert_eq!(
            rows[10],
            " 改期「Send the weekly update」：移到新的日期  Enter 確定  Esc 取消"
        );
    }

    #[test]
    fn a_notice_replaces_the_key_hints_until_the_next_key() {
        let mut app = App::new(Lang::ZhTw, date(2026, 10, 17), Vec::new());
        app.notice = Some(crate::app::Notice::NotATask);
        let terminal = draw_app(&mut app, 70, 11);
        let rows = text_rows(terminal.backend().buffer());
        assert_eq!(rows[10], " 行程沒有框，不能打勾");
        let cell = &terminal.backend().buffer()[(1, 10)];
        assert_eq!(cell.fg, Theme::catppuccin_mocha().notice);
    }

    #[test]
    fn colors_by_role() {
        let terminal = draw(Lang::ZhTw, 40, 26, date(2026, 10, 17));
        let buffer = terminal.backend().buffer();
        let theme = Theme::catppuccin_mocha();
        // The (x, y) cell: symbol, foreground color, whether it is bold
        let cell = |x: u16, y: u16| {
            let cell = &buffer[(x, y)];
            (
                cell.symbol(),
                cell.fg,
                cell.modifier.contains(Modifier::BOLD),
            )
        };
        // The key hints are muted
        assert_eq!(cell(1, 25), ("h", theme.muted, false));
        // Keyboard in the calendar: the calendar box's border uses the focus color
        assert_eq!(cell(0, 0), ("╭", theme.focus, false));
        // Header: weekdays muted, Sat and Sun in the weekend color, vertical line
        // The year in the top-left corner (2026年, shown even when January is on screen)
        assert_eq!(cell(5, 1), ("年", theme.muted, false));
        assert_eq!(cell(13, 1), ("一", theme.muted, false));
        assert_eq!(cell(28, 1), ("六", theme.weekend, false));
        assert_eq!(cell(31, 1), ("日", theme.weekend, false));
        assert_eq!(cell(11, 1), ("│", theme.line, false));
        // The line between months, the thick line between years
        assert_eq!(cell(11, 2), ("┼", theme.line, false));
        assert_eq!(cell(5, 21), ("━", theme.year_line, false));
        assert_eq!(cell(11, 21), ("┿", theme.year_line, false));
        // Month (10月), January's year (2027年), week numbers
        assert_eq!(cell(3, 3), ("1", theme.label, false));
        assert_eq!(cell(1, 22), ("2", theme.label, false));
        // Week numbers: the selected week (42) bright, other weeks (41) dim
        assert_eq!(cell(8, 5), ("4", theme.week_number, false));
        assert_eq!(cell(8, 4), ("4", theme.week_number_dim, false));
        // 10/16 (Fri) normal; 10/17 (Sat) is today: today's color wins
        // over the weekend color, and it is bold; 10/18 (Sun) weekend
        assert_eq!(cell(25, 5), ("1", theme.text, false));
        assert_eq!(cell(29, 5), ("7", theme.today, true));
        assert_eq!(cell(32, 5), ("8", theme.weekend, false));
        // The selection brackets
        assert_eq!(cell(27, 5), ("[", theme.selected, true));
        assert_eq!(cell(30, 5), ("]", theme.selected, true));
    }

    #[test]
    fn year_hint_in_the_header_is_muted() {
        // No January on screen: 2026 is shown left of the header's `Wk`,
        // as muted as the header (screen: see `calendar_in_english`)
        let terminal = draw(Lang::En, 40, 11, date(2026, 10, 17));
        let cell = &terminal.backend().buffer()[(3, 1)];
        assert_eq!(
            (cell.symbol(), cell.fg),
            ("2", Theme::catppuccin_mocha().muted)
        );
    }

    /// Task file for the multi-day tests (`tools/expected_screen.py`
    /// reads this same file for the expected screens)
    const RANGE_TASKS: &str = include_str!("../testdata/ranges.md");

    #[test]
    fn multi_day_items_are_listed_once_per_month() {
        // The agenda lists an item once on its start day, with the date as a range (`10/20~25`, across months
        // `10/28~11/3`, across years `12/30~1/2`); an item that crosses months is listed again at the top of
        // each later month; the dates are longer, so titles move right
        let mut app = App::new(
            Lang::ZhTw,
            date(2026, 10, 17),
            crate::tasks::parse(RANGE_TASKS).items,
        );
        let terminal = draw_app(&mut app, 70, 30);
        assert_eq!(
            text_rows(terminal.backend().buffer()),
            [
                "╭─────────────────────────────────╮╭─────────────────────────────────╮",
                "│2026年 週 │ 一 二 三 四 五 六 日 ││ 重要事項       2026/10/17（六） │",
                "│──────────┼──────────────────────││─────────────────────────────────│",
                "│  10月 40 │           1  2  3  4 ││ 10/16 17:30 Send the weekly upda│",
                "│       41 │  5  6  7  8  9 10 11 ││ 10/20~25 Business trip to NYC   │",
                "│       42 │ 12 13 14 15 16[17]18 ││ 10/22 Buy gifts for the team    │",
                "│       43 │ 19 20 21 22 23 24 25 ││ 10/28~11/3 Manager OOO          │",
                "│       44 │ 26 27 28 29 30 31    ││                                 │",
                "│──────────┼──────────────────────││─────────────────────────────────│",
                "│  11月 44 │                    1 ││ 10/28~11/3 Manager OOO          │",
                "│       45 │  2  3  4  5  6  7  8 ││  11/1 Prepare the monthly report│",
                "│       46 │  9 10 11 12 13 14 15 ││                                 │",
                "│       47 │ 16 17 18 19 20 21 22 ││                                 │",
                "│       48 │ 23 24 25 26 27 28 29 ││                                 │",
                "│       49 │ 30                   ││                                 │",
                "│──────────┼──────────────────────││─────────────────────────────────│",
                "│  12月 49 │     1  2  3  4  5  6 ││ 12/30~1/2 09:00 New Year trip   │",
                "│       50 │  7  8  9 10 11 12 13 ││                                 │",
                "│       51 │ 14 15 16 17 18 19 20 ││                                 │",
                "│       52 │ 21 22 23 24 25 26 27 ││                                 │",
                "│       53 │ 28 29 30 31          ││                                 │",
                "│━━━━━━━━━━┿━━━━━━━━━━━━━━━━━━━━━━││━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━│",
                "│2027年  1 │              1  2  3 ││ 12/30~1/2 09:00 New Year trip   │",
                "│   1月  2 │  4  5  6  7  8  9 10 ││                                 │",
                "│        3 │ 11 12 13 14 15 16 17 ││                                 │",
                "│        4 │ 18 19 20 21 22 23 24 ││                                 │",
                "│        5 │ 25 26 27 28 29 30 31 ││                                 │",
                "│──────────┼──────────────────────││─────────────────────────────────│",
                "╰─────────────────────────────────╯╰─────────────────────────────────╯",
                " hjkl 移動  Tab 切換  [] 換月  t 今天  s 日曆  ? 說明  , 設定  q 離開",
            ]
        );
        // Calendar: every day in a range is underlined (10/20-25, 10/28-11/3), the days just before and after
        // are not; colors do not change: the start day is orange, and days in the middle with no items of
        // their own (10/21, x 19-20) keep the normal color
        let buffer = terminal.backend().buffer();
        let underlined = |x: u16, y: u16| buffer[(x, y)].modifier.contains(Modifier::UNDERLINED);
        // 10/19 (Mon) x 13-14, 10/20 (Tue) x 16-17, 10/25
        // (Sun) x 31-32 are at y 6; 10/26 (Mon) and 10/28 (Wed) at y 7
        assert!(!underlined(14, 6));
        assert!(underlined(17, 6) && underlined(32, 6));
        assert!(!underlined(14, 7) && underlined(20, 7));
        let theme = Theme::catppuccin_mocha();
        assert_eq!(buffer[(17, 6)].fg, theme.has_item);
        assert_eq!(buffer[(20, 6)].fg, theme.text);
        // Keyboard in the agenda, Manager OOO chosen on November's line (Tab from 11/2): the brackets hold
        // the range; November is on top (the week of 11/2 is November's 2nd row and must stay 2 rows from
        // the top edge, so it scrolls up one more row and shows the line under October)
        let mut app = App::new(
            Lang::ZhTw,
            date(2026, 10, 17),
            crate::tasks::parse(RANGE_TASKS).items,
        );
        app.focus = Focus::Agenda;
        app.agenda_index = 4;
        app.selected = date(2026, 11, 2);
        assert_eq!(
            text_rows(draw_app(&mut app, 70, 30).backend().buffer()),
            [
                "╭─────────────────────────────────╮╭─────────────────────────────────╮",
                "│2026年 週 │ 一 二 三 四 五 六 日 ││ 重要事項        2026/11/2（一） │",
                "│──────────┼──────────────────────││─────────────────────────────────│",
                "│──────────┼──────────────────────││─────────────────────────────────│",
                "│  11月 44 │                    1 ││[10/28~11/3]Manager OOO          │",
                "│       45 │ [2] 3  4  5  6  7  8 ││  11/1 Prepare the monthly report│",
                "│       46 │  9 10 11 12 13 14 15 ││                                 │",
                "│       47 │ 16 17 18 19 20 21 22 ││                                 │",
                "│       48 │ 23 24 25 26 27 28 29 ││                                 │",
                "│       49 │ 30                   ││                                 │",
                "│──────────┼──────────────────────││─────────────────────────────────│",
                "│  12月 49 │     1  2  3  4  5  6 ││ 12/30~1/2 09:00 New Year trip   │",
                "│       50 │  7  8  9 10 11 12 13 ││                                 │",
                "│       51 │ 14 15 16 17 18 19 20 ││                                 │",
                "│       52 │ 21 22 23 24 25 26 27 ││                                 │",
                "│       53 │ 28 29 30 31          ││                                 │",
                "│━━━━━━━━━━┿━━━━━━━━━━━━━━━━━━━━━━││━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━│",
                "│2027年  1 │              1  2  3 ││ 12/30~1/2 09:00 New Year trip   │",
                "│   1月  2 │  4  5  6  7  8  9 10 ││                                 │",
                "│        3 │ 11 12 13 14 15 16 17 ││                                 │",
                "│        4 │ 18 19 20 21 22 23 24 ││                                 │",
                "│        5 │ 25 26 27 28 29 30 31 ││                                 │",
                "│──────────┼──────────────────────││─────────────────────────────────│",
                "│   2月  6 │  1  2  3  4  5  6  7 ││                                 │",
                "│        7 │  8  9 10 11 12 13 14 ││                                 │",
                "│        8 │ 15 16 17 18 19 20 21 ││                                 │",
                "│        9 │ 22 23 24 25 26 27 28 ││                                 │",
                "│──────────┼──────────────────────││─────────────────────────────────│",
                "╰─────────────────────────────────╯╰─────────────────────────────────╯",
                " hjkl 移動  Tab 切換  [] 換月  t 今天  s 日曆  ? 說明  , 設定  q 離開",
            ]
        );
    }

    #[test]
    fn a_multi_day_item_belongs_to_every_day_it_covers() {
        // Keyboard in the calendar, 10/22 selected: Business trip to NYC (10/20-25) and Buy gifts for the
        // team (10/22) both get the highlight background, Send the weekly update does not (screen: see above)
        let mut app = App::new(
            Lang::ZhTw,
            date(2026, 10, 17),
            crate::tasks::parse(RANGE_TASKS).items,
        );
        app.selected = date(2026, 10, 22);
        let terminal = draw_app(&mut app, 70, 30);
        let buffer = terminal.backend().buffer();
        let theme = Theme::catppuccin_mocha();
        assert_eq!(buffer[(40, 3)].bg, Color::Reset);
        assert_eq!(buffer[(40, 4)].bg, theme.cursor_line);
        assert_eq!(buffer[(40, 5)].bg, theme.cursor_line);
        assert_eq!(buffer[(40, 6)].bg, Color::Reset);
    }

    #[test]
    fn holidays_are_red() {
        // US holidays (type:holiday): Long weekend 10/10-12, Halloween 10/31;
        // 10/31 also has an item in Work, and the holiday red wins over orange
        let text = "# US holidays type:holiday\n- Long weekend @2026-10-10..2026-10-12\n\
                    - Halloween @2026-10-31\n# Work\n- [ ] On-call shift @2026-10-31\n";
        let mut app = App::new(
            Lang::ZhTw,
            date(2026, 10, 17),
            crate::tasks::parse(text).items,
        );
        let theme = Theme::catppuccin_mocha();
        let fg = |app: &mut App, x: u16, y: u16| {
            let terminal = draw_app(app, 70, 23);
            let cell = &terminal.backend().buffer()[(x, y)];
            (cell.symbol().to_string(), cell.fg)
        };
        // 10/10 (Sat) x 29, 10/11 (Sun) x 32 at y 4; 10/12 (Mon) x 14 at y 5;
        // 10/31 (Sat) x 29 at y 7; 10/9 (Fri) x 26 at y 4 is not a holiday
        assert_eq!(fg(&mut app, 29, 4), ("0".into(), theme.holiday));
        assert_eq!(fg(&mut app, 32, 4), ("1".into(), theme.holiday));
        assert_eq!(fg(&mut app, 14, 5), ("2".into(), theme.holiday));
        assert_eq!(fg(&mut app, 29, 7), ("1".into(), theme.holiday));
        assert_eq!(fg(&mut app, 26, 4), ("9".into(), theme.text));
        // If today is a holiday, today's green comes first
        app.today = date(2026, 10, 10);
        assert_eq!(fg(&mut app, 29, 4), ("0".into(), theme.today));
        app.today = date(2026, 10, 17);
        // The name in the side menu leaves out type:holiday; unchecked: no longer holidays (10/31 back to the
        // has-items orange, 10/10 back to weekend blue)
        assert_eq!(app.groups[0].name.as_deref(), Some("US holidays"));
        app.groups[0].shown = false;
        assert_eq!(fg(&mut app, 29, 7), ("1".into(), theme.has_item));
        assert_eq!(fg(&mut app, 29, 4), ("0".into(), theme.weekend));
    }

    #[test]
    fn events_gray_out_once_their_day_is_past() {
        // 10/9: an overdue unfinished task (orange); 10/10: a past event (gray, not struck through, and the
        // day goes back to weekend blue); 10/18: an upcoming event (orange)
        let text = "- [ ] Send the weekly update @2026-10-09\n- Farmers market @2026-10-10\n\
                    - Go hiking @2026-10-18 07:00\n";
        let mut app = App::new(
            Lang::ZhTw,
            date(2026, 10, 17),
            crate::tasks::parse(text).items,
        );
        let terminal = draw_app(&mut app, 70, 23);
        let buffer = terminal.backend().buffer();
        let theme = Theme::catppuccin_mocha();
        let cell = |x: u16, y: u16| {
            let cell = &buffer[(x, y)];
            (
                cell.symbol(),
                cell.fg,
                cell.modifier.contains(Modifier::CROSSED_OUT),
            )
        };
        // Calendar: 10/9 (Fri) x 26, 10/10 (Sat) x 29 at y 4; 10/18 (Sun) x 32 at y 5
        assert_eq!(cell(26, 4), ("9", theme.has_item, false));
        assert_eq!(cell(29, 4), ("0", theme.weekend, false));
        assert_eq!(cell(32, 5), ("8", theme.has_item, false));
        // Agenda (starts at x 36, one item per row): titles
        // at x 43; Go hiking has a time in front, so it is at x 49
        assert_eq!(cell(43, 3), ("S", theme.has_item, false));
        assert_eq!(cell(43, 4), ("F", theme.muted, false));
        assert_eq!(cell(38, 4), ("0", theme.muted, false));
        assert_eq!(cell(49, 5), ("G", theme.has_item, false));
    }

    #[test]
    fn days_with_open_items_are_orange() {
        // The 16th has an unfinished item → orange; the 15th has only finished ones → same as no items; the
        // 17th (today) has items → still today's color; the 18th (Sun) has items → orange wins over the
        // weekend color; the 14th has no items
        let text = "- [ ] Send the weekly update @2026-10-16\n- [x] Fix the login bug @2026-10-15\n\
                    - Dinner with friends @2026-10-17\n- [ ] Go hiking @2026-10-18 07:00\n";
        let mut app = App::new(
            Lang::ZhTw,
            date(2026, 10, 17),
            crate::tasks::parse(text).items,
        );
        let terminal = draw_app(&mut app, 40, 26);
        let buffer = terminal.backend().buffer();
        let theme = Theme::catppuccin_mocha();
        let fg = |x: u16| (buffer[(x, 5)].symbol(), buffer[(x, 5)].fg);
        assert_eq!(fg(20), ("4", theme.text));
        assert_eq!(fg(23), ("5", theme.text));
        assert_eq!(fg(26), ("6", theme.has_item));
        assert_eq!(fg(29), ("7", theme.today));
        assert_eq!(fg(32), ("8", theme.has_item));
    }

    /// An app in layout 3 with today 2026/10/17, `selected` selected, and `text` as the task file
    fn month_app(lang: Lang, selected: Date, text: &str) -> App {
        let mut app = App::new(lang, date(2026, 10, 17), crate::tasks::parse(text).items);
        app.view = View::Month;
        app.selected = selected;
        app
    }

    #[test]
    fn the_agenda_header_keeps_the_whole_date_when_narrow() {
        // Chinese, with a very narrow agenda: the date stays whole, 「重要事項」 drops each character that
        // does not fit, with at least one space between (56 cells: the date's 2 is not eaten; 57 cells: there
        // is a space between 「重」 and the date)
        let terminal = draw(Lang::ZhTw, 56, 10, date(2026, 10, 17));
        assert_eq!(
            text_rows(terminal.backend().buffer()),
            [
                "╭─────────────────────────────────╮╭───────────────────╮",
                "│2026年 週 │ 一 二 三 四 五 六 日 ││  2026/10/17（六） │",
                "│──────────┼──────────────────────││───────────────────│",
                "│  10月 40 │           1  2  3  4 ││                   │",
                "│       41 │  5  6  7  8  9 10 11 ││                   │",
                "│       42 │ 12 13 14 15 16[17]18 ││                   │",
                "│       43 │ 19 20 21 22 23 24 25 ││                   │",
                "│       44 │ 26 27 28 29 30 31    ││                   │",
                "╰─────────────────────────────────╯╰───────────────────╯",
                " hjkl 移動  Tab 切換  [] 換月  t 今天  s 日曆  ? 說明  ,",
            ]
        );
        let terminal = draw(Lang::ZhTw, 57, 10, date(2026, 10, 17));
        assert_eq!(
            text_rows(terminal.backend().buffer()),
            [
                "╭─────────────────────────────────╮╭────────────────────╮",
                "│2026年 週 │ 一 二 三 四 五 六 日 ││   2026/10/17（六） │",
                "│──────────┼──────────────────────││────────────────────│",
                "│  10月 40 │           1  2  3  4 ││                    │",
                "│       41 │  5  6  7  8  9 10 11 ││                    │",
                "│       42 │ 12 13 14 15 16[17]18 ││                    │",
                "│       43 │ 19 20 21 22 23 24 25 ││                    │",
                "│       44 │ 26 27 28 29 30 31    ││                    │",
                "╰─────────────────────────────────╯╰────────────────────╯",
                " hjkl 移動  Tab 切換  [] 換月  t 今天  s 日曆  ? 說明  ,",
            ]
        );
    }

    #[test]
    fn month_view_fills_the_space_with_six_equal_weeks() {
        // 80 × 30: inside the box 78 × 27 → day width 78 − 5 − 7 = 66, Mon-Wed 10 cells, the others 9; each
        // week (27 − 7) ÷ 6 = 3 rows, the 2 leftover rows stay at the bottom (vertical lines still reach the
        // bottom border). The top is the week before 10/1's week (9/21); the month follows the 1st; today,
        // which is selected, gets brackets; the top-right corner shows the selected day; a week that crosses
        // months takes its week number from the selected month's days (the 9/28 week shows 10/1's 40)
        let mut app = month_app(Lang::ZhTw, date(2026, 10, 17), "");
        assert_eq!(
            text_rows(draw_app(&mut app, 80, 30).backend().buffer()),
            [
                "╭────────────────────────────── 2026/10/17（六） ──────────────────────────────╮",
                "│  週   一         二         三         四        五        六        日      │",
                "├─────┬──────────┬──────────┬──────────┬─────────┬─────────┬─────────┬─────────┤",
                "│  40 │ 28       │ 29       │ 30       │         │         │         │         │",
                "│     │          │          │          │         │         │         │         │",
                "│     │          │          │          │         │         │         │         │",
                "├─────┼──────────┼──────────┼──────────┼─────────┼─────────┼─────────┼─────────┤",
                "│  40 │          │          │          │ 1 10月  │ 2       │ 3       │ 4       │",
                "│     │          │          │          │         │         │         │         │",
                "│     │          │          │          │         │         │         │         │",
                "├─────┼──────────┼──────────┼──────────┼─────────┼─────────┼─────────┼─────────┤",
                "│  41 │ 5        │ 6        │ 7        │ 8       │ 9       │ 10      │ 11      │",
                "│     │          │          │          │         │         │         │         │",
                "│     │          │          │          │         │         │         │         │",
                "├─────┼──────────┼──────────┼──────────┼─────────┼─────────┼─────────┼─────────┤",
                "│  42 │ 12       │ 13       │ 14       │ 15      │ 16      │ 17      │ 18      │",
                "│     │          │          │          │         │         │         │         │",
                "│     │          │          │          │         │         │         │         │",
                "├─────┼──────────┼──────────┼──────────┼─────────┼─────────┼─────────┼─────────┤",
                "│  43 │ 19       │ 20       │ 21       │ 22      │ 23      │ 24      │ 25      │",
                "│     │          │          │          │         │         │         │         │",
                "│     │          │          │          │         │         │         │         │",
                "├─────┼──────────┼──────────┼──────────┼─────────┼─────────┼─────────┼─────────┤",
                "│  44 │ 26       │ 27       │ 28       │ 29      │ 30      │ 31      │         │",
                "│     │          │          │          │         │         │         │         │",
                "│     │          │          │          │         │         │         │         │",
                "│     │          │          │          │         │         │         │         │",
                "│     │          │          │          │         │         │         │         │",
                "╰─────┴──────────┴──────────┴──────────┴─────────┴─────────┴─────────┴─────────╯",
                " hjkl 移動  Tab 切換  [] 換月  t 今天  s 日曆  ? 說明  , 設定  q 離開",
            ]
        );
        // 80 × 28: 3 rows per week fill it exactly, and the last week touches the bottom border (joined as ┴)
        let mut app = month_app(Lang::En, date(2026, 10, 17), "");
        assert_eq!(
            text_rows(draw_app(&mut app, 80, 28).backend().buffer()),
            [
                "╭────────────────────────────── Sat Oct 17, 2026 ──────────────────────────────╮",
                "│  Wk   Mo         Tu         We         Th        Fr        Sa        Su      │",
                "├─────┬──────────┬──────────┬──────────┬─────────┬─────────┬─────────┬─────────┤",
                "│  40 │ 28       │ 29       │ 30       │         │         │         │         │",
                "│     │          │          │          │         │         │         │         │",
                "│     │          │          │          │         │         │         │         │",
                "├─────┼──────────┼──────────┼──────────┼─────────┼─────────┼─────────┼─────────┤",
                "│  40 │          │          │          │ 1 Oct   │ 2       │ 3       │ 4       │",
                "│     │          │          │          │         │         │         │         │",
                "│     │          │          │          │         │         │         │         │",
                "├─────┼──────────┼──────────┼──────────┼─────────┼─────────┼─────────┼─────────┤",
                "│  41 │ 5        │ 6        │ 7        │ 8       │ 9       │ 10      │ 11      │",
                "│     │          │          │          │         │         │         │         │",
                "│     │          │          │          │         │         │         │         │",
                "├─────┼──────────┼──────────┼──────────┼─────────┼─────────┼─────────┼─────────┤",
                "│  42 │ 12       │ 13       │ 14       │ 15      │ 16      │ 17      │ 18      │",
                "│     │          │          │          │         │         │         │         │",
                "│     │          │          │          │         │         │         │         │",
                "├─────┼──────────┼──────────┼──────────┼─────────┼─────────┼─────────┼─────────┤",
                "│  43 │ 19       │ 20       │ 21       │ 22      │ 23      │ 24      │ 25      │",
                "│     │          │          │          │         │         │         │         │",
                "│     │          │          │          │         │         │         │         │",
                "├─────┼──────────┼──────────┼──────────┼─────────┼─────────┼─────────┼─────────┤",
                "│  44 │ 26       │ 27       │ 28       │ 29      │ 30      │ 31      │         │",
                "│     │          │          │          │         │         │         │         │",
                "│     │          │          │          │         │         │         │         │",
                "╰─────┴──────────┴──────────┴──────────┴─────────┴─────────┴─────────┴─────────╯",
                " hjkl move  Tab pane  [] month  t today  s calendars  ? help  , settings  q quit",
            ]
        );
    }

    /// Task file for the month grid tests (`tools/expected_screen.py`
    /// reads this same file for the expected screens)
    const MONTH_TASKS: &str = include_str!("../testdata/month.md");

    #[test]
    fn month_view_lists_each_days_items_in_its_cell() {
        // 80 × 30, 3 rows per week: 2 items listed under the date; on the same day, items
        // without a time first (like layout 1); 10/16 has 4 items that do not fit: 1 is listed
        // and the last line says 「+3」; items on other months' days (9/25, 11/1) are listed
        // too. A title that is too long borrows the next cells to the right (only where that
        // line is empty, covering the grid line between) and is cut where it can't: 10/13
        // borrows 10/14 and stops before 10/15; 10/12's neighbor has items, so it can't borrow
        let mut app = month_app(Lang::ZhTw, date(2026, 10, 17), MONTH_TASKS);
        assert_eq!(
            text_rows(draw_app(&mut app, 80, 30).backend().buffer()),
            [
                "╭────────────────────────────── 2026/10/17（六） ──────────────────────────────╮",
                "│  週   一         二         三         四        五        六        日      │",
                "├─────┬──────────┬──────────┬──────────┬─────────┬─────────┬─────────┬─────────┤",
                "│  40 │ 28       │ 29       │ 30       │         │         │         │         │",
                "│     │          │ ∙ Backyard BBQ      │         │         │         │         │",
                "│     │          │          │          │         │         │         │         │",
                "├─────┼──────────┼──────────┼──────────┼─────────┼─────────┼─────────┼─────────┤",
                "│  40 │          │          │          │ 1 10月  │ 2       │ 3       │ 4       │",
                "│     │          │          │          │         │         │         │         │",
                "│     │          │          │          │         │         │         │         │",
                "├─────┼──────────┼──────────┼──────────┼─────────┼─────────┼─────────┼─────────┤",
                "│  41 │ 5        │ 6        │ 7        │ 8       │ 9       │ 10      │ 11      │",
                "│     │ ∙ Pay the phone bill│          │         │         │ ∙ Farmers market  │",
                "│     │          │          │          │         │         │         │         │",
                "├─────┼──────────┼──────────┼──────────┼─────────┼─────────┼─────────┼─────────┤",
                "│  42 │ 12       │ 13       │ 14       │ 15      │ 16      │ 17      │ 18      │",
                "│     │ ∙ 14:00 C│ ∙ 15:00 Confirm aler│ ∙ Fix th│ ∙ Clean │ ∙ Dinner│ ∙ 07:00 │",
                "│     │          │          │          │         │ +3      │ ∙ 08:00 Coffee cha│",
                "├─────┼──────────┼──────────┼──────────┼─────────┼─────────┼─────────┼─────────┤",
                "│  43 │ 19       │ 20       │ 21       │ 22      │ 23      │ 24      │ 25      │",
                "│     │          │ ∙ A really long title that spills over  │         │         │",
                "│     │          │          │          │         │         │         │         │",
                "├─────┼──────────┼──────────┼──────────┼─────────┼─────────┼─────────┼─────────┤",
                "│  44 │ 26       │ 27       │ 28       │ 29      │ 30      │ 31      │         │",
                "│     │          │          │          │         │         │         │         │",
                "│     │          │          │          │         │         │         │         │",
                "│     │          │          │          │         │         │         │         │",
                "│     │          │          │          │         │         │         │         │",
                "╰─────┴──────────┴──────────┴──────────┴─────────┴─────────┴─────────┴─────────╯",
                " hjkl 移動  Tab 切換  [] 換月  t 今天  s 日曆  ? 說明  , 設定  q 離開",
            ]
        );
        // 80 × 24, 2 rows per week: only 1 item fits; with 2 or more it just shows 「+N」
        let mut app = month_app(Lang::En, date(2026, 10, 17), MONTH_TASKS);
        assert_eq!(
            text_rows(draw_app(&mut app, 80, 24).backend().buffer()),
            [
                "╭────────────────────────────── Sat Oct 17, 2026 ──────────────────────────────╮",
                "│  Wk   Mo         Tu         We         Th        Fr        Sa        Su      │",
                "├─────┬──────────┬──────────┬──────────┬─────────┬─────────┬─────────┬─────────┤",
                "│  40 │ 28       │ 29       │ 30       │         │         │         │         │",
                "│     │          │ ∙ Backyard BBQ      │         │         │         │         │",
                "├─────┼──────────┼──────────┼──────────┼─────────┼─────────┼─────────┼─────────┤",
                "│  40 │          │          │          │ 1 Oct   │ 2       │ 3       │ 4       │",
                "│     │          │          │          │         │         │         │         │",
                "├─────┼──────────┼──────────┼──────────┼─────────┼─────────┼─────────┼─────────┤",
                "│  41 │ 5        │ 6        │ 7        │ 8       │ 9       │ 10      │ 11      │",
                "│     │ ∙ Pay the phone bill│          │         │         │ ∙ Farmers market  │",
                "├─────┼──────────┼──────────┼──────────┼─────────┼─────────┼─────────┼─────────┤",
                "│  42 │ 12       │ 13       │ 14       │ 15      │ 16      │ 17      │ 18      │",
                "│     │ ∙ 14:00 C│ ∙ 15:00 Confirm aler│ ∙ Fix th│ +4      │ +2      │ ∙ 07:00 │",
                "├─────┼──────────┼──────────┼──────────┼─────────┼─────────┼─────────┼─────────┤",
                "│  43 │ 19       │ 20       │ 21       │ 22      │ 23      │ 24      │ 25      │",
                "│     │          │ ∙ A really long title that spills over  │         │         │",
                "├─────┼──────────┼──────────┼──────────┼─────────┼─────────┼─────────┼─────────┤",
                "│  44 │ 26       │ 27       │ 28       │ 29      │ 30      │ 31      │         │",
                "│     │          │          │          │         │         │         │         │",
                "│     │          │          │          │         │         │         │         │",
                "│     │          │          │          │         │         │         │         │",
                "╰─────┴──────────┴──────────┴──────────┴─────────┴─────────┴─────────┴─────────╯",
                " hjkl move  Tab pane  [] month  t today  s calendars  ? help  , settings  q quit",
            ]
        );
        // 80 × 16, only 1 row per week: no items listed, dots after
        // the date instead (1 item ⠂, 2 items ⠒, 4 or more ⠶)
        let mut app = month_app(Lang::ZhTw, date(2026, 10, 17), MONTH_TASKS);
        assert_eq!(
            text_rows(draw_app(&mut app, 80, 16).backend().buffer()),
            [
                "╭────────────────────────────── 2026/10/17（六） ──────────────────────────────╮",
                "│  週   一         二         三         四        五        六        日      │",
                "├─────┬──────────┬──────────┬──────────┬─────────┬─────────┬─────────┬─────────┤",
                "│  40 │ 28       │ 29 ⠂     │ 30       │         │         │         │         │",
                "├─────┼──────────┼──────────┼──────────┼─────────┼─────────┼─────────┼─────────┤",
                "│  40 │          │          │          │ 1 10月  │ 2       │ 3       │ 4       │",
                "├─────┼──────────┼──────────┼──────────┼─────────┼─────────┼─────────┼─────────┤",
                "│  41 │ 5 ⠂      │ 6        │ 7        │ 8       │ 9       │ 10 ⠂    │ 11      │",
                "├─────┼──────────┼──────────┼──────────┼─────────┼─────────┼─────────┼─────────┤",
                "│  42 │ 12 ⠂     │ 13 ⠂     │ 14       │ 15 ⠂    │ 16 ⠶    │ 17 ⠒    │ 18 ⠂    │",
                "├─────┼──────────┼──────────┼──────────┼─────────┼─────────┼─────────┼─────────┤",
                "│  43 │ 19       │ 20 ⠂     │ 21       │ 22      │ 23      │ 24      │ 25      │",
                "├─────┼──────────┼──────────┼──────────┼─────────┼─────────┼─────────┼─────────┤",
                "│  44 │ 26       │ 27       │ 28       │ 29      │ 30      │ 31      │         │",
                "╰─────┴──────────┴──────────┴──────────┴─────────┴─────────┴─────────┴─────────╯",
                " hjkl 移動  Tab 切換  [] 換月  t 今天  s 日曆  ? 說明  , 設定  q 離開",
            ]
        );
    }

    #[test]
    fn month_view_item_colors() {
        // 80 × 30 (screen: see `month_view_lists_each_days_items_in_its_cell`): cells start at x 7, 18, 29,
        // 40, 50, 60, 70; weeks 1 and 4 are at y 3 and 15
        let mut app = month_app(Lang::ZhTw, date(2026, 10, 17), MONTH_TASKS);
        let terminal = draw_app(&mut app, 80, 30);
        let buffer = terminal.backend().buffer();
        let theme = Theme::catppuccin_mocha();
        // The (x, y) cell: symbol, foreground color, whether it is struck through
        let cell = |x: u16, y: u16| {
            let cell = &buffer[(x, y)];
            (
                cell.symbol(),
                cell.fg,
                cell.modifier.contains(Modifier::CROSSED_OUT),
            )
        };
        // 10/12 (Mon) unfinished item: dot and title orange, time muted
        assert_eq!(cell(8, 16), ("∙", theme.has_item, false));
        assert_eq!(cell(10, 16), ("1", theme.muted, false));
        // 10/15 (Thu) finished item: whole line gray, title struck through (the dot is not)
        assert_eq!(cell(41, 16), ("∙", theme.muted, false));
        assert_eq!(cell(43, 16), ("F", theme.muted, true));
        // 10/17 (Sat) today: green
        assert_eq!(cell(63, 16), ("D", theme.today, false));
        // 10/16 (Fri) overflows: 「+3」 is muted
        assert_eq!(cell(52, 17), ("3", theme.muted, false));
        // 9/29 (Tue, in the top row 9/28-30) is in another month: whole line gray
        assert_eq!(cell(19, 4), ("∙", theme.other_month, false));
        assert_eq!(cell(21, 4), ("B", theme.other_month, false));
        // 10/20 (Tue)'s long title borrows cells up to 10/23 (Fri): text over the grid lines keeps its own
        // color (x 49 was a vertical line); the grid lines after the text keep the grid color
        assert_eq!(cell(49, 20), ("l", theme.has_item, false));
        assert_eq!(buffer[(59, 20)].symbol(), "│");
        assert_eq!(buffer[(59, 20)].fg, theme.grid);
        // Only 1 row per week: the dots follow the day's color (10/16 orange); all done (10/15) is muted
        let terminal = draw_app(&mut app, 80, 16);
        let buffer = terminal.backend().buffer();
        // 80 × 16: week 4 is at y 9; 10/16 is 「 16 ⠶」, 10/15 is 「 15 ⠂」
        assert_eq!(
            (buffer[(54, 9)].symbol(), buffer[(54, 9)].fg),
            ("⠶", theme.has_item)
        );
        assert_eq!(
            (buffer[(44, 9)].symbol(), buffer[(44, 9)].fg),
            ("⠂", theme.muted)
        );
    }

    #[test]
    fn month_view_with_the_side_menu_hides_unchecked_calendars() {
        // 121 cells wide: the side menu on the far left, the month grid takes all the rest
        let mut app = month_app(Lang::ZhTw, date(2026, 10, 17), MONTH_TASKS);
        let terminal = draw_app(&mut app, 121, 22);
        assert_eq!(
            text_rows(terminal.backend().buffer()),
            [
                "╭────────────────────╮╭─────────────────────────────────────── 2026/10/17（六） ────────────────────────────────────────╮",
                "│ 日曆               ││  週   一            二           三           四           五           六           日         │",
                "│────────────────────│├─────┬─────────────┬────────────┬────────────┬────────────┬────────────┬────────────┬────────────┤",
                "│ ☑ 未分組         7 ││  40 │ 28          │ 29         │ 30         │            │            │            │            │",
                "│ ☑ Personal       8 ││     │             │ ∙ Backyard BBQ          │            │            │            │            │",
                "│                    │├─────┼─────────────┼────────────┼────────────┼────────────┼────────────┼────────────┼────────────┤",
                "│                    ││  40 │             │            │            │ 1 10月     │ 2          │ 3          │ 4          │",
                "│                    ││     │             │            │            │            │            │            │            │",
                "│                    │├─────┼─────────────┼────────────┼────────────┼────────────┼────────────┼────────────┼────────────┤",
                "│                    ││  41 │ 5           │ 6          │ 7          │ 8          │ 9          │ 10         │ 11         │",
                "│                    ││     │ ∙ Pay the phone bill     │            │            │            │ ∙ Farmers market        │",
                "│                    │├─────┼─────────────┼────────────┼────────────┼────────────┼────────────┼────────────┼────────────┤",
                "│                    ││  42 │ 12          │ 13         │ 14         │ 15         │ 16         │ 17         │ 18         │",
                "│                    ││     │ ∙ 14:00 Code│ ∙ 15:00 Confirm alerting│ ∙ Fix the l│ +4         │ +2         │ ∙ 07:00 Go │",
                "│                    │├─────┼─────────────┼────────────┼────────────┼────────────┼────────────┼────────────┼────────────┤",
                "│                    ││  43 │ 19          │ 20         │ 21         │ 22         │ 23         │ 24         │ 25         │",
                "│                    ││     │             │ ∙ A really long title that spills over            │            │            │",
                "│                    │├─────┼─────────────┼────────────┼────────────┼────────────┼────────────┼────────────┼────────────┤",
                "│                    ││  44 │ 26          │ 27         │ 28         │ 29         │ 30         │ 31         │            │",
                "│                    ││     │             │            │            │            │            │            │            │",
                "╰────────────────────╯╰─────┴─────────────┴────────────┴────────────┴────────────┴────────────┴────────────┴────────────╯",
                " hjkl 移動  Tab 切換  [] 換月  t 今天  s 日曆  ? 說明  , 設定  q 離開",
            ]
        );
        // Personal unchecked: its items leave the cells, and those
        // days are no longer orange (10/18 back to weekend blue)
        app.groups[1].shown = false;
        let terminal = draw_app(&mut app, 121, 22);
        assert_eq!(
            text_rows(terminal.backend().buffer()),
            [
                "╭────────────────────╮╭─────────────────────────────────────── 2026/10/17（六） ────────────────────────────────────────╮",
                "│ 日曆               ││  週   一            二           三           四           五           六           日         │",
                "│────────────────────│├─────┬─────────────┬────────────┬────────────┬────────────┬────────────┬────────────┬────────────┤",
                "│ ☑ 未分組         7 ││  40 │ 28          │ 29         │ 30         │            │            │            │            │",
                "│ ☐ Personal       8 ││     │             │            │            │            │            │            │            │",
                "│                    │├─────┼─────────────┼────────────┼────────────┼────────────┼────────────┼────────────┼────────────┤",
                "│                    ││  40 │             │            │            │ 1 10月     │ 2          │ 3          │ 4          │",
                "│                    ││     │             │            │            │            │            │            │            │",
                "│                    │├─────┼─────────────┼────────────┼────────────┼────────────┼────────────┼────────────┼────────────┤",
                "│                    ││  41 │ 5           │ 6          │ 7          │ 8          │ 9          │ 10         │ 11         │",
                "│                    ││     │             │            │            │            │            │            │            │",
                "│                    │├─────┼─────────────┼────────────┼────────────┼────────────┼────────────┼────────────┼────────────┤",
                "│                    ││  42 │ 12          │ 13         │ 14         │ 15         │ 16         │ 17         │ 18         │",
                "│                    ││     │ ∙ 14:00 Code│ ∙ 15:00 Confirm alerting│ ∙ Fix the l│ +4         │            │            │",
                "│                    │├─────┼─────────────┼────────────┼────────────┼────────────┼────────────┼────────────┼────────────┤",
                "│                    ││  43 │ 19          │ 20         │ 21         │ 22         │ 23         │ 24         │ 25         │",
                "│                    ││     │             │            │            │            │            │            │            │",
                "│                    │├─────┼─────────────┼────────────┼────────────┼────────────┼────────────┼────────────┼────────────┤",
                "│                    ││  44 │ 26          │ 27         │ 28         │ 29         │ 30         │ 31         │            │",
                "│                    ││     │             │            │            │            │            │            │            │",
                "╰────────────────────╯╰─────┴─────────────┴────────────┴────────────┴────────────┴────────────┴────────────┴────────────╯",
                " hjkl 移動  Tab 切換  [] 換月  t 今天  s 日曆  ? 說明  , 設定  q 離開",
            ]
        );
        // 10/18 (Sun): the month grid starts at x 22, the Sunday column at x 108, week 4 at y 12 (2 rows per
        // week); the 「1」 of 「 18」 is at x 109
        let theme = Theme::catppuccin_mocha();
        let cell = &terminal.backend().buffer()[(109, 12)];
        assert_eq!((cell.symbol(), cell.fg), ("1", theme.weekend));
        // Checked again: orange
        app.groups[1].shown = true;
        let terminal = draw_app(&mut app, 121, 22);
        let cell = &terminal.backend().buffer()[(109, 12)];
        assert_eq!((cell.symbol(), cell.fg), ("1", theme.has_item));
    }

    #[test]
    fn month_view_without_grid_lines_gives_the_cells_more_room() {
        // Config `[month] grid_lines = false`: no grid lines, the cells touch,
        // and no line under the header either; 80 × 24: day width 78 − 5 = 73
        // (Mon-Wed 11 cells, the others 10), each week (21 − 1) ÷ 6 = 3 rows
        let mut app = month_app(Lang::ZhTw, date(2026, 10, 17), MONTH_TASKS);
        app.settings.month.grid_lines = false;
        app.screen = Rect::new(0, 0, 80, 24);
        assert_eq!(
            text_rows(draw_app(&mut app, 80, 24).backend().buffer()),
            [
                "╭────────────────────────────── 2026/10/17（六） ──────────────────────────────╮",
                "│  週  一         二         三         四        五        六        日       │",
                "│  40  28         29         30                                                │",
                "│                 ∙ Backyard BBQ                                               │",
                "│                                                                              │",
                "│  40                                   1 10月    2         3         4        │",
                "│                                                                              │",
                "│                                                                              │",
                "│  41  5          6          7          8         9         10        11       │",
                "│      ∙ Pay the phone bill                                 ∙ Farmers market   │",
                "│                                                                              │",
                "│  42  12         13         14         15        16        17        18       │",
                "│      ∙ 14:00 Co ∙ 15:00 Confirm alert ∙ Fix the ∙ Clean u ∙ Dinner  ∙ 07:00 G│",
                "│                                                 +3        ∙ 08:00 Coffee chat│",
                "│  43  19         20         21         22        23        24        25       │",
                "│                 ∙ A really long title that spills over                       │",
                "│                                                                              │",
                "│  44  26         27         28         29        30        31                 │",
                "│                                                                              │",
                "│                                                                              │",
                "│                                                                              │",
                "│                                                                              │",
                "╰──────────────────────────────────────────────────────────────────────────────╯",
                " hjkl 移動  Tab 切換  [] 換月  t 今天  s 日曆  ? 說明  , 設定  q 離開",
            ]
        );
        // Mouse: the cells without grid lines can be clicked too.
        // Tuesday is x 17-27, week 3 (the week of 10/5) is y 8-10
        assert_eq!(click_at(&app, 17, 8), Some(Click::Day(date(2026, 10, 6))));
        assert_eq!(click_at(&app, 27, 10), Some(Click::Day(date(2026, 10, 6))));
        assert_eq!(click_at(&app, 28, 10), Some(Click::Day(date(2026, 10, 7))));
    }

    #[test]
    fn month_view_without_item_icons() {
        // Config `[month] item_icons = false`: no dot before the items in the cells, just one space
        let mut app = month_app(Lang::En, date(2026, 10, 17), MONTH_TASKS);
        app.settings.month.item_icons = false;
        assert_eq!(
            text_rows(draw_app(&mut app, 80, 24).backend().buffer()),
            [
                "╭────────────────────────────── Sat Oct 17, 2026 ──────────────────────────────╮",
                "│  Wk   Mo         Tu         We         Th        Fr        Sa        Su      │",
                "├─────┬──────────┬──────────┬──────────┬─────────┬─────────┬─────────┬─────────┤",
                "│  40 │ 28       │ 29       │ 30       │         │         │         │         │",
                "│     │          │ Backyard BBQ        │         │         │         │         │",
                "├─────┼──────────┼──────────┼──────────┼─────────┼─────────┼─────────┼─────────┤",
                "│  40 │          │          │          │ 1 Oct   │ 2       │ 3       │ 4       │",
                "│     │          │          │          │         │         │         │         │",
                "├─────┼──────────┼──────────┼──────────┼─────────┼─────────┼─────────┼─────────┤",
                "│  41 │ 5        │ 6        │ 7        │ 8       │ 9       │ 10      │ 11      │",
                "│     │ Pay the phone bill  │          │         │         │ Farmers market    │",
                "├─────┼──────────┼──────────┼──────────┼─────────┼─────────┼─────────┼─────────┤",
                "│  42 │ 12       │ 13       │ 14       │ 15      │ 16      │ 17      │ 18      │",
                "│     │ 14:00 Cod│ 15:00 Confirm alerti│ Fix the │ +4      │ +2      │ 07:00 Go│",
                "├─────┼──────────┼──────────┼──────────┼─────────┼─────────┼─────────┼─────────┤",
                "│  43 │ 19       │ 20       │ 21       │ 22      │ 23      │ 24      │ 25      │",
                "│     │          │ A really long title that spills over    │         │         │",
                "├─────┼──────────┼──────────┼──────────┼─────────┼─────────┼─────────┼─────────┤",
                "│  44 │ 26       │ 27       │ 28       │ 29      │ 30      │ 31      │         │",
                "│     │          │          │          │         │         │         │         │",
                "│     │          │          │          │         │         │         │         │",
                "│     │          │          │          │         │         │         │         │",
                "╰─────┴──────────┴──────────┴──────────┴─────────┴─────────┴─────────┴─────────╯",
                " hjkl move  Tab pane  [] month  t today  s calendars  ? help  , settings  q quit",
            ]
        );
    }

    #[test]
    fn month_view_keeps_the_selected_week_in_the_middle() {
        // Config `[month] center_cursor = true`: the selected week stays on row 3 (also right after opening,
        // instead of the week before the 1st on top)
        let mut app = month_app(Lang::ZhTw, date(2026, 10, 17), "");
        app.settings.month.center_cursor = true;
        assert_eq!(
            text_rows(draw_app(&mut app, 80, 16).backend().buffer()),
            [
                "╭────────────────────────────── 2026/10/17（六） ──────────────────────────────╮",
                "│  週   一         二         三         四        五        六        日      │",
                "├─────┬──────────┬──────────┬──────────┬─────────┬─────────┬─────────┬─────────┤",
                "│  40 │          │          │          │ 1 10月  │ 2       │ 3       │ 4       │",
                "├─────┼──────────┼──────────┼──────────┼─────────┼─────────┼─────────┼─────────┤",
                "│  41 │ 5        │ 6        │ 7        │ 8       │ 9       │ 10      │ 11      │",
                "├─────┼──────────┼──────────┼──────────┼─────────┼─────────┼─────────┼─────────┤",
                "│  42 │ 12       │ 13       │ 14       │ 15      │ 16      │ 17      │ 18      │",
                "├─────┼──────────┼──────────┼──────────┼─────────┼─────────┼─────────┼─────────┤",
                "│  43 │ 19       │ 20       │ 21       │ 22      │ 23      │ 24      │ 25      │",
                "├─────┼──────────┼──────────┼──────────┼─────────┼─────────┼─────────┼─────────┤",
                "│  44 │ 26       │ 27       │ 28       │ 29      │ 30      │ 31      │         │",
                "├─────┼──────────┼──────────┼──────────┼─────────┼─────────┼─────────┼─────────┤",
                "│  44 │          │          │          │         │         │         │ 1 11月  │",
                "╰─────┴──────────┴──────────┴──────────┴─────────┴─────────┴─────────┴─────────╯",
                " hjkl 移動  Tab 切換  [] 換月  t 今天  s 日曆  ? 說明  , 設定  q 離開",
            ]
        );
        // One week down (the same even though 11/2 is not the last of
        // the 6 weeks): the whole screen scrolls down one week with it
        app.selected = date(2026, 11, 2);
        assert_eq!(
            text_rows(draw_app(&mut app, 80, 16).backend().buffer()),
            [
                "╭────────────────────────────── 2026/11/2（一） ───────────────────────────────╮",
                "│  週   一         二         三         四        五        六        日      │",
                "├─────┬──────────┬──────────┬──────────┬─────────┬─────────┬─────────┬─────────┤",
                "│  44 │ 26       │ 27       │ 28       │ 29      │ 30      │ 31      │         │",
                "├─────┼──────────┼──────────┼──────────┼─────────┼─────────┼─────────┼─────────┤",
                "│  44 │          │          │          │         │         │         │ 1 11月  │",
                "├─────┼──────────┼──────────┼──────────┼─────────┼─────────┼─────────┼─────────┤",
                "│  45 │ 2        │ 3        │ 4        │ 5       │ 6       │ 7       │ 8       │",
                "├─────┼──────────┼──────────┼──────────┼─────────┼─────────┼─────────┼─────────┤",
                "│  46 │ 9        │ 10       │ 11       │ 12      │ 13      │ 14      │ 15      │",
                "├─────┼──────────┼──────────┼──────────┼─────────┼─────────┼─────────┼─────────┤",
                "│  47 │ 16       │ 17       │ 18       │ 19      │ 20      │ 21      │ 22      │",
                "├─────┼──────────┼──────────┼──────────┼─────────┼─────────┼─────────┼─────────┤",
                "│  48 │ 23       │ 24       │ 25       │ 26      │ 27      │ 28      │ 29      │",
                "╰─────┴──────────┴──────────┴──────────┴─────────┴─────────┴─────────┴─────────╯",
                " hjkl 移動  Tab 切換  [] 換月  t 今天  s 日曆  ? 說明  , 設定  q 離開",
            ]
        );
    }

    #[test]
    fn relative_week_numbers_in_both_layouts() {
        // Config `[calendar] relative_weeks = true`: the selected week (10/12-18) shows 42, other weeks show
        // how many weeks away they are; in layout 1 the week that crosses months (10/26-11/1) has a row in
        // each month, and both rows show 2
        let mut app = App::new(Lang::ZhTw, date(2026, 10, 17), Vec::new());
        app.settings.calendar.relative_weeks = true;
        assert_eq!(
            text_rows(draw_app(&mut app, 35, 14).backend().buffer()),
            [
                "╭─────────────────────────────────╮",
                "│2026年 週 │ 一 二 三 四 五 六 日 │",
                "│──────────┼──────────────────────│",
                "│  10月  2 │           1  2  3  4 │",
                "│        1 │  5  6  7  8  9 10 11 │",
                "│       42 │ 12 13 14 15 16[17]18 │",
                "│        1 │ 19 20 21 22 23 24 25 │",
                "│        2 │ 26 27 28 29 30 31    │",
                "│──────────┼──────────────────────│",
                "│  11月  2 │                    1 │",
                "│        3 │  2  3  4  5  6  7  8 │",
                "│        4 │  9 10 11 12 13 14 15 │",
                "╰─────────────────────────────────╯",
                " hjkl 移動  Tab 切換  [] 換月  t 今",
            ]
        );
        // Same in layout 3
        app.view = View::Month;
        assert_eq!(
            text_rows(draw_app(&mut app, 80, 16).backend().buffer()),
            [
                "╭────────────────────────────── 2026/10/17（六） ──────────────────────────────╮",
                "│  週   一         二         三         四        五        六        日      │",
                "├─────┬──────────┬──────────┬──────────┬─────────┬─────────┬─────────┬─────────┤",
                "│   2 │ 28       │ 29       │ 30       │         │         │         │         │",
                "├─────┼──────────┼──────────┼──────────┼─────────┼─────────┼─────────┼─────────┤",
                "│   2 │          │          │          │ 1 10月  │ 2       │ 3       │ 4       │",
                "├─────┼──────────┼──────────┼──────────┼─────────┼─────────┼─────────┼─────────┤",
                "│   1 │ 5        │ 6        │ 7        │ 8       │ 9       │ 10      │ 11      │",
                "├─────┼──────────┼──────────┼──────────┼─────────┼─────────┼─────────┼─────────┤",
                "│  42 │ 12       │ 13       │ 14       │ 15      │ 16      │ 17      │ 18      │",
                "├─────┼──────────┼──────────┼──────────┼─────────┼─────────┼─────────┼─────────┤",
                "│   1 │ 19       │ 20       │ 21       │ 22      │ 23      │ 24      │ 25      │",
                "├─────┼──────────┼──────────┼──────────┼─────────┼─────────┼─────────┼─────────┤",
                "│   2 │ 26       │ 27       │ 28       │ 29      │ 30      │ 31      │         │",
                "╰─────┴──────────┴──────────┴──────────┴─────────┴─────────┴─────────┴─────────╯",
                " hjkl 移動  Tab 切換  [] 換月  t 今天  s 日曆  ? 說明  , 設定  q 離開",
            ]
        );
    }

    #[test]
    fn month_view_lists_a_multi_day_item_on_each_of_its_days() {
        // Business trip to NYC 10/20-25: one bar on its days' first line, across the grid lines, the title
        // written once; a one-day item on one of those days goes on the next line (10/22: Buy gifts for
        // the team); Manager OOO 10/28-11/3: its bar ends with the row (10/31), November starts a new row
        let mut app = month_app(Lang::ZhTw, date(2026, 10, 17), RANGE_TASKS);
        let terminal = draw_app(&mut app, 80, 30);
        assert_eq!(
            text_rows(terminal.backend().buffer()),
            [
                "╭────────────────────────────── 2026/10/17（六） ──────────────────────────────╮",
                "│  週   一         二         三         四        五        六        日      │",
                "├─────┬──────────┬──────────┬──────────┬─────────┬─────────┬─────────┬─────────┤",
                "│  40 │ 28       │ 29       │ 30       │         │         │         │         │",
                "│     │          │          │          │         │         │         │         │",
                "│     │          │          │          │         │         │         │         │",
                "├─────┼──────────┼──────────┼──────────┼─────────┼─────────┼─────────┼─────────┤",
                "│  40 │          │          │          │ 1 10月  │ 2       │ 3       │ 4       │",
                "│     │          │          │          │         │         │         │         │",
                "│     │          │          │          │         │         │         │         │",
                "├─────┼──────────┼──────────┼──────────┼─────────┼─────────┼─────────┼─────────┤",
                "│  41 │ 5        │ 6        │ 7        │ 8       │ 9       │ 10      │ 11      │",
                "│     │          │          │          │         │         │         │         │",
                "│     │          │          │          │         │         │         │         │",
                "├─────┼──────────┼──────────┼──────────┼─────────┼─────────┼─────────┼─────────┤",
                "│  42 │ 12       │ 13       │ 14       │ 15      │ 16      │ 17      │ 18      │",
                "│     │          │          │          │         │ ∙ 17:30 │         │         │",
                "│     │          │          │          │         │         │         │         │",
                "├─────┼──────────┼──────────┼──────────┼─────────┼─────────┼─────────┼─────────┤",
                "│  43 │ 19       │ 20       │ 21       │ 22      │ 23      │ 24      │ 25      │",
                "│     │          │ Business trip to NYC                                        │",
                "│     │          │          │          │ ∙ Buy gifts for the team    │         │",
                "├─────┼──────────┼──────────┼──────────┼─────────┼─────────┼─────────┼─────────┤",
                "│  44 │ 26       │ 27       │ 28       │ 29      │ 30      │ 31      │         │",
                "│     │          │          │ Manager OOO                            │         │",
                "│     │          │          │          │         │         │         │         │",
                "│     │          │          │          │         │         │         │         │",
                "│     │          │          │          │         │         │         │         │",
                "╰─────┴──────────┴──────────┴──────────┴─────────┴─────────┴─────────┴─────────╯",
                " hjkl 移動  Tab 切換  [] 換月  t 今天  s 日曆  ? 說明  , 設定  q 離開",
            ]
        );
        // Dates in a range are underlined: 10/20 (Tue) 「 20」 at x 19-20, y 19; 10/19 (Mon) at x 8-9 is not
        let buffer = terminal.backend().buffer();
        let underlined = |x: u16, y: u16| buffer[(x, y)].modifier.contains(Modifier::UNDERLINED);
        assert!(underlined(20, 19));
        assert!(!underlined(9, 19));
        // The bar (y 20) has the range background from Tuesday's left edge (x 18) to Sunday's right edge
        // (x 78), the grid lines between too (x 28, 39); not the grid line left of it, nor the next line
        let theme = Theme::catppuccin_mocha();
        for x in [18, 28, 39, 60, 78] {
            assert_eq!(buffer[(x, 20)].bg, theme.range, "x {x}");
        }
        assert_eq!(buffer[(17, 20)].symbol(), "│");
        assert_ne!(buffer[(17, 20)].bg, theme.range);
        assert_ne!(buffer[(18, 21)].bg, theme.range);
        // The title is normal text (not over yet), not the day's color
        assert_eq!(
            (buffer[(19, 20)].symbol(), buffer[(19, 20)].fg),
            ("B", theme.text)
        );
    }

    /// An app on the tasks page with today 2026/10/17 and the
    /// sample task file (mistakes recorded too, as in `main`)
    fn tasks_app(lang: Lang) -> App {
        let parsed = crate::tasks::parse(include_str!("../testdata/tasks.md"));
        let mut app = App::new(lang, date(2026, 10, 17), parsed.items);
        app.warnings = parsed.warnings;
        app.view = View::Tasks;
        app.focus = Focus::Tasks;
        app
    }

    #[test]
    fn tasks_page_lists_the_file_as_a_tree() {
        // 70 × 30: the whole task file listed by calendar, items without a date too;
        // subtasks folded (▸), followed by how many subtasks are done; date and time
        // each get a column on the right, aligned (blank when there is no time); the
        // bottom row is the original text of the chosen item (the first one)
        let mut app = tasks_app(Lang::ZhTw);
        assert_eq!(
            text_rows(draw_app(&mut app, 70, 30).backend().buffer()),
            [
                "╭────────────────────────────────────────────────────────────────────╮",
                "│ 任務                                                      1 處寫錯 │",
                "│────────────────────────────────────────────────────────────────────│",
                "│ Payments API v2                                                    │",
                "│   [x] Align with PM on the refund flow spec             9/28       │",
                "│   [x] Break down tickets and estimate                   9/30 10:00 │",
                "│ ▸ [ ] Implement the refund API  1/4                    10/14       │",
                "│   [ ] Code review: Amy's PR #482                       10/12 14:00 │",
                "│   [ ] Confirm alerting with SRE                        10/13 15:00 │",
                "│   [ ] Reply to Kevin about error codes                 10/14       │",
                "│   [x] Fix the staging DB migration                      10/8       │",
                "│    ∙  Code freeze                                      10/23       │",
                "│   [ ] Prepare the release checklist @2026-10-23        10/22       │",
                "│   [ ] Write the runbook, see https://wiki.example.com/payments/run │",
                "│    ∙  Release v2.3                                     10/27 09:30 │",
                "│ Recurring                                                          │",
                "│    ∙  Team sync                                        10/12 10:00 │",
                "│    ∙  Team sync                                        10/19 10:00 │",
                "│    ∙  Team sync                                        10/26 10:00 │",
                "│   [ ] Send the weekly update                           10/16 17:30 │",
                "│   [ ] Clean up this week's notes                       10/16       │",
                "│   [ ] Draft the release notes                          10/16       │",
                "│    ∙  Team happy hour                                  10/16 18:30 │",
                "│    ∙  Requirements meeting with PM                      10/8 14:00 │",
                "│    ∙  Engineering all-hands                             10/8 16:00 │",
                "│    ∙  Team offsite                                     10/29       │",
                "│────────────────────────────────────────────────────────────────────│",
                "│ 第 4 行  - [x] Align with PM on the refund flow spec @2026-09-28   │",
                "╰────────────────────────────────────────────────────────────────────╯",
                " jk 移動  hl 收合  Space 打勾  e 編輯  a 新增  s 日曆  Esc 月曆  ? 說",
            ]
        );
        // Expand a few items (▾) and choose line 61 (three levels deep):
        // scroll down until it is visible, 2 rows from the bottom edge
        let mut app = tasks_app(Lang::En);
        app.expanded.extend([6, 59, 60]);
        app.tasks_line = Some(61);
        assert_eq!(
            text_rows(draw_app(&mut app, 70, 16).backend().buffer()),
            [
                "╭────────────────────────────────────────────────────────────────────╮",
                "│ Tasks                                                 1 mistake(s) │",
                "│────────────────────────────────────────────────────────────────────│",
                "│    ∙  Halloween 🎃                                     10/31       │",
                "│ Learning Rust 🦀                                                   │",
                "│   [x] Finish chapter 4 of The Book (ownership)          10/3       │",
                "│   [ ] Read the Ratatui layout docs                                 │",
                "│ ▾ [ ] Write the task file parser for roh-cal  0/1      10/11       │",
                "│   ▾ [ ] Write a test for every rule  0/1                           │",
                "│       [ ] Test that `@Rukis` is not a date                         │",
                "│    ∙  RustConf livestream @Rukis                       10/29 23:00 │",
                "│    ∙  Rust async study group                           10/14 19:30 │",
                "│────────────────────────────────────────────────────────────────────│",
                "│ Line 61  - [ ] Test that `@Rukis` is not a date                    │",
                "╰────────────────────────────────────────────────────────────────────╯",
                " jk move  hl fold  Space check  e edit  a add  s calendars  Esc calend",
            ]
        );
    }

    #[test]
    fn tasks_page_colors_and_clicks() {
        // Dates not in this year get the year in front; done tasks gray and struck through; past events gray,
        // not struck through; dates in that day's color; the chosen row gets the highlight background
        let text = "# Work\n- [ ] Send the weekly update @2026-10-16\n- [x] Fix the login bug @2026-10-15\n\
                    - Hackathon @2026-10-10\n- Trip to Tokyo @2027-01-02\n  - [ ] Book flights\n";
        let mut app = App::new(
            Lang::ZhTw,
            date(2026, 10, 17),
            crate::tasks::parse(text).items,
        );
        app.view = View::Tasks;
        app.focus = Focus::Tasks;
        app.screen = Rect::new(0, 0, 40, 14);
        let terminal = draw_app(&mut app, 40, 14);
        let rows = text_rows(terminal.backend().buffer());
        assert_eq!(rows[7], "│ ▸  ∙  Trip to Tokyo  0/1    2027/1/2 │");
        let buffer = terminal.backend().buffer();
        let theme = Theme::catppuccin_mocha();
        let cell = |x: u16, y: u16| {
            let cell = &buffer[(x, y)];
            (
                cell.symbol(),
                cell.fg,
                cell.modifier.contains(Modifier::CROSSED_OUT),
            )
        };
        // Heading: Work (label color, bold)
        assert_eq!(buffer[(2, 3)].fg, theme.label);
        assert!(buffer[(2, 3)].modifier.contains(Modifier::BOLD));
        // Send the weekly update (y 4): not done → normal text, date (x 33-37) orange;
        // the chosen row gets the highlight across the whole row
        assert_eq!(cell(8, 4), ("S", theme.text, false));
        assert_eq!(cell(34, 4), ("0", theme.has_item, false));
        assert_eq!(buffer[(1, 4)].bg, theme.cursor_line);
        assert_eq!(buffer[(1, 5)].bg, Color::Reset);
        // Fix the login bug (y 5): done → gray, struck through; Hackathon (y 6): past event → gray, not
        // struck through
        assert_eq!(cell(8, 5), ("F", theme.muted, true));
        assert_eq!(cell(8, 6), ("H", theme.muted, false));
        // ▸ is muted
        assert_eq!(cell(2, 7), ("▸", theme.muted, false));
        // Click the Trip to Tokyo row (y 7): it gets chosen; click ▸ (x 2): it expands
        let click = |app: &mut App, column, row| {
            use ratatui::crossterm::event::{
                KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
            };
            app.handle_mouse(MouseEvent {
                kind: MouseEventKind::Down(MouseButton::Left),
                column,
                row,
                modifiers: KeyModifiers::NONE,
            });
        };
        click(&mut app, 10, 7);
        assert_eq!(
            app.tasks_chosen().map(|item| item.title.as_str()),
            Some("Trip to Tokyo")
        );
        assert!(app.expanded.is_empty());
        click(&mut app, 2, 7);
        assert!(app.expanded.contains(&5));
        // Clicking a heading row: ignored
        click(&mut app, 5, 3);
        assert_eq!(
            app.tasks_chosen().map(|item| item.title.as_str()),
            Some("Trip to Tokyo")
        );
    }

    #[test]
    fn tasks_page_points_at_format_mistakes() {
        // The header's right side shows how many mistakes there are; choose the line with a mistake (line 43,
        // @2026-13-01): the bottom row says what is wrong instead, and the title is red
        let mut app = tasks_app(Lang::ZhTw);
        app.tasks_line = Some(43);
        let terminal = draw_app(&mut app, 70, 16);
        assert_eq!(
            text_rows(terminal.backend().buffer()),
            [
                "╭────────────────────────────────────────────────────────────────────╮",
                "│ 任務                                                      1 處寫錯 │",
                "│────────────────────────────────────────────────────────────────────│",
                "│   [ ] Pay the credit card bill                         10/25       │",
                "│    ∙  Dentist appointment                              10/14 10:30 │",
                "│   [ ] Organize the bookshelf                                       │",
                "│   [ ] Buy Mom's birthday gift                           11/2       │",
                "│   [ ] Book the annual physical                         10/16       │",
                "│    ∙  Mom's birthday 🎂                                10/29       │",
                "│   [ ] Pay property tax @2026-13-01                                 │",
                "│   Moving                                                           │",
                "│   [ ] Get quotes from movers                            11/7       │",
                "│────────────────────────────────────────────────────────────────────│",
                "│ 第 43 行：@2026-13-01：沒有這一天                                  │",
                "╰────────────────────────────────────────────────────────────────────╯",
                " jk 移動  hl 收合  Space 打勾  e 編輯  a 新增  s 日曆  Esc 月曆  ? 說",
            ]
        );
        let buffer = terminal.backend().buffer();
        let theme = Theme::catppuccin_mocha();
        // The header's 「1 處寫錯」 and the bottom row are both red
        assert_eq!(buffer[(60, 1)].fg, theme.error);
        assert_eq!(buffer[(2, 13)].fg, theme.error);
        // The title in the Pay property tax row (y 9) is red (titles start at x 8); other rows are not
        assert_eq!(buffer[(8, 9)].fg, theme.error);
        assert_eq!(buffer[(8, 8)].fg, theme.text);
    }

    #[test]
    fn tasks_page_with_the_side_menu() {
        // 121 cells wide: side menu on the left; unchecked
        // calendars (Payments API v2, Recurring) are left out entirely
        let mut app = tasks_app(Lang::ZhTw);
        app.groups[0].shown = false;
        app.groups[1].shown = false;
        assert_eq!(
            text_rows(draw_app(&mut app, 121, 14).backend().buffer()),
            [
                "╭──────────────────────╮╭───────────────────────────────────────────────────────────────────────────────────────────────╮",
                "│ 日曆                 ││ 任務                                                                                 1 處寫錯 │",
                "│──────────────────────││───────────────────────────────────────────────────────────────────────────────────────────────│",
                "│ ☐ Payments API v2 12 ││ Personal                                                                                      │",
                "│ ☐ Recurring       14 ││   [ ] Pay the credit card bill                                                    10/25       │",
                "│ ☑ Personal         5 ││    ∙  Dentist appointment                                                         10/14 10:30 │",
                "│   ☑ Moving         4 ││   [ ] Organize the bookshelf                                                                  │",
                "│ ☑ US holidays      2 ││   [ ] Buy Mom's birthday gift                                                      11/2       │",
                "│ ☑ Learning Rust 🦀 4 ││   [ ] Book the annual physical                                                    10/16       │",
                "│ ☑ Tokyo trip       5 ││    ∙  Mom's birthday 🎂                                                           10/29       │",
                "│                      ││───────────────────────────────────────────────────────────────────────────────────────────────│",
                "│                      ││ 第 37 行  - [ ] Pay the credit card bill @2026-10-25                                          │",
                "╰──────────────────────╯╰───────────────────────────────────────────────────────────────────────────────────────────────╯",
                " jk 移動  hl 收合  Space 打勾  e 編輯  a 新增  s 日曆  Esc 月曆  ? 說明  , 設定  q 離開",
            ]
        );
    }

    #[test]
    fn the_day_popup_lists_every_item_of_the_day_over_the_big_calendar() {
        // 60 × 20, layout 3, 10/17 has 4 items: the popup is centered on the month
        // grid (60 × 19), 30 cells wide (the minimum), 6 rows tall → x 15, y 6; the top
        // border shows the date, one item per row (like layout 1's agenda), the first item
        // is chosen with its date in brackets; the bottom row switches to the popup's keys
        let text = "- [ ] Buy groceries @2026-10-17 17:30\n- Farmers market @2026-10-17 10:00\n\
                    - [x] Clean the garage @2026-10-17\n- Parents in town @2026-10-16..2026-10-18\n";
        let mut app = month_app(Lang::ZhTw, date(2026, 10, 17), text);
        app.focus = Focus::Day;
        let terminal = draw_app(&mut app, 60, 20);
        let buffer = terminal.backend().buffer();
        assert_eq!(
            area_rows(buffer, Rect::new(15, 6, 30, 6)),
            [
                "╭ 2026/10/17（六） ──────────╮",
                "│[10/16~18]Parents in town   │",
                "│ 10/17 Clean the garage     │",
                "│ 10/17 10:00 Farmers market │",
                "│ 10/17 17:30 Buy groceries  │",
                "╰────────────────────────────╯",
            ]
        );
        // The key hints are 61 cells, so at 60 cells wide the last character is cut
        assert_eq!(
            text_rows(buffer)[19],
            " jk 移動  Space 打勾  e 編輯  a 新增  r 改期  d 刪除  Esc 關"
        );
        // The border has the color of where the keyboard is, the brackets the selected red
        let theme = Theme::catppuccin_mocha();
        assert_eq!(buffer[(15, 6)].fg, theme.focus);
        assert_eq!(buffer[(16, 7)].fg, theme.selected);
        // Too short (60 × 6, month grid 5 rows): the box holds 3 items, and choosing the last one scrolls
        // down one item; a scrollbar on the right border
        app.day_index = 3;
        let terminal = draw_app(&mut app, 60, 6);
        let buffer = terminal.backend().buffer();
        assert_eq!(
            area_rows(buffer, Rect::new(15, 1, 29, 3)),
            [
                "│ 10/17 Clean the garage     ",
                "│ 10/17 10:00 Farmers market ",
                "│[10/17]17:30 Buy groceries  ",
            ]
        );
        assert!((1..4).any(|y| buffer[(44, y)].symbol() == "┃"));
        // Click the top row (Clean the garage): it gets chosen, the scroll stays; clicking the same spot
        // again is still it (no task file, so checking it off writes nothing)
        app.screen = Rect::new(0, 0, 60, 6);
        let click = |app: &mut App| {
            use ratatui::crossterm::event::{
                KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
            };
            app.handle_mouse(MouseEvent {
                kind: MouseEventKind::Down(MouseButton::Left),
                column: 17,
                row: 1,
                modifiers: KeyModifiers::NONE,
            });
        };
        click(&mut app);
        assert_eq!(app.day_index, 1);
        let terminal = draw_app(&mut app, 60, 6);
        assert_eq!(
            area_rows(terminal.backend().buffer(), Rect::new(15, 1, 29, 1)),
            ["│[10/17]Clean the garage     "]
        );
        click(&mut app);
        assert_eq!(app.day_index, 1);
    }

    #[test]
    fn month_view_scrolls_a_row_at_a_time() {
        // 80 × 16: 1 line per row. Right after opening the top row is September's last (9/28-30); selecting
        // 11/2 → its row (11/2-8) is two rows below the last one (10/26-31, then 11/1 alone), so the grid
        // scrolls down two rows and the top becomes 10/5
        let mut app = month_app(Lang::En, date(2026, 10, 17), "");
        draw_app(&mut app, 80, 16);
        assert_eq!(app.month_top, Some(date(2026, 9, 28)));
        app.selected = date(2026, 11, 2);
        assert_eq!(
            text_rows(draw_app(&mut app, 80, 16).backend().buffer()),
            [
                "╭────────────────────────────── Mon Nov 2, 2026 ───────────────────────────────╮",
                "│  Wk   Mo         Tu         We         Th        Fr        Sa        Su      │",
                "├─────┬──────────┬──────────┬──────────┬─────────┬─────────┬─────────┬─────────┤",
                "│  41 │ 5        │ 6        │ 7        │ 8       │ 9       │ 10      │ 11      │",
                "├─────┼──────────┼──────────┼──────────┼─────────┼─────────┼─────────┼─────────┤",
                "│  42 │ 12       │ 13       │ 14       │ 15      │ 16      │ 17      │ 18      │",
                "├─────┼──────────┼──────────┼──────────┼─────────┼─────────┼─────────┼─────────┤",
                "│  43 │ 19       │ 20       │ 21       │ 22      │ 23      │ 24      │ 25      │",
                "├─────┼──────────┼──────────┼──────────┼─────────┼─────────┼─────────┼─────────┤",
                "│  44 │ 26       │ 27       │ 28       │ 29      │ 30      │ 31      │         │",
                "├─────┼──────────┼──────────┼──────────┼─────────┼─────────┼─────────┼─────────┤",
                "│  44 │          │          │          │         │         │         │ 1 Nov   │",
                "├─────┼──────────┼──────────┼──────────┼─────────┼─────────┼─────────┼─────────┤",
                "│  45 │ 2        │ 3        │ 4        │ 5       │ 6       │ 7       │ 8       │",
                "╰─────┴──────────┴──────────┴──────────┴─────────┴─────────┴─────────┴─────────╯",
                " hjkl move  Tab pane  [] month  t today  s calendars  ? help  , settings  q quit",
            ]
        );
        assert_eq!(app.month_top, Some(date(2026, 10, 5)));
        // Select 9/20 (a Sunday, in the row 9/14-20, above the top): that row becomes the top; the selected
        // month becomes September, so October's days turn gray
        app.lang = Lang::ZhTw;
        app.selected = date(2026, 9, 20);
        assert_eq!(
            text_rows(draw_app(&mut app, 80, 16).backend().buffer()),
            [
                "╭────────────────────────────── 2026/9/20（日） ───────────────────────────────╮",
                "│  週   一         二         三         四        五        六        日      │",
                "├─────┬──────────┬──────────┬──────────┬─────────┬─────────┬─────────┬─────────┤",
                "│  38 │ 14       │ 15       │ 16       │ 17      │ 18      │ 19      │ 20      │",
                "├─────┼──────────┼──────────┼──────────┼─────────┼─────────┼─────────┼─────────┤",
                "│  39 │ 21       │ 22       │ 23       │ 24      │ 25      │ 26      │ 27      │",
                "├─────┼──────────┼──────────┼──────────┼─────────┼─────────┼─────────┼─────────┤",
                "│  40 │ 28       │ 29       │ 30       │         │         │         │         │",
                "├─────┼──────────┼──────────┼──────────┼─────────┼─────────┼─────────┼─────────┤",
                "│  40 │          │          │          │ 1 10月  │ 2       │ 3       │ 4       │",
                "├─────┼──────────┼──────────┼──────────┼─────────┼─────────┼─────────┼─────────┤",
                "│  41 │ 5        │ 6        │ 7        │ 8       │ 9       │ 10      │ 11      │",
                "├─────┼──────────┼──────────┼──────────┼─────────┼─────────┼─────────┼─────────┤",
                "│  42 │ 12       │ 13       │ 14       │ 15      │ 16      │ 17      │ 18      │",
                "╰─────┴──────────┴──────────┴──────────┴─────────┴─────────┴─────────┴─────────╯",
                " hjkl 移動  Tab 切換  [] 換月  t 今天  s 日曆  ? 說明  , 設定  q 離開",
            ]
        );
        assert_eq!(app.month_top, Some(date(2026, 9, 14)));
        // Moving within the 6 rows (10/18 is in the last one, 10/12-18): no scrolling
        app.selected = date(2026, 10, 18);
        draw_app(&mut app, 80, 16);
        assert_eq!(app.month_top, Some(date(2026, 9, 14)));
    }

    #[test]
    fn month_view_click_positions_map_to_the_cells() {
        // 80 × 30 (screen: see `month_view_fills_the_space_with_six_equal_weeks`): Mon x 7-16, Tue x 18-27,
        // Thu x 40-48, Sat x 60-68, Sun x 70-78; rows at y 3-5, 7-9, …, 23-25. The top row is 9/28-30
        // (Thu-Sun empty), the next one 10/1-4 (Mon-Wed empty), the last one 10/26-31 (Sun empty)
        let mut app = month_app(Lang::ZhTw, date(2026, 10, 17), "");
        app.screen = Rect::new(0, 0, 80, 30);
        draw_app(&mut app, 80, 30);
        // Any cell in a day's box counts as that day (not just the date on the first line)
        assert_eq!(date_at(&app, 7, 3), Some(date(2026, 9, 28)));
        assert_eq!(date_at(&app, 16, 5), Some(date(2026, 9, 28)));
        assert_eq!(date_at(&app, 40, 7), Some(date(2026, 10, 1)));
        assert_eq!(date_at(&app, 68, 25), Some(date(2026, 10, 31)));
        // The leftover rows under the last row count as the last row too
        assert_eq!(date_at(&app, 10, 27), Some(date(2026, 10, 26)));
        // No date: the empty cells of a split week, vertical lines, horizontal lines, the week-number
        // column, the header, the bottom border, the left border, the key hints
        for (x, y) in [
            (40, 3),
            (18, 7),
            (78, 25),
            (17, 3),
            (10, 6),
            (3, 3),
            (10, 1),
            (10, 28),
            (0, 3),
            (10, 29),
        ] {
            assert_eq!(date_at(&app, x, y), None, "({x}, {y})");
        }
    }

    #[test]
    fn month_view_marks_a_new_year_with_a_thick_line() {
        // 2027/1/1 (Fri) selected: the top row is 2026's last one (12/28-31), then 1/1-3; the line between
        // them is thick in the year color, meets the border as ┝ / ┥, and has the year on it from x 8
        // (Monday's cell starts at x 7)
        let mut app = month_app(Lang::En, date(2027, 1, 1), "");
        let terminal = draw_app(&mut app, 80, 30);
        assert_eq!(
            text_rows(terminal.backend().buffer()),
            [
                "╭────────────────────────────── Fri Jan 1, 2027 ───────────────────────────────╮",
                "│  Wk   Mo         Tu         We         Th        Fr        Sa        Su      │",
                "├─────┬──────────┬──────────┬──────────┬─────────┬─────────┬─────────┬─────────┤",
                "│  53 │ 28       │ 29       │ 30       │ 31      │         │         │         │",
                "│     │          │          │          │         │         │         │         │",
                "│     │          │          │          │         │         │         │         │",
                "┝━━━━━┿━ 2027 ━━━┿━━━━━━━━━━┿━━━━━━━━━━┿━━━━━━━━━┿━━━━━━━━━┿━━━━━━━━━┿━━━━━━━━━┥",
                "│   1 │          │          │          │         │ 1 Jan   │ 2       │ 3       │",
                "│     │          │          │          │         │         │         │         │",
                "│     │          │          │          │         │         │         │         │",
                "├─────┼──────────┼──────────┼──────────┼─────────┼─────────┼─────────┼─────────┤",
                "│   2 │ 4        │ 5        │ 6        │ 7       │ 8       │ 9       │ 10      │",
                "│     │          │          │          │         │         │         │         │",
                "│     │          │          │          │         │         │         │         │",
                "├─────┼──────────┼──────────┼──────────┼─────────┼─────────┼─────────┼─────────┤",
                "│   3 │ 11       │ 12       │ 13       │ 14      │ 15      │ 16      │ 17      │",
                "│     │          │          │          │         │         │         │         │",
                "│     │          │          │          │         │         │         │         │",
                "├─────┼──────────┼──────────┼──────────┼─────────┼─────────┼─────────┼─────────┤",
                "│   4 │ 18       │ 19       │ 20       │ 21      │ 22      │ 23      │ 24      │",
                "│     │          │          │          │         │         │         │         │",
                "│     │          │          │          │         │         │         │         │",
                "├─────┼──────────┼──────────┼──────────┼─────────┼─────────┼─────────┼─────────┤",
                "│   5 │ 25       │ 26       │ 27       │ 28      │ 29      │ 30      │ 31      │",
                "│     │          │          │          │         │         │         │         │",
                "│     │          │          │          │         │         │         │         │",
                "│     │          │          │          │         │         │         │         │",
                "│     │          │          │          │         │         │         │         │",
                "╰─────┴──────────┴──────────┴──────────┴─────────┴─────────┴─────────┴─────────╯",
                " hjkl move  Tab pane  [] month  t today  s calendars  ? help  , settings  q quit",
            ]
        );
        let buffer = terminal.backend().buffer();
        let theme = Theme::catppuccin_mocha();
        let cell = |x: u16, y: u16| (buffer[(x, y)].symbol(), buffer[(x, y)].fg);
        assert_eq!(cell(6, 6), ("┿", theme.year_line));
        assert_eq!(cell(9, 6), ("2", theme.label));
        assert_eq!(cell(15, 6), ("━", theme.year_line));
        assert_eq!(cell(0, 6), ("┝", theme.focus));
        assert_eq!(cell(79, 6), ("┥", theme.focus));
        // The next row line (above 1/4) is faint again
        assert_eq!(cell(10, 10), ("─", theme.grid));
    }

    #[test]
    fn month_view_without_grid_lines_still_marks_a_new_year() {
        // 80 × 24 without grid lines, 2027/1/1 selected: rows are 3 tall (22 − header 1 − year line 1 = 20,
        // 20 / 6 = 3); 12/28-31 at y 2-4, then the year line at y 5 (thick, ┝ / ┥ at the border, the year
        // from x 7 since Monday starts at x 6), then 1/1-3 at y 6
        let mut app = month_app(Lang::ZhTw, date(2027, 1, 1), "");
        app.settings.month.grid_lines = false;
        let terminal = draw_app(&mut app, 80, 24);
        assert_eq!(
            text_rows(terminal.backend().buffer()),
            [
                "╭─────────────────────────────── 2027/1/1（五） ───────────────────────────────╮",
                "│  週  一         二         三         四        五        六        日       │",
                "│  53  28         29         30         31                                     │",
                "│                                                                              │",
                "│                                                                              │",
                "┝━━━━━━ 2027年 ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━┥",
                "│   1                                             1 1月     2         3        │",
                "│                                                                              │",
                "│                                                                              │",
                "│   2  4          5          6          7         8         9         10       │",
                "│                                                                              │",
                "│                                                                              │",
                "│   3  11         12         13         14        15        16        17       │",
                "│                                                                              │",
                "│                                                                              │",
                "│   4  18         19         20         21        22        23        24       │",
                "│                                                                              │",
                "│                                                                              │",
                "│   5  25         26         27         28        29        30        31       │",
                "│                                                                              │",
                "│                                                                              │",
                "│                                                                              │",
                "╰──────────────────────────────────────────────────────────────────────────────╯",
                " hjkl 移動  Tab 切換  [] 換月  t 今天  s 日曆  ? 說明  , 設定  q 離開",
            ]
        );
        let buffer = terminal.backend().buffer();
        let theme = Theme::catppuccin_mocha();
        let cell = |x: u16, y: u16| (buffer[(x, y)].symbol(), buffer[(x, y)].fg);
        assert_eq!(cell(20, 5), ("━", theme.year_line));
        assert_eq!(cell(0, 5), ("┝", theme.focus));
        assert_eq!(cell(79, 5), ("┥", theme.focus));
        assert_eq!(cell(8, 5), ("2", theme.label));
    }

    #[test]
    fn month_view_colors_by_role() {
        // 80 × 30 (screen: see `month_view_fills_the_space_with_six_equal_weeks`): Mon x 7, Tue x 18,
        // Wed x 29, Thu x 40, Fri x 50, Sat x 60, Sun x 70; weeks 1-6 at y 3, 7, 11, 15, 19, 23. 10/16 (Fri)
        // selected, 10/15 (Thu) has an unfinished item
        let mut app = month_app(
            Lang::ZhTw,
            date(2026, 10, 16),
            "- [ ] Send the weekly update @2026-10-15\n",
        );
        let terminal = draw_app(&mut app, 80, 30);
        let buffer = terminal.backend().buffer();
        let theme = Theme::catppuccin_mocha();
        // The (x, y) cell: symbol, foreground color, whether it is bold
        let cell = |x: u16, y: u16| {
            let cell = &buffer[(x, y)];
            (
                cell.symbol(),
                cell.fg,
                cell.modifier.contains(Modifier::BOLD),
            )
        };
        // The selected day has no brackets; the date has that day's color (normal on a weekday), and dates
        // in the selected month are bold
        assert_eq!(cell(50, 15).0, " ");
        assert_eq!(cell(52, 15), ("6", theme.text, true));
        assert_eq!(cell(53, 15).0, " ");
        // Its whole cell (x 50-58, y 15-17) has the cursor-line background, from the top-left to the
        // bottom-right; the grid lines around it and the neighbor cells do not
        let bg = |x: u16, y: u16| buffer[(x, y)].bg;
        for (x, y) in [(50, 15), (58, 15), (50, 17), (58, 17), (54, 16)] {
            assert_eq!(bg(x, y), theme.cursor_line, "({x}, {y})");
        }
        for (x, y) in [(49, 15), (59, 15), (50, 14), (50, 18), (48, 15), (60, 15)] {
            assert_ne!(bg(x, y), theme.cursor_line, "({x}, {y})");
        }
        // 10/15's long title does not borrow into the selected cell: it stops at the grid line
        assert_eq!(cell(41, 16).0, "∙");
        assert_eq!(cell(50, 16).0, " ");
        // Today green, days with unfinished items orange, weekends blue
        assert_eq!(cell(62, 15), ("7", theme.today, true));
        assert_eq!(cell(42, 15), ("5", theme.has_item, true));
        assert_eq!(cell(72, 15), ("8", theme.weekend, true));
        // Dates in other months are gray, not bold: 9/28; 10/1's month is in the label color
        assert_eq!(cell(9, 3), ("8", theme.other_month, false));
        assert_eq!(cell(43, 7), ("1", theme.label, false));
        // Week numbers: the selected row (week 42) bright, other rows (week 40) dim
        assert_eq!(cell(3, 15), ("4", theme.week_number, false));
        assert_eq!(cell(3, 3), ("4", theme.week_number_dim, false));
        // Header: weekdays muted, Sat and Sun in the weekend color
        assert_eq!(cell(8, 1), ("一", theme.muted, false));
        assert_eq!(cell(61, 1), ("六", theme.weekend, false));
        // Grid lines are faint; the joints where they meet the outer border
        // have the border's color (keyboard in the calendar: focus color)
        assert_eq!(cell(6, 2), ("┬", theme.grid, false));
        assert_eq!(cell(6, 3), ("│", theme.grid, false));
        assert_eq!(cell(6, 10), ("┼", theme.grid, false));
        // The line above October's first row (y 6) is in the border color, like layout 1's month lines
        assert_eq!(cell(6, 6), ("┼", theme.line, false));
        assert_eq!(cell(10, 6), ("─", theme.line, false));
        // The leftover rows under the last week: vertical lines still reach the bottom border, which joins
        // them as ┴ (in the border color)
        assert_eq!(cell(6, 26), ("│", theme.grid, false));
        assert_eq!(cell(6, 28), ("┴", theme.focus, false));
        assert_eq!(cell(0, 6), ("├", theme.focus, false));
        assert_eq!(cell(79, 6), ("┤", theme.focus, false));
        // The date in the middle of the top border is in the normal color: 18 cells wide in a 78-cell
        // border → starts at 1 + (78 - 18) / 2 = 31, so its 「2」 is at x 32
        assert_eq!(cell(32, 0), ("2", theme.text, false));
    }

    #[test]
    fn selected_day_is_bold_in_its_own_color() {
        // Select 10/16 (Fri): the number is in the normal color and bold; today 10/17 keeps today's color
        let terminal = draw(Lang::ZhTw, 40, 26, date(2026, 10, 16));
        let buffer = terminal.backend().buffer();
        let theme = Theme::catppuccin_mocha();
        let cell = |x: u16, y: u16| {
            let cell = &buffer[(x, y)];
            (
                cell.symbol(),
                cell.fg,
                cell.modifier.contains(Modifier::BOLD),
            )
        };
        assert_eq!(cell(24, 5), ("[", theme.selected, true));
        assert_eq!(cell(26, 5), ("6", theme.text, true));
        assert_eq!(cell(27, 5), ("]", theme.selected, true));
        assert_eq!(cell(29, 5), ("7", theme.today, true));
    }
}
