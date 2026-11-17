use super::super::Toast;
use super::*;
use crate::test_utils::{mouse, mouse_event, press};
use crate::texts::Lang;
use jiff::civil::date;
use ratatui::layout::Rect;
use std::path::PathBuf;
use std::time::Instant;

/// An app with the help page open: today is 2026/10/17, terminal is `width` × `height`
fn app_with_help(width: u16, height: u16) -> App {
    let mut app = App::new(Lang::ZhTw, date(2026, 10, 17), Vec::new());
    app.screen = Rect::new(0, 0, width, height);
    app.handle_key(KeyEvent::from(KeyCode::Char('?')));
    app
}

#[test]
fn question_mark_opens_the_short_page_and_tab_switches() {
    let mut app = app_with_help(100, 40);
    assert_eq!(app.help(), Some(Help::default()));
    press(&mut app, KeyCode::Tab);
    assert_eq!(
        app.help(),
        Some(Help {
            full: true,
            scroll: 0
        })
    );
    press(&mut app, KeyCode::BackTab);
    assert_eq!(app.help(), Some(Help::default()));
}

#[test]
fn question_mark_again_shows_all_keys_and_a_third_time_closes_it() {
    // ? on the short version: the full one (back to the top); ? on the full one: closed, without quitting.
    // 44 × 20: the short version does not fit either, so j scrolls it first
    let mut app = app_with_help(44, 20);
    press(&mut app, KeyCode::Char('j'));
    assert_eq!(app.help().map(|help| help.scroll), Some(1));
    press(&mut app, KeyCode::Char('?'));
    assert_eq!(
        app.help(),
        Some(Help {
            full: true,
            scroll: 0
        })
    );
    press(&mut app, KeyCode::Char('?'));
    assert_eq!(app.help(), None);
    assert!(!app.should_quit);
    // Tab back to the short version first: ? shows the full one again
    let mut app = app_with_help(100, 40);
    press(&mut app, KeyCode::Tab);
    press(&mut app, KeyCode::Tab);
    press(&mut app, KeyCode::Char('?'));
    assert_eq!(app.help().map(|help| help.full), Some(true));
}

#[test]
fn q_and_esc_close_it_without_quitting() {
    for code in [KeyCode::Char('q'), KeyCode::Esc] {
        for full in [false, true] {
            let mut app = app_with_help(100, 40);
            if full {
                press(&mut app, KeyCode::Tab);
            }
            press(&mut app, code);
            assert_eq!(app.help(), None, "{code:?} {full}");
            assert!(!app.should_quit, "{code:?} {full}");
        }
    }
}

#[test]
fn other_keys_do_nothing_while_it_is_open() {
    let mut app = app_with_help(100, 40);
    // Moving, switching layout, switching pane, and adding are all ignored: the selected day, layout, and
    // pane stay the same
    for code in ['l', 'j', '3', 'a', 't', ']', 's', 'u'].map(KeyCode::Char) {
        press(&mut app, code);
    }
    assert_eq!(app.help(), Some(Help::default()));
    assert_eq!(app.selected, date(2026, 10, 17));
    assert_eq!(app.view, super::super::View::Planner);
    assert!(app.editor().is_none());
    // Ctrl+C still quits
    app.handle_key(KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL));
    assert!(app.should_quit);
}

#[test]
fn j_and_k_scroll_only_when_it_does_not_fit() {
    // 100 × 47: the full version fits, so j does nothing
    let mut app = app_with_help(100, 47);
    press(&mut app, KeyCode::Tab);
    press(&mut app, KeyCode::Char('j'));
    assert_eq!(app.help().map(|help| help.scroll), Some(0));
    // 100 × 20: full version 43 lines + padding 2 + border 2 = 47 lines; the box is at most 20 tall,
    // so 16 lines fit inside → it can scroll 27 lines
    let mut app = app_with_help(100, 20);
    press(&mut app, KeyCode::Tab);
    for _ in 0..28 {
        press(&mut app, KeyCode::Down);
    }
    assert_eq!(app.help().map(|help| help.scroll), Some(27));
    press(&mut app, KeyCode::Char('k'));
    assert_eq!(app.help().map(|help| help.scroll), Some(26));
    // The wheel does the same; the calendar below does not move
    app.handle_mouse(mouse_event(MouseEventKind::ScrollUp, 50, 10));
    app.handle_mouse(mouse_event(MouseEventKind::ScrollUp, 50, 10));
    assert_eq!(app.help().map(|help| help.scroll), Some(24));
    assert_eq!(app.selected, date(2026, 10, 17));
    // Tab switches back to the short version: back to the top
    press(&mut app, KeyCode::Tab);
    assert_eq!(app.help(), Some(Help::default()));
}

#[test]
fn clicking_outside_closes_it() {
    let mut app = app_with_help(100, 40);
    // A click inside the box (in the middle) is ignored, and nothing on the calendar below gets selected
    app.handle_mouse(mouse_event(MouseEventKind::Down(MouseButton::Left), 50, 20));
    assert!(app.help().is_some());
    assert_eq!(app.selected, date(2026, 10, 17));
    // A click in the top-left corner (outside the box): closes it, and that click selects nothing below
    app.handle_mouse(mouse_event(MouseEventKind::Down(MouseButton::Left), 0, 0));
    assert_eq!(app.help(), None);
    assert_eq!(app.selected, date(2026, 10, 17));
}

/// The help page open on a 70 × 23 screen with the task file and the config file under `/nonexistent`
/// (the box of `ui::help`'s `files_are_listed_on_top_of_the_keys`): each path starts at column 21, its first
/// line `/nonexistent/roh-cal/` (21 characters) on rows 4 and 6, the rest (`tasks.md`, `config.toml`) on
/// rows 5 and 7
fn help_with_files() -> App {
    let mut app = App::new(Lang::ZhTw, date(2026, 10, 17), Vec::new());
    app.tasks_path = Some(PathBuf::from("/nonexistent/roh-cal/tasks.md"));
    app.config_path = Some(PathBuf::from("/nonexistent/roh-cal/config.toml"));
    app.screen = Rect::new(0, 0, 70, 23);
    press(&mut app, KeyCode::Char('?'));
    app
}

#[test]
fn a_drag_over_the_paths_marks_text_until_a_key_or_the_next_press() {
    let mut app = help_with_files();
    let spot = |item, char| TextSpot { item, char };
    let down = MouseEventKind::Down(MouseButton::Left);
    let drag = MouseEventKind::Drag(MouseButton::Left);
    // A press on the task file's 「r」 of roh-cal: the drag starts there, but nothing is marked before the
    // mouse moves
    mouse(&mut app, down, 34, 4);
    assert_eq!(app.help_drag.map(|drag| drag.anchor), Some(spot(0, 13)));
    assert_eq!(app.help_selection(), None);
    // Dragged to 「k」 of tasks.md on the next row: both ends included
    mouse(&mut app, drag, 24, 5);
    assert_eq!(app.help_selection(), Some((spot(0, 13), spot(0, 24))));
    // Over the keys below (no path) or above the box (the nearest row inside is 「檔案」): stays as it was
    mouse(&mut app, drag, 30, 10);
    mouse(&mut app, drag, 30, 0);
    assert_eq!(app.help_selection(), Some((spot(0, 13), spot(0, 24))));
    // Past the end of config.toml's row: its last character; left of the box on the config file's first
    // row: its first character
    mouse(&mut app, drag, 60, 7);
    assert_eq!(app.help_selection(), Some((spot(0, 13), spot(1, 31))));
    mouse(&mut app, drag, 0, 6);
    assert_eq!(app.help_selection(), Some((spot(0, 13), spot(1, 0))));
    // On the label 任務檔: that row's first character, now before where the drag started (that character
    // stays in)
    mouse(&mut app, drag, 13, 4);
    assert_eq!(app.help_selection(), Some((spot(0, 0), spot(0, 13))));
    // The next press starts over: inside the box but not on a path, no text, and the page stays open
    mouse(&mut app, down, 30, 10);
    assert_eq!(app.help_drag, None);
    assert!(app.help().is_some());
    // A key clears it too
    mouse(&mut app, down, 34, 4);
    mouse(&mut app, drag, 24, 5);
    assert!(app.help_selection().is_some());
    press(&mut app, KeyCode::Char('j'));
    assert_eq!(app.help_drag, None);
}

#[test]
fn letting_go_of_a_drag_copies_the_paths_as_shown() {
    let mut app = help_with_files();
    let down = MouseEventKind::Down(MouseButton::Left);
    let drag = MouseEventKind::Drag(MouseButton::Left);
    let up = MouseEventKind::Up(MouseButton::Left);
    // A press and a let-go without moving: nothing to copy
    mouse(&mut app, down, 21, 4);
    mouse(&mut app, up, 21, 4);
    assert_eq!(app.to_copy, None);
    // The whole task file path over its two rows, past the note 「（還沒建立）」 after it: one line (wrapping
    // is only on screen), without the note
    mouse(&mut app, drag, 35, 5);
    mouse(&mut app, up, 35, 5);
    assert_eq!(
        app.to_copy.as_deref(),
        Some("/nonexistent/roh-cal/tasks.md")
    );
    // From tasks.md into the config file's first row: one file per line
    mouse(&mut app, down, 21, 5);
    mouse(&mut app, drag, 32, 6);
    mouse(&mut app, up, 32, 6);
    assert_eq!(app.to_copy.as_deref(), Some("tasks.md\n/nonexistent"));
    // The main loop copies it and says how many characters (the line break counts); the page stays open,
    // the text stays marked
    let now = Instant::now();
    let mut copied = None;
    app.copy_out(
        |text| {
            copied = Some(text.to_string());
            Ok(())
        },
        now,
    );
    assert_eq!(copied.as_deref(), Some("tasks.md\n/nonexistent"));
    assert_eq!(app.toast, Some((Toast::Copied(21), now)));
    assert!(app.help().is_some());
    assert!(app.help_selection().is_some());
}
