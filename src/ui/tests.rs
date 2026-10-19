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
        Click::Group(_) | Click::HideDone | Click::TaskRow { .. } | Click::View(_) => None,
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
            " 123 版型  hjkl[] 移動  Tab 切換  t",
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
            " 123 view  hjkl[] move  Tab pane  t",
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
            " 123 view  hjkl[] move  Tab pane  t",
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
            " 123 版型  hjkl[] 移動  Tab 切換  t",
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
            " 123 view  hjkl[] move  Tab pane  t",
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
            " 123 view  hjkl[] move  Tab pane  t",
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
            " 123 版型  hjkl[] 移動  Tab 切換  t",
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
const AGENDA_TASKS: &str = include_str!("../../testdata/agenda.md");

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
    // 121 × 12: the side menu fits 5 calendars (y 3-7; below them a rule and the 「隱藏做完的」 row,
    // y 8-9), and there are 12 (c01-c12, 1 item each)
    let text: String = (1..=12)
        .map(|n| format!("# c{n:02}\n- a @2026-10-{n:02}\n"))
        .collect();
    let mut app = App::new(
        Lang::ZhTw,
        date(2026, 10, 17),
        crate::tasks::parse(&text).items,
    );
    app.screen = Rect::new(0, 0, 121, 12);
    app.focus = Focus::Side;
    let names = |app: &mut App| {
        let terminal = draw_app(app, 121, 12);
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
    // The rule and the switch below the list (not scrolled with it); clicking the switch turns it on
    let terminal = draw_app(&mut app, 121, 12);
    let bottom = area_rows(terminal.backend().buffer(), Rect::new(0, 8, 22, 2));
    assert_eq!(bottom[0], "│────────────────────│");
    assert!(bottom[1].starts_with("│ ☐ 隱藏做完的"), "{}", bottom[1]);
    assert_eq!(click_at(&app, 5, 9), Some(Click::HideDone));
    assert_eq!(click_at(&app, 5, 8), None);
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
        use ratatui::crossterm::event::{KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
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
        " jk 移動  Space 勾選  a/A 新增日曆  e 改名  d 刪除  Esc 回去"
    );
    // Keyboard not in the side menu: not drawn; the key hints have s even when narrow (60 cells wide,
    // the end is cut off)
    app.focus = Focus::Calendar;
    let terminal = draw_app(&mut app, 60, 14);
    assert_eq!(terminal.backend().buffer()[(0, 3)].symbol(), "│");
    assert_ne!(terminal.backend().buffer()[(4, 3)].symbol(), "W");
    assert!(
        text_rows(terminal.backend().buffer())[13]
            .starts_with(" 123 版型  hjkl[] 移動  Tab 切換  t 今天  s 日曆")
    );
}

#[test]
fn the_shown_layouts_key_lights_up_on_the_bottom_row_and_clicks_switch() {
    let theme = Theme::default();
    for (view, focus, lit) in [
        (View::Planner, Focus::Calendar, 1),
        (View::Month, Focus::Calendar, 2),
        (View::Tasks, Focus::Tasks, 3),
    ] {
        let mut app = App::new(Lang::En, date(2026, 10, 17), Vec::new());
        (app.view, app.focus) = (view, focus);
        app.screen = Rect::new(0, 0, 100, 20);
        let terminal = draw_app(&mut app, 100, 20);
        let buffer = terminal.backend().buffer();
        assert!(text_rows(buffer)[19].starts_with(" 123 view  "), "{view:?}");
        for x in 1..=3 {
            let cell = &buffer[(x, 19)];
            let expected = if x == lit {
                (theme.focus, true)
            } else {
                (theme.muted, false)
            };
            assert_eq!(
                (cell.fg, cell.modifier.contains(Modifier::BOLD)),
                expected,
                "{view:?} {x}"
            );
        }
        // A click on a key = that layout; right next to the keys: nothing
        assert_eq!(click_at(&app, 1, 19), Some(Click::View(View::Planner)));
        assert_eq!(click_at(&app, 2, 19), Some(Click::View(View::Month)));
        assert_eq!(click_at(&app, 3, 19), Some(Click::View(View::Tasks)));
        assert_eq!(click_at(&app, 0, 19), None);
        assert_eq!(click_at(&app, 4, 19), None);
    }
    // Keyboard in the side menu: the bottom row has the side menu's keys, nothing to click there
    let mut app = App::new(Lang::En, date(2026, 10, 17), Vec::new());
    app.focus = Focus::Side;
    app.screen = Rect::new(0, 0, 100, 20);
    let terminal = draw_app(&mut app, 100, 20);
    assert!(text_rows(terminal.backend().buffer())[19].starts_with(" jk move"));
    assert_eq!(click_at(&app, 1, 19), None);
}

#[test]
fn side_menu_lists_the_calendars_when_wide() {
    // 121 cells wide: the calendars are listed on the far left (the task file's groups, in order of
    // appearance; items before any heading are 「未分組」), the number is how many items have a date
    // (Work has one item without a date); the last name is too long even at the 43-cell max width
    // (`[calendar_list] max_width`'s default), so it is cut; the key hints gain an `s`. The agenda has room for one column only and October's 8 items
    // do not fit: it scrolls 1 row, leaving one row after the selected day's (10/17) items
    let mut app = agenda_app(Lang::ZhTw);
    let terminal = draw_app(&mut app, 121, 14);
    assert_eq!(
        text_rows(terminal.backend().buffer()),
        [
            "╭─────────────────────────────────────────╮╭─────────────────────────────────╮╭─────────────────────────────────────────╮",
            "│ 日曆                                    ││2026年 週 │ 一 二 三 四 五 六 日 ││ 重要事項               2026/10/17（六） │",
            "│─────────────────────────────────────────││──────────┼──────────────────────││─────────────────────────────────────────│",
            "│ ☑ 未分組                              2 ││  10月 40 │           1  2  3  4 ││ 10/10 Farmers market                    │",
            "│ ☑ Personal                            5 ││       41 │  5  6  7  8  9 10 11 ││ 10/15 Fix the login bug                 ┃",
            "│ ☑ Work                                1 ││       42 │ 12 13 14 15 16[17]18 ││ 10/16 17:30 Send the weekly update      ┃",
            "│   ☑ Year-end odds and ends and plans  2 ││       43 │ 19 20 21 22 23 24 25 ││ 10/17 Dinner with friends               ┃",
            "│                                         ││       44 │ 26 27 28 29 30 31    ││ 10/17 08:00 Coffee chat                 ┃",
            "│                                         ││          │                      ││ 10/18 07:00 Go hiking                   │",
            "│                                         ││──────────┼──────────────────────││───────────────────────────── 還有 1 件 ─│",
            "│─────────────────────────────────────────││  11月 44 │                    1 ││  11/3 09:30 Release v2.3                │",
            "│ ☐ 隱藏做完的                            ││       45 │  2  3  4  5  6  7  8 ││                                         │",
            "╰─────────────────────────────────────────╯╰─────────────────────────────────╯╰─────────────────────────────────────────╯",
            " 123 版型  hjkl[] 移動  Tab 切換  t 今天  s 日曆  ? 說明  , 設定  q 離開",
        ]
    );
    // ☑ uses the `checked` color, names are normal text, numbers
    // and the header are muted, the border is a normal line
    let theme = Theme::default();
    let buffer = terminal.backend().buffer();
    let cell = |x: u16, y: u16| (buffer[(x, y)].symbol(), buffer[(x, y)].fg);
    assert_eq!(cell(2, 3), ("☑", theme.checked));
    assert_eq!(cell(4, 4), ("P", theme.text));
    assert_eq!(cell(40, 4), ("5", theme.muted));
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
            " 123 版型  hjkl[] 移動  Tab 切換  t 今天  s 日曆  ? 說明  , 設定  q 離開",
        ]
    );
}

#[test]
fn unchecked_calendars_hide_their_items() {
    // Personal unchecked: ☐ and the name are muted, the number still counts; its 5 items are not listed
    let mut app = agenda_app(Lang::En);
    app.groups[1].shown = false;
    let terminal = draw_app(&mut app, 121, 14);
    assert_eq!(
        text_rows(terminal.backend().buffer()),
        [
            "╭─────────────────────────────────────────╮╭─────────────────────────────────╮╭─────────────────────────────────────────╮",
            "│ Calendars                               ││  2026 Wk │ Mo Tu We Th Fr Sa Su ││ Agenda                 Sat Oct 17, 2026 │",
            "│─────────────────────────────────────────││──────────┼──────────────────────││─────────────────────────────────────────│",
            "│ ☑ Ungrouped                           2 ││   Oct 40 │           1  2  3  4 ││ 10/15 Fix the login bug                 │",
            "│ ☐ Personal                            5 ││       41 │  5  6  7  8  9 10 11 ││ 10/16 17:30 Send the weekly update      │",
            "│ ☑ Work                                1 ││       42 │ 12 13 14 15 16[17]18 ││ 10/20 A really long title that just keep│",
            "│   ☑ Year-end odds and ends and plans  2 ││       43 │ 19 20 21 22 23 24 25 ││                                         │",
            "│                                         ││       44 │ 26 27 28 29 30 31    ││                                         │",
            "│                                         ││──────────┼──────────────────────││─────────────────────────────────────────│",
            "│                                         ││   Nov 44 │                    1 ││  11/3 09:30 Release v2.3                │",
            "│─────────────────────────────────────────││       45 │  2  3  4  5  6  7  8 ││                                         │",
            "│ ☐ Hide done                             ││       46 │  9 10 11 12 13 14 15 ││                                         │",
            "╰─────────────────────────────────────────╯╰─────────────────────────────────╯╰─────────────────────────────────────────╯",
            " 123 view  hjkl[] move  Tab pane  t today  s calendars  ? help  , settings  q quit",
        ]
    );
    let theme = Theme::default();
    let buffer = terminal.backend().buffer();
    let cell = |x: u16, y: u16| (buffer[(x, y)].symbol(), buffer[(x, y)].fg);
    assert_eq!(cell(2, 4), ("☐", theme.muted));
    assert_eq!(cell(4, 4), ("P", theme.muted));
    assert_eq!(cell(2, 3), ("☑", theme.checked));
    // Dates keep their usual colors whatever items they have: 10/5 (Monday), 10/16 (Friday, an
    // ungrouped item) normal, 10/10 (Saturday) weekend blue
    assert_eq!(cell(57, 4), ("5", theme.text));
    assert_eq!(cell(72, 4), ("0", theme.weekend));
    assert_eq!(cell(69, 5), ("6", theme.text));
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
    let theme = Theme::default();
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
    let theme = Theme::default();
    let buffer = terminal.backend().buffer();
    assert_eq!(buffer[(0, 0)].fg, theme.focus);
    assert_eq!(buffer[(43, 0)].fg, theme.line);
    for x in 1..42 {
        assert_eq!(buffer[(x, 4)].bg, theme.selection, "x = {x}");
        assert_ne!(buffer[(x, 3)].bg, theme.selection, "x = {x}");
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
    for (x, y) in [(5, 7), (5, 1), (5, 2), (0, 3), (42, 3), (5, 13)] {
        assert_eq!(click_at(&app, x, y), None, "({x}, {y})");
    }
    // The calendar moved 43 cells right: the 16th is at x 67-69
    assert_eq!(date_at(&app, 67, 5), Some(date(2026, 10, 16)));
    assert_eq!(date_at(&app, 70, 5), Some(date(2026, 10, 17)));
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
            " 123 版型  hjkl[] 移動  Tab 切換  t 今天  s 日曆  ? 說明  , 設定  q 離",
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
            " 123 view  hjkl[] move  Tab pane  t today  s calendars  ? help  , settings  q quit",
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
fn item_colors_follow_the_item() {
    // Screen: see `agenda_lists_each_months_items_next_to_it`;
    // the inside of the agenda box starts at x 36
    let mut app = agenda_app(Lang::ZhTw);
    let terminal = draw_app(&mut app, 70, 23);
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
    // Agenda header: 「重要事項」 muted, the selected day on
    // the right in the normal color (2026/10/17 starts at x 52)
    assert_eq!(cell(37, 1), ("重", theme.muted, false));
    assert_eq!(cell(52, 1), ("2", theme.text, false));
    // 10/5 has an unfinished item: date and title in the normal color
    assert_eq!(cell(38, 3), ("1", theme.text, false));
    assert_eq!(cell(43, 3), ("P", theme.text, false));
    // 10/15's finished item: the whole line is gray (the date too), the title struck through
    assert_eq!(cell(37, 5), ("1", theme.muted, false));
    assert_eq!(cell(43, 5), ("F", theme.muted, true));
    // 10/17 is today and 10/18 a Sunday, but the line follows the item, not its day: the normal color;
    // the time is muted
    assert_eq!(cell(43, 7), ("D", theme.text, false));
    assert_eq!(cell(43, 8), ("0", theme.muted, false));
    assert_eq!(cell(49, 8), ("C", theme.text, false));
    assert_eq!(cell(49, 9), ("G", theme.text, false));
}

#[test]
fn the_selected_week_row_has_a_background() {
    // The week of 10/17 is at y 5: that whole row in the calendar box (x 1-33) gets the highlight
    // background; the rows above and below, and the same row in the agenda, do not
    let mut app = agenda_app(Lang::ZhTw);
    let theme = Theme::default();
    let bg =
        |terminal: &Terminal<TestBackend>, x: u16, y: u16| terminal.backend().buffer()[(x, y)].bg;
    let terminal = draw_app(&mut app, 70, 23);
    for x in 1..=33 {
        assert_eq!(bg(&terminal, x, 5), theme.selection, "x {x}");
    }
    assert_eq!(bg(&terminal, 0, 5), Color::Reset);
    assert_eq!(bg(&terminal, 20, 4), Color::Reset);
    assert_eq!(bg(&terminal, 20, 6), Color::Reset);
    assert_eq!(bg(&terminal, 40, 5), Color::Reset);
    // The week that crosses months is split into two rows: with 10/31 selected, only the October half
    // (y 7) gets the highlight, the November half (y 12) does not
    app.selected = date(2026, 10, 31);
    let terminal = draw_app(&mut app, 70, 23);
    assert_eq!(bg(&terminal, 20, 7), theme.selection);
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
            " 123 版型  hjkl[] 移動  Tab 切換  t 今天  s 日曆  ? 說明  , 設定  q 離",
        ]
    );
    let buffer = terminal.backend().buffer();
    let theme = Theme::default();
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
        assert_eq!(buffer[(x, 8)].bg, theme.selection, "x {x}");
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
        crate::tasks::parse(include_str!("../../testdata/tasks.md")).items,
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
            " 123 版型  hjkl[] 移動  Tab 切換  t 今天  s 日曆  ? 說明  , 設定  q 離",
        ]
    );
    assert_eq!(app.agenda_window, Some((date(2026, 10, 1), 3)));
    assert_eq!(app.calendar_top, Some(0));
    // The scrollbar thumb has the same color as the border (keyboard in the agenda: focus color)
    let theme = Theme::default();
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
            " 123 版型  hjkl[] 移動  Tab 切換  t 今天  s 日曆  ? 說明  , 設定  q 離",
        ]
    );
    assert_eq!(app.agenda_window, Some((date(2026, 10, 1), 3)));
    assert_eq!(app.calendar_top, Some(0));
    // That day's items get the highlight background (like
    // its week on the calendar); other days' items do not
    let buffer = terminal.backend().buffer();
    let theme = Theme::default();
    assert_eq!(buffer[(40, 7)].bg, theme.selection);
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
        crate::tasks::parse(include_str!("../../testdata/tasks.md")).items,
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
            " 123 版型  hjkl[] 移動  Tab 切換  t 今天  s 日曆  ? 說明  , 設定  q 離",
        ]
    );
    // The hint is muted; the line on both sides of it keeps the
    // line color (「還」 is at x 58, the line's last cell at x 68)
    let buffer = terminal.backend().buffer();
    let theme = Theme::default();
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
    // 70 × 12 (main view 70 × 11): the edit box is in the middle, 2 cells from each side (x 2-67), as tall as
    // what is in it: the typed text, a blank row, what it reads as (title, date), a blank row, the hint row;
    // with a blank row above and below and the border, 10 tall at y (11 − 10) / 2 = 0
    app.screen = Rect::new(0, 0, 70, 12);
    let mut terminal = draw_app(&mut app, 70, 12);
    let area = editor_area(&app);
    assert_eq!(area, Rect::new(2, 0, 66, 10));
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
            "│  可以打  日期 @2026-10-17  時間 @10:00  優先度 ! !! !!!",
            "",
        ]
    );
    assert!(
        rows[9].starts_with("╰ Enter 存檔  Esc 取消 ─"),
        "{}",
        rows[9]
    );
    // The item is on the selected day (choosing it in the agenda selected 10/16), so its date is left
    // out and the time stays as @17:30. The cursor is right after the typed text, at box x 2 + border
    // 1 + padding 1 + space 1 + 「- [ ] Pick up the cake 🎂 @17:30」 32 cells (31 chars)
    assert_eq!(
        terminal.get_cursor_position().expect("cursor"),
        Position::new(37, 2)
    );
    // Labels muted, values in the text color (the title row: 標 at x 5, the value at x 5 + 4 + 2)
    let theme = Theme::default();
    assert_eq!(terminal.backend().buffer()[(5, 4)].fg, theme.muted);
    assert_eq!(terminal.backend().buffer()[(11, 4)].fg, theme.text);
    // A time that does not exist: it stays in the title as text, and a red row says what is wrong; the
    // box grows by that row (11 tall, y 0)
    for code in [
        KeyCode::Backspace,
        KeyCode::Backspace,
        KeyCode::Char('9'),
        KeyCode::Char('9'),
    ] {
        app.handle_key(KeyEvent::from(code));
    }
    let terminal = draw_app(&mut app, 70, 12);
    let area = editor_area(&app);
    assert_eq!(area, Rect::new(2, 0, 66, 11));
    let rows = area_rows(terminal.backend().buffer(), area);
    assert_eq!(trimmed(&rows[4]), "│  標題  Pick up the cake 🎂 17:99");
    assert_eq!(trimmed(&rows[6]), "│  17:99：沒有這個時間");
    assert_eq!(terminal.backend().buffer()[(5, 6)].fg, theme.error);
    // Adding: the calendar it goes into (日曆), a gray hint where the title goes; the hint row wraps (it
    // does not fit 62 cells), the next line under the first thing; too tall for the blank rows above and
    // below: 10 tall at y 0
    let mut app = App::new(
        Lang::ZhTw,
        date(2026, 10, 17),
        crate::tasks::parse("# Work\n## SFAP\n- [ ] Estimate @2026-10-17\n").items,
    );
    app.handle_key(KeyEvent::from(KeyCode::Tab));
    app.handle_key(KeyEvent::from(KeyCode::Char('a')));
    app.screen = Rect::new(0, 0, 70, 12);
    let terminal = draw_app(&mut app, 70, 12);
    let area = editor_area(&app);
    assert_eq!(area, Rect::new(2, 0, 66, 10));
    let rows = area_rows(terminal.backend().buffer(), area);
    assert!(rows[0].starts_with("╭ 新增事項 ─"), "{}", rows[0]);
    assert_eq!(
        rows.iter().map(trimmed).collect::<Vec<_>>()[1..9],
        [
            "│  - [ ]",
            "",
            "│  標題  （打在 - [ ] 後面）",
            "│  日期  2026/10/17（六）",
            "│  日曆  Work > SFAP",
            "",
            "│  可以打  日期 @2026-10-17  時間 @10:00  優先度 ! !! !!!",
            "│          日曆 #名字",
        ]
    );
    assert_eq!(terminal.backend().buffer()[(11, 3)].fg, theme.muted);
}

#[test]
fn typing_a_priority_colors_the_line_and_its_word_in_the_preview_right_away() {
    use ratatui::crossterm::event::{KeyCode, KeyEvent};
    // Adding at 80 × 24 with a priority: the box is (2, 6, 76, 11) (title, date, priority, then the hint row),
    // the typed text on row 8 from x 5; the fields on rows 10-12, the hint row on row 14
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
    let terminal = draw_app(&mut app, 80, 24);
    assert_eq!(terminal.backend().buffer()[(5, 8)].symbol(), "-");
    assert_eq!(terminal.backend().buffer()[(5, 8)].fg, theme.priority_high);
    // With a title: the priority field says 高 in red, its label muted; the title in the text color
    type_text(&mut app, " Pay the fine");
    let terminal = draw_app(&mut app, 80, 24);
    let buffer = terminal.backend().buffer();
    assert_eq!(buffer[(5, 8)].fg, theme.priority_high);
    assert_eq!(color_of(buffer, 12, "高"), Some(theme.priority_high));
    assert_eq!(color_of(buffer, 12, "優"), Some(theme.muted));
    assert_eq!(color_of(buffer, 10, "P"), Some(theme.text));
    // In the typed text, `!!!` is in italics, the title is not
    let italic_on_8 = |symbol: &str| {
        (0..80)
            .map(|x| &buffer[(x, 8)])
            .find(|cell| cell.symbol() == symbol)
            .map(|cell| cell.modifier.contains(Modifier::ITALIC))
    };
    assert_eq!(italic_on_8("!"), Some(true));
    assert_eq!(italic_on_8("P"), Some(false));
    // The hint row: `! !! !!!` each in its priority's color, what to type in blue (the help page's section color), the labels
    // muted
    let marks: Vec<Color> = (0..80)
        .map(|x| &buffer[(x, 14)])
        .filter(|cell| cell.symbol() == "!")
        .map(|cell| cell.fg)
        .collect();
    let (low, medium, high) = (
        theme.priority_low,
        theme.priority_medium,
        theme.priority_high,
    );
    assert_eq!(marks, [low, medium, medium, high, high, high]);
    assert_eq!(color_of(buffer, 14, "@"), Some(theme.help_section));
    assert_eq!(color_of(buffer, 14, "日"), Some(theme.muted));
    // `!!` instead: orange; no `!`: the normal text color, no priority field (the box is a row shorter: the
    // fields on rows 10-11, the hint row on row 13)
    app.handle_key(KeyEvent::from(KeyCode::Home));
    for _ in 0..6 {
        app.handle_key(KeyEvent::from(KeyCode::Right));
    }
    app.handle_key(KeyEvent::from(KeyCode::Delete));
    let terminal = draw_app(&mut app, 80, 24);
    let buffer = terminal.backend().buffer();
    assert_eq!(buffer[(5, 8)].fg, theme.priority_medium);
    assert_eq!(color_of(buffer, 12, "中"), Some(theme.priority_medium));
    for _ in 0..2 {
        app.handle_key(KeyEvent::from(KeyCode::Delete));
    }
    let terminal = draw_app(&mut app, 80, 24);
    let buffer = terminal.backend().buffer();
    assert_eq!(buffer[(5, 8)].fg, theme.text);
    assert!((10..13).all(|row| color_of(buffer, row, "優").is_none()));
}

#[test]
fn an_empty_title_is_a_gray_hint_until_enter_cannot_save_it() {
    use ratatui::crossterm::event::{KeyCode, KeyEvent};
    let mut app = App::new(Lang::ZhTw, date(2026, 10, 17), Vec::new());
    app.screen = Rect::new(0, 0, 80, 24);
    app.handle_key(KeyEvent::from(KeyCode::Char('a')));
    let theme = Theme::default();
    let rows = |app: &mut App| -> Vec<String> {
        let terminal = draw_app(app, 80, 24);
        area_rows(terminal.backend().buffer(), editor_area(app))
            .iter()
            .map(|row| row.trim_end_matches([' ', '│']).to_string())
            .collect()
    };
    // Just opened: the title field is a gray hint, nothing red
    let opened = rows(&mut app);
    assert!(opened.contains(&"│  標題  （打在 - [ ] 後面）".to_string()));
    assert!(!opened.iter().any(|row| row.contains("標題是空的")));
    let terminal = draw_app(&mut app, 80, 24);
    assert_eq!(terminal.backend().buffer()[(11, 10)].fg, theme.muted);
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
    // calendar it goes inside; the hint row: type:holiday, and `#` and a name picks where it goes.
    // 80 × 24: the box is (2, 6, 76, 10)
    let mut app = App::new(Lang::ZhTw, date(2026, 10, 17), Vec::new());
    app.set_items(crate::tasks::parse(
        "# Work\n## SFAP\n- [ ] Fix the login bug\n",
    ));
    app.focus = Focus::Side;
    app.group_index = 1;
    app.screen = Rect::new(0, 0, 80, 24);
    app.handle_key(KeyEvent::from(KeyCode::Char('A')));
    let terminal = draw_app(&mut app, 80, 24);
    assert_eq!(editor_area(&app), Rect::new(2, 6, 76, 10));
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
            "│  可以打  假日日曆 type:holiday  上層日曆 #名字",
        ]
    );
    assert!(
        rows[9].starts_with("╰ Enter 存檔  Esc 取消 ─"),
        "{}",
        rows[9]
    );
    // a on the top level: 「最外層」
    app.handle_key(KeyEvent::from(KeyCode::Esc));
    app.handle_key(KeyEvent::from(KeyCode::Char('a')));
    let terminal = draw_app(&mut app, 80, 24);
    let rows = area_rows(terminal.backend().buffer(), editor_area(&app));
    assert_eq!(rows[5].trim_end_matches([' ', '│']), "│  上層日曆  最外層");
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
    // In the middle of the main view (above the key hints), like the day popup; inside: the typed text, a
    // blank row, title and date, a blank row, the hint row. 80 × 24 (main view 80 × 23): 76 wide (2 cells
    // from each side), 10 tall with the blank rows above and below, top at (23 − 10) / 2 = 6
    assert_eq!(at(&mut app, 80, 24), Rect::new(2, 6, 76, 10));
    // With the side menu (122 × 30: 22 cells wide), in the middle of what is right of it: the main view is
    // 100 × 29 from x 22 → x 22 + (100 − 80) / 2 = 32, y (29 − 10) / 2 = 9; 200 × 50: at most 80 wide
    assert_eq!(at(&mut app, 122, 30), Rect::new(32, 9, 80, 10));
    assert_eq!(at(&mut app, 200, 50), Rect::new(71, 19, 80, 10));
    // Narrower than 30: the whole width, the hint row wraps onto 3 lines (27 cells inside): 12 tall;
    // too short for the blank rows above and below (80 × 6: 5 rows above the key hints): 5 tall
    assert_eq!(at(&mut app, 29, 24), Rect::new(0, 5, 29, 12));
    assert_eq!(at(&mut app, 80, 6), Rect::new(2, 0, 76, 5));
    app.screen = Rect::new(0, 0, 80, 24);
    let click = |app: &mut App, column: u16, row: u16| {
        app.handle_mouse(MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column,
            row,
            modifiers: KeyModifiers::NONE,
        });
    };
    // Inside the box (its corners and the blank rows too): stays open
    for (column, row) in [(2, 6), (77, 15), (40, 7)] {
        click(&mut app, column, row);
        assert!(app.editor().is_some(), "({column}, {row})");
    }
    // Below it (where the box is not): cancels
    click(&mut app, 40, 20);
    assert!(app.editor().is_none());
    // Narrower than 30: no blank cell left of the text (one blank in front of the typed text, which
    // scrolls so the cursor at its end shows: 26 cells for 「- [ ] Send the weekly update」 28 chars
    // and the cursor → 3 chars scrolled away); 29 × 12 is too short for the blank rows above and below
    // too, so the text is on the row below the border, as at 80 × 6
    app.handle_key(KeyEvent::from(KeyCode::Char('e')));
    let terminal = draw_app(&mut app, 29, 12);
    let rows = area_rows(terminal.backend().buffer(), at(&mut app, 29, 12));
    assert!(
        rows[1].starts_with("│  ] Send the weekly update"),
        "{}",
        rows[1]
    );
    let terminal = draw_app(&mut app, 80, 6);
    let rows = area_rows(terminal.backend().buffer(), at(&mut app, 80, 6));
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
    app.screen = Rect::new(0, 0, 80, 24);
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
    // 80 × 24: the typed text starts at x 5 (box 2 + border + padding + a blank), so the `#` after
    // 「- [ ] 」 is at x 11 and the list's left border at x 10, right under the text (y 9). 25 wide: the keys
    // on its bottom border (23) and the borders; 4 calendars + borders = 6 tall
    app.handle_key(KeyEvent::from(KeyCode::Char('a')));
    type_text(&mut app, "#");
    let terminal = draw_app(&mut app, 80, 24);
    let buffer = terminal.backend().buffer();
    let list = Rect::new(10, 9, 25, 6);
    assert_eq!(
        app.editor().and_then(|editor| hash_list_area(&app, editor)),
        Some((list, 0))
    );
    assert_eq!(
        area_rows(buffer, list),
        [
            "╭───────────────────────╮",
            "│ Holidays              │",
            "│ Work                  │",
            "│ Work > SFAP           │",
            "│ Personal              │",
            "╰ Enter pick  Esc close ╯",
        ]
    );
    // The chosen row (the first) on the selection background, across the box
    for x in 11..34 {
        assert_eq!(buffer[(x, 10)].bg, theme.selection, "x {x}");
    }
    assert_ne!(buffer[(11, 11)].bg, theme.selection);
    // `sf`: only Work > SFAP; its S and F (characters 7 and 8 of the name, after a blank at x 11) in the
    // focus color and bold, the rest in the text color
    type_text(&mut app, "sf");
    let terminal = draw_app(&mut app, 80, 24);
    let buffer = terminal.backend().buffer();
    let list = Rect::new(10, 9, 25, 3);
    assert_eq!(area_rows(buffer, list)[1], "│ Work > SFAP           │");
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
    mouse(&mut app, MouseEventKind::ScrollDown, 40, 20);
    mouse(&mut app, MouseEventKind::ScrollDown, 40, 20);
    mouse(&mut app, MouseEventKind::ScrollUp, 40, 20);
    assert_eq!(app.editor().map(|editor| editor.hash_list.index), Some(1));
    mouse(&mut app, click, 20, 9);
    assert!(app.editor().and_then(Editor::suggestions).is_some());
    mouse(&mut app, click, 40, 20);
    assert!(app.editor().is_none());
    // Editing an item: the fields name its calendar, the hint row says `#name` picks another
    app.handle_key(KeyEvent::from(KeyCode::Tab));
    app.handle_key(KeyEvent::from(KeyCode::Char('e')));
    let terminal = draw_app(&mut app, 80, 24);
    let rows = area_rows(terminal.backend().buffer(), editor_area(&app));
    assert!(rows[0].starts_with("╭ Edit item ─"), "{}", rows[0]);
    assert_eq!(rows[6].trim_end_matches([' ', '│']), "│  Calendar  Work");
    assert!(
        rows[8]
            .trim_end_matches([' ', '│'])
            .ends_with("Calendar #name"),
        "{}",
        rows[8]
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
    app.screen = Rect::new(0, 0, 80, 24);
    app.handle_key(KeyEvent::from(KeyCode::Char('a')));
    type_text(&mut app, "#");
    for _ in 0..9 {
        app.handle_key(KeyEvent::from(KeyCode::Down));
    }
    let terminal = draw_app(&mut app, 80, 24);
    let list = Rect::new(10, 9, 25, 10);
    assert_eq!(
        app.editor().and_then(|editor| hash_list_area(&app, editor)),
        Some((list, 2))
    );
    let rows = area_rows(terminal.backend().buffer(), list);
    assert_eq!(rows[1], "│ C2                    │");
    assert_eq!(rows[8], "│ C9                    │");
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
    // The editor over layout 1 (80 × 24, the sample file): the ring around it
    // is blank, the calendar and agenda show again right outside it
    let parsed = crate::tasks::parse(include_str!("../../testdata/tasks.md"));
    let mut app = App::new(Lang::En, date(2026, 10, 17), parsed.items);
    app.handle_key(KeyEvent::from(KeyCode::Tab));
    app.handle_key(KeyEvent::from(KeyCode::Char('e')));
    let terminal = draw_app(&mut app, 80, 24);
    let buffer = terminal.backend().buffer();
    app.screen = Rect::new(0, 0, 80, 24);
    let editor = editor_area(&app);
    assert!(blank(buffer, &ring(editor, 1)));
    assert!(!blank(buffer, &ring(editor, 2)));
    // The day popup over the month grid (60 × 20): the box is (9, 5, 42, 8) (see
    // `the_day_popup_lists_every_item_of_the_day_over_the_big_calendar`)
    let text = "- [ ] Buy groceries @2026-10-17 17:30\n- Farmers market @2026-10-17 10:00\n\
                    - [x] Clean the garage @2026-10-17\n- Parents in town @2026-10-16..2026-10-18\n";
    let mut app = month_app(Lang::ZhTw, date(2026, 10, 17), text);
    app.focus = Focus::Day;
    let terminal = draw_app(&mut app, 60, 20);
    let buffer = terminal.backend().buffer();
    app.screen = Rect::new(0, 0, 60, 20);
    let popup = day_area(&app);
    assert_eq!(popup, Rect::new(9, 5, 42, 8));
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
    let terminal = draw(Lang::ZhTw, 40, 26, date(2026, 10, 17));
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
    // The key hints are muted; of the layout keys `123` at the left end, the shown layout's (1) is lit up
    assert_eq!(cell(1, 25), ("1", theme.focus, true));
    assert_eq!(cell(2, 25), ("2", theme.muted, false));
    assert_eq!(cell(11, 25), ("h", theme.muted, false));
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
    assert_eq!((cell.symbol(), cell.fg), ("2", Theme::default().muted));
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
            " 123 版型  hjkl[] 移動  Tab 切換  t 今天  s 日曆  ? 說明  , 設定  q 離",
        ]
    );
    // Calendar: every day in a range is underlined (10/20-25, 10/28-11/3), the days just before and after
    // are not; colors do not change: the start day (10/20) and the days in the middle (10/21, x 19-20)
    // keep the normal color
    let buffer = terminal.backend().buffer();
    let underlined = |x: u16, y: u16| buffer[(x, y)].modifier.contains(Modifier::UNDERLINED);
    // 10/19 (Mon) x 13-14, 10/20 (Tue) x 16-17, 10/25
    // (Sun) x 31-32 are at y 6; 10/26 (Mon) and 10/28 (Wed) at y 7
    assert!(!underlined(14, 6));
    assert!(underlined(17, 6) && underlined(32, 6));
    assert!(!underlined(14, 7) && underlined(20, 7));
    let theme = Theme::default();
    assert_eq!(buffer[(17, 6)].fg, theme.text);
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
            " 123 版型  hjkl[] 移動  Tab 切換  t 今天  s 日曆  ? 說明  , 設定  q 離",
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
    let theme = Theme::default();
    assert_eq!(buffer[(40, 3)].bg, Color::Reset);
    assert_eq!(buffer[(40, 4)].bg, theme.selection);
    assert_eq!(buffer[(40, 5)].bg, theme.selection);
    assert_eq!(buffer[(40, 6)].bg, Color::Reset);
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
        let terminal = draw_app(app, 70, 23);
        let cell = &terminal.backend().buffer()[(x, y)];
        (cell.symbol().to_string(), cell.fg)
    };
    // 10/10 (Sat) x 29, 10/11 (Sun) x 32 at y 4; 10/12 (Mon) x 14 at y 5;
    // 10/31 (Sat) x 29 at y 7; 10/9 (Fri) x 26 at y 4 is not a holiday
    assert_eq!(fg(&mut app, 29, 4), ("0".into(), theme.weekend));
    assert_eq!(fg(&mut app, 32, 4), ("1".into(), theme.weekend));
    assert_eq!(fg(&mut app, 14, 5), ("2".into(), theme.weekend));
    assert_eq!(fg(&mut app, 29, 7), ("1".into(), theme.weekend));
    assert_eq!(fg(&mut app, 26, 4), ("9".into(), theme.text));
    // If today is a holiday, today's green comes first
    app.today = date(2026, 10, 10);
    assert_eq!(fg(&mut app, 29, 4), ("0".into(), theme.today));
    app.today = date(2026, 10, 17);
    // The name in the side menu leaves out type:holiday; unchecked: no longer holidays (10/12, a
    // Monday, back to normal; 10/10 a Saturday, still a weekend)
    assert_eq!(app.groups[0].name.as_deref(), Some("US holidays"));
    app.groups[0].shown = false;
    assert_eq!(fg(&mut app, 14, 5), ("2".into(), theme.text));
    assert_eq!(fg(&mut app, 29, 4), ("0".into(), theme.weekend));
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
    let terminal = draw_app(&mut app, 70, 23);
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
    // Calendar: 10/9 (Fri) x 26, 10/10 (Sat) x 29 at y 4; 10/18 (Sun) x 32 at y 5
    assert_eq!(cell(26, 4), ("9", theme.text, false));
    assert_eq!(cell(29, 4), ("0", theme.weekend, false));
    assert_eq!(cell(32, 5), ("8", theme.weekend, false));
    // Agenda (starts at x 36, one item per row): titles
    // at x 43; Go hiking has a time in front, so it is at x 49
    assert_eq!(cell(43, 3), ("S", theme.text, false));
    assert_eq!(cell(43, 4), ("F", theme.muted, false));
    assert_eq!(cell(38, 4), ("0", theme.muted, false));
    assert_eq!(cell(49, 5), ("G", theme.text, false));
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
    let terminal = draw_app(&mut app, 40, 26);
    let buffer = terminal.backend().buffer();
    let theme = Theme::default();
    let fg = |x: u16| (buffer[(x, 5)].symbol(), buffer[(x, 5)].fg);
    assert_eq!(fg(20), ("4", theme.text));
    assert_eq!(fg(23), ("5", theme.text));
    assert_eq!(fg(26), ("6", theme.text));
    assert_eq!(fg(29), ("7", theme.today));
    assert_eq!(fg(32), ("8", theme.weekend));
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
            " 123 版型  hjkl[] 移動  Tab 切換  t 今天  s 日曆  ? 說明",
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
            " 123 版型  hjkl[] 移動  Tab 切換  t 今天  s 日曆  ? 說明",
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
            "├─────┼──────────┼──────────┼──────────┼─────────┼─────────┼─────────┼─────────┤",
            "│  46 │ 9        │ 10       │ 11       │ 12      │ 13      │ 14      │ 15      │",
            "╰─────┴──────────┴──────────┴──────────┴─────────┴─────────┴─────────┴─────────╯",
            " 123 版型  hjkl[] 移動  Tab 切換  t 今天  s 日曆  ? 說明  , 設定  q 離開",
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
            "│  40 │ 28       │ 29       │ 30       │ 1 Oct   │ 2       │ 3       │ 4       │",
            "│ Oct │          │          │          │         │         │         │         │",
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
            "│  44 │ 26       │ 27       │ 28       │ 29      │ 30      │ 31      │ 1 Nov   │",
            "│ Nov │          │          │          │         │         │         │         │",
            "│     │          │          │          │         │         │         │         │",
            "├─────┼──────────┼──────────┼──────────┼─────────┼─────────┼─────────┼─────────┤",
            "│  45 │ 2        │ 3        │ 4        │ 5       │ 6       │ 7       │ 8       │",
            "│     │          │          │          │         │         │         │         │",
            "│     │          │          │          │         │         │         │         │",
            "╰─────┴──────────┴──────────┴──────────┴─────────┴─────────┴─────────┴─────────╯",
            " 123 view  hjkl[] move  Tab pane  t today  s calendars  ? help  , settings  q qu",
        ]
    );
}

/// Task file for the month grid tests (`tools/expected_screen.py`
/// reads this same file for the expected screens)
const MONTH_TASKS: &str = include_str!("../../testdata/month.md");

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
            "│  40 │ 28       │ 29       │ 30       │ 1 10月  │ 2       │ 3       │ 4       │",
            "│ 10月│          │ Backyard BBQ        │         │         │         │         │",
            "│     │          │          │          │         │         │         │         │",
            "├─────┼──────────┼──────────┼──────────┼─────────┼─────────┼─────────┼─────────┤",
            "│  41 │ 5        │ 6        │ 7        │ 8       │ 9       │ 10      │ 11      │",
            "│     │ Pay the phone bill  │          │         │         │ Farmers market    │",
            "│     │          │          │          │         │         │         │         │",
            "├─────┼──────────┼──────────┼──────────┼─────────┼─────────┼─────────┼─────────┤",
            "│  42 │ 12       │ 13       │ 14       │ 15      │ 16      │ 17      │ 18      │",
            "│     │ 14:00 Cod│ 15:00 Confirm alerti│ Fix the │ 🚧Team h│ Dinner w│ 07:00 Go│",
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
            " 123 版型  hjkl[] 移動  Tab 切換  t 今天  s 日曆  ? 說明  , 設定  q 離開",
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
            " 123 view  hjkl[] move  Tab pane  t today  s calendars  ? help  , settings  q qu",
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
            " 123 版型  hjkl[] 移動  Tab 切換  t 今天  s 日曆  ? 說明  , 設定  q 離開",
        ]
    );
}

#[test]
fn month_view_item_colors() {
    // 80 × 30 (screen: see `month_view_lists_each_days_items_in_its_cell`): cells start at x 7, 18, 29,
    // 40, 50, 60, 70; weeks 1, 3 and 4 are at y 3, 11 and 15
    let mut app = month_app(Lang::ZhTw, date(2026, 10, 17), MONTH_TASKS);
    let terminal = draw_app(&mut app, 80, 30);
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
    assert_eq!(cell(8, 12), ("1", theme.muted, false));
    assert_eq!(cell(14, 12), ("C", theme.text, false));
    // 10/15 (Thu) finished item: whole line gray, title struck through
    assert_eq!(cell(41, 12), ("F", theme.muted, true));
    // 10/17 (Sat) is today: its item is still in the normal color (only the date is green)
    assert_eq!(cell(61, 12), ("D", theme.text, false));
    // 10/16 (Fri) overflows: 「+3」 is muted
    assert_eq!(cell(52, 13), ("3", theme.muted, false));
    // 9/29 (Tue, in the top week 9/28-10/4) is in another month: not grayed out, the normal color
    assert_eq!(cell(19, 4), ("B", theme.text, false));
    assert_eq!(cell(20, 4), ("a", theme.text, false));
    // 10/20 (Tue)'s long title borrows cells up to 10/23 (Fri): text over the grid lines keeps its own
    // color (x 49 was a vertical line); the grid lines after the text keep the grid color
    assert_eq!(cell(49, 16), ("s", theme.text, false));
    assert_eq!(buffer[(59, 16)].symbol(), "│");
    assert_eq!(buffer[(59, 16)].fg, theme.grid);
    // Only 1 row per week: the dots follow the day's color (10/16, a Friday: normal); all done (10/15)
    // is muted
    let terminal = draw_app(&mut app, 80, 16);
    let buffer = terminal.backend().buffer();
    // 80 × 16: week 3 is at y 7; 10/16 is 「 16 ⠶」, 10/15 is 「 15 ⠂」
    assert_eq!(
        (buffer[(54, 7)].symbol(), buffer[(54, 7)].fg),
        ("⠶", theme.text)
    );
    assert_eq!(
        (buffer[(44, 7)].symbol(), buffer[(44, 7)].fg),
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
            "│ ☑ 未分組         7 ││  40 │ 28          │ 29         │ 30         │ 1 10月     │ 2          │ 3          │ 4          │",
            "│ ☑ Personal       8 ││ 10月│             │ Backyard BBQ            │            │            │            │            │",
            "│                    │├─────┼─────────────┼────────────┼────────────┼────────────┼────────────┼────────────┼────────────┤",
            "│                    ││  41 │ 5           │ 6          │ 7          │ 8          │ 9          │ 10         │ 11         │",
            "│                    ││     │ Pay the phone bill       │            │            │            │ Farmers market          │",
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
            "│────────────────────││  45 │ 2           │ 3          │ 4          │ 5          │ 6          │ 7          │ 8          │",
            "│ ☐ 隱藏做完的       ││     │             │            │            │            │            │            │            │",
            "╰────────────────────╯╰─────┴─────────────┴────────────┴────────────┴────────────┴────────────┴────────────┴────────────╯",
            " 123 版型  hjkl[] 移動  Tab 切換  t 今天  s 日曆  ? 說明  , 設定  q 離開",
        ]
    );
    // Personal unchecked: its items leave the cells (10/18 stays weekend blue either way)
    app.groups[1].shown = false;
    let terminal = draw_app(&mut app, 121, 22);
    assert_eq!(
        text_rows(terminal.backend().buffer()),
        [
            "╭────────────────────╮╭─────────────────────────────────────── 2026/10/17（六） ────────────────────────────────────────╮",
            "│ 日曆               ││  週   一            二           三           四           五           六           日         │",
            "│────────────────────│├─────┬─────────────┬────────────┬────────────┬────────────┬────────────┬────────────┬────────────┤",
            "│ ☑ 未分組         7 ││  40 │ 28          │ 29         │ 30         │ 1 10月     │ 2          │ 3          │ 4          │",
            "│ ☐ Personal       8 ││ 10月│             │            │            │            │            │            │            │",
            "│                    │├─────┼─────────────┼────────────┼────────────┼────────────┼────────────┼────────────┼────────────┤",
            "│                    ││  41 │ 5           │ 6          │ 7          │ 8          │ 9          │ 10         │ 11         │",
            "│                    ││     │             │            │            │            │            │            │            │",
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
            "│────────────────────││  45 │ 2           │ 3          │ 4          │ 5          │ 6          │ 7          │ 8          │",
            "│ ☐ 隱藏做完的       ││     │             │            │            │            │            │            │            │",
            "╰────────────────────╯╰─────┴─────────────┴────────────┴────────────┴────────────┴────────────┴────────────┴────────────╯",
            " 123 版型  hjkl[] 移動  Tab 切換  t 今天  s 日曆  ? 說明  , 設定  q 離開",
        ]
    );
    // 10/18 (Sun): the month grid starts at x 22, the Sunday column at x 108, week 3 at y 9 (2 rows per
    // week); the 「1」 of 「 18」 is at x 109
    let theme = Theme::default();
    let cell = &terminal.backend().buffer()[(109, 9)];
    assert_eq!((cell.symbol(), cell.fg), ("1", theme.weekend));
}

#[test]
fn month_view_without_grid_lines_gives_the_cells_more_room() {
    // Config `[month] grid_lines = false`: no grid lines, the cells touch, and no line under the header
    // either; only where a month ends, its edge (a row after the first week for September's, a row
    // above and below the week of 10/26 for October's); 80 × 24: day width 78 − 5 = 73 (Mon-Wed 11
    // cells, the others 10), each week (21 − 1 − 3) ÷ 6 = 2 rows
    let mut app = month_app(Lang::ZhTw, date(2026, 10, 17), MONTH_TASKS);
    app.settings.month.grid_lines = false;
    app.screen = Rect::new(0, 0, 80, 24);
    assert_eq!(
        text_rows(draw_app(&mut app, 80, 24).backend().buffer()),
        [
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
            " 123 版型  hjkl[] 移動  Tab 切換  t 今天  s 日曆  ? 說明  , 設定  q 離開",
        ]
    );
    // Mouse: the cells without grid lines can be clicked too. Tuesday is x 17-27, week 2 (the week of
    // 10/5) is y 5-6, below September's edge at y 4 (nothing to click there)
    assert_eq!(click_at(&app, 17, 5), Some(Click::Day(date(2026, 10, 6))));
    assert_eq!(click_at(&app, 27, 6), Some(Click::Day(date(2026, 10, 6))));
    assert_eq!(click_at(&app, 28, 6), Some(Click::Day(date(2026, 10, 7))));
    assert_eq!(click_at(&app, 20, 4), None);
}

#[test]
fn month_view_keeps_the_selected_week_in_the_middle() {
    // Config `[month] center_cursor = true`: the selected week stays on row 3 (also right after opening,
    // instead of the week with the 1st on top)
    let mut app = month_app(Lang::ZhTw, date(2026, 10, 17), "");
    app.settings.month.center_cursor = true;
    assert_eq!(
        text_rows(draw_app(&mut app, 80, 16).backend().buffer()),
        [
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
            " 123 版型  hjkl[] 移動  Tab 切換  t 今天  s 日曆  ? 說明  , 設定  q 離開",
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
            " 123 版型  hjkl[] 移動  Tab 切換  t 今天  s 日曆  ? 說明  , 設定  q 離開",
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
            " 123 版型  hjkl[] 移動  Tab 切換  t",
        ]
    );
    // Same in layout 2
    app.view = View::Month;
    assert_eq!(
        text_rows(draw_app(&mut app, 80, 16).backend().buffer()),
        [
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
            " 123 版型  hjkl[] 移動  Tab 切換  t 今天  s 日曆  ? 說明  , 設定  q 離開",
        ]
    );
}

#[test]
fn month_view_lists_a_multi_day_item_on_each_of_its_days() {
    // Business trip to NYC 10/20-25: one bar on its days' first line, across the grid lines, the title
    // written once; a one-day item on one of those days goes on the next line (10/22: Buy gifts for
    // the team); Manager OOO 10/28-11/3: its bar breaks where November starts (between 10/31 and 11/1)
    // and at the end of the week, and each part writes the title again
    let mut app = month_app(Lang::ZhTw, date(2026, 10, 17), RANGE_TASKS);
    let terminal = draw_app(&mut app, 80, 30);
    assert_eq!(
        text_rows(terminal.backend().buffer()),
        [
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
            "│     │          │          │          │ Buy gifts for the team      │         │",
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
            " 123 版型  hjkl[] 移動  Tab 切換  t 今天  s 日曆  ? 說明  , 設定  q 離開",
        ]
    );
    // Dates in a range are underlined: 10/20 (Tue) 「 20」 at x 19-20, y 15; 10/19 (Mon) at x 8-9 is not
    let buffer = terminal.backend().buffer();
    let underlined = |x: u16, y: u16| buffer[(x, y)].modifier.contains(Modifier::UNDERLINED);
    assert!(underlined(20, 15));
    assert!(!underlined(9, 15));
    // The bar (y 16) has the range background from Tuesday's left edge (x 18) to Sunday's right edge
    // (x 78), the grid lines between too (x 28, 39); not the grid line left of it, nor the next line
    let theme = Theme::default();
    for x in [18, 28, 39, 60, 78] {
        assert_eq!(buffer[(x, 16)].bg, theme.range, "x {x}");
    }
    assert_eq!(buffer[(17, 16)].symbol(), "│");
    assert_ne!(buffer[(17, 16)].bg, theme.range);
    assert_ne!(buffer[(18, 17)].bg, theme.range);
    // The title is normal text (not over yet), not the day's color
    assert_eq!(
        (buffer[(19, 16)].symbol(), buffer[(19, 16)].fg),
        ("B", theme.text)
    );
    // Manager OOO (y 20): the grid line between 10/31 and 11/1 (x 69) is not covered; November's part
    // has the background again, and its title the normal color (other months are not grayed out)
    assert_eq!(buffer[(69, 20)].symbol(), "│");
    assert_ne!(buffer[(69, 20)].bg, theme.range);
    assert_eq!(buffer[(70, 20)].bg, theme.range);
    assert_eq!(
        (buffer[(71, 20)].symbol(), buffer[(71, 20)].fg),
        ("M", theme.text)
    );
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
    app.screen = Rect::new(0, 0, 60, 22);
    assert_eq!(
        text_rows(draw_app(&mut app, 60, 22).backend().buffer()),
        [
            "╭──────────────────────────────────────────────────────────╮",
            "│ Tasks                                                    │",
            "│──────────────────────────────────────────────────────────│",
            "│ Work                                                     │",
            "│   SFAP                                                   │",
            "│   ▸ [ ] Fix the login bug  0/1               10/20       │",
            "│     Bugs                                                 │",
            "│       [ ] Session times out too early        10/21       │",
            "│   CAIS                                                   │",
            "│     [ ] Ship the monthly report              10/22       │",
            "│ Personal                                                 │",
            "│    ∙  Gym                                    10/18       │",
            "│   Moving                                                 │",
            "│     [ ] Pack the books                        11/1       │",
            "│   Wishlist                                               │",
            "│ Work                                                     │",
            "│    ∙  Team sync                              10/19 10:00 │",
            "│ Someday                                                  │",
            "│──────────────────────────────────────────────────────────│",
            "│ Line 3  - [ ] Fix the login bug @2026-10-20              │",
            "╰──────────────────────────────────────────────────────────╯",
            " 123 view  jk move  hl fold  Space check  e edit  a add  s c",
        ]
    );
    // `▸` of the first item (inside SFAP, 1 level in): x = 1 + 1 + 2 = 4 on the screen, row 5
    assert_eq!(
        click_at(&app, 4, 5),
        Some(Click::TaskRow {
            row: 2,
            on_marker: true
        })
    );
    assert_eq!(
        click_at(&app, 2, 5),
        Some(Click::TaskRow {
            row: 2,
            on_marker: false
        })
    );
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
            "│   [ ] Write the runbook, see                                       │",
            "│       https://wiki.example.com/payments/runbook                    │",
            "│    ∙  Release v2.3                                     10/27 09:30 │",
            "│ Recurring                                                          │",
            "│    ∙  Team sync                                        10/12 10:00 │",
            "│    ∙  Team sync                                        10/19 10:00 │",
            "│    ∙  Team sync                                        10/26 10:00 │",
            "│   [ ] Send the weekly update                           10/16 17:30 │",
            "│   [ ] Clean up this week's notes                       10/16       │",
            "│   [/] 🚧 Draft the release notes                       10/16       │",
            "│    ∙  Team happy hour                                  10/16 18:30 │",
            "│    ∙  Requirements meeting with PM                      10/8 14:00 │",
            "│    ∙  Engineering all-hands                             10/8 16:00 │",
            "│────────────────────────────────────────────────────────────────────│",
            "│ 第 4 行  - [x] Align with PM on the refund flow spec @2026-09-28   │",
            "╰────────────────────────────────────────────────────────────────────╯",
            " 123 版型  jk 移動  hl 收合  Space 打勾  e 編輯  a 新增  s 日曆  ? 說",
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
            " 123 view  jk move  hl fold  Space check  e edit  a add  s calendars",
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
    // At 40 wide 「Send the weekly update」 does not fit before its date: 「update」 wraps onto the next line,
    // under the title; the rows below move down one line (expected rows from tools/expected_screen.py)
    assert_eq!(rows[4], "│   [ ] Send the weekly          10/16 │");
    assert_eq!(rows[5], "│       update                         │");
    assert_eq!(rows[8], "│ ▸  ∙  Trip to Tokyo  0/1    2027/1/2 │");
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
    // Heading: Work (label color, bold)
    assert_eq!(buffer[(2, 3)].fg, theme.label);
    assert!(buffer[(2, 3)].modifier.contains(Modifier::BOLD));
    // Send the weekly update (y 4-5): not done → normal text, also on the wrapped line; date (x 33-37) in
    // its day's color (normal); the chosen item gets the highlight across both of its lines
    assert_eq!(cell(8, 4), ("S", theme.text, false));
    assert_eq!(cell(8, 5), ("u", theme.text, false));
    assert_eq!(cell(34, 4), ("0", theme.text, false));
    assert_eq!(buffer[(1, 4)].bg, theme.selection);
    assert_eq!(buffer[(37, 5)].bg, theme.selection);
    assert_eq!(buffer[(1, 6)].bg, Color::Reset);
    // Fix the login bug (y 6): done → gray, struck through; Hackathon (y 7): past event → gray, not
    // struck through
    assert_eq!(cell(8, 6), ("F", theme.muted, true));
    assert_eq!(cell(8, 7), ("H", theme.muted, false));
    // ▸ is muted
    assert_eq!(cell(2, 8), ("▸", theme.muted, false));
    // Click the Trip to Tokyo row (y 8): it gets chosen; click ▸ (x 2): it expands
    let click = |app: &mut App, column, row| {
        use ratatui::crossterm::event::{KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
        app.handle_mouse(MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column,
            row,
            modifiers: KeyModifiers::NONE,
        });
    };
    click(&mut app, 10, 8);
    assert_eq!(
        app.tasks_chosen().map(|item| item.title.as_str()),
        Some("Trip to Tokyo")
    );
    assert!(app.expanded.is_empty());
    click(&mut app, 2, 8);
    assert!(app.expanded.contains(&5));
    // Clicking a heading row: ignored
    click(&mut app, 5, 3);
    assert_eq!(
        app.tasks_chosen().map(|item| item.title.as_str()),
        Some("Trip to Tokyo")
    );
    // Clicking the wrapped line of an item (y 5): that item
    click(&mut app, 20, 5);
    assert_eq!(
        app.tasks_chosen().map(|item| item.title.as_str()),
        Some("Send the weekly update")
    );
}

#[test]
fn a_wrapped_item_is_kept_on_screen_whole() {
    // 40 × 14 (7 lines for the list): the last item takes 4 lines; choosing it scrolls so all 4 show,
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
    let terminal = draw_app(&mut app, 40, 14);
    assert_eq!(
        text_rows(terminal.backend().buffer())[3..10],
        [
            "│   [ ] c                              │",
            "│   [ ] d                              │",
            "│   [ ] e                              │",
            "│   [ ] one two three four five six    │",
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
            "│     [ ] Get quotes from movers                          11/7       │",
            "│────────────────────────────────────────────────────────────────────│",
            "│ 第 43 行：@2026-13-01：沒有這一天                                  │",
            "╰────────────────────────────────────────────────────────────────────╯",
            " 123 版型  jk 移動  hl 收合  Space 打勾  e 編輯  a 新增  s 日曆  ? 說",
        ]
    );
    let buffer = terminal.backend().buffer();
    let theme = Theme::default();
    // The header's 「1 處寫錯」 and the bottom row are both red
    assert_eq!(buffer[(60, 1)].fg, theme.error);
    assert_eq!(buffer[(2, 13)].fg, theme.error);
    // The title in the Pay property tax row (y 9) is red and underlined, so it does not look like a
    // `!!!` title (titles start at x 8); other rows are neither
    let underlined = |x: u16, y: u16| buffer[(x, y)].modifier.contains(Modifier::UNDERLINED);
    assert_eq!(buffer[(8, 9)].fg, theme.error);
    // Its last character (28 characters: x 8-35), and not the blank after it
    assert!(underlined(8, 9) && underlined(35, 9) && !underlined(36, 9));
    assert_eq!(buffer[(8, 8)].fg, theme.text);
    assert!(!underlined(8, 8));
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
            "│──────────────────────││───────────────────────────────────────────────────────────────────────────────────────────────│",
            "│ ☐ 隱藏做完的         ││ 第 37 行  - [ ] Pay the credit card bill !!! @2026-10-25                                      │",
            "╰──────────────────────╯╰───────────────────────────────────────────────────────────────────────────────────────────────╯",
            " 123 版型  jk 移動  hl 收合  Space 打勾  e 編輯  a 新增  s 日曆  ? 說明  , 設定  q 離開",
        ]
    );
}

#[test]
fn a_long_title_wraps_in_the_day_popup() {
    // 60 × 20, layout 2: the long title makes the popup as wide as the month grid (60, x 0, too wide for
    // blank cells left and right); inside the border 58 cells, the date takes 7 and one blank stays at the
    // end, so the title wraps at 50 cells (expected lines from the Python `wrap` in
    // tools/expected_screen.py), the second line under the title; 3 lines + border + a blank row above
    // and below = 7 tall → y 6, the lines at y 8-10
    let text = "- [ ] A really long title that just keeps going and going and going past the edge of the box @2026-10-17\n- [ ] Pack @2026-10-17\n";
    let mut app = month_app(Lang::En, date(2026, 10, 17), text);
    app.focus = Focus::Day;
    app.screen = Rect::new(0, 0, 60, 20);
    let terminal = draw_app(&mut app, 60, 20);
    let buffer = terminal.backend().buffer();
    assert_eq!(day_area(&app), Rect::new(0, 6, 60, 7));
    let rows: Vec<String> = area_rows(buffer, Rect::new(1, 8, 58, 3))
        .iter()
        .map(|row| row.trim_end().to_string())
        .collect();
    assert_eq!(
        rows,
        [
            "[10/17]A really long title that just keeps going and",
            "       going and going past the edge of the box",
            " 10/17 Pack",
        ]
    );
    // The chosen (first) item: both lines have the selection background from border to border
    let theme = Theme::default();
    assert_eq!(buffer[(1, 9)].bg, theme.selection);
    assert_eq!(buffer[(58, 9)].bg, theme.selection);
    assert_eq!(buffer[(1, 10)].bg, Color::Reset);
    // A click on the wrapped line is the first item; on the next line, the second
    assert_eq!(day_item_at(&app, 20, 9), Some(0));
    assert_eq!(day_item_at(&app, 20, 10), Some(1));
    // Too short (60 × 8, month grid 7 rows: 5 lines inside the border, no blank rows): 4 short items and
    // the long one last take 6 lines; choosing the long one scrolls one line so both its lines show
    let text = "- [ ] a @2026-10-17\n- [ ] b @2026-10-17\n- [ ] c @2026-10-17\n- [ ] d @2026-10-17\n- [ ] A really long title that just keeps going and going and going past the edge of the box @2026-10-17\n";
    let mut app = month_app(Lang::En, date(2026, 10, 17), text);
    app.focus = Focus::Day;
    app.day_index = 4;
    let terminal = draw_app(&mut app, 60, 8);
    let rows = text_rows(terminal.backend().buffer());
    assert!(
        rows[5].contains("going and going past the edge of the box"),
        "{rows:#?}"
    );
    assert!(rows[1].contains(" 10/17 b"), "{rows:#?}");
}

#[test]
fn the_day_popup_lists_every_item_of_the_day_over_the_big_calendar() {
    // 60 × 20, layout 2, 10/17 has 4 items: the popup is centered on the month grid (60 × 19);
    // there is room, so a blank row above and below the items and a blank cell left and right:
    // longest item 27 + border 2 + a blank cell 1 + padding 2 = 32, but the top border needs more: the
    // date 18 + 「 未完成 1 / 已完成 1 」 21 + one line cell + 2 corners = 42 wide, 4 + 2 + 2 = 8 rows tall
    // → x 9, y 5; the top border shows the date, one item per row (like layout 1's agenda), the
    // first item is chosen with its date in brackets; the bottom row switches to the popup's keys
    let text = "- [ ] Buy groceries @2026-10-17 17:30\n- Farmers market @2026-10-17 10:00\n\
                    - [x] Clean the garage @2026-10-17\n- Parents in town @2026-10-16..2026-10-18\n";
    let mut app = month_app(Lang::ZhTw, date(2026, 10, 17), text);
    app.focus = Focus::Day;
    let terminal = draw_app(&mut app, 60, 20);
    let buffer = terminal.backend().buffer();
    assert_eq!(
        area_rows(buffer, Rect::new(9, 5, 42, 8)),
        [
            "╭ 2026/10/17（六） ─ 未完成 1 / 已完成 1 ╮",
            "│                                        │",
            "│ [10/16~18]Parents in town              │",
            "│  10/17 Clean the garage                │",
            "│  10/17 10:00 Farmers market            │",
            "│  10/17 17:30 Buy groceries             │",
            "│                                        │",
            "╰────────────────────────────────────────╯",
        ]
    );
    // The key hints are 61 cells, so at 60 cells wide the last character is cut
    assert_eq!(
        text_rows(buffer)[19],
        " jk 移動  Space 打勾  e 編輯  a 新增  r 改期  d 刪除  Esc 關"
    );
    // The border has the color of where the keyboard is, the brackets the selected red; the chosen row
    // (y 7) has the selection background from border to border (x 10-49, the padding cells too), the
    // others none
    let theme = Theme::default();
    assert_eq!(buffer[(9, 5)].fg, theme.focus);
    assert_eq!(buffer[(11, 7)].fg, theme.selected);
    assert_eq!(buffer[(10, 7)].bg, theme.selection);
    assert_eq!(buffer[(49, 7)].bg, theme.selection);
    assert_eq!(buffer[(50, 7)].bg, Color::Reset);
    assert_eq!(buffer[(11, 8)].bg, Color::Reset);
    // Too short (60 × 6, month grid 5 rows): no rows of padding (the cells left and right stay), the
    // box holds 3 items, and choosing the last one scrolls down one item; a scrollbar on the right
    // border
    app.day_index = 3;
    let terminal = draw_app(&mut app, 60, 6);
    let buffer = terminal.backend().buffer();
    assert_eq!(
        area_rows(buffer, Rect::new(9, 1, 41, 3)),
        [
            "│  10/17 Clean the garage                ",
            "│  10/17 10:00 Farmers market            ",
            "│ [10/17]17:30 Buy groceries             ",
        ]
    );
    assert!((1..4).any(|y| buffer[(50, y)].symbol() == "┃"));
    // Click the top row (Clean the garage): it gets chosen, the scroll stays; clicking the same spot
    // again is still it (no task file, so checking it off writes nothing)
    app.screen = Rect::new(0, 0, 60, 6);
    let click = |app: &mut App| {
        use ratatui::crossterm::event::{KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
        app.handle_mouse(MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: 12,
            row: 1,
            modifiers: KeyModifiers::NONE,
        });
    };
    click(&mut app);
    assert_eq!(app.day_index, 1);
    let terminal = draw_app(&mut app, 60, 6);
    assert_eq!(
        area_rows(terminal.backend().buffer(), Rect::new(9, 1, 41, 1)),
        ["│ [10/17]Clean the garage                "]
    );
    click(&mut app);
    assert_eq!(app.day_index, 1);
}

#[test]
fn month_view_scrolls_a_row_at_a_time() {
    // 80 × 16: 1 line per week. Right after opening the top week is the one with 10/1 (9/28-10/4);
    // selecting 11/16 → its week is two weeks below the last one (11/2-8), so the grid scrolls down two
    // weeks and the top becomes 10/12
    let mut app = month_app(Lang::En, date(2026, 10, 17), "");
    draw_app(&mut app, 80, 16);
    assert_eq!(app.month_top, Some(date(2026, 9, 28)));
    app.selected = date(2026, 11, 16);
    assert_eq!(
        text_rows(draw_app(&mut app, 80, 16).backend().buffer()),
        [
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
            " 123 view  hjkl[] move  Tab pane  t today  s calendars  ? help  , settings  q qu",
        ]
    );
    assert_eq!(app.month_top, Some(date(2026, 10, 12)));
    // Select 9/20 (a Sunday, in the week 9/14-20, above the top): that week becomes the top; the
    // selected month becomes September, so October's days turn gray
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
            "│  40 │ 28       │ 29       │ 30       │ 1 10月  │ 2       │ 3       │ 4       │",
            "├─────┼──────────┼──────────┼──────────┼─────────┼─────────┼─────────┼─────────┤",
            "│  41 │ 5        │ 6        │ 7        │ 8       │ 9       │ 10      │ 11      │",
            "├─────┼──────────┼──────────┼──────────┼─────────┼─────────┼─────────┼─────────┤",
            "│  42 │ 12       │ 13       │ 14       │ 15      │ 16      │ 17      │ 18      │",
            "├─────┼──────────┼──────────┼──────────┼─────────┼─────────┼─────────┼─────────┤",
            "│  43 │ 19       │ 20       │ 21       │ 22      │ 23      │ 24      │ 25      │",
            "╰─────┴──────────┴──────────┴──────────┴─────────┴─────────┴─────────┴─────────╯",
            " 123 版型  hjkl[] 移動  Tab 切換  t 今天  s 日曆  ? 說明  , 設定  q 離開",
        ]
    );
    assert_eq!(app.month_top, Some(date(2026, 9, 14)));
    // Moving within the 6 weeks (10/18 is in the last one, 10/12-18): no scrolling
    app.selected = date(2026, 10, 18);
    draw_app(&mut app, 80, 16);
    assert_eq!(app.month_top, Some(date(2026, 9, 14)));
}

#[test]
fn month_view_click_positions_map_to_the_cells() {
    // 80 × 30 (screen: see `month_view_fills_the_space_with_six_equal_weeks`): Mon x 7-16, Tue x 18-27,
    // Thu x 40-48, Sat x 60-68, Sun x 70-78; weeks at y 3-5, 7-9, …, 23-25. The top week is 9/28-10/4,
    // the 6th 11/2-8; the 3 rows left over show the line (y 26) and the first row of 11/9-15 (y 27)
    let mut app = month_app(Lang::ZhTw, date(2026, 10, 17), "");
    app.screen = Rect::new(0, 0, 80, 30);
    draw_app(&mut app, 80, 30);
    // Any cell in a day's box counts as that day (not just the date on the first line); a week holding
    // two months has a day in every cell
    assert_eq!(date_at(&app, 7, 3), Some(date(2026, 9, 28)));
    assert_eq!(date_at(&app, 16, 5), Some(date(2026, 9, 28)));
    assert_eq!(date_at(&app, 40, 3), Some(date(2026, 10, 1)));
    assert_eq!(date_at(&app, 68, 25), Some(date(2026, 11, 7)));
    assert_eq!(date_at(&app, 78, 25), Some(date(2026, 11, 8)));
    // The cut-off 7th week can be clicked too
    assert_eq!(date_at(&app, 10, 27), Some(date(2026, 11, 9)));
    // No date: vertical lines, horizontal lines, the week-number column, the header, the bottom border,
    // the left border, the key hints
    for (x, y) in [
        (17, 3),
        (10, 6),
        (10, 26),
        (3, 3),
        (10, 1),
        (10, 28),
        (0, 3),
        (10, 29),
    ] {
        assert_eq!(date_at(&app, x, y), None, "({x}, {y})");
    }
    // 80 × 29: only 1 row left over, no room for the next week: that row counts as the 6th week
    app.screen = Rect::new(0, 0, 80, 29);
    draw_app(&mut app, 80, 29);
    assert_eq!(date_at(&app, 10, 26), Some(date(2026, 11, 2)));
}

#[test]
fn a_long_title_does_not_borrow_into_the_next_month() {
    // 10/31 (Sat) has a long title and 11/1 (Sun) next to it is empty: the title stops at the grid
    // line between them (x 69) instead of running into November. 80 × 30: the week of 10/26 is at y 19
    let mut app = month_app(
        Lang::En,
        date(2026, 10, 17),
        "- [ ] Return the rental car keys @2026-10-31\n",
    );
    let terminal = draw_app(&mut app, 80, 30);
    let buffer = terminal.backend().buffer();
    let text: String = (60..79).map(|x| buffer[(x, 20)].symbol()).collect();
    assert_eq!(text, " Return t│         ");
}

#[test]
fn the_cut_off_week_at_the_bottom_stays_inside_the_box() {
    // 80 × 30: the 3 rows left over show the line (y 26) and the first row of 11/9-15 (y 27). The bar
    // of 11/10-12 would go on the next row, the bottom border (y 28): it is left out, the border stays
    let mut app = month_app(
        Lang::En,
        date(2026, 10, 17),
        "- Trip @2026-11-10..2026-11-12\n",
    );
    let terminal = draw_app(&mut app, 80, 30);
    let buffer = terminal.backend().buffer();
    assert!(
        text_rows(buffer)[27].starts_with("│  46 │ 9        │ 10       │ 11  "),
        "{}",
        text_rows(buffer)[27]
    );
    let border: String = (17..50).map(|x| buffer[(x, 28)].symbol()).collect();
    assert_eq!(
        border,
        format!("┴{}┴{}┴{}┴", "─".repeat(10), "─".repeat(10), "─".repeat(9))
    );
    let theme = Theme::default();
    assert_ne!(buffer[(20, 28)].bg, theme.range);
}

#[test]
fn month_view_puts_the_week_across_the_new_year_on_one_row() {
    // 2027/1/1 (Fri) selected: the top week is 12/28-1/3, one row with days of both years; its week
    // number comes from the selected month's days (week 1 of 2027). The year's edge is a thick staircase
    // in the year color: between 12/31 and 1/1 (x 49), then under 12/28-31 (y 6)
    let mut app = month_app(Lang::En, date(2027, 1, 1), "");
    let terminal = draw_app(&mut app, 80, 30);
    assert_eq!(
        text_rows(terminal.backend().buffer()),
        [
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
            " 123 view  hjkl[] move  Tab pane  t today  s calendars  ? help  , settings  q qu",
        ]
    );
    let buffer = terminal.backend().buffer();
    let theme = Theme::default();
    let cell = |x: u16, y: u16| (buffer[(x, y)].symbol(), buffer[(x, y)].fg);
    assert_eq!(cell(49, 4), ("┃", theme.year_line));
    assert_eq!(cell(49, 2), ("┰", theme.year_line));
    assert_eq!(cell(20, 6), ("━", theme.year_line));
    assert_eq!(cell(3, 6), ("━", theme.year_line));
    assert_eq!(cell(17, 6), ("┿", theme.year_line));
    assert_eq!(cell(49, 6), ("╃", theme.year_line));
    assert_eq!(cell(55, 6), ("─", theme.grid));
    assert_eq!(cell(0, 6), ("┝", theme.focus));
    assert_eq!(cell(79, 6), ("┤", theme.focus));
    // The week with 1/1: the month and the year under its week number, in the label color
    assert_eq!(cell(2, 4), ("J", theme.label));
    assert_eq!(cell(2, 5), ("2", theme.label));
    // February starts on a Monday: the line above 2/1 (y 22) is straight, all in the border color
    assert_eq!(cell(20, 22), ("─", theme.line));
    assert_eq!(cell(75, 22), ("─", theme.line));
    assert_eq!(cell(49, 22), ("┼", theme.line));
    assert_eq!(cell(49, 24), ("│", theme.grid));
    // Selecting 12/31 in the same week: the row stays, and its week number is week 53 of 2026
    app.selected = date(2026, 12, 31);
    let row = text_rows(draw_app(&mut app, 80, 30).backend().buffer())[3].clone();
    assert!(row.starts_with("│  53 │ 28 "), "{row}");
}

#[test]
fn month_view_without_grid_lines_draws_only_the_months_edges() {
    // 80 × 24 without grid lines, 2027/1/1 selected: no grid, only where a month ends: the year's edge
    // thick (between 12/31 and 1/1 in the first column of 1/1's cell, x 49; under 12/28-31 on its own
    // row, y 5), February's straight (2/1 is a Monday; y 18); rows are 3 tall (21 − header 1 − 2 = 18)
    let mut app = month_app(Lang::ZhTw, date(2027, 1, 1), "");
    app.settings.month.grid_lines = false;
    let terminal = draw_app(&mut app, 80, 24);
    assert_eq!(
        text_rows(terminal.backend().buffer()),
        [
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
            " 123 版型  hjkl[] 移動  Tab 切換  t 今天  s 日曆  ? 說明  , 設定  q 離開",
        ]
    );
    let buffer = terminal.backend().buffer();
    let theme = Theme::default();
    let cell = |x: u16, y: u16| (buffer[(x, y)].symbol(), buffer[(x, y)].fg);
    assert_eq!(cell(49, 3), ("┃", theme.year_line));
    assert_eq!(cell(49, 5), ("┛", theme.year_line));
    assert_eq!(cell(20, 5), ("━", theme.year_line));
    assert_eq!(cell(3, 5), ("━", theme.year_line));
    assert_eq!(cell(0, 5), ("┝", theme.focus));
    assert_eq!(cell(60, 5).0, " ");
    assert_eq!(cell(79, 5).0, "│");
    assert_eq!(cell(20, 18), ("─", theme.line));
    assert_eq!(cell(0, 18), ("├", theme.focus));
    assert_eq!(cell(79, 18), ("┤", theme.focus));
    // Between other weeks there is no line: the week of 1/11 comes right after the week of 1/4
    assert_eq!(cell(20, 8).0, " ");
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
    assert_eq!(cell(50, 11).0, " ");
    assert_eq!(cell(52, 11), ("6", theme.text, true));
    assert_eq!(cell(53, 11).0, " ");
    // Its whole cell (x 50-58, y 11-13) has the cursor-line background, from the top-left to the
    // bottom-right; the grid lines around it and the neighbor cells do not
    let bg = |x: u16, y: u16| buffer[(x, y)].bg;
    for (x, y) in [(50, 11), (58, 11), (50, 13), (58, 13), (54, 12)] {
        assert_eq!(bg(x, y), theme.selection, "({x}, {y})");
    }
    for (x, y) in [(49, 11), (59, 11), (50, 10), (50, 14), (48, 11), (60, 11)] {
        assert_ne!(bg(x, y), theme.selection, "({x}, {y})");
    }
    // 10/15's long title does not borrow into the selected cell: it stops at the grid line
    assert_eq!(cell(41, 12).0, "S");
    assert_eq!(cell(50, 12).0, " ");
    // Today green, weekends blue, other days normal (10/15 has an unfinished item: no different)
    assert_eq!(cell(62, 11), ("7", theme.today, true));
    assert_eq!(cell(42, 11), ("5", theme.text, true));
    assert_eq!(cell(72, 11), ("8", theme.weekend, true));
    // Dates in other months: not grayed out (the day's color, 9/28 a Monday: normal), only not bold;
    // 10/1's month (same row) is in the label color
    assert_eq!(cell(9, 3), ("8", theme.text, false));
    assert_eq!(cell(43, 3), ("1", theme.label, false));
    // Week numbers: the selected row (week 42) bright, other rows (week 40) dim
    assert_eq!(cell(3, 11), ("4", theme.week_number, false));
    assert_eq!(cell(3, 3), ("4", theme.week_number_dim, false));
    // Header: weekdays muted, Sat and Sun in the weekend color
    assert_eq!(cell(8, 1), ("一", theme.muted, false));
    assert_eq!(cell(61, 1), ("六", theme.weekend, false));
    // Grid lines are faint within a month; the joints where they meet the outer border
    // have the border's color (keyboard in the calendar: focus color)
    assert_eq!(cell(6, 2), ("┬", theme.grid, false));
    assert_eq!(cell(6, 3), ("│", theme.grid, false));
    assert_eq!(cell(6, 10), ("┼", theme.grid, false));
    assert_eq!(cell(45, 6), ("─", theme.grid, false));
    // Where September ends, a staircase in the border color, like layout 1's month lines: the
    // vertical stretch between 9/30 (Wed) and 10/1 (Thu) at x 39, then the line under 9/28-30 and the
    // week-number column (y 6); its crossings take that color too
    assert_eq!(cell(39, 4), ("│", theme.line, false));
    assert_eq!(cell(39, 2), ("┬", theme.line, false));
    assert_eq!(cell(39, 6), ("┼", theme.line, false));
    assert_eq!(cell(6, 6), ("┼", theme.line, false));
    assert_eq!(cell(10, 6), ("─", theme.line, false));
    assert_eq!(cell(3, 6), ("─", theme.line, false));
    assert_eq!(cell(49, 6), ("┼", theme.grid, false));
    // Where October ends (11/1 is a Sunday): the line above 11/1 (y 18, x 70-78), the vertical stretch
    // between 10/31 and 11/1 (x 69, y 19-21), the line under 10/26-31 (y 22)
    assert_eq!(cell(74, 18), ("─", theme.line, false));
    assert_eq!(cell(65, 18), ("─", theme.grid, false));
    assert_eq!(cell(69, 20), ("│", theme.line, false));
    assert_eq!(cell(65, 22), ("─", theme.line, false));
    assert_eq!(cell(74, 22), ("─", theme.grid, false));
    // The rows left over under the 6th week: the line before the next week (y 26) and that week cut
    // off; the vertical lines reach the bottom border, which joins them as ┴ (in the border color)
    assert_eq!(cell(6, 26), ("┼", theme.grid, false));
    assert_eq!(cell(6, 27), ("│", theme.grid, false));
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
    let theme = Theme::default();
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
    // `popup` = look only inside layout 2's day popup (the month grid behind it shows the same titles);
    // `gap` = what is between 🚧 and the title (nothing in the month grid, where every cell counts)
    let check = |app: &mut App, view: &str, popup: bool, gap: &str| {
        // Tall enough for 10/17's 4 items in its month grid cell
        let terminal = draw_app(app, 100, 50);
        let buffer = terminal.backend().buffer();
        let area = if popup { day_area(app) } else { buffer.area };
        let text = area_rows(buffer, area).join("\n");
        assert!(
            text.contains(&format!("🚧{gap}Draft")) && text.contains(&format!("🚧{gap}Trip")),
            "{view}: {text}"
        );
        assert_eq!(text.matches('🚧').count(), 2, "{view}: {text}");
        assert_eq!(style(buffer, area, "Draft"), (true, false), "{view}");
        assert_eq!(style(buffer, area, "Trip"), (true, false), "{view}");
        assert_eq!(style(buffer, area, "Shop"), (false, false), "{view}");
        assert_eq!(style(buffer, area, "Clean"), (false, true), "{view}");
    };
    check(&mut app, "layout 1", false, " ");
    app.view = View::Month;
    check(&mut app, "layout 2", false, "");
    app.focus = Focus::Day;
    check(&mut app, "day popup", true, " ");
    app.view = View::Tasks;
    app.focus = Focus::Tasks;
    check(&mut app, "tasks page", false, " ");
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
        grid.contains("│ 🚧Team") && !grid.contains("18:30"),
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
    // Widest line: 「 10/17 Send the report」 22 + 「 CAIS」 5 + border 2 + a blank cell 1 + padding 2 = 32;
    // the top border needs more: the date 18 + 「 3 open / 0 done 」 17 + one line cell + 2 corners = 38
    let text = "- [ ] Loose end @2026-10-17\n# CAIS\n## Reports\n- [ ] Send the report @2026-10-17\n\
                # Life\n- [ ] Pay the fine !!! @2026-10-17\n# Holidays type:holiday\n- Day off @2026-10-17\n";
    let mut app = month_app(Lang::En, date(2026, 10, 17), text);
    app.screen = Rect::new(0, 0, 80, 24);
    app.focus = Focus::Day;
    let terminal = draw_app(&mut app, 80, 24);
    let buffer = terminal.backend().buffer();
    let area = day_area(&app);
    // The outermost calendar ends one blank cell before the padding; none outside a calendar or for a
    // holiday
    assert_eq!(
        area_rows(buffer, area),
        [
            "╭ Sat Oct 17, 2026 ─ 3 open / 0 done ╮",
            "│                                    │",
            "│ [10/17]Loose end                   │",
            "│  10/17 Send the report       CAIS  │",
            "│  10/17 Pay the fine          Life  │",
            "│  10/17 Day off                     │",
            "│                                    │",
            "╰────────────────────────────────────╯",
        ]
    );
    // Colored like the time: muted, or the whole line's color with a priority
    let theme = Theme::default();
    assert_eq!(buffer[(area.x + 31, area.y + 3)].symbol(), "C");
    assert_eq!(buffer[(area.x + 31, area.y + 3)].fg, theme.muted);
    assert_eq!(buffer[(area.x + 31, area.y + 4)].symbol(), "L");
    assert_eq!(buffer[(area.x + 31, area.y + 4)].fg, theme.priority_high);
}

#[test]
fn the_day_popup_writes_the_chosen_items_calendar_path_on_its_bottom_border() {
    // Box 38 wide: the top border's date 18 + 「 2 open / 0 done 」 17 + one line cell + 2 corners
    let text =
        "- [ ] Loose end @2026-10-17\n# CAIS\n## Reports\n- [ ] Send the report @2026-10-17\n";
    let mut app = month_app(Lang::En, date(2026, 10, 17), text);
    app.screen = Rect::new(0, 0, 80, 24);
    app.focus = Focus::Day;
    let bottom = |app: &mut App| {
        let terminal = draw_app(app, 80, 24);
        let buffer = terminal.backend().buffer();
        let area = day_area(app);
        let rows = area_rows(buffer, area);
        let color = buffer[(area.x + 2, area.bottom() - 1)].fg;
        (rows.last().cloned().unwrap_or_default(), color)
    };
    // The chosen item is in CAIS > Reports: its whole path on the left of the bottom border, in the color
    // of the tasks page's calendar headings
    app.day_index = 1;
    assert_eq!(
        bottom(&mut app),
        (
            "╰ CAIS > Reports ────────────────────╯".to_string(),
            Theme::default().label
        )
    );
    // The chosen item is outside any calendar: a plain border
    app.day_index = 0;
    assert_eq!(bottom(&mut app).0, "╰────────────────────────────────────╯");
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
