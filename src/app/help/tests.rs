use super::*;
use crate::texts::Lang;
use jiff::civil::date;
use ratatui::layout::Rect;

/// An app with the help page open: today is 2026/10/17, terminal is `width` × `height`
fn app_with_help(width: u16, height: u16) -> App {
    let mut app = App::new(Lang::ZhTw, date(2026, 10, 17), Vec::new());
    app.screen = Rect::new(0, 0, width, height);
    app.handle_key(KeyEvent::from(KeyCode::Char('?')));
    app
}

/// Presses one key
fn press(app: &mut App, code: KeyCode) {
    app.handle_key(KeyEvent::from(code));
}

/// One mouse event at (`column`, `row`)
fn mouse(kind: MouseEventKind, column: u16, row: u16) -> MouseEvent {
    MouseEvent {
        kind,
        column,
        row,
        modifiers: KeyModifiers::NONE,
    }
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
    // 100 × 40: the full version fits, so j does nothing
    let mut app = app_with_help(100, 40);
    press(&mut app, KeyCode::Tab);
    press(&mut app, KeyCode::Char('j'));
    assert_eq!(app.help().map(|help| help.scroll), Some(0));
    // 100 × 20: full version 35 lines + padding 2 + border 2 = 39 lines; the box is at most 20 tall,
    // so 16 lines fit inside → it can scroll 19 lines
    let mut app = app_with_help(100, 20);
    press(&mut app, KeyCode::Tab);
    for _ in 0..20 {
        press(&mut app, KeyCode::Down);
    }
    assert_eq!(app.help().map(|help| help.scroll), Some(19));
    press(&mut app, KeyCode::Char('k'));
    assert_eq!(app.help().map(|help| help.scroll), Some(18));
    // The wheel does the same; the calendar below does not move
    app.handle_mouse(mouse(MouseEventKind::ScrollUp, 50, 10));
    app.handle_mouse(mouse(MouseEventKind::ScrollUp, 50, 10));
    assert_eq!(app.help().map(|help| help.scroll), Some(16));
    assert_eq!(app.selected, date(2026, 10, 17));
    // Tab switches back to the short version: back to the top
    press(&mut app, KeyCode::Tab);
    assert_eq!(app.help(), Some(Help::default()));
}

#[test]
fn clicking_outside_closes_it() {
    let mut app = app_with_help(100, 40);
    // A click inside the box (in the middle) is ignored, and nothing on the calendar below gets selected
    app.handle_mouse(mouse(MouseEventKind::Down(MouseButton::Left), 50, 20));
    assert!(app.help().is_some());
    assert_eq!(app.selected, date(2026, 10, 17));
    // A click in the top-left corner (outside the box): closes it, and that click selects nothing below
    app.handle_mouse(mouse(MouseEventKind::Down(MouseButton::Left), 0, 0));
    assert_eq!(app.help(), None);
    assert_eq!(app.selected, date(2026, 10, 17));
}
