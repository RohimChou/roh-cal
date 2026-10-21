use super::super::write::Notice;
use super::super::write::tests::{app_with_task_file, clean_up};
use super::*;
use crate::test_utils::press;
use jiff::civil::date;
use ratatui::crossterm::event::{MouseButton, MouseEvent, MouseEventKind};

#[test]
fn r_then_hjkl_and_enter_moves_the_chosen_item() {
    let text = "- [ ] Weekly update @2026-10-16 17:30\n- [ ] Coffee chat @2026-10-17 08:00\n";
    let (mut app, path) = app_with_task_file("move", text);
    // Choose "Weekly update" and press r: the keyboard moves to the calendar, and the hint says
    // which item is moving
    press(&mut app, KeyCode::Char('r'));
    assert_eq!(app.focus, Focus::Calendar);
    assert_eq!(
        app.moving().map(|m| m.item.title.as_str()),
        Some("Weekly update")
    );
    // One week and one day later (10/24), Enter: only the date changes, the time stays;
    // back in the agenda with the moved item chosen
    press(&mut app, KeyCode::Char('j'));
    press(&mut app, KeyCode::Char('l'));
    press(&mut app, KeyCode::Enter);
    assert_eq!(
        std::fs::read_to_string(&path).expect("read"),
        "- [ ] Weekly update @2026-10-24 17:30\n- [ ] Coffee chat @2026-10-17 08:00\n"
    );
    assert!(app.moving().is_none());
    assert_eq!(app.focus, Focus::Agenda);
    assert_eq!(
        app.chosen()
            .map(|entry| (entry.item.title.as_str(), entry.date)),
        Some(("Weekly update", date(2026, 10, 24)))
    );
    // u undoes it
    press(&mut app, KeyCode::Char('u'));
    assert_eq!(std::fs::read_to_string(&path).expect("read"), text);
    clean_up(&path);
}

#[test]
fn esc_cancels_the_move() {
    let text = "- [ ] Weekly update @2026-10-16 17:30\n";
    let (mut app, path) = app_with_task_file("move-cancel", text);
    press(&mut app, KeyCode::Char('r'));
    press(&mut app, KeyCode::Char(']'));
    assert_eq!(app.selected, date(2026, 11, 1));
    // Esc: the file is untouched, the selected day goes back to 10/16, back to the agenda
    press(&mut app, KeyCode::Esc);
    assert_eq!(std::fs::read_to_string(&path).expect("read"), text);
    assert_eq!(app.selected, date(2026, 10, 16));
    assert!(app.moving().is_none());
    assert_eq!(app.focus, Focus::Agenda);
    // Enter without changing the date: no write, no undo step
    press(&mut app, KeyCode::Char('r'));
    press(&mut app, KeyCode::Enter);
    press(&mut app, KeyCode::Char('u'));
    assert_eq!(app.notice, Some(Notice::NothingToUndo));
    clean_up(&path);
}

#[test]
fn other_keys_do_nothing_while_moving() {
    let text = "- [ ] Weekly update @2026-10-16\n";
    let (mut app, path) = app_with_task_file("move-keys", text);
    press(&mut app, KeyCode::Char('r'));
    // q, Tab, Space, u are all ignored (no quitting, no pane switch, no checking)
    for code in ['q', ' ', 'u', 's', '3', 'a', 'e']
        .map(KeyCode::Char)
        .into_iter()
        .chain([KeyCode::Tab])
    {
        press(&mut app, code);
        assert!(app.moving().is_some(), "{code:?}");
        assert!(app.editor().is_none(), "{code:?}");
    }
    assert!(!app.should_quit);
    assert_eq!(app.view, View::Planner);
    assert_eq!(std::fs::read_to_string(&path).expect("read"), text);
    clean_up(&path);
}

#[test]
fn clicking_a_day_picks_it_and_clicking_it_again_moves_there() {
    // 70 × 23, October at the top: 10/20 (Tue) is at x 15-17, y 6
    let text = "- [ ] Weekly update @2026-10-16\n";
    let (mut app, path) = app_with_task_file("move-click", text);
    app.screen = ratatui::layout::Rect::new(0, 0, 70, 23);
    app.calendar_top = Some(0);
    press(&mut app, KeyCode::Char('r'));
    let click = MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: 16,
        row: 6,
        modifiers: KeyModifiers::NONE,
    };
    app.handle_mouse(click);
    assert_eq!(app.selected, date(2026, 10, 20));
    assert!(app.moving().is_some());
    app.handle_mouse(click);
    assert_eq!(
        std::fs::read_to_string(&path).expect("read"),
        "- [ ] Weekly update @2026-10-20\n"
    );
    assert!(app.moving().is_none());
    clean_up(&path);
}

#[test]
fn moving_a_multi_day_item_keeps_its_length() {
    // "Manager OOO" 10/28-11/3: press r on the November row (11/2 selected), one week later → the whole
    // span moves a week later, same number of days
    let text = "- Manager OOO @2026-10-28..2026-11-03\n";
    let (mut app, path) = app_with_task_file("move-range", text);
    app.agenda_index = 1;
    app.selected = date(2026, 11, 2);
    press(&mut app, KeyCode::Char('r'));
    press(&mut app, KeyCode::Char('j'));
    press(&mut app, KeyCode::Enter);
    assert_eq!(
        std::fs::read_to_string(&path).expect("read"),
        "- Manager OOO @2026-11-04..2026-11-10\n"
    );
    // Back in the agenda: the November row of the moved item is chosen (11/9 is also one of its days)
    assert_eq!(app.selected, date(2026, 11, 9));
    assert_eq!(
        app.chosen().map(|entry| entry.date),
        Some(date(2026, 11, 4))
    );
    clean_up(&path);
}
