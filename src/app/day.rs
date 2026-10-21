//! The day popup: `Enter` on the calendar (or double-clicking a day) opens it, in layouts 1 and 2. It lists all
//! items on the selected day (also ones a cell cannot fit and shows as `+N`). The keys are the same as in
//! layout 1's agenda (drawing is in `ui::day`)

use super::{App, ClickTarget, Focus};
use crate::tasks::Item;
use crate::ui;
use ratatui::{crossterm::event::KeyCode, layout::Position};
use std::time::Instant;

/// A spot in the popup's text: the `char`-th character of the title of the `item`-th item (`App::day_items`).
/// Sorting goes in reading order: by item, then by character
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct TextSpot {
    pub item: usize,
    pub char: usize,
}

/// Text being dragged over with the mouse in the popup: the character where the left button went down, and
/// once the mouse has moved with it held, the character it is over now
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TextDrag {
    pub anchor: TextSpot,
    pub head: Option<TextSpot>,
}

impl App {
    /// The popup's items in display order (`day_index` counts in it): `items_on(selected)`, and with
    /// `[day_popup] by_calendar` grouped by their outermost calendar, the calendars in the calendar list's
    /// order and items outside any calendar first; inside a group the order stays
    pub fn day_items(&self) -> Vec<&Item> {
        let mut items = self.items_on(self.selected);
        if self.settings.day_popup.by_calendar {
            items.sort_by_key(|item| {
                item.group.as_ref().map(|group| {
                    self.groups.iter().position(|calendar| {
                        calendar.parents.is_empty()
                            && calendar.name.as_deref() == Some(group.outermost())
                    })
                })
            });
        }
        items
    }

    /// Opens the popup with the first item chosen; does not open if the day has no items
    pub(super) fn open_day_popup(&mut self) {
        if self.items_on(self.selected).is_empty() {
            return;
        }
        self.day_index = 0;
        self.day_top = 0;
        self.day_drag = None;
        self.focus = Focus::Day;
    }

    /// The text dragged over in the popup, from its first to its last character (both included, in reading
    /// order); `None` until the mouse moves with the button held
    pub fn day_selection(&self) -> Option<(TextSpot, TextSpot)> {
        let drag = self.day_drag?;
        let head = drag.head?;
        Some((drag.anchor.min(head), drag.anchor.max(head)))
    }

    /// The dragged-over text (`day_selection`): each item's part of its title, one item per line (a title
    /// wrapped in the popup is still one line), e.g. 「Send the weekly update\nPack」
    pub fn day_selected_text(&self) -> Option<String> {
        let (first, last) = self.day_selection()?;
        let items = self.day_items();
        let lines: Vec<String> = (first.item..=last.item)
            .filter_map(|index| {
                let title = &items.get(index)?.title;
                let from = if index == first.item { first.char } else { 0 };
                let to = if index == last.item {
                    last.char + 1
                } else {
                    usize::MAX
                };
                Some(title.chars().take(to).skip(from).collect())
            })
            .collect();
        Some(lines.join("\n"))
    }

    /// The left button let go in the popup: the text dragged over (if any) goes to the clipboard (the main
    /// loop copies it, `App::copy_out`); it stays marked until a key or the next press
    pub(super) fn release_day(&mut self) {
        self.to_copy = self.day_selected_text();
    }

    /// The mouse moved with the left button held, now at (`column`, `row`): after a press on a title in the
    /// popup, the dragged-over text now ends at the character under it (past the box's edge: the nearest row
    /// inside; on a calendar's name or a blank row it stays where it was). A click right after a drag is not
    /// a double click
    pub(super) fn drag_day(&mut self, column: u16, row: u16) {
        let spot = ui::day_text_at(self, column, row, true);
        if let Some(drag) = &mut self.day_drag
            && let Some(spot) = spot
        {
            drag.head = Some(spot);
            self.last_click = None;
        }
    }

    /// Keyboard in the popup: `j` / `k` next / previous item, `c` grouped by calendar or not. Keys that
    /// write match the agenda: `Space` / `x` check, `r` reschedule, `e` / `Enter` edit,
    /// `a` add, `A` add a subtask, `d` delete (closing with `Esc` / `q` is in `handle_normal_key`)
    pub(super) fn handle_day_key(&mut self, code: KeyCode) {
        match code {
            KeyCode::Char('j') | KeyCode::Down => self.choose_day(1),
            KeyCode::Char('k') | KeyCode::Up => self.choose_day(-1),
            KeyCode::Char(' ' | 'x') => self.toggle_chosen(),
            KeyCode::Char('/') => self.toggle_chosen_progress(),
            KeyCode::Char('r') => self.start_moving(),
            KeyCode::Char('e') | KeyCode::Enter => self.start_editing(),
            KeyCode::Char('a') => self.start_adding(),
            KeyCode::Char('A') => self.start_adding_subtask(),
            KeyCode::Char('d') => self.ask_delete(),
            KeyCode::Char('c') => self.toggle_by_calendar(),
            _ => {}
        }
    }

    /// `c` or a click on the switch on the bottom border: `[day_popup] by_calendar` on ⇄ off, saved to the
    /// config file right away (like 「隱藏做完的」); the same item stays chosen
    fn toggle_by_calendar(&mut self) {
        let line = self.chosen_item().map(|item| item.line);
        let before = self.settings.clone();
        self.settings.day_popup.by_calendar = !before.day_popup.by_calendar;
        self.save_switch(&before);
        if let Some(line) = line {
            self.choose_day_line(line);
        }
    }

    /// Chooses the item `step` items further down in the popup (negative = up); stops at the first / last item
    pub(super) fn choose_day(&mut self, step: isize) {
        let last = self.day_items().len().saturating_sub(1);
        self.day_index = self.day_index.saturating_add_signed(step).min(last);
    }

    /// Left click at time `now` while the popup is open: clicking an item chooses it (and a drag may start
    /// there, `drag_day`); a double click checks it (like the agenda); clicking the switch on the bottom
    /// border flips it (like `c`).
    /// A click outside the box closes the popup (like `Esc`); clicks on the border or empty space are ignored
    pub(super) fn click_day(&mut self, column: u16, row: u16, now: Instant) {
        // A press on a title starts dragging over text from the character under it; anywhere else, no text
        self.day_drag =
            ui::day_text_at(self, column, row, false).map(|anchor| TextDrag { anchor, head: None });
        if !ui::day_area(self).contains(Position::new(column, row)) {
            self.focus = Focus::Calendar;
            return;
        }
        if ui::day_switch_area(self).contains(Position::new(column, row)) {
            self.toggle_by_calendar();
            return;
        }
        let Some(index) = ui::day_item_at(self, column, row) else {
            return;
        };
        self.day_index = index;
        if let Some(line) = self.chosen_item().map(|item| item.line)
            && self.is_double_click(ClickTarget::Item(line), now)
        {
            self.toggle_chosen();
        }
    }

    /// After a write, chooses the item on file line `line` in the popup. If it is no longer on the selected
    /// day (e.g. its date changed), the choice stays at the same position
    pub(super) fn choose_day_line(&mut self, line: usize) {
        if let Some(index) = self.day_items().iter().position(|item| item.line == line) {
            self.day_index = index;
        }
        self.keep_day_choice();
    }

    /// The day has fewer items (after a delete, an undo, or a date change): if the choice is now out of range,
    /// choose the last item; if no items are left, close the popup
    pub(super) fn keep_day_choice(&mut self) {
        match self.day_items().len().checked_sub(1) {
            Some(last) => self.day_index = self.day_index.min(last),
            None => self.focus = Focus::Calendar,
        }
    }
}

#[cfg(test)]
mod tests;
