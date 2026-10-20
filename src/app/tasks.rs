//! Tasks page (`3`): lists the whole task file by calendar (group),
//! subtasks as a tree (collapsed by default), undated items included;
//! `j` / `k` up / down, `l` / `h` expand / collapse (`+` / `-`
//! all), `L` / `H` indent / outdent, `J` / `K` move up / down

use super::{App, Notice};
use crate::{
    edit, external_editor,
    tasks::{Heading, Item},
};
use color_eyre::Result;
use ratatui::{DefaultTerminal, crossterm::event::KeyCode};
use std::time::Instant;

/// One row of the tasks page
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TaskRow {
    /// A calendar (group) heading
    Heading {
        /// The heading's name; `None` = the 「未分組」 part at the top of the file, before any heading
        name: Option<String>,
        /// How many calendars it is inside (for indentation): 0 = top level, 1 = e.g. `SFAP` in `Work`
        depth: usize,
        /// Its heading's line in the file, so the row can be chosen (`App::tasks_line`); `None` = no line of
        /// its own (「未分組」, an outer calendar listed above one inside it, or no heading lines known)
        line: Option<usize>,
    },
    /// An item
    Item {
        /// Its position in `App::items`
        index: usize,
        /// Whether it has subtasks
        children: bool,
        /// Whether its subtasks are expanded (always `false` without subtasks)
        expanded: bool,
    },
}

impl App {
    /// Every row of the tasks page, in file order: every heading of a checked calendar (one with no items,
    /// or whose items are all hidden, too), each item under its heading; the calendars a heading is inside
    /// come before it as a tree (like the side menu: `Work` still gets a row above `SFAP` when the heading
    /// right above is not in `Work`). Without heading lines (`App::headings` empty), a heading row comes
    /// whenever an item's calendar changes; items before any heading are under 「未分組」.
    /// A subtask is listed only if every level above it is expanded;
    /// calendars unchecked in the side menu are left out entirely
    pub fn tasks_rows(&self) -> Vec<TaskRow> {
        let mut rows = Vec::new();
        // The calendar of the last heading row (`None` inside = 「未分組」); `None` = no heading row yet
        let mut last: Option<Option<&Heading>> = None;
        // The path of the last heading listed, e.g. `Work > SFAP`: those calendars are already on screen
        let mut listed: Vec<&String> = Vec::new();
        let mut headings = self.headings.iter().peekable();
        for (index, item) in self.items.iter().enumerate() {
            // The headings above this item, each one listed (a calendar with no items gets its row too)
            while let Some(line) = headings.next_if(|line| line.line < item.line) {
                if self.heading_shown(&line.heading) {
                    push_heading(&mut rows, &mut listed, Some(&line.heading), Some(line.line));
                    last = Some(Some(&line.heading));
                }
            }
            if !self.is_shown(item) || !self.ancestors_expanded(item) {
                continue;
            }
            // No heading row for its calendar yet (e.g. no heading lines known, or before any heading)
            if last != Some(item.group.as_ref()) {
                push_heading(&mut rows, &mut listed, item.group.as_ref(), None);
                last = Some(item.group.as_ref());
            }
            let children = self.items.iter().any(|other| other.parent == Some(index));
            rows.push(TaskRow::Item {
                index,
                children,
                // If all subtasks were deleted, it does not count as expanded even if it is still remembered
                expanded: children && self.expanded.contains(&item.line),
            });
        }
        // The headings after the last item
        for line in headings {
            if self.heading_shown(&line.heading) {
                push_heading(&mut rows, &mut listed, Some(&line.heading), Some(line.line));
            }
        }
        rows
    }

    /// Whether the calendar of this heading is shown: it and every calendar it is inside are checked
    fn heading_shown(&self, heading: &Heading) -> bool {
        let path: Vec<&String> = heading.parents.iter().chain([&heading.name]).collect();
        self.groups
            .iter()
            .filter(|group| {
                let outer = group.path();
                group.name.is_some()
                    && outer.len() <= path.len()
                    && outer.iter().eq(path[..outer.len()].iter().copied())
            })
            .all(|group| group.shown)
    }

    /// Whether every level above `item` (parent, parent's parent, ...) is expanded
    fn ancestors_expanded(&self, item: &Item) -> bool {
        let mut parent = item.parent;
        while let Some(index) = parent {
            let Some(ancestor) = self.items.get(index) else {
                return false;
            };
            if !self.expanded.contains(&ancestor.line) {
                return false;
            }
            parent = ancestor.parent;
        }
        true
    }

    /// The chosen row on the tasks page (its position in `tasks_rows()`): the item on line `tasks_line`;
    /// if that item is not on screen (e.g. just collapsed, or its
    /// calendar is unchecked) or nothing was chosen yet, the first item
    pub fn tasks_cursor(&self, rows: &[TaskRow]) -> Option<usize> {
        let item_line = |row: &TaskRow| match row {
            TaskRow::Item { index, .. } => self.items.get(*index).map(|item| item.line),
            TaskRow::Heading { .. } => None,
        };
        rows.iter()
            .position(|row| self.tasks_line.is_some() && self.row_line(row) == self.tasks_line)
            .or_else(|| rows.iter().position(|row| item_line(row).is_some()))
    }

    /// The file line of a row that can be chosen: an item's line, or a heading row's own heading line
    fn row_line(&self, row: &TaskRow) -> Option<usize> {
        match row {
            TaskRow::Item { index, .. } => self.items.get(*index).map(|item| item.line),
            TaskRow::Heading { line, .. } => *line,
        }
    }

    /// The path of the calendar whose heading row is chosen on the tasks page (e.g. `Work`, `SFAP`); `None`
    /// when an item is chosen
    pub fn tasks_chosen_calendar(&self) -> Option<Vec<String>> {
        let rows = self.tasks_rows();
        let TaskRow::Heading {
            line: Some(line), ..
        } = rows.get(self.tasks_cursor(&rows)?)?
        else {
            return None;
        };
        self.headings
            .iter()
            .find(|heading| heading.line == *line)
            .map(|heading| {
                let heading = &heading.heading;
                heading
                    .parents
                    .iter()
                    .chain([&heading.name])
                    .cloned()
                    .collect()
            })
    }

    /// The item chosen on the tasks page
    pub fn tasks_chosen(&self) -> Option<&Item> {
        let rows = self.tasks_rows();
        match rows.get(self.tasks_cursor(&rows)?)? {
            TaskRow::Item { index, .. } => self.items.get(*index),
            TaskRow::Heading { .. } => None,
        }
    }

    /// The file line (from 1) of the row chosen on the tasks page: an item's, or a heading row's own heading
    /// line; `None` when nothing can be chosen (an empty file)
    fn tasks_chosen_line(&self) -> Option<usize> {
        let rows = self.tasks_rows();
        self.row_line(rows.get(self.tasks_cursor(&rows)?)?)
    }

    /// `o`: opens the task file in the user's editor on the chosen row's line, and comes back when the editor
    /// quits; what was changed there shows up right away (the main loop reloads a file changed elsewhere).
    /// Without a task file (tests) it does nothing; an editor that cannot be started is said on the bottom row
    pub(super) fn open_in_editor(&mut self, terminal: &mut DefaultTerminal) -> Result<()> {
        let Some(path) = self.tasks_path.clone() else {
            return Ok(());
        };
        // A first task file at the default location: create its folder, or the editor cannot save there
        // (if that fails, the editor says why when saving)
        if let Some(dir) = path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let mut command = external_editor::command(
            std::env::var_os("VISUAL"),
            std::env::var_os("EDITOR"),
            &path,
            self.tasks_chosen_line(),
        );
        if !external_editor::open(terminal, &mut command)? {
            let program = command.get_program().to_string_lossy().into_owned();
            self.notice = Some(Notice::NoEditor(program));
        }
        Ok(())
    }

    /// Keyboard on the tasks page: `j` / `k` next / previous row (an item, or a calendar's heading: `a` there adds
    /// at the end of that calendar), `gg` / `G` the first / last row (like vim; `after_g`: the key before this
    /// one was `g`), `[` / `]` the previous / next calendar's heading row,
    /// `l` / `h` expand / collapse subtasks, `+` / `-` expand all / collapse all,
    /// `L` / `H` indent / outdent, `J` / `K` move down / up;
    /// the keys that write the file are the same as in the agenda: `Space` / `x` check
    /// off, `e` / `Enter` edit, `a` add, `A` add a subtask, `d` delete, `r` reschedule;
    /// `o` opens the task file in the user's editor on the chosen row's line
    pub(super) fn handle_tasks_key(&mut self, code: KeyCode, after_g: bool) {
        match code {
            KeyCode::Char('j') | KeyCode::Down => self.choose_task(1),
            KeyCode::Char('k') | KeyCode::Up => self.choose_task(-1),
            KeyCode::Char('g') if after_g => self.choose_task(isize::MIN),
            KeyCode::Char('g') => self.pending_g = true,
            KeyCode::Char('G') => self.choose_task(isize::MAX),
            KeyCode::Char('l') | KeyCode::Right => self.expand_task(),
            KeyCode::Char('h') | KeyCode::Left => self.collapse_task(),
            KeyCode::Char('+') => self.expand_all(),
            KeyCode::Char('-') => self.collapse_all(),
            KeyCode::Char('L') => self.indent_task(),
            KeyCode::Char('H') => self.outdent_task(),
            KeyCode::Char('J') => self.move_task(true),
            KeyCode::Char('K') => self.move_task(false),
            KeyCode::Char(' ' | 'x') => self.toggle_chosen(),
            KeyCode::Char('/') => self.toggle_chosen_progress(),
            KeyCode::Char('e') | KeyCode::Enter => self.start_editing(),
            KeyCode::Char('a') => self.start_adding(),
            KeyCode::Char('A') => self.start_adding_subtask(),
            KeyCode::Char('d') => self.ask_delete(),
            KeyCode::Char('r') => self.start_moving(),
            KeyCode::Char('[') => self.jump_calendar(-1),
            KeyCode::Char(']') => self.jump_calendar(1),
            KeyCode::Char('o') => self.open_editor = true,
            _ => {}
        }
    }

    /// `[` / `]`: chooses the heading row of the previous (`step` = -1) / next (1) calendar on the tasks
    /// page, like `[` `]` on the calendar go to the previous / next month; stays put when there is no
    /// calendar that way. Only heading rows with a line of their own count (the ones `j` / `k` can choose
    /// too; not 「未分組」 or an outer calendar listed above one inside it)
    fn jump_calendar(&mut self, step: isize) {
        let rows = self.tasks_rows();
        let Some(cursor) = self.tasks_cursor(&rows) else {
            return;
        };
        // (position in `rows`, line in the file) of each heading row that can be chosen
        let headings: Vec<(usize, usize)> = rows
            .iter()
            .enumerate()
            .filter_map(|(position, row)| match row {
                TaskRow::Heading {
                    line: Some(line), ..
                } => Some((position, *line)),
                _ => None,
            })
            .collect();
        // The calendar the chosen row is in: the last heading at or above it; above the first heading
        // (e.g. in 「未分組」), `]` goes to the first one
        let current = headings
            .iter()
            .rposition(|&(position, _)| position <= cursor);
        let target = match current {
            Some(current) => current.checked_add_signed(step),
            None if step > 0 => Some(0),
            None => None,
        };
        if let Some(&(_, line)) = target.and_then(|target| headings.get(target)) {
            self.tasks_line = Some(line);
        }
    }

    /// Chooses the `step`-th row forward (negative = backward) from the chosen one: items and calendars'
    /// heading rows (not the ones without a heading line of their own); stops at the first / last
    pub(super) fn choose_task(&mut self, step: isize) {
        let rows = self.tasks_rows();
        let lines: Vec<usize> = rows.iter().filter_map(|row| self.row_line(row)).collect();
        let Some(last) = lines.len().checked_sub(1) else {
            return;
        };
        let chosen = self
            .tasks_cursor(&rows)
            .and_then(|cursor| self.row_line(&rows[cursor]));
        let current = chosen
            .and_then(|line| lines.iter().position(|&other| other == line))
            .unwrap_or(0);
        self.tasks_line = Some(lines[current.saturating_add_signed(step).min(last)]);
    }

    /// Whether the item on line `line` of the file has subtasks
    fn has_children(&self, line: usize) -> bool {
        self.items
            .iter()
            .position(|item| item.line == line)
            .is_some_and(|index| self.items.iter().any(|other| other.parent == Some(index)))
    }

    /// `l`: if the chosen item has subtasks and is collapsed,
    /// expand it; if already expanded, jump to its first subtask
    fn expand_task(&mut self) {
        let Some(line) = self.tasks_chosen().map(|item| item.line) else {
            return;
        };
        if !self.has_children(line) {
            return;
        }
        if self.expanded.insert(line) {
            return;
        }
        self.choose_task(1);
    }

    /// `h`: if the chosen item is expanded, collapse it;
    /// otherwise jump to its parent (like VS Code's file tree)
    fn collapse_task(&mut self) {
        let Some((line, parent)) = self.tasks_chosen().map(|item| (item.line, item.parent)) else {
            return;
        };
        // Remembered as expanded but with no subtasks (all deleted)
        // is a stale record: clear it and still jump to the parent
        if self.expanded.remove(&line) && self.has_children(line) {
            return;
        }
        if let Some(parent) = parent.and_then(|index| self.items.get(index)) {
            self.tasks_line = Some(parent.line);
        }
    }

    /// `+`: expands every item that has subtasks
    fn expand_all(&mut self) {
        let parents: Vec<usize> = self
            .items
            .iter()
            .filter_map(|item| item.parent)
            .filter_map(|index| self.items.get(index))
            .map(|parent| parent.line)
            .collect();
        self.expanded.extend(parents);
    }

    /// `-`: collapses everything; if the chosen item is a subtask
    /// (it would be hidden), choose its top-level item instead
    fn collapse_all(&mut self) {
        let top = self.tasks_chosen().map(|chosen| {
            let mut item = chosen;
            while let Some(parent) = item.parent.and_then(|index| self.items.get(index)) {
                item = parent;
            }
            item.line
        });
        if let Some(line) = top {
            self.tasks_line = Some(line);
        }
        self.expanded.clear();
    }

    /// `L`: indents the chosen item (with its subtasks) one level, making it a subtask of the
    /// item above at the same level (indent = that item's indent + 2 spaces, same as `A`);
    /// does nothing if there is no item above at the same level (the first one in that group, at that level)
    fn indent_task(&mut self) {
        let Some(item) = self.tasks_chosen().cloned() else {
            return;
        };
        let Some(above) = self.sibling_above(&item).cloned() else {
            return;
        };
        let to = format!("{}  ", edit::indent_of(&above.line_text));
        // Expand the new parent so the indented item stays visible
        self.reindent_task(&item, &above, &to, above.line);
    }

    /// `H`: outdents the chosen item (with its subtasks) one level, aligned with its
    /// parent; the items below it at the same level become its subtasks (same as Obsidian).
    /// Does nothing if it is already at the top level
    fn outdent_task(&mut self) {
        let Some(item) = self.tasks_chosen().cloned() else {
            return;
        };
        let Some(parent) = item.parent.and_then(|index| self.items.get(index)).cloned() else {
            return;
        };
        let to = edit::indent_of(&parent.line_text).to_string();
        // Expand it: the items below that become its subtasks stay visible
        self.reindent_task(&item, &parent, &to, item.line);
    }

    /// The item above `item` at the same level (same parent, under the same heading;
    /// anything in between is that item's subtasks); `None` if there is none
    fn sibling_above(&self, item: &Item) -> Option<&Item> {
        let index = self
            .items
            .iter()
            .position(|other| other.line == item.line)?;
        self.items[..index]
            .iter()
            .rev()
            .find(|other| other.depth <= item.depth)
            .filter(|other| {
                other.depth == item.depth
                    && other.parent == item.parent
                    && other.section == item.section
            })
    }

    /// The item below `item` at the same level (same parent, under the same heading;
    /// anything in between is `item`'s own subtasks); `None` if there is none
    fn sibling_below(&self, item: &Item) -> Option<&Item> {
        let index = self
            .items
            .iter()
            .position(|other| other.line == item.line)?;
        self.items[index + 1..]
            .iter()
            .find(|other| other.depth <= item.depth)
            .filter(|other| {
                other.depth == item.depth
                    && other.parent == item.parent
                    && other.section == item.section
            })
    }

    /// `J` / `K`: swaps the chosen item (with its subtasks) with the item below
    /// (`down`) / above at the same level, and writes back to the task file;
    /// the last / first item at that level does not move (it never moves to another parent
    /// or group). It stays chosen after the move (line numbers follow `edit::Swap`)
    fn move_task(&mut self, down: bool) {
        let Some(item) = self.tasks_chosen().cloned() else {
            return;
        };
        let other = if down {
            self.sibling_below(&item)
        } else {
            self.sibling_above(&item)
        };
        let Some(other) = other.cloned() else {
            return;
        };
        let (upper, lower) = if down {
            (&item, &other)
        } else {
            (&other, &item)
        };
        let Some((path, text)) = self.read_tasks() else {
            return;
        };
        // Even right after opening, with nothing chosen yet (the first item
        // by default), remember it first so the choice can follow the move
        self.tasks_line = Some(item.line);
        match edit::swap(&text, upper, lower) {
            Ok((after, swap)) => {
                self.write_tasks_swapped(&path, text, after, item.title.clone(), Some(swap));
            }
            Err(edit::Changed) => self.reload(&text),
        }
    }

    /// Changes the indentation of `item`'s block to `to` (computed from the `anchor` line)
    /// and writes back to the task file; if written, expands the item on line `expand`.
    /// Reloads if these two lines were changed elsewhere
    fn reindent_task(&mut self, item: &Item, anchor: &Item, to: &str, expand: usize) {
        let Some((path, text)) = self.read_tasks() else {
            return;
        };
        match edit::reindent(&text, item, anchor, to) {
            Ok(after) => {
                if self.write_tasks(&path, text, after, item.title.clone()) {
                    self.expanded.insert(expand);
                }
            }
            Err(edit::Changed) => self.reload(&text),
        }
    }

    /// Clicks row `row` of the tasks page (its position in `tasks_rows()`) at time `now`:
    /// one click chooses it, a double click checks it off; a click on `▸` / `▾` expands / collapses;
    /// a click on a calendar's heading row only chooses it
    pub(super) fn click_task(&mut self, row: usize, on_marker: bool, now: Instant) {
        let (index, children) = match self.tasks_rows().get(row).cloned() {
            Some(TaskRow::Item {
                index, children, ..
            }) => (index, children),
            // A calendar's heading row: just choose it
            Some(TaskRow::Heading {
                line: Some(line), ..
            }) => {
                self.tasks_line = Some(line);
                self.focus = super::Focus::Tasks;
                return;
            }
            _ => return,
        };
        let Some(line) = self.items.get(index).map(|item| item.line) else {
            return;
        };
        // On `▸` / `▾`: expand / collapse
        let on_toggle = on_marker && children;
        if on_toggle && !self.expanded.remove(&line) {
            self.expanded.insert(line);
        }
        self.tasks_line = Some(line);
        self.focus = super::Focus::Tasks;
        // A double click on the item (not on `▸`) = check it off (same as the agenda)
        if !on_toggle && self.is_double_click(super::ClickTarget::Item(line), now) {
            self.toggle_chosen();
        }
    }

    /// After a write, the tasks page chooses the item on line `line`;
    /// if it is under a collapsed parent, expand every level above it
    pub(super) fn choose_task_line(&mut self, line: usize) {
        self.tasks_line = Some(line);
        let mut parent = self
            .items
            .iter()
            .find(|item| item.line == line)
            .and_then(|item| item.parent);
        while let Some(index) = parent {
            let Some(ancestor) = self.items.get(index) else {
                break;
            };
            self.expanded.insert(ancestor.line);
            parent = ancestor.parent;
        }
    }

    /// After a delete, the tasks page chooses the item at row `row` (its position in
    /// `tasks_rows()`) or the first item after it; if there is none after it, the last item
    pub(super) fn choose_task_row_near(&mut self, row: usize) {
        let rows = self.tasks_rows();
        let line_of = |row: &TaskRow| match row {
            TaskRow::Item { index, .. } => self.items.get(*index).map(|item| item.line),
            TaskRow::Heading { .. } => None,
        };
        self.tasks_line = rows
            .iter()
            .skip(row)
            .find_map(line_of)
            .or_else(|| rows.iter().rev().find_map(line_of));
    }
}

/// Puts the heading row of `heading`'s calendar (`None` = 「未分組」) on `rows`, after rows for the calendars it is
/// inside that the last heading listed (`listed`, its path) was not in; the calendar itself always gets a
/// row (e.g. back in `Work` after `Work > SFAP`), with its heading's `line` in the file (the outer ones none)
fn push_heading<'a>(
    rows: &mut Vec<TaskRow>,
    listed: &mut Vec<&'a String>,
    heading: Option<&'a Heading>,
    line: Option<usize>,
) {
    let path: Vec<&String> = heading
        .iter()
        .flat_map(|heading| heading.parents.iter().chain([&heading.name]))
        .collect();
    if path.is_empty() {
        rows.push(TaskRow::Heading {
            name: None,
            depth: 0,
            line: None,
        });
    }
    let same = path
        .iter()
        .zip(listed.iter())
        .take_while(|(name, other)| name == other)
        .count()
        .min(path.len().saturating_sub(1));
    for (depth, name) in path.iter().enumerate().skip(same) {
        rows.push(TaskRow::Heading {
            name: Some((*name).clone()),
            depth,
            line: (depth + 1 == path.len()).then_some(line).flatten(),
        });
    }
    *listed = path;
}

#[cfg(test)]
mod tests;
