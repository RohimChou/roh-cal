//! Rescheduling (`r`): the calendar itself is the date picker. After `r` the keyboard moves to the calendar,
//! where the usual keys (`hjkl`, `[` `]`, `t`, the mouse) pick the new date;
//! `Enter` (or clicking the same day again) writes back to the task file, `Esc` cancels
//!
//! Like dragging: the item moves as many days later (earlier) as the selected day moved; a multi-day item
//! moves as a whole

use super::{App, Focus, Mode, View};
use crate::edit;
use crate::tasks::Item;
use crate::ui::{self, Click};
use jiff::{ToSpan, civil::Date};
use ratatui::crossterm::event::{
    KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
};

/// Rescheduling in progress: the item to move, and the layout and pane to return to when done (or cancelled)
#[derive(Debug)]
pub struct Moving {
    /// The item to move (as read when `r` was pressed)
    pub item: Item,
    /// The selected day when `r` was pressed: the item moves as many days as the selected day moves
    from: Date,
    /// Which layout and pane we were in before `r`
    back: (View, Focus),
}

impl App {
    /// `r`: starts moving the item chosen in the agenda (or tasks page). The keyboard moves to the calendar,
    /// and the selected day stays on the item's day; the tasks page borrows layout 1's calendar
    pub(super) fn start_moving(&mut self) {
        let Some(item) = self.chosen_item().cloned() else {
            return;
        };
        let back = (self.view, self.focus);
        // The tasks page has no calendar: borrow layout 1's, starting at the item's day (or today if undated)
        if self.view == View::Tasks {
            self.view = View::Planner;
            self.selected = item.date.unwrap_or(self.today);
            self.scroll_to_month();
        }
        self.mode = Mode::Moving(Moving {
            item,
            from: self.selected,
            back,
        });
        self.focus = Focus::Calendar;
    }

    /// Keys while rescheduling: moving the selected day works as usual; `Enter` confirms, `Esc` cancels,
    /// `Ctrl+C` quits, other keys are ignored
    pub(super) fn handle_moving_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.should_quit = true;
            }
            KeyCode::Enter => self.finish_moving(),
            KeyCode::Esc => self.cancel_moving(),
            KeyCode::Char('t') => self.go_today(),
            KeyCode::Char('[') => self.jump_month(-1),
            KeyCode::Char(']') => self.jump_month(1),
            // Only keys that move the date (things like `a` to add on the calendar are ignored)
            code @ (KeyCode::Char('h' | 'j' | 'k' | 'l')
            | KeyCode::Left
            | KeyCode::Right
            | KeyCode::Up
            | KeyCode::Down) => self.handle_calendar_key(code),
            _ => {}
        }
    }

    /// Mouse while rescheduling: the wheel moves the selected day as usual (a week); clicking a date picks the
    /// new date (clicking the same day again = confirm); clicks elsewhere are ignored
    pub(super) fn handle_moving_mouse(&mut self, mouse: MouseEvent) {
        match mouse.kind {
            MouseEventKind::ScrollDown => self.scroll(1),
            MouseEventKind::ScrollUp => self.scroll(-1),
            MouseEventKind::Down(MouseButton::Left) => {
                if let Some(Click::Day(date)) = ui::click_at(self, mouse.column, mouse.row) {
                    self.click_while_moving(date);
                }
            }
            _ => {}
        }
    }

    /// Click on a calendar day while rescheduling: select it; clicking the already selected day again = confirm
    pub(super) fn click_while_moving(&mut self, day: Date) {
        if day == self.selected {
            self.finish_moving();
        } else {
            self.selected = day;
        }
    }

    /// Confirm: move the item as many days as the selected day moved and write back to the task file;
    /// go back to the pane from before `r` (the agenda or layout 2's popup has the moved item chosen)
    fn finish_moving(&mut self) {
        let Moving { item, from, back } = match std::mem::take(&mut self.mode) {
            Mode::Moving(moving) => moving,
            // Not rescheduling: do nothing (put it back)
            other => {
                self.mode = other;
                return;
            }
        };
        let to = self.selected;
        // The new day: shift by as many days as the selected day moved (no write if it did not move); an item
        // with no date yet gets the selected day
        let new_first = match item.date {
            Some(first) if to != from => Some(first.saturating_add((to - from).get_days().days())),
            Some(_) => None,
            None => Some(to),
        };
        // Moved (or no move needed); `false` if the write failed or the file was changed elsewhere
        let mut moved = true;
        if let Some(new_first) = new_first {
            let Some((path, text)) = self.read_tasks() else {
                // Cannot read the task file: not moved; the selected day goes back to the one at `r`
                // (same as cancelling)
                self.selected = from;
                self.go_back(back);
                return;
            };
            moved = match edit::reschedule(&text, &item, new_first) {
                Ok(after) => self.write_tasks(&path, text, after, item.title),
                Err(edit::Changed) => {
                    self.reload(&text);
                    false
                }
            };
        }
        self.go_back(back);
        // The agenda chooses the moved item (only that line changes, so its line number is the same); if the
        // move failed or it is not found, stay on the calendar
        if back.1 == Focus::Agenda {
            let index = self
                .agenda()
                .iter()
                .position(|entry| entry.item.line == item.line && entry.is_on(to));
            match index {
                Some(index) if moved => self.agenda_index = index,
                _ => self.focus = Focus::Calendar,
            }
        }
        // Layout 2's popup: open on the new day with the moved item chosen; if the move failed, close it
        if back.1 == Focus::Day {
            if moved {
                self.choose_day_line(item.line);
            } else {
                self.focus = Focus::Calendar;
            }
        }
    }

    /// Cancel: the selected day goes back to the one at `r`, and the pane goes back to the one before `r`
    fn cancel_moving(&mut self) {
        match std::mem::take(&mut self.mode) {
            Mode::Moving(Moving { from, back, .. }) => {
                self.selected = from;
                self.go_back(back);
            }
            other => self.mode = other,
        }
    }

    /// Goes back to the layout and pane from before `r`
    fn go_back(&mut self, (view, focus): (View, Focus)) {
        self.view = view;
        self.focus = focus;
    }
}

#[cfg(test)]
mod tests;
