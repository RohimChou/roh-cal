//! One-line editor (like tuxedo): type the line's text directly; below it, a live preview shows
//! how the line will be read; `Enter` saves (it cannot save while a mistake is not fixed),
//! `Esc` cancels
//!
//! - `e` / `Enter`: edit the line of the chosen item in the agenda (or the tasks page)
//! - `a`: add an item (in the chosen item's group; on the calendar, in the group of the selected day's
//!   first item, or at the end of the file if that day has none), with the selected day filled in
//!   (not on the tasks page)
//! - `A`: add a subtask to the chosen item

use super::{App, Focus, Mode};
use crate::edit::{self, Place};
use crate::tasks::Item;
use ratatui::crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};
use tui_input::{Input, backend::crossterm::to_input_request};

/// The editor: the typed text and what to do with it
#[derive(Debug)]
pub struct Editor {
    /// The typed text (the line, without indentation) and the cursor
    pub input: Input,
    /// Which item to edit, or where to add the new one
    pub target: Target,
}

/// What the editor does when it saves
#[derive(Debug, Clone)]
pub enum Target {
    /// Edit this item (as it was read when the editor opened)
    Edit(Item),
    /// Add a new item, placed here
    Add(Place),
}

impl Editor {
    /// How the line typed so far reads (shown live on screen; if there is a problem it cannot be saved)
    pub fn check(&self) -> Result<Item, edit::Problem> {
        edit::check(self.input.value())
    }
}

impl App {
    /// `e` / `Enter`: opens the editor with the line of the chosen item in
    /// the agenda (or the tasks page), without indentation, cursor at the end
    pub(super) fn start_editing(&mut self) {
        let Some(item) = self.chosen_item().cloned() else {
            return;
        };
        let input = Input::new(item.line_text.trim().to_string());
        self.mode = Mode::Editor(Editor {
            input,
            target: Target::Edit(item),
        });
    }

    /// `a`: adds an item, prefilled with `- [ ]  @選取的日期`,
    /// cursor where the title goes (no date on the tasks page)
    ///
    /// Keyboard in the agenda, the tasks page or layout 3's popup: placed in the
    /// chosen item's group; on the calendar: placed in the group of the selected
    /// day's first item, or at the end of the file if that day has none
    pub(super) fn start_adding(&mut self) {
        let near = match self.focus {
            Focus::Agenda | Focus::Tasks | Focus::Day => self.chosen_item().cloned(),
            Focus::Calendar | Focus::Side => self
                .items_on(self.selected)
                .first()
                .map(|item| (*item).clone()),
        };
        let place = near.map_or(Place::End, Place::InGroupOf);
        let prefix = "- [ ] ";
        // The tasks page has no selected day: no date filled in
        let input = if self.view == super::View::Tasks {
            Input::new(prefix.to_string())
        } else {
            Input::new(format!("{prefix} @{}", self.selected)).with_cursor(prefix.chars().count())
        };
        self.mode = Mode::Editor(Editor {
            input,
            target: Target::Add(place),
        });
    }

    /// `A`: adds a subtask to the chosen item in the agenda (or the tasks page)
    /// (prefilled with `- [ ] `; subtasks often have no date of their own)
    pub(super) fn start_adding_subtask(&mut self) {
        let Some(item) = self.chosen_item().cloned() else {
            return;
        };
        self.mode = Mode::Editor(Editor {
            input: Input::new("- [ ] ".to_string()),
            target: Target::Add(Place::ChildOf(item)),
        });
    }

    /// Keys while the editor is open: `Enter` saves, `Esc` cancels, `Ctrl+C` quits;
    /// every other key is typing (cursor movement and deleting: see tui-input)
    pub(super) fn handle_editor_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.should_quit = true;
            }
            KeyCode::Enter => self.save_editor(),
            KeyCode::Esc => self.mode = Mode::Normal,
            _ => {
                if let Mode::Editor(editor) = &mut self.mode
                    && let Some(request) = to_input_request(&Event::Key(key))
                {
                    editor.input.handle(request);
                }
            }
        }
    }

    /// Saves: edit = the line is replaced by the typed text (indentation kept); add = the typed text goes
    /// where it belongs. Writes back to the task file, and the agenda (tasks page) keeps that item chosen
    ///
    /// If the typed text has a problem (not an item, a wrong date, no title), it is not saved and the
    /// editor stays open (red text on screen explains); an edit that changed nothing is not written.
    /// If the task file cannot be read or written: the editor also stays open (the typed text is kept)
    /// and the bottom row says why; if the file was changed elsewhere: reload and close the editor
    fn save_editor(&mut self) {
        let Some(editor) = self.editor() else {
            return;
        };
        // Save only if it can be saved; `checked` = the item this typed line reads as
        let Ok(checked) = editor.check() else {
            return;
        };
        let new = editor.input.value().trim().to_string();
        let target = editor.target.clone();
        match target {
            Target::Edit(item) => {
                if new != item.line_text.trim() {
                    let Some((path, text)) = self.read_tasks() else {
                        return;
                    };
                    match edit::replace(&text, &item, &new) {
                        Ok(after) => {
                            if !self.write_tasks(&path, text, after, item.title) {
                                return;
                            }
                        }
                        Err(edit::Changed) => {
                            // Changed elsewhere: reload; the keyboard goes back to this
                            // layout's main box (the tasks page stays the tasks page)
                            self.mode = Mode::Normal;
                            self.reload(&text);
                            self.focus = self.home_focus();
                            return;
                        }
                    }
                }
                self.mode = Mode::Normal;
                self.choose_line(item.line);
            }
            Target::Add(place) => {
                let Some((path, text)) = self.read_tasks_for_adding() else {
                    return;
                };
                match edit::add(&text, &new, &place) {
                    Ok((after, line)) => {
                        if !self.write_tasks(&path, text, after, checked.title) {
                            return;
                        }
                        self.mode = Mode::Normal;
                        // The agenda chooses the new item (always visible on the tasks page); if it is not in
                        // the agenda (e.g. a subtask without a date), the originally chosen item stays chosen
                        // (its line number did not change)
                        let shown = self.view == super::View::Tasks
                            || self.agenda().iter().any(|entry| entry.item.line == line);
                        match &place {
                            _ if shown => self.choose_line(line),
                            Place::InGroupOf(item) | Place::ChildOf(item)
                                if self.focus == Focus::Agenda =>
                            {
                                self.choose_line(item.line);
                            }
                            _ => {}
                        }
                    }
                    Err(edit::Changed) => {
                        self.mode = Mode::Normal;
                        self.reload(&text);
                        self.focus = self.home_focus();
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::write::Notice;
    use super::super::write::tests::{app_with_task_file, clean_up, press};
    use super::*;
    use jiff::civil::date;

    /// Types text into the editor one character at a time
    fn type_text(app: &mut App, text: &str) {
        for c in text.chars() {
            press(app, KeyCode::Char(c));
        }
    }

    #[test]
    fn a_failed_write_keeps_the_editor_and_what_was_typed() {
        use std::os::unix::fs::PermissionsExt;
        let text = "- [ ] 早上 @2026-10-10\n";
        let (mut app, path) = app_with_task_file("editor-failed", text);
        press(&mut app, KeyCode::Char('e'));
        type_text(&mut app, " 改");
        // Make the folder read-only: the temp file cannot be written (reading still works)
        let dir = path.parent().expect("dir");
        let mode = |mode| std::fs::set_permissions(dir, std::fs::Permissions::from_mode(mode));
        mode(0o555).expect("read-only");
        press(&mut app, KeyCode::Enter);
        // The editor is still open, the typed text is still there, and the bottom row says why
        assert_eq!(
            app.editor().map(|editor| editor.input.value()),
            Some("- [ ] 早上 @2026-10-10 改")
        );
        assert_eq!(
            app.notice,
            Some(Notice::Failed(std::io::ErrorKind::PermissionDenied))
        );
        assert_eq!(std::fs::read_to_string(&path).expect("read"), text);
        // Make it writable again: pressing Enter once more saves and closes the editor
        mode(0o755).expect("writable");
        press(&mut app, KeyCode::Enter);
        assert!(app.editor().is_none());
        assert_eq!(
            std::fs::read_to_string(&path).expect("read"),
            "- [ ] 早上 @2026-10-10 改\n"
        );
        clean_up(&path);
    }

    #[test]
    fn e_opens_the_line_and_enter_saves_what_was_typed() {
        let text = "# 例行\n  - [ ] 交週報 @2026-10-16 17:30\n";
        let (mut app, path) = app_with_task_file("edit", text);
        // e: the editor holds that line (without indentation), cursor at the end
        press(&mut app, KeyCode::Char('e'));
        let editor = app.editor().expect("編輯框");
        assert_eq!(editor.input.value(), "- [ ] 交週報 @2026-10-16 17:30");
        // Change 17:30 to 18:00: the cursor is at the end, press Backspace 5 times, then type
        for _ in 0..5 {
            press(&mut app, KeyCode::Backspace);
        }
        type_text(&mut app, "18:00 給主管");
        press(&mut app, KeyCode::Enter);
        // Indentation kept; 「給主管」 follows the time and is part of the title
        assert_eq!(
            std::fs::read_to_string(&path).expect("read"),
            "# 例行\n  - [ ] 交週報 @2026-10-16 18:00 給主管\n"
        );
        assert!(app.editor().is_none());
        assert_eq!(
            app.chosen().map(|entry| entry.item.title.as_str()),
            Some("交週報 給主管")
        );
        // u undoes
        press(&mut app, KeyCode::Char('u'));
        assert_eq!(std::fs::read_to_string(&path).expect("read"), text);
        clean_up(&path);
    }

    #[test]
    fn a_in_the_agenda_adds_to_the_chosen_items_group() {
        let text = "# 工作\n- [ ] 寫報告 @2026-10-16\n# 生活\n- 爬山 @2026-10-18\n";
        let (mut app, path) = app_with_task_file("add", text);
        // a: the selected day is prefilled, cursor where the title goes; the top border says 「新增」
        press(&mut app, KeyCode::Char('a'));
        let editor = app.editor().expect("編輯框");
        assert_eq!(editor.input.value(), "- [ ]  @2026-10-16");
        assert_eq!(editor.input.cursor(), 6);
        type_text(&mut app, "交週報");
        press(&mut app, KeyCode::Enter);
        // Added at the end of 「工作」; the agenda chooses the new item
        assert_eq!(
            std::fs::read_to_string(&path).expect("read"),
            "# 工作\n- [ ] 寫報告 @2026-10-16\n- [ ] 交週報 @2026-10-16\n# 生活\n- 爬山 @2026-10-18\n"
        );
        assert_eq!(
            app.chosen().map(|entry| entry.item.title.as_str()),
            Some("交週報")
        );
        // u: removes the whole line; the notice names the added item
        press(&mut app, KeyCode::Char('u'));
        assert_eq!(std::fs::read_to_string(&path).expect("read"), text);
        assert_eq!(app.notice, Some(Notice::Undone("交週報".into())));
        clean_up(&path);
    }

    #[test]
    fn a_on_the_calendar_adds_on_the_selected_day() {
        // Selected 10/20 (nothing that day): added at the end of
        // the file; the keyboard moves to the agenda with it chosen
        let text = "# 工作\n- [ ] 寫報告 @2026-10-16\n";
        let (mut app, path) = app_with_task_file("add-calendar", text);
        press(&mut app, KeyCode::Esc);
        app.selected = date(2026, 10, 20);
        press(&mut app, KeyCode::Char('a'));
        type_text(&mut app, "看醫生");
        press(&mut app, KeyCode::Enter);
        assert_eq!(
            std::fs::read_to_string(&path).expect("read"),
            "# 工作\n- [ ] 寫報告 @2026-10-16\n- [ ] 看醫生 @2026-10-20\n"
        );
        assert_eq!(app.focus, Focus::Agenda);
        // Selected 10/16 (「寫報告」 is that day): added to 「寫報告」's
        // group; layout 3 has no agenda, so the keyboard stays on the calendar
        app.view = crate::app::View::Month;
        app.focus = Focus::Calendar;
        app.selected = date(2026, 10, 16);
        press(&mut app, KeyCode::Char('a'));
        type_text(&mut app, "印出來");
        press(&mut app, KeyCode::Enter);
        assert!(
            std::fs::read_to_string(&path)
                .expect("read")
                .contains("- [ ] 看醫生 @2026-10-20\n- [ ] 印出來 @2026-10-16\n")
        );
        assert_eq!(app.focus, Focus::Calendar);
        clean_up(&path);
    }

    #[test]
    fn shift_a_adds_a_subtask_and_keeps_the_parent_chosen() {
        let text = "- [ ] 寫報告 @2026-10-16\n- [ ] 交週報 @2026-10-16\n";
        let (mut app, path) = app_with_task_file("add-subtask", text);
        press(&mut app, KeyCode::Char('A'));
        assert_eq!(
            app.editor().map(|editor| editor.input.value()),
            Some("- [ ] ")
        );
        type_text(&mut app, "找資料");
        press(&mut app, KeyCode::Enter);
        assert_eq!(
            std::fs::read_to_string(&path).expect("read"),
            "- [ ] 寫報告 @2026-10-16\n  - [ ] 找資料\n- [ ] 交週報 @2026-10-16\n"
        );
        // The subtask has no date and is not in the agenda: 「寫報告」 stays chosen
        assert_eq!(
            app.chosen().map(|entry| entry.item.title.as_str()),
            Some("寫報告")
        );
        clean_up(&path);
    }

    #[test]
    fn adding_creates_a_missing_task_file_and_needs_a_title() {
        let (mut app, path) = app_with_task_file("add-new-file", "");
        std::fs::remove_file(&path).expect("remove");
        app.focus = Focus::Calendar;
        // Enter without a title: cannot save, the editor stays open
        press(&mut app, KeyCode::Char('a'));
        press(&mut app, KeyCode::Enter);
        assert_eq!(
            app.editor().map(Editor::check),
            Some(Err(edit::Problem::EmptyTitle))
        );
        assert!(!path.exists());
        // Title typed: the file is created
        type_text(&mut app, "第一件事");
        press(&mut app, KeyCode::Enter);
        assert_eq!(
            std::fs::read_to_string(&path).expect("read"),
            "- [ ] 第一件事 @2026-10-17\n"
        );
        clean_up(&path);
    }

    #[test]
    fn a_line_with_a_mistake_is_not_saved() {
        let text = "- [ ] 交週報 @2026-10-16\n";
        let (mut app, path) = app_with_task_file("edit-mistake", text);
        press(&mut app, KeyCode::Enter);
        // Change the date to 10/32, which does not exist: Enter does
        // not save, the editor stays open, the check result is an error
        for _ in 0..2 {
            press(&mut app, KeyCode::Backspace);
        }
        type_text(&mut app, "32");
        press(&mut app, KeyCode::Enter);
        let editor = app.editor().expect("還開著");
        assert!(editor.check().is_err());
        assert_eq!(std::fs::read_to_string(&path).expect("read"), text);
        // Remove the leading 「- 」: not an item, not saved either
        press(&mut app, KeyCode::Home);
        press(&mut app, KeyCode::Delete);
        press(&mut app, KeyCode::Delete);
        assert_eq!(
            app.editor().map(Editor::check),
            Some(Err(edit::Problem::NotAnItem))
        );
        // Esc: not saved, closed
        press(&mut app, KeyCode::Esc);
        assert!(app.editor().is_none());
        assert_eq!(std::fs::read_to_string(&path).expect("read"), text);
        clean_up(&path);
    }

    #[test]
    fn keys_go_to_the_editor_while_it_is_open() {
        // q, u, j, Space are all typing: they do not quit, undo, change the chosen item or check it off
        let text = "- [ ] 交週報 @2026-10-16\n";
        let (mut app, path) = app_with_task_file("edit-keys", text);
        press(&mut app, KeyCode::Char('e'));
        type_text(&mut app, " qu j");
        assert!(!app.should_quit);
        assert_eq!(
            app.editor().map(|editor| editor.input.value()),
            Some("- [ ] 交週報 @2026-10-16 qu j")
        );
        // Saving with no change: nothing is written, no undo is recorded
        press(&mut app, KeyCode::Esc);
        press(&mut app, KeyCode::Char('e'));
        press(&mut app, KeyCode::Enter);
        press(&mut app, KeyCode::Char('u'));
        assert_eq!(app.notice, Some(Notice::NothingToUndo));
        clean_up(&path);
    }

    #[test]
    fn taking_the_date_away_leaves_the_agenda() {
        // Date removed: after saving, the agenda no longer has it and the keyboard goes back to the calendar
        let text = "- [ ] 交週報 @2026-10-16\n";
        let (mut app, path) = app_with_task_file("edit-undated", text);
        press(&mut app, KeyCode::Char('e'));
        for _ in 0.." @2026-10-16".chars().count() {
            press(&mut app, KeyCode::Backspace);
        }
        press(&mut app, KeyCode::Enter);
        assert_eq!(
            std::fs::read_to_string(&path).expect("read"),
            "- [ ] 交週報\n"
        );
        assert_eq!(app.focus, Focus::Calendar);
        assert_eq!(app.selected, date(2026, 10, 16));
        clean_up(&path);
    }
}
