//! One-line editor (like tuxedo): type the line's text directly; below it, a live preview shows
//! how the line will be read; `Enter` saves (it cannot save while a mistake is not fixed),
//! `Esc` cancels
//!
//! - `e` / `Enter`: edit the line of the chosen item in the agenda (or the tasks page); the top border names
//!   its calendar, and picking another one in the `#` list moves it there (with its subtasks) on save
//! - `a`: add an item (in the chosen item's group; on the calendar, in the group of the selected day's
//!   first item, or in the first calendar that is not a holiday calendar if that day has none), on the
//!   selected day (not on the tasks page); the top border names the calendar. Typing `#` and part of a
//!   calendar's name opens a list of the calendars that match (fuzzy, like fzy); picking one moves the new
//!   item there and takes the `#` word out of the text (`HashList`)
//! - on the calendar views, the selected day's own date is left out of the typed text (a time stays as
//!   `@10:00`) and put back on save (`Editor::day`); a multi-day item and the tasks page show the whole line
//! - `A`: add a subtask to the chosen item
//! - with no calendar in the file yet, a new item goes into a new `# 預設` (`# Default`) calendar
//! - `a` in the calendar list: add a calendar (type its name; it becomes a `# name` heading); `A`: add it
//!   inside the chosen calendar (one `#` more, at the end of that calendar). Typing `#` and part of a
//!   calendar's name picks the calendar it goes inside (「最外層」 = the top level)
//! - `e` in the calendar list: rename the chosen calendar; `#` and part of another calendar's name moves it
//!   inside that one (with its items and the calendars inside it) on save

use super::{App, Focus, Mode};
use crate::edit::{self, Place};
use crate::search::{self, Found};
use crate::tasks::{Heading, HeadingLine, Item, Priority};
use crate::ui;
use jiff::civil::Date;
use ratatui::crossterm::event::{
    Event, KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
};
use ratatui::layout::Position;
use tui_input::{Input, backend::crossterm::to_input_request};

/// The editor: the typed text and what to do with it
#[derive(Debug)]
pub struct Editor {
    /// The typed text (the line, without indentation) and the cursor
    pub input: Input,
    /// Which item to edit, or where to add the new one
    pub target: Target,
    /// Editing or adding on this day: its `@date` is left out of the typed text and put back on save
    /// (`edit::hide_day` / `edit::show_day`); `None` = the typed text is the line as it is
    pub day: Option<Date>,
    /// The `#` list of calendars to pick from
    pub hash_list: HashList,
}

/// The `#` list: typing `#` and part of a calendar's name (e.g. `#sf`) lists the calendars that match, best
/// match first, to pick where the new item goes
#[derive(Debug, Default)]
pub struct HashList {
    /// The calendars that can be picked (their paths, in the calendar list's order); empty = `#` is only text
    /// (e.g. adding a subtask)
    pub calendars: Vec<Vec<String>>,
    /// What the top level is listed as (「最外層」) when picking where a calendar goes (its path is empty);
    /// `None` when picking where an item goes
    pub top: Option<String>,
    /// Whether it is open: typing in a `#` word opens it; `Esc`, moving the cursor or picking one closes it
    pub open: bool,
    /// The chosen row (0 = the best match)
    pub index: usize,
}

/// What the editor does when it saves
#[derive(Debug, Clone)]
pub enum Target {
    /// Edit this item (as it was read when the editor opened); `calendar` = the calendar picked in the `#`
    /// list (it moves there on save), `None` = it stays where it is
    Edit {
        item: Item,
        calendar: Option<Vec<String>>,
    },
    /// Add a new item, placed here
    Add(Place),
    /// Add a calendar inside the calendar with the path `parent` (empty = at the top level; the `#` list
    /// changes it); `others` = every calendar's path as the editor opened (the name cannot be one already
    /// in `parent`)
    AddCalendar {
        parent: Vec<String>,
        others: Vec<Vec<String>>,
    },
    /// Rename a calendar: `headings` = its heading lines (as read when the editor opened), `parent` = the
    /// calendar it is in, `others` = every other calendar's path
    RenameCalendar {
        headings: Vec<HeadingLine>,
        parent: Vec<String>,
        others: Vec<Vec<String>>,
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
    /// The path of the calendar (e.g. `Work`, `SFAP`) an added item (not a subtask) goes into, or an edited
    /// item is in (or moves to); `None` otherwise, or for an item outside any calendar
    pub fn calendar(&self) -> Option<Vec<String>> {
        match &self.target {
            Target::Add(Place::InGroupOf(item)) => calendar_of(item),
            Target::Add(Place::InCalendar(path)) => Some(path.clone()),
            Target::Edit { item, calendar } => calendar.clone().or_else(|| calendar_of(item)),
            _ => None,
        }
    }

    /// The `#` word the cursor is in (a word starting with `#`, after a space or at the start), as the
    /// position of its `#` (in characters) and what is typed after the `#` up to the cursor; e.g. with the
    /// cursor at the end of `- [ ] Pack #wo` → (11, `wo`)
    pub fn hash_word(&self) -> Option<(usize, String)> {
        let chars: Vec<char> = self.input.value().chars().collect();
        let before = &chars[..self.input.cursor().min(chars.len())];
        let start = before
            .iter()
            .rposition(|c| c.is_whitespace())
            .map_or(0, |at| at + 1);
        (start < before.len() && before[start] == '#')
            .then(|| (start, before[start + 1..].iter().collect()))
    }

    /// Each calendar in the `#` list as it is shown, e.g. `Work > SFAP` (the top level: `HashList::top`)
    pub fn calendar_names(&self) -> Vec<String> {
        self.hash_list
            .calendars
            .iter()
            .map(|path| match &self.hash_list.top {
                Some(top) if path.is_empty() => top.clone(),
                _ => path.join(" > "),
            })
            .collect()
    }

    /// What the open `#` list shows: the calendars that match the `#` word at the cursor, best match first
    /// (just `#`: all of them, in the calendar list's order); `None` when it is closed or nothing matches
    pub fn suggestions(&self) -> Option<Vec<Found>> {
        if !self.hash_list.open {
            return None;
        }
        let (_, query) = self.hash_word()?;
        let found = search::search(&query, &self.calendar_names());
        (!found.is_empty()).then_some(found)
    }

    /// Picks the calendar on row `index` of the open `#` list: the new item goes into it (an edited item moves
    /// there on save; a calendar goes inside it), and the `#` word (with one space after it) is taken out of
    /// the typed text, the cursor where it was
    fn pick_calendar(&mut self, index: usize) {
        let (Some((start, _)), Some(found)) = (self.hash_word(), self.suggestions()) else {
            return;
        };
        let Some(path) = found
            .get(index)
            .map(|found| self.hash_list.calendars[found.index].clone())
        else {
            return;
        };
        let chars: Vec<char> = self.input.value().chars().collect();
        let mut end = start;
        while end < chars.len() && !chars[end].is_whitespace() {
            end += 1;
        }
        if chars.get(end) == Some(&' ') {
            end += 1;
        }
        let text = chars[..start].iter().chain(&chars[end..]).collect();
        self.input = Input::new(text).with_cursor(start);
        self.hash_list.open = false;
        match &mut self.target {
            Target::Edit { calendar, .. } => *calendar = Some(path),
            Target::AddCalendar { parent, .. } | Target::RenameCalendar { parent, .. } => {
                *parent = path
            }
            target => *target = Target::Add(Place::InCalendar(path)),
        }
    }

    /// A typing key (cursor movement and deleting: see tui-input): a change typed in a `#` word opens the `#`
    /// list at its best match; moving the cursor closes it
    fn type_key(&mut self, key: KeyEvent) {
        let Some(request) = to_input_request(&Event::Key(key)) else {
            return;
        };
        let changed = self.input.handle(request).is_some_and(|state| state.value);
        self.hash_list.open = changed && self.hash_word().is_some();
        self.hash_list.index = 0;
    }

    /// The line the typed text stands for: the typed text, with the hidden day put back (see `day`)
    pub fn line(&self) -> String {
        let text = self.input.value();
        match self.day {
            Some(day) => edit::show_day(text, day),
            None => text.to_string(),
        }
    }

    /// The priority the typed line has so far (`!`, `!!`, `!!!`), even before it can be saved (e.g. no title
    /// yet), so the typed text takes its color right away; `None` without one, or for a calendar's name
    pub fn priority(&self) -> Option<Priority> {
        match self.target {
            Target::Edit { .. } | Target::Add(_) => crate::tasks::parse(&self.line())
                .items
                .first()
                .and_then(|item| item.priority),
            Target::AddCalendar { .. } | Target::RenameCalendar { .. } => None,
        }
    }

    /// How the text typed so far reads (shown live on screen; if there is a problem it cannot be saved)
    pub fn check(&self) -> Result<Reading, edit::Problem> {
        match &self.target {
            Target::AddCalendar { parent, others }
            | Target::RenameCalendar { parent, others, .. } => {
                // The names of the calendars right inside `parent`
                let taken: Vec<String> = others
                    .iter()
                    .filter_map(|path| path.split_last())
                    .filter(|(_, above)| *above == parent.as_slice())
                    .map(|(name, _)| name.clone())
                    .collect();
                edit::check_calendar(self.input.value(), &taken).map(Reading::Calendar)
            }
            Target::Edit { .. } | Target::Add(_) => edit::check(&self.line()).map(Reading::Item),
        }
    }
}

/// The path of the calendar `item` is in (e.g. `Work`, `SFAP`); `None` outside any calendar
fn calendar_of(item: &Item) -> Option<Vec<String>> {
    item.group.as_ref().map(|heading| {
        heading
            .parents
            .iter()
            .chain([&heading.name])
            .cloned()
            .collect()
    })
}

impl App {
    /// `e` / `Enter`: opens the editor with the line of the chosen item in
    /// the agenda (or the tasks page), without indentation, cursor at the end. An item on the selected
    /// day (not on the tasks page) shows without its date (`edit::hide_day`, which leaves a multi-day
    /// range as it is)
    pub(super) fn start_editing(&mut self) {
        let Some(item) = self.chosen_item().cloned() else {
            return;
        };
        let line = item.line_text.trim();
        let day = Some(self.selected)
            .filter(|&day| self.view != super::View::Tasks && item.date == Some(day));
        let input =
            Input::new(day.map_or_else(|| line.to_string(), |day| edit::hide_day(line, day)));
        self.mode = Mode::Editor(Editor {
            input,
            target: Target::Edit {
                item,
                calendar: None,
            },
            day,
            hash_list: HashList {
                calendars: self.calendars(),
                ..HashList::default()
            },
        });
    }

    /// `a`: adds an item, prefilled with `- [ ] `; it goes on the selected day (the date is put back on
    /// save, see `Editor::day`), except on the tasks page (no date)
    ///
    /// Keyboard in the agenda, the tasks page or layout 2's popup: placed in the
    /// chosen item's group; on the calendar: placed in the group of the selected
    /// day's first item, or in the first calendar that is not a holiday calendar if that day has none (no
    /// such calendar: at the end of the file)
    pub(super) fn start_adding(&mut self) {
        let near = match self.focus {
            Focus::Agenda | Focus::Tasks | Focus::Day => self.chosen_item().cloned(),
            Focus::Calendar | Focus::Side => self
                .items_on(self.selected)
                .first()
                .map(|item| (*item).clone()),
        };
        let first_calendar = || {
            self.calendars()
                .into_iter()
                .find(|path| !self.is_holiday_calendar(path))
        };
        // On the tasks page with a calendar's heading row chosen: at the end of that calendar
        let heading = (self.focus == Focus::Tasks)
            .then(|| self.tasks_chosen_calendar())
            .flatten();
        let place = match (near, heading) {
            (_, Some(path)) => Place::InCalendar(path),
            (Some(item), None) => Place::InGroupOf(item),
            (None, None) => first_calendar().map_or(Place::End, Place::InCalendar),
        };
        // The tasks page has no selected day: no date
        let day = (self.view != super::View::Tasks).then_some(self.selected);
        self.mode = Mode::Editor(Editor {
            input: Input::new("- [ ] ".to_string()),
            target: Target::Add(place),
            day,
            hash_list: HashList {
                calendars: self.calendars(),
                ..HashList::default()
            },
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
            day: None,
            hash_list: HashList::default(),
        });
    }

    /// `a` in the calendar list: opens an empty editor for the new calendar's name (it goes at the top
    /// level, so it cannot take a top-level calendar's name)
    pub(super) fn start_adding_calendar(&mut self) {
        self.open_calendar_adder(Vec::new());
    }

    /// `A` in the calendar list: like `a`, but the new calendar goes inside the chosen one (at its end, one
    /// `#` more); nothing happens on 「Ungrouped」 (no heading), the 「隱藏做完的」 row, or a calendar that
    /// already has 6 `#`
    pub(super) fn start_adding_subcalendar(&mut self) {
        let Some(parent) = self
            .groups
            .get(self.group_index)
            .filter(|group| group.name.is_some())
            .map(super::Group::path)
        else {
            return;
        };
        if self.calendar_level(&parent) >= 6 {
            return;
        }
        self.open_calendar_adder(parent);
    }

    /// Opens an empty editor for a new calendar inside `parent` (empty = the top level); it cannot take the
    /// name of a calendar already there. The `#` list picks another place: 「最外層」 first, then every
    /// calendar that can still have one inside (fewer than 6 `#`)
    fn open_calendar_adder(&mut self, parent: Vec<String>) {
        let others = self.calendars();
        let calendars = std::iter::once(Vec::new())
            .chain(
                others
                    .iter()
                    .filter(|path| self.calendar_level(path) < 6)
                    .cloned(),
            )
            .collect();
        self.mode = Mode::Editor(Editor {
            input: Input::default(),
            target: Target::AddCalendar { parent, others },
            day: None,
            hash_list: HashList {
                calendars,
                top: Some(self.lang.texts().top_level.to_string()),
                ..HashList::default()
            },
        });
    }

    /// How many `#` the calendar with this path has (its last heading); 0 when there is none
    fn calendar_level(&self, path: &[String]) -> usize {
        self.headings
            .iter()
            .rev()
            .map(|line| &line.heading)
            .find(|heading| {
                heading
                    .parents
                    .iter()
                    .chain([&heading.name])
                    .eq(path.iter())
            })
            .map_or(0, |heading| heading.level)
    }

    /// `e` in the calendar list: opens the editor with the chosen calendar's heading text, without the `#`s
    /// (e.g. `US holidays type:holiday`, so a holiday calendar stays one unless `type:holiday` is taken
    /// out); 「Ungrouped」 has no heading, so nothing happens. It cannot take the name of another calendar
    /// at the same place (inside the same calendar). The `#` list picks a calendar to move it inside:
    /// 「最外層」 first, then every calendar except itself, the ones inside it, and the ones where a heading
    /// would get more than 6 `#`
    pub(super) fn start_renaming_calendar(&mut self) {
        let headings = self.chosen_calendar_headings();
        let Some(first) = headings.first() else {
            return;
        };
        let text = first.text.trim_start_matches('#').trim().to_string();
        let parent = first.heading.parents.clone();
        let own: Vec<String> = parent
            .iter()
            .chain([&first.heading.name])
            .cloned()
            .collect();
        let others: Vec<Vec<String>> = self
            .calendars()
            .into_iter()
            .filter(|path| *path != own)
            .collect();
        // How many levels the calendars inside it go below it
        let depth = self.calendar_depth(&own);
        let calendars = std::iter::once(Vec::new())
            .chain(
                others
                    .iter()
                    .filter(|path| !path.starts_with(&own))
                    .filter(|path| self.calendar_level(path) + 1 + depth <= 6)
                    .cloned(),
            )
            .collect();
        self.mode = Mode::Editor(Editor {
            input: Input::new(text),
            target: Target::RenameCalendar {
                headings,
                parent,
                others,
            },
            day: None,
            hash_list: HashList {
                calendars,
                top: Some(self.lang.texts().top_level.to_string()),
                ..HashList::default()
            },
        });
    }

    /// How many `#` deeper than the calendar with this path the deepest calendar inside it is (0 = none
    /// inside it)
    fn calendar_depth(&self, path: &[String]) -> usize {
        let level = self.calendar_level(path);
        self.headings
            .iter()
            .map(|line| &line.heading)
            .filter(|heading| {
                heading
                    .parents
                    .iter()
                    .chain([&heading.name])
                    .take(path.len())
                    .eq(path.iter())
            })
            .map(|heading| heading.level.saturating_sub(level))
            .max()
            .unwrap_or(0)
    }

    /// Keys while the editor is open: `Enter` saves, `Esc` cancels, `Ctrl+C` quits;
    /// every other key is typing (cursor movement and deleting: see tui-input). While the `#` list is open:
    /// up / down (`Ctrl+P` / `Ctrl+N`) choose a calendar, `Enter` / `Tab` pick it, `Esc` closes the list
    pub(super) fn handle_editor_key(&mut self, key: KeyEvent) {
        let control = key.modifiers.contains(KeyModifiers::CONTROL);
        if key.code == KeyCode::Char('c') && control {
            self.should_quit = true;
            return;
        }
        let Mode::Editor(editor) = &mut self.mode else {
            return;
        };
        if let Some(found) = editor.suggestions() {
            let index = editor.hash_list.index.min(found.len() - 1);
            match key.code {
                KeyCode::Up => editor.hash_list.index = index.saturating_sub(1),
                KeyCode::Char('p') if control => editor.hash_list.index = index.saturating_sub(1),
                KeyCode::Down => editor.hash_list.index = (index + 1).min(found.len() - 1),
                KeyCode::Char('n') if control => {
                    editor.hash_list.index = (index + 1).min(found.len() - 1);
                }
                KeyCode::Enter | KeyCode::Tab => editor.pick_calendar(index),
                KeyCode::Esc => editor.hash_list.open = false,
                _ => editor.type_key(key),
            }
            return;
        }
        match key.code {
            KeyCode::Enter => self.save_editor(),
            KeyCode::Esc => self.mode = Mode::Normal,
            _ => editor.type_key(key),
        }
    }

    /// Mouse while the editor is open: a click on a calendar in the `#` list picks it and the wheel moves
    /// up / down the list; a click outside the box and the list = cancel (`Esc`); everything else is ignored
    pub(super) fn handle_editor_mouse(&mut self, mouse: MouseEvent) {
        let editor_area = ui::editor_area(self);
        let list = self
            .editor()
            .and_then(|editor| ui::hash_list_area(self, editor));
        let Mode::Editor(editor) = &mut self.mode else {
            return;
        };
        let at = Position::new(mouse.column, mouse.row);
        let count = editor.suggestions().map_or(0, |found| found.len());
        match mouse.kind {
            MouseEventKind::Down(MouseButton::Left) => {
                if let Some((area, top)) = list
                    && area.contains(at)
                {
                    // A calendar's row (not the border)
                    if mouse.row > area.top() && mouse.row + 1 < area.bottom() {
                        editor.pick_calendar(top + usize::from(mouse.row - area.top() - 1));
                    }
                } else if !editor_area.contains(at) {
                    self.mode = Mode::Normal;
                }
            }
            MouseEventKind::ScrollUp if count > 0 => {
                editor.hash_list.index = editor.hash_list.index.min(count - 1).saturating_sub(1);
            }
            MouseEventKind::ScrollDown if count > 0 => {
                editor.hash_list.index = (editor.hash_list.index + 1).min(count - 1);
            }
            _ => {}
        }
    }

    /// Every calendar's path (its parents' names, then its own), in the calendar list's order
    fn calendars(&self) -> Vec<Vec<String>> {
        self.groups
            .iter()
            .filter(|group| group.name.is_some())
            .map(super::Group::path)
            .collect()
    }

    /// Whether the calendar with this path is a holiday calendar (`type:holiday`)
    fn is_holiday_calendar(&self, path: &[String]) -> bool {
        self.headings.iter().any(|line| {
            let heading = &line.heading;
            heading.holiday
                && heading
                    .parents
                    .iter()
                    .chain([&heading.name])
                    .eq(path.iter())
        })
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
        let new = editor.line().trim().to_string();
        // Opened and saved without touching it: nothing to write (even if putting the hidden day back
        // would move the date to another place in the line)
        let untouched = |item: &Item| {
            editor.day.map_or(new == item.line_text.trim(), |day| {
                editor.input.value().trim() == edit::hide_day(item.line_text.trim(), day)
            })
        };
        let target = editor.target.clone();
        match (target, reading) {
            (Target::AddCalendar { parent, .. }, Reading::Calendar(heading)) => {
                self.save_new_calendar(&parent, &new, &heading.name);
            }
            (
                Target::RenameCalendar {
                    headings, parent, ..
                },
                Reading::Calendar(heading),
            ) => {
                self.save_renamed_calendar(&headings, &parent, &new, &heading.name);
            }
            (Target::Edit { item, calendar }, _) => {
                // Another calendar picked in the `#` list (not the one it is in): it moves there
                let moving = calendar.filter(|path| Some(path) != calendar_of(&item).as_ref());
                let mut line = item.line;
                if moving.is_some() || !untouched(&item) {
                    let Some((path, text)) = self.read_tasks() else {
                        return;
                    };
                    let changed = match &moving {
                        Some(calendar) => edit::move_to_calendar(&text, &item, &new, calendar),
                        None => edit::replace(&text, &item, &new).map(|after| (after, item.line)),
                    };
                    match changed {
                        Ok((after, moved_to)) => {
                            if !self.write_tasks(&path, text, after, item.title) {
                                return;
                            }
                            line = moved_to;
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
                self.choose_line(line);
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

    /// Saves a new calendar: `# <new>` at the end of the task file, or inside `parent` (`edit::add_calendar_in`);
    /// the calendar list chooses it (the keyboard stays there). If the task file cannot be read or written,
    /// the editor stays open and the bottom row says why; if the parent is gone (changed elsewhere): reload
    /// and close the editor
    fn save_new_calendar(&mut self, parent: &[String], new: &str, name: &str) {
        let Some((path, text)) = self.read_tasks_for_adding() else {
            return;
        };
        let after = match edit::add_calendar_in(&text, parent, new) {
            Ok((after, _)) => after,
            Err(edit::Changed) => {
                self.mode = Mode::Normal;
                self.reload(&text);
                return;
            }
        };
        if !self.write_tasks(&path, text, after, name.to_string()) {
            return;
        }
        self.mode = Mode::Normal;
        if let Some(index) = self
            .groups
            .iter()
            .position(|group| group.parents == parent && group.name.as_deref() == Some(name))
        {
            self.group_index = index;
        }
    }
}

impl App {
    /// Saves a calendar's new name: each of its heading lines (`headings`) becomes `<its #s> <new>`; with
    /// another `parent` picked in the `#` list, it also moves inside that one (`edit::move_calendar`). The
    /// calendar stays chosen, and it and the calendars inside it keep their checked state (their paths
    /// change with the name and place). Nothing changed: nothing is written. If the task file cannot be read
    /// or written, the editor stays open and the bottom row says why; if it was changed elsewhere: reload
    /// and close the editor
    fn save_renamed_calendar(
        &mut self,
        headings: &[HeadingLine],
        parent: &[String],
        new: &str,
        name: &str,
    ) {
        let Some(first) = headings.first().map(|line| line.heading.clone()) else {
            return;
        };
        let Some((path, text)) = self.read_tasks() else {
            return;
        };
        let checked = self.checked_states();
        let changed = if parent == first.parents.as_slice() {
            edit::rename_calendar(&text, headings, new).map(Some)
        } else {
            edit::move_calendar(&text, headings, new, parent)
        };
        match changed {
            Ok(Some(after)) => {
                if after != text && !self.write_tasks(&path, text, after, name.to_string()) {
                    return;
                }
                self.mode = Mode::Normal;
                let old: Vec<String> = first.parents.iter().chain([&first.name]).cloned().collect();
                let renamed: Vec<String> =
                    parent.iter().cloned().chain([name.to_string()]).collect();
                self.carry_checked(&checked, &old, &renamed);
            }
            // Cannot go there (the `#` list does not offer such places): nothing is written
            Ok(None) => self.mode = Mode::Normal,
            Err(edit::Changed) => {
                self.mode = Mode::Normal;
                self.reload(&text);
            }
        }
    }
}

#[cfg(test)]
mod tests;
