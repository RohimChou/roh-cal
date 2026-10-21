//! Small helpers the tests share: pressing keys, typing into the editor box, the mouse

use crate::app::App;
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseEvent, MouseEventKind};

/// Presses one key (no modifier such as Ctrl)
pub fn press(app: &mut App, code: KeyCode) {
    app.handle_key(KeyEvent::from(code));
}

/// Types text into the editor box one character at a time
pub fn type_text(app: &mut App, text: &str) {
    for c in text.chars() {
        press(app, KeyCode::Char(c));
    }
}

/// One mouse event at (`column`, `row`), no modifier (for `App::handle_mouse`)
pub fn mouse_event(kind: MouseEventKind, column: u16, row: u16) -> MouseEvent {
    MouseEvent {
        kind,
        column,
        row,
        modifiers: KeyModifiers::NONE,
    }
}

/// One mouse action at (`column`, `row`): a click, a wheel step, a drag…
pub fn mouse(app: &mut App, kind: MouseEventKind, column: u16, row: u16) {
    app.handle_mouse(mouse_event(kind, column, row));
}
