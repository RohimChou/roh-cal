//! One-line editor (like tuxedo): type the line's text directly; below it, a live preview shows
//! how the line will be read; `Enter` saves (it cannot save while a mistake is not fixed),
//! `Esc` cancels
//!
//! - `e` / `Enter`: edit the line of the chosen item in the agenda (or the tasks page)
//! - `a`: add an item (in the chosen item's group; on the calendar, in the group of the selected day's
//!   first item, or at the end of the file if that day has none), with the selected day filled in
//!   (not on the tasks page)
//! - `A`: add a subtask to the chosen item
//! - with no calendar in the file yet, a new item goes into a new `# 預設` (`# Default`) calendar
//! - `a` in the calendar list: add a calendar (type its name; it becomes a `# name` heading)
//! - `e` in the calendar list: rename the chosen calendar

use super::{App, Focus, Mode};
use crate::edit::{self, Place};
use crate::tasks::{Heading, HeadingLine, Item};
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
    /// Add a calendar; `taken` = the names the calendars already have (as the editor opened)
    AddCalendar { taken: Vec<String> },
    /// Rename a calendar: `headings` = its heading lines (as read when the editor opened), `taken` = the
    /// names the other calendars have
    RenameCalendar {
        headings: Vec<HeadingLine>,
        taken: Vec<String>,
    },
}

/// What the text typed so far reads as
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Reading {
    /// An item (editing or adding an item)
    Item(Item),
    /// A calendar's heading (adding a calendar)
    Calendar(Heading),
}

impl Editor {
    /// How the text typed so far reads (shown live on screen; if there is a problem it cannot be saved)
    pub fn check(&self) -> Result<Reading, edit::Problem> {
        match &self.target {
            Target::AddCalendar { taken } | Target::RenameCalendar { taken, .. } => {
                edit::check_calendar(self.input.value(), taken).map(Reading::Calendar)
            }
            Target::Edit(_) | Target::Add(_) => edit::check(self.input.value()).map(Reading::Item),
        }
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

    /// `a`: adds an item, prefilled with `- [ ]  @<selected day>`,
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

    /// `a` in the calendar list: opens an empty editor for the new calendar's name (it goes at the top
    /// level, so it cannot take a top-level calendar's name)
    pub(super) fn start_adding_calendar(&mut self) {
        let taken = self
            .groups
            .iter()
            .filter(|group| group.parents.is_empty())
            .filter_map(|group| group.name.clone())
            .collect();
        self.mode = Mode::Editor(Editor {
            input: Input::default(),
            target: Target::AddCalendar { taken },
        });
    }

    /// `e` in the calendar list: opens the editor with the chosen calendar's heading text, without the `#`s
    /// (e.g. `US holidays type:holiday`, so a holiday calendar stays one unless `type:holiday` is taken
    /// out); 「Ungrouped」 has no heading, so nothing happens. It cannot take the name of another calendar
    /// at the same place (inside the same calendar)
    pub(super) fn start_renaming_calendar(&mut self) {
        let headings = self.chosen_calendar_headings();
        let Some(first) = headings.first() else {
            return;
        };
        let text = first.text.trim_start_matches('#').trim().to_string();
        let taken = self
            .groups
            .iter()
            .filter(|group| group.parents == first.heading.parents && !group.is(&first.heading))
            .filter_map(|group| group.name.clone())
            .collect();
        self.mode = Mode::Editor(Editor {
            input: Input::new(text),
            target: Target::RenameCalendar { headings, taken },
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
        // Save only if it can be saved; `reading` = what the typed text reads as
        let Ok(reading) = editor.check() else {
            return;
        };
        let new = editor.input.value().trim().to_string();
        let target = editor.target.clone();
        match (target, reading) {
            (Target::AddCalendar { .. }, Reading::Calendar(heading)) => {
                self.save_new_calendar(&new, &heading.name);
            }
            (Target::RenameCalendar { headings, .. }, Reading::Calendar(heading)) => {
                self.save_renamed_calendar(&headings, &new, &heading.name);
            }
            (Target::Edit(item), _) => {
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
            (Target::Add(place), Reading::Item(checked)) => {
                let Some((path, text)) = self.read_tasks_for_adding() else {
                    return;
                };
                // No calendar in the file yet (e.g. a brand-new task file): a new item (not a subtask) goes
                // into a new 「預設」 calendar at the end, written in the same step
                let (with_calendar, place) = match place {
                    Place::InGroupOf(_) | Place::End
                        if crate::tasks::parse(&text).headings.is_empty() =>
                    {
                        let texts = self.lang.texts();
                        let (with_calendar, _) = edit::add_calendar(&text, texts.default_calendar);
                        (with_calendar, Place::End)
                    }
                    place => (text.clone(), place),
                };
                match edit::add(&with_calendar, &new, &place) {
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
            // `check` reads the typed text as the kind the target asks for, so the others do not happen
            _ => {}
        }
    }

    /// Saves a new calendar: `# <new>` at the end of the task file; the calendar list chooses it (the
    /// keyboard stays there). If the task file cannot be read or written, the editor stays open and the
    /// bottom row says why
    fn save_new_calendar(&mut self, new: &str, name: &str) {
        let Some((path, text)) = self.read_tasks_for_adding() else {
            return;
        };
        let (after, _) = edit::add_calendar(&text, new);
        if !self.write_tasks(&path, text, after, name.to_string()) {
            return;
        }
        self.mode = Mode::Normal;
        if let Some(index) = self
            .groups
            .iter()
            .position(|group| group.parents.is_empty() && group.name.as_deref() == Some(name))
        {
            self.group_index = index;
        }
    }
}

impl App {
    /// Saves a calendar's new name: each of its heading lines (`headings`) becomes `<its #s> <new>`; the
    /// calendar stays chosen, and it and the calendars inside it keep their checked state (their paths
    /// change with the name). Nothing changed: nothing is written. If the task file cannot be read or
    /// written, the editor stays open and the bottom row says why; if it was changed elsewhere: reload and
    /// close the editor
    fn save_renamed_calendar(&mut self, headings: &[HeadingLine], new: &str, name: &str) {
        let Some(first) = headings.first().map(|line| line.heading.clone()) else {
            return;
        };
        let Some((path, text)) = self.read_tasks() else {
            return;
        };
        let checked = self.checked_states();
        match edit::rename_calendar(&text, headings, new) {
            Ok(after) => {
                if after != text && !self.write_tasks(&path, text, after, name.to_string()) {
                    return;
                }
                self.mode = Mode::Normal;
                let old: Vec<String> = first.parents.iter().chain([&first.name]).cloned().collect();
                let renamed: Vec<String> = first
                    .parents
                    .iter()
                    .cloned()
                    .chain([name.to_string()])
                    .collect();
                self.carry_checked(&checked, &old, &renamed);
            }
            Err(edit::Changed) => {
                self.mode = Mode::Normal;
                self.reload(&text);
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
    fn a_in_the_calendar_list_adds_a_calendar_at_the_end() {
        let text = "# Work\n- [ ] Review PRs @2026-10-10\n";
        let (mut app, path) = app_with_task_file("new-calendar", text);
        app.focus = Focus::Side;
        press(&mut app, KeyCode::Char('a'));
        // A name some calendar already has cannot be saved: the editor stays open
        type_text(&mut app, "Work");
        assert_eq!(
            app.editor().map(Editor::check),
            Some(Err(edit::Problem::NameTaken))
        );
        press(&mut app, KeyCode::Enter);
        assert!(app.editor().is_some());
        // A new name: written last, after a blank line; the calendar list shows it, chosen
        for _ in 0..4 {
            press(&mut app, KeyCode::Backspace);
        }
        type_text(&mut app, "Side projects");
        press(&mut app, KeyCode::Enter);
        assert!(app.editor().is_none());
        assert_eq!(
            std::fs::read_to_string(&path).expect("read"),
            "# Work\n- [ ] Review PRs @2026-10-10\n\n# Side projects\n"
        );
        let names: Vec<Option<&str>> = app
            .groups
            .iter()
            .map(|group| group.name.as_deref())
            .collect();
        assert_eq!(names, [Some("Work"), Some("Side projects")]);
        assert_eq!((app.focus, app.group_index), (Focus::Side, 1));
        // u takes it back out
        press(&mut app, KeyCode::Char('u'));
        assert_eq!(std::fs::read_to_string(&path).expect("read"), text);
        assert_eq!(app.groups.len(), 1);
        assert_eq!(app.notice, Some(Notice::Undone("Side projects".into())));
        clean_up(&path);
    }

    #[test]
    fn e_in_the_calendar_list_renames_every_heading_of_the_calendar() {
        // Work appears twice (one calendar) and has SFAP inside it
        let text = "# Work\n- [ ] Review PRs @2026-10-10\n## SFAP\n- [ ] Fix the login bug @2026-10-11\n\
                    # Days off type:holiday\n- Long weekend @2026-10-12\n# Work\n- [ ] Deploy @2026-10-20\n";
        let (mut app, path) = app_with_task_file("rename-calendar", text);
        let names = |app: &App| -> Vec<(String, bool)> {
            app.groups
                .iter()
                .map(|group| (group.path().join(" > "), group.shown))
                .collect()
        };
        app.focus = Focus::Side;
        // The holiday calendar (third in the tree), unchecked: the editor holds its heading text,
        // type:holiday too
        app.group_index = 2;
        app.groups[2].shown = false;
        press(&mut app, KeyCode::Char('e'));
        let typed = "Days off type:holiday";
        assert_eq!(app.editor().map(|editor| editor.input.value()), Some(typed));
        // The name of another calendar at the same level cannot be taken
        for _ in typed.chars() {
            press(&mut app, KeyCode::Backspace);
        }
        type_text(&mut app, "Work");
        press(&mut app, KeyCode::Enter);
        assert!(app.editor().is_some());
        // A new name: its heading is rewritten, the calendar stays chosen and unchecked
        for _ in 0..4 {
            press(&mut app, KeyCode::Backspace);
        }
        type_text(&mut app, "Holidays type:holiday");
        press(&mut app, KeyCode::Enter);
        let holidays = text.replace("# Days off", "# Holidays");
        assert_eq!(std::fs::read_to_string(&path).expect("read"), holidays);
        assert_eq!(app.group_index, 2);
        assert_eq!(names(&app)[2], ("Holidays".to_string(), false));
        // Work, with SFAP unchecked: both Work headings are renamed, and SFAP (now inside Day job) stays
        // unchecked
        app.groups[1].shown = false;
        app.group_index = 0;
        press(&mut app, KeyCode::Char('e'));
        for _ in 0..4 {
            press(&mut app, KeyCode::Backspace);
        }
        type_text(&mut app, "Day job");
        press(&mut app, KeyCode::Enter);
        let renamed = holidays.replace("# Work", "# Day job");
        assert_eq!(std::fs::read_to_string(&path).expect("read"), renamed);
        assert_eq!(
            names(&app),
            [
                ("Day job".to_string(), true),
                ("Day job > SFAP".to_string(), false),
                ("Holidays".to_string(), false),
            ]
        );
        assert_eq!(app.group_index, 0);
        // Enter without changing anything: nothing written (no undo step)
        let steps = app.undo.len();
        press(&mut app, KeyCode::Char('e'));
        press(&mut app, KeyCode::Enter);
        assert!(app.editor().is_none());
        assert_eq!(app.undo.len(), steps);
        // u brings the old name back
        press(&mut app, KeyCode::Char('u'));
        assert_eq!(std::fs::read_to_string(&path).expect("read"), holidays);
        clean_up(&path);
    }

    #[test]
    fn a_failed_write_keeps_the_editor_and_what_was_typed() {
        use std::os::unix::fs::PermissionsExt;
        let text = "- [ ] Review PRs @2026-10-10\n";
        let (mut app, path) = app_with_task_file("editor-failed", text);
        press(&mut app, KeyCode::Char('e'));
        type_text(&mut app, " for Amy");
        // Make the folder read-only: the temp file cannot be written (reading still works)
        let dir = path.parent().expect("dir");
        let mode = |mode| std::fs::set_permissions(dir, std::fs::Permissions::from_mode(mode));
        mode(0o555).expect("read-only");
        press(&mut app, KeyCode::Enter);
        // The editor is still open, the typed text is still there, and the bottom row says why
        assert_eq!(
            app.editor().map(|editor| editor.input.value()),
            Some("- [ ] Review PRs @2026-10-10 for Amy")
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
            "- [ ] Review PRs @2026-10-10 for Amy\n"
        );
        clean_up(&path);
    }

    #[test]
    fn e_opens_the_line_and_enter_saves_what_was_typed() {
        let text = "# Recurring\n  - [ ] Send the weekly update @2026-10-16 17:30\n";
        let (mut app, path) = app_with_task_file("edit", text);
        // e: the editor holds that line (without indentation), cursor at the end
        press(&mut app, KeyCode::Char('e'));
        let editor = app.editor().expect("editor");
        assert_eq!(
            editor.input.value(),
            "- [ ] Send the weekly update @2026-10-16 17:30"
        );
        // Change 17:30 to 18:00: the cursor is at the end, press Backspace 5 times, then type
        for _ in 0..5 {
            press(&mut app, KeyCode::Backspace);
        }
        type_text(&mut app, "18:00 to my manager");
        press(&mut app, KeyCode::Enter);
        // Indentation kept; "to my manager" follows the time and is part of the title
        assert_eq!(
            std::fs::read_to_string(&path).expect("read"),
            "# Recurring\n  - [ ] Send the weekly update @2026-10-16 18:00 to my manager\n"
        );
        assert!(app.editor().is_none());
        assert_eq!(
            app.chosen().map(|entry| entry.item.title.as_str()),
            Some("Send the weekly update to my manager")
        );
        // u undoes
        press(&mut app, KeyCode::Char('u'));
        assert_eq!(std::fs::read_to_string(&path).expect("read"), text);
        clean_up(&path);
    }

    #[test]
    fn a_in_the_agenda_adds_to_the_chosen_items_group() {
        let text =
            "# Work\n- [ ] Write the design doc @2026-10-16\n# Personal\n- Go hiking @2026-10-18\n";
        let (mut app, path) = app_with_task_file("add", text);
        // a: the selected day is prefilled, cursor where the title goes; the top border says 「新增」
        // (zh-TW for "New item")
        press(&mut app, KeyCode::Char('a'));
        let editor = app.editor().expect("editor");
        assert_eq!(editor.input.value(), "- [ ]  @2026-10-16");
        assert_eq!(editor.input.cursor(), 6);
        type_text(&mut app, "Send the weekly update");
        press(&mut app, KeyCode::Enter);
        // Added at the end of "Work"; the agenda chooses the new item
        assert_eq!(
            std::fs::read_to_string(&path).expect("read"),
            "# Work\n- [ ] Write the design doc @2026-10-16\n- [ ] Send the weekly update @2026-10-16\n# Personal\n- Go hiking @2026-10-18\n"
        );
        assert_eq!(
            app.chosen().map(|entry| entry.item.title.as_str()),
            Some("Send the weekly update")
        );
        // u: removes the whole line; the notice names the added item
        press(&mut app, KeyCode::Char('u'));
        assert_eq!(std::fs::read_to_string(&path).expect("read"), text);
        assert_eq!(
            app.notice,
            Some(Notice::Undone("Send the weekly update".into()))
        );
        clean_up(&path);
    }

    #[test]
    fn with_no_calendar_yet_a_new_item_goes_into_a_default_calendar() {
        // An empty task file: 「預設」 is written first, the item under it, in one step (u takes both back)
        let (mut app, path) = app_with_task_file("default-calendar", "");
        let read = || std::fs::read_to_string(&path).expect("read");
        app.focus = Focus::Calendar;
        press(&mut app, KeyCode::Char('a'));
        type_text(&mut app, "Get a flu shot");
        press(&mut app, KeyCode::Enter);
        assert_eq!(read(), "# 預設\n- [ ] Get a flu shot @2026-10-17\n");
        let names: Vec<Option<&str>> = app
            .groups
            .iter()
            .map(|group| group.name.as_deref())
            .collect();
        assert_eq!(names, [Some("預設")]);
        // Once there is a calendar, adding works as usual (the end of the file is inside 預設)
        app.focus = Focus::Calendar;
        app.selected = date(2026, 10, 20);
        press(&mut app, KeyCode::Char('a'));
        type_text(&mut app, "Renew passport");
        press(&mut app, KeyCode::Enter);
        assert_eq!(
            read(),
            "# 預設\n- [ ] Get a flu shot @2026-10-17\n- [ ] Renew passport @2026-10-20\n"
        );
        press(&mut app, KeyCode::Char('u'));
        press(&mut app, KeyCode::Char('u'));
        assert_eq!(read(), "");
        clean_up(&path);
        // Only items before any heading (no calendar yet): 預設 goes at the end; a subtask (A) does not
        // make one
        let (mut app, path) =
            app_with_task_file("default-calendar-loose", "- [ ] Loose @2026-10-17\n");
        press(&mut app, KeyCode::Char('A'));
        type_text(&mut app, "Sub");
        press(&mut app, KeyCode::Enter);
        assert_eq!(
            std::fs::read_to_string(&path).expect("read"),
            "- [ ] Loose @2026-10-17\n  - [ ] Sub\n"
        );
        app.focus = Focus::Calendar;
        press(&mut app, KeyCode::Char('a'));
        type_text(&mut app, "New");
        press(&mut app, KeyCode::Enter);
        assert_eq!(
            std::fs::read_to_string(&path).expect("read"),
            "- [ ] Loose @2026-10-17\n  - [ ] Sub\n\n# 預設\n- [ ] New @2026-10-17\n"
        );
        clean_up(&path);
    }

    #[test]
    fn a_on_the_calendar_adds_on_the_selected_day() {
        // Selected 10/20 (nothing that day): added at the end of
        // the file; the keyboard moves to the agenda with it chosen
        let text = "# Work\n- [ ] Write the design doc @2026-10-16\n";
        let (mut app, path) = app_with_task_file("add-calendar", text);
        press(&mut app, KeyCode::Esc);
        app.selected = date(2026, 10, 20);
        press(&mut app, KeyCode::Char('a'));
        type_text(&mut app, "Get a flu shot");
        press(&mut app, KeyCode::Enter);
        assert_eq!(
            std::fs::read_to_string(&path).expect("read"),
            "# Work\n- [ ] Write the design doc @2026-10-16\n- [ ] Get a flu shot @2026-10-20\n"
        );
        assert_eq!(app.focus, Focus::Agenda);
        // Selected 10/16 ("Write the design doc" is that day): added to its
        // group; layout 3 has no agenda, so the keyboard stays on the calendar
        app.view = crate::app::View::Month;
        app.focus = Focus::Calendar;
        app.selected = date(2026, 10, 16);
        press(&mut app, KeyCode::Char('a'));
        type_text(&mut app, "Share it with the team");
        press(&mut app, KeyCode::Enter);
        assert!(std::fs::read_to_string(&path).expect("read").contains(
            "- [ ] Get a flu shot @2026-10-20\n- [ ] Share it with the team @2026-10-16\n"
        ));
        assert_eq!(app.focus, Focus::Calendar);
        clean_up(&path);
    }

    #[test]
    fn shift_a_adds_a_subtask_and_keeps_the_parent_chosen() {
        let text =
            "- [ ] Write the design doc @2026-10-16\n- [ ] Send the weekly update @2026-10-16\n";
        let (mut app, path) = app_with_task_file("add-subtask", text);
        press(&mut app, KeyCode::Char('A'));
        assert_eq!(
            app.editor().map(|editor| editor.input.value()),
            Some("- [ ] ")
        );
        type_text(&mut app, "Gather requirements");
        press(&mut app, KeyCode::Enter);
        assert_eq!(
            std::fs::read_to_string(&path).expect("read"),
            "- [ ] Write the design doc @2026-10-16\n  - [ ] Gather requirements\n- [ ] Send the weekly update @2026-10-16\n"
        );
        // The subtask has no date and is not in the agenda: "Write the design doc" stays chosen
        assert_eq!(
            app.chosen().map(|entry| entry.item.title.as_str()),
            Some("Write the design doc")
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
        // Title typed: the file is created, the item in a new 預設 calendar (the file had none)
        type_text(&mut app, "First task");
        press(&mut app, KeyCode::Enter);
        assert_eq!(
            std::fs::read_to_string(&path).expect("read"),
            "# 預設\n- [ ] First task @2026-10-17\n"
        );
        clean_up(&path);
    }

    #[test]
    fn a_line_with_a_mistake_is_not_saved() {
        let text = "- [ ] Send the weekly update @2026-10-16\n";
        let (mut app, path) = app_with_task_file("edit-mistake", text);
        press(&mut app, KeyCode::Enter);
        // Change the date to 10/32, which does not exist: Enter does
        // not save, the editor stays open, the check result is an error
        for _ in 0..2 {
            press(&mut app, KeyCode::Backspace);
        }
        type_text(&mut app, "32");
        press(&mut app, KeyCode::Enter);
        let editor = app.editor().expect("still open");
        assert!(editor.check().is_err());
        assert_eq!(std::fs::read_to_string(&path).expect("read"), text);
        // Remove the leading `- `: not an item, not saved either
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
        let text = "- [ ] Send the weekly update @2026-10-16\n";
        let (mut app, path) = app_with_task_file("edit-keys", text);
        press(&mut app, KeyCode::Char('e'));
        type_text(&mut app, " qu j");
        assert!(!app.should_quit);
        assert_eq!(
            app.editor().map(|editor| editor.input.value()),
            Some("- [ ] Send the weekly update @2026-10-16 qu j")
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
        let text = "- [ ] Send the weekly update @2026-10-16\n";
        let (mut app, path) = app_with_task_file("edit-undated", text);
        press(&mut app, KeyCode::Char('e'));
        for _ in 0.." @2026-10-16".chars().count() {
            press(&mut app, KeyCode::Backspace);
        }
        press(&mut app, KeyCode::Enter);
        assert_eq!(
            std::fs::read_to_string(&path).expect("read"),
            "- [ ] Send the weekly update\n"
        );
        assert_eq!(app.focus, Focus::Calendar);
        assert_eq!(app.selected, date(2026, 10, 16));
        clean_up(&path);
    }
}
