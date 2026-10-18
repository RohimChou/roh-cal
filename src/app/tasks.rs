//! Tasks page (`3`): lists the whole task file by calendar (group),
//! subtasks as a tree (collapsed by default), undated items included;
//! `j` / `k` up / down, `l` / `h` expand / collapse (`+` / `-`
//! all), `L` / `H` indent / outdent, `J` / `K` move up / down

use super::App;
use crate::{edit, tasks::Item};
use ratatui::crossterm::event::KeyCode;

/// One row of the tasks page
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TaskRow {
    /// A calendar (group) heading
    Heading {
        /// The heading's name; `None` = the 「未分組」 part at the top of the file, before any heading
        name: Option<String>,
        /// How many calendars it is inside (for indentation): 0 = top level, 1 = e.g. `SFAP` in `Work`
        depth: usize,
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
    /// Every row of the tasks page, in file order: a heading row comes first whenever the calendar changes,
    /// with the calendars it is inside before it as a tree (like the side menu: `Work` with no items of its
    /// own still gets a row above `SFAP`, unless the heading right above is in `Work` too);
    /// a subtask is listed only if every level above it is expanded;
    /// calendars unchecked in the side menu are left out entirely
    pub fn tasks_rows(&self) -> Vec<TaskRow> {
        let mut rows = Vec::new();
        // Which group the last listed item was in (a new group gets a heading)
        let mut previous = None;
        // The path of the last heading listed, e.g. `Work > SFAP`: those calendars are already on screen
        let mut listed: Vec<&String> = Vec::new();
        for (index, item) in self.items.iter().enumerate() {
            if !self.is_shown(item) || !self.ancestors_expanded(item) {
                continue;
            }
            if previous != Some(&item.group) {
                let path: Vec<&String> = item
                    .group
                    .iter()
                    .flat_map(|heading| heading.parents.iter().chain([&heading.name]))
                    .collect();
                // Calendars shared with the last heading's path are listed already; the calendar itself
                // always gets a row (e.g. back in `Work` after `Work > SFAP`)
                let same = path
                    .iter()
                    .zip(&listed)
                    .take_while(|(name, other)| name == other)
                    .count()
                    .min(path.len().saturating_sub(1));
                if path.is_empty() {
                    rows.push(TaskRow::Heading {
                        name: None,
                        depth: 0,
                    });
                }
                for (depth, name) in path.iter().enumerate().skip(same) {
                    rows.push(TaskRow::Heading {
                        name: Some((*name).clone()),
                        depth,
                    });
                }
                listed = path;
                previous = Some(&item.group);
            }
            let children = self.items.iter().any(|other| other.parent == Some(index));
            rows.push(TaskRow::Item {
                index,
                children,
                // If all subtasks were deleted, it does not count as expanded even if it is still remembered
                expanded: children && self.expanded.contains(&item.line),
            });
        }
        rows
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
        let line_of = |row: &TaskRow| match row {
            TaskRow::Item { index, .. } => self.items.get(*index).map(|item| item.line),
            TaskRow::Heading { .. } => None,
        };
        rows.iter()
            .position(|row| self.tasks_line.is_some() && line_of(row) == self.tasks_line)
            .or_else(|| rows.iter().position(|row| line_of(row).is_some()))
    }

    /// The item chosen on the tasks page
    pub fn tasks_chosen(&self) -> Option<&Item> {
        let rows = self.tasks_rows();
        match rows.get(self.tasks_cursor(&rows)?)? {
            TaskRow::Item { index, .. } => self.items.get(*index),
            TaskRow::Heading { .. } => None,
        }
    }

    /// Keyboard on the tasks page: `j` / `k` next / previous item (headings skipped), `[` / `]` the previous /
    /// next calendar's first item,
    /// `l` / `h` expand / collapse subtasks, `+` / `-` expand all / collapse all,
    /// `L` / `H` indent / outdent, `J` / `K` move down / up;
    /// the keys that write the file are the same as in the agenda: `Space` / `x` check
    /// off, `e` / `Enter` edit, `a` add, `A` add a subtask, `d` delete, `r` reschedule
    pub(super) fn handle_tasks_key(&mut self, code: KeyCode) {
        match code {
            KeyCode::Char('j') | KeyCode::Down => self.choose_task(1),
            KeyCode::Char('k') | KeyCode::Up => self.choose_task(-1),
            KeyCode::Char('l') | KeyCode::Right => self.expand_task(),
            KeyCode::Char('h') | KeyCode::Left => self.collapse_task(),
            KeyCode::Char('+') => self.expand_all(),
            KeyCode::Char('-') => self.collapse_all(),
            KeyCode::Char('L') => self.indent_task(),
            KeyCode::Char('H') => self.outdent_task(),
            KeyCode::Char('J') => self.move_task(true),
            KeyCode::Char('K') => self.move_task(false),
            KeyCode::Char(' ' | 'x') => self.toggle_chosen(),
            KeyCode::Char('e') | KeyCode::Enter => self.start_editing(),
            KeyCode::Char('a') => self.start_adding(),
            KeyCode::Char('A') => self.start_adding_subtask(),
            KeyCode::Char('d') => self.ask_delete(),
            KeyCode::Char('r') => self.start_moving(),
            KeyCode::Char('[') => self.jump_calendar(-1),
            KeyCode::Char(']') => self.jump_calendar(1),
            _ => {}
        }
    }

    /// `[` / `]`: chooses the first item of the previous (`step` = -1) / next (1) calendar (heading) on the
    /// tasks page, like `[` `]` on the calendar go to the 1st of the previous / next month; stays put when
    /// there is no calendar that way. A heading with no items right under it (`Work` listed above
    /// `Work > SFAP`) is skipped
    fn jump_calendar(&mut self, step: isize) {
        let rows = self.tasks_rows();
        let Some(cursor) = self.tasks_cursor(&rows) else {
            return;
        };
        let headings: Vec<usize> = rows
            .iter()
            .enumerate()
            .filter(|&(position, row)| {
                matches!(row, TaskRow::Heading { .. })
                    && matches!(rows.get(position + 1), Some(TaskRow::Item { .. }))
            })
            .map(|(position, _)| position)
            .collect();
        // The calendar the chosen item is in: the last heading above it
        let Some(current) = headings.iter().rposition(|&heading| heading < cursor) else {
            return;
        };
        let Some(&heading) = current
            .checked_add_signed(step)
            .and_then(|target| headings.get(target))
        else {
            return;
        };
        let first = rows[heading + 1..]
            .iter()
            .take_while(|row| matches!(row, TaskRow::Item { .. }))
            .find_map(|row| match row {
                TaskRow::Item { index, .. } => self.items.get(*index),
                TaskRow::Heading { .. } => None,
            });
        if let Some(item) = first {
            self.tasks_line = Some(item.line);
        }
    }

    /// Chooses the `step`-th item forward (negative = backward),
    /// headings not counted; stops at the first / last item
    pub(super) fn choose_task(&mut self, step: isize) {
        let rows = self.tasks_rows();
        let lines: Vec<usize> = rows
            .iter()
            .filter_map(|row| match row {
                TaskRow::Item { index, .. } => self.items.get(*index).map(|item| item.line),
                TaskRow::Heading { .. } => None,
            })
            .collect();
        let Some(last) = lines.len().checked_sub(1) else {
            return;
        };
        let current = self
            .tasks_line
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

    /// Clicks row `row` of the tasks page (its position in `tasks_rows()`):
    /// one click chooses it; a click on `▸` / `▾` expands / collapses;
    /// clicks on headings are ignored
    pub(super) fn click_task(&mut self, row: usize, on_marker: bool) {
        let Some(TaskRow::Item {
            index, children, ..
        }) = self.tasks_rows().get(row).cloned()
        else {
            return;
        };
        let Some(line) = self.items.get(index).map(|item| item.line) else {
            return;
        };
        if on_marker && children {
            if !self.expanded.remove(&line) {
                self.expanded.insert(line);
            }
        } else if self.focus == super::Focus::Tasks && self.tasks_line == Some(line) {
            // Clicking the already chosen item again = check it off (same as the agenda)
            self.toggle_chosen();
            return;
        }
        self.tasks_line = Some(line);
        self.focus = super::Focus::Tasks;
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

#[cfg(test)]
mod tests;
