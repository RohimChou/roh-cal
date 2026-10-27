//! Writing the task file: check off, delete, undo. Always re-read before
//! writing; if the line was changed elsewhere, don't write and reload instead.
//! Changes made elsewhere while nothing is being written are picked up too (`refresh_tasks`)

use super::{App, Focus, Mode, SideRow, View};
use crate::edit;
use crate::files;
use crate::tasks::{self, Item, Kind, Parsed};
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use std::io::ErrorKind;
use std::path::{Path, PathBuf};

/// The most undo steps kept
const UNDO_LEVELS: usize = 50;

/// A notice that temporarily replaces the bottom row (the result of a write); gone on the next key or click
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Notice {
    /// The task file was changed elsewhere (e.g. just edited in Obsidian): not written, reloaded
    Reloaded,
    /// Reading / writing the task file failed (e.g. no permission, the file was deleted): not written
    Failed(ErrorKind),
    /// Tried to check off an event (no box): events cannot be checked off
    NotATask,
    /// The task file has some mistakes (e.g. `@2026-13-01`); said as soon as it opens
    Mistakes(usize),
    /// Undo worked; carries the title of the item that was restored, e.g. `Team sync`
    Undone(String),
    /// Nothing to undo (no writes yet, or already undone all the way back)
    NothingToUndo,
    /// The settings page's switches could not be saved to the config
    /// file (e.g. no permission, the config file was broken elsewhere)
    SettingsFailed(ErrorKind),
    /// `o` could not start the editor (e.g. `$EDITOR` names one that is not installed); carries its name
    NoEditor(String),
}

/// The record of one write (for undo): the whole file before and after
/// the write, and the title of the item it changed (for the notice)
#[derive(Debug)]
pub(super) struct UndoStep {
    /// The whole file before roh-cal wrote it (undo writes this back)
    before: String,
    /// The whole file roh-cal wrote (undo only happens if the file
    /// still matches it exactly; otherwise it was changed elsewhere)
    after: String,
    /// The title of the item it changed (`Undone: Team sync`)
    title: String,
    /// For a move up / down (`J` / `K`), which two blocks were swapped: line numbers
    /// the screen remembers follow it (reversed on undo); `None` for other writes
    swap: Option<edit::Swap>,
}

impl App {
    /// Checks off / unchecks the chosen item in the agenda (or the tasks page) and
    /// writes back to the task file; the result (failed, reloaded) goes in `notice`
    ///
    /// Does nothing without a task file (tests) or without a chosen item
    pub(super) fn toggle_chosen(&mut self) {
        self.change_chosen_box(edit::toggle);
    }

    /// `/`: marks the chosen task in progress (`[/]`), or back to open if it already is; written back like
    /// `toggle_chosen`
    pub(super) fn toggle_chosen_progress(&mut self) {
        self.change_chosen_box(edit::toggle_progress);
    }

    /// Changes the chosen task's box with `change` (`edit::toggle` or `edit::toggle_progress`) and writes
    /// back; an event has no box (says so in `notice`); the line changed elsewhere: reload instead
    fn change_chosen_box(&mut self, change: fn(&str, &Item) -> Result<String, edit::Changed>) {
        let Some(item) = self.chosen_item().cloned() else {
            return;
        };
        if item.kind == Kind::Event {
            self.notice = Some(Notice::NotATask);
            return;
        }
        let Some((path, text)) = self.read_tasks() else {
            return;
        };
        let line = item.line;
        match change(&text, &item) {
            Ok(after) => {
                if self.write_tasks(&path, text, after, item.title) {
                    self.keep_chosen_line(line);
                }
            }
            Err(edit::Changed) => self.reload(&text),
        }
    }

    /// `d`: first counts how many items deleting the chosen one would remove
    /// (including subtasks), then asks the user in the bottom row (only `d` again or `y` deletes)
    pub(super) fn ask_delete(&mut self) {
        let Some(item) = self.chosen_item().cloned() else {
            return;
        };
        let Some((_, text)) = self.read_tasks() else {
            return;
        };
        match edit::delete(&text, &item) {
            Ok((_, count)) => self.mode = Mode::ConfirmDelete(item, count),
            Err(edit::Changed) => self.reload(&text),
        }
    }

    /// Keys while asking "delete?": `d` (so `dd` deletes, like vim) or `y` deletes (with subtasks), `Ctrl+C`
    /// quits, any other key just cancels
    ///
    /// If the count from when it asked differs from the count now (e.g. a
    /// subtask was just added elsewhere), it does not delete and reloads instead
    pub(super) fn answer_delete(&mut self, key: KeyEvent) {
        let (item, count) = match std::mem::take(&mut self.mode) {
            Mode::ConfirmDelete(item, count) => (item, count),
            // Not asking: do nothing (put it back)
            other => {
                self.mode = other;
                return;
            }
        };
        if key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL) {
            self.should_quit = true;
            return;
        }
        if !matches!(key.code, KeyCode::Char('d' | 'y')) {
            return;
        }
        // Deleting from the day popup (either layout): writing adjusts or closes the popup itself
        let in_popup = self.focus == Focus::Day;
        // Tasks page: which row was chosen before deleting (choose that position after deleting)
        let row = if self.view == View::Tasks {
            self.tasks_cursor(&self.tasks_rows()).unwrap_or(0)
        } else {
            0
        };
        let Some((path, text)) = self.read_tasks() else {
            return;
        };
        match edit::delete(&text, &item) {
            Ok((after, deleted)) if deleted == count => {
                self.write_tasks(&path, text, after, item.title);
                if self.view == View::Tasks {
                    // Tasks page: choose the same row (now the next item);
                    // if the last one was deleted, choose the last item
                    self.choose_task_row_near(row);
                    return;
                }
                // The day popup: set_items already adjusted the chosen
                // position (it says 「沒有事」 if that day has no items left)
                if in_popup {
                    return;
                }
                // The agenda chooses the next item (the same
                // position); if none are left, go back to the calendar
                match self.agenda().len().checked_sub(1) {
                    Some(last) => self.choose(self.agenda_index.min(last)),
                    None => self.focus = Focus::Calendar,
                }
            }
            // Changed elsewhere: reload; the keyboard goes back to this
            // layout's main box (the tasks page stays the tasks page)
            _ => {
                self.reload(&text);
                self.focus = self.home_focus();
            }
        }
    }

    /// `d` in the calendar list: first counts how many items deleting the chosen calendar (with the calendars
    /// inside it) would remove, then
    /// asks in the bottom row (only `d` again or `y` deletes); 「Ungrouped」 has no heading, so nothing happens
    pub(super) fn ask_delete_calendar(&mut self) {
        let headings = self.chosen_calendar_headings();
        let Some(name) = headings.first().map(|line| line.heading.name.clone()) else {
            return;
        };
        let Some((_, text)) = self.read_tasks() else {
            return;
        };
        match edit::delete_calendar(&text, &headings) {
            Ok((_, count)) => {
                self.mode = Mode::ConfirmDeleteCalendar {
                    name,
                    headings,
                    count,
                };
            }
            Err(edit::Changed) => self.reload(&text),
        }
    }

    /// Keys while asking "delete this calendar?": `d` or `y` deletes it (its headings and everything under them),
    /// `Ctrl+C` quits, any other key just cancels. If the count from when it asked differs from the count
    /// now (e.g. an item was just added to it elsewhere), it does not delete and reloads instead
    pub(super) fn answer_delete_calendar(&mut self, key: KeyEvent) {
        let (name, headings, count) = match std::mem::take(&mut self.mode) {
            Mode::ConfirmDeleteCalendar {
                name,
                headings,
                count,
            } => (name, headings, count),
            // Not asking: do nothing (put it back)
            other => {
                self.mode = other;
                return;
            }
        };
        if key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL) {
            self.should_quit = true;
            return;
        }
        if !matches!(key.code, KeyCode::Char('d' | 'y')) {
            return;
        }
        let Some((path, text)) = self.read_tasks() else {
            return;
        };
        match edit::delete_calendar(&text, &headings) {
            Ok((after, deleted)) if deleted == count => {
                self.write_tasks(&path, text, after, name);
            }
            _ => self.reload(&text),
        }
    }

    /// `z` in the calendar list: archives the chosen calendar (` archived:<today>` in its headings), or brings an
    /// archived one back: `archived:<date>` comes out of the headings of the outermost archived calendar it is
    /// in or is (the one with the tag; a calendar archived together with the one it is inside comes back with
    /// it). It stays chosen, now in the other part of the list; `u` undoes it. Nothing happens for
    /// 「Ungrouped」
    pub(super) fn toggle_archived(&mut self) {
        let Some(chosen) = self.chosen_group() else {
            return;
        };
        let path = chosen.path();
        let target = if chosen.archived {
            (1..=path.len()).find_map(|depth| {
                self.groups
                    .iter()
                    .find(|group| group.archived && group.path() == path[..depth])
            })
        } else {
            Some(chosen)
        };
        let Some(target) = target else {
            return;
        };
        let day = (!target.archived).then_some(self.today);
        let headings: Vec<tasks::HeadingLine> = self
            .headings
            .iter()
            .filter(|line| target.is(&line.heading))
            .cloned()
            .collect();
        let Some(name) = headings.first().map(|line| line.heading.name.clone()) else {
            return;
        };
        let Some((path, text)) = self.read_tasks() else {
            return;
        };
        match edit::set_archived(&text, &headings, day) {
            Ok(after) => {
                self.write_tasks(&path, text, after, name);
            }
            Err(edit::Changed) => self.reload(&text),
        }
    }

    /// `L` / `H` in the calendar list: moves the chosen calendar one level in (inside the calendar right
    /// above it at the same level) / out, with the calendars inside it; it stays chosen and they keep their
    /// checked state. Nothing happens when it cannot move (see `edit::shift_calendar`) or for 「Ungrouped」
    pub(super) fn shift_calendar(&mut self, deeper: bool) {
        let headings = self.chosen_calendar_headings();
        let Some(first) = headings.first().cloned() else {
            return;
        };
        let Some((path, text)) = self.read_tasks() else {
            return;
        };
        let checked = self.checked_states();
        match edit::shift_calendar(&text, &headings, deeper) {
            Ok(Some(after)) => {
                if !self.write_tasks(&path, text, after, first.heading.name.clone()) {
                    return;
                }
                // Only `#`s changed, so its first heading is on the same line, with its new path
                let path_of = |heading: &tasks::Heading| -> Vec<String> {
                    heading
                        .parents
                        .iter()
                        .chain([&heading.name])
                        .cloned()
                        .collect()
                };
                let Some(moved) = self.headings.iter().find(|line| line.line == first.line) else {
                    return;
                };
                let new = path_of(&moved.heading);
                self.carry_checked(&checked, &path_of(&first.heading), &new);
            }
            Ok(None) => {}
            Err(edit::Changed) => self.reload(&text),
        }
    }

    /// `u`: undoes roh-cal's last write; the result goes in `notice`
    ///
    /// Undoes only if the file matches exactly what roh-cal last wrote; if it was changed elsewhere,
    /// the file is left alone and reloaded instead, so other people's changes are not overwritten
    pub(super) fn undo(&mut self) {
        let Some(path) = self.tasks_path.clone() else {
            self.notice = Some(Notice::NothingToUndo);
            return;
        };
        let Some(step) = self.undo.pop() else {
            self.notice = Some(Notice::NothingToUndo);
            return;
        };
        let current = match std::fs::read_to_string(&path) {
            Ok(text) => text,
            Err(error) => {
                // Undo failed: put this step back so it can be tried again later
                self.undo.push(step);
                self.notice = Some(Notice::Failed(error.kind()));
                return;
            }
        };
        if current != step.after {
            self.reload(&current);
            return;
        }
        if let Err(error) = files::write_atomically(&path, &step.before) {
            self.undo.push(step);
            self.notice = Some(Notice::Failed(error.kind()));
            return;
        }
        match step.swap {
            // A move up / down: move it back the other way
            Some(swap) => self.remap_lines(|line| Some(swap.reversed().map(line))),
            None => self.remap_lines(edit::line_map(&step.after, &step.before)),
        }
        self.show_tasks(step.before);
        self.notice = Some(Notice::Undone(step.title));
    }

    /// The task file's path and its current content (always re-read before writing); `None`
    /// without a task file (tests); if it cannot be read, records a notice and is also `None`
    pub(super) fn read_tasks(&mut self) -> Option<(PathBuf, String)> {
        let path = self.tasks_path.clone()?;
        match std::fs::read_to_string(&path) {
            Ok(text) => Some((path, text)),
            Err(error) => {
                self.notice = Some(Notice::Failed(error.kind()));
                None
            }
        }
    }

    /// For adding: the task file's path and its current content; if the file does not exist
    /// yet (e.g. the default location's first use), it counts as empty and is created on save
    pub(super) fn read_tasks_for_adding(&mut self) -> Option<(PathBuf, String)> {
        let path = self.tasks_path.clone()?;
        match std::fs::read_to_string(&path) {
            Ok(text) => Some((path, text)),
            Err(error) if error.kind() == ErrorKind::NotFound => Some((path, String::new())),
            Err(error) => {
                self.notice = Some(Notice::Failed(error.kind()));
                None
            }
        }
    }

    /// Writes `after` to the task file, updates the items on screen and records an undo
    /// step (`before` = the content before the write, `title` = the item it changed);
    /// on failure, records a notice (file and screen untouched). Returns whether it was written
    pub(super) fn write_tasks(
        &mut self,
        path: &Path,
        before: String,
        after: String,
        title: String,
    ) -> bool {
        self.write_tasks_swapped(path, before, after, title, None)
    }

    /// Same as `write_tasks`; for a move up / down (`swap` has a value), line numbers
    /// the screen remembers follow the two swapped blocks instead of `edit::line_map`
    pub(super) fn write_tasks_swapped(
        &mut self,
        path: &Path,
        before: String,
        after: String,
        title: String,
        swap: Option<edit::Swap>,
    ) -> bool {
        if let Err(error) = files::write_atomically(path, &after) {
            self.notice = Some(Notice::Failed(error.kind()));
            return false;
        }
        // Line numbers the screen remembers (the chosen item, expanded subtasks) move along
        match swap {
            Some(swap) => self.remap_lines(|line| Some(swap.map(line))),
            None => self.remap_lines(edit::line_map(&before, &after)),
        }
        self.show_tasks(after.clone());
        self.undo.push(UndoStep {
            before,
            after,
            title,
            swap,
        });
        // Keep only the last UNDO_LEVELS steps
        if self.undo.len() > UNDO_LEVELS {
            self.undo.remove(0);
        }
        true
    }

    /// The task file was changed elsewhere when roh-cal was about to write: reload from the file's
    /// current content `text` and tell the user this was not written
    pub(super) fn reload(&mut self, text: &str) {
        self.take_change_from_elsewhere(text.to_string());
        self.notice = Some(Notice::Reloaded);
    }

    /// Picks up changes made to the task file elsewhere (e.g. saved in Obsidian, or by a script) while
    /// roh-cal is open; the main loop calls it every round. Quiet: no notice, the screen just updates.
    ///
    /// Reads the file only when its modified time changed, and reloads only when the content differs from
    /// what the screen shows (roh-cal's own writes change the time but not that). Waits while an item is
    /// being worked on (editor, rescheduling, delete question), so it does not change underneath; that
    /// write re-reads the file anyway. A file that cannot be read (e.g. deleted) leaves the screen as it is
    pub(super) fn refresh_tasks(&mut self) {
        let busy = match self.mode {
            Mode::Normal | Mode::Help(_) | Mode::Settings { .. } | Mode::Splash { .. } => false,
            Mode::Editor(_)
            | Mode::Moving(_)
            | Mode::ConfirmDelete(..)
            | Mode::ConfirmDeleteCalendar { .. } => true,
        };
        if busy {
            return;
        }
        let Some(path) = &self.tasks_path else {
            return;
        };
        let modified = std::fs::metadata(path)
            .and_then(|meta| meta.modified())
            .ok();
        if modified == self.tasks_modified {
            return;
        }
        // Not readable right now: keep the old time, so the next round tries again
        let Ok(text) = std::fs::read_to_string(path) else {
            return;
        };
        self.tasks_modified = modified;
        if text != self.tasks_text {
            self.take_change_from_elsewhere(text);
        }
    }

    /// Shows `text`, the task file as changed elsewhere: line numbers the screen remembers (the chosen
    /// item, expanded subtasks) move along with `edit::line_map`, and the item chosen in the agenda or
    /// layout 2's popup stays chosen even if its day's order changed (e.g. marked `[/]` elsewhere); the
    /// earlier undo steps no longer match the file, so they are cleared
    fn take_change_from_elsewhere(&mut self, text: String) {
        let map = edit::line_map(&self.tasks_text, &text);
        // Where the chosen item's line is in the new file (`None` = its line was removed or replaced)
        let chosen = self.chosen_item().and_then(|item| map(item.line));
        self.remap_lines(&map);
        self.show_tasks(text);
        self.undo.clear();
        if let Some(line) = chosen {
            self.keep_chosen_line(line);
        }
    }

    /// Shows the task file's content `text` (at start, after a write, an undo or a reload) and remembers
    /// it, so `refresh_tasks` can tell changes made elsewhere
    pub(crate) fn show_tasks(&mut self, text: String) {
        self.set_items(tasks::parse(&text));
        self.tasks_text = text;
    }

    /// Maps the line numbers the screen remembers (the item chosen on the tasks page, expanded
    /// subtasks) to line numbers after the write with `map`; ones with no match (deleted) are forgotten
    fn remap_lines(&mut self, map: impl Fn(usize) -> Option<usize>) {
        self.expanded = self.expanded.iter().filter_map(|&line| map(line)).collect();
        self.tasks_line = self.tasks_line.and_then(&map);
    }

    /// Swaps in a new set of items (at start, after a write, a reload or an undo); the side menu re-lists
    /// the calendars from the new file, and calendars that existed before keep their checked state.
    /// If the agenda has fewer items (e.g. the item just added was undone), the keyboard
    /// is in the agenda and the chosen position is out of range: choose the last item;
    /// if no items are left, go back to the calendar;
    /// the day popup keeps its choice in range too, and stays open with none left (see `keep_day_choice`)
    pub(crate) fn set_items(
        &mut self,
        Parsed {
            items,
            warnings,
            headings,
        }: Parsed,
    ) {
        // The popup's items may have changed: text dragged over no longer points at the same characters
        self.day_drag = None;
        let mut groups = super::Group::list(&items, &headings);
        // Each calendar keeps its check; one archived or brought back since starts over (unchecked / checked)
        for group in &mut groups {
            if let Some(old) = self
                .groups
                .iter()
                .find(|old| old.same(group) && old.archived == group.archived)
            {
                group.shown = old.shown;
            }
        }
        // The chosen calendar found again by its path (calendars added or removed above it, e.g. in another
        // editor, move it to another index)
        let moved_to = self
            .chosen_group()
            .and_then(|chosen| groups.iter().position(|group| group.same(chosen)));
        self.groups = groups;
        // A switch row stays chosen; a calendar stays chosen wherever it is now; when it is gone, the one in
        // its place (the last one when past the end)
        if let SideRow::Calendar(index) = self.side_row {
            let index = moved_to.unwrap_or(index.min(self.groups.len().saturating_sub(1)));
            self.side_row = SideRow::Calendar(index);
        }
        self.items = items;
        self.warnings = warnings;
        self.headings = headings;
        self.keep_choice_in_range();
    }
}

#[cfg(test)]
pub(crate) mod tests;
