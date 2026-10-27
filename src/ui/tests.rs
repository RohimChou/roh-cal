use super::*;
use crate::app::{Focus, SideRow, Toast};
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
        Click::Group(_) | Click::HideAll | Click::HideDone | Click::TaskRow { .. } => None,
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
    let terminal = draw(Lang::ZhTw, 42, 26, date(2026, 10, 17));
    assert_eq!(
        text_rows(terminal.backend().buffer()),
        [
            " ┃ 1 月曆   2 大月曆   3 任務",
            "╭────────────────────────────────────────╮",
            "│2026年 週 │ 一  二  三  四  五  六  日  │",
            "│──────────┼─────────────────────────────│",
            "│  10月 40 │              1   2   3   4  │",
            "│       41 │  5   6   7   8   9  10  11  │",
            "│       42 │ 12  13  14  15  16 [17] 18  │",
            "│       43 │ 19  20  21  22  23  24  25  │",
            "│       44 │ 26  27  28  29  30  31      │",
            "│──────────┼─────────────────────────────│",
            "│  11月 44 │                          1  │",
            "│       45 │  2   3   4   5   6   7   8  │",
            "│       46 │  9  10  11  12  13  14  15  │",
            "│       47 │ 16  17  18  19  20  21  22  │",
            "│       48 │ 23  24  25  26  27  28  29  │",
            "│       49 │ 30                          │",
            "│──────────┼─────────────────────────────│",
            "│  12月 49 │      1   2   3   4   5   6  │",
            "│       50 │  7   8   9  10  11  12  13  │",
            "│       51 │ 14  15  16  17  18  19  20  │",
            "│       52 │ 21  22  23  24  25  26  27  │",
            "│       53 │ 28  29  30  31              │",
            "│━━━━━━━━━━┿━━━━━━━━━━━━━━━━━━━━━━━━━━━━━│",
            "│2027年  1 │                  1   2   3  │",
            "╰────────────────────────────────────────╯",
            " hjkl[] 移動 | Tab 切換 | t 今天 | s 日曆",
        ]
    );
}

#[test]
fn calendar_in_english() {
    let terminal = draw(Lang::En, 42, 11, date(2026, 10, 17));
    assert_eq!(
        text_rows(terminal.backend().buffer()),
        [
            " ┃ 1 calendar   2 month   3 tasks",
            "╭────────────────────────────────────────╮",
            "│  2026 Wk │ Mo  Tu  We  Th  Fr  Sa  Su  │",
            "│──────────┼─────────────────────────────│",
            "│   Oct 40 │              1   2   3   4  │",
            "│       41 │  5   6   7   8   9  10  11  │",
            "│       42 │ 12  13  14  15  16 [17] 18  │",
            "│       43 │ 19  20  21  22  23  24  25  │",
            "│       44 │ 26  27  28  29  30  31      │",
            "╰────────────────────────────────────────╯",
            " hjkl[] move | Tab pane | t today | s cale",
        ]
    );
}

#[test]
fn selection_brackets_and_the_month_on_top() {
    // Another month selected: that month goes on top; no January on
    // screen, so the header shows the year; brackets hug a one-digit date
    let terminal = draw(Lang::En, 42, 9, date(2026, 11, 2));
    assert_eq!(
        text_rows(terminal.backend().buffer()),
        [
            " ┃ 1 calendar   2 month   3 tasks",
            "╭────────────────────────────────────────╮",
            "│  2026 Wk │ Mo  Tu  We  Th  Fr  Sa  Su  │",
            "│──────────┼─────────────────────────────│",
            "│   Nov 44 │                          1  │",
            "│       45 │ [2]  3   4   5   6   7   8  │",
            "│       46 │  9  10  11  12  13  14  15  │",
            "╰────────────────────────────────────────╯",
            " hjkl[] move | Tab pane | t today | s cale",
        ]
    );
    // Sunday: `]` goes in the extra cell at the end; the calendar has only 3 rows and must keep 1 row of
    // margin, so it scrolls up one more row (the thick year line)
    let terminal = draw(Lang::ZhTw, 42, 9, date(2027, 1, 3));
    assert_eq!(
        text_rows(terminal.backend().buffer()),
        [
            " ┃ 1 月曆   2 大月曆   3 任務",
            "╭────────────────────────────────────────╮",
            "│2027年 週 │ 一  二  三  四  五  六  日  │",
            "│──────────┼─────────────────────────────│",
            "│━━━━━━━━━━┿━━━━━━━━━━━━━━━━━━━━━━━━━━━━━│",
            "│2027年  1 │                  1   2  [3] │",
            "│   1月  2 │  4   5   6   7   8   9  10  │",
            "╰────────────────────────────────────────╯",
            " hjkl[] 移動 | Tab 切換 | t 今天 | s 日曆",
        ]
    );
}

#[test]
fn calendar_scrolls_down_only_when_the_selection_gets_close_to_the_bottom() {
    // The calendar has 6 rows: 10/17 → 10/24 → 10/31 → 11/7 going down; the selected row keeps at least 2
    // rows from the bottom edge and scrolls only when it can't
    let mut app = App::new(Lang::En, date(2026, 10, 17), Vec::new());
    draw_app(&mut app, 42, 12);
    for day in [date(2026, 10, 24), date(2026, 10, 31), date(2026, 11, 7)] {
        app.selected = day;
        draw_app(&mut app, 42, 12);
    }
    // October's first 4 rows have scrolled off the top;
    // no January on screen, so the header shows the year
    assert_eq!(
        text_rows(draw_app(&mut app, 42, 12).backend().buffer()),
        [
            " ┃ 1 calendar   2 month   3 tasks",
            "╭────────────────────────────────────────╮",
            "│  2026 Wk │ Mo  Tu  We  Th  Fr  Sa  Su  │",
            "│──────────┼─────────────────────────────│",
            "│       44 │ 26  27  28  29  30  31      │",
            "│──────────┼─────────────────────────────│",
            "│   Nov 44 │                          1  │",
            "│       45 │  2   3   4   5   6  [7]  8  │",
            "│       46 │  9  10  11  12  13  14  15  │",
            "│       47 │ 16  17  18  19  20  21  22  │",
            "╰────────────────────────────────────────╯",
            " hjkl[] move | Tab pane | t today | s cale",
        ]
    );
    // One week back: the selected row keeps at least 2 rows from the top edge, so it scrolls up 2 rows
    app.selected = date(2026, 10, 31);
    assert_eq!(
        text_rows(draw_app(&mut app, 42, 12).backend().buffer()),
        [
            " ┃ 1 calendar   2 month   3 tasks",
            "╭────────────────────────────────────────╮",
            "│  2026 Wk │ Mo  Tu  We  Th  Fr  Sa  Su  │",
            "│──────────┼─────────────────────────────│",
            "│       42 │ 12  13  14  15  16  17  18  │",
            "│       43 │ 19  20  21  22  23  24  25  │",
            "│       44 │ 26  27  28  29  30 [31]     │",
            "│──────────┼─────────────────────────────│",
            "│   Nov 44 │                          1  │",
            "│       45 │  2   3   4   5   6   7   8  │",
            "╰────────────────────────────────────────╯",
            " hjkl[] move | Tab pane | t today | s cale",
        ]
    );
}

#[test]
fn calendar_scrolls_up_into_earlier_months() {
    // From 10/17 one week up to 10/10: only 1 row is left above,
    // so it scrolls up 1 row and shows the line after September
    let mut app = App::new(Lang::ZhTw, date(2026, 10, 17), Vec::new());
    draw_app(&mut app, 42, 11);
    app.selected = date(2026, 10, 10);
    assert_eq!(
        text_rows(draw_app(&mut app, 42, 11).backend().buffer()),
        [
            " ┃ 1 月曆   2 大月曆   3 任務",
            "╭────────────────────────────────────────╮",
            "│2026年 週 │ 一  二  三  四  五  六  日  │",
            "│──────────┼─────────────────────────────│",
            "│──────────┼─────────────────────────────│",
            "│  10月 40 │              1   2   3   4  │",
            "│       41 │  5   6   7   8   9 [10] 11  │",
            "│       42 │ 12  13  14  15  16  17  18  │",
            "│       43 │ 19  20  21  22  23  24  25  │",
            "╰────────────────────────────────────────╯",
            " hjkl[] 移動 | Tab 切換 | t 今天 | s 日曆",
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
    // 47 × 27, October on top (screen: see
    // `calendar_from_this_month_down_with_a_thick_line_between_years`)
    let mut app = App::new(Lang::ZhTw, date(2026, 10, 17), Vec::new());
    app.screen = Rect::new(0, 0, 47, 27);
    draw_app(&mut app, 47, 27);
    // A day is 4 cells wide from x 12 (border, year/month, week number, │), and the blanks before and after
    // its date count as that day: the 16th (Friday, the 5th day) is x 28-31
    assert_eq!(date_at(&app, 28, 6), Some(date(2026, 10, 16)));
    assert_eq!(date_at(&app, 31, 6), Some(date(2026, 10, 16)));
    assert_eq!(date_at(&app, 32, 6), Some(date(2026, 10, 17)));
    assert_eq!(date_at(&app, 39, 6), Some(date(2026, 10, 18)));
    // 11/1 is in the last cell of November's first row (its `1` at x 38); 12/31 is a Thursday (x 24-27)
    assert_eq!(date_at(&app, 38, 10), Some(date(2026, 11, 1)));
    assert_eq!(date_at(&app, 25, 21), Some(date(2026, 12, 31)));
    // No date: the extra cell at the end, the blanks before 10/1, the line between months, the year/month
    // column, the week-number column, the header, the border, the key hints, the blank space on the right
    for (x, y) in [
        (40, 6),
        (13, 4),
        (20, 9),
        (5, 6),
        (9, 6),
        (20, 2),
        (0, 6),
        (20, 26),
        (44, 6),
    ] {
        assert_eq!(date_at(&app, x, y), None, "({x}, {y})");
    }
}

#[test]
fn click_positions_follow_the_scroll() {
    // Scrolled to 11/7 (screen: see
    // `calendar_scrolls_down_only_when_the_selection_gets_close_to_the_bottom`)
    let mut app = App::new(Lang::En, date(2026, 10, 17), Vec::new());
    app.screen = Rect::new(0, 0, 47, 12);
    draw_app(&mut app, 47, 12);
    for day in [date(2026, 10, 24), date(2026, 10, 31), date(2026, 11, 7)] {
        app.selected = day;
        draw_app(&mut app, 47, 12);
    }
    // The top row is the week of 10/26, then a line, then 11/1 (Sunday, x 36-39), then the week of 11/2
    assert_eq!(date_at(&app, 13, 4), Some(date(2026, 10, 26)));
    assert_eq!(date_at(&app, 13, 5), None);
    assert_eq!(date_at(&app, 38, 6), Some(date(2026, 11, 1)));
    assert_eq!(date_at(&app, 13, 7), Some(date(2026, 11, 2)));
}

/// Task file for the agenda tests (`tools/expected_screen.py`
/// reads this same file for the expected screens)
const AGENDA_TASKS: &str = include_str!("../../testdata/agenda.md");

/// Calendars with archived ones among them (`archived:<date>` in their headings)
const ARCHIVED_TASKS: &str = include_str!("../../testdata/archived.md");

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
    // 121 × 14: the side menu fits 5 calendars (y 7-11; above them the 「隱藏全部」 and 「隱藏做完的」 rows
    // and a rule, y 4-6), and there are 12 (c01-c12, 1 item each)
    let text: String = (1..=12)
        .map(|n| format!("# c{n:02}\n- a @2026-10-{n:02}\n"))
        .collect();
    let mut app = App::new(
        Lang::ZhTw,
        date(2026, 10, 17),
        crate::tasks::parse(&text).items,
    );
    app.screen = Rect::new(0, 0, 121, 14);
    app.focus = Focus::Side;
    let names = |app: &mut App| {
        let terminal = draw_app(app, 121, 14);
        let buffer = terminal.backend().buffer();
        let rows = area_rows(buffer, Rect::new(0, 7, 21, 5));
        let thumb: Vec<u16> = (7..12)
            .filter(|&y| buffer[(21, y)].symbol() == "┃")
            .collect();
        // Split each row 「│ ☑ c01 … 1」 on whitespace; the 3rd part is the name
        let names = rows
            .iter()
            .map(|row| row.split_whitespace().nth(2).unwrap_or("").to_string());
        (names.collect::<Vec<_>>(), thumb)
    };
    // The two switches and their rule above the list (not scrolled with it); clicks hit the switches
    let terminal = draw_app(&mut app, 121, 14);
    let top = area_rows(terminal.backend().buffer(), Rect::new(0, 4, 22, 3));
    assert!(top[0].starts_with("│ ☐\u{a0}隱藏全部"), "{}", top[0]);
    assert!(top[1].starts_with("│ ☐\u{a0}隱藏做完的"), "{}", top[1]);
    assert_eq!(top[2], "│────────────────────│");
    assert_eq!(click_at(&app, 5, 4), Some(Click::HideAll));
    assert_eq!(click_at(&app, 5, 5), Some(Click::HideDone));
    assert_eq!(click_at(&app, 5, 6), None);
    // Choose the 1st: no scroll, the scrollbar is at the top
    let (rows, thumb) = names(&mut app);
    assert_eq!(rows, ["c01", "c02", "c03", "c04", "c05"]);
    assert_eq!(thumb.first(), Some(&7));
    // Choose the 9th: scroll only once it leaves the box,
    // just enough to put it on the last row (c05 on top)
    app.side_row = SideRow::Calendar(8);
    assert_eq!(names(&mut app).0, ["c05", "c06", "c07", "c08", "c09"]);
    // The last one: the scrollbar is at the bottom
    app.side_row = SideRow::Calendar(11);
    let (rows, thumb) = names(&mut app);
    assert_eq!(rows, ["c08", "c09", "c10", "c11", "c12"]);
    assert_eq!(thumb.last(), Some(&11));
    // Back to the 8th: still visible, no scroll; the 6th: scroll so it is just on the first row (c06 on
    // top); clicking the first row = c06
    app.side_row = SideRow::Calendar(7);
    assert_eq!(names(&mut app).0, ["c08", "c09", "c10", "c11", "c12"]);
    app.side_row = SideRow::Calendar(5);
    assert_eq!(names(&mut app).0, ["c06", "c07", "c08", "c09", "c10"]);
    assert_eq!(click_at(&app, 5, 7), Some(Click::Group(5)));
    // Click the last row (c10): it gets chosen and does not scroll away; clicking the same
    // spot again = the same calendar, which unchecks it
    let click = |app: &mut App| {
        use ratatui::crossterm::event::{KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
        app.handle_mouse(MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: 5,
            row: 11,
            modifiers: KeyModifiers::NONE,
        });
    };
    click(&mut app);
    assert_eq!(app.side_row, SideRow::Calendar(9));
    assert_eq!(names(&mut app).0, ["c06", "c07", "c08", "c09", "c10"]);
    click(&mut app);
    assert!(!app.groups[9].shown);
    // On a switch row the list keeps the first calendar in view
    app.side_row = SideRow::HideAll;
    assert_eq!(names(&mut app).0, ["c01", "c02", "c03", "c04", "c05"]);
}

#[test]
fn a_narrow_side_menu_covers_the_left_of_the_screen() {
    // 60 × 15, keyboard in the side menu (after s): the side menu is 22 cells
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
    let terminal = draw_app(&mut app, 60, 15);
    let buffer = terminal.backend().buffer();
    assert_eq!(
        area_rows(buffer, Rect::new(0, 1, 22, 9)),
        [
            "╭────────────────────╮",
            "│ 日曆               │",
            "│────────────────────│",
            "│ ☐\u{a0}隱藏全部         │",
            "│ ☐\u{a0}隱藏做完的       │",
            "│────────────────────│",
            "│ ☑\u{a0}Work           1 │",
            "│ ☑\u{a0}Personal       1 │",
            "│                    │",
        ]
    );
    assert_eq!(buffer[(21, 14 - 1)].symbol(), "╯");
    assert_eq!(buffer[(22, 1)].symbol(), "─");
    // Keyboard in the side menu: the key hints are the calendar list's
    assert_eq!(
        text_rows(buffer)[14].trim_end(),
        " jk 移動 | Space 勾選 | Enter 只看這個 | a/A 新增日曆 | e 改"
    );
    // Keyboard not in the side menu: not drawn; the key hints have s even when narrow (60 cells wide,
    // the end is cut off)
    app.focus = Focus::Calendar;
    let terminal = draw_app(&mut app, 60, 15);
    assert_eq!(terminal.backend().buffer()[(0, 4)].symbol(), "│");
    assert_ne!(terminal.backend().buffer()[(4, 4)].symbol(), "W");
    assert!(
        text_rows(terminal.backend().buffer())[14]
            .starts_with(" hjkl[] 移動 | Tab 切換 | t 今天 | s 日曆")
    );
}

#[test]
fn the_bottom_row_starts_with_the_keys() {
    // No layout keys at its left end (the tab bar at the top has the views): the key hints start right
    // away, and a click there does nothing
    for (view, focus, start) in [
        (View::Planner, Focus::Calendar, " hjkl[] move | Tab pane | "),
        (View::Month, Focus::Calendar, " hjkl[] move | Tab pane | "),
        (View::Tasks, Focus::Tasks, " jk move | hl fold | "),
    ] {
        let mut app = App::new(Lang::En, date(2026, 10, 17), Vec::new());
        (app.view, app.focus) = (view, focus);
        app.screen = Rect::new(0, 0, 100, 20);
        let terminal = draw_app(&mut app, 100, 20);
        let bottom = &text_rows(terminal.backend().buffer())[19];
        assert!(bottom.starts_with(start), "{view:?}: {bottom}");
        for x in 0..5 {
            assert_eq!(click_at(&app, x, 19), None, "{view:?} {x}");
        }
    }
}

#[test]
fn side_menu_lists_the_calendars_when_wide() {
    // 121 cells wide: the calendars are listed on the far left (the task file's groups, in order of
    // appearance; items before any heading are 「未分組」), the number is how many items have a date
    // (Work has one item without a date); the last name is too long even at the 38-cell max of the auto
    // width (`CalendarListSettings::AUTO_MAX_WIDTH`), so it is cut; the key hints gain an `s`. The agenda has room for one column only and October's 8 items
    // do not fit: it scrolls 1 row, leaving one row after the selected day's (10/17) items
    let mut app = agenda_app(Lang::ZhTw);
    let terminal = draw_app(&mut app, 128, 15);
    assert_eq!(
        text_rows(terminal.backend().buffer()),
        [
            " ┃ 1 月曆   2 大月曆   3 任務",
            "╭────────────────────────────────────╮╭────────────────────────────────────────╮╭──────────────────────────────────────────────╮",
            "│ 日曆                               ││2026年 週 │ 一  二  三  四  五  六  日  ││ 重要事項                    2026/10/17（六） │",
            "│────────────────────────────────────││──────────┼─────────────────────────────││──────────────────────────────────────────────│",
            "│ ☐\u{a0}隱藏全部                         ││  10月 40 │              1   2   3   4  ││ 10/10 Farmers market                         │",
            "│ ☐\u{a0}隱藏做完的                       ││       41 │  5⠂  6   7   8   9  10⠂ 11  ││ 10/15 Fix the login bug                      ┃",
            "│────────────────────────────────────││       42 │ 12  13  14  15⠂ 16⠂[17⠒]18⠂ ││ 10/16 17:30 Send the weekly update           ┃",
            "│ ☑\u{a0}未分組                         2 ││       43 │ 19  20⠂ 21  22  23  24  25  ││ 10/17 Dinner with friends                    ┃",
            "│ ☑\u{a0}Personal                       5 ││       44 │ 26  27  28  29  30  31      ││ 10/17 08:00 Coffee chat                      ┃",
            "│ ☑\u{a0}Work                           1 ││          │                             ││ 10/18 07:00 Go hiking                        │",
            "│   ☑\u{a0}Year-end odds and ends and p 2 ││──────────┼─────────────────────────────││────────────────────────────────── 還有 1 件 ─│",
            "│                                    ││  11月 44 │                          1  ││  11/3 09:30 Release v2.3                     │",
            "│                                    ││       45 │  2   3⠂  4   5   6   7   8  ││                                              │",
            "╰────────────────────────────────────╯╰────────────────────────────────────────╯╰──────────────────────────────────────────────╯",
            " hjkl[] 移動 | Tab 切換 | t 今天 | s 日曆 | ? 說明 | , 設定 | q 離開",
        ]
    );
    // ☑ uses the `checked` color, names and the header are normal text, numbers are muted, the border is a
    // normal line; a switch that is off: ☐ muted, its name normal text
    let theme = Theme::default();
    let buffer = terminal.backend().buffer();
    let cell = |x: u16, y: u16| (buffer[(x, y)].symbol(), buffer[(x, y)].fg);
    assert_eq!(cell(2, 7), ("☑", theme.checked));
    assert_eq!(cell(4, 8), ("P", theme.text));
    assert_eq!(cell(35, 8), ("5", theme.muted));
    assert_eq!(cell(2, 4), ("☐", theme.muted));
    assert_eq!(cell(4, 4), ("隱", theme.text));
    assert_eq!(cell(2, 2), ("日", theme.text));
    assert_eq!(cell(0, 1), ("╭", theme.line));
    // `s` hides it: the calendar moves back to the far left,
    // and the key hints still have `s` (press it again to open)
    app.side_hidden = true;
    assert_eq!(
        text_rows(draw_app(&mut app, 128, 15).backend().buffer()),
        [
            " ┃ 1 月曆   2 大月曆   3 任務",
            "╭────────────────────────────────────────╮╭────────────────────────────────────────────────────────────────────────────────────╮",
            "│2026年 週 │ 一  二  三  四  五  六  日  ││ 重要事項                                                          2026/10/17（六） │",
            "│──────────┼─────────────────────────────││────────────────────────────────────────────────────────────────────────────────────│",
            "│  10月 40 │              1   2   3   4  ││ 10/10 Farmers market                                                               │",
            "│       41 │  5⠂  6   7   8   9  10⠂ 11  ││ 10/15 Fix the login bug                                                            ┃",
            "│       42 │ 12  13  14  15⠂ 16⠂[17⠒]18⠂ ││ 10/16 17:30 Send the weekly update                                                 ┃",
            "│       43 │ 19  20⠂ 21  22  23  24  25  ││ 10/17 Dinner with friends                                                          ┃",
            "│       44 │ 26  27  28  29  30  31      ││ 10/17 08:00 Coffee chat                                                            ┃",
            "│          │                             ││ 10/18 07:00 Go hiking                                                              │",
            "│──────────┼─────────────────────────────││──────────────────────────────────────────────────────────────────────── 還有 1 件 ─│",
            "│  11月 44 │                          1  ││  11/3 09:30 Release v2.3                                                           │",
            "│       45 │  2   3⠂  4   5   6   7   8  ││                                                                                    │",
            "╰────────────────────────────────────────╯╰────────────────────────────────────────────────────────────────────────────────────╯",
            " hjkl[] 移動 | Tab 切換 | t 今天 | s 日曆 | ? 說明 | , 設定 | q 離開",
        ]
    );
}

#[test]
fn unchecked_calendars_hide_their_items() {
    // Personal unchecked: ☐ and the name are muted, the number still counts; its 5 items are not listed
    let mut app = agenda_app(Lang::En);
    app.groups[1].shown = false;
    let terminal = draw_app(&mut app, 128, 15);
    assert_eq!(
        text_rows(terminal.backend().buffer()),
        [
            " ┃ 1 calendar   2 month   3 tasks",
            "╭────────────────────────────────────╮╭────────────────────────────────────────╮╭──────────────────────────────────────────────╮",
            "│ Calendars                          ││  2026 Wk │ Mo  Tu  We  Th  Fr  Sa  Su  ││ Agenda                      Sat Oct 17, 2026 │",
            "│────────────────────────────────────││──────────┼─────────────────────────────││──────────────────────────────────────────────│",
            "│ ☐\u{a0}Hide all                         ││   Oct 40 │              1   2   3   4  ││ 10/15 Fix the login bug                      │",
            "│ ☐\u{a0}Hide done                        ││       41 │  5   6   7   8   9  10  11  ││ 10/16 17:30 Send the weekly update           │",
            "│────────────────────────────────────││       42 │ 12  13  14  15⠂ 16⠂[17] 18  ││ 10/20 A really long title that just keeps goi│",
            "│ ☑\u{a0}Ungrouped                      2 ││       43 │ 19  20⠂ 21  22  23  24  25  ││                                              │",
            "│ ☐\u{a0}Personal                       5 ││       44 │ 26  27  28  29  30  31      ││                                              │",
            "│ ☑\u{a0}Work                           1 ││──────────┼─────────────────────────────││──────────────────────────────────────────────│",
            "│   ☑\u{a0}Year-end odds and ends and p 2 ││   Nov 44 │                          1  ││  11/3 09:30 Release v2.3                     │",
            "│                                    ││       45 │  2   3⠂  4   5   6   7   8  ││                                              │",
            "│                                    ││       46 │  9  10  11  12  13  14  15  ││                                              │",
            "╰────────────────────────────────────╯╰────────────────────────────────────────╯╰──────────────────────────────────────────────╯",
            " hjkl[] move | Tab pane | t today | s calendars | ? help | , settings | q quit",
        ]
    );
    let theme = Theme::default();
    let buffer = terminal.backend().buffer();
    let cell = |x: u16, y: u16| (buffer[(x, y)].symbol(), buffer[(x, y)].fg);
    assert_eq!(cell(2, 8), ("☐", theme.muted));
    assert_eq!(cell(4, 8), ("P", theme.muted));
    assert_eq!(cell(2, 7), ("☑", theme.checked));
    // Dates keep their usual colors whatever items they have: 10/5 (Monday), 10/16 (Friday, an
    // ungrouped item) normal, 10/10 (Saturday) weekend blue
    assert_eq!(cell(52, 5), ("5", theme.text));
    assert_eq!(cell(72, 5), ("0", theme.weekend));
    assert_eq!(cell(68, 6), ("6", theme.text));
}

#[test]
fn archived_calendars_get_their_own_part_at_the_bottom_of_the_side_menu() {
    // testdata/archived.md: Work > Payments API v2, Tokyo trip (with Packing list inside it) and 2025 holidays
    // are archived: under the rule 「─── 已封存 ───」, unchecked, so their items (10/2 Release v2.3) are not
    // listed; Payments API v2 sits inside a running calendar, so it starts at the left of the archive part
    let mut app = App::new(
        Lang::ZhTw,
        date(2026, 10, 17),
        crate::tasks::parse(ARCHIVED_TASKS).items,
    );
    app.screen = Rect::new(0, 0, 128, 21);
    let terminal = draw_app(&mut app, 128, 21);
    assert_eq!(
        text_rows(terminal.backend().buffer()),
        [
            " ┃ 1 月曆   2 大月曆   3 任務",
            "╭─────────────────────╮╭────────────────────────────────────────╮╭─────────────────────────────────────────────────────────────╮",
            "│ 日曆                ││2026年 週 │ 一  二  三  四  五  六  日  ││ 重要事項                                   2026/10/17（六） │",
            "│─────────────────────││──────────┼─────────────────────────────││─────────────────────────────────────────────────────────────│",
            "│ ☐\u{a0}隱藏全部          ││  10月 40 │              1   2   3   4  ││ 10/14 Index the old orders                                  │",
            "│ ☐\u{a0}隱藏做完的        ││       41 │  5   6   7   8   9  10  11  ││ 10/14 10:30 Dentist appointment                             │",
            "│─────────────────────││       42 │ 12  13  14⠒ 15  16 [17] 18  ││ 10/20 Ship the v2.4 release                                 │",
            "│ ☑\u{a0}Work            1 ││       43 │ 19  20⠂ 21⠂ 22  23  24  25  ││ 10/21 10:00 Design review                                   │",
            "│   ☑\u{a0}Search revamp 2 ││       44 │ 26  27  28  29  30  31      ││                                                             │",
            "│ ☑\u{a0}Personal        1 ││──────────┼─────────────────────────────││─────────────────────────────────────────────────────────────│",
            "│────── 已封存 ───────││  11月 44 │                          1  ││                                                             │",
            "│ ☐\u{a0}Payments API v2 2 ││       45 │  2   3   4   5   6   7   8  ││                                                             │",
            "│ ☐\u{a0}Tokyo trip      1 ││       46 │  9  10  11  12  13  14  15  ││                                                             │",
            "│   ☐\u{a0}Packing list  0 ││       47 │ 16  17  18  19  20  21  22  ││                                                             │",
            "│ ☐\u{a0}2025 holidays   1 ││       48 │ 23  24  25  26  27  28  29  ││                                                             │",
            "│                     ││       49 │ 30                          ││                                                             │",
            "│                     ││──────────┼─────────────────────────────││─────────────────────────────────────────────────────────────│",
            "│                     ││  12月 49 │      1   2   3   4   5   6  ││                                                             │",
            "│                     ││       50 │  7   8   9  10  11  12  13  ││                                                             │",
            "╰─────────────────────╯╰────────────────────────────────────────╯╰─────────────────────────────────────────────────────────────╯",
            " hjkl[] 移動 | Tab 切換 | t 今天 | s 日曆 | ? 說明 | , 設定 | q 離開",
        ]
    );
    // The rule (y 10) is a line with the muted name in the middle of the 21 cells inside the box: 6 `─`
    // (x 1-6), 「 已封存 」 (x 7-14), 7 `─` (x 15-21)
    let theme = Theme::default();
    let buffer = terminal.backend().buffer();
    let cell = |x: u16, y: u16| (buffer[(x, y)].symbol(), buffer[(x, y)].fg);
    assert_eq!(cell(1, 10), ("─", theme.line));
    assert_eq!(cell(6, 10), ("─", theme.line));
    assert_eq!(cell(8, 10), ("已", theme.muted));
    assert_eq!(cell(15, 10), ("─", theme.line));
    assert_eq!(cell(21, 10), ("─", theme.line));
    // Clicks: the rule hits nothing, the rows under it the archived calendars (3 = the first archived one in
    // `App::groups`), the blank rows below them nothing
    assert_eq!(click_at(&app, 5, 9), Some(Click::Group(2)));
    assert_eq!(click_at(&app, 5, 10), None);
    assert_eq!(click_at(&app, 5, 11), Some(Click::Group(3)));
    assert_eq!(click_at(&app, 5, 14), Some(Click::Group(6)));
    assert_eq!(click_at(&app, 5, 15), None);
    // Payments API v2 checked: its item is back
    app.lang = Lang::En;
    app.groups[3].shown = true;
    let terminal = draw_app(&mut app, 128, 21);
    assert_eq!(
        text_rows(terminal.backend().buffer()),
        [
            " ┃ 1 calendar   2 month   3 tasks",
            "╭─────────────────────╮╭────────────────────────────────────────╮╭─────────────────────────────────────────────────────────────╮",
            "│ Calendars           ││  2026 Wk │ Mo  Tu  We  Th  Fr  Sa  Su  ││ Agenda                                     Sat Oct 17, 2026 │",
            "│─────────────────────││──────────┼─────────────────────────────││─────────────────────────────────────────────────────────────│",
            "│ ☐\u{a0}Hide all          ││   Oct 40 │              1   2⠂  3   4  ││  10/2 Release v2.3                                          │",
            "│ ☐\u{a0}Hide done         ││       41 │  5   6   7   8   9  10  11  ││ 10/14 Index the old orders                                  │",
            "│─────────────────────││       42 │ 12  13  14⠒ 15  16 [17] 18  ││ 10/14 10:30 Dentist appointment                             │",
            "│ ☑\u{a0}Work            1 ││       43 │ 19  20⠂ 21⠂ 22  23  24  25  ││ 10/20 Ship the v2.4 release                                 │",
            "│   ☑\u{a0}Search revamp 2 ││       44 │ 26  27  28  29  30  31      ││ 10/21 10:00 Design review                                   │",
            "│ ☑\u{a0}Personal        1 ││──────────┼─────────────────────────────││─────────────────────────────────────────────────────────────│",
            "│───── Archived ──────││   Nov 44 │                          1  ││                                                             │",
            "│ ☑\u{a0}Payments API v2 2 ││       45 │  2   3   4   5   6   7   8  ││                                                             │",
            "│ ☐\u{a0}Tokyo trip      1 ││       46 │  9  10  11  12  13  14  15  ││                                                             │",
            "│   ☐\u{a0}Packing list  0 ││       47 │ 16  17  18  19  20  21  22  ││                                                             │",
            "│ ☐\u{a0}2025 holidays   1 ││       48 │ 23  24  25  26  27  28  29  ││                                                             │",
            "│                     ││       49 │ 30                          ││                                                             │",
            "│                     ││──────────┼─────────────────────────────││─────────────────────────────────────────────────────────────│",
            "│                     ││   Dec 49 │      1   2   3   4   5   6  ││                                                             │",
            "│                     ││       50 │  7   8   9  10  11  12  13  ││                                                             │",
            "╰─────────────────────╯╰────────────────────────────────────────╯╰─────────────────────────────────────────────────────────────╯",
            " hjkl[] move | Tab pane | t today | s calendars | ? help | , settings | q quit",
        ]
    );
}

#[test]
fn the_running_calendars_take_at_most_30_rows_above_the_archive_part() {
    use ratatui::crossterm::event::{KeyCode, KeyEvent};
    // 121 × 46: under the switches the calendars get 37 rows (y 7-43). 33 running calendars r01-r33 and 10
    // archived ones a01-a10 (1 item each): the running ones take 30 rows (y 7-36) and scroll, the rule is
    // on y 37, the archived ones get the 6 rows left (y 38-43) and scroll on their own
    let running: String = (1..=33)
        .map(|n| format!("# r{n:02}\n- a @2026-10-01\n"))
        .collect();
    let archived: String = (1..=10)
        .map(|n| format!("# a{n:02} archived:2026-10-01\n- a @2026-10-01\n"))
        .collect();
    let mut app = App::new(
        Lang::ZhTw,
        date(2026, 10, 17),
        crate::tasks::parse(&format!("{running}{archived}")).items,
    );
    app.screen = Rect::new(0, 0, 121, 46);
    app.focus = Focus::Side;
    // The names on rows `y` to `y + height - 1` (split 「│ ☑ r01 … 1」 on whitespace: the 3rd part), and
    // the rows among them with the scrollbar's thumb on the right border
    let names = |app: &mut App, y: u16, height: u16| {
        let terminal = draw_app(app, 121, 46);
        let buffer = terminal.backend().buffer();
        let names: Vec<String> = area_rows(buffer, Rect::new(0, y, 21, height))
            .iter()
            .map(|row| row.split_whitespace().nth(2).unwrap_or("").to_string())
            .collect();
        let thumb: Vec<u16> = (y..y + height)
            .filter(|&row| buffer[(21, row)].symbol() == "┃")
            .collect();
        (names, thumb)
    };
    let numbered = |prefix: &str, numbers: std::ops::RangeInclusive<u32>| -> Vec<String> {
        numbers.map(|n| format!("{prefix}{n:02}")).collect()
    };
    // At start: r01-r30 with the thumb at the top, the rule, a01-a06 with their own thumb at the top
    let (rows, thumb) = names(&mut app, 7, 30);
    assert_eq!(rows, numbered("r", 1..=30));
    assert_eq!(thumb.first(), Some(&7));
    let terminal = draw_app(&mut app, 121, 46);
    let rule = &area_rows(terminal.backend().buffer(), Rect::new(0, 37, 22, 1))[0];
    assert_eq!(rule, "│────── 已封存 ──────│");
    let (rows, thumb) = names(&mut app, 38, 6);
    assert_eq!(rows, numbered("a", 1..=6));
    assert_eq!(thumb.first(), Some(&38));
    // G to the last archived one: only the archive part scrolls (a05-a10, its thumb at the bottom)
    app.handle_key(KeyEvent::from(KeyCode::Char('G')));
    let (rows, thumb) = names(&mut app, 38, 6);
    assert_eq!(rows, numbered("a", 5..=10));
    assert_eq!(thumb.last(), Some(&43));
    assert_eq!(names(&mut app, 7, 30).0, numbered("r", 1..=30));
    // A click on the archive part's first row hits a05 (33 running ones come before it in `App::groups`); the
    // rule hits nothing
    assert_eq!(click_at(&app, 5, 38), Some(Click::Group(33 + 4)));
    assert_eq!(click_at(&app, 5, 37), None);
    // The last running one (r33): the running part scrolls (r04-r33), the archive part stays
    app.side_row = SideRow::Calendar(32);
    assert_eq!(names(&mut app, 7, 30).0, numbered("r", 4..=33));
    assert_eq!(names(&mut app, 38, 6).0, numbered("a", 5..=10));
    // j from there goes on into the archive part: a01, which scrolls it back up
    app.handle_key(KeyEvent::from(KeyCode::Char('j')));
    assert_eq!(app.side_row, SideRow::Calendar(33));
    assert_eq!(names(&mut app, 38, 6).0, numbered("a", 1..=6));
    // A short box (121 × 13: 4 rows under the switches): the running part keeps 2 rows (still scrolled to r04),
    // then the rule and 1 archived row (a01, chosen)
    let terminal = draw_app(&mut app, 121, 13);
    let rows = area_rows(terminal.backend().buffer(), Rect::new(0, 7, 22, 4));
    assert!(rows[0].starts_with("│ ☑\u{a0}r04 "), "{}", rows[0]);
    assert!(rows[1].starts_with("│ ☑\u{a0}r05 "), "{}", rows[1]);
    assert_eq!(rows[2], "│────── 已封存 ──────│");
    assert!(rows[3].starts_with("│ ☐\u{a0}a01 "), "{}", rows[3]);
    // Without archived calendars there is no rule and no 30-row cap: all 33 fit in the 37 rows
    let mut app = App::new(
        Lang::ZhTw,
        date(2026, 10, 17),
        crate::tasks::parse(&running).items,
    );
    let (rows, thumb) = names(&mut app, 7, 37);
    assert_eq!(rows[..33], numbered("r", 1..=33));
    assert!(rows[33..].iter().all(String::is_empty));
    assert!(thumb.is_empty());
}

#[test]
fn a_checked_calendar_inside_an_unchecked_one_still_shows() {
    // Work unchecked on its own: SFAP inside it keeps its ☑ in the usual colors and its item (a) is still
    // listed; only Work's own items would be hidden
    let mut app = App::new(Lang::En, date(2026, 10, 17), Vec::new());
    app.set_items(crate::tasks::parse(
        "# Work\n## SFAP\n- a @2026-10-10\n# Personal\n- b @2026-10-11\n",
    ));
    app.groups[0].shown = false;
    let terminal = draw_app(&mut app, 121, 15);
    let buffer = terminal.backend().buffer();
    assert_eq!(
        area_rows(buffer, Rect::new(0, 7, 22, 3)),
        [
            "│ ☐\u{a0}Work           0 │",
            "│   ☑\u{a0}SFAP         1 │",
            "│ ☑\u{a0}Personal       1 │",
        ]
    );
    let theme = Theme::default();
    let cell = |x: u16, y: u16| (buffer[(x, y)].symbol(), buffer[(x, y)].fg);
    assert_eq!(cell(2, 7), ("☐", theme.muted));
    assert_eq!(cell(4, 8), ("☑", theme.checked));
    assert_eq!(cell(6, 8), ("S", theme.text));
    assert_eq!(cell(2, 9), ("☑", theme.checked));
    let titles: Vec<&str> = app
        .agenda()
        .iter()
        .map(|entry| entry.item.title.as_str())
        .collect();
    assert_eq!(titles, ["a", "b"]);
}

#[test]
fn side_menu_focus_lights_its_border_and_the_chosen_calendar() {
    // Keyboard in the side menu, the 2nd one chosen (Personal): the side menu's border is lit, the
    // calendar's is not; that whole row gets the highlight background, text colors unchanged
    let mut app = agenda_app(Lang::ZhTw);
    app.focus = Focus::Side;
    app.side_row = SideRow::Calendar(1);
    let terminal = draw_app(&mut app, 121, 15);
    let theme = Theme::default();
    let buffer = terminal.backend().buffer();
    assert_eq!(buffer[(0, 1)].fg, theme.focus);
    assert_eq!(buffer[(38, 1)].fg, theme.line);
    for x in 1..37 {
        assert_eq!(buffer[(x, 8)].bg, theme.selection, "x = {x}");
        assert_ne!(buffer[(x, 7)].bg, theme.selection, "x = {x}");
    }
    assert_eq!(
        (buffer[(2, 8)].symbol(), buffer[(2, 8)].fg),
        ("☑", theme.checked)
    );
    // The 「隱藏全部」 row chosen: it gets the background, the calendars do not
    app.side_row = SideRow::HideAll;
    let terminal = draw_app(&mut app, 121, 15);
    let buffer = terminal.backend().buffer();
    assert_eq!(buffer[(1, 4)].bg, theme.selection);
    assert_ne!(buffer[(1, 5)].bg, theme.selection);
    assert_ne!(buffer[(1, 8)].bg, theme.selection);
}

#[test]
fn click_positions_in_the_side_menu() {
    // 121 × 15 (screen: see `side_menu_lists_the_calendars_when_wide`): the switches at y 4 and 5, a
    // rule, then the calendars from y 7, one per row
    let mut app = agenda_app(Lang::ZhTw);
    app.screen = Rect::new(0, 0, 121, 15);
    draw_app(&mut app, 121, 15);
    assert_eq!(click_at(&app, 5, 4), Some(Click::HideAll));
    assert_eq!(click_at(&app, 30, 5), Some(Click::HideDone));
    assert_eq!(click_at(&app, 5, 7), Some(Click::Group(0)));
    assert_eq!(click_at(&app, 1, 10), Some(Click::Group(3)));
    assert_eq!(click_at(&app, 20, 8), Some(Click::Group(1)));
    // No calendar: the blank space below, the header, the header line, the switches' rule, the left and
    // right borders, the key hints
    for (x, y) in [(5, 11), (5, 2), (5, 3), (5, 6), (0, 7), (37, 7), (5, 14)] {
        assert_eq!(click_at(&app, x, y), None, "({x}, {y})");
    }
    // The calendar moved 38 cells right: the 16th is at x 66-69
    assert_eq!(date_at(&app, 66, 6), Some(date(2026, 10, 16)));
    assert_eq!(date_at(&app, 70, 6), Some(date(2026, 10, 17)));
}

#[test]
fn agenda_lists_each_months_items_next_to_it() {
    // One agenda column: October's 8 items outnumber its 5 weeks, so the month grows 3 rows down; sorted
    // by date and time (on the same day, items without a time first); items without a date are not
    // listed; titles that are too long are cut
    let mut app = agenda_app(Lang::ZhTw);
    assert_eq!(
        text_rows(draw_app(&mut app, 77, 23).backend().buffer()),
        [
            " ┃ 1 月曆   2 大月曆   3 任務",
            "╭────────────────────────────────────────╮╭─────────────────────────────────╮",
            "│2026年 週 │ 一  二  三  四  五  六  日  ││ 重要事項       2026/10/17（六） │",
            "│──────────┼─────────────────────────────││─────────────────────────────────│",
            "│  10月 40 │              1   2   3   4  ││  10/5 Pay the phone bill        │",
            "│       41 │  5⠂  6   7   8   9  10⠂ 11  ││ 10/10 Farmers market            │",
            "│       42 │ 12  13  14  15⠂ 16⠂[17⠒]18⠂ ││ 10/15 Fix the login bug         │",
            "│       43 │ 19  20⠂ 21  22  23  24  25  ││ 10/16 17:30 Send the weekly upda│",
            "│       44 │ 26  27  28  29  30  31      ││ 10/17 Dinner with friends       │",
            "│          │                             ││ 10/17 08:00 Coffee chat         │",
            "│          │                             ││ 10/18 07:00 Go hiking           │",
            "│          │                             ││ 10/20 A really long title that j│",
            "│──────────┼─────────────────────────────││─────────────────────────────────│",
            "│  11月 44 │                          1  ││  11/3 09:30 Release v2.3        │",
            "│       45 │  2   3⠂  4   5   6   7   8  ││                                 │",
            "│       46 │  9  10  11  12  13  14  15  ││                                 │",
            "│       47 │ 16  17  18  19  20  21  22  ││                                 │",
            "│       48 │ 23  24  25  26  27  28  29  ││                                 │",
            "│       49 │ 30                          ││                                 │",
            "│──────────┼─────────────────────────────││─────────────────────────────────│",
            "│  12月 49 │      1   2   3   4   5   6  ││ 12/31 23:00 New Year's Eve count│",
            "╰────────────────────────────────────────╯╰─────────────────────────────────╯",
            " hjkl[] 移動 | Tab 切換 | t 今天 | s 日曆 | ? 說明 | , 設定 | q 離開",
        ]
    );
}

#[test]
fn the_calendar_without_dots() {
    // Config `[calendar] dots = false`: no dots right of the dates, the selected day reads `[17]`; the
    // calendar keeps its width (the cell after each date stays blank), the agenda is unchanged
    let mut app = agenda_app(Lang::ZhTw);
    app.settings.calendar.dots = false;
    assert_eq!(
        text_rows(draw_app(&mut app, 77, 23).backend().buffer()),
        [
            " ┃ 1 月曆   2 大月曆   3 任務",
            "╭────────────────────────────────────────╮╭─────────────────────────────────╮",
            "│2026年 週 │ 一  二  三  四  五  六  日  ││ 重要事項       2026/10/17（六） │",
            "│──────────┼─────────────────────────────││─────────────────────────────────│",
            "│  10月 40 │              1   2   3   4  ││  10/5 Pay the phone bill        │",
            "│       41 │  5   6   7   8   9  10  11  ││ 10/10 Farmers market            │",
            "│       42 │ 12  13  14  15  16 [17] 18  ││ 10/15 Fix the login bug         │",
            "│       43 │ 19  20  21  22  23  24  25  ││ 10/16 17:30 Send the weekly upda│",
            "│       44 │ 26  27  28  29  30  31      ││ 10/17 Dinner with friends       │",
            "│          │                             ││ 10/17 08:00 Coffee chat         │",
            "│          │                             ││ 10/18 07:00 Go hiking           │",
            "│          │                             ││ 10/20 A really long title that j│",
            "│──────────┼─────────────────────────────││─────────────────────────────────│",
            "│  11月 44 │                          1  ││  11/3 09:30 Release v2.3        │",
            "│       45 │  2   3   4   5   6   7   8  ││                                 │",
            "│       46 │  9  10  11  12  13  14  15  ││                                 │",
            "│       47 │ 16  17  18  19  20  21  22  ││                                 │",
            "│       48 │ 23  24  25  26  27  28  29  ││                                 │",
            "│       49 │ 30                          ││                                 │",
            "│──────────┼─────────────────────────────││─────────────────────────────────│",
            "│  12月 49 │      1   2   3   4   5   6  ││ 12/31 23:00 New Year's Eve count│",
            "╰────────────────────────────────────────╯╰─────────────────────────────────╯",
            " hjkl[] 移動 | Tab 切換 | t 今天 | s 日曆 | ? 說明 | , 設定 | q 離開",
        ]
    );
}

#[test]
fn the_agenda_stays_one_column_when_wide() {
    // Agenda 58 cells wide: still one column. October's 8 items need more than its 6 rows (2/3 of the
    // calendar's 9), so its agenda scrolls to keep 10/17's two items in view with one row after them:
    // it starts at the 2nd item
    let mut app = agenda_app(Lang::En);
    assert_eq!(
        text_rows(draw_app(&mut app, 102, 15).backend().buffer()),
        [
            " ┃ 1 calendar   2 month   3 tasks",
            "╭────────────────────────────────────────╮╭──────────────────────────────────────────────────────────╮",
            "│  2026 Wk │ Mo  Tu  We  Th  Fr  Sa  Su  ││ Agenda                                  Sat Oct 17, 2026 │",
            "│──────────┼─────────────────────────────││──────────────────────────────────────────────────────────│",
            "│   Oct 40 │              1   2   3   4  ││ 10/10 Farmers market                                     │",
            "│       41 │  5⠂  6   7   8   9  10⠂ 11  ││ 10/15 Fix the login bug                                  ┃",
            "│       42 │ 12  13  14  15⠂ 16⠂[17⠒]18⠂ ││ 10/16 17:30 Send the weekly update                       ┃",
            "│       43 │ 19  20⠂ 21  22  23  24  25  ││ 10/17 Dinner with friends                                ┃",
            "│       44 │ 26  27  28  29  30  31      ││ 10/17 08:00 Coffee chat                                  ┃",
            "│          │                             ││ 10/18 07:00 Go hiking                                    │",
            "│──────────┼─────────────────────────────││───────────────────────────────────────────────── 1 more ─│",
            "│   Nov 44 │                          1  ││  11/3 09:30 Release v2.3                                 │",
            "│       45 │  2   3⠂  4   5   6   7   8  ││                                                          │",
            "╰────────────────────────────────────────╯╰──────────────────────────────────────────────────────────╯",
            " hjkl[] move | Tab pane | t today | s calendars | ? help | , settings | q quit",
        ]
    );
}

#[test]
fn clicking_an_item_selects_its_day() {
    // One column (screen: see `agenda_lists_each_months_items_next_to_it`):
    // the inside of the agenda box starts at x 43
    let mut app = agenda_app(Lang::ZhTw);
    app.screen = Rect::new(0, 0, 77, 24);
    draw_app(&mut app, 77, 24);
    assert_eq!(date_at(&app, 47, 4), Some(date(2026, 10, 5)));
    assert_eq!(date_at(&app, 75, 9), Some(date(2026, 10, 17)));
    assert_eq!(date_at(&app, 47, 13), Some(date(2026, 11, 3)));
    // Agenda rows without an item, and the header; on the
    // left, the rows that exist only for items have no date
    assert_eq!(date_at(&app, 47, 14), None);
    assert_eq!(date_at(&app, 47, 2), None);
    assert_eq!(date_at(&app, 20, 9), None);
    // Which agenda item was clicked (in `App::agenda()` order)
    assert_eq!(click_at(&app, 47, 4), Some(Click::Item(0)));
    assert_eq!(click_at(&app, 20, 6), Some(Click::Day(date(2026, 10, 14))));
    // Wide, still one column (screen: see `the_agenda_stays_one_column_when_wide`, scrolled one row): a
    // row is one item all the way across, the 2nd item (10/10) at y 4, the 7th (10/18) at y 9
    let mut app = agenda_app(Lang::En);
    app.screen = Rect::new(0, 0, 102, 16);
    draw_app(&mut app, 102, 16);
    assert_eq!(date_at(&app, 47, 4), Some(date(2026, 10, 10)));
    assert_eq!(date_at(&app, 98, 4), Some(date(2026, 10, 10)));
    assert_eq!(date_at(&app, 47, 9), Some(date(2026, 10, 18)));
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
        crate::tasks::parse(include_str!("../../testdata/tasks.md")).items,
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
        // The calendar's only `[` is the selected day's (its `]` comes after its dots, if it has any)
        let day = format!("[{}", app.selected.day());
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
fn item_colors_follow_the_item() {
    // Screen: see `agenda_lists_each_months_items_next_to_it`;
    // the inside of the agenda box starts at x 43
    let mut app = agenda_app(Lang::ZhTw);
    let terminal = draw_app(&mut app, 77, 24);
    let buffer = terminal.backend().buffer();
    let theme = Theme::default();
    // The (x, y) cell: symbol, foreground color, whether it is struck through
    let cell = |x: u16, y: u16| {
        let cell = &buffer[(x, y)];
        (
            cell.symbol(),
            cell.fg,
            cell.modifier.contains(Modifier::CROSSED_OUT),
        )
    };
    // Agenda header: 「重要事項」 and the selected day on the right both in the normal color (2026/10/17
    // starts at x 59)
    assert_eq!(cell(44, 2), ("重", theme.text, false));
    assert_eq!(cell(59, 2), ("2", theme.text, false));
    // 10/5 has an unfinished item: date and title in the normal color
    assert_eq!(cell(45, 4), ("1", theme.text, false));
    assert_eq!(cell(50, 4), ("P", theme.text, false));
    // 10/15's finished item: the whole line is gray (the date too), the title struck through
    assert_eq!(cell(44, 6), ("1", theme.muted, false));
    assert_eq!(cell(50, 6), ("F", theme.muted, true));
    // 10/17 is today and 10/18 a Sunday, but the line follows the item, not its day: the normal color;
    // the time is muted
    assert_eq!(cell(50, 8), ("D", theme.text, false));
    assert_eq!(cell(50, 9), ("0", theme.muted, false));
    assert_eq!(cell(56, 9), ("C", theme.text, false));
    assert_eq!(cell(56, 10), ("G", theme.text, false));
}

#[test]
fn the_selected_week_row_has_a_background() {
    // The week of 10/17 is at y 6: that whole row in the calendar box (x 1-40) gets the highlight
    // background; the rows above and below, and the same row in the agenda, do not
    let mut app = agenda_app(Lang::ZhTw);
    let theme = Theme::default();
    let bg =
        |terminal: &Terminal<TestBackend>, x: u16, y: u16| terminal.backend().buffer()[(x, y)].bg;
    let terminal = draw_app(&mut app, 77, 24);
    for x in 1..=40 {
        assert_eq!(bg(&terminal, x, 6), theme.selection, "x {x}");
    }
    assert_eq!(bg(&terminal, 0, 6), Color::Reset);
    assert_eq!(bg(&terminal, 20, 5), Color::Reset);
    assert_eq!(bg(&terminal, 20, 7), Color::Reset);
    assert_eq!(bg(&terminal, 47, 6), Color::Reset);
    // The week that crosses months is split into two rows: with 10/31 selected, only the October half
    // (y 8) gets the highlight, the November half (y 13) does not
    app.selected = date(2026, 10, 31);
    let terminal = draw_app(&mut app, 77, 24);
    assert_eq!(bg(&terminal, 20, 8), theme.selection);
    assert_eq!(bg(&terminal, 20, 13), Color::Reset);
}

#[test]
fn agenda_focus_highlights_the_chosen_item() {
    // Keyboard in the agenda, the 6th item chosen (10/17 08:00 Coffee chat): no brackets, only a background
    let mut app = agenda_app(Lang::ZhTw);
    app.focus = Focus::Agenda;
    app.agenda_index = 5;
    let terminal = draw_app(&mut app, 77, 24);
    assert_eq!(
        text_rows(terminal.backend().buffer()),
        [
            " ┃ 1 月曆   2 大月曆   3 任務",
            "╭────────────────────────────────────────╮╭─────────────────────────────────╮",
            "│2026年 週 │ 一  二  三  四  五  六  日  ││ 重要事項       2026/10/17（六） │",
            "│──────────┼─────────────────────────────││─────────────────────────────────│",
            "│  10月 40 │              1   2   3   4  ││  10/5 Pay the phone bill        │",
            "│       41 │  5⠂  6   7   8   9  10⠂ 11  ││ 10/10 Farmers market            │",
            "│       42 │ 12  13  14  15⠂ 16⠂[17⠒]18⠂ ││ 10/15 Fix the login bug         │",
            "│       43 │ 19  20⠂ 21  22  23  24  25  ││ 10/16 17:30 Send the weekly upda│",
            "│       44 │ 26  27  28  29  30  31      ││ 10/17 Dinner with friends       │",
            "│          │                             ││ 10/17 08:00 Coffee chat         │",
            "│          │                             ││ 10/18 07:00 Go hiking           │",
            "│          │                             ││ 10/20 A really long title that j│",
            "│──────────┼─────────────────────────────││─────────────────────────────────│",
            "│  11月 44 │                          1  ││  11/3 09:30 Release v2.3        │",
            "│       45 │  2   3⠂  4   5   6   7   8  ││                                 │",
            "│       46 │  9  10  11  12  13  14  15  ││                                 │",
            "│       47 │ 16  17  18  19  20  21  22  ││                                 │",
            "│       48 │ 23  24  25  26  27  28  29  ││                                 │",
            "│       49 │ 30                          ││                                 │",
            "│──────────┼─────────────────────────────││─────────────────────────────────│",
            "│  12月 49 │      1   2   3   4   5   6  ││ 12/31 23:00 New Year's Eve count│",
            "│       50 │  7   8   9  10  11  12  13  ││                                 │",
            "╰────────────────────────────────────────╯╰─────────────────────────────────╯",
            " hjkl[] 移動 | Tab 切換 | t 今天 | s 日曆 | ? 說明 | , 設定 | q 離開",
        ]
    );
    let buffer = terminal.backend().buffer();
    let theme = Theme::default();
    // The agenda's border lights up, and the calendar box's goes back to a normal line
    assert_eq!(buffer[(42, 1)].fg, theme.focus);
    assert_eq!(buffer[(0, 1)].fg, theme.line);
    // Its date keeps the line's color (no bracket in the selected color); the chosen item gets the
    // highlight across the whole column (x 43-75), the items above and below do not
    assert_eq!(
        (buffer[(44, 9)].symbol(), buffer[(44, 9)].fg),
        ("1", theme.text)
    );
    // The second half of a wide (CJK) character is not sent to the terminal, so skip it (on a real
    // terminal the background covers both cells)
    let mut x = 43;
    while x <= 75 {
        assert_eq!(buffer[(x, 9)].bg, theme.selection, "x {x}");
        x += u16::try_from(Span::raw(buffer[(x, 9)].symbol()).width().max(1)).unwrap();
    }
    assert_eq!(buffer[(47, 8)].bg, Color::Reset);
    assert_eq!(buffer[(47, 10)].bg, Color::Reset);
}

#[test]
fn the_chosen_item_is_always_on_screen() {
    // Sample task file: October has far more items than weeks (a month does not fit, so the agenda
    // scrolls inside it); choose items one by one going down, then going up; the chosen item (its
    // background) and the selected day on the calendar both stay on screen
    let mut app = App::new(
        Lang::En,
        date(2026, 10, 17),
        crate::tasks::parse(include_str!("../../testdata/tasks.md")).items,
    );
    app.focus = Focus::Agenda;
    let count = app.agenda().len();
    for index in (0..count).chain((0..count).rev()) {
        app.agenda_index = index;
        app.selected = app.agenda()[index].date;
        let first = app.agenda()[index].item.date.expect("has a date");
        let terminal = draw_app(&mut app, 70, 13);
        let buffer = terminal.backend().buffer();
        let rows = text_rows(buffer);
        let body = &rows[3..rows.len() - 2];
        // The chosen item's row: the agenda's last cell inside the border (x 68) has the selection
        // background, and the row has its date (for a multi-day item, the range starts with it)
        let chosen = format!(" {}/{}", first.month(), first.day());
        assert!(
            (3..rows.len() - 2).any(|y| {
                let y16 = u16::try_from(y).unwrap();
                buffer[(68, y16)].bg == Theme::default().selection && rows[y].contains(&chosen)
            }),
            "item {index} is not on screen: {body:#?}"
        );
        // The selected day is also visible in the calendar box (left of `││` in each row; its `]` comes
        // after its dots, if it has any)
        let day = format!("[{}", app.selected.day());
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
    app.screen = Rect::new(0, 0, 70, 12);
    app.focus = Focus::Agenda;
    app.agenda_index = 7;
    app.selected = date(2026, 10, 20);
    let terminal = draw_app(&mut app, 77, 12);
    assert_eq!(
        text_rows(terminal.backend().buffer()),
        [
            " ┃ 1 月曆   2 大月曆   3 任務",
            "╭────────────────────────────────────────╮╭─────────────────────────────────╮",
            "│2026年 週 │ 一  二  三  四  五  六  日  ││ 重要事項       2026/10/20（二） │",
            "│──────────┼─────────────────────────────││─────────────────────────────────│",
            "│  10月 40 │              1   2   3   4  ││ 10/16 17:30 Send the weekly upda│",
            "│       41 │  5⠂  6   7   8   9  10⠂ 11  ││ 10/17 Dinner with friends       │",
            "│       42 │ 12  13  14  15⠂ 16⠂ 17⠒ 18⠂ ││ 10/17 08:00 Coffee chat         ┃",
            "│       43 │ 19 [20⠂]21  22  23  24  25  ││ 10/18 07:00 Go hiking           ┃",
            "│       44 │ 26  27  28  29  30  31      ││ 10/20 A really long title that j┃",
            "│──────────┼─────────────────────────────││─────────────────────────────────│",
            "╰────────────────────────────────────────╯╰─────────────────────────────────╯",
            " hjkl[] 移動 | Tab 切換 | t 今天 | s 日曆 | ? 說明 | , 設定 | q 離開",
        ]
    );
    assert_eq!(app.agenda_window, Some((date(2026, 10, 1), 3)));
    assert_eq!(app.calendar_top, Some(0));
    // The scrollbar thumb has the same color as the border (keyboard in the agenda: focus color)
    let theme = Theme::default();
    let cell = &terminal.backend().buffer()[(76, 6)];
    assert_eq!((cell.symbol(), cell.fg), ("┃", theme.focus));
    // Clicks match the screen: the agenda's first row is the 4th item (10/16 Send the weekly update)
    assert_eq!(click_at(&app, 47, 4), Some(Click::Item(3)));
    // Back to the 1st item (10/5): the agenda scrolls back to the top
    app.agenda_index = 0;
    app.selected = date(2026, 10, 5);
    draw_app(&mut app, 77, 12);
    assert_eq!(app.agenda_window, Some((date(2026, 10, 1), 0)));
}

#[test]
fn the_agenda_follows_the_calendar() {
    // Keyboard in the calendar, only 5 of October's items fit: select 10/20, and the agenda scrolls until
    // the 10/20 item is visible; the calendar does not move
    let mut app = agenda_app(Lang::ZhTw);
    draw_app(&mut app, 77, 12);
    app.selected = date(2026, 10, 20);
    let terminal = draw_app(&mut app, 77, 12);
    assert_eq!(
        text_rows(terminal.backend().buffer()),
        [
            " ┃ 1 月曆   2 大月曆   3 任務",
            "╭────────────────────────────────────────╮╭─────────────────────────────────╮",
            "│2026年 週 │ 一  二  三  四  五  六  日  ││ 重要事項       2026/10/20（二） │",
            "│──────────┼─────────────────────────────││─────────────────────────────────│",
            "│  10月 40 │              1   2   3   4  ││ 10/16 17:30 Send the weekly upda│",
            "│       41 │  5⠂  6   7   8   9  10⠂ 11  ││ 10/17 Dinner with friends       │",
            "│       42 │ 12  13  14  15⠂ 16⠂ 17⠒ 18⠂ ││ 10/17 08:00 Coffee chat         ┃",
            "│       43 │ 19 [20⠂]21  22  23  24  25  ││ 10/18 07:00 Go hiking           ┃",
            "│       44 │ 26  27  28  29  30  31      ││ 10/20 A really long title that j┃",
            "│──────────┼─────────────────────────────││─────────────────────────────────│",
            "╰────────────────────────────────────────╯╰─────────────────────────────────╯",
            " hjkl[] 移動 | Tab 切換 | t 今天 | s 日曆 | ? 說明 | , 設定 | q 離開",
        ]
    );
    assert_eq!(app.agenda_window, Some((date(2026, 10, 1), 3)));
    assert_eq!(app.calendar_top, Some(0));
    // That day's items get the highlight background (like
    // its week on the calendar); other days' items do not
    let buffer = terminal.backend().buffer();
    let theme = Theme::default();
    assert_eq!(buffer[(47, 8)].bg, theme.selection);
    assert_eq!(buffer[(47, 7)].bg, Color::Reset);
    // Select 10/19, which has no items: the agenda does not scroll (no jumping up and down)
    app.selected = date(2026, 10, 19);
    draw_app(&mut app, 77, 12);
    assert_eq!(app.agenda_window, Some((date(2026, 10, 1), 3)));
    // Select 10/5: the agenda scrolls back to the top
    app.selected = date(2026, 10, 5);
    draw_app(&mut app, 77, 12);
    assert_eq!(app.agenda_window, Some((date(2026, 10, 1), 0)));
}

#[test]
fn the_year_in_the_corner_is_the_top_weeks() {
    // 6 calendar rows: select 12/28, then go one week down to 2027/1/4; the top of the screen is still
    // the December week → the top-left corner shows 2026年
    let mut app = App::new(Lang::ZhTw, date(2026, 10, 17), Vec::new());
    app.selected = date(2026, 12, 28);
    draw_app(&mut app, 42, 12);
    app.selected = date(2027, 1, 4);
    let rows = text_rows(draw_app(&mut app, 42, 12).backend().buffer());
    assert_eq!(rows[2], "│2026年 週 │ 一  二  三  四  五  六  日  │");
    assert_eq!(rows[4], "│       53 │ 28  29  30  31              │");
}

#[test]
fn a_busy_month_stops_at_16_rows_on_a_tall_screen() {
    // 50 × 41: the calendar has 35 rows, 2/3 of them is 23, but a month takes at most 16; October's 20
    // items need 20, so October's block is rows 4 to 19 and the line under it is at y 20
    let text: String = (1..=20)
        .map(|day| format!("- [ ] Item {day} @2026-10-{day:02}\n"))
        .collect();
    let mut app = App::new(
        Lang::En,
        date(2026, 10, 17),
        crate::tasks::parse(&text).items,
    );
    let rows = text_rows(draw_app(&mut app, 50, 41).backend().buffer());
    let lines: Vec<usize> = (4..rows.len())
        .filter(|&y| rows[y].starts_with("│──────────┼"))
        .collect();
    assert_eq!(lines.first(), Some(&20), "{rows:#?}");
}

#[test]
fn a_busy_month_takes_at_most_two_thirds() {
    // Sample task file, 70 × 25: the calendar has 19 rows and October has many items → it
    // takes 19 × 2 / 3 = 12 rows, and a few weeks of November show below
    let mut app = App::new(
        Lang::ZhTw,
        date(2026, 10, 17),
        crate::tasks::parse(include_str!("../../testdata/tasks.md")).items,
    );
    let rows = text_rows(draw_app(&mut app, 70, 25).backend().buffer());
    let body = &rows[4..rows.len() - 2];
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
    let terminal = draw_app(&mut app, 77, 12);
    assert_eq!(
        text_rows(terminal.backend().buffer()),
        [
            " ┃ 1 月曆   2 大月曆   3 任務",
            "╭────────────────────────────────────────╮╭─────────────────────────────────╮",
            "│2026年 週 │ 一  二  三  四  五  六  日  ││ 重要事項       2026/10/17（六） │",
            "│──────────┼─────────────────────────────││─────────────────────────────────│",
            "│  10月 40 │              1   2   3   4  ││ 10/15 Fix the login bug         │",
            "│       41 │  5⠂  6   7   8   9  10⠂ 11  ││ 10/16 17:30 Send the weekly upda┃",
            "│       42 │ 12  13  14  15⠂ 16⠂[17⠒]18⠂ ││ 10/17 Dinner with friends       ┃",
            "│       43 │ 19  20⠂ 21  22  23  24  25  ││ 10/17 08:00 Coffee chat         ┃",
            "│       44 │ 26  27  28  29  30  31      ││ 10/18 07:00 Go hiking           │",
            "│──────────┼─────────────────────────────││───────────────────── 還有 1 件 ─│",
            "╰────────────────────────────────────────╯╰─────────────────────────────────╯",
            " hjkl[] 移動 | Tab 切換 | t 今天 | s 日曆 | ? 說明 | , 設定 | q 離開",
        ]
    );
    // The hint is muted; the line on both sides of it keeps the
    // line color (「還」 is at x 65, the line's last cell at x 75)
    let buffer = terminal.backend().buffer();
    let theme = Theme::default();
    assert_eq!(
        (buffer[(65, 9)].symbol(), buffer[(65, 9)].fg),
        ("還", theme.muted)
    );
    assert_eq!(
        (buffer[(47, 9)].symbol(), buffer[(47, 9)].fg),
        ("─", theme.line)
    );
    assert_eq!(
        (buffer[(75, 9)].symbol(), buffer[(75, 9)].fg),
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
    // 70 × 13 (main view 70 × 11, under the tab bar): the edit box is in the middle, 2 cells from each
    // side (x 2-67), as tall as what is in it: the typed text, a blank row, what it reads as (title,
    // date), a blank row, the hint row; with a blank row above and below and the border, 10 tall at
    // y 1 + (11 − 10) / 2 = 1
    app.screen = Rect::new(0, 0, 70, 13);
    let mut terminal = draw_app(&mut app, 70, 13);
    let area = editor_area(&app);
    assert_eq!(area, Rect::new(2, 1, 66, 10));
    let rows = area_rows(terminal.backend().buffer(), area);
    let trimmed = |row: &String| row.trim_end_matches([' ', '│']).to_string();
    assert!(rows[0].starts_with("╭ 編輯事項 ─"), "{}", rows[0]);
    assert!(rows[0].ends_with("─╮"));
    assert_eq!(
        rows.iter().map(trimmed).collect::<Vec<_>>()[1..9],
        [
            "",
            "│  - [ ] Pick up the cake 🎂 @17:30",
            "",
            "│  標題  Pick up the cake 🎂",
            "│  日期  2026/10/16（五） 17:30",
            "",
            // What can be typed: today's date, a time, the priorities (no `#`: the file has no calendar)
            "│  語法：@2026-10-17 日期 | @10:00 時間 | ! !! !!! 優先度",
            "",
        ]
    );
    assert!(
        rows[9].starts_with("╰ Enter 存檔 | Esc 取消 ─"),
        "{}",
        rows[9]
    );
    // The item is on the selected day (choosing it in the agenda selected 10/16), so its date is left
    // out and the time stays as @17:30. The cursor is right after the typed text, at box x 2 + border
    // 1 + padding 1 + space 1 + 「- [ ] Pick up the cake 🎂 @17:30」 32 cells (31 chars)
    assert_eq!(
        terminal.get_cursor_position().expect("cursor"),
        Position::new(37, 3)
    );
    // Labels muted, values in the text color (the title row: 標 at x 5, the value at x 5 + 4 + 2)
    let theme = Theme::default();
    assert_eq!(terminal.backend().buffer()[(5, 5)].fg, theme.muted);
    assert_eq!(terminal.backend().buffer()[(11, 5)].fg, theme.text);
    // A time that does not exist: it stays in the title as text, and a red row says what is wrong; the
    // box grows by that row (11 tall, y 1)
    for code in [
        KeyCode::Backspace,
        KeyCode::Backspace,
        KeyCode::Char('9'),
        KeyCode::Char('9'),
    ] {
        app.handle_key(KeyEvent::from(code));
    }
    let terminal = draw_app(&mut app, 70, 13);
    let area = editor_area(&app);
    assert_eq!(area, Rect::new(2, 1, 66, 11));
    let rows = area_rows(terminal.backend().buffer(), area);
    assert_eq!(trimmed(&rows[4]), "│  標題  Pick up the cake 🎂 17:99");
    assert_eq!(trimmed(&rows[6]), "│  17:99：沒有這個時間");
    assert_eq!(terminal.backend().buffer()[(5, 7)].fg, theme.error);
    // Adding: the calendar it goes into (日曆), the title field blank; the hint row wraps (it
    // does not fit 62 cells), the next line under the first thing; too tall for the blank rows above and
    // below: 10 tall at y 1
    let mut app = App::new(
        Lang::ZhTw,
        date(2026, 10, 17),
        crate::tasks::parse("# Work\n## SFAP\n- [ ] Estimate @2026-10-17\n").items,
    );
    app.handle_key(KeyEvent::from(KeyCode::Tab));
    app.handle_key(KeyEvent::from(KeyCode::Char('a')));
    app.screen = Rect::new(0, 0, 70, 13);
    let terminal = draw_app(&mut app, 70, 13);
    let area = editor_area(&app);
    assert_eq!(area, Rect::new(2, 1, 66, 10));
    let rows = area_rows(terminal.backend().buffer(), area);
    assert!(rows[0].starts_with("╭ 新增事項 ─"), "{}", rows[0]);
    assert_eq!(
        rows.iter().map(trimmed).collect::<Vec<_>>()[1..9],
        [
            "│  - [ ]",
            "",
            "│  標題",
            "│  日期  2026/10/17（六）",
            "│  日曆  Work > SFAP",
            "",
            "│  語法：@2026-10-17 日期 | @10:00 時間 | ! !! !!! 優先度",
            "│        #名字 日曆",
        ]
    );
}

#[test]
fn typing_a_priority_colors_the_line_and_its_word_in_the_preview_right_away() {
    use ratatui::crossterm::event::{KeyCode, KeyEvent};
    // Adding at 80 × 25 with a priority: the box is (2, 7, 76, 11) (title, date, priority, then the hint row),
    // the typed text on row 9 from x 5; the fields on rows 11-13, the hint row on row 15
    let mut app = App::new(Lang::ZhTw, date(2026, 10, 17), Vec::new());
    app.handle_key(KeyEvent::from(KeyCode::Char('a')));
    let type_text = |app: &mut App, text: &str| {
        for c in text.chars() {
            app.handle_key(KeyEvent::from(KeyCode::Char(c)));
        }
    };
    let theme = Theme::default();
    // The color of the first cell on `row` showing `symbol`
    let color_of = |buffer: &Buffer, row: u16, symbol: &str| {
        (0..80)
            .map(|x| &buffer[(x, row)])
            .find(|cell| cell.symbol() == symbol)
            .map(|cell| cell.fg)
    };
    // No title yet (it cannot be saved), but `!!!` already: the typed text is red
    type_text(&mut app, "!!!");
    let terminal = draw_app(&mut app, 80, 25);
    assert_eq!(terminal.backend().buffer()[(5, 9)].symbol(), "-");
    assert_eq!(terminal.backend().buffer()[(5, 9)].fg, theme.priority_high);
    // With a title: the priority field says 高 in red, its label muted; the title in the text color
    type_text(&mut app, " Pay the fine");
    let terminal = draw_app(&mut app, 80, 25);
    let buffer = terminal.backend().buffer();
    assert_eq!(buffer[(5, 9)].fg, theme.priority_high);
    assert_eq!(color_of(buffer, 13, "高"), Some(theme.priority_high));
    assert_eq!(color_of(buffer, 13, "優"), Some(theme.muted));
    assert_eq!(color_of(buffer, 11, "P"), Some(theme.text));
    // In the typed text, `!!!` is in italics, the title is not
    let italic_on_9 = |symbol: &str| {
        (0..80)
            .map(|x| &buffer[(x, 9)])
            .find(|cell| cell.symbol() == symbol)
            .map(|cell| cell.modifier.contains(Modifier::ITALIC))
    };
    assert_eq!(italic_on_9("!"), Some(true));
    assert_eq!(italic_on_9("P"), Some(false));
    // The hint row: `! !! !!!` each in its priority's color, what to type in blue (the help page's section color), the labels
    // muted, a `|` in the dimmer line color between two
    let marks: Vec<Color> = (0..80)
        .map(|x| &buffer[(x, 15)])
        .filter(|cell| cell.symbol() == "!")
        .map(|cell| cell.fg)
        .collect();
    let (low, medium, high) = (
        theme.priority_low,
        theme.priority_medium,
        theme.priority_high,
    );
    assert_eq!(marks, [low, medium, medium, high, high, high]);
    assert_eq!(color_of(buffer, 15, "@"), Some(theme.help_section));
    assert_eq!(color_of(buffer, 15, "日"), Some(theme.muted));
    assert_eq!(color_of(buffer, 15, "|"), Some(theme.line));
    // `!!` instead: orange; no `!`: the normal text color, no priority field (the box is a row shorter: the
    // fields on rows 11-12, the hint row on row 14)
    app.handle_key(KeyEvent::from(KeyCode::Home));
    for _ in 0..6 {
        app.handle_key(KeyEvent::from(KeyCode::Right));
    }
    app.handle_key(KeyEvent::from(KeyCode::Delete));
    let terminal = draw_app(&mut app, 80, 25);
    let buffer = terminal.backend().buffer();
    assert_eq!(buffer[(5, 9)].fg, theme.priority_medium);
    assert_eq!(color_of(buffer, 13, "中"), Some(theme.priority_medium));
    for _ in 0..2 {
        app.handle_key(KeyEvent::from(KeyCode::Delete));
    }
    let terminal = draw_app(&mut app, 80, 25);
    let buffer = terminal.backend().buffer();
    assert_eq!(buffer[(5, 9)].fg, theme.text);
    assert!((11..14).all(|row| color_of(buffer, row, "優").is_none()));
}

#[test]
fn an_empty_title_is_blank_until_enter_cannot_save_it() {
    use ratatui::crossterm::event::{KeyCode, KeyEvent};
    let mut app = App::new(Lang::ZhTw, date(2026, 10, 17), Vec::new());
    app.screen = Rect::new(0, 0, 80, 24);
    app.handle_key(KeyEvent::from(KeyCode::Char('a')));
    let rows = |app: &mut App| -> Vec<String> {
        let terminal = draw_app(app, 80, 24);
        area_rows(terminal.backend().buffer(), editor_area(app))
            .iter()
            .map(|row| row.trim_end_matches([' ', '│']).to_string())
            .collect()
    };
    // Just opened: the title field is blank, nothing red
    let opened = rows(&mut app);
    assert!(opened.contains(&"│  標題".to_string()));
    assert!(!opened.iter().any(|row| row.contains("標題是空的")));
    // Enter cannot save it: it stays open, and a red row says why
    app.handle_key(KeyEvent::from(KeyCode::Enter));
    assert!(app.editor().is_some());
    assert!(rows(&mut app).contains(&"│  標題是空的".to_string()));
    // Typing again: the red row goes
    app.handle_key(KeyEvent::from(KeyCode::Char('x')));
    assert!(!rows(&mut app).iter().any(|row| row.contains("標題是空的")));
}

#[test]
fn the_calendar_editor_says_where_a_new_calendar_goes() {
    use ratatui::crossterm::event::{KeyCode, KeyEvent};
    // A in the calendar list on Work > SFAP: the fields say the name (a gray hint until it is typed) and the
    // calendar it goes inside; the hint row: type:holiday, archived: and today, and `#` and a name picks
    // where it goes.
    // 80 × 25: the box is (2, 7, 76, 10)
    let mut app = App::new(Lang::ZhTw, date(2026, 10, 17), Vec::new());
    app.set_items(crate::tasks::parse(
        "# Work\n## SFAP\n- [ ] Fix the login bug\n",
    ));
    app.focus = Focus::Side;
    app.side_row = SideRow::Calendar(1);
    app.screen = Rect::new(0, 0, 80, 25);
    app.handle_key(KeyEvent::from(KeyCode::Char('A')));
    let terminal = draw_app(&mut app, 80, 25);
    assert_eq!(editor_area(&app), Rect::new(2, 7, 76, 10));
    let rows = area_rows(terminal.backend().buffer(), editor_area(&app));
    let trimmed: Vec<String> = rows
        .iter()
        .map(|row| row.trim_end_matches([' ', '│']).to_string())
        .collect();
    assert!(rows[0].starts_with("╭ 新增日曆 ─"), "{}", rows[0]);
    assert_eq!(
        trimmed[4..8],
        [
            "│  名字      （打日曆的名字）",
            "│  上層日曆  Work > SFAP",
            "",
            "│  語法：type:holiday 假日日曆 | archived:2026-10-17 封存 | #名字 上層日曆",
        ]
    );
    assert!(
        rows[9].starts_with("╰ Enter 存檔 | Esc 取消 ─"),
        "{}",
        rows[9]
    );
    // a on the top level: 「最外層」
    app.handle_key(KeyEvent::from(KeyCode::Esc));
    app.handle_key(KeyEvent::from(KeyCode::Char('a')));
    let terminal = draw_app(&mut app, 80, 25);
    let rows = area_rows(terminal.backend().buffer(), editor_area(&app));
    assert_eq!(rows[5].trim_end_matches([' ', '│']), "│  上層日曆  最外層");
}

#[test]
fn the_calendar_editor_shows_the_day_a_calendar_was_archived() {
    use ratatui::crossterm::event::{KeyCode, KeyEvent};
    // e on Q3, archived on 10/5 (a Monday): the fields say its name, 「封存」 with that day (written like an
    // item's date) and where it is
    let mut app = App::new(Lang::ZhTw, date(2026, 10, 17), Vec::new());
    app.set_items(crate::tasks::parse(
        "# Work\n- a\n# Q3 archived:2026-10-05\n- b\n",
    ));
    app.focus = Focus::Side;
    app.side_row = SideRow::Calendar(1);
    app.screen = Rect::new(0, 0, 80, 24);
    app.handle_key(KeyEvent::from(KeyCode::Char('e')));
    let fields = |app: &mut App| -> Vec<String> {
        let terminal = draw_app(app, 80, 24);
        area_rows(terminal.backend().buffer(), editor_area(app))
            .iter()
            .map(|row| row.trim_end_matches([' ', '│']).to_string())
            .skip(4)
            .take_while(|row| !row.is_empty())
            .collect()
    };
    assert_eq!(
        fields(&mut app),
        [
            "│  名字      Q3",
            "│  封存      2026/10/5（一）",
            "│  上層日曆  最外層",
        ]
    );
    // The tag taken out: no 「封存」 field
    for _ in " archived:2026-10-05".chars() {
        app.handle_key(KeyEvent::from(KeyCode::Backspace));
    }
    assert_eq!(fields(&mut app), ["│  名字      Q3", "│  上層日曆  最外層"]);
}

#[test]
fn the_editor_sits_in_the_middle_like_the_day_popup_and_a_click_outside_cancels() {
    use ratatui::crossterm::event::{
        KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
    };
    let mut app = App::new(
        Lang::En,
        date(2026, 10, 17),
        crate::tasks::parse("- [ ] Send the weekly update @2026-10-16\n").items,
    );
    app.handle_key(KeyEvent::from(KeyCode::Tab));
    app.handle_key(KeyEvent::from(KeyCode::Char('e')));
    let at = |app: &mut App, width, height| {
        app.screen = Rect::new(0, 0, width, height);
        editor_area(app)
    };
    // In the middle of the main view (below the tab bar, above the key hints), like the day popup;
    // inside: the typed text, a blank row, title and date, a blank row, the hint row. 80 × 25 (main view
    // 80 × 23 from y 1): 76 wide (2 cells from each side), 10 tall with the blank rows above and below, top
    // at 1 + (23 − 10) / 2 = 7
    assert_eq!(at(&mut app, 80, 25), Rect::new(2, 7, 76, 10));
    // With the side menu (122 × 31: 22 cells wide), in the middle of what is right of it: the main view is
    // 100 × 29 from (22, 1) → x 22 + (100 − 80) / 2 = 32, y 1 + (29 − 10) / 2 = 10; 200 × 51: at most 80
    // wide
    assert_eq!(at(&mut app, 122, 31), Rect::new(32, 10, 80, 10));
    assert_eq!(at(&mut app, 200, 51), Rect::new(71, 20, 80, 10));
    // Narrower than 30: the whole width, the hint row wraps onto 3 lines (27 cells inside): 12 tall;
    // too short for the blank rows above and below (80 × 7: 5 rows between the tab bar and the key
    // hints): 5 tall
    assert_eq!(at(&mut app, 29, 25), Rect::new(0, 6, 29, 12));
    assert_eq!(at(&mut app, 80, 7), Rect::new(2, 1, 76, 5));
    app.screen = Rect::new(0, 0, 80, 25);
    let click = |app: &mut App, column: u16, row: u16| {
        app.handle_mouse(MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column,
            row,
            modifiers: KeyModifiers::NONE,
        });
    };
    // Inside the box (its corners and the blank rows too): stays open
    for (column, row) in [(2, 7), (77, 16), (40, 8)] {
        click(&mut app, column, row);
        assert!(app.editor().is_some(), "({column}, {row})");
    }
    // Below it (where the box is not): cancels
    click(&mut app, 40, 21);
    assert!(app.editor().is_none());
    // Narrower than 30: no blank cell left of the text (one blank in front of the typed text, which
    // scrolls so the cursor at its end shows: 26 cells for 「- [ ] Send the weekly update」 28 chars
    // and the cursor → 3 chars scrolled away); 29 × 13 is too short for the blank rows above and below
    // too, so the text is on the row below the border, as at 80 × 7
    app.handle_key(KeyEvent::from(KeyCode::Char('e')));
    let terminal = draw_app(&mut app, 29, 13);
    let rows = area_rows(terminal.backend().buffer(), at(&mut app, 29, 13));
    assert!(
        rows[1].starts_with("│  ] Send the weekly update"),
        "{}",
        rows[1]
    );
    let terminal = draw_app(&mut app, 80, 7);
    let rows = area_rows(terminal.backend().buffer(), at(&mut app, 80, 7));
    assert!(
        rows[1].starts_with("│  - [ ] Send the weekly"),
        "{}",
        rows[1]
    );
}

#[test]
fn the_hash_list_opens_under_the_hash_and_takes_clicks() {
    use ratatui::crossterm::event::{
        KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
    };
    let text = "# Holidays type:holiday\n- Day off @2026-10-09\n# Work\n- [ ] Estimate @2026-10-17\n\
                    ## SFAP\n- [ ] Fix the login bug @2026-10-12\n# Personal\n- Gym @2026-10-13\n";
    let mut app = App::new(
        Lang::En,
        date(2026, 10, 17),
        crate::tasks::parse(text).items,
    );
    app.screen = Rect::new(0, 0, 80, 25);
    let theme = Theme::default();
    let type_text = |app: &mut App, text: &str| {
        for c in text.chars() {
            app.handle_key(KeyEvent::from(KeyCode::Char(c)));
        }
    };
    let mouse = |app: &mut App, kind, column, row| {
        app.handle_mouse(MouseEvent {
            kind,
            column,
            row,
            modifiers: KeyModifiers::NONE,
        });
    };
    let click = MouseEventKind::Down(MouseButton::Left);
    // 80 × 25: the typed text starts at x 5 (box 2 + border + padding + a blank), so the `#` after
    // 「- [ ] 」 is at x 11 and the list's left border at x 10, right under the text (y 9: the hint row takes
    // two lines here, so the box is a row taller and starts a row higher). 26 wide: the keys on its bottom
    // border (24, with a `|` between them) and the borders; 4 calendars + borders = 6 tall
    app.handle_key(KeyEvent::from(KeyCode::Char('a')));
    type_text(&mut app, "#");
    let terminal = draw_app(&mut app, 80, 25);
    let buffer = terminal.backend().buffer();
    let list = Rect::new(10, 9, 26, 6);
    assert_eq!(
        app.editor().and_then(|editor| hash_list_area(&app, editor)),
        Some((list, 0))
    );
    assert_eq!(
        area_rows(buffer, list),
        [
            "╭────────────────────────╮",
            "│ Holidays               │",
            "│ Work                   │",
            "│ Work > SFAP            │",
            "│ Personal               │",
            "╰ Enter pick | Esc close ╯",
        ]
    );
    // The chosen row (the first) on the selection background, across the box
    for x in 11..35 {
        assert_eq!(buffer[(x, 10)].bg, theme.selection, "x {x}");
    }
    assert_ne!(buffer[(11, 11)].bg, theme.selection);
    // `sf`: only Work > SFAP; its S and F (characters 7 and 8 of the name, after a blank at x 11) in the
    // focus color and bold, the rest in the text color
    type_text(&mut app, "sf");
    let terminal = draw_app(&mut app, 80, 25);
    let buffer = terminal.backend().buffer();
    let list = Rect::new(10, 9, 26, 3);
    assert_eq!(area_rows(buffer, list)[1], "│ Work > SFAP            │");
    for x in [19, 20] {
        assert_eq!(buffer[(x, 10)].fg, theme.focus, "x {x}");
        assert!(buffer[(x, 10)].modifier.contains(Modifier::BOLD));
    }
    assert_eq!(buffer[(18, 10)].fg, theme.text);
    assert!(!buffer[(18, 10)].modifier.contains(Modifier::BOLD));
    // A click on it picks it
    mouse(&mut app, click, 15, 10);
    let editor = app.editor().expect("editor");
    assert_eq!(
        editor.calendar().map(|path| path.join(" > ")).as_deref(),
        Some("Work > SFAP")
    );
    assert_eq!(editor.input.value(), "- [ ] ");
    assert!(editor.suggestions().is_none());
    // The wheel moves the chosen row; a click on the top border does nothing; a click outside the list
    // and the box cancels
    type_text(&mut app, "#");
    mouse(&mut app, MouseEventKind::ScrollDown, 40, 21);
    mouse(&mut app, MouseEventKind::ScrollDown, 40, 21);
    mouse(&mut app, MouseEventKind::ScrollUp, 40, 21);
    assert_eq!(app.editor().map(|editor| editor.hash_list.index), Some(1));
    mouse(&mut app, click, 20, 9);
    assert!(app.editor().and_then(Editor::suggestions).is_some());
    mouse(&mut app, click, 40, 21);
    assert!(app.editor().is_none());
    // Editing an item: the fields name its calendar, the hint row (on two lines) says `#name` picks another
    app.handle_key(KeyEvent::from(KeyCode::Tab));
    app.handle_key(KeyEvent::from(KeyCode::Char('e')));
    let terminal = draw_app(&mut app, 80, 25);
    let rows = area_rows(terminal.backend().buffer(), editor_area(&app));
    assert!(rows[0].starts_with("╭ Edit item ─"), "{}", rows[0]);
    assert_eq!(rows[6].trim_end_matches([' ', '│']), "│  Calendar  Work");
    assert!(
        rows[9]
            .trim_end_matches([' ', '│'])
            .ends_with("#name calendar"),
        "{}",
        rows[9]
    );
    // More than 8 calendars: 8 rows, scrolled so the chosen one (the last) shows at the bottom
    let text: String = (0..10)
        .map(|n| format!("# C{n}\n- Item {n} @2026-10-17\n"))
        .collect();
    let mut app = App::new(
        Lang::En,
        date(2026, 10, 17),
        crate::tasks::parse(&text).items,
    );
    app.screen = Rect::new(0, 0, 80, 25);
    app.handle_key(KeyEvent::from(KeyCode::Char('a')));
    type_text(&mut app, "#");
    for _ in 0..9 {
        app.handle_key(KeyEvent::from(KeyCode::Down));
    }
    let terminal = draw_app(&mut app, 80, 25);
    let list = Rect::new(10, 9, 26, 10);
    assert_eq!(
        app.editor().and_then(|editor| hash_list_area(&app, editor)),
        Some((list, 2))
    );
    let rows = area_rows(terminal.backend().buffer(), list);
    assert_eq!(rows[1], "│ C2                     │");
    assert_eq!(rows[8], "│ C9                     │");
    assert_eq!(terminal.backend().buffer()[(11, 17)].bg, theme.selection);
}

#[test]
fn popups_have_a_blank_ring_around_them() {
    use ratatui::crossterm::event::{KeyCode, KeyEvent};
    // The cells of the ring one cell outside `area`, and of the ring one further out
    let ring = |area: Rect, distance: u16| {
        let outer = area.outer(Margin::new(distance, distance));
        let (left, right) = (outer.left(), outer.right() - 1);
        let (top, bottom) = (outer.top(), outer.bottom() - 1);
        let mut cells: Vec<(u16, u16)> = (left..=right)
            .flat_map(|x| [(x, top), (x, bottom)])
            .collect();
        cells.extend((top..=bottom).flat_map(|y| [(left, y), (right, y)]));
        cells
    };
    let blank = |buffer: &Buffer, cells: &[(u16, u16)]| {
        cells.iter().all(|&cell| buffer[cell].symbol() == " ")
    };
    // The editor over layout 1 (80 × 25, the sample file): the ring around it
    // is blank, the calendar and agenda show again right outside it
    let parsed = crate::tasks::parse(include_str!("../../testdata/tasks.md"));
    let mut app = App::new(Lang::En, date(2026, 10, 17), parsed.items);
    app.handle_key(KeyEvent::from(KeyCode::Tab));
    app.handle_key(KeyEvent::from(KeyCode::Char('e')));
    let terminal = draw_app(&mut app, 80, 25);
    let buffer = terminal.backend().buffer();
    app.screen = Rect::new(0, 0, 80, 25);
    let editor = editor_area(&app);
    assert!(blank(buffer, &ring(editor, 1)));
    assert!(!blank(buffer, &ring(editor, 2)));
    // The day popup over the month grid (60 × 21): the box is (9, 6, 42, 8) (see
    // `the_day_popup_lists_every_item_of_the_day_over_the_big_calendar`)
    let text = "- [ ] Buy groceries @2026-10-17 17:30\n- Farmers market @2026-10-17 10:00\n\
                    - [x] Clean the garage @2026-10-17\n- Parents in town @2026-10-16..2026-10-18\n";
    let mut app = month_app(Lang::ZhTw, date(2026, 10, 17), text);
    app.focus = Focus::Day;
    let terminal = draw_app(&mut app, 60, 21);
    let buffer = terminal.backend().buffer();
    app.screen = Rect::new(0, 0, 60, 21);
    let popup = day_area(&app);
    assert_eq!(popup, Rect::new(9, 6, 42, 8));
    assert!(blank(buffer, &ring(popup, 1)));
    assert!(!blank(buffer, &ring(popup, 2)));
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
    assert_eq!(cell.fg, Theme::default().notice);
}

#[test]
fn colors_by_role() {
    let terminal = draw(Lang::ZhTw, 47, 27, date(2026, 10, 17));
    let buffer = terminal.backend().buffer();
    let theme = Theme::default();
    // The (x, y) cell: symbol, foreground color, whether it is bold
    let cell = |x: u16, y: u16| {
        let cell = &buffer[(x, y)];
        (
            cell.symbol(),
            cell.fg,
            cell.modifier.contains(Modifier::BOLD),
        )
    };
    // The key hints are muted, with a `|` in the dimmer line color between two groups (「 hjkl[] 移動 | Tab
    // 切換 | 」: h at x 1, the `|` at x 13 and 24)
    assert_eq!(cell(1, 26), ("h", theme.muted, false));
    assert_eq!(cell(13, 26), ("|", theme.line, false));
    assert_eq!(cell(15, 26), ("T", theme.muted, false));
    assert_eq!(cell(24, 26), ("|", theme.line, false));
    // Keyboard in the calendar: the calendar box's border uses the focus color
    assert_eq!(cell(0, 1), ("╭", theme.focus, false));
    // Header: weekdays in the normal text color, Sat and Sun in the weekend color, vertical line
    // The year in the top-left corner (2026年, shown even when January is on screen)
    assert_eq!(cell(5, 2), ("年", theme.text, false));
    assert_eq!(cell(13, 2), ("一", theme.text, false));
    assert_eq!(cell(33, 2), ("六", theme.weekend, false));
    assert_eq!(cell(37, 2), ("日", theme.weekend, false));
    assert_eq!(cell(11, 2), ("│", theme.line, false));
    // The line between months, the thick line between years
    assert_eq!(cell(11, 3), ("┼", theme.line, false));
    assert_eq!(cell(5, 22), ("━", theme.year_line, false));
    assert_eq!(cell(11, 22), ("┿", theme.year_line, false));
    // Month (10月), January's year (2027年), week numbers
    assert_eq!(cell(3, 4), ("1", theme.label, false));
    assert_eq!(cell(1, 23), ("2", theme.label, false));
    // Week numbers: the selected week (42) bright, other weeks (41) dim
    assert_eq!(cell(8, 6), ("4", theme.week_number, false));
    assert_eq!(cell(8, 5), ("4", theme.week_number_dim, false));
    // 10/16 (Fri) normal; 10/17 (Sat) is today: today's color wins
    // over the weekend color, and it is bold; 10/18 (Sun) weekend
    assert_eq!(cell(29, 6), ("1", theme.text, false));
    assert_eq!(cell(34, 6), ("7", theme.today, true));
    assert_eq!(cell(38, 6), ("8", theme.weekend, false));
    // The selection brackets
    assert_eq!(cell(32, 6), ("[", theme.selected, true));
    assert_eq!(cell(35, 6), ("]", theme.selected, true));
}

#[test]
fn year_hint_in_the_header_is_normal_text() {
    // No January on screen: 2026 is shown left of the header's `Wk`,
    // in the header's normal text color (screen: see `calendar_in_english`)
    let terminal = draw(Lang::En, 40, 12, date(2026, 10, 17));
    let cell = &terminal.backend().buffer()[(3, 2)];
    assert_eq!((cell.symbol(), cell.fg), ("2", Theme::default().text));
}

/// Task file for the multi-day tests (`tools/expected_screen.py`
/// reads this same file for the expected screens)
const RANGE_TASKS: &str = include_str!("../../testdata/ranges.md");

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
    let terminal = draw_app(&mut app, 77, 31);
    assert_eq!(
        text_rows(terminal.backend().buffer()),
        [
            " ┃ 1 月曆   2 大月曆   3 任務",
            "╭────────────────────────────────────────╮╭─────────────────────────────────╮",
            "│2026年 週 │ 一  二  三  四  五  六  日  ││ 重要事項       2026/10/17（六） │",
            "│──────────┼─────────────────────────────││─────────────────────────────────│",
            "│  10月 40 │              1   2   3   4  ││ 10/16 17:30 Send the weekly upda│",
            "│       41 │  5   6   7   8   9  10  11  ││ 10/20~25 Business trip to NYC   │",
            "│       42 │ 12  13  14  15  16⠂[17] 18  ││ 10/22 Buy gifts for the team    │",
            "│       43 │ 19  20⠂ 21⠂ 22⠒ 23⠂ 24⠂ 25⠂ ││ 10/28~11/3 Manager OOO          │",
            "│       44 │ 26  27  28⠂ 29⠂ 30⠂ 31⠂     ││                                 │",
            "│──────────┼─────────────────────────────││─────────────────────────────────│",
            "│  11月 44 │                          1⠒ ││ 10/28~11/3 Manager OOO          │",
            "│       45 │  2⠂  3⠂  4   5   6   7   8  ││  11/1 Prepare the monthly report│",
            "│       46 │  9  10  11  12  13  14  15  ││                                 │",
            "│       47 │ 16  17  18  19  20  21  22  ││                                 │",
            "│       48 │ 23  24  25  26  27  28  29  ││                                 │",
            "│       49 │ 30                          ││                                 │",
            "│──────────┼─────────────────────────────││─────────────────────────────────│",
            "│  12月 49 │      1   2   3   4   5   6  ││ 12/30~1/2 09:00 New Year trip   │",
            "│       50 │  7   8   9  10  11  12  13  ││                                 │",
            "│       51 │ 14  15  16  17  18  19  20  ││                                 │",
            "│       52 │ 21  22  23  24  25  26  27  ││                                 │",
            "│       53 │ 28  29  30⠂ 31⠂             ││                                 │",
            "│━━━━━━━━━━┿━━━━━━━━━━━━━━━━━━━━━━━━━━━━━││━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━│",
            "│2027年  1 │                  1⠂  2⠂  3  ││ 12/30~1/2 09:00 New Year trip   │",
            "│   1月  2 │  4   5   6   7   8   9  10  ││                                 │",
            "│        3 │ 11  12  13  14  15  16  17  ││                                 │",
            "│        4 │ 18  19  20  21  22  23  24  ││                                 │",
            "│        5 │ 25  26  27  28  29  30  31  ││                                 │",
            "│──────────┼─────────────────────────────││─────────────────────────────────│",
            "╰────────────────────────────────────────╯╰─────────────────────────────────╯",
            " hjkl[] 移動 | Tab 切換 | t 今天 | s 日曆 | ? 說明 | , 設定 | q 離開",
        ]
    );
    // Calendar: every day in a range is underlined (10/20-25, 10/28-11/3), the days just before and after
    // are not; colors do not change: the start day (10/20) and the days in the middle (10/21, x 21-22)
    // keep the normal color
    let buffer = terminal.backend().buffer();
    let underlined = |x: u16, y: u16| buffer[(x, y)].modifier.contains(Modifier::UNDERLINED);
    // 10/19 (Mon) x 13-14, 10/20 (Tue) x 17-18, 10/25
    // (Sun) x 37-38 are at y 7; 10/26 (Mon) x 13-14 and 10/28 (Wed) x 21-22 at y 8
    assert!(!underlined(14, 7));
    assert!(underlined(18, 7) && underlined(38, 7));
    assert!(!underlined(14, 8) && underlined(22, 8));
    let theme = Theme::default();
    assert_eq!(buffer[(18, 7)].fg, theme.text);
    assert_eq!(buffer[(22, 7)].fg, theme.text);
    // Keyboard in the agenda, Manager OOO chosen on November's line (Tab from 11/2); November is on top (the week of 11/2 is November's 2nd row and must stay 2 rows from
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
        text_rows(draw_app(&mut app, 77, 31).backend().buffer()),
        [
            " ┃ 1 月曆   2 大月曆   3 任務",
            "╭────────────────────────────────────────╮╭─────────────────────────────────╮",
            "│2026年 週 │ 一  二  三  四  五  六  日  ││ 重要事項        2026/11/2（一） │",
            "│──────────┼─────────────────────────────││─────────────────────────────────│",
            "│──────────┼─────────────────────────────││─────────────────────────────────│",
            "│  11月 44 │                          1⠒ ││ 10/28~11/3 Manager OOO          │",
            "│       45 │ [2⠂] 3⠂  4   5   6   7   8  ││  11/1 Prepare the monthly report│",
            "│       46 │  9  10  11  12  13  14  15  ││                                 │",
            "│       47 │ 16  17  18  19  20  21  22  ││                                 │",
            "│       48 │ 23  24  25  26  27  28  29  ││                                 │",
            "│       49 │ 30                          ││                                 │",
            "│──────────┼─────────────────────────────││─────────────────────────────────│",
            "│  12月 49 │      1   2   3   4   5   6  ││ 12/30~1/2 09:00 New Year trip   │",
            "│       50 │  7   8   9  10  11  12  13  ││                                 │",
            "│       51 │ 14  15  16  17  18  19  20  ││                                 │",
            "│       52 │ 21  22  23  24  25  26  27  ││                                 │",
            "│       53 │ 28  29  30⠂ 31⠂             ││                                 │",
            "│━━━━━━━━━━┿━━━━━━━━━━━━━━━━━━━━━━━━━━━━━││━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━│",
            "│2027年  1 │                  1⠂  2⠂  3  ││ 12/30~1/2 09:00 New Year trip   │",
            "│   1月  2 │  4   5   6   7   8   9  10  ││                                 │",
            "│        3 │ 11  12  13  14  15  16  17  ││                                 │",
            "│        4 │ 18  19  20  21  22  23  24  ││                                 │",
            "│        5 │ 25  26  27  28  29  30  31  ││                                 │",
            "│──────────┼─────────────────────────────││─────────────────────────────────│",
            "│   2月  6 │  1   2   3   4   5   6   7  ││                                 │",
            "│        7 │  8   9  10  11  12  13  14  ││                                 │",
            "│        8 │ 15  16  17  18  19  20  21  ││                                 │",
            "│        9 │ 22  23  24  25  26  27  28  ││                                 │",
            "│──────────┼─────────────────────────────││─────────────────────────────────│",
            "╰────────────────────────────────────────╯╰─────────────────────────────────╯",
            " hjkl[] 移動 | Tab 切換 | t 今天 | s 日曆 | ? 說明 | , 設定 | q 離開",
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
    let terminal = draw_app(&mut app, 77, 31);
    let buffer = terminal.backend().buffer();
    let theme = Theme::default();
    assert_eq!(buffer[(47, 4)].bg, Color::Reset);
    assert_eq!(buffer[(47, 5)].bg, theme.selection);
    assert_eq!(buffer[(47, 6)].bg, theme.selection);
    assert_eq!(buffer[(47, 7)].bg, Color::Reset);
}

#[test]
fn holidays_are_days_off() {
    // US holidays (type:holiday): Long weekend 10/10-12, Halloween 10/31: in the weekend color, like a
    // day off (10/12 is a Monday); 10/31 also has an item in Work, which does not change the color
    let text = "# US holidays type:holiday\n- Long weekend @2026-10-10..2026-10-12\n\
                    - Halloween @2026-10-31\n# Work\n- [ ] On-call shift @2026-10-31\n";
    let mut app = App::new(
        Lang::ZhTw,
        date(2026, 10, 17),
        crate::tasks::parse(text).items,
    );
    let theme = Theme::default();
    let fg = |app: &mut App, x: u16, y: u16| {
        let terminal = draw_app(app, 77, 24);
        let cell = &terminal.backend().buffer()[(x, y)];
        (cell.symbol().to_string(), cell.fg)
    };
    // 10/10 (Sat) x 34, 10/11 (Sun) x 38 at y 5; 10/12 (Mon) x 14 at y 6;
    // 10/31 (Sat) x 34 at y 8; 10/9 (Fri) x 30 at y 5 is not a holiday
    assert_eq!(fg(&mut app, 34, 5), ("0".into(), theme.weekend));
    assert_eq!(fg(&mut app, 38, 5), ("1".into(), theme.weekend));
    assert_eq!(fg(&mut app, 14, 6), ("2".into(), theme.weekend));
    assert_eq!(fg(&mut app, 34, 8), ("1".into(), theme.weekend));
    assert_eq!(fg(&mut app, 30, 5), ("9".into(), theme.text));
    // If today is a holiday, today's green comes first
    app.today = date(2026, 10, 10);
    assert_eq!(fg(&mut app, 34, 5), ("0".into(), theme.today));
    app.today = date(2026, 10, 17);
    // The name in the side menu leaves out type:holiday; unchecked: no longer holidays (10/12, a
    // Monday, back to normal; 10/10 a Saturday, still a weekend)
    assert_eq!(app.groups[0].name.as_deref(), Some("US holidays"));
    app.groups[0].shown = false;
    assert_eq!(fg(&mut app, 14, 6), ("2".into(), theme.text));
    assert_eq!(fg(&mut app, 34, 5), ("0".into(), theme.weekend));
}

#[test]
fn a_holiday_is_all_blue() {
    // A holiday (in a type:holiday calendar) not over yet: its whole line in the day-off blue, the date
    // and time too; a normal item on the same day is in the normal color. In English, so every
    // character is one cell and a text's position in the row is its x
    let text = "# US holidays type:holiday\n- Halloween @2026-10-31 18:00\n\
                    - Long weekend @2026-10-24..2026-10-26\n# Home\n- [ ] Carve the pumpkin @2026-10-31\n";
    let theme = Theme::default();
    // The color of the first character of `text` on screen
    let color = |buffer: &Buffer, text: &str| {
        let (x, y) = text_rows(buffer)
            .iter()
            .enumerate()
            .find_map(|(y, row)| row.find(text).map(|at| (row[..at].chars().count(), y)))
            .expect(text);
        let cell = &buffer[(u16::try_from(x).unwrap(), u16::try_from(y).unwrap())];
        (cell.fg, cell.bg)
    };
    // Layout 1's agenda: 「 10/31 18:00 Halloween」 all blue, 「 10/31 Carve the pumpkin」 normal
    let mut app = App::new(
        Lang::En,
        date(2026, 10, 17),
        crate::tasks::parse(text).items,
    );
    let terminal = draw_app(&mut app, 70, 23);
    let buffer = terminal.backend().buffer();
    assert_eq!(color(buffer, "10/31 18:00").0, theme.weekend);
    assert_eq!(color(buffer, "18:00 Hall").0, theme.weekend);
    assert_eq!(color(buffer, "Halloween").0, theme.weekend);
    assert_eq!(color(buffer, "Carve").0, theme.text);
    // The month grid (119 wide, so the cells hold 「 18:00 Hallowee」): the cell's item and the
    // multi-day bar's title too, the bar keeps its background
    let mut app = month_app(Lang::En, date(2026, 10, 17), text);
    let terminal = draw_app(&mut app, 119, 30);
    let buffer = terminal.backend().buffer();
    assert_eq!(color(buffer, "Hallow").0, theme.weekend);
    assert_eq!(color(buffer, "Long weekend"), (theme.weekend, theme.range));
    assert_eq!(color(buffer, "Carve").0, theme.text);
}

#[test]
fn a_priority_colors_the_whole_line() {
    // `!!!` / `!!` / `!` not over yet: the whole line (date, time, title) in that priority's color, a
    // holiday's too; done or past: gray as usual. In English, so a text's position in the row is its x
    let text = "# Work\n- [ ] Fix the audit log !!! @2026-10-21 14:00\n- [ ] Call the bank !! @2026-10-22\n\
                    - [ ] Renew the passport ! @2026-10-23\n- [x] Pay the fine !!! @2026-10-20\n\
                    - Old release !!! @2026-10-12\n- Offsite !! @2026-10-26..2026-10-28\n\
                    - [ ] Book the hotel ! @2026-11-02\n- [ ] Broken !!! @2026-13-01\n\
                    # US holidays type:holiday\n- Halloween !!! @2026-10-31\n";
    let theme = Theme::default();
    // The (foreground, background) of the first character of `text` on screen
    let color = |buffer: &Buffer, text: &str| {
        let (x, y) = text_rows(buffer)
            .iter()
            .enumerate()
            .find_map(|(y, row)| row.find(text).map(|at| (row[..at].chars().count(), y)))
            .expect(text);
        let cell = &buffer[(u16::try_from(x).unwrap(), u16::try_from(y).unwrap())];
        (cell.fg, cell.bg)
    };
    let parsed = crate::tasks::parse(text);
    // Layout 1's agenda
    let mut app = App::new(Lang::En, date(2026, 10, 17), parsed.items.clone());
    let terminal = draw_app(&mut app, 70, 23);
    let buffer = terminal.backend().buffer();
    assert_eq!(color(buffer, "10/21 14:00").0, theme.priority_high);
    assert_eq!(color(buffer, "14:00 Fix").0, theme.priority_high);
    assert_eq!(color(buffer, "Fix the").0, theme.priority_high);
    assert_eq!(color(buffer, "Call the").0, theme.priority_medium);
    assert_eq!(color(buffer, "Renew").0, theme.priority_low);
    assert_eq!(color(buffer, "Offsite").0, theme.priority_medium);
    assert_eq!(color(buffer, "Halloween").0, theme.priority_high);
    assert_eq!(color(buffer, "Pay the").0, theme.muted);
    assert_eq!(color(buffer, "Old rel").0, theme.muted);
    // The month grid (119 wide): cells and the multi-day bar (which keeps its background); November's
    // item too (other months are not grayed out)
    let mut app = month_app(Lang::En, date(2026, 10, 17), text);
    let terminal = draw_app(&mut app, 119, 30);
    let buffer = terminal.backend().buffer();
    assert_eq!(color(buffer, "14:00 Fix").0, theme.priority_high);
    assert_eq!(color(buffer, "Call").0, theme.priority_medium);
    assert_eq!(color(buffer, "Renew").0, theme.priority_low);
    assert_eq!(
        color(buffer, "Offsite"),
        (theme.priority_medium, theme.range)
    );
    assert_eq!(color(buffer, "Hallow").0, theme.priority_high);
    assert_eq!(color(buffer, "Pay").0, theme.muted);
    assert_eq!(color(buffer, "Book").0, theme.priority_low);
    // Tasks page: the title only (the date column keeps its day's color); a line with a mistake stays red
    let mut app = App::new(Lang::En, date(2026, 10, 17), parsed.items);
    app.warnings = parsed.warnings;
    app.view = View::Tasks;
    app.focus = Focus::Tasks;
    let terminal = draw_app(&mut app, 70, 23);
    let buffer = terminal.backend().buffer();
    assert_eq!(color(buffer, "Fix the").0, theme.priority_high);
    assert_eq!(color(buffer, "Renew").0, theme.priority_low);
    assert_eq!(color(buffer, "Pay the").0, theme.muted);
    assert_eq!(color(buffer, "Broken").0, theme.error);
    assert_eq!(color(buffer, "10/21").0, theme.text);
}

#[test]
fn events_gray_out_once_their_day_is_past() {
    // 10/9 (Fri): an overdue unfinished task; 10/10 (Sat): a past event (gray, not struck through);
    // 10/18 (Sun): an upcoming event. Dates keep their usual colors; items not over yet are in the normal
    // color whatever their day
    let text = "- [ ] Send the weekly update @2026-10-09\n- Farmers market @2026-10-10\n\
                    - Go hiking @2026-10-18 07:00\n";
    let mut app = App::new(
        Lang::ZhTw,
        date(2026, 10, 17),
        crate::tasks::parse(text).items,
    );
    let terminal = draw_app(&mut app, 77, 24);
    let buffer = terminal.backend().buffer();
    let theme = Theme::default();
    let cell = |x: u16, y: u16| {
        let cell = &buffer[(x, y)];
        (
            cell.symbol(),
            cell.fg,
            cell.modifier.contains(Modifier::CROSSED_OUT),
        )
    };
    // Calendar: 10/9 (Fri) x 30, 10/10 (Sat) x 34 at y 5; 10/18 (Sun) x 38 at y 6
    assert_eq!(cell(30, 5), ("9", theme.text, false));
    assert_eq!(cell(34, 5), ("0", theme.weekend, false));
    assert_eq!(cell(38, 6), ("8", theme.weekend, false));
    // Agenda (starts at x 43, one item per row): titles
    // at x 50; Go hiking has a time in front, so it is at x 56
    assert_eq!(cell(50, 4), ("S", theme.text, false));
    assert_eq!(cell(50, 5), ("F", theme.muted, false));
    assert_eq!(cell(45, 5), ("0", theme.muted, false));
    assert_eq!(cell(56, 6), ("G", theme.text, false));
}

#[test]
fn items_do_not_change_the_date_color() {
    // The 16th has an unfinished item, the 15th only finished ones, the 14th none: all the normal
    // color; the 17th (today) is today's color, the 18th (Sun) the weekend color, items or not
    let text = "- [ ] Send the weekly update @2026-10-16\n- [x] Fix the login bug @2026-10-15\n\
                    - Dinner with friends @2026-10-17\n- [ ] Go hiking @2026-10-18 07:00\n";
    let mut app = App::new(
        Lang::ZhTw,
        date(2026, 10, 17),
        crate::tasks::parse(text).items,
    );
    let terminal = draw_app(&mut app, 47, 27);
    let buffer = terminal.backend().buffer();
    let theme = Theme::default();
    let fg = |x: u16| (buffer[(x, 6)].symbol(), buffer[(x, 6)].fg);
    assert_eq!(fg(22), ("4", theme.text));
    assert_eq!(fg(26), ("5", theme.text));
    assert_eq!(fg(30), ("6", theme.text));
    assert_eq!(fg(34), ("7", theme.today));
    assert_eq!(fg(38), ("8", theme.weekend));
}

/// An app in layout 2 with today 2026/10/17, `selected` selected, and `text` as the task file
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
    let terminal = draw(Lang::ZhTw, 63, 10, date(2026, 10, 17));
    assert_eq!(
        text_rows(terminal.backend().buffer()),
        [
            " ┃ 1 月曆   2 大月曆   3 任務",
            "╭────────────────────────────────────────╮╭───────────────────╮",
            "│2026年 週 │ 一  二  三  四  五  六  日  ││  2026/10/17（六） │",
            "│──────────┼─────────────────────────────││───────────────────│",
            "│  10月 40 │              1   2   3   4  ││                   │",
            "│       41 │  5   6   7   8   9  10  11  ││                   │",
            "│       42 │ 12  13  14  15  16 [17] 18  ││                   │",
            "│       43 │ 19  20  21  22  23  24  25  ││                   │",
            "╰────────────────────────────────────────╯╰───────────────────╯",
            " hjkl[] 移動 | Tab 切換 | t 今天 | s 日曆 | ? 說明 | , 設定 | q",
        ]
    );
    let terminal = draw(Lang::ZhTw, 64, 10, date(2026, 10, 17));
    assert_eq!(
        text_rows(terminal.backend().buffer()),
        [
            " ┃ 1 月曆   2 大月曆   3 任務",
            "╭────────────────────────────────────────╮╭────────────────────╮",
            "│2026年 週 │ 一  二  三  四  五  六  日  ││   2026/10/17（六） │",
            "│──────────┼─────────────────────────────││────────────────────│",
            "│  10月 40 │              1   2   3   4  ││                    │",
            "│       41 │  5   6   7   8   9  10  11  ││                    │",
            "│       42 │ 12  13  14  15  16 [17] 18  ││                    │",
            "│       43 │ 19  20  21  22  23  24  25  ││                    │",
            "╰────────────────────────────────────────╯╰────────────────────╯",
            " hjkl[] 移動 | Tab 切換 | t 今天 | s 日曆 | ? 說明 | , 設定 | q",
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
            "   1 月曆 ┃ 2 大月曆   3 任務",
            "╭────────────────────────────── 2026/10/17（六） ──────────────────────────────╮",
            "│  週   一         二         三         四        五        六        日      │",
            "├─────┬──────────┬──────────┬──────────┬─────────┬─────────┬─────────┬─────────┤",
            "│  40 │ 28       │ 29       │ 30       │ 1 10月  │ 2       │ 3       │ 4       │",
            "│ 10月│          │          │          │         │         │         │         │",
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
            "│  44 │ 26       │ 27       │ 28       │ 29      │ 30      │ 31      │ 1 11月  │",
            "│ 11月│          │          │          │         │         │         │         │",
            "│     │          │          │          │         │         │         │         │",
            "├─────┼──────────┼──────────┼──────────┼─────────┼─────────┼─────────┼─────────┤",
            "│  45 │ 2        │ 3        │ 4        │ 5       │ 6       │ 7       │ 8       │",
            "│     │          │          │          │         │         │         │         │",
            "│     │          │          │          │         │         │         │         │",
            "│     │          │          │          │         │         │         │         │",
            "╰─────┴──────────┴──────────┴──────────┴─────────┴─────────┴─────────┴─────────╯",
            " hjkl[] 移動 | Tab 切換 | t 今天 | s 日曆 | ? 說明 | , 設定 | q 離開",
        ]
    );
    // 80 × 28: 3 rows per week fill it exactly, and the last week touches the bottom border (joined as ┴)
    let mut app = month_app(Lang::En, date(2026, 10, 17), "");
    assert_eq!(
        text_rows(draw_app(&mut app, 80, 28).backend().buffer()),
        [
            "   1 calendar ┃ 2 month   3 tasks",
            "╭────────────────────────────── Sat Oct 17, 2026 ──────────────────────────────╮",
            "│  Wk   Mo         Tu         We         Th        Fr        Sa        Su      │",
            "├─────┬──────────┬──────────┬──────────┬─────────┬─────────┬─────────┬─────────┤",
            "│  40 │ 28       │ 29       │ 30       │ 1 Oct   │ 2       │ 3       │ 4       │",
            "│ Oct │          │          │          │         │         │         │         │",
            "├─────┼──────────┼──────────┼──────────┼─────────┼─────────┼─────────┼─────────┤",
            "│  41 │ 5        │ 6        │ 7        │ 8       │ 9       │ 10      │ 11      │",
            "│     │          │          │          │         │         │         │         │",
            "├─────┼──────────┼──────────┼──────────┼─────────┼─────────┼─────────┼─────────┤",
            "│  42 │ 12       │ 13       │ 14       │ 15      │ 16      │ 17      │ 18      │",
            "│     │          │          │          │         │         │         │         │",
            "├─────┼──────────┼──────────┼──────────┼─────────┼─────────┼─────────┼─────────┤",
            "│  43 │ 19       │ 20       │ 21       │ 22      │ 23      │ 24      │ 25      │",
            "│     │          │          │          │         │         │         │         │",
            "├─────┼──────────┼──────────┼──────────┼─────────┼─────────┼─────────┼─────────┤",
            "│  44 │ 26       │ 27       │ 28       │ 29      │ 30      │ 31      │ 1 Nov   │",
            "│ Nov │          │          │          │         │         │         │         │",
            "├─────┼──────────┼──────────┼──────────┼─────────┼─────────┼─────────┼─────────┤",
            "│  45 │ 2        │ 3        │ 4        │ 5       │ 6       │ 7       │ 8       │",
            "│     │          │          │          │         │         │         │         │",
            "├─────┼──────────┼──────────┼──────────┼─────────┼─────────┼─────────┼─────────┤",
            "│  46 │ 9        │ 10       │ 11       │ 12      │ 13      │ 14      │ 15      │",
            "│     │          │          │          │         │         │         │         │",
            "├─────┼──────────┼──────────┼──────────┼─────────┼─────────┼─────────┼─────────┤",
            "│  47 │ 16       │ 17       │ 18       │ 19      │ 20      │ 21      │ 22      │",
            "╰─────┴──────────┴──────────┴──────────┴─────────┴─────────┴─────────┴─────────╯",
            " hjkl[] move | Tab pane | t today | s calendars | ? help | , settings | q quit",
        ]
    );
}

/// Task file for the month grid tests (`tools/expected_screen.py`
/// reads this same file for the expected screens)
const MONTH_TASKS: &str = include_str!("../../testdata/month.md");

#[test]
fn month_view_lists_each_days_items_in_its_cell() {
    // 80 × 31, 3 rows per week: 2 items listed under the date; on the same day, items
    // without a time first (like layout 1); 10/16 has 4 items that do not fit: 1 is listed
    // and the last line says 「+3」; items on other months' days (9/25, 11/1) are listed
    // too. A title that is too long borrows the next cells to the right (only a day with
    // nothing at all, covering the grid line between) and is cut where it can't: 10/13
    // borrows 10/14 and stops before 10/15; 10/12's neighbor has items, so it can't borrow;
    // 10/18 has an item, so other days can't borrow it, but 10/17 is the selected day: its
    // 2nd line borrows 10/18's empty 2nd line (its background shows whose text it is)
    let mut app = month_app(Lang::ZhTw, date(2026, 10, 17), MONTH_TASKS);
    assert_eq!(
        text_rows(draw_app(&mut app, 80, 31).backend().buffer()),
        [
            "   1 月曆 ┃ 2 大月曆   3 任務",
            "╭────────────────────────────── 2026/10/17（六） ──────────────────────────────╮",
            "│  週   一         二         三         四        五        六        日      │",
            "├─────┬──────────┬──────────┬──────────┬─────────┬─────────┬─────────┬─────────┤",
            "│  40 │ 28       │ 29       │ 30       │ 1 10月  │ 2       │ 3       │ 4       │",
            "│ 10月│          │ Backyard BBQ        │         │         │         │         │",
            "│     │          │          │          │         │         │         │         │",
            "├─────┼──────────┼──────────┼──────────┼─────────┼─────────┼─────────┼─────────┤",
            "│  41 │ 5        │ 6        │ 7        │ 8       │ 9       │ 10      │ 11      │",
            "│     │ Pay the phone bill  │          │         │         │ Farmers market    │",
            "│     │          │          │          │         │         │         │         │",
            "├─────┼──────────┼──────────┼──────────┼─────────┼─────────┼─────────┼─────────┤",
            "│  42 │ 12       │ 13       │ 14       │ 15      │ 16      │ 17      │ 18      │",
            "│     │ 14:00 Cod│ 15:00 Confirm alerti│ Fix the │ 🚧 Team │ Dinner w│ 07:00 Go│",
            "│     │          │          │          │         │ +3      │ 08:00 Coffee chat │",
            "├─────┼──────────┼──────────┼──────────┼─────────┼─────────┼─────────┼─────────┤",
            "│  43 │ 19       │ 20       │ 21       │ 22      │ 23      │ 24      │ 25      │",
            "│     │          │ A really long title that spills over    │         │         │",
            "│     │          │          │          │         │         │         │         │",
            "├─────┼──────────┼──────────┼──────────┼─────────┼─────────┼─────────┼─────────┤",
            "│  44 │ 26       │ 27       │ 28       │ 29      │ 30      │ 31      │ 1 11月  │",
            "│ 11月│          │          │          │         │         │         │ Buy a bi│",
            "│     │          │          │          │         │         │         │         │",
            "├─────┼──────────┼──────────┼──────────┼─────────┼─────────┼─────────┼─────────┤",
            "│  45 │ 2        │ 3        │ 4        │ 5       │ 6       │ 7       │ 8       │",
            "│     │          │          │          │         │         │         │         │",
            "│     │          │          │          │         │         │         │         │",
            "├─────┼──────────┼──────────┼──────────┼─────────┼─────────┼─────────┼─────────┤",
            "│  46 │ 9        │ 10       │ 11       │ 12      │ 13      │ 14      │ 15      │",
            "╰─────┴──────────┴──────────┴──────────┴─────────┴─────────┴─────────┴─────────╯",
            " hjkl[] 移動 | Tab 切換 | t 今天 | s 日曆 | ? 說明 | , 設定 | q 離開",
        ]
    );
    // 80 × 25, 2 rows per week: only 1 item fits; with 2 or more it just shows 「+N」
    let mut app = month_app(Lang::En, date(2026, 10, 17), MONTH_TASKS);
    assert_eq!(
        text_rows(draw_app(&mut app, 80, 25).backend().buffer()),
        [
            "   1 calendar ┃ 2 month   3 tasks",
            "╭────────────────────────────── Sat Oct 17, 2026 ──────────────────────────────╮",
            "│  Wk   Mo         Tu         We         Th        Fr        Sa        Su      │",
            "├─────┬──────────┬──────────┬──────────┬─────────┬─────────┬─────────┬─────────┤",
            "│  40 │ 28       │ 29       │ 30       │ 1 Oct   │ 2       │ 3       │ 4       │",
            "│ Oct │          │ Backyard BBQ        │         │         │         │         │",
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
            "│  44 │ 26       │ 27       │ 28       │ 29      │ 30      │ 31      │ 1 Nov   │",
            "│ Nov │          │          │          │         │         │         │ Buy a bi│",
            "├─────┼──────────┼──────────┼──────────┼─────────┼─────────┼─────────┼─────────┤",
            "│  45 │ 2        │ 3        │ 4        │ 5       │ 6       │ 7       │ 8       │",
            "│     │          │          │          │         │         │         │         │",
            "├─────┼──────────┼──────────┼──────────┼─────────┼─────────┼─────────┼─────────┤",
            "│  46 │ 9        │ 10       │ 11       │ 12      │ 13      │ 14      │ 15      │",
            "╰─────┴──────────┴──────────┴──────────┴─────────┴─────────┴─────────┴─────────╯",
            " hjkl[] move | Tab pane | t today | s calendars | ? help | , settings | q quit",
        ]
    );
    // 80 × 17, only 1 row per week: no items listed, dots after
    // the date instead (1 item ⠂, 2 items ⠒, 4 or more ⠶)
    let mut app = month_app(Lang::ZhTw, date(2026, 10, 17), MONTH_TASKS);
    assert_eq!(
        text_rows(draw_app(&mut app, 80, 17).backend().buffer()),
        [
            "   1 月曆 ┃ 2 大月曆   3 任務",
            "╭────────────────────────────── 2026/10/17（六） ──────────────────────────────╮",
            "│  週   一         二         三         四        五        六        日      │",
            "├─────┬──────────┬──────────┬──────────┬─────────┬─────────┬─────────┬─────────┤",
            "│  40 │ 28       │ 29 ⠂     │ 30       │ 1 10月  │ 2       │ 3       │ 4       │",
            "├─────┼──────────┼──────────┼──────────┼─────────┼─────────┼─────────┼─────────┤",
            "│  41 │ 5 ⠂      │ 6        │ 7        │ 8       │ 9       │ 10 ⠂    │ 11      │",
            "├─────┼──────────┼──────────┼──────────┼─────────┼─────────┼─────────┼─────────┤",
            "│  42 │ 12 ⠂     │ 13 ⠂     │ 14       │ 15 ⠂    │ 16 ⠶    │ 17 ⠒    │ 18 ⠂    │",
            "├─────┼──────────┼──────────┼──────────┼─────────┼─────────┼─────────┼─────────┤",
            "│  43 │ 19       │ 20 ⠂     │ 21       │ 22      │ 23      │ 24      │ 25      │",
            "├─────┼──────────┼──────────┼──────────┼─────────┼─────────┼─────────┼─────────┤",
            "│  44 │ 26       │ 27       │ 28       │ 29      │ 30      │ 31      │ 1 11月⠂ │",
            "├─────┼──────────┼──────────┼──────────┼─────────┼─────────┼─────────┼─────────┤",
            "│  45 │ 2        │ 3        │ 4        │ 5       │ 6       │ 7       │ 8       │",
            "╰─────┴──────────┴──────────┴──────────┴─────────┴─────────┴─────────┴─────────╯",
            " hjkl[] 移動 | Tab 切換 | t 今天 | s 日曆 | ? 說明 | , 設定 | q 離開",
        ]
    );
}

#[test]
fn month_view_item_colors() {
    // 80 × 31 (screen: see `month_view_lists_each_days_items_in_its_cell`): cells start at x 7, 18, 29,
    // 40, 50, 60, 70; weeks 1, 3 and 4 are at y 4, 12 and 16
    let mut app = month_app(Lang::ZhTw, date(2026, 10, 17), MONTH_TASKS);
    let terminal = draw_app(&mut app, 80, 31);
    let buffer = terminal.backend().buffer();
    let theme = Theme::default();
    // The (x, y) cell: symbol, foreground color, whether it is struck through
    let cell = |x: u16, y: u16| {
        let cell = &buffer[(x, y)];
        (
            cell.symbol(),
            cell.fg,
            cell.modifier.contains(Modifier::CROSSED_OUT),
        )
    };
    // 10/12 (Mon) unfinished item: time muted, title in the normal color
    assert_eq!(cell(8, 13), ("1", theme.muted, false));
    assert_eq!(cell(14, 13), ("C", theme.text, false));
    // 10/15 (Thu) finished item: whole line gray, title struck through
    assert_eq!(cell(41, 13), ("F", theme.muted, true));
    // 10/17 (Sat) is today: its item is still in the normal color (only the date is green)
    assert_eq!(cell(61, 13), ("D", theme.text, false));
    // 10/16 (Fri) overflows: 「+3」 is muted
    assert_eq!(cell(52, 14), ("3", theme.muted, false));
    // 9/29 (Tue, in the top week 9/28-10/4) is in another month: not grayed out, the normal color
    assert_eq!(cell(19, 5), ("B", theme.text, false));
    assert_eq!(cell(20, 5), ("a", theme.text, false));
    // 10/20 (Tue)'s long title borrows cells up to 10/23 (Fri): text over the grid lines keeps its own
    // color (x 49 was a vertical line); the grid lines after the text keep the grid color
    assert_eq!(cell(49, 17), ("s", theme.text, false));
    assert_eq!(buffer[(59, 17)].symbol(), "│");
    assert_eq!(buffer[(59, 17)].fg, theme.grid);
    // Only 1 row per week: the dots follow the day's color (10/16, a Friday: normal); all done (10/15)
    // is muted
    let terminal = draw_app(&mut app, 80, 17);
    let buffer = terminal.backend().buffer();
    // 80 × 17: week 3 is at y 8; 10/16 is 「 16 ⠶」, 10/15 is 「 15 ⠂」
    assert_eq!(
        (buffer[(54, 8)].symbol(), buffer[(54, 8)].fg),
        ("⠶", theme.text)
    );
    assert_eq!(
        (buffer[(44, 8)].symbol(), buffer[(44, 8)].fg),
        ("⠂", theme.muted)
    );
}

#[test]
fn month_view_with_the_side_menu_hides_unchecked_calendars() {
    // 121 cells wide: the side menu on the far left, the month grid takes all the rest
    let mut app = month_app(Lang::ZhTw, date(2026, 10, 17), MONTH_TASKS);
    let terminal = draw_app(&mut app, 121, 23);
    assert_eq!(
        text_rows(terminal.backend().buffer()),
        [
            "   1 月曆 ┃ 2 大月曆   3 任務",
            "╭────────────────────╮╭─────────────────────────────────────── 2026/10/17（六） ────────────────────────────────────────╮",
            "│ 日曆               ││  週   一            二           三           四           五           六           日         │",
            "│────────────────────│├─────┬─────────────┬────────────┬────────────┬────────────┬────────────┬────────────┬────────────┤",
            "│ ☐\u{a0}隱藏全部         ││  40 │ 28          │ 29         │ 30         │ 1 10月     │ 2          │ 3          │ 4          │",
            "│ ☐\u{a0}隱藏做完的       ││ 10月│             │ Backyard BBQ            │            │            │            │            │",
            "│────────────────────│├─────┼─────────────┼────────────┼────────────┼────────────┼────────────┼────────────┼────────────┤",
            "│ ☑\u{a0}未分組         7 ││  41 │ 5           │ 6          │ 7          │ 8          │ 9          │ 10         │ 11         │",
            "│ ☑\u{a0}Personal       8 ││     │ Pay the phone bill       │            │            │            │ Farmers market          │",
            "│                    │├─────┼─────────────┼────────────┼────────────┼────────────┼────────────┼────────────┼────────────┤",
            "│                    ││  42 │ 12          │ 13         │ 14         │ 15         │ 16         │ 17         │ 18         │",
            "│                    ││     │ 14:00 Code r│ 15:00 Confirm alerting w│ Fix the log│ +4         │ +2         │ 07:00 Go hi│",
            "│                    │├─────┼─────────────┼────────────┼────────────┼────────────┼────────────┼────────────┼────────────┤",
            "│                    ││  43 │ 19          │ 20         │ 21         │ 22         │ 23         │ 24         │ 25         │",
            "│                    ││     │             │ A really long title that spills over │            │            │            │",
            "│                    │├─────┼─────────────┼────────────┼────────────┼────────────┼────────────┼────────────┼────────────┤",
            "│                    ││  44 │ 26          │ 27         │ 28         │ 29         │ 30         │ 31         │ 1 11月     │",
            "│                    ││ 11月│             │            │            │            │            │            │ Buy a birth│",
            "│                    │├─────┼─────────────┼────────────┼────────────┼────────────┼────────────┼────────────┼────────────┤",
            "│                    ││  45 │ 2           │ 3          │ 4          │ 5          │ 6          │ 7          │ 8          │",
            "│                    ││     │             │            │            │            │            │            │            │",
            "╰────────────────────╯╰─────┴─────────────┴────────────┴────────────┴────────────┴────────────┴────────────┴────────────╯",
            " hjkl[] 移動 | Tab 切換 | t 今天 | s 日曆 | ? 說明 | , 設定 | q 離開",
        ]
    );
    // Personal unchecked: its items leave the cells (10/18 stays weekend blue either way)
    app.groups[1].shown = false;
    let terminal = draw_app(&mut app, 121, 23);
    assert_eq!(
        text_rows(terminal.backend().buffer()),
        [
            "   1 月曆 ┃ 2 大月曆   3 任務",
            "╭────────────────────╮╭─────────────────────────────────────── 2026/10/17（六） ────────────────────────────────────────╮",
            "│ 日曆               ││  週   一            二           三           四           五           六           日         │",
            "│────────────────────│├─────┬─────────────┬────────────┬────────────┬────────────┬────────────┬────────────┬────────────┤",
            "│ ☐\u{a0}隱藏全部         ││  40 │ 28          │ 29         │ 30         │ 1 10月     │ 2          │ 3          │ 4          │",
            "│ ☐\u{a0}隱藏做完的       ││ 10月│             │            │            │            │            │            │            │",
            "│────────────────────│├─────┼─────────────┼────────────┼────────────┼────────────┼────────────┼────────────┼────────────┤",
            "│ ☑\u{a0}未分組         7 ││  41 │ 5           │ 6          │ 7          │ 8          │ 9          │ 10         │ 11         │",
            "│ ☐\u{a0}Personal       8 ││     │             │            │            │            │            │            │            │",
            "│                    │├─────┼─────────────┼────────────┼────────────┼────────────┼────────────┼────────────┼────────────┤",
            "│                    ││  42 │ 12          │ 13         │ 14         │ 15         │ 16         │ 17         │ 18         │",
            "│                    ││     │ 14:00 Code r│ 15:00 Confirm alerting w│ Fix the log│ +4         │            │            │",
            "│                    │├─────┼─────────────┼────────────┼────────────┼────────────┼────────────┼────────────┼────────────┤",
            "│                    ││  43 │ 19          │ 20         │ 21         │ 22         │ 23         │ 24         │ 25         │",
            "│                    ││     │             │            │            │            │            │            │            │",
            "│                    │├─────┼─────────────┼────────────┼────────────┼────────────┼────────────┼────────────┼────────────┤",
            "│                    ││  44 │ 26          │ 27         │ 28         │ 29         │ 30         │ 31         │ 1 11月     │",
            "│                    ││ 11月│             │            │            │            │            │            │            │",
            "│                    │├─────┼─────────────┼────────────┼────────────┼────────────┼────────────┼────────────┼────────────┤",
            "│                    ││  45 │ 2           │ 3          │ 4          │ 5          │ 6          │ 7          │ 8          │",
            "│                    ││     │             │            │            │            │            │            │            │",
            "╰────────────────────╯╰─────┴─────────────┴────────────┴────────────┴────────────┴────────────┴────────────┴────────────╯",
            " hjkl[] 移動 | Tab 切換 | t 今天 | s 日曆 | ? 說明 | , 設定 | q 離開",
        ]
    );
    // 10/18 (Sun): the month grid starts at x 22, the Sunday column at x 108, week 3 at y 10 (2 rows per
    // week); the 「1」 of 「 18」 is at x 109
    let theme = Theme::default();
    let cell = &terminal.backend().buffer()[(109, 10)];
    assert_eq!((cell.symbol(), cell.fg), ("1", theme.weekend));
}

#[test]
fn month_view_without_grid_lines_gives_the_cells_more_room() {
    // Config `[month] grid_lines = false`: no grid lines, the cells touch, and no line under the header
    // either; only where a month ends, its edge (a row after the first week for September's, a row
    // above and below the week of 10/26 for October's); 80 × 25: day width 78 − 5 = 73 (Mon-Wed 11
    // cells, the others 10), each week (21 − 1 − 3) ÷ 6 = 2 rows
    let mut app = month_app(Lang::ZhTw, date(2026, 10, 17), MONTH_TASKS);
    app.settings.month.grid_lines = false;
    app.screen = Rect::new(0, 0, 80, 25);
    assert_eq!(
        text_rows(draw_app(&mut app, 80, 25).backend().buffer()),
        [
            "   1 月曆 ┃ 2 大月曆   3 任務",
            "╭────────────────────────────── 2026/10/17（六） ──────────────────────────────╮",
            "│  週  一         二         三         四        五        六        日       │",
            "│  40  28         29         30        │1 10月    2         3         4        │",
            "│ 10月            Backyard BBQ         │                                       │",
            "├──────────────────────────────────────┘                                       │",
            "│  41  5          6          7          8         9         10        11       │",
            "│      Pay the phone bill                                   Farmers market     │",
            "│  42  12         13         14         15        16        17        18       │",
            "│      14:00 Code 15:00 Confirm alertin Fix the l +4        +2        07:00 Go │",
            "│  43  19         20         21         22        23        24        25       │",
            "│                 A really long title that spills over                         │",
            "│                                                                    ┌─────────┤",
            "│  44  26         27         28         29        30        31       │1 11月   │",
            "│ 11月                                                               │Buy a bir│",
            "├────────────────────────────────────────────────────────────────────┘         │",
            "│  45  2          3          4          5         6         7         8        │",
            "│                                                                              │",
            "│  46  9          10         11         12        13        14        15       │",
            "│                                                                              │",
            "│  47  16         17         18         19        20        21        22       │",
            "│                                                                              │",
            "│  48  23         24         25         26        27        28        29       │",
            "╰──────────────────────────────────────────────────────────────────────────────╯",
            " hjkl[] 移動 | Tab 切換 | t 今天 | s 日曆 | ? 說明 | , 設定 | q 離開",
        ]
    );
    // Mouse: the cells without grid lines can be clicked too. Tuesday is x 17-27, week 2 (the week of
    // 10/5) is y 6-7, below September's edge at y 5 (nothing to click there)
    assert_eq!(click_at(&app, 17, 6), Some(Click::Day(date(2026, 10, 6))));
    assert_eq!(click_at(&app, 27, 7), Some(Click::Day(date(2026, 10, 6))));
    assert_eq!(click_at(&app, 28, 7), Some(Click::Day(date(2026, 10, 7))));
    assert_eq!(click_at(&app, 20, 5), None);
}

#[test]
fn the_month_grid_wheel_scrolls_and_keeps_the_selected_day() {
    use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseEvent, MouseEventKind};
    // Layout 2 at 80 × 30 with 10/17 selected: it opens with the week of 10/1 (Monday 9/28) on top
    let mut app = App::new(Lang::ZhTw, date(2026, 10, 17), Vec::new());
    app.view = View::Month;
    app.screen = Rect::new(0, 0, 80, 30);
    draw_app(&mut app, 80, 30);
    assert_eq!(app.month_top, Some(date(2026, 9, 28)));
    let wheel = |app: &mut App, kind| {
        app.handle_mouse(MouseEvent {
            kind,
            column: 40,
            row: 10,
            modifiers: KeyModifiers::NONE,
        });
    };
    // Wheel down 3: 10/19 on top (the 6 weeks are 10/19-11/29), so 10/17's week is off screen; it stays
    // selected, and drawing does not scroll back to it
    for _ in 0..3 {
        wheel(&mut app, MouseEventKind::ScrollDown);
    }
    draw_app(&mut app, 80, 30);
    assert_eq!(app.selected, date(2026, 10, 17));
    assert_eq!(app.month_top, Some(date(2026, 10, 19)));
    // Wheel up: 10/12 on top
    wheel(&mut app, MouseEventKind::ScrollUp);
    draw_app(&mut app, 80, 30);
    assert_eq!(app.month_top, Some(date(2026, 10, 12)));
    // Down again (10/19 on top), then a key that moves the selection (l: 10/18): the view comes back just
    // enough to show its week, 10/12, on the top row
    wheel(&mut app, MouseEventKind::ScrollDown);
    app.handle_key(KeyEvent::from(KeyCode::Char('l')));
    draw_app(&mut app, 80, 30);
    assert_eq!(app.selected, date(2026, 10, 18));
    assert_eq!(app.month_top, Some(date(2026, 10, 12)));
}

#[test]
fn month_view_keeps_the_selected_week_in_the_middle() {
    // Config `[month] center_cursor = true`: the selected week stays on row 3 (also right after opening,
    // instead of the week with the 1st on top)
    let mut app = month_app(Lang::ZhTw, date(2026, 10, 17), "");
    app.settings.month.center_cursor = true;
    assert_eq!(
        text_rows(draw_app(&mut app, 80, 17).backend().buffer()),
        [
            "   1 月曆 ┃ 2 大月曆   3 任務",
            "╭────────────────────────────── 2026/10/17（六） ──────────────────────────────╮",
            "│  週   一         二         三         四        五        六        日      │",
            "├─────┬──────────┬──────────┬──────────┬─────────┬─────────┬─────────┬─────────┤",
            "│  40 │ 28       │ 29       │ 30       │ 1 10月  │ 2       │ 3       │ 4       │",
            "├─────┼──────────┼──────────┼──────────┼─────────┼─────────┼─────────┼─────────┤",
            "│  41 │ 5        │ 6        │ 7        │ 8       │ 9       │ 10      │ 11      │",
            "├─────┼──────────┼──────────┼──────────┼─────────┼─────────┼─────────┼─────────┤",
            "│  42 │ 12       │ 13       │ 14       │ 15      │ 16      │ 17      │ 18      │",
            "├─────┼──────────┼──────────┼──────────┼─────────┼─────────┼─────────┼─────────┤",
            "│  43 │ 19       │ 20       │ 21       │ 22      │ 23      │ 24      │ 25      │",
            "├─────┼──────────┼──────────┼──────────┼─────────┼─────────┼─────────┼─────────┤",
            "│  44 │ 26       │ 27       │ 28       │ 29      │ 30      │ 31      │ 1 11月  │",
            "├─────┼──────────┼──────────┼──────────┼─────────┼─────────┼─────────┼─────────┤",
            "│  45 │ 2        │ 3        │ 4        │ 5       │ 6       │ 7       │ 8       │",
            "╰─────┴──────────┴──────────┴──────────┴─────────┴─────────┴─────────┴─────────╯",
            " hjkl[] 移動 | Tab 切換 | t 今天 | s 日曆 | ? 說明 | , 設定 | q 離開",
        ]
    );
    // One week down (the same even though 11/2 is not the last of
    // the 6 weeks): the whole screen scrolls down one week with it
    app.selected = date(2026, 11, 2);
    assert_eq!(
        text_rows(draw_app(&mut app, 80, 17).backend().buffer()),
        [
            "   1 月曆 ┃ 2 大月曆   3 任務",
            "╭────────────────────────────── 2026/11/2（一） ───────────────────────────────╮",
            "│  週   一         二         三         四        五        六        日      │",
            "├─────┬──────────┬──────────┬──────────┬─────────┬─────────┬─────────┬─────────┤",
            "│  43 │ 19       │ 20       │ 21       │ 22      │ 23      │ 24      │ 25      │",
            "├─────┼──────────┼──────────┼──────────┼─────────┼─────────┼─────────┼─────────┤",
            "│  44 │ 26       │ 27       │ 28       │ 29      │ 30      │ 31      │ 1 11月  │",
            "├─────┼──────────┼──────────┼──────────┼─────────┼─────────┼─────────┼─────────┤",
            "│  45 │ 2        │ 3        │ 4        │ 5       │ 6       │ 7       │ 8       │",
            "├─────┼──────────┼──────────┼──────────┼─────────┼─────────┼─────────┼─────────┤",
            "│  46 │ 9        │ 10       │ 11       │ 12      │ 13      │ 14      │ 15      │",
            "├─────┼──────────┼──────────┼──────────┼─────────┼─────────┼─────────┼─────────┤",
            "│  47 │ 16       │ 17       │ 18       │ 19      │ 20      │ 21      │ 22      │",
            "├─────┼──────────┼──────────┼──────────┼─────────┼─────────┼─────────┼─────────┤",
            "│  48 │ 23       │ 24       │ 25       │ 26      │ 27      │ 28      │ 29      │",
            "╰─────┴──────────┴──────────┴──────────┴─────────┴─────────┴─────────┴─────────╯",
            " hjkl[] 移動 | Tab 切換 | t 今天 | s 日曆 | ? 說明 | , 設定 | q 離開",
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
        text_rows(draw_app(&mut app, 42, 15).backend().buffer()),
        [
            " ┃ 1 月曆   2 大月曆   3 任務",
            "╭────────────────────────────────────────╮",
            "│2026年 週 │ 一  二  三  四  五  六  日  │",
            "│──────────┼─────────────────────────────│",
            "│  10月  2 │              1   2   3   4  │",
            "│        1 │  5   6   7   8   9  10  11  │",
            "│       42 │ 12  13  14  15  16 [17] 18  │",
            "│        1 │ 19  20  21  22  23  24  25  │",
            "│        2 │ 26  27  28  29  30  31      │",
            "│──────────┼─────────────────────────────│",
            "│  11月  2 │                          1  │",
            "│        3 │  2   3   4   5   6   7   8  │",
            "│        4 │  9  10  11  12  13  14  15  │",
            "╰────────────────────────────────────────╯",
            " hjkl[] 移動 | Tab 切換 | t 今天 | s 日曆",
        ]
    );
    // Same in layout 2
    app.view = View::Month;
    assert_eq!(
        text_rows(draw_app(&mut app, 80, 17).backend().buffer()),
        [
            "   1 月曆 ┃ 2 大月曆   3 任務",
            "╭────────────────────────────── 2026/10/17（六） ──────────────────────────────╮",
            "│  週   一         二         三         四        五        六        日      │",
            "├─────┬──────────┬──────────┬──────────┬─────────┬─────────┬─────────┬─────────┤",
            "│   2 │ 28       │ 29       │ 30       │ 1 10月  │ 2       │ 3       │ 4       │",
            "├─────┼──────────┼──────────┼──────────┼─────────┼─────────┼─────────┼─────────┤",
            "│   1 │ 5        │ 6        │ 7        │ 8       │ 9       │ 10      │ 11      │",
            "├─────┼──────────┼──────────┼──────────┼─────────┼─────────┼─────────┼─────────┤",
            "│  42 │ 12       │ 13       │ 14       │ 15      │ 16      │ 17      │ 18      │",
            "├─────┼──────────┼──────────┼──────────┼─────────┼─────────┼─────────┼─────────┤",
            "│   1 │ 19       │ 20       │ 21       │ 22      │ 23      │ 24      │ 25      │",
            "├─────┼──────────┼──────────┼──────────┼─────────┼─────────┼─────────┼─────────┤",
            "│   2 │ 26       │ 27       │ 28       │ 29      │ 30      │ 31      │ 1 11月  │",
            "├─────┼──────────┼──────────┼──────────┼─────────┼─────────┼─────────┼─────────┤",
            "│   3 │ 2        │ 3        │ 4        │ 5       │ 6       │ 7       │ 8       │",
            "╰─────┴──────────┴──────────┴──────────┴─────────┴─────────┴─────────┴─────────╯",
            " hjkl[] 移動 | Tab 切換 | t 今天 | s 日曆 | ? 說明 | , 設定 | q 離開",
        ]
    );
}

#[test]
fn month_view_lists_a_multi_day_item_on_each_of_its_days() {
    // Business trip to NYC 10/20-25: one bar on its days' first line, across the grid lines, the title
    // written once; a one-day item on one of those days goes on the next line (10/22: Buy gifts for
    // the team, cut off: the days after it have the bar); Manager OOO 10/28-11/3: its bar breaks where
    // November starts (between 10/31 and 11/1)
    // and at the end of the week, and each part writes the title again
    let mut app = month_app(Lang::ZhTw, date(2026, 10, 17), RANGE_TASKS);
    let terminal = draw_app(&mut app, 80, 31);
    assert_eq!(
        text_rows(terminal.backend().buffer()),
        [
            "   1 月曆 ┃ 2 大月曆   3 任務",
            "╭────────────────────────────── 2026/10/17（六） ──────────────────────────────╮",
            "│  週   一         二         三         四        五        六        日      │",
            "├─────┬──────────┬──────────┬──────────┬─────────┬─────────┬─────────┬─────────┤",
            "│  40 │ 28       │ 29       │ 30       │ 1 10月  │ 2       │ 3       │ 4       │",
            "│ 10月│          │          │          │         │         │         │         │",
            "│     │          │          │          │         │         │         │         │",
            "├─────┼──────────┼──────────┼──────────┼─────────┼─────────┼─────────┼─────────┤",
            "│  41 │ 5        │ 6        │ 7        │ 8       │ 9       │ 10      │ 11      │",
            "│     │          │          │          │         │         │         │         │",
            "│     │          │          │          │         │         │         │         │",
            "├─────┼──────────┼──────────┼──────────┼─────────┼─────────┼─────────┼─────────┤",
            "│  42 │ 12       │ 13       │ 14       │ 15      │ 16      │ 17      │ 18      │",
            "│     │          │          │          │         │ 17:30 Se│         │         │",
            "│     │          │          │          │         │         │         │         │",
            "├─────┼──────────┼──────────┼──────────┼─────────┼─────────┼─────────┼─────────┤",
            "│  43 │ 19       │ 20       │ 21       │ 22      │ 23      │ 24      │ 25      │",
            "│     │          │ Business trip to NYC                                        │",
            "│     │          │          │          │ Buy gift│         │         │         │",
            "├─────┼──────────┼──────────┼──────────┼─────────┼─────────┼─────────┼─────────┤",
            "│  44 │ 26       │ 27       │ 28       │ 29      │ 30      │ 31      │ 1 11月  │",
            "│ 11月│          │          │ Manager OOO                            │ Manager │",
            "│     │          │          │          │         │         │         │ Prepare │",
            "├─────┼──────────┼──────────┼──────────┼─────────┼─────────┼─────────┼─────────┤",
            "│  45 │ 2        │ 3        │ 4        │ 5       │ 6       │ 7       │ 8       │",
            "│     │ Manager OOO         │          │         │         │         │         │",
            "│     │          │          │          │         │         │         │         │",
            "├─────┼──────────┼──────────┼──────────┼─────────┼─────────┼─────────┼─────────┤",
            "│  46 │ 9        │ 10       │ 11       │ 12      │ 13      │ 14      │ 15      │",
            "╰─────┴──────────┴──────────┴──────────┴─────────┴─────────┴─────────┴─────────╯",
            " hjkl[] 移動 | Tab 切換 | t 今天 | s 日曆 | ? 說明 | , 設定 | q 離開",
        ]
    );
    // Dates in a range are underlined: 10/20 (Tue) 「 20」 at x 19-20, y 16; 10/19 (Mon) at x 8-9 is not
    let buffer = terminal.backend().buffer();
    let underlined = |x: u16, y: u16| buffer[(x, y)].modifier.contains(Modifier::UNDERLINED);
    assert!(underlined(20, 16));
    assert!(!underlined(9, 16));
    // The bar (y 17) has the range background from Tuesday's left edge (x 18) to Sunday's right edge
    // (x 78), the grid lines between too (x 28, 39); not the grid line left of it, nor the next line
    let theme = Theme::default();
    for x in [18, 28, 39, 60, 78] {
        assert_eq!(buffer[(x, 17)].bg, theme.range, "x {x}");
    }
    assert_eq!(buffer[(17, 17)].symbol(), "│");
    assert_ne!(buffer[(17, 17)].bg, theme.range);
    assert_ne!(buffer[(18, 18)].bg, theme.range);
    // The title is normal text (not over yet), not the day's color
    assert_eq!(
        (buffer[(19, 17)].symbol(), buffer[(19, 17)].fg),
        ("B", theme.text)
    );
    // Manager OOO (y 21): the grid line between 10/31 and 11/1 (x 69) is not covered; November's part
    // has the background again, and its title the normal color (other months are not grayed out)
    assert_eq!(buffer[(69, 21)].symbol(), "│");
    assert_ne!(buffer[(69, 21)].bg, theme.range);
    assert_eq!(buffer[(70, 21)].bg, theme.range);
    assert_eq!(
        (buffer[(71, 21)].symbol(), buffer[(71, 21)].fg),
        ("M", theme.text)
    );
}

#[test]
fn stacked_bars_take_turns_between_two_backgrounds() {
    // Three overlapping trips in the week of 10/19, one line each: the 1st and 3rd lines in `range`, the
    // 2nd in `range_alt`, so bars on lines next to each other read as two. In English, so a text's
    // position in the row is its x
    let text = "- Trip A @2026-10-20..2026-10-23\n- Trip B @2026-10-21..2026-10-24\n\
                - Trip C @2026-10-22..2026-10-25\n";
    let mut app = month_app(Lang::En, date(2026, 10, 17), text);
    let terminal = draw_app(&mut app, 100, 40);
    let buffer = terminal.backend().buffer();
    let rows = text_rows(buffer);
    // Where `text` starts on screen (x, y)
    let at = |text: &str| {
        rows.iter()
            .enumerate()
            .find_map(|(y, row)| row.find(text).map(|x| (x, y)))
            .expect(text)
    };
    let background =
        |x: usize, y: usize| buffer[(u16::try_from(x).unwrap(), u16::try_from(y).unwrap())].bg;
    let theme = Theme::default();
    let (a, b, c) = (at("Trip A"), at("Trip B"), at("Trip C"));
    // On lines right below each other
    assert_eq!((b.1, c.1), (a.1 + 1, a.1 + 2));
    assert_eq!(background(a.0, a.1), theme.range);
    assert_eq!(background(b.0, b.1), theme.range_alt);
    assert_eq!(background(c.0, c.1), theme.range);
    // The whole of B's band: from its title to Saturday, past the end of its text
    for x in b.0..b.0 + 30 {
        assert_eq!(background(x, b.1), theme.range_alt, "x {x}");
    }
}

/// An app on the tasks page with today 2026/10/17 and the
/// sample task file (mistakes recorded too, as in `main`)
fn tasks_app(lang: Lang) -> App {
    let parsed = crate::tasks::parse(include_str!("../../testdata/tasks.md"));
    let mut app = App::new(lang, date(2026, 10, 17), parsed.items);
    app.warnings = parsed.warnings;
    app.view = View::Tasks;
    app.focus = Focus::Tasks;
    app
}

#[test]
fn tasks_page_with_collapsed_calendars() {
    // testdata/calendars.md with Work > SFAP and Personal collapsed: their headings get ▸ and the calendar
    // list's number, and nothing inside them is listed (Bugs inside SFAP, Moving and Wishlist inside
    // Personal); Work itself stays open; the chosen row is the first item still listed
    let mut app = App::new(Lang::En, date(2026, 10, 17), Vec::new());
    app.set_items(crate::tasks::parse(include_str!(
        "../../testdata/calendars.md"
    )));
    (app.view, app.focus) = (View::Tasks, Focus::Tasks);
    app.collapsed = std::collections::BTreeSet::from([
        vec!["Work".to_string(), "SFAP".to_string()],
        vec!["Personal".to_string()],
    ]);
    assert_eq!(
        text_rows(draw_app(&mut app, 60, 22).backend().buffer()),
        [
            "   1 calendar   2 month ┃ 3 tasks",
            "╭──────────────────────────────────────────────────────────╮",
            "│ Tasks                                                    │",
            "│──────────────────────────────────────────────────────────│",
            "│ ▾ Work                                                   │",
            "│   ▸ SFAP  2                                              │",
            "│   ▾ CAIS                                                 │",
            "│       ☐\u{a0}Ship the monthly report              10/22       │",
            "│ ▸ Personal  2                                            │",
            "│ ▾ Work                                                   │",
            "│     ∙ Team sync                              10/19 10:00 │",
            "│   Someday                                                │",
            "│                                                          │",
            "│                                                          │",
            "│                                                          │",
            "│                                                          │",
            "│                                                          │",
            "│                                                          │",
            "│──────────────────────────────────────────────────────────│",
            "│ Line 8  - [ ] Ship the monthly report @2026-10-22        │",
            "╰──────────────────────────────────────────────────────────╯",
            " jk move | hl fold | Space check | e edit | a add | o editor",
        ]
    );
}

#[test]
fn tasks_page_lists_calendars_inside_calendars_as_a_tree() {
    // testdata/calendars.md: every heading is listed, `# Work` with no items of its own, and the calendars
    // with no items at all (`## Wishlist` in Personal, `# Someday`); a calendar is indented 2 cells per
    // calendar it is inside, and so are its items (under its heading); `### Bugs` goes one deeper; `# Work`
    // again near the end gets a heading again
    let mut app = App::new(Lang::En, date(2026, 10, 17), Vec::new());
    app.set_items(crate::tasks::parse(include_str!(
        "../../testdata/calendars.md"
    )));
    (app.view, app.focus) = (View::Tasks, Focus::Tasks);
    app.screen = Rect::new(0, 0, 60, 23);
    assert_eq!(
        text_rows(draw_app(&mut app, 60, 23).backend().buffer()),
        [
            "   1 calendar   2 month ┃ 3 tasks",
            "╭──────────────────────────────────────────────────────────╮",
            "│ Tasks                                                    │",
            "│──────────────────────────────────────────────────────────│",
            "│ ▾ Work                                                   │",
            "│   ▾ SFAP                                                 │",
            "│     ▸ ☐\u{a0}Fix the login bug  0/1               10/20       │",
            "│     ▾ Bugs                                               │",
            "│         ☐\u{a0}Session times out too early        10/21       │",
            "│   ▾ CAIS                                                 │",
            "│       ☐\u{a0}Ship the monthly report              10/22       │",
            "│ ▾ Personal                                               │",
            "│     ∙ Gym                                    10/18       │",
            "│   ▾ Moving                                               │",
            "│       ☐\u{a0}Pack the books                        11/1       │",
            "│     Wishlist                                             │",
            "│ ▾ Work                                                   │",
            "│     ∙ Team sync                              10/19 10:00 │",
            "│   Someday                                                │",
            "│──────────────────────────────────────────────────────────│",
            "│ Line 3  - [ ] Fix the login bug @2026-10-20              │",
            "╰──────────────────────────────────────────────────────────╯",
            " jk move | hl fold | Space check | e edit | a add | o editor",
        ]
    );
    // `▸` of the first item (under SFAP's heading, which is 1 level in): x = 1 + 1 + 2 × 2 = 6 on the
    // screen, row 6
    assert_eq!(
        click_at(&app, 6, 6),
        Some(Click::TaskRow {
            row: 2,
            on_marker: true
        })
    );
    assert_eq!(
        click_at(&app, 2, 6),
        Some(Click::TaskRow {
            row: 2,
            on_marker: false
        })
    );
    // A calendar heading's `▾` (Work, row 4: x = 1 + 1) is a marker too; the rest of the row only chooses it;
    // an empty calendar (Someday, row 18) has no marker
    assert_eq!(
        click_at(&app, 2, 4),
        Some(Click::TaskRow {
            row: 0,
            on_marker: true
        })
    );
    assert_eq!(
        click_at(&app, 5, 4),
        Some(Click::TaskRow {
            row: 0,
            on_marker: false
        })
    );
    assert!(matches!(
        click_at(&app, 2, 18),
        Some(Click::TaskRow {
            on_marker: false,
            ..
        })
    ));
    // A right click on `▸` expands that item's whole subtree; another one collapses it
    use ratatui::crossterm::event::{KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
    let right_click = |app: &mut App| {
        app.handle_mouse(MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Right),
            column: 6,
            row: 6,
            modifiers: KeyModifiers::NONE,
        });
    };
    let expanded = |app: &App| {
        matches!(
            app.tasks_rows()[2],
            crate::app::TaskRow::Item { expanded: true, .. }
        )
    };
    right_click(&mut app);
    assert!(expanded(&app));
    right_click(&mut app);
    assert!(!expanded(&app));
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
            "   1 月曆   2 大月曆 ┃ 3 任務",
            "╭────────────────────────────────────────────────────────────────────╮",
            "│ 任務                                                      1 處寫錯 │",
            "│────────────────────────────────────────────────────────────────────│",
            "│ ▾ Payments API v2                                                  │",
            "│     ☑\u{a0}Align with PM on the refund flow spec             9/28       │",
            "│     ☑\u{a0}Break down tickets and estimate                   9/30 10:00 │",
            "│   ▸ ☐\u{a0}Implement the refund API  1/4                    10/14       │",
            "│     ☐\u{a0}Code review: Amy's PR #482                       10/12 14:00 │",
            "│     ☐\u{a0}Confirm alerting with SRE                        10/13 15:00 │",
            "│     ☐\u{a0}Reply to Kevin about error codes                 10/14       │",
            "│     ☑\u{a0}Fix the staging DB migration                      10/8       │",
            "│     ∙ Code freeze                                      10/23       │",
            "│     ☐\u{a0}Prepare the release checklist @2026-10-23        10/22       │",
            "│     ☐\u{a0}Write the runbook, see                                       │",
            "│       https://wiki.example.com/payments/runbook                    │",
            "│     ∙ Release v2.3                                     10/27 09:30 │",
            "│ ▾ Recurring                                                        │",
            "│     ∙ Team sync                                        10/19 10:00 │",
            "│     ∙ Team sync                                        10/26 10:00 │",
            "│     ☐\u{a0}Send the weekly update                           10/16 17:30 │",
            "│     ☐\u{a0}Clean up this week's notes                       10/16       │",
            "│     ☐\u{a0}🚧 Draft the release notes                       10/16       │",
            "│     ∙ Team offsite                                     10/29       │",
            "│     ☐\u{a0}Send the weekly update                           10/23 17:30 │",
            "│     ☑\u{a0}Renew the VPN certificate                         10/5       │",
            "│────────────────────────────────────────────────────────────────────│",
            "│ 第 4 行  - [x] Align with PM on the refund flow spec @2026-09-28   │",
            "╰────────────────────────────────────────────────────────────────────╯",
            " jk 移動 | hl 收合 | Space 打勾 | e 編輯 | a 新增 | o 編輯器 | s 日曆",
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
            "   1 calendar   2 month ┃ 3 tasks",
            "╭────────────────────────────────────────────────────────────────────╮",
            "│ Tasks                                                 1 mistake(s) │",
            "│────────────────────────────────────────────────────────────────────│",
            "│ ▾ Learning Rust 🦀                                                 │",
            "│     ☑\u{a0}Finish chapter 4 of The Book (ownership)          10/3       │",
            "│     ☐\u{a0}Read the Ratatui layout docs                                 │",
            "│   ▾ ☐\u{a0}Write the task file parser for roh-cal  0/1      10/11       │",
            "│     ▾ ☐\u{a0}Write a test for every rule  0/1                           │",
            "│         ☐\u{a0}Test that `@Rukis` is not a date                         │",
            "│     ∙ RustConf livestream @Rukis                       10/29 23:00 │",
            "│ ▾ Tokyo trip                                                       │",
            "│────────────────────────────────────────────────────────────────────│",
            "│ Line 61  - [ ] Test that `@Rukis` is not a date                    │",
            "╰────────────────────────────────────────────────────────────────────╯",
            " jk move | hl fold | Space check | e edit | a add | o editor | s calen",
        ]
    );
}

#[test]
fn tasks_page_colors_and_clicks() {
    // Dates not in this year get the year in front; done tasks gray and struck through; past events (listed
    // only with an open task under them) gray, not struck through; dates in that day's color; the chosen
    // row gets the highlight background
    let text = "# Work\n- [ ] Send the weekly update @2026-10-16\n- [x] Fix the login bug @2026-10-15\n\
                    - Hackathon @2026-10-10\n  - [ ] Write it up\n- Trip to Tokyo @2027-01-02\n  - [ ] Book flights\n";
    let mut app = App::new(
        Lang::ZhTw,
        date(2026, 10, 17),
        crate::tasks::parse(text).items,
    );
    app.view = View::Tasks;
    app.focus = Focus::Tasks;
    app.screen = Rect::new(0, 0, 40, 15);
    let terminal = draw_app(&mut app, 40, 15);
    let rows = text_rows(terminal.backend().buffer());
    // At 40 wide 「Send the weekly update」 does not fit before its date: 「update」 wraps onto the next line,
    // under the title; the rows below move down one line (expected rows from tools/expected_screen.py)
    assert_eq!(rows[4], "│ ▾ Work                               │");
    assert_eq!(rows[5], "│     ☐\u{a0}Send the weekly          10/16 │");
    assert_eq!(rows[6], "│       update                         │");
    assert_eq!(rows[8], "│   ▸ ∙ Hackathon  0/1           10/10 │");
    assert_eq!(rows[9], "│   ▸ ∙ Trip to Tokyo  0/1    2027/1/2 │");
    let buffer = terminal.backend().buffer();
    let theme = Theme::default();
    let cell = |x: u16, y: u16| {
        let cell = &buffer[(x, y)];
        (
            cell.symbol(),
            cell.fg,
            cell.modifier.contains(Modifier::CROSSED_OUT),
        )
    };
    // Header 「 任務」 (the border at x 0, a blank at x 1): normal text
    assert_eq!(cell(2, 2), ("任", theme.text, false));
    // Heading: ▾ muted, Work in the label color, not bold
    assert_eq!(cell(2, 4), ("▾", theme.muted, false));
    assert_eq!(buffer[(4, 4)].fg, theme.label);
    assert!(!buffer[(4, 4)].modifier.contains(Modifier::BOLD));
    // Send the weekly update (y 5-6): not done → ☐ and the title in normal text, also on the wrapped line;
    // date (x 33-37) in its day's color (normal); the chosen item gets the highlight across both of its
    // lines
    assert_eq!(cell(6, 5), ("☐", theme.text, false));
    assert_eq!(cell(8, 5), ("S", theme.text, false));
    assert_eq!(cell(8, 6), ("u", theme.text, false));
    assert_eq!(cell(34, 5), ("0", theme.text, false));
    assert_eq!(buffer[(1, 5)].bg, theme.selection);
    assert_eq!(buffer[(37, 6)].bg, theme.selection);
    assert_eq!(buffer[(1, 7)].bg, Color::Reset);
    // Fix the login bug (y 7): done → ☑ gray, the title gray and struck through; Hackathon (y 8): past
    // event → ∙ and the title gray, not struck through
    assert_eq!(cell(6, 7), ("☑", theme.muted, false));
    assert_eq!(cell(8, 7), ("F", theme.muted, true));
    assert_eq!(cell(6, 8), ("∙", theme.muted, false));
    assert_eq!(cell(8, 8), ("H", theme.muted, false));
    // ▸ is muted
    assert_eq!(cell(4, 9), ("▸", theme.muted, false));
    // Click the Trip to Tokyo row (y 9): it gets chosen; click ▸ (x 4): it expands
    let click = |app: &mut App, column, row| {
        use ratatui::crossterm::event::{KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
        app.handle_mouse(MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column,
            row,
            modifiers: KeyModifiers::NONE,
        });
    };
    click(&mut app, 10, 9);
    assert_eq!(
        app.tasks_chosen().map(|item| item.title.as_str()),
        Some("Trip to Tokyo")
    );
    assert!(app.expanded.is_empty());
    click(&mut app, 4, 9);
    assert!(app.expanded.contains(&6));
    // Clicking a heading row: ignored
    click(&mut app, 5, 4);
    assert_eq!(
        app.tasks_chosen().map(|item| item.title.as_str()),
        Some("Trip to Tokyo")
    );
    // Clicking the wrapped line of an item (y 6): that item
    click(&mut app, 20, 6);
    assert_eq!(
        app.tasks_chosen().map(|item| item.title.as_str()),
        Some("Send the weekly update")
    );
}

#[test]
fn a_wrapped_item_is_kept_on_screen_whole() {
    // 40 × 15 (7 lines for the list): the last item takes 4 lines; choosing it scrolls so all 4 show,
    // not only its first line and the margin below it (expected rows from tools/expected_screen.py)
    let text = "# Work\n- [ ] a\n- [ ] b\n- [ ] c\n- [ ] d\n- [ ] e\n- [ ] one two three four five six seven eight nine ten eleven twelve thirteen fourteen fifteen sixteen\n";
    let mut app = App::new(
        Lang::En,
        date(2026, 10, 17),
        crate::tasks::parse(text).items,
    );
    app.view = View::Tasks;
    app.focus = Focus::Tasks;
    app.tasks_line = Some(7);
    let terminal = draw_app(&mut app, 40, 15);
    assert_eq!(
        text_rows(terminal.backend().buffer())[4..11],
        [
            "│     ☐\u{a0}c                              │",
            "│     ☐\u{a0}d                              │",
            "│     ☐\u{a0}e                              │",
            "│     ☐\u{a0}one two three four five six    │",
            "│       seven eight nine ten eleven    │",
            "│       twelve thirteen fourteen       │",
            "│       fifteen sixteen                │",
        ]
    );
}

#[test]
fn tasks_page_points_at_format_mistakes() {
    // The header's right side shows how many mistakes there are; choose the line with a mistake (line 43,
    // @2026-13-01): the bottom row says what is wrong instead, and the title is red
    let mut app = tasks_app(Lang::ZhTw);
    app.tasks_line = Some(43);
    let terminal = draw_app(&mut app, 70, 17);
    assert_eq!(
        text_rows(terminal.backend().buffer()),
        [
            "   1 月曆   2 大月曆 ┃ 3 任務",
            "╭────────────────────────────────────────────────────────────────────╮",
            "│ 任務                                                      1 處寫錯 │",
            "│────────────────────────────────────────────────────────────────────│",
            "│ ▾ Personal                                                         │",
            "│     ☐\u{a0}Pay the credit card bill                         10/25       │",
            "│     ☐\u{a0}Organize the bookshelf                                       │",
            "│     ☐\u{a0}Buy Mom's birthday gift                           11/2       │",
            "│     ☐\u{a0}Book the annual physical                         10/16       │",
            "│     ∙ Mom's birthday 🎂                                10/29       │",
            "│     ☐\u{a0}Pay property tax @2026-13-01                                 │",
            "│   ▾ Moving                                                         │",
            "│       ☐\u{a0}Get quotes from movers                          11/7       │",
            "│────────────────────────────────────────────────────────────────────│",
            "│ 第 43 行：@2026-13-01：沒有這一天                                  │",
            "╰────────────────────────────────────────────────────────────────────╯",
            " jk 移動 | hl 收合 | Space 打勾 | e 編輯 | a 新增 | o 編輯器 | s 日曆",
        ]
    );
    let buffer = terminal.backend().buffer();
    let theme = Theme::default();
    // The header's 「1 處寫錯」 and the bottom row are both red
    assert_eq!(buffer[(60, 2)].fg, theme.error);
    assert_eq!(buffer[(2, 14)].fg, theme.error);
    // The title in the Pay property tax row (y 10) is red and underlined, so it does not look like a
    // `!!!` title (titles start at x 8); other rows are neither
    let underlined = |x: u16, y: u16| buffer[(x, y)].modifier.contains(Modifier::UNDERLINED);
    assert_eq!(buffer[(8, 10)].fg, theme.error);
    // Its last character (28 characters: x 8-35), and not the blank after it
    assert!(underlined(8, 10) && underlined(35, 10) && !underlined(36, 10));
    assert_eq!(buffer[(8, 9)].fg, theme.text);
    assert!(!underlined(8, 9));
}

#[test]
fn tasks_page_with_the_side_menu() {
    // 121 cells wide: side menu on the left; unchecked
    // calendars (Payments API v2, Recurring) are left out entirely. 15 tall: all 7 calendars fit
    let mut app = tasks_app(Lang::ZhTw);
    app.groups[0].shown = false;
    app.groups[1].shown = false;
    assert_eq!(
        text_rows(draw_app(&mut app, 121, 16).backend().buffer()),
        [
            "   1 月曆   2 大月曆 ┃ 3 任務",
            "╭──────────────────────╮╭───────────────────────────────────────────────────────────────────────────────────────────────╮",
            "│ 日曆                 ││ 任務                                                                                 1 處寫錯 │",
            "│──────────────────────││───────────────────────────────────────────────────────────────────────────────────────────────│",
            "│ ☐\u{a0}隱藏全部           ││ ▾ Personal                                                                                    │",
            "│ ☐\u{a0}隱藏做完的         ││     ☐\u{a0}Pay the credit card bill                                                    10/25       │",
            "│──────────────────────││     ☐\u{a0}Organize the bookshelf                                                                  │",
            "│ ☐\u{a0}Payments API v2 12 ││     ☐\u{a0}Buy Mom's birthday gift                                                      11/2       │",
            "│ ☐\u{a0}Recurring       14 ││     ☐\u{a0}Book the annual physical                                                    10/16       │",
            "│ ☑\u{a0}Personal         5 ││     ∙ Mom's birthday 🎂                                                           10/29       │",
            "│   ☑\u{a0}Moving         4 ││     ☐\u{a0}Pay property tax @2026-13-01                                                            │",
            "│ ☑\u{a0}US holidays      2 ││   ▾ Moving                                                                                    │",
            "│ ☑\u{a0}Learning Rust 🦀 4 ││───────────────────────────────────────────────────────────────────────────────────────────────│",
            "│ ☑\u{a0}Tokyo trip       5 ││ 第 37 行  - [ ] Pay the credit card bill !!! @2026-10-25                                      │",
            "╰──────────────────────╯╰───────────────────────────────────────────────────────────────────────────────────────────────╯",
            " jk 移動 | hl 收合 | Space 打勾 | e 編輯 | a 新增 | o 編輯器 | s 日曆 | ? 說明 | , 設定 | q 離開",
        ]
    );
}

#[test]
fn a_long_title_wraps_in_the_day_popup() {
    // 60 × 21, layout 2: the long title makes the popup as wide as it gets, the month grid less 2 cells on
    // each side (56, x 2; too wide for blank cells left and right inside); inside the border 54 cells, a
    // single-day item has no date, just one blank cell and its check box in front, and one blank stays at
    // the end, so the title wraps at 50 cells (expected lines from the Python `wrap` in
    // tools/expected_screen.py), the second line under the title (not under the box, so it does not look
    // like the next item); 3 lines + border + a blank row above and below = 7 tall → y 7, the lines at
    // y 9-11
    let text = "- [ ] A really long title that just keeps going and going and going past the edge of the box @2026-10-17\n- [ ] Pack @2026-10-17\n";
    let mut app = month_app(Lang::En, date(2026, 10, 17), text);
    app.focus = Focus::Day;
    app.screen = Rect::new(0, 0, 60, 21);
    let terminal = draw_app(&mut app, 60, 21);
    let buffer = terminal.backend().buffer();
    assert_eq!(day_area(&app), Rect::new(2, 7, 56, 7));
    let rows: Vec<String> = area_rows(buffer, Rect::new(3, 9, 54, 3))
        .iter()
        .map(|row| row.trim_end().to_string())
        .collect();
    assert_eq!(
        rows,
        [
            " ☐\u{a0}A really long title that just keeps going and",
            "   going and going past the edge of the box",
            " ☐\u{a0}Pack",
        ]
    );
    // The chosen (first) item: both lines have the selection background from border to border
    let theme = Theme::default();
    assert_eq!(buffer[(3, 10)].bg, theme.selection);
    assert_eq!(buffer[(56, 10)].bg, theme.selection);
    assert_eq!(buffer[(3, 11)].bg, Color::Reset);
    // The 2 cells left of the box: a blank ring cell, then the month grid's own left border
    assert_eq!(buffer[(1, 10)].symbol(), " ");
    assert_eq!(buffer[(0, 10)].symbol(), "│");
    // A click on the wrapped line is the first item; on the next line, the second
    assert_eq!(day_item_at(&app, 20, 10), Some(0));
    assert_eq!(day_item_at(&app, 20, 11), Some(1));
    // Too short (60 × 9, month grid 7 rows: 5 lines inside the border, no blank rows): 4 short items and
    // the long one last take 6 lines; choosing the long one scrolls one line so both its lines show
    let text = "- [ ] a @2026-10-17\n- [ ] b @2026-10-17\n- [ ] c @2026-10-17\n- [ ] d @2026-10-17\n- [ ] A really long title that just keeps going and going and going past the edge of the box @2026-10-17\n";
    let mut app = month_app(Lang::En, date(2026, 10, 17), text);
    app.focus = Focus::Day;
    app.day_index = 4;
    let terminal = draw_app(&mut app, 60, 9);
    let rows = text_rows(terminal.backend().buffer());
    assert!(
        rows[6].contains("│   going and going past the edge of the box "),
        "{rows:#?}"
    );
    assert!(rows[2].contains("│ ☐\u{a0}b "), "{rows:#?}");
}

#[test]
fn dragging_over_a_wrapped_title_in_the_day_popup_marks_its_characters() {
    // The long title of `a_long_title_wraps_in_the_day_popup` (60 × 20): on both of its lines (y 8, 9) the
    // characters start at x 6; the break left out the space after the first line's 「and」 (char 45).
    // Positions from a separate Python count
    let text = "- [ ] A really long title that just keeps going and going and going past the edge of the box @2026-10-17\n- [ ] Pack !!! @2026-10-17\n";
    let mut app = month_app(Lang::En, date(2026, 10, 17), text);
    app.focus = Focus::Day;
    app.screen = Rect::new(0, 0, 60, 20);
    let mouse = |app: &mut App, kind, column, row| {
        use ratatui::crossterm::event::{KeyModifiers, MouseEvent};
        app.handle_mouse(MouseEvent {
            kind,
            column,
            row,
            modifiers: KeyModifiers::NONE,
        });
    };
    use ratatui::crossterm::event::{
        MouseButton::Left, MouseEventKind::Down, MouseEventKind::Drag,
    };
    let spot = |item, char| TextSpot { item, char };
    // Press on 「title」 (char 14, x 20), drag to the 「d」 of the next line's second 「and」 (char 54, x 14)
    mouse(&mut app, Down(Left), 20, 8);
    mouse(&mut app, Drag(Left), 14, 9);
    assert_eq!(app.day_selection(), Some((spot(0, 14), spot(0, 54))));
    // As text: one line, with the space the break left out
    assert_eq!(
        app.day_selected_text().as_deref(),
        Some("title that just keeps going and going and")
    );
    let terminal = draw_app(&mut app, 60, 20);
    let buffer = terminal.backend().buffer();
    let theme = Theme::default();
    let marked = |x: u16, y: u16| buffer[(x, y)].bg == theme.text_selection;
    // First line: from 「t」 to the end of 「and」 (x 50), not the blank after it; second line: from its start
    // to 「d」; Pack (y 10) not at all
    assert!(!marked(19, 8) && marked(20, 8) && marked(50, 8) && !marked(51, 8));
    assert!(!marked(5, 9) && marked(6, 9) && marked(14, 9) && !marked(15, 9));
    assert!(!marked(8, 10));
    // The rest of the chosen item's lines keep the selection background; marked characters are in the
    // normal text color
    assert_eq!(buffer[(19, 8)].bg, theme.selection);
    assert_eq!(buffer[(20, 8)].fg, theme.text);
    // Past the box's right edge on the second line: up to its last character 「x」 (char 85, x 45)
    mouse(&mut app, Drag(Left), 59, 9);
    assert_eq!(app.day_selection(), Some((spot(0, 14), spot(0, 85))));
    let terminal = draw_app(&mut app, 60, 20);
    let buffer = terminal.backend().buffer();
    assert_eq!(buffer[(45, 9)].bg, theme.text_selection);
    // Down onto Pack (item 1): to the 「c」 under the mouse (x 8). Its `!!!` red turns the normal text color
    // where marked (red is hard to read on the marking), 「k」 stays red
    mouse(&mut app, Drag(Left), 8, 10);
    assert_eq!(app.day_selection(), Some((spot(0, 14), spot(1, 2))));
    let terminal = draw_app(&mut app, 60, 20);
    let buffer = terminal.backend().buffer();
    assert_eq!(
        (buffer[(6, 10)].fg, buffer[(6, 10)].bg),
        (theme.text, theme.text_selection)
    );
    assert_eq!(buffer[(9, 10)].fg, theme.priority_high);
}

#[test]
fn the_day_popup_opens_on_a_day_with_no_items_too() {
    // 60 × 21, layout 2, 10/19 has no items: the popup still opens, its only line a gray 「沒有事」; the
    // narrowest box (30 wide: wider than the date 18 + 2 corners), the padding around the line, 1 + 2 + 2
    // = 5 rows tall, centered on the month grid (60 × 19 from y 1) → x 15, y 8; no counts on the top border
    let mut app = month_app(
        Lang::ZhTw,
        date(2026, 10, 19),
        "- [ ] Buy groceries @2026-10-17\n",
    );
    app.focus = Focus::Day;
    let terminal = draw_app(&mut app, 60, 21);
    let buffer = terminal.backend().buffer();
    assert_eq!(
        area_rows(buffer, Rect::new(15, 8, 30, 5)),
        [
            "╭ 2026/10/19（一） ──────────╮",
            "│                            │",
            "│ 沒有事                     │",
            "│                            │",
            "╰────────────── ☑\u{a0}照日曆分組 ╯",
        ]
    );
    assert_eq!(buffer[(17, 10)].fg, Theme::default().muted);
}

#[test]
fn the_day_popup_lists_every_item_of_the_day_over_the_big_calendar() {
    // 60 × 21, layout 2, 10/17 has 4 items: the popup is centered on the month grid (60 × 19 from y 1);
    // there is room, so a blank row above and below the items and a blank cell left and right:
    // longest item 27 + border 2 + a blank cell 1 + padding 2 = 32, but the top border needs more: the
    // date 18 + 「 未完成 1 / 已完成 1 」 21 + one line cell + 2 corners = 42 wide, 4 + 2 + 2 = 8 rows tall
    // → x 9, y 6; the top border shows the date, one item per row (like layout 1's agenda, but each item
    // starts with its check box, ☑ once done, or `∙` for an event, and only the multi-day item writes its
    // dates), the first item is chosen; the bottom row switches to the popup's keys
    let text = "- [ ] Buy groceries @2026-10-17 17:30\n- Farmers market @2026-10-17 10:00\n\
                    - [x] Clean the garage @2026-10-17\n- Parents in town @2026-10-16..2026-10-18\n";
    let mut app = month_app(Lang::ZhTw, date(2026, 10, 17), text);
    app.focus = Focus::Day;
    let terminal = draw_app(&mut app, 60, 21);
    let buffer = terminal.backend().buffer();
    assert_eq!(
        area_rows(buffer, Rect::new(9, 6, 42, 8)),
        [
            "╭ 2026/10/17（六） ─ 未完成 1 / 已完成 1 ╮",
            "│                                        │",
            "│  ∙ 10/16~18 Parents in town            │",
            "│  ☑\u{a0}Clean the garage                    │",
            "│  ∙ 10:00 Farmers market                │",
            "│  ☐\u{a0}17:30 Buy groceries                 │",
            "│                                        │",
            "╰────────────────────────── ☑\u{a0}照日曆分組 ╯",
        ]
    );
    // The bottom border's right end: the switch for grouping by calendar (on by default). The key hints
    // are 87 cells, so at 60 cells wide 「d 刪除 | c 分組 | Esc 關閉」 is cut
    assert_eq!(
        text_rows(buffer)[20],
        " jk 移動 | hl 換天 | Space 打勾 | e 編輯 | a 新增 | r 改期 |"
    );
    // The border has the color of where the keyboard is; the chosen row (y 8) has the selection
    // background from border to border (x 10-49, the padding cells too), the others none; its dates are
    // in the line's color, like the title; its `∙` (x 12) is colored like a time: muted
    let theme = Theme::default();
    assert_eq!(buffer[(9, 6)].fg, theme.focus);
    assert_eq!(buffer[(12, 8)].symbol(), "∙");
    assert_eq!(buffer[(12, 8)].fg, theme.muted);
    assert_eq!(buffer[(14, 8)].fg, theme.text);
    assert_eq!(buffer[(10, 8)].bg, theme.selection);
    assert_eq!(buffer[(49, 8)].bg, theme.selection);
    assert_eq!(buffer[(50, 8)].bg, Color::Reset);
    assert_eq!(buffer[(11, 9)].bg, Color::Reset);
    // Too short (60 × 7, month grid 5 rows): no rows of padding (the cells left and right stay), the
    // box holds 3 items, and choosing the last one scrolls down one item; a scrollbar on the right
    // border
    app.day_index = 3;
    let terminal = draw_app(&mut app, 60, 7);
    let buffer = terminal.backend().buffer();
    assert_eq!(
        area_rows(buffer, Rect::new(9, 2, 41, 3)),
        [
            "│  ☑\u{a0}Clean the garage                    ",
            "│  ∙ 10:00 Farmers market                ",
            "│  ☐\u{a0}17:30 Buy groceries                 ",
        ]
    );
    assert!((2..5).any(|y| buffer[(50, y)].symbol() == "┃"));
    // Click the top row (Clean the garage): it gets chosen, the scroll stays; clicking the same spot
    // again right away (a double click) is still it (no task file, so checking it off writes nothing)
    app.screen = Rect::new(0, 0, 60, 7);
    let click = |app: &mut App| {
        use ratatui::crossterm::event::{KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
        app.handle_mouse(MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: 12,
            row: 2,
            modifiers: KeyModifiers::NONE,
        });
    };
    click(&mut app);
    assert_eq!(app.day_index, 1);
    let terminal = draw_app(&mut app, 60, 7);
    assert_eq!(
        area_rows(terminal.backend().buffer(), Rect::new(9, 2, 41, 1)),
        ["│  ☑\u{a0}Clean the garage                    "]
    );
    assert_eq!(
        terminal.backend().buffer()[(10, 2)].bg,
        Theme::default().selection
    );
    click(&mut app);
    assert_eq!(app.day_index, 1);
}

#[test]
fn month_view_scrolls_a_row_at_a_time() {
    // 80 × 17: 1 line per week. Right after opening the top week is the one with 10/1 (9/28-10/4);
    // selecting 11/16 → its week is two weeks below the last one (11/2-8), so the grid scrolls down two
    // weeks and the top becomes 10/12
    let mut app = month_app(Lang::En, date(2026, 10, 17), "");
    draw_app(&mut app, 80, 17);
    assert_eq!(app.month_top, Some(date(2026, 9, 28)));
    app.selected = date(2026, 11, 16);
    assert_eq!(
        text_rows(draw_app(&mut app, 80, 17).backend().buffer()),
        [
            "   1 calendar ┃ 2 month   3 tasks",
            "╭────────────────────────────── Mon Nov 16, 2026 ──────────────────────────────╮",
            "│  Wk   Mo         Tu         We         Th        Fr        Sa        Su      │",
            "├─────┬──────────┬──────────┬──────────┬─────────┬─────────┬─────────┬─────────┤",
            "│  42 │ 12       │ 13       │ 14       │ 15      │ 16      │ 17      │ 18      │",
            "├─────┼──────────┼──────────┼──────────┼─────────┼─────────┼─────────┼─────────┤",
            "│  43 │ 19       │ 20       │ 21       │ 22      │ 23      │ 24      │ 25      │",
            "├─────┼──────────┼──────────┼──────────┼─────────┼─────────┼─────────┼─────────┤",
            "│  44 │ 26       │ 27       │ 28       │ 29      │ 30      │ 31      │ 1 Nov   │",
            "├─────┼──────────┼──────────┼──────────┼─────────┼─────────┼─────────┼─────────┤",
            "│  45 │ 2        │ 3        │ 4        │ 5       │ 6       │ 7       │ 8       │",
            "├─────┼──────────┼──────────┼──────────┼─────────┼─────────┼─────────┼─────────┤",
            "│  46 │ 9        │ 10       │ 11       │ 12      │ 13      │ 14      │ 15      │",
            "├─────┼──────────┼──────────┼──────────┼─────────┼─────────┼─────────┼─────────┤",
            "│  47 │ 16       │ 17       │ 18       │ 19      │ 20      │ 21      │ 22      │",
            "╰─────┴──────────┴──────────┴──────────┴─────────┴─────────┴─────────┴─────────╯",
            " hjkl[] move | Tab pane | t today | s calendars | ? help | , settings | q quit",
        ]
    );
    assert_eq!(app.month_top, Some(date(2026, 10, 12)));
    // Select 9/20 (a Sunday, in the week 9/14-20, above the top): that week becomes the top; the
    // selected month becomes September, so October's days turn gray
    app.lang = Lang::ZhTw;
    app.selected = date(2026, 9, 20);
    assert_eq!(
        text_rows(draw_app(&mut app, 80, 17).backend().buffer()),
        [
            "   1 月曆 ┃ 2 大月曆   3 任務",
            "╭────────────────────────────── 2026/9/20（日） ───────────────────────────────╮",
            "│  週   一         二         三         四        五        六        日      │",
            "├─────┬──────────┬──────────┬──────────┬─────────┬─────────┬─────────┬─────────┤",
            "│  38 │ 14       │ 15       │ 16       │ 17      │ 18      │ 19      │ 20      │",
            "├─────┼──────────┼──────────┼──────────┼─────────┼─────────┼─────────┼─────────┤",
            "│  39 │ 21       │ 22       │ 23       │ 24      │ 25      │ 26      │ 27      │",
            "├─────┼──────────┼──────────┼──────────┼─────────┼─────────┼─────────┼─────────┤",
            "│  40 │ 28       │ 29       │ 30       │ 1 10月  │ 2       │ 3       │ 4       │",
            "├─────┼──────────┼──────────┼──────────┼─────────┼─────────┼─────────┼─────────┤",
            "│  41 │ 5        │ 6        │ 7        │ 8       │ 9       │ 10      │ 11      │",
            "├─────┼──────────┼──────────┼──────────┼─────────┼─────────┼─────────┼─────────┤",
            "│  42 │ 12       │ 13       │ 14       │ 15      │ 16      │ 17      │ 18      │",
            "├─────┼──────────┼──────────┼──────────┼─────────┼─────────┼─────────┼─────────┤",
            "│  43 │ 19       │ 20       │ 21       │ 22      │ 23      │ 24      │ 25      │",
            "╰─────┴──────────┴──────────┴──────────┴─────────┴─────────┴─────────┴─────────╯",
            " hjkl[] 移動 | Tab 切換 | t 今天 | s 日曆 | ? 說明 | , 設定 | q 離開",
        ]
    );
    assert_eq!(app.month_top, Some(date(2026, 9, 14)));
    // Moving within the 6 weeks (10/18 is in the last one, 10/12-18): no scrolling
    app.selected = date(2026, 10, 18);
    draw_app(&mut app, 80, 17);
    assert_eq!(app.month_top, Some(date(2026, 9, 14)));
}

#[test]
fn month_view_click_positions_map_to_the_cells() {
    // 80 × 31 (screen: see `month_view_fills_the_space_with_six_equal_weeks`): Mon x 7-16, Tue x 18-27,
    // Thu x 40-48, Sat x 60-68, Sun x 70-78; weeks at y 4-6, 8-10, …, 24-26. The top week is 9/28-10/4,
    // the 6th 11/2-8; the 3 rows left over show the line (y 27) and the first row of 11/9-15 (y 28)
    let mut app = month_app(Lang::ZhTw, date(2026, 10, 17), "");
    app.screen = Rect::new(0, 0, 80, 31);
    draw_app(&mut app, 80, 31);
    // Any cell in a day's box counts as that day (not just the date on the first line); a week holding
    // two months has a day in every cell
    assert_eq!(date_at(&app, 7, 4), Some(date(2026, 9, 28)));
    assert_eq!(date_at(&app, 16, 6), Some(date(2026, 9, 28)));
    assert_eq!(date_at(&app, 40, 4), Some(date(2026, 10, 1)));
    assert_eq!(date_at(&app, 68, 26), Some(date(2026, 11, 7)));
    assert_eq!(date_at(&app, 78, 26), Some(date(2026, 11, 8)));
    // The cut-off 7th week can be clicked too
    assert_eq!(date_at(&app, 10, 28), Some(date(2026, 11, 9)));
    // No date: vertical lines, horizontal lines, the week-number column, the header, the bottom border,
    // the left border, the key hints
    for (x, y) in [
        (17, 4),
        (10, 7),
        (10, 27),
        (3, 4),
        (10, 2),
        (10, 29),
        (0, 4),
        (10, 30),
    ] {
        assert_eq!(date_at(&app, x, y), None, "({x}, {y})");
    }
    // 80 × 30: only 1 row left over, no room for the next week: that row counts as the 6th week
    app.screen = Rect::new(0, 0, 80, 30);
    draw_app(&mut app, 80, 30);
    assert_eq!(date_at(&app, 10, 27), Some(date(2026, 11, 2)));
}

#[test]
fn t_in_the_month_view_draws_todays_week_on_row_2() {
    // 80 × 31: the weeks start at y 4, 8, 12 (3 rows each + a grid line). Today is 10/17 (week 42);
    // from far away, `t` draws week 41 on top and week 42 below it
    use ratatui::crossterm::event::{KeyCode, KeyEvent};
    let mut app = month_app(Lang::En, date(2027, 3, 1), "");
    app.month_top = Some(date(2027, 2, 22));
    app.handle_key(KeyEvent::from(KeyCode::Char('t')));
    let rows = text_rows(draw_app(&mut app, 80, 31).backend().buffer());
    assert!(rows[4].starts_with("│  41 │ 5 "), "{}", rows[4]);
    assert!(rows[8].starts_with("│  42 │ 12 "), "{}", rows[8]);
    // With `[month] center_cursor = true` it stays on row 3 as usual
    app.settings.month.center_cursor = true;
    app.selected = date(2027, 3, 1);
    app.handle_key(KeyEvent::from(KeyCode::Char('t')));
    let rows = text_rows(draw_app(&mut app, 80, 31).backend().buffer());
    assert!(rows[12].starts_with("│  42 │ 12 "), "{}", rows[12]);
}

#[test]
fn switching_to_the_month_view_draws_the_selected_week_on_row_2() {
    // 80 × 31, from layout 1 with Thu 12/24 selected: its week (12/21, week 52) on row 2 (y 8), the week
    // before (12/14, week 51) on top (y 4), not the week of 12/1
    use ratatui::crossterm::event::{KeyCode, KeyEvent};
    let mut app = App::new(Lang::En, date(2026, 10, 17), Vec::new());
    app.selected = date(2026, 12, 24);
    app.handle_key(KeyEvent::from(KeyCode::Char('2')));
    let rows = text_rows(draw_app(&mut app, 80, 31).backend().buffer());
    assert!(rows[4].starts_with("│  51 │ 14 "), "{}", rows[4]);
    assert!(rows[8].starts_with("│  52 │ 21 "), "{}", rows[8]);
}

#[test]
fn a_long_title_does_not_borrow_into_the_next_month() {
    // 10/31 (Sat) has a long title and 11/1 (Sun) next to it is empty: the title stops at the grid
    // line between them (x 69) instead of running into November. 80 × 31: the week of 10/26 is at y 20
    let mut app = month_app(
        Lang::En,
        date(2026, 10, 17),
        "- [ ] Return the rental car keys @2026-10-31\n",
    );
    let terminal = draw_app(&mut app, 80, 31);
    let buffer = terminal.backend().buffer();
    let text: String = (60..79).map(|x| buffer[(x, 21)].symbol()).collect();
    assert_eq!(text, " Return t│         ");
}

#[test]
fn the_selected_days_background_follows_its_long_title() {
    // 80 × 31: 10/20 (Tue) is x 18-27, y 16-18; its long title (37 cells, x 18-54) borrows the empty
    // days up to Sunday. Selected, the background follows the title into them, up to one cell past it
    // (x 55), over the grid line it covers (x 28); the rest of those cells stays plain
    let mut app = month_app(Lang::En, date(2026, 10, 20), MONTH_TASKS);
    let terminal = draw_app(&mut app, 80, 31);
    let buffer = terminal.backend().buffer();
    let theme = &app.theme;
    let bg = |x: u16, y: u16| buffer[(x, y)].bg;
    for (x, y) in [(18, 16), (27, 18), (28, 17), (40, 17), (54, 17), (55, 17)] {
        assert_eq!(bg(x, y), theme.selection, "({x}, {y})");
    }
    for (x, y) in [(56, 17), (59, 17), (78, 17), (28, 16), (29, 16), (29, 18)] {
        assert_ne!(bg(x, y), theme.selection, "({x}, {y})");
    }
    // Another day selected (10/19): the same title has no background
    let mut app = month_app(Lang::En, date(2026, 10, 19), MONTH_TASKS);
    let terminal = draw_app(&mut app, 80, 31);
    let buffer = terminal.backend().buffer();
    assert_eq!(buffer[(7, 17)].bg, app.theme.selection);
    for x in [28, 40, 55] {
        assert_ne!(buffer[(x, 17)].bg, app.theme.selection, "x {x}");
    }
}

#[test]
fn the_selected_day_also_borrows_an_empty_line_next_to_items() {
    // 80 × 31: 10/17 (Sat, x 60-68) has 「 08:00 Coffee chat」 (18 cells) on its 2nd line (y 14); 10/18
    // (Sun, x 70-78) has its own item on the 1st line (y 13), its 2nd line is empty. Selected, 10/17
    // borrows it: the text runs to x 77, the background up to x 78, over the grid line (x 69)
    let mut app = month_app(Lang::En, date(2026, 10, 17), MONTH_TASKS);
    let terminal = draw_app(&mut app, 80, 31);
    let buffer = terminal.backend().buffer();
    let theme = &app.theme;
    let text: String = (60..79).map(|x| buffer[(x, 14)].symbol()).collect();
    assert_eq!(text, " 08:00 Coffee chat ");
    for x in [69, 77, 78] {
        assert_eq!(buffer[(x, 14)].bg, theme.selection, "x {x}");
    }
    // 10/18's own line keeps its own (plain) background
    for x in [70, 78] {
        assert_ne!(buffer[(x, 13)].bg, theme.selection, "x {x}");
    }
    // Not selected (10/16 is): the title stays in its cell, cut at the grid line
    let mut app = month_app(Lang::En, date(2026, 10, 16), MONTH_TASKS);
    let terminal = draw_app(&mut app, 80, 31);
    let buffer = terminal.backend().buffer();
    let text: String = (60..79).map(|x| buffer[(x, 14)].symbol()).collect();
    assert_eq!(text, " 08:00 Co│         ");
}

#[test]
fn with_overflow_off_every_title_stays_in_its_cell() {
    // `[month] overflow = false`, 80 × 31, 10/17 selected: 10/20's long title (x 18-27, y 17) is cut at
    // the grid line (x 28) although the days after it are empty; the selected day's 「 08:00 Coffee chat」
    // (y 14) is cut too, with no background past its cell
    let mut app = month_app(Lang::En, date(2026, 10, 17), MONTH_TASKS);
    app.settings.month.overflow = false;
    let terminal = draw_app(&mut app, 80, 31);
    let buffer = terminal.backend().buffer();
    let text: String = (18..40).map(|x| buffer[(x, 17)].symbol()).collect();
    assert_eq!(text, " A really │          │");
    let text: String = (60..79).map(|x| buffer[(x, 14)].symbol()).collect();
    assert_eq!(text, " 08:00 Co│         ");
    assert_ne!(buffer[(69, 14)].bg, app.theme.selection);
}

#[test]
fn the_cut_off_week_at_the_bottom_stays_inside_the_box() {
    // 80 × 31: the 3 rows left over show the line (y 27) and the first row of 11/9-15 (y 28). The bar
    // of 11/10-12 would go on the next row, the bottom border (y 29): it is left out, the border stays
    let mut app = month_app(
        Lang::En,
        date(2026, 10, 17),
        "- Trip @2026-11-10..2026-11-12\n",
    );
    let terminal = draw_app(&mut app, 80, 31);
    let buffer = terminal.backend().buffer();
    assert!(
        text_rows(buffer)[28].starts_with("│  46 │ 9        │ 10       │ 11  "),
        "{}",
        text_rows(buffer)[28]
    );
    let border: String = (17..50).map(|x| buffer[(x, 29)].symbol()).collect();
    assert_eq!(
        border,
        format!("┴{}┴{}┴{}┴", "─".repeat(10), "─".repeat(10), "─".repeat(9))
    );
    let theme = Theme::default();
    assert_ne!(buffer[(20, 29)].bg, theme.range);
}

#[test]
fn month_view_puts_the_week_across_the_new_year_on_one_row() {
    // 2027/1/1 (Fri) selected: the top week is 12/28-1/3, one row with days of both years; its week
    // number comes from the selected month's days (week 1 of 2027). The year's edge is a thick staircase
    // in the year color: between 12/31 and 1/1 (x 49), then under 12/28-31 (y 7)
    let mut app = month_app(Lang::En, date(2027, 1, 1), "");
    let terminal = draw_app(&mut app, 80, 31);
    assert_eq!(
        text_rows(terminal.backend().buffer()),
        [
            "   1 calendar ┃ 2 month   3 tasks",
            "╭────────────────────────────── Fri Jan 1, 2027 ───────────────────────────────╮",
            "│  Wk   Mo         Tu         We         Th        Fr        Sa        Su      │",
            "├─────┬──────────┬──────────┬──────────┬─────────┰─────────┬─────────┬─────────┤",
            "│   1 │ 28       │ 29       │ 30       │ 31      ┃ 1 Jan   │ 2       │ 3       │",
            "│ Jan │          │          │          │         ┃         │         │         │",
            "│ 2027│          │          │          │         ┃         │         │         │",
            "┝━━━━━┿━━━━━━━━━━┿━━━━━━━━━━┿━━━━━━━━━━┿━━━━━━━━━╃─────────┼─────────┼─────────┤",
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
            "├─────┼──────────┼──────────┼──────────┼─────────┼─────────┼─────────┼─────────┤",
            "│   6 │ 1 Feb    │ 2        │ 3        │ 4       │ 5       │ 6       │ 7       │",
            "│ Feb │          │          │          │         │         │         │         │",
            "│     │          │          │          │         │         │         │         │",
            "├─────┼──────────┼──────────┼──────────┼─────────┼─────────┼─────────┼─────────┤",
            "│   7 │ 8        │ 9        │ 10       │ 11      │ 12      │ 13      │ 14      │",
            "╰─────┴──────────┴──────────┴──────────┴─────────┴─────────┴─────────┴─────────╯",
            " hjkl[] move | Tab pane | t today | s calendars | ? help | , settings | q quit",
        ]
    );
    let buffer = terminal.backend().buffer();
    let theme = Theme::default();
    let cell = |x: u16, y: u16| (buffer[(x, y)].symbol(), buffer[(x, y)].fg);
    assert_eq!(cell(49, 5), ("┃", theme.year_line));
    assert_eq!(cell(49, 3), ("┰", theme.year_line));
    assert_eq!(cell(20, 7), ("━", theme.year_line));
    assert_eq!(cell(3, 7), ("━", theme.year_line));
    assert_eq!(cell(17, 7), ("┿", theme.year_line));
    assert_eq!(cell(49, 7), ("╃", theme.year_line));
    assert_eq!(cell(55, 7), ("─", theme.grid));
    assert_eq!(cell(0, 7), ("┝", theme.focus));
    assert_eq!(cell(79, 7), ("┤", theme.focus));
    // The week with 1/1: the month and the year under its week number, in the label color
    assert_eq!(cell(2, 5), ("J", theme.label));
    assert_eq!(cell(2, 6), ("2", theme.label));
    // February starts on a Monday: the line above 2/1 (y 23) is straight, all in the border color
    assert_eq!(cell(20, 23), ("─", theme.line));
    assert_eq!(cell(75, 23), ("─", theme.line));
    assert_eq!(cell(49, 23), ("┼", theme.line));
    assert_eq!(cell(49, 25), ("│", theme.grid));
    // Selecting 12/31 in the same week: the row stays, and its week number is week 53 of 2026
    app.selected = date(2026, 12, 31);
    let row = text_rows(draw_app(&mut app, 80, 31).backend().buffer())[4].clone();
    assert!(row.starts_with("│  53 │ 28 "), "{row}");
}

#[test]
fn month_view_without_grid_lines_draws_only_the_months_edges() {
    // 80 × 25 without grid lines, 2027/1/1 selected: no grid, only where a month ends: the year's edge
    // thick (between 12/31 and 1/1 in the first column of 1/1's cell, x 49; under 12/28-31 on its own
    // row, y 6), February's straight (2/1 is a Monday; y 19); rows are 3 tall (21 − header 1 − 2 = 18)
    let mut app = month_app(Lang::ZhTw, date(2027, 1, 1), "");
    app.settings.month.grid_lines = false;
    let terminal = draw_app(&mut app, 80, 25);
    assert_eq!(
        text_rows(terminal.backend().buffer()),
        [
            "   1 月曆 ┃ 2 大月曆   3 任務",
            "╭─────────────────────────────── 2027/1/1（五） ───────────────────────────────╮",
            "│  週  一         二         三         四        五        六        日       │",
            "│   1  28         29         30         31       ┃1 1月     2         3        │",
            "│ 1月                                            ┃                             │",
            "│ 2027                                           ┃                             │",
            "┝━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━┛                             │",
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
            "├──────────────────────────────────────────────────────────────────────────────┤",
            "│   6  1 2月      2          3          4         5         6         7        │",
            "│ 2月                                                                          │",
            "│                                                                              │",
            "╰──────────────────────────────────────────────────────────────────────────────╯",
            " hjkl[] 移動 | Tab 切換 | t 今天 | s 日曆 | ? 說明 | , 設定 | q 離開",
        ]
    );
    let buffer = terminal.backend().buffer();
    let theme = Theme::default();
    let cell = |x: u16, y: u16| (buffer[(x, y)].symbol(), buffer[(x, y)].fg);
    assert_eq!(cell(49, 4), ("┃", theme.year_line));
    assert_eq!(cell(49, 6), ("┛", theme.year_line));
    assert_eq!(cell(20, 6), ("━", theme.year_line));
    assert_eq!(cell(3, 6), ("━", theme.year_line));
    assert_eq!(cell(0, 6), ("┝", theme.focus));
    assert_eq!(cell(60, 6).0, " ");
    assert_eq!(cell(79, 6).0, "│");
    assert_eq!(cell(20, 19), ("─", theme.line));
    assert_eq!(cell(0, 19), ("├", theme.focus));
    assert_eq!(cell(79, 19), ("┤", theme.focus));
    // Between other weeks there is no line: the week of 1/11 comes right after the week of 1/4
    assert_eq!(cell(20, 9).0, " ");
}

#[test]
fn month_view_colors_by_role() {
    // 80 × 31 (screen: see `month_view_fills_the_space_with_six_equal_weeks`): Mon x 7, Tue x 18,
    // Wed x 29, Thu x 40, Fri x 50, Sat x 60, Sun x 70; weeks 1-6 at y 4, 8, 12, 16, 20, 24. 10/16 (Fri)
    // selected, 10/15 (Thu) has an unfinished item
    let mut app = month_app(
        Lang::ZhTw,
        date(2026, 10, 16),
        "- [ ] Send the weekly update @2026-10-15\n",
    );
    let terminal = draw_app(&mut app, 80, 31);
    let buffer = terminal.backend().buffer();
    let theme = Theme::default();
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
    assert_eq!(cell(50, 12).0, " ");
    assert_eq!(cell(52, 12), ("6", theme.text, true));
    assert_eq!(cell(53, 12).0, " ");
    // Its whole cell (x 50-58, y 12-14) has the cursor-line background, from the top-left to the
    // bottom-right; the grid lines around it and the neighbor cells do not
    let bg = |x: u16, y: u16| buffer[(x, y)].bg;
    for (x, y) in [(50, 12), (58, 12), (50, 14), (58, 14), (54, 13)] {
        assert_eq!(bg(x, y), theme.selection, "({x}, {y})");
    }
    for (x, y) in [(49, 12), (59, 12), (50, 11), (50, 15), (48, 12), (60, 12)] {
        assert_ne!(bg(x, y), theme.selection, "({x}, {y})");
    }
    // 10/15's long title does not borrow into the selected cell: it stops at the grid line
    assert_eq!(cell(41, 13).0, "S");
    assert_eq!(cell(50, 13).0, " ");
    // Today green, weekends blue, other days normal (10/15 has an unfinished item: no different)
    assert_eq!(cell(62, 12), ("7", theme.today, true));
    assert_eq!(cell(42, 12), ("5", theme.text, true));
    assert_eq!(cell(72, 12), ("8", theme.weekend, true));
    // Dates in other months: not grayed out (the day's color, 9/28 a Monday: normal), only not bold;
    // 10/1's month (same row) is in the label color
    assert_eq!(cell(9, 4), ("8", theme.text, false));
    assert_eq!(cell(43, 4), ("1", theme.label, false));
    // Week numbers: the selected row (week 42) bright, other rows (week 40) dim
    assert_eq!(cell(3, 12), ("4", theme.week_number, false));
    assert_eq!(cell(3, 4), ("4", theme.week_number_dim, false));
    // Header: weekdays in the normal text color, Sat and Sun in the weekend color
    assert_eq!(cell(8, 2), ("一", theme.text, false));
    assert_eq!(cell(61, 2), ("六", theme.weekend, false));
    // Grid lines are faint within a month; the joints where they meet the outer border
    // have the border's color (keyboard in the calendar: focus color)
    assert_eq!(cell(6, 3), ("┬", theme.grid, false));
    assert_eq!(cell(6, 4), ("│", theme.grid, false));
    assert_eq!(cell(6, 11), ("┼", theme.grid, false));
    assert_eq!(cell(45, 7), ("─", theme.grid, false));
    // Where September ends, a staircase in the border color, like layout 1's month lines: the
    // vertical stretch between 9/30 (Wed) and 10/1 (Thu) at x 39, then the line under 9/28-30 and the
    // week-number column (y 7); its crossings take that color too
    assert_eq!(cell(39, 5), ("│", theme.line, false));
    assert_eq!(cell(39, 3), ("┬", theme.line, false));
    assert_eq!(cell(39, 7), ("┼", theme.line, false));
    assert_eq!(cell(6, 7), ("┼", theme.line, false));
    assert_eq!(cell(10, 7), ("─", theme.line, false));
    assert_eq!(cell(3, 7), ("─", theme.line, false));
    assert_eq!(cell(49, 7), ("┼", theme.grid, false));
    // Where October ends (11/1 is a Sunday): the line above 11/1 (y 19, x 70-78), the vertical stretch
    // between 10/31 and 11/1 (x 69, y 20-22), the line under 10/26-31 (y 23)
    assert_eq!(cell(74, 19), ("─", theme.line, false));
    assert_eq!(cell(65, 19), ("─", theme.grid, false));
    assert_eq!(cell(69, 21), ("│", theme.line, false));
    assert_eq!(cell(65, 23), ("─", theme.line, false));
    assert_eq!(cell(74, 23), ("─", theme.grid, false));
    // The rows left over under the 6th week: the line before the next week (y 27) and that week cut
    // off; the vertical lines reach the bottom border, which joins them as ┴ (in the border color)
    assert_eq!(cell(6, 27), ("┼", theme.grid, false));
    assert_eq!(cell(6, 28), ("│", theme.grid, false));
    assert_eq!(cell(6, 29), ("┴", theme.focus, false));
    assert_eq!(cell(0, 7), ("├", theme.focus, false));
    assert_eq!(cell(79, 7), ("┤", theme.focus, false));
    // The date in the middle of the top border is in the normal color: 18 cells wide in a 78-cell
    // border → starts at 1 + (78 - 18) / 2 = 31, so its 「2」 is at x 32
    assert_eq!(cell(32, 1), ("2", theme.text, false));
}

#[test]
fn selected_day_is_bold_in_its_own_color() {
    // Select 10/16 (Fri): the number is in the normal color and bold; today 10/17 keeps today's color
    let terminal = draw(Lang::ZhTw, 47, 27, date(2026, 10, 16));
    let buffer = terminal.backend().buffer();
    let theme = Theme::default();
    let cell = |x: u16, y: u16| {
        let cell = &buffer[(x, y)];
        (
            cell.symbol(),
            cell.fg,
            cell.modifier.contains(Modifier::BOLD),
        )
    };
    assert_eq!(cell(28, 6), ("[", theme.selected, true));
    assert_eq!(cell(30, 6), ("6", theme.text, true));
    assert_eq!(cell(31, 6), ("]", theme.selected, true));
    assert_eq!(cell(34, 6), ("7", theme.today, true));
}

#[test]
fn dots_right_of_each_date_count_its_items() {
    // The week of 10/12 at y 6; day k's dots cell is at x 12 + 4k + 3 (border, year/month, week number, │,
    // then a blank and the date before it): 10/13 has 2 items (one in Errands), 10/14 2, 10/15 3 (one done),
    // 10/16 5 (Trip and 4 more: still 4 dots), 10/17 and 10/18 only Trip (a multi-day item counts on every
    // day it covers); 10/12 none
    let text = "# Work\n- [ ] Plan @2026-10-13\n- [ ] Pack @2026-10-14\n- [ ] Print @2026-10-14\n\
                - [x] Pay @2026-10-15\n- Call @2026-10-15\n- Lunch @2026-10-15\n\
                - Trip @2026-10-16..2026-10-18\n- [ ] A @2026-10-16\n- [ ] B @2026-10-16\n\
                - [ ] C @2026-10-16\n- [ ] D @2026-10-16\n# Errands\n- [ ] Shop @2026-10-13\n";
    let mut app = App::new(
        Lang::En,
        date(2026, 10, 17),
        crate::tasks::parse(text).items,
    );
    let theme = Theme::default();
    let cells = |app: &mut App| {
        let terminal = draw_app(app, 47, 27);
        let buffer = terminal.backend().buffer();
        [15, 19, 23, 27, 31, 35, 39]
            .map(|x| (buffer[(x, 6)].symbol().to_string(), buffer[(x, 6)].fg))
    };
    let dot = |symbol: &str| (symbol.to_string(), theme.muted);
    // 10/17 is selected and has a dot: its `]` comes after the dot (x 36)
    assert_eq!(
        cells(&mut app),
        [
            dot(" "),
            dot("⠒"),
            dot("⠒"),
            dot("⠖"),
            dot("⠶"),
            dot("⠂"),
            dot("⠂")
        ]
    );
    let terminal = draw_app(&mut app, 47, 27);
    assert_eq!(terminal.backend().buffer()[(36, 6)].symbol(), "]");
    // An unchecked calendar's items are not counted (10/13 loses Shop), nor are done tasks hidden by
    // 「Hide done」 (10/15 loses Pay)
    let errands = app
        .groups
        .iter()
        .position(|group| group.name.as_deref() == Some("Errands"))
        .expect("Errands");
    app.groups[errands].shown = false;
    app.settings.calendar_list.hide_done = true;
    assert_eq!(
        cells(&mut app),
        [
            dot(" "),
            dot("⠂"),
            dot("⠒"),
            dot("⠒"),
            dot("⠶"),
            dot("⠂"),
            dot("⠂")
        ]
    );
}

#[test]
fn dots_take_the_color_of_the_days_most_urgent_open_item() {
    // The week of 10/12 at y 6 (day k's dots at x 12 + 4k + 3), today 10/17: 10/12 a plain task (muted),
    // 10/13 `!!!` among `!` and none (red), 10/14 `!!` and `!` (orange), 10/15 a done `!!!` and an open `!`
    // (yellow: done tasks do not count), 10/16 a done `!!!` and a past `!!!` event (muted), 10/17 a holiday
    // (blue), 10/18 a holiday and a `!` (yellow: a priority beats the holiday)
    let text = "# Work\n- [ ] Plain @2026-10-12\n- [ ] Plan @2026-10-13\n- [ ] Fix ! @2026-10-13\n\
                - [ ] Ship !!! @2026-10-13\n- [ ] Pack !! @2026-10-14\n- [ ] Print ! @2026-10-14\n\
                - [x] Pay !!! @2026-10-15\n- [ ] Mail ! @2026-10-15\n- [x] Done !!! @2026-10-16\n\
                - Lunch !!! @2026-10-16\n- [ ] Chore ! @2026-10-18\n\
                # Holidays type:holiday\n- Day off @2026-10-17\n- Day off 2 @2026-10-18\n";
    let mut app = App::new(
        Lang::En,
        date(2026, 10, 17),
        crate::tasks::parse(text).items,
    );
    let theme = Theme::default();
    let terminal = draw_app(&mut app, 47, 27);
    let buffer = terminal.backend().buffer();
    let cells = [15, 19, 23, 27, 31, 35, 39]
        .map(|x| (buffer[(x, 6)].symbol().to_string(), buffer[(x, 6)].fg));
    let dot = |symbol: &str, color| (symbol.to_string(), color);
    assert_eq!(
        cells,
        [
            dot("⠂", theme.muted),
            dot("⠖", theme.priority_high),
            dot("⠒", theme.priority_medium),
            dot("⠒", theme.priority_low),
            dot("⠒", theme.muted),
            dot("⠂", theme.weekend),
            dot("⠒", theme.priority_low),
        ]
    );
}

#[test]
fn a_multi_day_items_underline_runs_on_between_its_days() {
    // A (10/13-14), B (10/15-16) right after it, Trip (10/17-20) into the next week. Day k's date is at
    // x 12 + 4k + 1..2 and the cell after it (its dots) at 12 + 4k + 3; the week of 10/12 at y 6, of 10/19
    // at y 7
    let text = "- A @2026-10-13..2026-10-14\n- B @2026-10-15..2026-10-16\n- Trip @2026-10-17..2026-10-20\n";
    let mut app = App::new(
        Lang::En,
        date(2026, 10, 17),
        crate::tasks::parse(text).items,
    );
    let terminal = draw_app(&mut app, 47, 27);
    let buffer = terminal.backend().buffer();
    let underlined = |x: u16, y: u16| buffer[(x, y)].modifier.contains(Modifier::UNDERLINED);
    // Between the days of one item: 13-14 (A), 15-16 (B), 17-18 (Trip: 17's dot, then the `]` in 18's
    // blank), 19-20 (Trip, next week)
    for (x, y) in [
        (19, 6),
        (20, 6),
        (27, 6),
        (28, 6),
        (35, 6),
        (36, 6),
        (15, 7),
        (16, 7),
    ] {
        assert!(underlined(x, y), "({x}, {y})");
    }
    // Between two items (14-15, 16-17 at the `[`), before the first day (12-13), after Sunday (18) and
    // after the last day (20-21): not
    for (x, y) in [
        (16, 6),
        (23, 6),
        (24, 6),
        (31, 6),
        (32, 6),
        (39, 6),
        (19, 7),
        (20, 7),
    ] {
        assert!(!underlined(x, y), "({x}, {y})");
    }
    // The cells between two days take the first day's color: 13 the normal color, 17 today's
    let theme = Theme::default();
    assert_eq!(buffer[(20, 6)].underline_color, theme.text);
    assert_eq!(buffer[(36, 6)].underline_color, theme.today);
}

#[test]
fn overlapping_multi_day_items_underline_in_their_own_color() {
    // C (10/13-16) and D (10/15-18) overlap on 10/15-16: those dates and the cells between them take
    // `range_overlap`; the rest take their day's color as usual (cells between two days: the earlier
    // day's). The week of 10/12 at y 6, day k's date at x 12 + 4k + 1..2
    let text = "- C @2026-10-13..2026-10-16\n- D @2026-10-15..2026-10-18\n";
    let mut app = App::new(
        Lang::En,
        date(2026, 10, 17),
        crate::tasks::parse(text).items,
    );
    let terminal = draw_app(&mut app, 47, 27);
    let buffer = terminal.backend().buffer();
    let theme = Theme::default();
    let expected = [
        (20, theme.text),          // between 13 and 14: C
        (24, theme.text),          // between 14 and 15: only C covers both
        (25, theme.range_overlap), // 15: C and D
        (28, theme.range_overlap), // between 15 and 16: C and D
        (30, theme.range_overlap), // 16: C and D
        (32, theme.text),          // between 16 and 17 (17's `[`): only D
        (34, theme.today),         // 17 (today): D
        (36, theme.today),         // between 17 and 18 (the `]`): D
        (38, theme.weekend),       // 18 (Sunday): D
    ];
    for (x, color) in expected {
        let cell = &buffer[(x, 6)];
        assert!(cell.modifier.contains(Modifier::UNDERLINED), "x {x}");
        assert_eq!(cell.underline_color, color, "x {x}");
    }
}

#[test]
fn an_in_progress_title_is_bold_with_an_icon_in_every_view() {
    // `[/]` Draft and the `[/]` multi-day Trip are bold with 🚧 in front; open Shop is not; done Clean is
    // struck through
    let text = "- [ ] Shop @2026-10-17\n- [/] Draft @2026-10-17\n- [x] Clean @2026-10-17\n\
                - [/] Trip @2026-10-16..2026-10-18\n";
    let mut app = App::new(
        Lang::En,
        date(2026, 10, 17),
        crate::tasks::parse(text).items,
    );
    // The screen size the popup's area is worked out from (the same as each draw below)
    app.screen = Rect::new(0, 0, 100, 50);
    // Where `word` starts inside `area` (counting cells: 🚧 takes 2) and its first cell's (bold, struck
    // through)
    let style = |buffer: &Buffer, area: Rect, word: &str| {
        let rows = area_rows(buffer, area);
        let (y, row) = rows
            .iter()
            .enumerate()
            .find(|(_, row)| row.contains(word))
            .unwrap_or_else(|| panic!("{word} is not on screen: {rows:#?}"));
        let x = Span::raw(&row[..row.find(word).unwrap()]).width();
        let cell = &buffer[(
            area.x + u16::try_from(x).unwrap(),
            area.y + u16::try_from(y).unwrap(),
        )];
        (
            cell.modifier.contains(Modifier::BOLD),
            cell.modifier.contains(Modifier::CROSSED_OUT),
        )
    };
    // `popup` = look only inside layout 2's day popup (the month grid behind it shows the same titles)
    let check = |app: &mut App, view: &str, popup: bool| {
        // Tall enough for 10/17's 4 items in its month grid cell
        let terminal = draw_app(app, 100, 50);
        let buffer = terminal.backend().buffer();
        let area = if popup { day_area(app) } else { buffer.area };
        let text = area_rows(buffer, area).join("\n");
        assert!(
            text.contains("🚧 Draft") && text.contains("🚧 Trip"),
            "{view}: {text}"
        );
        assert_eq!(text.matches('🚧').count(), 2, "{view}: {text}");
        assert_eq!(style(buffer, area, "Draft"), (true, false), "{view}");
        assert_eq!(style(buffer, area, "Trip"), (true, false), "{view}");
        assert_eq!(style(buffer, area, "Shop"), (false, false), "{view}");
        assert_eq!(style(buffer, area, "Clean"), (false, true), "{view}");
    };
    check(&mut app, "layout 1", false);
    app.view = View::Month;
    check(&mut app, "layout 2", false);
    app.focus = Focus::Day;
    check(&mut app, "day popup", true);
    app.view = View::Tasks;
    app.focus = Focus::Tasks;
    check(&mut app, "tasks page", false);
}

#[test]
fn with_cross_out_done_off_no_title_is_struck_through_in_any_view() {
    // `cross_out_done = false`: done Clean is only gray, no cell is struck through; turned back on, its 5
    // letters are (in the day popup: counting only inside the popup)
    let text = "- [ ] Shop @2026-10-17\n- [x] Clean @2026-10-17\n";
    let mut app = App::new(
        Lang::En,
        date(2026, 10, 17),
        crate::tasks::parse(text).items,
    );
    app.screen = Rect::new(0, 0, 100, 50);
    // How many cells are struck through with the setting `on` (`popup`: only inside layout 2's day popup)
    let struck = |app: &mut App, on: bool, popup: bool| {
        app.settings.cross_out_done = on;
        let terminal = draw_app(app, 100, 50);
        let buffer = terminal.backend().buffer();
        let area = if popup { day_area(app) } else { buffer.area };
        let rows = area_rows(buffer, area).join("\n");
        assert!(rows.contains("Clean"), "{rows}");
        (area.top()..area.bottom())
            .flat_map(|y| (area.left()..area.right()).map(move |x| (x, y)))
            .filter(|&at| buffer[at].modifier.contains(Modifier::CROSSED_OUT))
            .count()
    };
    let check = |app: &mut App, view: &str, popup: bool| {
        assert_eq!(struck(app, false, popup), 0, "{view}");
        assert_eq!(struck(app, true, popup), 5, "{view}");
    };
    check(&mut app, "layout 1", false);
    app.view = View::Month;
    check(&mut app, "layout 2", false);
    app.focus = Focus::Day;
    check(&mut app, "day popup", true);
    app.view = View::Tasks;
    app.focus = Focus::Tasks;
    check(&mut app, "tasks page", false);
}

#[test]
fn an_in_progress_item_leaves_its_time_out_of_a_month_grid_cell_but_the_day_popup_shows_it() {
    // In a narrow cell, 「18:30 🚧」 would leave no room for the title
    let text = "- [/] Team happy hour @2026-10-17 18:30\n- [ ] Buy groceries @2026-10-17 17:30\n";
    let mut app = month_app(Lang::En, date(2026, 10, 17), text);
    app.screen = Rect::new(0, 0, 80, 30);
    let terminal = draw_app(&mut app, 80, 30);
    let grid = text_rows(terminal.backend().buffer()).join("\n");
    assert!(
        grid.contains("│ 🚧 Team") && !grid.contains("18:30"),
        "{grid}"
    );
    // An item not in progress keeps its time
    assert!(grid.contains("17:30 Buy"), "{grid}");
    // The day popup has room: the time is there
    app.focus = Focus::Day;
    let terminal = draw_app(&mut app, 80, 30);
    let popup = area_rows(terminal.backend().buffer(), day_area(&app)).join("\n");
    assert!(popup.contains("18:30 🚧 Team happy hour"), "{popup}");
}

#[test]
fn the_day_popup_writes_each_items_outermost_calendar_at_the_right() {
    // 10/17: one item outside any calendar, one in CAIS > Reports, one `!!!` in Life, a holiday.
    // Widest line: 「 ☐ Send the report」 18 + 「 CAIS」 5 + border 2 + a blank cell 1 + padding 2 = 28;
    // the top border needs more: the date 18 + 「 3 open / 0 done 」 17 + one line cell + 2 corners = 38
    let text = "- [ ] Loose end @2026-10-17\n# CAIS\n## Reports\n- [ ] Send the report @2026-10-17\n\
                # Life\n- [ ] Pay the fine !!! @2026-10-17\n# Holidays type:holiday\n- Day off @2026-10-17\n";
    let mut app = month_app(Lang::En, date(2026, 10, 17), text);
    app.screen = Rect::new(0, 0, 80, 24);
    app.focus = Focus::Day;
    app.settings.day_popup.by_calendar = false;
    let terminal = draw_app(&mut app, 80, 24);
    let buffer = terminal.backend().buffer();
    let area = day_area(&app);
    // Not grouped by calendar (☐ on the bottom border): the outermost calendar ends one blank cell before
    // the padding; none outside a calendar or for a holiday
    assert_eq!(
        area_rows(buffer, area),
        [
            "╭ Sat Oct 17, 2026 ─ 3 open / 0 done ╮",
            "│                                    │",
            "│  ☐\u{a0}Loose end                       │",
            "│  ☐\u{a0}Send the report           CAIS  │",
            "│  ☐\u{a0}Pay the fine              Life  │",
            "│  ∙ Day off                         │",
            "│                                    │",
            "╰───────────────────── ☐\u{a0}By calendar ╯",
        ]
    );
    // The calendar and the check box are colored like the time: muted, or the whole line's color with a
    // priority (or a holiday's)
    let theme = Theme::default();
    assert_eq!(buffer[(area.x + 31, area.y + 3)].symbol(), "C");
    assert_eq!(buffer[(area.x + 31, area.y + 3)].fg, theme.muted);
    assert_eq!(buffer[(area.x + 31, area.y + 4)].symbol(), "L");
    assert_eq!(buffer[(area.x + 31, area.y + 4)].fg, theme.priority_high);
    assert_eq!(buffer[(area.x + 3, area.y + 3)].symbol(), "☐");
    assert_eq!(buffer[(area.x + 3, area.y + 3)].fg, theme.muted);
    assert_eq!(buffer[(area.x + 3, area.y + 4)].fg, theme.priority_high);
    assert_eq!(buffer[(area.x + 3, area.y + 5)].symbol(), "∙");
    assert_eq!(buffer[(area.x + 3, area.y + 5)].fg, theme.weekend);
}

#[test]
fn the_day_popup_lists_items_under_their_calendars_as_a_tree() {
    // By time the calendars take turns (Day off, CAIS > Reports 08:00, Life 09:00, CAIS 17:00, no calendar
    // 20:00); grouped (the default): the item outside any calendar first, then the calendars in the
    // calendar list's order as a tree like the tasks page (CAIS's own items, then Reports inside it), each
    // name on its own line, 2 cells further in per level, and its items 2 cells further in than it, by
    // time inside it. Widest line: 「     ☐ 08:00 Send the report」 28 + border 2 + a blank cell 1 +
    // padding 2 = 33; the top border needs more: the date 18 + 「 4 open / 0 done 」 17 + one line cell +
    // 2 corners = 38. Expected rows from a separate Python count of the indents
    let text = "- [ ] Loose end @2026-10-17 20:00\n# CAIS\n- [ ] Ship the fix @2026-10-17 17:00\n\
                ## Reports\n- [ ] Send the report @2026-10-17 08:00\n\
                # Life\n- [ ] Pay the fine !!! @2026-10-17 09:00\n# Holidays type:holiday\n- Day off @2026-10-17\n";
    let mut app = month_app(Lang::En, date(2026, 10, 17), text);
    app.screen = Rect::new(0, 0, 80, 24);
    app.focus = Focus::Day;
    let titles: Vec<&str> = app
        .day_items()
        .iter()
        .map(|item| item.title.as_str())
        .collect();
    assert_eq!(
        titles,
        [
            "Loose end",
            "Ship the fix",
            "Send the report",
            "Pay the fine",
            "Day off"
        ]
    );
    let terminal = draw_app(&mut app, 80, 24);
    let buffer = terminal.backend().buffer();
    let area = day_area(&app);
    assert_eq!(
        area_rows(buffer, area),
        [
            "╭ Sat Oct 17, 2026 ─ 4 open / 0 done ╮",
            "│                                    │",
            "│  ☐\u{a0}20:00 Loose end                 │",
            "│  CAIS                              │",
            "│    ☐\u{a0}17:00 Ship the fix            │",
            "│    Reports                         │",
            "│      ☐\u{a0}08:00 Send the report       │",
            "│  Life                              │",
            "│    ☐\u{a0}09:00 Pay the fine            │",
            "│  Holidays                          │",
            "│    ∙ Day off                       │",
            "│                                    │",
            "╰───────────────────── ☑\u{a0}By calendar ╯",
        ]
    );
    // A calendar's name is in the color of the tasks page's calendar headings, at every level
    let theme = Theme::default();
    assert_eq!(buffer[(area.x + 3, area.y + 3)].symbol(), "C");
    assert_eq!(buffer[(area.x + 3, area.y + 3)].fg, theme.label);
    assert_eq!(buffer[(area.x + 5, area.y + 5)].symbol(), "R");
    assert_eq!(buffer[(area.x + 5, area.y + 5)].fg, theme.label);
    // Clicking a calendar's name chooses nothing; the line under it is its first item
    assert_eq!(day_item_at(&app, area.x + 5, area.y + 3), None);
    assert_eq!(day_item_at(&app, area.x + 5, area.y + 4), Some(1));
    assert_eq!(day_item_at(&app, area.x + 7, area.y + 5), None);
    assert_eq!(day_item_at(&app, area.x + 7, area.y + 6), Some(2));
    // The calendars' headings show the chosen item's path, so the bottom border does not (only the switch)
    app.day_index = 2;
    let terminal = draw_app(&mut app, 80, 24);
    let rows = area_rows(terminal.backend().buffer(), area);
    assert_eq!(rows[12], "╰───────────────────── ☑\u{a0}By calendar ╯");
    // Only Reports has an item that day: CAIS still gets its heading above it
    let text = "# CAIS\n## Reports\n- [ ] Send the report @2026-10-17 08:00\n";
    let mut app = month_app(Lang::En, date(2026, 10, 17), text);
    app.screen = Rect::new(0, 0, 80, 24);
    app.focus = Focus::Day;
    let terminal = draw_app(&mut app, 80, 24);
    let rows = area_rows(terminal.backend().buffer(), day_area(&app));
    assert!(rows[2].starts_with("│  CAIS "), "{rows:#?}");
    assert!(rows[3].starts_with("│    Reports "), "{rows:#?}");
    assert!(
        rows[4].starts_with("│      ☐\u{a0}08:00 Send the report "),
        "{rows:#?}"
    );
}

#[test]
fn the_day_popup_lists_subtasks_under_their_parent() {
    // 10/17: 「Plan the trip」 has no date, its subtask 「Book flights」 and that one's 「Compare prices」 are
    // on the day; 「Fly out」 (07:40) comes before Book flights (09:00) by time, but the family stays
    // together, in the place of its first item (Compare prices, no time). Widest line: 「      ☐ 09:00 Book
    // flights」 25 + border 2 + a blank cell 1 + padding 2 = 30; the top border needs more: 38 (as in
    // `the_day_popup_lists_items_under_their_outermost_calendar`). Expected rows from a separate Python
    // count of the indents
    let text = "# Trip\n- [ ] Plan the trip\n  - [ ] Book flights @2026-10-17 09:00\n\
                \x20   - [ ] Compare prices @2026-10-17\n- Fly out @2026-10-17 07:40\n";
    let mut app = month_app(Lang::En, date(2026, 10, 17), text);
    app.screen = Rect::new(0, 0, 80, 25);
    app.focus = Focus::Day;
    let terminal = draw_app(&mut app, 80, 25);
    let buffer = terminal.backend().buffer();
    let area = day_area(&app);
    // Grouped by calendar: the parent not on the day where a check box would be, gray, with no box of its
    // own; each level 2 cells further in. The first item (Book flights) is chosen
    assert_eq!(
        area_rows(buffer, area),
        [
            "╭ Sat Oct 17, 2026 ─ 2 open / 0 done ╮",
            "│                                    │",
            "│  Trip                              │",
            "│    Plan the trip                   │",
            "│      ☐\u{a0}09:00 Book flights          │",
            "│        ☐\u{a0}Compare prices            │",
            "│    ∙ 07:40 Fly out                 │",
            "│                                    │",
            "╰───────────────────── ☑\u{a0}By calendar ╯",
        ]
    );
    let theme = Theme::default();
    assert_eq!(buffer[(area.x + 5, area.y + 3)].symbol(), "P");
    assert_eq!(buffer[(area.x + 5, area.y + 3)].fg, theme.muted);
    // The parent is not an item of the popup: clicking it chooses nothing, j skips it
    assert_eq!(day_item_at(&app, area.x + 6, area.y + 3), None);
    assert_eq!(day_item_at(&app, area.x + 6, area.y + 4), Some(0));
    let titles: Vec<&str> = app
        .day_items()
        .iter()
        .map(|item| item.title.as_str())
        .collect();
    assert_eq!(titles, ["Book flights", "Compare prices", "Fly out"]);
    // Not grouped: the same tree, starting at the left, each item's calendar at the right end (none for
    // the parent), the chosen one's path on the bottom border
    app.settings.day_popup.by_calendar = false;
    let terminal = draw_app(&mut app, 80, 25);
    assert_eq!(
        area_rows(terminal.backend().buffer(), day_area(&app)),
        [
            "╭ Sat Oct 17, 2026 ─ 2 open / 0 done ╮",
            "│                                    │",
            "│  Plan the trip                     │",
            "│    ☐\u{a0}09:00 Book flights      Trip  │",
            "│      ☐\u{a0}Compare prices        Trip  │",
            "│  ∙ 07:40 Fly out             Trip  │",
            "│                                    │",
            "╰ Trip ─────────────── ☐\u{a0}By calendar ╯",
        ]
    );
    // Too short (60 × 7: 3 lines inside the box): after scrolling down to Fly out, choosing Book flights
    // again brings both headings above it back into view (the calendar's and the parent's)
    app.settings.day_popup.by_calendar = true;
    app.screen = Rect::new(0, 0, 60, 7);
    app.day_index = 2;
    let terminal = draw_app(&mut app, 60, 7);
    let rows = area_rows(terminal.backend().buffer(), day_area(&app));
    assert!(rows[3].starts_with("│    ∙ 07:40 Fly out "), "{rows:#?}");
    app.day_index = 0;
    let terminal = draw_app(&mut app, 60, 7);
    let rows = area_rows(terminal.backend().buffer(), day_area(&app));
    assert!(rows[1].starts_with("│  Trip "), "{rows:#?}");
    assert!(rows[2].starts_with("│    Plan the trip "), "{rows:#?}");
    assert!(
        rows[3].starts_with("│      ☐\u{a0}09:00 Book flights "),
        "{rows:#?}"
    );
}

#[test]
fn the_day_popup_writes_the_chosen_items_calendar_path_on_its_bottom_border_when_not_grouped() {
    // Box 38 wide: the top border's date 18 + 「 2 open / 0 done 」 17 + one line cell + 2 corners
    let text =
        "- [ ] Loose end @2026-10-17\n# CAIS\n## Reports\n- [ ] Send the report @2026-10-17\n";
    let mut app = month_app(Lang::En, date(2026, 10, 17), text);
    app.screen = Rect::new(0, 0, 80, 24);
    app.focus = Focus::Day;
    app.settings.day_popup.by_calendar = false;
    let bottom = |app: &mut App| {
        let terminal = draw_app(app, 80, 24);
        let buffer = terminal.backend().buffer();
        let area = day_area(app);
        let rows = area_rows(buffer, area);
        let color = buffer[(area.x + 2, area.bottom() - 1)].fg;
        (rows.last().cloned().unwrap_or_default(), color)
    };
    // Not grouped, the chosen item is in CAIS > Reports: its whole path on the left of the bottom border,
    // in the color of the tasks page's calendar headings; the switch stays at the right end
    app.day_index = 1;
    assert_eq!(
        bottom(&mut app),
        (
            "╰ CAIS > Reports ───── ☐\u{a0}By calendar ╯".to_string(),
            Theme::default().label
        )
    );
    // The border after the path keeps its own color (the path is 16 cells: 「─」 at x 17 is the border)
    let terminal = draw_app(&mut app, 80, 24);
    let buffer = terminal.backend().buffer();
    let area = day_area(&app);
    let y = area.bottom() - 1;
    let theme = Theme::default();
    assert_eq!(buffer[(area.x + 17, y)].symbol(), "─");
    assert_eq!(buffer[(area.x + 17, y)].fg, theme.focus);
    // The chosen item is outside any calendar: only the switch
    app.day_index = 0;
    assert_eq!(
        bottom(&mut app).0,
        "╰───────────────────── ☐\u{a0}By calendar ╯"
    );
    // Grouped, the calendars' headings already show the path: only the switch, for every item
    app.settings.day_popup.by_calendar = true;
    app.day_index = 1;
    assert_eq!(
        bottom(&mut app).0,
        "╰───────────────────── ☑\u{a0}By calendar ╯"
    );
    // ☑ in the `checked` color, its name in the text color (the box is taller now, with the headings)
    let terminal = draw_app(&mut app, 80, 24);
    let buffer = terminal.backend().buffer();
    let area = day_area(&app);
    let y = area.bottom() - 1;
    assert_eq!(buffer[(area.right() - 15, y)].symbol(), "☑");
    assert_eq!(buffer[(area.right() - 15, y)].fg, theme.checked);
    assert_eq!(buffer[(area.right() - 13, y)].fg, theme.text);
    // A click there flips it: the switch's 15 cells end next to the corner
    assert_eq!(
        day_switch_area(&app),
        Rect::new(area.right() - 16, y, 15, 1)
    );
    // Not grouped, a path too long for the room left of the switch is cut, one line cell before the switch
    // (38 - 2 corners - the switch 15 - one line cell = 20 cells)
    let text =
        "# CAIS\n## 2603_Monthly_report_handover_to_CAIS\n- [ ] Send the report @2026-10-17\n";
    let mut app = month_app(Lang::En, date(2026, 10, 17), text);
    app.screen = Rect::new(0, 0, 80, 24);
    app.focus = Focus::Day;
    app.settings.day_popup.by_calendar = false;
    assert_eq!(
        bottom(&mut app).0,
        "╰ CAIS > 2603_Monthly─ ☐\u{a0}By calendar ╯"
    );
}

#[test]
fn a_toast_covers_the_right_end_of_the_bottom_row() {
    // 60 × 20, the popup open: its keys on the bottom row (96 cells, cut at 60), the note over their end
    // with a blank cell on each side, in the notice color (expected row from a separate Python count)
    let mut app = month_app(Lang::En, date(2026, 10, 17), "- [ ] Pack @2026-10-17\n");
    app.focus = Focus::Day;
    app.toast = Some((Toast::Copied(11), std::time::Instant::now()));
    let terminal = draw_app(&mut app, 60, 20);
    let buffer = terminal.backend().buffer();
    assert_eq!(
        text_rows(buffer)[19],
        " jk move | hl day | Space check | e edit |  Copied 11 chars"
    );
    // (`text_rows` drops blanks at the end: the last cell is the note's blank)
    assert_eq!(buffer[(58, 19)].symbol(), "s");
    assert_eq!(buffer[(59, 19)].symbol(), " ");
    assert_eq!(buffer[(44, 19)].fg, Theme::default().notice);
    // One character, and in Chinese; failing to copy
    let texts = |lang: Lang, toast| lang.texts().toast(toast);
    assert_eq!(texts(Lang::En, Toast::Copied(1)), "Copied 1 char");
    assert_eq!(texts(Lang::ZhTw, Toast::Copied(11)), "已複製 11 個字");
    assert_eq!(texts(Lang::ZhTw, Toast::CopyFailed), "沒辦法複製到剪貼簿");
    // Gone: the keys come back whole
    app.toast = None;
    let terminal = draw_app(&mut app, 60, 20);
    assert_eq!(
        text_rows(terminal.backend().buffer())[19],
        " jk move | hl day | Space check | e edit | a add | r resched"
    );
}

#[test]
fn the_day_popups_top_border_counts_the_days_tasks_on_the_right() {
    // 10/17: 2 open (more than half of 3: red), 1 done (always green); an event does not count
    let text = "- [ ] Email Kevin @2026-10-17\n- [ ] Draft the notes @2026-10-17\n\
                - [x] Pay the fine @2026-10-17\n- Standup @2026-10-17 09:00\n";
    let mut app = month_app(Lang::En, date(2026, 10, 17), text);
    app.screen = Rect::new(0, 0, 80, 24);
    app.focus = Focus::Day;
    let terminal = draw_app(&mut app, 80, 24);
    let buffer = terminal.backend().buffer();
    let area = day_area(&app);
    let top = &area_rows(buffer, area)[0];
    // The counts end at the top-right corner, the date stays on the left
    assert!(top.starts_with("╭ Sat Oct 17, 2026 ─"), "{top}");
    assert!(top.ends_with(" 2 open / 1 done ╮"), "{top}");
    // Colors: 「2 open」 red, 「/」 muted, 「1 done」 green (`today`)
    let theme = Theme::default();
    // Where `text` starts, in cells (the border characters take several bytes each)
    let x_of = |text: &str| {
        area.x + u16::try_from(Span::raw(&top[..top.find(text).unwrap()]).width()).unwrap()
    };
    assert_eq!(buffer[(x_of("2 open"), area.y)].fg, theme.priority_high);
    assert_eq!(buffer[(x_of("/ 1"), area.y)].fg, theme.muted);
    assert_eq!(buffer[(x_of("1 done"), area.y)].fg, theme.today);
}

#[test]
fn the_tab_bar_lists_the_views_and_lights_the_shown_one() {
    // The top row: a cell for the bar (┃ on the shown view), then 「 N name 」 for each view; the names stay
    // where they are whichever view is shown (expected rows from `tab_row` in tools/expected_screen.py)
    let top =
        |app: &mut App, width| text_rows(draw_app(app, width, 12).backend().buffer())[0].clone();
    let mut app = App::new(Lang::En, date(2026, 10, 17), Vec::new());
    assert_eq!(top(&mut app, 60), " ┃ 1 calendar   2 month   3 tasks");
    app.view = View::Tasks;
    app.focus = Focus::Tasks;
    assert_eq!(top(&mut app, 60), "   1 calendar   2 month ┃ 3 tasks");
    app.view = View::Month;
    app.focus = Focus::Calendar;
    assert_eq!(top(&mut app, 60), "   1 calendar ┃ 2 month   3 tasks");
    // Colors (layout 2 shown: its bar at x 14, 「 2 month 」 at x 15-23; nord-bright, computed apart): the
    // whole tab on one background, the selection color #5c5366 a quarter of the way to the theme's
    // background nord0 #2e3440; the bar is the focus color #c69bbe a little faint, 35% of the way to the
    // selection color; the text bold in the focus color; the other tabs muted, on no background
    let terminal = draw_app(&mut app, 60, 12);
    let buffer = terminal.backend().buffer();
    let theme = Theme::default();
    let cell = |x: u16| {
        let cell = &buffer[(x, 0)];
        (
            cell.symbol(),
            cell.fg,
            cell.bg,
            cell.modifier.contains(Modifier::BOLD),
        )
    };
    let tab = Color::Rgb(81, 75, 93);
    assert_eq!(cell(14), ("┃", Color::Rgb(161, 130, 159), tab, false));
    assert_eq!(cell(16), ("2", theme.focus, tab, true));
    for x in 15..=23 {
        let (_, fg, bg, bold) = cell(x);
        assert_eq!((fg, bg, bold), (theme.focus, tab, true), "x {x}");
    }
    assert_eq!(cell(13).2, Color::Reset);
    assert_eq!(cell(24).2, Color::Reset);
    assert_eq!(cell(3), ("1", theme.muted, Color::Reset, false));
    assert_eq!(cell(26), ("3", theme.muted, Color::Reset, false));
    // The terminal theme does not know the background under it: the whole tab on the plain selection color
    app.theme = Theme::builtin("terminal").expect("terminal");
    let terminal = draw_app(&mut app, 60, 12);
    let buffer = terminal.backend().buffer();
    assert!((14..=23).all(|x| buffer[(x, 0)].bg == app.theme.selection));
    // In Chinese; on a narrow screen the end is cut off
    let mut app = App::new(Lang::ZhTw, date(2026, 10, 17), Vec::new());
    assert_eq!(top(&mut app, 60), " ┃ 1 月曆   2 大月曆   3 任務");
    assert_eq!(top(&mut app, 25), " ┃ 1 月曆   2 大月曆   3");
}

#[test]
fn with_the_tab_bar_off_the_views_take_the_top_row() {
    // `tab_bar = false`: no tab bar; each view's screen is the one a row taller with the tab bar, minus
    // that row (the views start on the top row)
    for (view, focus) in [
        (View::Planner, Focus::Calendar),
        (View::Month, Focus::Calendar),
        (View::Tasks, Focus::Tasks),
    ] {
        let mut with_bar = App::new(Lang::En, date(2026, 10, 17), Vec::new());
        (with_bar.view, with_bar.focus) = (view, focus);
        let rows_with_bar = text_rows(draw_app(&mut with_bar, 47, 27).backend().buffer());
        let mut app = App::new(Lang::En, date(2026, 10, 17), Vec::new());
        (app.view, app.focus) = (view, focus);
        app.settings.tab_bar = false;
        let rows = text_rows(draw_app(&mut app, 47, 26).backend().buffer());
        assert_eq!(rows[..], rows_with_bar[1..], "{view:?}");
        assert!(rows[0].starts_with('╭'), "{view:?}: {}", rows[0]);
    }
    // Clicks: the top row is no tab, and the dates are a row higher than with the tab bar (October on
    // top: the 16th at x 28-31, y 5)
    let mut app = App::new(Lang::En, date(2026, 10, 17), Vec::new());
    app.settings.tab_bar = false;
    app.screen = Rect::new(0, 0, 47, 26);
    draw_app(&mut app, 47, 26);
    assert_eq!(tab_at(&app, 3, 0), None);
    assert_eq!(date_at(&app, 29, 5), Some(date(2026, 10, 16)));
}
