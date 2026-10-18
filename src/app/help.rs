//! Keys and mouse while the help page (`?`) is open: `Tab` switches short / full version; scroll up / down
//! when it does not fit; `?` / `q` / `Esc` or a click outside closes it

use super::{App, Mode};
use crate::ui;
use ratatui::{
    crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind},
    layout::Position,
};

/// The open help page: which version, and how many lines it is scrolled down
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Help {
    /// `true` = full version (all keys); `false` = short version (the most used keys)
    pub full: bool,
    /// How many lines it is scrolled down when it does not fit the screen
    pub scroll: u16,
}

impl App {
    /// Keys while the help page is open: `j` / `k` (`↓` / `↑`) scroll when it does not fit; `Tab` switches
    /// short / full version (back to the top);
    /// `?` / `q` / `Esc` close it; `Ctrl+C` quits; other keys are ignored
    pub(super) fn handle_help_key(&mut self, key: KeyEvent) {
        let Some(help) = self.help() else {
            return;
        };
        match key.code {
            KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.should_quit = true;
            }
            KeyCode::Char('j') | KeyCode::Down => self.scroll_help(1),
            KeyCode::Char('k') | KeyCode::Up => self.scroll_help(-1),
            KeyCode::Tab | KeyCode::BackTab => {
                self.mode = Mode::Help(Help {
                    full: !help.full,
                    scroll: 0,
                });
            }
            KeyCode::Char('?' | 'q') | KeyCode::Esc => self.mode = Mode::Normal,
            _ => {}
        }
    }

    /// Mouse while the help page is open: the wheel scrolls it (the calendar below does not move), a click
    /// outside closes it, a click inside is ignored
    pub(super) fn handle_help_mouse(&mut self, mouse: MouseEvent) {
        let Some(help) = self.help() else {
            return;
        };
        match mouse.kind {
            MouseEventKind::ScrollDown => self.scroll_help(1),
            MouseEventKind::ScrollUp => self.scroll_help(-1),
            MouseEventKind::Down(MouseButton::Left)
                if !ui::help_area(self, help.full)
                    .contains(Position::new(mouse.column, mouse.row)) =>
            {
                self.mode = Mode::Normal;
            }
            _ => {}
        }
    }

    /// Scrolls the help page one line down (`step` = 1) / up (-1): no move if it fits, stops at the end
    fn scroll_help(&mut self, step: i16) {
        let Some(help) = self.help() else {
            return;
        };
        // The most lines it can scroll (for the current terminal size); after the terminal grows taller, the
        // old scroll may be too far, so clamp it first
        let max = ui::help_max_scroll(self, help.full);
        let scroll = help.scroll.min(max).saturating_add_signed(step).min(max);
        self.mode = Mode::Help(Help { scroll, ..help });
    }
}

#[cfg(test)]
mod tests {
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
    fn question_mark_q_and_esc_close_it_without_quitting() {
        for code in [KeyCode::Char('?'), KeyCode::Char('q'), KeyCode::Esc] {
            let mut app = app_with_help(100, 40);
            press(&mut app, code);
            assert_eq!(app.help(), None, "{code:?}");
            assert!(!app.should_quit, "{code:?}");
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
        // 100 × 20: full version 33 lines + padding 2 + border 2 = 37 lines; the box is at most 20 tall,
        // so 16 lines fit inside → it can scroll 17 lines
        let mut app = app_with_help(100, 20);
        press(&mut app, KeyCode::Tab);
        for _ in 0..18 {
            press(&mut app, KeyCode::Down);
        }
        assert_eq!(app.help().map(|help| help.scroll), Some(17));
        press(&mut app, KeyCode::Char('k'));
        assert_eq!(app.help().map(|help| help.scroll), Some(16));
        // The wheel does the same; the calendar below does not move
        app.handle_mouse(mouse(MouseEventKind::ScrollUp, 50, 10));
        app.handle_mouse(mouse(MouseEventKind::ScrollUp, 50, 10));
        assert_eq!(app.help().map(|help| help.scroll), Some(14));
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
}
