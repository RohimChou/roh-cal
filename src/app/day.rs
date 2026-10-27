//! The day popup: `Enter` on the calendar (or double-clicking a day) opens it, in layouts 1 and 2. It lists all
//! items on the selected day (also ones a cell cannot fit and shows as `+N`), each subtask under its parent.
//! The keys are the same as in layout 1's agenda (drawing is in `ui::day`)

use super::{App, ClickTarget, Focus};
use crate::tasks::Item;
use crate::ui;
use ratatui::{crossterm::event::KeyCode, layout::Position};
use std::collections::{HashMap, HashSet};
use std::time::Instant;

/// One row of the popup's list (`App::day_entries`): an item on the selected day, or a parent of one that is
/// not on that day itself (drawn gray, like a heading; it cannot be chosen)
#[derive(Debug, Clone, Copy)]
pub struct DayEntry<'a> {
    pub item: &'a Item,
    /// Whether the item is on the selected day; `false` = only listed so its subtasks show whose they are
    pub on_day: bool,
}

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
    /// The popup's items in display order (`day_index` counts in it): `day_entries` without the parents that
    /// are not on the day
    pub fn day_items(&self) -> Vec<&Item> {
        self.day_entries()
            .into_iter()
            .filter(|entry| entry.on_day)
            .map(|entry| entry.item)
            .collect()
    }

    /// The popup's rows in display order: the items on the selected day (`items_on`) as a tree, each subtask
    /// under its parent. A parent not on that day itself (another day, no date, or hidden as done) is listed
    /// too, so its subtasks show whose they are; e.g. on 10/26:
    /// 「Plan the trip」 (no date) → 「Book flights」 (10/26) → 「Compare prices」 (10/26).
    /// Items under the same parent (or at the outermost level) keep `items_on`'s order, a whole family taking
    /// the place of its first item there (a parent with an in-progress subtask comes first with it). With
    /// `[day_popup] by_calendar` the families are grouped by their calendar, the calendars in the calendar
    /// list's order (each right before the calendars inside it, as on the tasks page) and items outside any
    /// calendar first; inside a group the order stays
    pub fn day_entries(&self) -> Vec<DayEntry<'_>> {
        // Each item on the day with the parents above it, outermost first
        let mut chains: Vec<Vec<&Item>> = self
            .items_on(self.selected)
            .into_iter()
            .map(|item| self.with_parents(item))
            .collect();
        // Where each item (by its file line) first shows up in `items_on`'s order, as itself or as a parent
        let mut first = HashMap::new();
        for (place, chain) in chains.iter().enumerate() {
            for item in chain {
                first.entry(item.line).or_insert(place);
            }
        }
        // Families together: a parent's chain is the start of its subtasks' chains, so it sorts before them
        chains.sort_by_cached_key(|chain| {
            chain
                .iter()
                .map(|item| first.get(&item.line).copied())
                .collect::<Vec<_>>()
        });
        if self.settings.day_popup.by_calendar {
            // A family is all in one calendar: the item on the day stands for it
            chains.sort_by_key(|chain| {
                chain
                    .last()
                    .and_then(|item| item.group.as_ref())
                    .map(|heading| self.groups.iter().position(|group| group.is(heading)))
            });
        }
        // Each item once: a parent comes right before its first subtask (or on its own, when on the day)
        let mut listed = HashSet::new();
        let mut entries = Vec::new();
        for chain in &chains {
            for (level, item) in chain.iter().enumerate() {
                if listed.insert(item.line) {
                    entries.push(DayEntry {
                        item,
                        on_day: level + 1 == chain.len(),
                    });
                }
            }
        }
        entries
    }

    /// `item` and its parent, its parent's parent, …, the outermost first
    fn with_parents<'a>(&'a self, item: &'a Item) -> Vec<&'a Item> {
        let mut chain = vec![item];
        let mut parent = item.parent;
        while let Some(index) = parent {
            let Some(above) = self.items.get(index) else {
                break;
            };
            chain.push(above);
            parent = above.parent;
        }
        chain.reverse();
        chain
    }

    /// Opens the popup with the first item chosen (a day with no items too: it says 「沒有事」)
    pub(super) fn open_day_popup(&mut self) {
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
    /// inside; on a heading or a blank row it stays where it was). A click right after a drag is not
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

    /// Keyboard in the popup: `j` / `k` next / previous item, `h` / `l` previous / next day, `c` grouped by
    /// calendar or not; the rest are
    /// the keys on the chosen item, as in the agenda (`App::handle_item_key`; closing with `Esc` / `q` is in
    /// `handle_normal_key`)
    pub(super) fn handle_day_key(&mut self, code: KeyCode) {
        match code {
            KeyCode::Char('j') | KeyCode::Down => self.choose_day(1),
            KeyCode::Char('k') | KeyCode::Up => self.choose_day(-1),
            KeyCode::Char('h') | KeyCode::Left => self.step_day(-1),
            KeyCode::Char('l') | KeyCode::Right => self.step_day(1),
            KeyCode::Char('c') => self.toggle_by_calendar(),
            _ => self.handle_item_key(code),
        }
    }

    /// `h` / `l` in the popup: the popup shows the day `days` later (negative = earlier) with its first item
    /// chosen, and stays open (a day with no items too); the calendar's selected day moves with it
    fn step_day(&mut self, days: i64) {
        self.move_by(days);
        self.open_day_popup();
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
    /// choose the last item; with no items left the popup stays open, saying 「沒有事」
    pub(super) fn keep_day_choice(&mut self) {
        let last = self.day_items().len().saturating_sub(1);
        self.day_index = self.day_index.min(last);
    }
}

#[cfg(test)]
mod tests;
