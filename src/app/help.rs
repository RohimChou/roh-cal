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
mod tests;
