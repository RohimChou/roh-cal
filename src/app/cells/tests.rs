use super::super::View;
use super::super::write::tests::{app_with_task_file, clean_up};
use super::*;
use crate::app::Mode;
use crate::test_utils::{mouse, press};
use crate::texts::Lang;
use jiff::civil::date;
use ratatui::crossterm::event::{MouseButton, MouseEventKind};
use ratatui::layout::Rect;

/// 10/17 has 4 items, in the order its box lists them: "Trip to NYC" (10/16 to 10/18, started earlier),
/// "Triage bugs" (no time), "Team sync" 10:00, "Weekly update" 17:30; the trip is on 10/16 and 10/18 too;
/// 10/20 has one
const DAYS: &str = "# Work\n- [ ] Weekly update @2026-10-17 17:30\n- Team sync @2026-10-17 10:00\n\
                    - [x] Triage bugs @2026-10-17\n- Trip to NYC @2026-10-16..2026-10-18\n\
                    - [ ] Deploy to prod @2026-10-20\n";

/// The week view with `selected` selected and the keyboard on the calendar (80 × 25: no calendar list)
fn week_app(selected: Date) -> App {
    let mut app = App::new(
        Lang::En,
        date(2026, 10, 17),
        crate::tasks::parse(DAYS).items,
    );
    app.view = View::Week;
    app.selected = selected;
    app.screen = Rect::new(0, 0, 80, 25);
    app
}

/// (selected day, the chosen item's title)
fn chosen(app: &App) -> (Date, Option<String>) {
    (
        app.selected,
        app.chosen_item().map(|item| item.title.clone()),
    )
}

#[test]
fn tab_moves_into_the_selected_days_items_and_back() {
    let mut app = week_app(date(2026, 10, 17));
    press(&mut app, KeyCode::Tab);
    assert_eq!(app.focus, Focus::Cell);
    assert_eq!(
        chosen(&app),
        (date(2026, 10, 17), Some("Trip to NYC".into()))
    );
    press(&mut app, KeyCode::Tab);
    assert_eq!((app.focus, app.chosen_item()), (Focus::Calendar, None));
    // A day with nothing (10/19): the next day with items (10/20); nothing later (10/25): the last item of
    // the day before with items
    app.selected = date(2026, 10, 19);
    press(&mut app, KeyCode::Tab);
    assert_eq!(
        chosen(&app),
        (date(2026, 10, 20), Some("Deploy to prod".into()))
    );
    press(&mut app, KeyCode::Esc);
    assert_eq!(app.focus, Focus::Calendar);
    app.selected = date(2026, 10, 25);
    press(&mut app, KeyCode::Tab);
    assert_eq!(
        chosen(&app),
        (date(2026, 10, 20), Some("Deploy to prod".into()))
    );
    // Shift+Tab goes back to the calendar too
    press(&mut app, KeyCode::BackTab);
    assert_eq!(app.focus, Focus::Calendar);
    // No items at all: the keyboard stays on the calendar
    let mut app = App::new(Lang::En, date(2026, 10, 17), Vec::new());
    app.view = View::Week;
    press(&mut app, KeyCode::Tab);
    assert_eq!(app.focus, Focus::Calendar);
}

#[test]
fn j_and_k_go_through_the_items_and_on_to_the_next_day_with_items() {
    let mut app = week_app(date(2026, 10, 16));
    press(&mut app, KeyCode::Tab);
    let mut walk = vec![chosen(&app)];
    for _ in 0..7 {
        press(&mut app, KeyCode::Char('j'));
        walk.push(chosen(&app));
    }
    let item = |day, title: &str| (date(2026, 10, day), Some(title.to_string()));
    assert_eq!(
        walk,
        [
            item(16, "Trip to NYC"),
            item(17, "Trip to NYC"),
            item(17, "Triage bugs"),
            item(17, "Team sync"),
            item(17, "Weekly update"),
            // 10/18 has the trip only; 10/19 has nothing, so 10/20 comes next
            item(18, "Trip to NYC"),
            item(20, "Deploy to prod"),
            // The last item: it stays
            item(20, "Deploy to prod"),
        ]
    );
    // k: back to the previous day with items, on its last item
    press(&mut app, KeyCode::Char('k'));
    assert_eq!(chosen(&app), item(18, "Trip to NYC"));
    press(&mut app, KeyCode::Up);
    assert_eq!(chosen(&app), item(17, "Weekly update"));
    // The wheel goes one item at a time too
    app.handle_mouse(crate::test_utils::mouse_event(
        MouseEventKind::ScrollUp,
        10,
        10,
    ));
    assert_eq!(chosen(&app), item(17, "Team sync"));
    // h / l: the first item of the previous / next day with items
    press(&mut app, KeyCode::Char('l'));
    assert_eq!(chosen(&app), item(18, "Trip to NYC"));
    press(&mut app, KeyCode::Char('l'));
    assert_eq!(chosen(&app), item(20, "Deploy to prod"));
    press(&mut app, KeyCode::Char('l'));
    assert_eq!(chosen(&app), item(20, "Deploy to prod"));
    press(&mut app, KeyCode::Left);
    press(&mut app, KeyCode::Char('h'));
    assert_eq!(chosen(&app), item(17, "Trip to NYC"));
    // Esc: back to the calendar, on the day the item was on
    press(&mut app, KeyCode::Esc);
    assert_eq!(
        (app.focus, app.selected),
        (Focus::Calendar, date(2026, 10, 17))
    );
}

#[test]
fn the_keys_on_an_item_work_on_the_chosen_one() {
    // Space on an open task asks first (like the agenda); `t`, `[` go back to the calendar
    let mut app = week_app(date(2026, 10, 17));
    press(&mut app, KeyCode::Tab);
    for _ in 0..3 {
        press(&mut app, KeyCode::Char('j'));
    }
    press(&mut app, KeyCode::Char(' '));
    assert!(matches!(app.mode, Mode::ConfirmCheck(_)));
    press(&mut app, KeyCode::Esc);
    assert_eq!(
        chosen(&app),
        (date(2026, 10, 17), Some("Weekly update".into()))
    );
    press(&mut app, KeyCode::Char('t'));
    assert_eq!(app.focus, Focus::Calendar);
}

#[test]
fn deleting_or_moving_the_chosen_item_keeps_a_choice() {
    let (mut app, path) = app_with_task_file("cells_delete", DAYS);
    app.view = View::Week;
    app.focus = Focus::Calendar;
    app.selected = date(2026, 10, 17);
    press(&mut app, KeyCode::Tab);
    press(&mut app, KeyCode::Char('j'));
    // Delete "Triage bugs" (2nd): the same position, now "Team sync"
    press(&mut app, KeyCode::Char('d'));
    press(&mut app, KeyCode::Char('d'));
    assert_eq!(chosen(&app), (date(2026, 10, 17), Some("Team sync".into())));
    // Move "Team sync" two days later: chosen on its new day
    press(&mut app, KeyCode::Char('r'));
    press(&mut app, KeyCode::Char('l'));
    press(&mut app, KeyCode::Char('l'));
    press(&mut app, KeyCode::Enter);
    assert_eq!(
        (app.focus, chosen(&app)),
        (Focus::Cell, (date(2026, 10, 19), Some("Team sync".into())))
    );
    // Delete it: 10/19 has nothing left, back to the calendar
    press(&mut app, KeyCode::Char('d'));
    press(&mut app, KeyCode::Char('y'));
    assert_eq!(
        (app.focus, app.selected),
        (Focus::Calendar, date(2026, 10, 19))
    );
    clean_up(&path);
}

#[test]
fn the_choice_comes_back_after_another_layout_or_the_calendar_list() {
    // Layout 1 and back with the same day: the same item chosen
    let mut app = week_app(date(2026, 10, 17));
    press(&mut app, KeyCode::Tab);
    press(&mut app, KeyCode::Char('j'));
    press(&mut app, KeyCode::Char('1'));
    assert_eq!(app.focus, Focus::Calendar);
    press(&mut app, KeyCode::Char('4'));
    assert_eq!(
        (app.focus, chosen(&app)),
        (
            Focus::Cell,
            (date(2026, 10, 17), Some("Triage bugs".into()))
        )
    );
    // The calendar list called up (narrow) and put away: back on the item
    press(&mut app, KeyCode::Char('s'));
    assert_eq!(app.focus, Focus::Side);
    press(&mut app, KeyCode::Char('s'));
    assert_eq!(
        (app.focus, chosen(&app)),
        (
            Focus::Cell,
            (date(2026, 10, 17), Some("Triage bugs".into()))
        )
    );
}

#[test]
fn clicking_an_item_in_a_box_chooses_it_and_a_double_click_checks_it() {
    // 80 × 25, the 7 days from 10/16: 10/17 is the right box of row 1 (x 40-79, y 1-6), its 4 items on
    // y 2-5
    let mut app = week_app(date(2026, 10, 16));
    // As drawn: the 7 days from 10/16 stay put while the selected day moves among them
    app.week_top = Some(date(2026, 10, 16));
    let down = MouseEventKind::Down(MouseButton::Left);
    mouse(&mut app, down, 50, 4);
    assert_eq!(app.focus, Focus::Cell);
    assert_eq!(chosen(&app), (date(2026, 10, 17), Some("Team sync".into())));
    // The box's top border (its date): the day only, the keyboard back on the calendar
    mouse(&mut app, down, 50, 1);
    assert_eq!(
        (app.focus, app.selected),
        (Focus::Calendar, date(2026, 10, 17))
    );
    // A blank line under the items of 10/16 (one item, on y 2): its day
    mouse(&mut app, down, 5, 4);
    assert_eq!(
        (app.focus, app.selected),
        (Focus::Calendar, date(2026, 10, 16))
    );
    // A double click on an open task: asks whether to check it off
    mouse(&mut app, down, 50, 5);
    mouse(&mut app, down, 50, 5);
    assert_eq!(
        chosen(&app),
        (date(2026, 10, 17), Some("Weekly update".into()))
    );
    assert!(matches!(app.mode, Mode::ConfirmCheck(_)));
}

#[test]
fn the_month_grid_chooses_in_the_order_its_cell_shows() {
    // 10/17: coffee at 08:00, and a conference from 10/17 to 10/18 at 09:00. The week view's box lists them
    // by time (coffee first); the month grid's cell puts the conference's bar on its own line first
    let text = "- Coffee @2026-10-17 08:00\n- Conference @2026-10-17..2026-10-18 09:00\n";
    let mut app = App::new(
        Lang::En,
        date(2026, 10, 17),
        crate::tasks::parse(text).items,
    );
    app.view = View::Week;
    press(&mut app, KeyCode::Tab);
    assert_eq!(chosen(&app), (date(2026, 10, 17), Some("Coffee".into())));
    press(&mut app, KeyCode::Esc);
    press(&mut app, KeyCode::Char('2'));
    press(&mut app, KeyCode::Tab);
    assert_eq!(
        chosen(&app),
        (date(2026, 10, 17), Some("Conference".into()))
    );
    press(&mut app, KeyCode::Char('j'));
    assert_eq!(chosen(&app), (date(2026, 10, 17), Some("Coffee".into())));
    // Back from the side menu (narrow: called up over the screen): the same item
    press(&mut app, KeyCode::Char('s'));
    press(&mut app, KeyCode::Esc);
    assert_eq!(
        (app.focus, chosen(&app)),
        (Focus::Cell, (date(2026, 10, 17), Some("Coffee".into())))
    );
}

#[test]
fn clicking_an_item_in_a_month_cell_chooses_it() {
    // 80 × 31 with grid lines, 9/27 on top: 10/17 (Saturday) at x 70-78, its date on y 12, the trip's bar
    // on y 13 and 「+3」 on y 14
    let mut app = week_app(date(2026, 10, 17));
    app.view = View::Month;
    app.screen = Rect::new(0, 0, 80, 31);
    app.month_top = Some(date(2026, 9, 27));
    let down = MouseEventKind::Down(MouseButton::Left);
    mouse(&mut app, down, 72, 13);
    assert_eq!(
        (app.focus, chosen(&app)),
        (
            Focus::Cell,
            (date(2026, 10, 17), Some("Trip to NYC".into()))
        )
    );
    // 「+3」 (the chosen item shows): that day's popup opens
    mouse(&mut app, down, 72, 14);
    assert_eq!((app.focus, app.selected), (Focus::Day, date(2026, 10, 17)));
    press(&mut app, KeyCode::Esc);
    // The 4th item chosen stands where 「+3」 was: a double click there checks it (asks first)
    press(&mut app, KeyCode::Tab);
    for _ in 0..3 {
        press(&mut app, KeyCode::Char('j'));
    }
    assert_eq!(
        chosen(&app),
        (date(2026, 10, 17), Some("Weekly update".into()))
    );
    mouse(&mut app, down, 72, 14);
    mouse(&mut app, down, 72, 14);
    assert_eq!(app.focus, Focus::Cell);
    assert!(matches!(app.mode, Mode::ConfirmCheck(_)));
}

#[test]
fn clicking_a_line_saying_how_many_more_opens_the_days_popup() {
    // 80 × 21, the 7 days from 10/12: 10/17 is the right box of row 3 (x 40-79, y 11-15, 3 lines inside on
    // y 12-14) with 4 items: 2 show, and 「2 more, Enter shows all」 on y 14. A click there opens the popup
    let text = "- [ ] One @2026-10-17\n- [ ] Two @2026-10-17\n- [ ] Three @2026-10-17\n- [ ] Four @2026-10-17\n";
    let mut app = App::new(
        Lang::En,
        date(2026, 10, 17),
        crate::tasks::parse(text).items,
    );
    app.view = View::Week;
    app.screen = Rect::new(0, 0, 80, 21);
    app.selected = date(2026, 10, 12);
    app.week_top = Some(date(2026, 10, 12));
    let down = MouseEventKind::Down(MouseButton::Left);
    mouse(&mut app, down, 70, 14);
    assert_eq!((app.focus, app.selected), (Focus::Day, date(2026, 10, 17)));
    press(&mut app, KeyCode::Esc);
    // With its items chosen and scrolled to the last (item 3 on top): 「2 more above」 on y 12 opens it too
    press(&mut app, KeyCode::Tab);
    for _ in 0..3 {
        press(&mut app, KeyCode::Char('j'));
    }
    app.cell_top = Some((date(2026, 10, 17), 2));
    mouse(&mut app, down, 70, 12);
    assert_eq!((app.focus, app.selected), (Focus::Day, date(2026, 10, 17)));
}
