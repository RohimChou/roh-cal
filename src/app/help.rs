//! Keys and mouse while the help page (`?`) is open: `Tab` switches short / full version, `?` again goes
//! from the short version to the full one and closes the full one; scroll up / down when it does not fit;
//! dragging over the file paths on top copies them; `q` / `Esc` or a click outside closes it

use super::{App, Mode, TextDrag, TextSpot, text_drag};
use crate::{paths, ui};
use ratatui::{
    crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind},
    layout::Position,
};
use std::path::Path;

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
    /// short / full version (back to the top); `?` on the short version shows the full one (so `?` three
    /// times = short, full, closed), on the full one it closes;
    /// `q` / `Esc` close it; `Ctrl+C` quits; other keys are ignored
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
            KeyCode::Char('?') if !help.full => {
                self.mode = Mode::Help(Help {
                    full: true,
                    scroll: 0,
                });
            }
            KeyCode::Char('?' | 'q') | KeyCode::Esc => self.mode = Mode::Normal,
            _ => {}
        }
    }

    /// Mouse while the help page is open: the wheel scrolls it (the calendar below does not move), a click
    /// outside closes it. Inside, a press on a file path starts dragging over its text from the character
    /// under it (a press anywhere else in the box does nothing); moving with the button held marks the text
    /// up to the character under the mouse (`help_selection`; past the box's edge: the nearest row inside;
    /// on a row with no path it stays where it was); letting go copies it (the main loop does,
    /// `App::copy_out`), and it stays marked until a key or the next press
    pub(super) fn handle_help_mouse(&mut self, mouse: MouseEvent) {
        let Some(help) = self.help() else {
            return;
        };
        let (column, row) = (mouse.column, mouse.row);
        match mouse.kind {
            MouseEventKind::ScrollDown => self.scroll_help(1),
            MouseEventKind::ScrollUp => self.scroll_help(-1),
            MouseEventKind::Down(MouseButton::Left) => {
                self.help_drag = ui::help_text_at(self, help, column, row, false)
                    .map(|anchor| TextDrag { anchor, head: None });
                if !ui::help_area(self, help.full).contains(Position::new(column, row)) {
                    self.mode = Mode::Normal;
                }
            }
            MouseEventKind::Drag(MouseButton::Left) => {
                let spot = ui::help_text_at(self, help, column, row, true);
                if let Some(drag) = &mut self.help_drag
                    && let Some(spot) = spot
                {
                    drag.head = Some(spot);
                }
            }
            MouseEventKind::Up(MouseButton::Left) => self.to_copy = self.help_selected_text(),
            _ => {}
        }
    }

    /// The files listed on top of the help page, top to bottom: the task file and the config file, each with
    /// its label; one with no path (no HOME, tests) is left out. `TextSpot::item` counts in this list
    pub fn help_files(&self) -> Vec<(&'static str, &Path)> {
        let help = self.lang.texts().help;
        [
            (help.task_file, &self.tasks_path),
            (help.config_file, &self.config_path),
        ]
        .into_iter()
        .filter_map(|(label, path)| Some((label, path.as_deref()?)))
        .collect()
    }

    /// The path text dragged over on the help page, from its first to its last character (both included, in
    /// reading order); `None` until the mouse moves with the button held
    pub fn help_selection(&self) -> Option<(TextSpot, TextSpot)> {
        self.help_drag?.range()
    }

    /// The dragged-over text (`help_selection`): each file's part of its path as shown (`paths::shown`, the
    /// home folder as `~`), one file per line (a path wrapped on the page is still one line), e.g.
    /// 「~/.config/roh-cal/config.toml」
    pub fn help_selected_text(&self) -> Option<String> {
        let (first, last) = self.help_selection()?;
        let shown: Vec<String> = self
            .help_files()
            .into_iter()
            .map(|(_, path)| paths::shown(path).0)
            .collect();
        let shown: Vec<&str> = shown.iter().map(String::as_str).collect();
        Some(text_drag::text_between(&shown, first, last))
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
