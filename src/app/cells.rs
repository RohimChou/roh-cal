//! Items chosen where the calendar itself lists them: the week view's boxes and the month grid's cells. `Tab`
//! from the calendar moves the keyboard in among the selected day's items (`Focus::Cell`), like layout 1's
//! agenda: `j` / `k` choose one by one and go on to the next / previous day that has items, and the keys on
//! an item (`handle_item_key`) work on the chosen one. Drawing is in `ui::week` and `ui::month`

use super::{App, ClickTarget, Focus};
use crate::tasks::Item;
use crate::ui;
use jiff::{ToSpan, civil::Date};
use ratatui::crossterm::event::KeyCode;
use std::time::Instant;

impl App {
    /// The selected day's items in the order its box / cell lists them (`cell_index` counts in it)
    pub fn cell_items(&self) -> Vec<&Item> {
        ui::cell_items(self, self.selected)
    }

    /// The nearest day after `from` (`step` = 1) or before it (-1) that has an item on screen (in a checked
    /// calendar, not hidden as done); `None` when there is none that way
    fn day_with_items(&self, from: Date, step: i64) -> Option<Date> {
        let next = from.saturating_add(step.days());
        let shown = self
            .items
            .iter()
            .filter(|item| self.is_shown(item))
            .filter_map(|item| Some((item.date?, item.end.or(item.date)?)));
        if step > 0 {
            shown
                .filter(|&(_, last)| last >= next)
                .map(|(first, _)| first.max(next))
                .min()
        } else {
            shown
                .filter(|&(first, _)| first <= next)
                .map(|(_, last)| last.min(next))
                .max()
        }
    }

    /// `Tab` from the calendar (week view, month grid): the keyboard goes to the selected day's first item;
    /// a day with none: the first item of the nearest later day that has one, else the last item of the
    /// nearest earlier one (the selected day follows); with no items at all, or cells too short to list
    /// items (`ui::cells_show_items`), it does not move. The month grid scrolled away with the wheel comes
    /// back to the selected day
    pub(super) fn enter_cells(&mut self) {
        if !ui::cells_show_items(self) {
            return;
        }
        self.month_free = None;
        if !self.cell_items().is_empty() {
            self.choose_cell(0);
        } else if let Some(day) = self.day_with_items(self.selected, 1) {
            self.selected = day;
            self.choose_cell(0);
        } else if let Some(day) = self.day_with_items(self.selected, -1) {
            self.selected = day;
            self.choose_cell(self.cell_items().len().saturating_sub(1));
        }
    }

    /// Chooses item `index` of the selected day's cell; the keyboard goes there
    fn choose_cell(&mut self, index: usize) {
        self.set_cell_index(index);
        self.focus = Focus::Cell;
    }

    /// Item `index` of the selected day's cell becomes the chosen one; its file line is remembered too
    /// (`cell_line`), since the month grid and the week view order a day's items differently
    fn set_cell_index(&mut self, index: usize) {
        self.cell_index = index;
        self.cell_line = self.cell_items().get(index).map(|item| item.line);
    }

    /// The layout changed under the chosen item (another layout, or the side menu put away after switching
    /// under it): the same item (by its file line) in this layout's order; no longer on the day: as
    /// `keep_cell_choice`
    pub(super) fn keep_cell_line(&mut self) {
        let index = self
            .cell_line
            .and_then(|line| self.cell_items().iter().position(|item| item.line == line));
        match index {
            Some(index) => self.set_cell_index(index),
            None => self.keep_cell_choice(),
        }
    }

    /// Keyboard on a cell's items: `j` / `k` the next / previous item (past the day's last one, the first item
    /// of the next day that has items; before its first, the last of the previous such day), `h` / `l` the
    /// first item of the previous / next day that has items; the rest are the keys on the chosen item
    pub(super) fn handle_cell_key(&mut self, code: KeyCode) {
        match code {
            KeyCode::Char('j') | KeyCode::Down => self.choose_next_cell(1),
            KeyCode::Char('k') | KeyCode::Up => self.choose_next_cell(-1),
            KeyCode::Char('l') | KeyCode::Right => self.choose_cell_day(1),
            KeyCode::Char('h') | KeyCode::Left => self.choose_cell_day(-1),
            _ => self.handle_item_key(code),
        }
    }

    /// The item `step` (1 or -1) further on, going on to the next / previous day that has items; at the very
    /// first / last item it stays (also the wheel)
    pub(super) fn choose_next_cell(&mut self, step: i64) {
        let count = self.cell_items().len();
        if step > 0 && self.cell_index + 1 < count {
            self.choose_cell(self.cell_index + 1);
        } else if step < 0 && self.cell_index > 0 && count > 0 {
            self.choose_cell((self.cell_index - 1).min(count - 1));
        } else if let Some(day) = self.day_with_items(self.selected, step) {
            self.selected = day;
            let last = self.cell_items().len().saturating_sub(1);
            self.choose_cell(if step > 0 { 0 } else { last });
        }
    }

    /// The first item of the previous (`step` = -1) / next (1) day that has items; none that way: stays
    fn choose_cell_day(&mut self, step: i64) {
        if let Some(day) = self.day_with_items(self.selected, step) {
            self.selected = day;
            self.choose_cell(0);
        }
    }

    /// A click on item `index` of `day`'s cell at time `now`: chooses it (the keyboard moves there); a double
    /// click checks it off like `Space` (an open task asks first). Clicking the chosen item again later only
    /// chooses it
    pub(super) fn click_cell_item(&mut self, day: Date, index: usize, now: Instant) {
        self.selected = day;
        self.choose_cell(index);
        if let Some(line) = self.chosen_item().map(|item| item.line)
            && self.is_double_click(ClickTarget::Item(line), now)
        {
            self.ask_check();
        }
    }

    /// After a write, chooses the item on file line `line`: on the selected day if it is there, else on its
    /// own (first) day, which becomes the selected one (e.g. rescheduled); gone: the choice stays at the
    /// same position (see `keep_cell_choice`)
    pub(super) fn choose_cell_line(&mut self, line: usize) {
        if let Some(index) = self.cell_items().iter().position(|item| item.line == line) {
            self.set_cell_index(index);
            return;
        }
        let day = self
            .items
            .iter()
            .find(|item| item.line == line && self.is_shown(item))
            .and_then(|item| item.date);
        if let Some(day) = day {
            self.selected = day;
            if let Some(index) = self.cell_items().iter().position(|item| item.line == line) {
                self.set_cell_index(index);
                return;
            }
        }
        self.keep_cell_choice();
    }

    /// The selected day has fewer items now (a delete, an undo, hidden once done, changed elsewhere): a choice
    /// past the end moves to the last item; with none left the keyboard goes back to the calendar
    pub(super) fn keep_cell_choice(&mut self) {
        match self.cell_items().len().checked_sub(1) {
            Some(last) => self.set_cell_index(self.cell_index.min(last)),
            None => self.focus = Focus::Calendar,
        }
    }
}

#[cfg(test)]
mod tests;
