//! Writing the task file: check off, delete, undo. Always re-read before
//! writing; if the line was changed elsewhere, don't write and reload instead

use super::{App, Focus, Mode, View};
use crate::edit;
use crate::files;
use crate::tasks::{self, Kind, Parsed};
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
        match edit::toggle(&text, &item) {
            Ok(after) => {
                self.write_tasks(&path, text, after, item.title);
            }
            Err(edit::Changed) => self.reload(&text),
        }
    }

    /// `d`: first counts how many items deleting the chosen one would remove
    /// (including subtasks), then asks the user in the bottom row (only `y` deletes)
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

    /// Keys while asking "delete?": `y` deletes (with subtasks), `Ctrl+C` quits, any other key just cancels
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
        if key.code != KeyCode::Char('y') {
            return;
        }
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
                // Layout 3's popup: set_items already adjusted the chosen
                // position (closes it if that day has no items left)
                if self.view == View::Month {
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
        self.set_items(tasks::parse(&step.before));
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
        self.set_items(tasks::parse(&after));
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

    /// The task file was changed elsewhere: reload from the file's current content `text` and
    /// tell the user this was not written; the earlier undo steps no longer match, so clear them
    pub(super) fn reload(&mut self, text: &str) {
        self.set_items(tasks::parse(text));
        self.undo.clear();
        self.notice = Some(Notice::Reloaded);
    }

    /// Maps the line numbers the screen remembers (the item chosen on the tasks page, expanded
    /// subtasks) to line numbers after the write with `map`; ones with no match (deleted) are forgotten
    fn remap_lines(&mut self, map: impl Fn(usize) -> Option<usize>) {
        self.expanded = self.expanded.iter().filter_map(|&line| map(line)).collect();
        self.tasks_line = self.tasks_line.and_then(&map);
    }

    /// Swaps in a new set of items (after a write, a reload or an undo); the side menu re-lists the
    /// calendars from the new file, and calendars that existed before keep their checked state.
    /// If the agenda has fewer items (e.g. the item just added was undone), the keyboard
    /// is in the agenda and the chosen position is out of range: choose the last item;
    /// if no items are left, go back to the calendar;
    /// layout 3's popup works the same way (see `keep_day_choice`)
    fn set_items(&mut self, Parsed { items, warnings }: Parsed) {
        let mut groups = super::Group::list(&items);
        for group in &mut groups {
            if let Some(old) = self.groups.iter().find(|old| old.name == group.name) {
                group.shown = old.shown;
            }
        }
        self.groups = groups;
        self.group_index = self.group_index.min(self.groups.len().saturating_sub(1));
        self.items = items;
        self.warnings = warnings;
        if self.focus == Focus::Day {
            self.keep_day_choice();
        }
        if self.focus == Focus::Agenda && self.agenda_index >= self.agenda().len() {
            match self.agenda().len().checked_sub(1) {
                Some(last) => self.choose(last),
                None => self.focus = Focus::Calendar,
            }
        }
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::texts::Lang;
    use jiff::civil::date;
    use ratatui::crossterm::event::{
        KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
    };

    /// A task file in a temp folder (content `text`) and an app that reads
    /// it (today 2026/10/17, keyboard in the agenda, first item chosen);
    /// returns the app and the file's path. Never touches the real task file;
    /// delete `path`'s folder when done
    pub(crate) fn app_with_task_file(name: &str, text: &str) -> (App, PathBuf) {
        let dir = files::test_dir(name);
        std::fs::create_dir_all(&dir).expect("dir");
        let path = dir.join("tasks.md");
        std::fs::write(&path, text).expect("write");
        let mut app = App::new(Lang::ZhTw, date(2026, 10, 17), tasks::parse(text).items);
        app.tasks_path = Some(path.clone());
        // Choose the first item, like pressing Tab: the keyboard moves
        // to the agenda, and the calendar selects that item's day
        app.choose(0);
        (app, path)
    }

    /// Presses one key
    pub(crate) fn press(app: &mut App, code: KeyCode) {
        app.handle_key(KeyEvent::from(code));
    }

    /// Deletes the test's temp folder
    pub(crate) fn clean_up(path: &Path) {
        std::fs::remove_dir_all(path.parent().expect("dir")).expect("clean up");
    }

    #[test]
    fn space_or_x_checks_the_chosen_item_off_in_the_file() {
        let text = "# Recurring\n- [ ] Review PRs @2026-10-17 09:00\n- [ ] Deploy to staging @2026-10-17 14:00\n";
        let (mut app, path) = app_with_task_file("toggle", text);
        let read = || std::fs::read_to_string(&path).expect("read");
        // Choose the second item ("Deploy to staging") and press Space: only that line changes
        press(&mut app, KeyCode::Char('j'));
        press(&mut app, KeyCode::Char(' '));
        assert_eq!(
            read(),
            "# Recurring\n- [ ] Review PRs @2026-10-17 09:00\n- [x] Deploy to staging @2026-10-17 14:00\n"
        );
        // The items on screen update too; the same item is still chosen, no notice
        assert!(app.items[1].done);
        assert_eq!(app.agenda_index, 1);
        assert_eq!(app.notice, None);
        // x works too: press it again to uncheck
        press(&mut app, KeyCode::Char('x'));
        assert_eq!(read(), text);
        assert!(!app.items[1].done);
        clean_up(&path);
    }

    #[test]
    fn an_event_cannot_be_checked_off() {
        let text = "- Dentist appointment @2026-10-17 10:30\n";
        let (mut app, path) = app_with_task_file("event", text);
        press(&mut app, KeyCode::Char(' '));
        assert_eq!(std::fs::read_to_string(&path).expect("read"), text);
        assert_eq!(app.notice, Some(Notice::NotATask));
        // Next key: the notice disappears
        press(&mut app, KeyCode::Char('j'));
        assert_eq!(app.notice, None);
        clean_up(&path);
    }

    #[test]
    fn toggle_reloads_instead_of_writing_when_the_file_changed_elsewhere() {
        let (mut app, path) = app_with_task_file("changed", "- [ ] Review PRs @2026-10-17 09:00\n");
        // A line was added above, elsewhere (e.g. Obsidian)
        let edited = "- [ ] Go to the gym @2026-10-17 08:00\n- [ ] Review PRs @2026-10-17 09:00\n";
        std::fs::write(&path, edited).expect("edit");
        press(&mut app, KeyCode::Char(' '));
        // File untouched, reloaded (now two items), notice shown
        assert_eq!(std::fs::read_to_string(&path).expect("read"), edited);
        assert_eq!(app.items.len(), 2);
        assert_eq!(app.notice, Some(Notice::Reloaded));
        clean_up(&path);
    }

    #[test]
    fn toggle_says_why_when_the_file_is_gone() {
        let (mut app, path) = app_with_task_file("gone", "- [ ] Review PRs @2026-10-17 09:00\n");
        std::fs::remove_file(&path).expect("remove");
        press(&mut app, KeyCode::Char(' '));
        assert_eq!(app.notice, Some(Notice::Failed(ErrorKind::NotFound)));
        // No new file was quietly created
        assert!(!path.exists());
        clean_up(&path);
    }

    #[test]
    fn toggle_without_a_task_file_does_nothing() {
        let mut app = App::new(
            Lang::ZhTw,
            date(2026, 10, 17),
            tasks::parse("- [ ] Review PRs @2026-10-17\n").items,
        );
        app.focus = Focus::Agenda;
        press(&mut app, KeyCode::Char(' '));
        assert!(!app.items[0].done);
        assert_eq!(app.notice, None);
    }

    #[test]
    fn a_notice_from_a_click_stays_after_the_button_is_released() {
        // 70 × 23, October at the top: the agenda's first row (y 3) is the event on 10/10;
        // a double click = trying to check it off, and events cannot be checked off
        let text = "- Team sync @2026-10-10\n";
        let (mut app, path) = app_with_task_file("click-notice", text);
        app.focus = Focus::Calendar;
        app.screen = ratatui::layout::Rect::new(0, 0, 70, 23);
        app.calendar_top = Some(0);
        let mouse = |kind, column| MouseEvent {
            kind,
            column,
            row: 3,
            modifiers: KeyModifiers::NONE,
        };
        let down = MouseEventKind::Down(MouseButton::Left);
        let up = MouseEventKind::Up(MouseButton::Left);
        for _ in 0..2 {
            app.handle_mouse(mouse(down, 40));
            app.handle_mouse(mouse(up, 40));
        }
        // The release does not clear the notice; neither does moving or dragging
        assert_eq!(app.notice, Some(Notice::NotATask));
        app.handle_mouse(mouse(MouseEventKind::Moved, 50));
        app.handle_mouse(mouse(MouseEventKind::Drag(MouseButton::Left), 50));
        assert_eq!(app.notice, Some(Notice::NotATask));
        // The next press: it has been seen, so clear it
        app.handle_mouse(mouse(down, 5));
        assert_eq!(app.notice, None);
        clean_up(&path);
    }

    #[test]
    fn clicking_the_chosen_item_again_checks_it_off() {
        // 70 × 23, October at the top: the agenda's first row (y 3, from x 36) is 10/10's "Review PRs"
        let text = "- [ ] Review PRs @2026-10-10\n";
        let (mut app, path) = app_with_task_file("click", text);
        app.focus = Focus::Calendar;
        app.screen = ratatui::layout::Rect::new(0, 0, 70, 23);
        app.calendar_top = Some(0);
        let click = MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: 40,
            row: 3,
            modifiers: KeyModifiers::NONE,
        };
        let read = || std::fs::read_to_string(&path).expect("read");
        // First click: choose it (the keyboard moves to the agenda), not checked off yet
        app.handle_mouse(click);
        assert_eq!(app.focus, Focus::Agenda);
        assert_eq!(read(), text);
        // Click again: checked off
        app.handle_mouse(click);
        assert_eq!(read(), "- [x] Review PRs @2026-10-10\n");
        clean_up(&path);
    }

    #[test]
    fn undoing_an_add_keeps_the_agenda_choice_on_an_item() {
        // Add an item in the agenda (the new one, the 2nd item, is chosen),
        // then u: the agenda has only 1 item left, so the last one is chosen
        // (not the 2nd item that no longer exists, so checking off still works)
        let text = "- [ ] Review PRs @2026-10-10\n";
        let (mut app, path) = app_with_task_file("undo-add", text);
        press(&mut app, KeyCode::Char('a'));
        for c in "Buy groceries".chars() {
            press(&mut app, KeyCode::Char(c));
        }
        press(&mut app, KeyCode::Enter);
        assert_eq!(app.agenda_index, 1);
        press(&mut app, KeyCode::Char('u'));
        assert_eq!(std::fs::read_to_string(&path).expect("read"), text);
        assert_eq!(
            (
                app.focus,
                app.chosen().map(|entry| entry.item.title.as_str())
            ),
            (Focus::Agenda, Some("Review PRs"))
        );
        // Delete the only item too: the agenda is empty, back to the calendar
        press(&mut app, KeyCode::Char('d'));
        press(&mut app, KeyCode::Char('y'));
        assert_eq!(app.focus, Focus::Calendar);
        clean_up(&path);
    }

    #[test]
    fn u_undoes_the_last_writes_one_at_a_time_then_has_nothing_left() {
        let text =
            "- [ ] Review PRs @2026-10-17 09:00\n- [ ] Deploy to staging @2026-10-17 14:00\n";
        let (mut app, path) = app_with_task_file("undo", text);
        let read = || std::fs::read_to_string(&path).expect("read");
        // Check off both items, then go back to the calendar
        for code in [' ', 'j', ' '].map(KeyCode::Char) {
            press(&mut app, code);
        }
        press(&mut app, KeyCode::Esc);
        assert_eq!(
            read(),
            "- [x] Review PRs @2026-10-17 09:00\n- [x] Deploy to staging @2026-10-17 14:00\n"
        );
        // Press u on the calendar: first undoes the last write ("Deploy to staging"), then "Review PRs";
        // the screen follows
        press(&mut app, KeyCode::Char('u'));
        assert_eq!(
            read(),
            "- [x] Review PRs @2026-10-17 09:00\n- [ ] Deploy to staging @2026-10-17 14:00\n"
        );
        assert_eq!(app.notice, Some(Notice::Undone("Deploy to staging".into())));
        press(&mut app, KeyCode::Char('u'));
        assert_eq!(read(), text);
        assert!(!app.items[0].done);
        press(&mut app, KeyCode::Char('u'));
        assert_eq!(app.notice, Some(Notice::NothingToUndo));
        clean_up(&path);
    }

    #[test]
    fn u_leaves_the_file_alone_when_it_changed_elsewhere() {
        let (mut app, path) =
            app_with_task_file("undo-changed", "- [ ] Review PRs @2026-10-17 09:00\n");
        // Check off and uncheck (two steps recorded), then the file is changed again elsewhere
        press(&mut app, KeyCode::Char(' '));
        press(&mut app, KeyCode::Char(' '));
        let edited = "- [ ] Review PRs @2026-10-17 09:00\n- [ ] Book a haircut @2026-10-17 10:00\n";
        std::fs::write(&path, edited).expect("edit");
        press(&mut app, KeyCode::Char('u'));
        // The other changes stay, it only reloads; the two recorded steps no longer match, so all are cleared
        assert_eq!(std::fs::read_to_string(&path).expect("read"), edited);
        assert_eq!(app.items.len(), 2);
        assert_eq!(app.notice, Some(Notice::Reloaded));
        press(&mut app, KeyCode::Char('u'));
        assert_eq!(app.notice, Some(Notice::NothingToUndo));
        clean_up(&path);
    }

    #[test]
    fn undo_keeps_only_the_last_50_steps() {
        let (mut app, path) =
            app_with_task_file("undo-limit", "- [ ] Review PRs @2026-10-17 09:00\n");
        // Check off / uncheck 51 times (odd count → it ends checked off)
        for _ in 0..=UNDO_LEVELS {
            press(&mut app, KeyCode::Char('x'));
        }
        // Keep pressing u: only 50 undos work (pressing more does not go past that)
        let undone = (0..UNDO_LEVELS + 5)
            .filter(|_| {
                press(&mut app, KeyCode::Char('u'));
                matches!(app.notice, Some(Notice::Undone(_)))
            })
            .count();
        assert_eq!(undone, UNDO_LEVELS);
        // The earliest step was dropped: the file stays as it was after the first check-off
        assert_eq!(
            std::fs::read_to_string(&path).expect("read"),
            "- [x] Review PRs @2026-10-17 09:00\n"
        );
        clean_up(&path);
    }

    /// 10/17's "Review PRs" has one subtask (no date, not in the agenda), "Deploy to staging" has none
    /// (for the delete tests)
    const WITH_SUBTASK: &str = "- [ ] Review PRs @2026-10-17 09:00\n  - [ ] Amy's PR\n- [ ] Deploy to staging @2026-10-17 14:00\n";

    #[test]
    fn d_then_y_deletes_with_subtasks_and_u_brings_them_back() {
        let (mut app, path) = app_with_task_file("delete", WITH_SUBTASK);
        let read = || std::fs::read_to_string(&path).expect("read");
        // d: asks first, file not touched yet; "Review PRs" + subtask = 2 items
        press(&mut app, KeyCode::Char('d'));
        assert_eq!(
            app.confirm_delete()
                .map(|(item, count)| (item.title.as_str(), count)),
            Some(("Review PRs", 2))
        );
        assert_eq!(read(), WITH_SUBTASK);
        // y: deletes "Review PRs" and its subtask, only "Deploy to staging" is left; the agenda chooses it
        press(&mut app, KeyCode::Char('y'));
        assert_eq!(read(), "- [ ] Deploy to staging @2026-10-17 14:00\n");
        assert_eq!(app.confirm_delete(), None);
        assert_eq!(
            app.chosen().map(|entry| entry.item.title.as_str()),
            Some("Deploy to staging")
        );
        // u: the whole block comes back
        press(&mut app, KeyCode::Char('u'));
        assert_eq!(read(), WITH_SUBTASK);
        assert_eq!(app.notice, Some(Notice::Undone("Review PRs".into())));
        clean_up(&path);
    }

    #[test]
    fn any_key_other_than_y_cancels_the_delete() {
        let (mut app, path) = app_with_task_file("delete-cancel", WITH_SUBTASK);
        for other in [KeyCode::Char('j'), KeyCode::Char('n'), KeyCode::Esc] {
            press(&mut app, KeyCode::Char('d'));
            // The key used to cancel only cancels: still in the agenda, still on the first item
            press(&mut app, other);
            assert_eq!(app.confirm_delete(), None, "{other:?}");
            assert_eq!(
                (app.focus, app.agenda_index),
                (Focus::Agenda, 0),
                "{other:?}"
            );
        }
        assert_eq!(std::fs::read_to_string(&path).expect("read"), WITH_SUBTASK);
        clean_up(&path);
    }

    #[test]
    fn ctrl_c_quits_from_the_delete_question() {
        let (mut app, path) = app_with_task_file("delete-quit", WITH_SUBTASK);
        press(&mut app, KeyCode::Char('d'));
        app.handle_key(KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL));
        assert!(app.should_quit);
        assert_eq!(std::fs::read_to_string(&path).expect("read"), WITH_SUBTASK);
        clean_up(&path);
    }

    #[test]
    fn a_click_or_the_wheel_cancels_the_delete_question() {
        // Clicking or scrolling while asking "delete?": it just cancels (like any other
        // key); that click does nothing else (does not choose or check off what is under it)
        let (mut app, path) = app_with_task_file("delete-click", WITH_SUBTASK);
        app.screen = ratatui::layout::Rect::new(0, 0, 70, 23);
        let selected = app.selected;
        for kind in [
            MouseEventKind::Down(MouseButton::Left),
            MouseEventKind::ScrollDown,
        ] {
            press(&mut app, KeyCode::Char('d'));
            app.handle_mouse(MouseEvent {
                kind,
                column: 5,
                row: 5,
                modifiers: KeyModifiers::NONE,
            });
            assert_eq!(app.confirm_delete(), None, "{kind:?}");
            assert_eq!(
                (app.focus, app.selected),
                (Focus::Agenda, selected),
                "{kind:?}"
            );
        }
        // Moving the mouse does not count: still asking
        press(&mut app, KeyCode::Char('d'));
        app.handle_mouse(MouseEvent {
            kind: MouseEventKind::Moved,
            column: 5,
            row: 5,
            modifiers: KeyModifiers::NONE,
        });
        assert!(app.confirm_delete().is_some());
        assert_eq!(std::fs::read_to_string(&path).expect("read"), WITH_SUBTASK);
        clean_up(&path);
    }

    #[test]
    fn y_does_not_delete_when_the_subtasks_changed_meanwhile() {
        let (mut app, path) = app_with_task_file("delete-changed", WITH_SUBTASK);
        press(&mut app, KeyCode::Char('d'));
        // It was 2 items when it asked; before pressing y, a subtask was added
        // elsewhere → it would delete 3, which differs from what it asked
        let edited = "- [ ] Review PRs @2026-10-17 09:00\n  - [ ] Amy's PR\n  - [ ] Kevin's PR\n- [ ] Deploy to staging @2026-10-17 14:00\n";
        std::fs::write(&path, edited).expect("edit");
        press(&mut app, KeyCode::Char('y'));
        assert_eq!(std::fs::read_to_string(&path).expect("read"), edited);
        assert_eq!(app.notice, Some(Notice::Reloaded));
        clean_up(&path);
    }

    #[test]
    fn deleting_the_last_item_goes_back_to_the_calendar() {
        let (mut app, path) = app_with_task_file("delete-last", "- [ ] Review PRs @2026-10-17\n");
        press(&mut app, KeyCode::Char('d'));
        press(&mut app, KeyCode::Char('y'));
        assert_eq!(std::fs::read_to_string(&path).expect("read"), "");
        assert_eq!(app.focus, Focus::Calendar);
        clean_up(&path);
    }

    #[test]
    fn rewriting_keeps_the_calendars_checked_state() {
        // "Personal" is unchecked; after a check-off writes the file and
        // the calendars are re-listed, "Personal" is still unchecked
        let text = "- [ ] Review PRs @2026-10-17 09:00\n# Personal\n- [ ] Go hiking @2026-10-18\n";
        let (mut app, path) = app_with_task_file("groups", text);
        app.groups[1].shown = false;
        press(&mut app, KeyCode::Char(' '));
        assert!(app.items[0].done);
        assert!(app.groups[0].shown && !app.groups[1].shown);
        clean_up(&path);
    }
}
