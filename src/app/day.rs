//! The day popup: `Enter` on the calendar (or double-clicking a day) opens it, in layouts 1 and 2. It lists all
//! items on the selected day (also ones a cell cannot fit and shows as `+N`). The keys are the same as in
//! layout 1's agenda (drawing is in `ui::day`)

use super::{App, Focus};
use crate::ui;
use ratatui::{crossterm::event::KeyCode, layout::Position};

impl App {
    /// Opens the popup with the first item chosen; does not open if the day has no items
    pub(super) fn open_day_popup(&mut self) {
        if self.items_on(self.selected).is_empty() {
            return;
        }
        self.day_index = 0;
        self.day_clicked = false;
        self.day_top = 0;
        self.focus = Focus::Day;
    }

    /// Keyboard in the popup: `j` / `k` next / previous item. Keys that write match the agenda:
    /// `Space` / `x` check, `r` reschedule, `e` / `Enter` edit,
    /// `a` add, `A` add a subtask, `d` delete (closing with `Esc` / `q` is in `handle_normal_key`)
    pub(super) fn handle_day_key(&mut self, code: KeyCode) {
        match code {
            KeyCode::Char('j') | KeyCode::Down => self.choose_day(1),
            KeyCode::Char('k') | KeyCode::Up => self.choose_day(-1),
            KeyCode::Char(' ' | 'x') => self.toggle_chosen(),
            KeyCode::Char('r') => self.start_moving(),
            KeyCode::Char('e') | KeyCode::Enter => self.start_editing(),
            KeyCode::Char('a') => self.start_adding(),
            KeyCode::Char('A') => self.start_adding_subtask(),
            KeyCode::Char('d') => self.ask_delete(),
            _ => {}
        }
    }

    /// Chooses the item `step` items further down in the popup (negative = up); stops at the first / last item
    pub(super) fn choose_day(&mut self, step: isize) {
        let last = self.items_on(self.selected).len().saturating_sub(1);
        self.day_index = self.day_index.saturating_add_signed(step).min(last);
    }

    /// Left click while the popup is open: clicking an item chooses it; clicking the chosen item again
    /// checks it (like the agenda; the first click after opening only chooses, see `day_clicked`).
    /// A click outside the box closes the popup (like `Esc`); clicks on the border or empty space are ignored
    pub(super) fn click_day(&mut self, column: u16, row: u16) {
        if !ui::day_area(self).contains(Position::new(column, row)) {
            self.focus = Focus::Calendar;
            return;
        }
        match ui::day_item_at(self, column, row) {
            Some(index) if index == self.day_index && self.day_clicked => self.toggle_chosen(),
            Some(index) => {
                self.day_index = index;
                self.day_clicked = true;
            }
            None => {}
        }
    }

    /// After a write, chooses the item on file line `line` in the popup. If it is no longer on the selected
    /// day (e.g. its date changed), the choice stays at the same position
    pub(super) fn choose_day_line(&mut self, line: usize) {
        if let Some(index) = self
            .items_on(self.selected)
            .iter()
            .position(|item| item.line == line)
        {
            self.day_index = index;
        }
        self.keep_day_choice();
    }

    /// The day has fewer items (after a delete, an undo, or a date change): if the choice is now out of range,
    /// choose the last item; if no items are left, close the popup
    pub(super) fn keep_day_choice(&mut self) {
        match self.items_on(self.selected).len().checked_sub(1) {
            Some(last) => self.day_index = self.day_index.min(last),
            None => self.focus = Focus::Calendar,
        }
    }
}

#[cfg(test)]
mod tests;
