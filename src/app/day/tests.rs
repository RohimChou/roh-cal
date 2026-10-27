use super::super::write::tests::{app_with_task_file, clean_up};
use super::super::{Toast, View};
use super::*;
use crate::test_utils::{mouse, press, type_text};
use crate::texts::Lang;
use jiff::civil::date;
use ratatui::crossterm::event::{MouseButton, MouseEventKind};
use ratatui::layout::Rect;
use std::time::{Duration, Instant};

/// 10/17 has 4 items. Popup order: "Trip to NYC" (starts 10/16), "Triage bugs" (no time),
/// "Team sync" 10:00, "Weekly update" 17:30; 10/20 has one more
const DAY: &str = "# Work\n- [ ] Weekly update @2026-10-17 17:30\n- Team sync @2026-10-17 10:00\n\
                       - [x] Triage bugs @2026-10-17\n- Trip to NYC @2026-10-16..2026-10-18\n\
                       - [ ] Deploy to prod @2026-10-20\n";

/// An app in layout 2 with 10/17 selected and the keyboard on the calendar (no task file)
fn month_app() -> App {
    let mut app = App::new(
        Lang::ZhTw,
        date(2026, 10, 17),
        crate::tasks::parse(DAY).items,
    );
    app.view = View::Month;
    app.screen = Rect::new(0, 0, 60, 20);
    app
}

/// Title of the item chosen in the popup
fn chosen(app: &App) -> Option<String> {
    app.chosen_item().map(|item| item.title.clone())
}

#[test]
fn enter_opens_the_days_items_and_esc_or_q_closes_them() {
    let mut app = month_app();
    press(&mut app, KeyCode::Enter);
    assert_eq!((app.view, app.focus), (View::Month, Focus::Day));
    assert_eq!(chosen(&app).as_deref(), Some("Trip to NYC"));
    // j stops at the last item; k goes back one; Tab is ignored
    for _ in 0..10 {
        press(&mut app, KeyCode::Char('j'));
    }
    assert_eq!(chosen(&app).as_deref(), Some("Weekly update"));
    press(&mut app, KeyCode::Up);
    press(&mut app, KeyCode::Tab);
    assert_eq!(
        (app.focus, chosen(&app).as_deref()),
        (Focus::Day, Some("Team sync"))
    );
    // Esc closes it, back to the calendar; reopening chooses the first item; q also just closes, no quit
    press(&mut app, KeyCode::Esc);
    assert_eq!((app.view, app.focus), (View::Month, Focus::Calendar));
    press(&mut app, KeyCode::Enter);
    assert_eq!(chosen(&app).as_deref(), Some("Trip to NYC"));
    press(&mut app, KeyCode::Char('q'));
    assert_eq!(app.focus, Focus::Calendar);
    assert!(!app.should_quit);
    // A day with no items (10/19): opens too (it says 「沒有事」), nothing chosen
    app.selected = date(2026, 10, 19);
    press(&mut app, KeyCode::Enter);
    assert_eq!((app.focus, chosen(&app)), (Focus::Day, None));
}

#[test]
fn h_and_l_show_the_previous_or_next_day_in_the_popup() {
    // 10/17's popup with Team sync (the 3rd item) chosen: l = 10/18 (only Trip to NYC); h = back to 10/17,
    // the first item chosen again; the popup stays open and the calendar's selected day moves along
    let mut app = month_app();
    press(&mut app, KeyCode::Enter);
    press(&mut app, KeyCode::Char('j'));
    press(&mut app, KeyCode::Char('j'));
    assert_eq!(chosen(&app).as_deref(), Some("Team sync"));
    press(&mut app, KeyCode::Char('l'));
    assert_eq!(
        (app.selected, app.focus, chosen(&app).as_deref()),
        (date(2026, 10, 18), Focus::Day, Some("Trip to NYC"))
    );
    press(&mut app, KeyCode::Char('h'));
    assert_eq!(
        (app.selected, chosen(&app).as_deref()),
        (date(2026, 10, 17), Some("Trip to NYC"))
    );
    // → twice = 10/19, no items: still open (it says 「沒有事」), nothing chosen; once more = 10/20; ← back
    press(&mut app, KeyCode::Right);
    press(&mut app, KeyCode::Right);
    assert_eq!(
        (app.selected, app.focus, chosen(&app)),
        (date(2026, 10, 19), Focus::Day, None)
    );
    press(&mut app, KeyCode::Right);
    assert_eq!(chosen(&app).as_deref(), Some("Deploy to prod"));
    press(&mut app, KeyCode::Left);
    assert_eq!((app.selected, chosen(&app)), (date(2026, 10, 19), None));
    // The other day's first item gets chosen, not the same position: 10/17's 2nd (Pack) → 10/18's 1st (Fly
    // out), not its 2nd (Check in)
    let mut app = App::new(
        Lang::ZhTw,
        date(2026, 10, 17),
        crate::tasks::parse(
            "- [ ] Shop @2026-10-17\n- [ ] Pack @2026-10-17\n\
             - [ ] Fly out @2026-10-18\n- [ ] Check in @2026-10-18\n",
        )
        .items,
    );
    app.view = View::Month;
    press(&mut app, KeyCode::Enter);
    press(&mut app, KeyCode::Char('j'));
    assert_eq!(chosen(&app).as_deref(), Some("Pack"));
    press(&mut app, KeyCode::Char('l'));
    assert_eq!(chosen(&app).as_deref(), Some("Fly out"));
    // Layout 1's popup is the same one
    let mut app = month_app();
    app.view = View::Planner;
    press(&mut app, KeyCode::Enter);
    press(&mut app, KeyCode::Char('l'));
    assert_eq!(
        (app.view, app.selected, app.focus),
        (View::Planner, date(2026, 10, 18), Focus::Day)
    );
}

#[test]
fn t_in_the_popup_shows_today_and_keeps_it_open() {
    // From 10/20's popup (l three times from 10/17): t = back to today (10/17), the popup still open on its
    // first item; the grid puts today's week (from Mon 10/12) on row 2 as usual, the week before on top
    let mut app = month_app();
    press(&mut app, KeyCode::Enter);
    for _ in 0..3 {
        press(&mut app, KeyCode::Char('l'));
    }
    assert_eq!(chosen(&app).as_deref(), Some("Deploy to prod"));
    press(&mut app, KeyCode::Char('t'));
    assert_eq!(
        (app.selected, app.focus, chosen(&app).as_deref()),
        (date(2026, 10, 17), Focus::Day, Some("Trip to NYC"))
    );
    assert_eq!(app.month_top, Some(date(2026, 10, 5)));
    // Esc closes it as usual; t on the calendar keeps the keyboard there
    press(&mut app, KeyCode::Esc);
    press(&mut app, KeyCode::Char('t'));
    assert_eq!(app.focus, Focus::Calendar);
}

#[test]
fn only_a_double_click_opens_the_day_from_the_month_grid() {
    let mut app = month_app();
    let start = Instant::now();
    let at = |ms: u64| start + Duration::from_millis(ms);
    let (oct16, oct17, oct18) = (date(2026, 10, 16), date(2026, 10, 17), date(2026, 10, 18));
    // The selected day clicked again 0.9 s later: only selected, no popup
    app.click_calendar_day(oct17, at(0));
    app.click_calendar_day(oct17, at(900));
    assert_eq!(app.focus, Focus::Calendar);
    // Another click 0.3 s after that one: a double click, the popup opens
    app.click_calendar_day(oct17, at(1200));
    assert_eq!(
        (app.focus, chosen(&app).as_deref()),
        (Focus::Day, Some("Trip to NYC"))
    );
    // Two quick clicks on different days: the second day is selected, no popup
    press(&mut app, KeyCode::Esc);
    app.click_calendar_day(oct16, at(2000));
    app.click_calendar_day(oct17, at(2200));
    assert_eq!((app.selected, app.focus), (oct17, Focus::Calendar));
    // A day that was not selected, double-clicked: the first click selects it, the second opens it
    app.click_calendar_day(oct18, at(3000));
    app.click_calendar_day(oct18, at(3300));
    assert_eq!((app.selected, app.focus), (oct18, Focus::Day));
}

#[test]
fn the_mouse_needs_two_quick_presses_to_open_the_day() {
    // October on top of the grid; find the cell that maps to 10/17
    let mut app = month_app();
    app.month_top = Some(date(2026, 9, 28));
    let (column, row) = (0..60)
        .flat_map(|x| (0..20).map(move |y| (x, y)))
        .find(|&(x, y)| ui::click_at(&app, x, y) == Some(ui::Click::Day(date(2026, 10, 17))))
        .expect("10/17 is on screen");
    // One press on the selected day: nothing opens
    mouse(
        &mut app,
        MouseEventKind::Down(MouseButton::Left),
        column,
        row,
    );
    assert_eq!(app.focus, Focus::Calendar);
    // A second press right away: a double click
    mouse(
        &mut app,
        MouseEventKind::Down(MouseButton::Left),
        column,
        row,
    );
    assert_eq!(app.focus, Focus::Day);
}

#[test]
fn clicks_and_the_wheel_choose_in_the_popup() {
    let mut app = month_app();
    press(&mut app, KeyCode::Enter);
    let area = ui::day_area(&app);
    // Click the 3rd item (below the border, a blank row of padding and the calendar's heading `Work`):
    // chooses "Team sync"; clicks on the border, the padding and the heading are ignored
    mouse(
        &mut app,
        MouseEventKind::Down(MouseButton::Left),
        area.x + 3,
        area.y + 5,
    );
    assert_eq!(chosen(&app).as_deref(), Some("Team sync"));
    for (column, row) in [
        (area.x, area.y + 5),
        (area.x + 3, area.y + 1),
        (area.x + 3, area.y + 2),
    ] {
        mouse(
            &mut app,
            MouseEventKind::Down(MouseButton::Left),
            column,
            row,
        );
    }
    assert_eq!(
        (app.focus, chosen(&app).as_deref()),
        (Focus::Day, Some("Team sync"))
    );
    // The wheel moves down one item and stops at the last one
    mouse(&mut app, MouseEventKind::ScrollDown, 0, 0);
    mouse(&mut app, MouseEventKind::ScrollDown, 0, 0);
    assert_eq!(chosen(&app).as_deref(), Some("Weekly update"));
    // Click outside the box: closes it
    mouse(&mut app, MouseEventKind::Down(MouseButton::Left), 0, 0);
    assert_eq!(app.focus, Focus::Calendar);
}

#[test]
fn a_drag_over_titles_marks_text_until_a_key_or_the_next_press() {
    // 10/17's popup (60 × 20): a blank row, `Work` (y + 2), then one item per row, 2 cells in after a blank
    // cell of padding: Trip to NYC 「∙ 10/16~18 」 (title from x + 16), Triage bugs 「☑ 」 (x + 7), Team sync
    // 「∙ 10:00 」 (x + 13)
    let mut app = month_app();
    press(&mut app, KeyCode::Enter);
    let area = ui::day_area(&app);
    let (x, y) = (area.x, area.y);
    let spot = |item, char| TextSpot { item, char };
    let down = MouseEventKind::Down(MouseButton::Left);
    let drag = MouseEventKind::Drag(MouseButton::Left);
    // A press on Triage bugs' 「i」: the drag starts there, but nothing is marked before the mouse moves
    mouse(&mut app, down, x + 9, y + 4);
    assert_eq!(app.day_drag.map(|drag| drag.anchor), Some(spot(1, 2)));
    assert_eq!(app.day_selection(), None);
    // Dragged to Team sync's 「m」: both ends included
    mouse(&mut app, drag, x + 16, y + 5);
    assert_eq!(app.day_selection(), Some((spot(1, 2), spot(2, 3))));
    // Over the calendar's name `Work`: stays as it was
    mouse(&mut app, drag, x + 5, y + 2);
    assert_eq!(app.day_selection(), Some((spot(1, 2), spot(2, 3))));
    // Left of the box on Team sync's row: its first character; on Trip to NYC's dates: its first character,
    // now before where the drag started (that character stays in)
    mouse(&mut app, drag, 0, y + 5);
    assert_eq!(app.day_selection(), Some((spot(1, 2), spot(2, 0))));
    mouse(&mut app, drag, x + 7, y + 3);
    assert_eq!(app.day_selection(), Some((spot(0, 0), spot(1, 2))));
    // After a drag, the next press on the same item is not a double click (which would check it)
    assert_eq!(app.last_click, None);
    // The next press starts over: on the calendar's name, no text
    mouse(&mut app, down, x + 5, y + 2);
    assert_eq!(app.day_drag, None);
    // A key clears it too
    mouse(&mut app, down, x + 9, y + 4);
    mouse(&mut app, drag, x + 16, y + 5);
    assert!(app.day_selection().is_some());
    press(&mut app, KeyCode::Char('j'));
    assert_eq!(app.day_drag, None);
}

#[test]
fn letting_go_of_a_drag_copies_its_text_and_says_so_for_two_seconds() {
    // Same popup as above: from Triage bugs' 「i」 to Team sync's 「m」
    let mut app = month_app();
    press(&mut app, KeyCode::Enter);
    let area = ui::day_area(&app);
    let (x, y) = (area.x, area.y);
    let up = MouseEventKind::Up(MouseButton::Left);
    // A press and a let-go without moving: nothing to copy
    mouse(
        &mut app,
        MouseEventKind::Down(MouseButton::Left),
        x + 9,
        y + 4,
    );
    mouse(&mut app, up, x + 9, y + 4);
    assert_eq!(app.to_copy, None);
    // Dragged, then let go: the rest of Triage bugs and the start of Team sync, one item per line
    mouse(
        &mut app,
        MouseEventKind::Drag(MouseButton::Left),
        x + 16,
        y + 5,
    );
    mouse(&mut app, up, x + 16, y + 5);
    assert_eq!(app.to_copy.as_deref(), Some("iage bugs\nTeam"));
    // The main loop copies it: 14 characters (the line break counts), said at the bottom right; the text
    // stays marked
    let start = Instant::now();
    let mut copied = None;
    app.copy_out(
        |text| {
            copied = Some(text.to_string());
            Ok(())
        },
        start,
    );
    assert_eq!(copied.as_deref(), Some("iage bugs\nTeam"));
    assert_eq!(app.toast, Some((Toast::Copied(14), start)));
    assert_eq!(app.to_copy, None);
    assert!(app.day_selection().is_some());
    // Still up after 1.9 s, gone at 2 s
    app.end_finished_toast(start + Duration::from_millis(1900));
    assert!(app.toast.is_some());
    app.end_finished_toast(start + Duration::from_secs(2));
    assert_eq!(app.toast, None);
    // Copying fails (e.g. no clipboard command): says so instead
    mouse(&mut app, up, x + 16, y + 5);
    app.copy_out(|_| Err(color_eyre::eyre::eyre!("no pbcopy")), start);
    assert_eq!(app.toast, Some((Toast::CopyFailed, start)));
}

/// An app with a task file, in layout 2, 10/17 selected, the popup open
fn popup_with_file(name: &str) -> (App, std::path::PathBuf) {
    let (mut app, path) = app_with_task_file(name, DAY);
    app.view = View::Month;
    app.focus = Focus::Calendar;
    app.selected = date(2026, 10, 17);
    app.screen = Rect::new(0, 0, 60, 20);
    press(&mut app, KeyCode::Enter);
    (app, path)
}

#[test]
fn checking_and_editing_keep_the_popup_on_the_same_item() {
    let (mut app, path) = popup_with_file("day-popup-edit");
    let read = || std::fs::read_to_string(&path).expect("read");
    // Space on "Triage bugs" (2nd item): unchecks it
    press(&mut app, KeyCode::Char('j'));
    press(&mut app, KeyCode::Char(' '));
    assert!(read().contains("- [ ] Triage bugs @2026-10-17\n"));
    assert_eq!(
        (app.focus, chosen(&app).as_deref()),
        (Focus::Day, Some("Triage bugs"))
    );
    // Click the chosen item, then again 0.9 s later: only chosen, not checked. Another click 0.3 s after
    // that one is a double click: checked, and the file is back as it was
    let area = ui::day_area(&app);
    let start = Instant::now();
    let click = |app: &mut App, ms: u64| {
        app.click_day(area.x + 3, area.y + 4, start + Duration::from_millis(ms));
    };
    click(&mut app, 0);
    click(&mut app, 900);
    assert!(read().contains("- [ ] Triage bugs @2026-10-17\n"));
    click(&mut app, 1200);
    assert_eq!(read(), DAY);
    // e, add the time 18:00 (written @18:00: the day's date is left out in the editor): it moves to the
    // end, and the popup still has it chosen
    press(&mut app, KeyCode::Char('e'));
    type_text(&mut app, " @18:00");
    press(&mut app, KeyCode::Enter);
    assert!(read().contains("- [x] Triage bugs @2026-10-17 18:00\n"));
    assert_eq!(
        (app.focus, app.day_index, chosen(&app).as_deref()),
        (Focus::Day, 3, Some("Triage bugs"))
    );
    clean_up(&path);
}

#[test]
fn a_reschedule_that_does_not_happen_goes_back_to_the_same_day() {
    use super::super::write::Notice;
    let (mut app, path) = popup_with_file("day-popup-not-moved");
    let chosen_day = |app: &App| (app.selected, app.focus, chosen(app));
    let before = (
        date(2026, 10, 17),
        Focus::Day,
        Some("Triage bugs".to_string()),
    );
    // "Triage bugs": r → 3 days later → Esc cancels: back to the 10/17 popup, still chosen
    press(&mut app, KeyCode::Char('j'));
    press(&mut app, KeyCode::Char('r'));
    for _ in 0..3 {
        press(&mut app, KeyCode::Char('l'));
    }
    press(&mut app, KeyCode::Esc);
    assert_eq!(chosen_day(&app), before);
    // r again → 3 days later → the task file is gone, Enter: not moved, back to the 10/17 popup as well,
    // and the bottom row says it could not write
    press(&mut app, KeyCode::Char('r'));
    for _ in 0..3 {
        press(&mut app, KeyCode::Char('l'));
    }
    std::fs::remove_file(&path).expect("remove");
    press(&mut app, KeyCode::Enter);
    assert_eq!(chosen_day(&app), before);
    assert!(matches!(app.notice, Some(Notice::Failed(_))));
    clean_up(&path);
}

#[test]
fn layout_1_opens_the_same_popup_and_deleting_keeps_it_open() {
    // Layout 1: Enter on 10/17 opens the popup too; deleting its first item (d, y) keeps the popup open
    // with the next item chosen (not the agenda); deleting until the day is empty keeps it open too, with
    // nothing chosen
    let (mut app, path) = app_with_task_file("day-popup-layout-1", DAY);
    app.view = View::Planner;
    app.focus = Focus::Calendar;
    app.selected = date(2026, 10, 17);
    app.screen = Rect::new(0, 0, 70, 23);
    press(&mut app, KeyCode::Enter);
    assert_eq!(
        (app.focus, chosen(&app).as_deref()),
        (Focus::Day, Some("Trip to NYC"))
    );
    press(&mut app, KeyCode::Char('d'));
    press(&mut app, KeyCode::Char('y'));
    assert_eq!(
        (app.focus, chosen(&app).as_deref()),
        (Focus::Day, Some("Triage bugs"))
    );
    for _ in 0..3 {
        press(&mut app, KeyCode::Char('d'));
        press(&mut app, KeyCode::Char('y'));
    }
    assert_eq!(
        (app.view, app.focus, chosen(&app)),
        (View::Planner, Focus::Day, None)
    );
    clean_up(&path);
}

#[test]
fn deleting_moving_and_adding_from_the_popup() {
    let (mut app, path) = popup_with_file("day-popup-write");
    let read = || std::fs::read_to_string(&path).expect("read");
    // Last item ("Weekly update"): r → l → Enter: moved to 10/18; the popup switches to 10/18
    // with it chosen ("Trip to NYC", "Weekly update")
    for _ in 0..3 {
        press(&mut app, KeyCode::Char('j'));
    }
    press(&mut app, KeyCode::Char('r'));
    assert_eq!(app.focus, Focus::Calendar);
    press(&mut app, KeyCode::Char('l'));
    press(&mut app, KeyCode::Enter);
    assert!(read().contains("- [ ] Weekly update @2026-10-18 17:30\n"));
    assert_eq!(app.selected, date(2026, 10, 18));
    assert_eq!(
        (app.focus, app.day_index, chosen(&app).as_deref()),
        (Focus::Day, 1, Some("Weekly update"))
    );
    // u: "Weekly update" goes back to 10/17; 10/18 only has "Trip to NYC" left, and the choice
    // shrinks with it
    press(&mut app, KeyCode::Char('u'));
    assert_eq!(read(), DAY);
    assert_eq!(
        (app.focus, chosen(&app).as_deref()),
        (Focus::Day, Some("Trip to NYC"))
    );
    // a: prefilled with 10/18, added at the end of "Work"; the popup has the new item chosen
    press(&mut app, KeyCode::Char('a'));
    type_text(&mut app, "File expenses");
    press(&mut app, KeyCode::Enter);
    assert!(
        read().ends_with("- [ ] Deploy to prod @2026-10-20\n- [ ] File expenses @2026-10-18\n")
    );
    assert_eq!(
        (app.focus, chosen(&app).as_deref()),
        (Focus::Day, Some("File expenses"))
    );
    // d, y deletes "File expenses": chooses the last item ("Trip to NYC"); deleting "Trip to NYC" too
    // leaves the day empty: the popup stays open (it says 「沒有事」), still in layout 2
    press(&mut app, KeyCode::Char('d'));
    press(&mut app, KeyCode::Char('y'));
    assert_eq!(
        (app.focus, chosen(&app).as_deref()),
        (Focus::Day, Some("Trip to NYC"))
    );
    press(&mut app, KeyCode::Char('d'));
    press(&mut app, KeyCode::Char('y'));
    assert!(!read().contains("Trip to NYC"));
    assert_eq!((app.view, app.focus), (View::Month, Focus::Day));
    clean_up(&path);
}

#[test]
fn c_and_a_click_on_the_bottom_border_switch_grouping_by_calendar() {
    // By time the calendars take turns (Work 09:00, Life 12:00, Work 17:00); grouped, Work's two come first
    let text = "# Work\n- [ ] Standup @2026-10-17 09:00\n- [ ] Deploy @2026-10-17 17:00\n\
                # Life\n- [ ] Gym @2026-10-17 12:00\n";
    let mut app = App::new(
        Lang::ZhTw,
        date(2026, 10, 17),
        crate::tasks::parse(text).items,
    );
    app.view = View::Month;
    app.screen = Rect::new(0, 0, 60, 20);
    let dir = crate::files::test_dir("day-popup-by-calendar");
    let path = dir.join("config.toml");
    app.config_path = Some(path.clone());
    press(&mut app, KeyCode::Enter);
    press(&mut app, KeyCode::Char('j'));
    assert_eq!(
        (app.day_index, chosen(&app).as_deref()),
        (1, Some("Deploy"))
    );
    // c: one list by time (Standup, Gym, Deploy), Deploy stays chosen; written to the config file right
    // away
    press(&mut app, KeyCode::Char('c'));
    assert!(!app.settings.day_popup.by_calendar);
    assert_eq!(
        (app.day_index, chosen(&app).as_deref()),
        (2, Some("Deploy"))
    );
    assert_eq!(
        std::fs::read_to_string(&path).expect("read"),
        "[day_popup]\nby_calendar = false\n"
    );
    // A click on the switch flips it back; the popup stays open
    let switch = ui::day_switch_area(&app);
    mouse(
        &mut app,
        MouseEventKind::Down(MouseButton::Left),
        switch.x + 1,
        switch.y,
    );
    assert!(app.settings.day_popup.by_calendar);
    assert_eq!(
        (app.focus, app.day_index, chosen(&app).as_deref()),
        (Focus::Day, 1, Some("Deploy"))
    );
    assert!(
        std::fs::read_to_string(&path)
            .expect("read")
            .contains("by_calendar = true")
    );
    app.config_path = None;
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn subtasks_sit_under_their_parent_and_a_parent_not_on_the_day_comes_with_them() {
    // 10/17 by time: the in-progress 「Compare prices」 first, then no time 「Check the hotel」 and 「Fix the
    // build」, then 08:00 Pay rent, 09:00 Send the report, 10:00 Team sync, 11:00 Book flights, 12:00 Add a
    // test. 「Plan the trip」 has no date and 「Write the report」 is on 10/16: both only come as parents
    // (in parentheses below). A family takes the place of its first item (Plan the trip's: Compare prices)
    let text = "# Work\n- [ ] Plan the trip\n  - [ ] Book flights @2026-10-17 11:00\n\
                \x20 - [/] Compare prices @2026-10-17 15:00\n    - [ ] Check the hotel @2026-10-17\n\
                - [ ] Team sync @2026-10-17 10:00\n- [x] Write the report @2026-10-16\n\
                \x20 - [ ] Send the report @2026-10-17 09:00\n- [x] Fix the build @2026-10-17\n\
                \x20 - [ ] Add a test @2026-10-17 12:00\n# Life\n- [ ] Pay rent @2026-10-17 08:00\n";
    let mut app = App::new(
        Lang::En,
        date(2026, 10, 17),
        crate::tasks::parse(text).items,
    );
    app.view = View::Month;
    app.screen = Rect::new(0, 0, 60, 20);
    let rows = |app: &App| -> Vec<String> {
        app.day_entries()
            .iter()
            .map(|entry| match entry.on_day {
                true => entry.item.title.clone(),
                false => format!("({})", entry.item.title),
            })
            .collect()
    };
    // Grouped by calendar (the default): Work's families, then Life's
    assert_eq!(
        rows(&app),
        [
            "(Plan the trip)",
            "Compare prices",
            "Check the hotel",
            "Book flights",
            "Fix the build",
            "Add a test",
            "(Write the report)",
            "Send the report",
            "Team sync",
            "Pay rent",
        ]
    );
    // Not grouped: Pay rent (08:00) goes back between the families by time
    app.settings.day_popup.by_calendar = false;
    assert_eq!(
        rows(&app),
        [
            "(Plan the trip)",
            "Compare prices",
            "Check the hotel",
            "Book flights",
            "Fix the build",
            "Add a test",
            "Pay rent",
            "(Write the report)",
            "Send the report",
            "Team sync",
        ]
    );
    // 「Hide done」 hides 「Fix the build」 itself: it stays over 「Add a test」 as a parent, the family now in
    // that one's place (12:00, last in Work)
    app.settings.day_popup.by_calendar = true;
    app.settings.calendar_list.hide_done = true;
    assert_eq!(
        rows(&app),
        [
            "(Plan the trip)",
            "Compare prices",
            "Check the hotel",
            "Book flights",
            "(Write the report)",
            "Send the report",
            "Team sync",
            "(Fix the build)",
            "Add a test",
            "Pay rent",
        ]
    );
    // The keys skip the parents: 4 × j from Compare prices is Team sync, 5 × j Add a test
    press(&mut app, KeyCode::Enter);
    assert_eq!(chosen(&app).as_deref(), Some("Compare prices"));
    for _ in 0..4 {
        press(&mut app, KeyCode::Char('j'));
    }
    assert_eq!(chosen(&app).as_deref(), Some("Team sync"));
    press(&mut app, KeyCode::Char('j'));
    assert_eq!(chosen(&app).as_deref(), Some("Add a test"));
}
