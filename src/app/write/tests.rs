use super::*;
use crate::test_utils::press;
use crate::texts::Lang;
use jiff::civil::date;
use ratatui::crossterm::event::{
    KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
};
use std::time::{Duration, Instant};

/// A task file in a temp folder (content `text`) and an app that reads
/// it (today 2026/10/17, keyboard in the agenda, first item chosen);
/// returns the app and the file's path. Never touches the real task file;
/// delete `path`'s folder when done
pub(crate) fn app_with_task_file(name: &str, text: &str) -> (App, PathBuf) {
    let dir = files::test_dir(name);
    std::fs::create_dir_all(&dir).expect("dir");
    let path = dir.join("tasks.md");
    std::fs::write(&path, text).expect("write");
    let mut app = App::new(Lang::ZhTw, date(2026, 10, 17), Vec::new());
    app.show_tasks(text.to_string());
    app.tasks_path = Some(path.clone());
    // Choose the first item, like pressing Tab: the keyboard moves
    // to the agenda, and the calendar selects that item's day
    app.choose(0);
    (app, path)
}

/// Deletes the test's temp folder
pub(crate) fn clean_up(path: &Path) {
    std::fs::remove_dir_all(path.parent().expect("dir")).expect("clean up");
}

#[test]
fn space_or_x_checks_the_chosen_item_off_in_the_file() {
    let text = "# Recurring\n- [ ] Review PRs @2026-10-17 09:00\n- [ ] Deploy to staging @2026-10-17 14:00\n";
    let (mut app, path) = app_with_task_file("toggle", text);
    let read = || std::fs::read_to_string(&path).expect("read");
    // Choose the second item ("Deploy to staging") and press Space: only that line changes
    press(&mut app, KeyCode::Char('j'));
    press(&mut app, KeyCode::Char(' '));
    assert_eq!(
        read(),
        "# Recurring\n- [ ] Review PRs @2026-10-17 09:00\n- [x] Deploy to staging @2026-10-17 14:00\n"
    );
    // The items on screen update too; the same item is still chosen, no notice
    assert!(app.items[1].done);
    assert_eq!(app.agenda_index, 1);
    assert_eq!(app.notice, None);
    // x works too: press it again to uncheck
    press(&mut app, KeyCode::Char('x'));
    assert_eq!(read(), text);
    assert!(!app.items[1].done);
    clean_up(&path);
}

#[test]
fn an_event_cannot_be_checked_off() {
    let text = "- Dentist appointment @2026-10-17 10:30\n";
    let (mut app, path) = app_with_task_file("event", text);
    press(&mut app, KeyCode::Char(' '));
    assert_eq!(std::fs::read_to_string(&path).expect("read"), text);
    assert_eq!(app.notice, Some(Notice::NotATask));
    // Next key: the notice disappears
    press(&mut app, KeyCode::Char('j'));
    assert_eq!(app.notice, None);
    clean_up(&path);
}

#[test]
fn toggle_reloads_instead_of_writing_when_the_file_changed_elsewhere() {
    let (mut app, path) = app_with_task_file("changed", "- [ ] Review PRs @2026-10-17 09:00\n");
    // A line was added above, elsewhere (e.g. Obsidian)
    let edited = "- [ ] Go to the gym @2026-10-17 08:00\n- [ ] Review PRs @2026-10-17 09:00\n";
    std::fs::write(&path, edited).expect("edit");
    press(&mut app, KeyCode::Char(' '));
    // File untouched, reloaded (now two items), notice shown
    assert_eq!(std::fs::read_to_string(&path).expect("read"), edited);
    assert_eq!(app.items.len(), 2);
    assert_eq!(app.notice, Some(Notice::Reloaded));
    clean_up(&path);
}

#[test]
fn toggle_says_why_when_the_file_is_gone() {
    let (mut app, path) = app_with_task_file("gone", "- [ ] Review PRs @2026-10-17 09:00\n");
    std::fs::remove_file(&path).expect("remove");
    press(&mut app, KeyCode::Char(' '));
    assert_eq!(app.notice, Some(Notice::Failed(ErrorKind::NotFound)));
    // No new file was quietly created
    assert!(!path.exists());
    clean_up(&path);
}

#[test]
fn toggle_without_a_task_file_does_nothing() {
    let mut app = App::new(
        Lang::ZhTw,
        date(2026, 10, 17),
        tasks::parse("- [ ] Review PRs @2026-10-17\n").items,
    );
    app.focus = Focus::Agenda;
    press(&mut app, KeyCode::Char(' '));
    assert!(!app.items[0].done);
    assert_eq!(app.notice, None);
}

#[test]
fn a_notice_from_a_click_stays_after_the_button_is_released() {
    // 77 × 23, October at the top: the agenda's first row (y 4, from x 43) is the event on 10/10;
    // a double click = trying to check it off, and events cannot be checked off
    let text = "- Team sync @2026-10-10\n";
    let (mut app, path) = app_with_task_file("click-notice", text);
    app.focus = Focus::Calendar;
    app.screen = ratatui::layout::Rect::new(0, 0, 77, 23);
    app.calendar_top = Some(0);
    let mouse = |kind, column| MouseEvent {
        kind,
        column,
        row: 4,
        modifiers: KeyModifiers::NONE,
    };
    let down = MouseEventKind::Down(MouseButton::Left);
    let up = MouseEventKind::Up(MouseButton::Left);
    for _ in 0..2 {
        app.handle_mouse(mouse(down, 47));
        app.handle_mouse(mouse(up, 47));
    }
    // The release does not clear the notice; neither does moving or dragging
    assert_eq!(app.notice, Some(Notice::NotATask));
    app.handle_mouse(mouse(MouseEventKind::Moved, 57));
    app.handle_mouse(mouse(MouseEventKind::Drag(MouseButton::Left), 57));
    assert_eq!(app.notice, Some(Notice::NotATask));
    // The next press: it has been seen, so clear it
    app.handle_mouse(mouse(down, 5));
    assert_eq!(app.notice, None);
    clean_up(&path);
}

#[test]
fn only_a_double_click_checks_an_item_off() {
    // 77 × 23, October at the top: the agenda's first row (y 4, from x 43) is 10/10's "Review PRs"
    let text = "- [ ] Review PRs @2026-10-10\n";
    let (mut app, path) = app_with_task_file("click", text);
    app.focus = Focus::Calendar;
    app.screen = ratatui::layout::Rect::new(0, 0, 77, 23);
    app.calendar_top = Some(0);
    let click = MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: 47,
        row: 4,
        modifiers: KeyModifiers::NONE,
    };
    let read = || std::fs::read_to_string(&path).expect("read");
    // First click: choose it (the keyboard moves to the agenda), not checked off yet
    app.handle_mouse(click);
    assert_eq!(app.focus, Focus::Agenda);
    assert_eq!(read(), text);
    // Clicked again 0.9 s later: still only chosen
    let later = Instant::now() + Duration::from_millis(900);
    app.click_item(0, later);
    assert_eq!(read(), text);
    // Another click 0.3 s after that one: a double click, checked off
    app.click_item(0, later + Duration::from_millis(300));
    assert_eq!(read(), "- [x] Review PRs @2026-10-10\n");
    clean_up(&path);
}

#[test]
fn undoing_an_add_keeps_the_agenda_choice_on_an_item() {
    // Add an item in the agenda (the new one, the 2nd item, is chosen),
    // then u: the agenda has only 1 item left, so the last one is chosen
    // (not the 2nd item that no longer exists, so checking off still works)
    let text = "- [ ] Review PRs @2026-10-10\n";
    let (mut app, path) = app_with_task_file("undo-add", text);
    press(&mut app, KeyCode::Char('a'));
    for c in "Buy groceries".chars() {
        press(&mut app, KeyCode::Char(c));
    }
    press(&mut app, KeyCode::Enter);
    assert_eq!(app.agenda_index, 1);
    press(&mut app, KeyCode::Char('u'));
    assert_eq!(std::fs::read_to_string(&path).expect("read"), text);
    assert_eq!(
        (
            app.focus,
            app.chosen().map(|entry| entry.item.title.as_str())
        ),
        (Focus::Agenda, Some("Review PRs"))
    );
    // Delete the only item too: the agenda is empty, back to the calendar
    press(&mut app, KeyCode::Char('d'));
    press(&mut app, KeyCode::Char('y'));
    assert_eq!(app.focus, Focus::Calendar);
    clean_up(&path);
}

#[test]
fn u_undoes_the_last_writes_one_at_a_time_then_has_nothing_left() {
    let text = "- [ ] Review PRs @2026-10-17 09:00\n- [ ] Deploy to staging @2026-10-17 14:00\n";
    let (mut app, path) = app_with_task_file("undo", text);
    let read = || std::fs::read_to_string(&path).expect("read");
    // Check off both items, then go back to the calendar
    for code in [' ', 'j', ' '].map(KeyCode::Char) {
        press(&mut app, code);
    }
    press(&mut app, KeyCode::Esc);
    assert_eq!(
        read(),
        "- [x] Review PRs @2026-10-17 09:00\n- [x] Deploy to staging @2026-10-17 14:00\n"
    );
    // Press u on the calendar: first undoes the last write ("Deploy to staging"), then "Review PRs";
    // the screen follows
    press(&mut app, KeyCode::Char('u'));
    assert_eq!(
        read(),
        "- [x] Review PRs @2026-10-17 09:00\n- [ ] Deploy to staging @2026-10-17 14:00\n"
    );
    assert_eq!(app.notice, Some(Notice::Undone("Deploy to staging".into())));
    press(&mut app, KeyCode::Char('u'));
    assert_eq!(read(), text);
    assert!(!app.items[0].done);
    press(&mut app, KeyCode::Char('u'));
    assert_eq!(app.notice, Some(Notice::NothingToUndo));
    clean_up(&path);
}

#[test]
fn u_leaves_the_file_alone_when_it_changed_elsewhere() {
    let (mut app, path) =
        app_with_task_file("undo-changed", "- [ ] Review PRs @2026-10-17 09:00\n");
    // Check off and uncheck (two steps recorded), then the file is changed again elsewhere
    press(&mut app, KeyCode::Char(' '));
    press(&mut app, KeyCode::Char(' '));
    let edited = "- [ ] Review PRs @2026-10-17 09:00\n- [ ] Book a haircut @2026-10-17 10:00\n";
    std::fs::write(&path, edited).expect("edit");
    press(&mut app, KeyCode::Char('u'));
    // The other changes stay, it only reloads; the two recorded steps no longer match, so all are cleared
    assert_eq!(std::fs::read_to_string(&path).expect("read"), edited);
    assert_eq!(app.items.len(), 2);
    assert_eq!(app.notice, Some(Notice::Reloaded));
    press(&mut app, KeyCode::Char('u'));
    assert_eq!(app.notice, Some(Notice::NothingToUndo));
    clean_up(&path);
}

#[test]
fn undo_keeps_only_the_last_50_steps() {
    let (mut app, path) = app_with_task_file("undo-limit", "- [ ] Review PRs @2026-10-17 09:00\n");
    // Check off / uncheck 51 times (odd count → it ends checked off)
    for _ in 0..=UNDO_LEVELS {
        press(&mut app, KeyCode::Char('x'));
    }
    // Keep pressing u: only 50 undos work (pressing more does not go past that)
    let undone = (0..UNDO_LEVELS + 5)
        .filter(|_| {
            press(&mut app, KeyCode::Char('u'));
            matches!(app.notice, Some(Notice::Undone(_)))
        })
        .count();
    assert_eq!(undone, UNDO_LEVELS);
    // The earliest step was dropped: the file stays as it was after the first check-off
    assert_eq!(
        std::fs::read_to_string(&path).expect("read"),
        "- [x] Review PRs @2026-10-17 09:00\n"
    );
    clean_up(&path);
}

/// 10/17's "Review PRs" has one subtask (no date, not in the agenda), "Deploy to staging" has none
/// (for the delete tests)
const WITH_SUBTASK: &str = "- [ ] Review PRs @2026-10-17 09:00\n  - [ ] Amy's PR\n- [ ] Deploy to staging @2026-10-17 14:00\n";

#[test]
fn d_in_the_calendar_list_asks_then_deletes_the_calendar_and_u_brings_it_back() {
    let text = "# Work\n- [ ] Review PRs @2026-10-10\n  - [ ] Read the diff\n\n# Personal\n- Gym @2026-10-12\n";
    let (mut app, path) = app_with_task_file("delete-calendar", text);
    let read = || std::fs::read_to_string(&path).expect("read");
    app.focus = Focus::Side;
    // d: asks first (Work holds 2 items), file not touched yet; any other key, a click or the wheel
    // cancels
    press(&mut app, KeyCode::Char('d'));
    assert_eq!(app.confirm_delete_calendar(), Some(("Work", 2)));
    press(&mut app, KeyCode::Char('n'));
    assert_eq!(app.confirm_delete_calendar(), None);
    press(&mut app, KeyCode::Char('d'));
    app.handle_mouse(MouseEvent {
        kind: MouseEventKind::ScrollDown,
        column: 0,
        row: 0,
        modifiers: KeyModifiers::NONE,
    });
    assert_eq!(app.confirm_delete_calendar(), None);
    assert_eq!(read(), text);
    // y: the heading and everything under it are gone; Personal is left, chosen
    press(&mut app, KeyCode::Char('d'));
    press(&mut app, KeyCode::Char('y'));
    assert_eq!(read(), "# Personal\n- Gym @2026-10-12\n");
    let names: Vec<Option<&str>> = app
        .groups
        .iter()
        .map(|group| group.name.as_deref())
        .collect();
    assert_eq!(names, [Some("Personal")]);
    assert_eq!(
        (app.focus, app.side_row),
        (Focus::Side, SideRow::Calendar(0))
    );
    // u: it all comes back
    press(&mut app, KeyCode::Char('u'));
    assert_eq!(read(), text);
    assert_eq!(app.notice, Some(Notice::Undone("Work".into())));
    assert_eq!(app.groups.len(), 2);
    clean_up(&path);
    // Ungrouped items (before any heading) have no heading to delete: d does nothing
    let mut app = App::new(Lang::ZhTw, date(2026, 10, 17), Vec::new());
    app.set_items(tasks::parse("- loose\n# Work\n- a\n"));
    app.focus = Focus::Side;
    press(&mut app, KeyCode::Char('d'));
    assert_eq!(app.confirm_delete_calendar(), None);
}

#[test]
fn z_archives_the_chosen_calendar_and_brings_it_back() {
    let text = "# Work\n- [ ] a @2026-10-10\n## Q3\n- [ ] b @2026-10-11\n# Old archived:2026-09-30\n- c @2026-10-12\n\
                    ## Notes\n- d @2026-10-13\n# Personal\n- e @2026-10-14\n";
    let (mut app, path) = app_with_task_file("archive-calendar", text);
    let read = || std::fs::read_to_string(&path).expect("read");
    let list = |app: &App| -> Vec<(String, bool, bool)> {
        app.groups
            .iter()
            .map(|group| (group.path().join(" > "), group.archived, group.shown))
            .collect()
    };
    let state = |path: &str, archived, shown| (path.to_string(), archived, shown);
    app.focus = Focus::Side;
    // z on Q3 (list: Work, Work > Q3, Personal, then the archived Old, Old > Notes): its heading gets
    // today (10/17); it goes to the archive part, unchecked, and stays chosen there
    app.side_row = SideRow::Calendar(1);
    press(&mut app, KeyCode::Char('z'));
    assert_eq!(
        read(),
        text.replace("## Q3\n", "## Q3 archived:2026-10-17\n")
    );
    assert_eq!(
        list(&app),
        [
            state("Work", false, true),
            state("Personal", false, true),
            state("Work > Q3", true, false),
            state("Old", true, false),
            state("Old > Notes", true, false),
        ]
    );
    assert_eq!(app.side_row, SideRow::Calendar(2));
    // u: as it was, Q3 back in its place and checked
    press(&mut app, KeyCode::Char('u'));
    assert_eq!(read(), text);
    assert_eq!(app.notice, Some(Notice::Undone("Q3".into())));
    assert_eq!(list(&app)[1], state("Work > Q3", false, true));
    assert_eq!(app.side_row, SideRow::Calendar(1));
    // z on Notes, archived only because Old is: Old's tag comes out, so both come back (checked); Notes
    // stays chosen
    app.side_row = SideRow::Calendar(4);
    press(&mut app, KeyCode::Char('z'));
    assert_eq!(
        read(),
        text.replace("# Old archived:2026-09-30\n", "# Old\n")
    );
    assert_eq!(
        list(&app),
        [
            state("Work", false, true),
            state("Work > Q3", false, true),
            state("Old", false, true),
            state("Old > Notes", false, true),
            state("Personal", false, true),
        ]
    );
    assert_eq!(app.side_row, SideRow::Calendar(3));
    clean_up(&path);
    // Ungrouped items (before any heading) have no heading to tag: z does nothing
    let text = "- loose\n# Work\n- a\n";
    let (mut app, path) = app_with_task_file("archive-ungrouped", text);
    app.focus = Focus::Side;
    app.side_row = SideRow::Calendar(0);
    press(&mut app, KeyCode::Char('z'));
    assert_eq!(std::fs::read_to_string(&path).expect("read"), text);
    clean_up(&path);
}

#[test]
fn l_and_h_move_a_calendar_in_and_out_in_the_calendar_list() {
    let text = "# Work\n- [ ] a @2026-10-10\n# SFAP\n- [ ] b @2026-10-11\n## Bugs\n- [ ] c @2026-10-12\n\
                    # Personal\n- d @2026-10-13\n";
    let (mut app, path) = app_with_task_file("shift-calendar", text);
    let read = || std::fs::read_to_string(&path).expect("read");
    let tree = |app: &App| -> Vec<(String, bool)> {
        app.groups
            .iter()
            .map(|group| (group.path().join(" > "), group.shown))
            .collect()
    };
    app.focus = Focus::Side;
    // Bugs (inside SFAP) unchecked; SFAP chosen
    app.groups[2].shown = false;
    app.side_row = SideRow::Calendar(1);
    // L: SFAP goes inside Work, Bugs along with it (still unchecked); SFAP stays chosen
    press(&mut app, KeyCode::Char('L'));
    let inside = "# Work\n- [ ] a @2026-10-10\n## SFAP\n- [ ] b @2026-10-11\n### Bugs\n- [ ] c @2026-10-12\n\
                      # Personal\n- d @2026-10-13\n";
    assert_eq!(read(), inside);
    assert_eq!(
        tree(&app),
        [
            ("Work".to_string(), true),
            ("Work > SFAP".to_string(), true),
            ("Work > SFAP > Bugs".to_string(), false),
            ("Personal".to_string(), true),
        ]
    );
    assert_eq!(app.side_row, SideRow::Calendar(1));
    // H: back out
    press(&mut app, KeyCode::Char('H'));
    assert_eq!(read(), text);
    assert_eq!(app.side_row, SideRow::Calendar(1));
    assert_eq!(tree(&app)[2], ("SFAP > Bugs".to_string(), false));
    // Cannot move: SFAP is already at the top; Work has no calendar above it to go inside
    let steps = app.undo.len();
    press(&mut app, KeyCode::Char('H'));
    app.side_row = SideRow::Calendar(0);
    press(&mut app, KeyCode::Char('L'));
    assert_eq!((read(), app.undo.len()), (text.to_string(), steps));
    // u undoes the last move (H): SFAP is inside Work again
    press(&mut app, KeyCode::Char('u'));
    assert_eq!(read(), inside);
    clean_up(&path);
}

#[test]
fn d_then_y_deletes_with_subtasks_and_u_brings_them_back() {
    let (mut app, path) = app_with_task_file("delete", WITH_SUBTASK);
    let read = || std::fs::read_to_string(&path).expect("read");
    // d: asks first, file not touched yet; "Review PRs" + subtask = 2 items
    press(&mut app, KeyCode::Char('d'));
    assert_eq!(
        app.confirm_delete()
            .map(|(item, count)| (item.title.as_str(), count)),
        Some(("Review PRs", 2))
    );
    assert_eq!(read(), WITH_SUBTASK);
    // y: deletes "Review PRs" and its subtask, only "Deploy to staging" is left; the agenda chooses it
    press(&mut app, KeyCode::Char('y'));
    assert_eq!(read(), "- [ ] Deploy to staging @2026-10-17 14:00\n");
    assert_eq!(app.confirm_delete(), None);
    assert_eq!(
        app.chosen().map(|entry| entry.item.title.as_str()),
        Some("Deploy to staging")
    );
    // u: the whole block comes back
    press(&mut app, KeyCode::Char('u'));
    assert_eq!(read(), WITH_SUBTASK);
    assert_eq!(app.notice, Some(Notice::Undone("Review PRs".into())));
    clean_up(&path);
}

#[test]
fn dd_deletes_like_dy() {
    // An item: d, then d again (like vim's dd)
    let (mut app, path) = app_with_task_file("delete-dd", WITH_SUBTASK);
    press(&mut app, KeyCode::Char('d'));
    press(&mut app, KeyCode::Char('d'));
    assert_eq!(
        std::fs::read_to_string(&path).expect("read"),
        "- [ ] Deploy to staging @2026-10-17 14:00\n"
    );
    clean_up(&path);
    // A calendar in the calendar list: the same
    let text = "# Work\n- [ ] Review PRs @2026-10-10\n# Personal\n- Gym @2026-10-12\n";
    let (mut app, path) = app_with_task_file("delete-calendar-dd", text);
    app.focus = Focus::Side;
    press(&mut app, KeyCode::Char('d'));
    press(&mut app, KeyCode::Char('d'));
    assert_eq!(
        std::fs::read_to_string(&path).expect("read"),
        "# Personal\n- Gym @2026-10-12\n"
    );
    clean_up(&path);
}

#[test]
fn any_key_other_than_d_or_y_cancels_the_delete() {
    let (mut app, path) = app_with_task_file("delete-cancel", WITH_SUBTASK);
    for other in [KeyCode::Char('j'), KeyCode::Char('n'), KeyCode::Esc] {
        press(&mut app, KeyCode::Char('d'));
        // The key used to cancel only cancels: still in the agenda, still on the first item
        press(&mut app, other);
        assert_eq!(app.confirm_delete(), None, "{other:?}");
        assert_eq!(
            (app.focus, app.agenda_index),
            (Focus::Agenda, 0),
            "{other:?}"
        );
    }
    assert_eq!(std::fs::read_to_string(&path).expect("read"), WITH_SUBTASK);
    clean_up(&path);
}

#[test]
fn ctrl_c_quits_from_the_delete_question() {
    let (mut app, path) = app_with_task_file("delete-quit", WITH_SUBTASK);
    press(&mut app, KeyCode::Char('d'));
    app.handle_key(KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL));
    assert!(app.should_quit);
    assert_eq!(std::fs::read_to_string(&path).expect("read"), WITH_SUBTASK);
    clean_up(&path);
}

#[test]
fn a_click_or_the_wheel_cancels_the_delete_question() {
    // Clicking or scrolling while asking "delete?": it just cancels (like any other
    // key); that click does nothing else (does not choose or check off what is under it)
    let (mut app, path) = app_with_task_file("delete-click", WITH_SUBTASK);
    app.screen = ratatui::layout::Rect::new(0, 0, 70, 23);
    let selected = app.selected;
    for kind in [
        MouseEventKind::Down(MouseButton::Left),
        MouseEventKind::ScrollDown,
    ] {
        press(&mut app, KeyCode::Char('d'));
        app.handle_mouse(MouseEvent {
            kind,
            column: 5,
            row: 5,
            modifiers: KeyModifiers::NONE,
        });
        assert_eq!(app.confirm_delete(), None, "{kind:?}");
        assert_eq!(
            (app.focus, app.selected),
            (Focus::Agenda, selected),
            "{kind:?}"
        );
    }
    // Moving the mouse does not count: still asking
    press(&mut app, KeyCode::Char('d'));
    app.handle_mouse(MouseEvent {
        kind: MouseEventKind::Moved,
        column: 5,
        row: 5,
        modifiers: KeyModifiers::NONE,
    });
    assert!(app.confirm_delete().is_some());
    assert_eq!(std::fs::read_to_string(&path).expect("read"), WITH_SUBTASK);
    clean_up(&path);
}

#[test]
fn y_does_not_delete_when_the_subtasks_changed_meanwhile() {
    let (mut app, path) = app_with_task_file("delete-changed", WITH_SUBTASK);
    press(&mut app, KeyCode::Char('d'));
    // It was 2 items when it asked; before pressing y, a subtask was added
    // elsewhere → it would delete 3, which differs from what it asked
    let edited = "- [ ] Review PRs @2026-10-17 09:00\n  - [ ] Amy's PR\n  - [ ] Kevin's PR\n- [ ] Deploy to staging @2026-10-17 14:00\n";
    std::fs::write(&path, edited).expect("edit");
    press(&mut app, KeyCode::Char('y'));
    assert_eq!(std::fs::read_to_string(&path).expect("read"), edited);
    assert_eq!(app.notice, Some(Notice::Reloaded));
    clean_up(&path);
}

#[test]
fn deleting_the_last_item_goes_back_to_the_calendar() {
    let (mut app, path) = app_with_task_file("delete-last", "- [ ] Review PRs @2026-10-17\n");
    press(&mut app, KeyCode::Char('d'));
    press(&mut app, KeyCode::Char('y'));
    assert_eq!(std::fs::read_to_string(&path).expect("read"), "");
    assert_eq!(app.focus, Focus::Calendar);
    clean_up(&path);
}

#[test]
fn rewriting_keeps_the_calendars_checked_state() {
    // "Personal" is unchecked; after a check-off writes the file and
    // the calendars are re-listed, "Personal" is still unchecked
    let text = "- [ ] Review PRs @2026-10-17 09:00\n# Personal\n- [ ] Go hiking @2026-10-18\n";
    let (mut app, path) = app_with_task_file("groups", text);
    app.groups[1].shown = false;
    press(&mut app, KeyCode::Char(' '));
    assert!(app.items[0].done);
    assert!(app.groups[0].shown && !app.groups[1].shown);
    clean_up(&path);
}

#[test]
fn a_change_made_elsewhere_shows_up_without_a_key_but_roh_cals_own_writes_do_not_count() {
    let (mut app, path) = app_with_task_file("refresh", "- [ ] Review PRs @2026-10-17 09:00\n");
    // A line was added elsewhere (e.g. saved in Obsidian): picked up quietly, no notice
    let edited = "- [ ] Review PRs @2026-10-17 09:00\n- [ ] Book a haircut @2026-10-17 10:00\n";
    std::fs::write(&path, edited).expect("edit");
    app.refresh_tasks();
    assert_eq!(app.items.len(), 2);
    assert_eq!(app.notice, None);
    // roh-cal's own write changes the file too, but it is what the screen shows: undo still works
    press(&mut app, KeyCode::Char(' '));
    app.refresh_tasks();
    press(&mut app, KeyCode::Char('u'));
    assert!(matches!(app.notice, Some(Notice::Undone(_))));
    assert_eq!(std::fs::read_to_string(&path).expect("read"), edited);
    clean_up(&path);
}

#[test]
fn a_change_made_elsewhere_waits_until_the_editor_closes() {
    let (mut app, path) =
        app_with_task_file("refresh-editor", "- [ ] Review PRs @2026-10-17 09:00\n");
    press(&mut app, KeyCode::Char('e'));
    let edited = "- [ ] Go to the gym @2026-10-17 08:00\n- [ ] Review PRs @2026-10-17 09:00\n";
    std::fs::write(&path, edited).expect("edit");
    // The editor is open: the items it points at stay as they are
    app.refresh_tasks();
    assert_eq!(app.items.len(), 1);
    // Closed: picked up on the next round
    press(&mut app, KeyCode::Esc);
    app.refresh_tasks();
    assert_eq!(app.items.len(), 2);
    clean_up(&path);
}

#[test]
fn the_chosen_item_on_the_tasks_page_follows_a_line_added_elsewhere() {
    let text = "- [ ] Review PRs @2026-10-17 09:00\n- [ ] Deploy to staging @2026-10-17 14:00\n";
    let (mut app, path) = app_with_task_file("refresh-line", text);
    app.set_view(View::Tasks);
    app.tasks_line = Some(2);
    // A line added above it elsewhere: "Deploy to staging" moves to line 3, and stays chosen
    std::fs::write(
        &path,
        format!("- [ ] Go to the gym @2026-10-17 08:00\n{text}"),
    )
    .expect("edit");
    app.refresh_tasks();
    assert_eq!(app.tasks_line, Some(3));
    assert_eq!(
        app.chosen_item().map(|item| item.title.as_str()),
        Some("Deploy to staging")
    );
    clean_up(&path);
}

#[test]
fn a_task_file_that_cannot_be_read_keeps_the_screen_and_is_tried_again() {
    use std::os::unix::fs::PermissionsExt;
    let (mut app, path) =
        app_with_task_file("refresh-unreadable", "- [ ] Review PRs @2026-10-17 09:00\n");
    let edited = "- [ ] Review PRs @2026-10-17 09:00\n- [ ] Book a haircut @2026-10-17 10:00\n";
    std::fs::write(&path, edited).expect("edit");
    // Changed but not readable right now: the screen stays as it is
    let mode = |bits| std::fs::Permissions::from_mode(bits);
    std::fs::set_permissions(&path, mode(0o000)).expect("chmod");
    app.refresh_tasks();
    assert_eq!(app.items.len(), 1);
    // Readable again (its modified time did not change): picked up on the next round
    std::fs::set_permissions(&path, mode(0o644)).expect("chmod");
    app.refresh_tasks();
    assert_eq!(app.items.len(), 2);
    clean_up(&path);
}

#[test]
fn slash_marks_the_chosen_task_in_progress_and_back() {
    let text = "- [ ] Draft the release notes @2026-10-17\n- Standup @2026-10-17 09:00\n";
    let (mut app, path) = app_with_task_file("progress", text);
    let read = || std::fs::read_to_string(&path).expect("read");
    // `/` on the chosen task (in the agenda): `[/]`, and the screen follows
    press(&mut app, KeyCode::Char('/'));
    assert_eq!(
        read(),
        "- [/] Draft the release notes @2026-10-17\n- Standup @2026-10-17 09:00\n"
    );
    assert!(app.items[0].in_progress);
    // Again: back to open; `u` undoes that
    press(&mut app, KeyCode::Char('/'));
    assert_eq!(read(), text);
    press(&mut app, KeyCode::Char('u'));
    assert!(app.items[0].in_progress);
    // Space checks an in-progress task off
    press(&mut app, KeyCode::Char(' '));
    assert!(app.items[0].done && !app.items[0].in_progress);
    // An event has no box: the bottom row says so, the file stays
    let before = read();
    press(&mut app, KeyCode::Char('j'));
    press(&mut app, KeyCode::Char('/'));
    assert_eq!(app.notice, Some(Notice::NotATask));
    assert_eq!(read(), before);
    clean_up(&path);
}

#[test]
fn slash_works_in_the_day_popup_and_on_the_tasks_page() {
    let text = "- [ ] Draft the release notes @2026-10-17\n";
    let (mut app, path) = app_with_task_file("progress-views", text);
    let read = || std::fs::read_to_string(&path).expect("read");
    // Layout 2's day popup
    app.set_view(View::Month);
    app.focus = Focus::Day;
    press(&mut app, KeyCode::Char('/'));
    assert_eq!(read(), "- [/] Draft the release notes @2026-10-17\n");
    // The tasks page
    app.set_view(View::Tasks);
    press(&mut app, KeyCode::Char('/'));
    assert_eq!(read(), text);
    clean_up(&path);
}

#[test]
fn the_chosen_item_stays_chosen_when_space_or_slash_moves_it_within_its_day() {
    let text = "- [ ] Email Kevin @2026-10-17\n- [ ] Estimate 2601RC @2026-10-17\n- [ ] Pay the fine @2026-10-17\n";
    let (mut app, path) = app_with_task_file("progress-choice", text);
    let chosen = |app: &App| {
        app.chosen_item()
            .map(|item| (item.title.clone(), item.line))
    };
    // Agenda: choose Estimate (2nd); `/` moves it to the top of the day, and it is still the chosen one
    press(&mut app, KeyCode::Char('j'));
    press(&mut app, KeyCode::Char('/'));
    assert_eq!(app.agenda_index, 0);
    assert_eq!(chosen(&app), Some(("Estimate 2601RC".into(), 2)));
    // Space checks it off: back to its place in the file order, still chosen
    press(&mut app, KeyCode::Char(' '));
    assert_eq!(app.agenda_index, 1);
    assert_eq!(chosen(&app), Some(("Estimate 2601RC".into(), 2)));
    // Layout 2's day popup: the same
    app.set_view(View::Month);
    app.focus = Focus::Day;
    app.day_index = 2;
    press(&mut app, KeyCode::Char('/'));
    assert_eq!(app.day_index, 0);
    assert_eq!(chosen(&app), Some(("Pay the fine".into(), 3)));
    clean_up(&path);
}

#[test]
fn with_done_tasks_hidden_checking_one_off_leaves_the_next_one_chosen() {
    let text = "- [ ] Email Kevin @2026-10-17\n- [ ] Estimate 2601RC @2026-10-17\n- [ ] Pay the fine @2026-10-17\n";
    let (mut app, path) = app_with_task_file("progress-hidden", text);
    app.settings.calendar_list.hide_done = true;
    // Email is checked off and hidden: the next one (Estimate) is chosen, ready for the next Space
    press(&mut app, KeyCode::Char(' '));
    assert_eq!(app.focus, Focus::Agenda);
    assert_eq!(
        app.chosen_item().map(|item| item.title.as_str()),
        Some("Estimate 2601RC")
    );
    clean_up(&path);
}

#[test]
fn the_chosen_item_stays_chosen_when_a_change_made_elsewhere_reorders_its_day() {
    let text = "- [ ] Email Kevin @2026-10-17\n- [ ] Estimate 2601RC @2026-10-17\n- [ ] Pay the fine @2026-10-17\n";
    let (mut app, path) = app_with_task_file("refresh-choice", text);
    let chosen = |app: &App| app.chosen_item().map(|item| item.title.clone());
    // Agenda: Pay the fine (3rd) is chosen; it is marked `[/]` elsewhere and moves to the top of its day
    app.choose(2);
    let marked = text.replace("- [ ] Pay", "- [/] Pay");
    std::fs::write(&path, &marked).expect("edit");
    app.refresh_tasks();
    assert_eq!(app.agenda_index, 0);
    assert_eq!(chosen(&app).as_deref(), Some("Pay the fine"));
    // A line added above it elsewhere: its file line moves (3 → 4), and it is still the chosen one
    app.choose(2);
    assert_eq!(chosen(&app).as_deref(), Some("Estimate 2601RC"));
    let added = format!("- [ ] Call Mom @2026-10-17\n{marked}");
    std::fs::write(&path, &added).expect("edit");
    app.refresh_tasks();
    assert_eq!(app.agenda_index, 3);
    assert_eq!(chosen(&app).as_deref(), Some("Estimate 2601RC"));
    // Layout 2's day popup: Pay the fine (in progress), then file order: Call Mom, Email (2), Estimate.
    // Email is marked `[/]` elsewhere: in progress ones keep file order, so it moves to the top
    app.set_view(View::Month);
    app.focus = Focus::Day;
    app.day_index = 2;
    assert_eq!(chosen(&app).as_deref(), Some("Email Kevin"));
    std::fs::write(&path, added.replace("- [ ] Email", "- [/] Email")).expect("edit");
    app.refresh_tasks();
    assert_eq!(app.day_index, 0);
    assert_eq!(chosen(&app).as_deref(), Some("Email Kevin"));
    clean_up(&path);
}
