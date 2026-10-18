//! Tasks page (`4`): lists the whole task file by calendar (group),
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
        /// How many `#` (for indentation)
        level: usize,
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
    /// Every row of the tasks page, in file order: a heading row comes first whenever the calendar changes;
    /// a subtask is listed only if every level above it is expanded;
    /// calendars unchecked in the side menu are left out entirely
    pub fn tasks_rows(&self) -> Vec<TaskRow> {
        let mut rows = Vec::new();
        // Which group the last listed item was in (a new group gets a heading)
        let mut previous = None;
        for (index, item) in self.items.iter().enumerate() {
            if !self.is_shown(item) || !self.ancestors_expanded(item) {
                continue;
            }
            if previous != Some(&item.group) {
                rows.push(TaskRow::Heading {
                    name: item.group.as_ref().map(|heading| heading.name.clone()),
                    level: item.group.as_ref().map_or(1, |heading| heading.level),
                });
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
    /// there is no calendar that way
    fn jump_calendar(&mut self, step: isize) {
        let rows = self.tasks_rows();
        let Some(cursor) = self.tasks_cursor(&rows) else {
            return;
        };
        let headings: Vec<usize> = rows
            .iter()
            .enumerate()
            .filter(|(_, row)| matches!(row, TaskRow::Heading { .. }))
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
mod tests {
    use super::super::{Focus, View};
    use super::*;
    use crate::texts::Lang;
    use jiff::civil::date;
    use ratatui::crossterm::event::KeyEvent;

    /// An app on the tasks page (today 2026/10/17) whose task file is `text`
    fn tasks_app(text: &str) -> App {
        let mut app = App::new(
            Lang::ZhTw,
            date(2026, 10, 17),
            crate::tasks::parse(text).items,
        );
        app.handle_key(KeyEvent::from(KeyCode::Char('4')));
        app
    }

    /// Presses one key
    fn press(app: &mut App, code: KeyCode) {
        app.handle_key(KeyEvent::from(code));
    }

    #[test]
    fn brackets_jump_to_the_previous_or_next_calendar() {
        let mut app = tasks_app("# A\n- a1\n- a2\n# B\n- b1\n## C\n- c1\n");
        let chosen = |app: &App| app.tasks_chosen().map(|item| item.title.clone());
        // From a2 (A's second item): ] = B's first item, then C's (a `##` heading is a calendar too)
        press(&mut app, KeyCode::Char('j'));
        press(&mut app, KeyCode::Char(']'));
        assert_eq!(chosen(&app).as_deref(), Some("b1"));
        press(&mut app, KeyCode::Char(']'));
        assert_eq!(chosen(&app).as_deref(), Some("c1"));
        // No calendar after C: stays
        press(&mut app, KeyCode::Char(']'));
        assert_eq!(chosen(&app).as_deref(), Some("c1"));
        // [ goes back one calendar at a time, to its first item; stays at the first calendar
        press(&mut app, KeyCode::Char('['));
        assert_eq!(chosen(&app).as_deref(), Some("b1"));
        press(&mut app, KeyCode::Char('['));
        press(&mut app, KeyCode::Char('['));
        assert_eq!(chosen(&app).as_deref(), Some("a1"));
        // The calendar's date did not move (on the tasks page [ ] are not months)
        assert_eq!(app.selected, date(2026, 10, 17));
    }

    /// What each row looks like: heading `#name`, item `title` (two spaces per level
    /// in front; an item with subtasks gets `+` when collapsed, `-` when expanded)
    fn shape(app: &App) -> Vec<String> {
        app.tasks_rows()
            .iter()
            .map(|row| match row {
                TaskRow::Heading { name, .. } => format!("#{}", name.as_deref().unwrap_or("")),
                TaskRow::Item {
                    index,
                    children,
                    expanded,
                } => {
                    let item = &app.items[*index];
                    let mark = match (children, expanded) {
                        (false, _) => "",
                        (true, false) => "+",
                        (true, true) => "-",
                    };
                    format!("{}{mark}{}", "  ".repeat(item.depth), item.title)
                }
            })
            .collect()
    }

    /// The tasks page tests' file: one ungrouped item, "Work" with three levels of subtasks,
    /// and "Personal"
    const TREE: &str = "- Renew passport\n# Work\n\
                        - [ ] Design doc @2026-10-16\n  - [ ] Research\n    - [ ] Read RFCs\n  - [x] Outline\n\
                        - [ ] Triage bugs\n# Personal\n- Go hiking @2026-10-18\n";

    #[test]
    fn rows_list_the_whole_file_by_calendar_with_subtasks_collapsed() {
        let app = tasks_app(TREE);
        assert_eq!((app.view, app.focus), (View::Tasks, Focus::Tasks));
        // Undated items are listed too; subtasks are collapsed by default
        assert_eq!(
            shape(&app),
            [
                "#",
                "Renew passport",
                "#Work",
                "+Design doc",
                "Triage bugs",
                "#Personal",
                "Go hiking"
            ]
        );
    }

    #[test]
    fn l_and_h_expand_and_collapse_like_a_file_tree() {
        let mut app = tasks_app(TREE);
        // The first item is "Renew passport"; move down to "Design doc" and press l to expand
        press(&mut app, KeyCode::Char('j'));
        assert_eq!(
            app.tasks_chosen().map(|item| item.title.as_str()),
            Some("Design doc")
        );
        press(&mut app, KeyCode::Char('l'));
        assert_eq!(
            shape(&app),
            [
                "#",
                "Renew passport",
                "#Work",
                "-Design doc",
                "  +Research",
                "  Outline",
                "Triage bugs",
                "#Personal",
                "Go hiking"
            ]
        );
        // Press l when already expanded: jump to the first subtask; l again expands it
        press(&mut app, KeyCode::Char('l'));
        assert_eq!(
            app.tasks_chosen().map(|item| item.title.as_str()),
            Some("Research")
        );
        press(&mut app, KeyCode::Right);
        assert!(shape(&app).contains(&"    Read RFCs".to_string()));
        // h: an expanded item collapses first; h again jumps
        // back to the parent; h once more collapses the parent
        press(&mut app, KeyCode::Char('h'));
        assert!(!shape(&app).contains(&"    Read RFCs".to_string()));
        press(&mut app, KeyCode::Char('h'));
        assert_eq!(
            app.tasks_chosen().map(|item| item.title.as_str()),
            Some("Design doc")
        );
        press(&mut app, KeyCode::Left);
        assert_eq!(shape(&app)[3], "+Design doc");
        // l on an item without subtasks: nothing happens
        press(&mut app, KeyCode::Char('j'));
        press(&mut app, KeyCode::Char('l'));
        assert_eq!(
            app.tasks_chosen().map(|item| item.title.as_str()),
            Some("Triage bugs")
        );
    }

    #[test]
    fn plus_and_minus_expand_and_collapse_everything() {
        let mut app = tasks_app(TREE);
        // +: every level expands, and the same item ("Renew passport") stays chosen
        press(&mut app, KeyCode::Char('+'));
        assert_eq!(
            shape(&app),
            [
                "#",
                "Renew passport",
                "#Work",
                "-Design doc",
                "  -Research",
                "    Read RFCs",
                "  Outline",
                "Triage bugs",
                "#Personal",
                "Go hiking"
            ]
        );
        assert_eq!(
            app.tasks_chosen().map(|item| item.title.as_str()),
            Some("Renew passport")
        );
        // Move down to the deepest item, "Read RFCs"; -: everything
        // collapses and its top-level item "Design doc" is chosen instead
        for _ in 0..3 {
            press(&mut app, KeyCode::Char('j'));
        }
        assert_eq!(
            app.tasks_chosen().map(|item| item.title.as_str()),
            Some("Read RFCs")
        );
        press(&mut app, KeyCode::Char('-'));
        assert_eq!(
            shape(&app),
            [
                "#",
                "Renew passport",
                "#Work",
                "+Design doc",
                "Triage bugs",
                "#Personal",
                "Go hiking"
            ]
        );
        assert_eq!(app.tasks_line, Some(3));
    }

    #[test]
    fn j_and_k_skip_headings_and_stop_at_the_ends() {
        let mut app = tasks_app(TREE);
        for _ in 0..10 {
            press(&mut app, KeyCode::Char('j'));
        }
        assert_eq!(
            app.tasks_chosen().map(|item| item.title.as_str()),
            Some("Go hiking")
        );
        press(&mut app, KeyCode::Up);
        assert_eq!(
            app.tasks_chosen().map(|item| item.title.as_str()),
            Some("Triage bugs")
        );
        for _ in 0..10 {
            press(&mut app, KeyCode::Char('k'));
        }
        assert_eq!(
            app.tasks_chosen().map(|item| item.title.as_str()),
            Some("Renew passport")
        );
    }

    #[test]
    fn unchecked_calendars_leave_the_tasks_page() {
        let mut app = tasks_app(TREE);
        app.groups[1].shown = false;
        assert_eq!(
            shape(&app),
            ["#", "Renew passport", "#Personal", "Go hiking"]
        );
    }

    /// A task file in a temp folder (content `text`) and an
    /// app with the tasks page open; call `clean_up` when done
    fn tasks_app_with_file(name: &str, text: &str) -> (App, std::path::PathBuf) {
        let (mut app, path) = super::super::write::tests::app_with_task_file(name, text);
        press(&mut app, KeyCode::Char('4'));
        (app, path)
    }

    /// Types text into the editor one character at a time
    fn type_text(app: &mut App, text: &str) {
        for c in text.chars() {
            press(app, KeyCode::Char(c));
        }
    }

    #[test]
    fn the_writing_keys_work_on_the_tasks_page() {
        use super::super::write::tests::clean_up;
        let text = "# Work\n- [ ] Design doc\n  - [ ] Research\n- [ ] Weekly update @2026-10-16\n\
                    # Personal\n- Go hiking @2026-10-18\n";
        let (mut app, path) = tasks_app_with_file("tasks-keys", text);
        let read = || std::fs::read_to_string(&path).expect("read");
        // Space: undated items can be checked off too; the same item stays chosen
        press(&mut app, KeyCode::Char(' '));
        assert!(read().contains("- [x] Design doc\n"));
        assert_eq!(
            app.tasks_chosen().map(|item| item.title.as_str()),
            Some("Design doc")
        );
        // A: the subtask goes below its existing subtasks; the parent expands and the new item is chosen
        press(&mut app, KeyCode::Char('A'));
        type_text(&mut app, "Outline");
        press(&mut app, KeyCode::Enter);
        assert!(read().contains("  - [ ] Research\n  - [ ] Outline\n- [ ] Weekly update"));
        assert!(app.expanded.contains(&2));
        assert_eq!(
            app.tasks_chosen().map(|item| item.title.as_str()),
            Some("Outline")
        );
        // d, y: deleted; the next item ("Weekly update", now on line 4) is chosen
        press(&mut app, KeyCode::Char('d'));
        press(&mut app, KeyCode::Char('y'));
        assert!(!read().contains("Outline"));
        assert_eq!(
            app.tasks_chosen().map(|item| item.title.as_str()),
            Some("Weekly update")
        );
        // u: "Outline" comes back; "Weekly update" moves down one line along with it and is still chosen
        press(&mut app, KeyCode::Char('u'));
        assert!(read().contains("Outline"));
        assert_eq!(app.tasks_line, Some(5));
        assert_eq!(
            app.tasks_chosen().map(|item| item.title.as_str()),
            Some("Weekly update")
        );
        // a: no date on the tasks page; added at the end of "Work", and the new item is chosen
        press(&mut app, KeyCode::Char('a'));
        assert_eq!(
            app.editor().map(|editor| editor.input.value()),
            Some("- [ ] ")
        );
        type_text(&mut app, "Sprint planning @2026-10-20");
        press(&mut app, KeyCode::Enter);
        assert!(read().contains(
            "- [ ] Weekly update @2026-10-16\n- [ ] Sprint planning @2026-10-20\n# Personal"
        ));
        assert_eq!(
            app.tasks_chosen().map(|item| item.title.as_str()),
            Some("Sprint planning")
        );
        // e: edits the chosen item
        press(&mut app, KeyCode::Char('e'));
        type_text(&mut app, " 10:00");
        press(&mut app, KeyCode::Enter);
        assert!(read().contains("- [ ] Sprint planning @2026-10-20 10:00\n"));
        assert_eq!((app.view, app.focus), (View::Tasks, Focus::Tasks));
        clean_up(&path);
    }

    #[test]
    fn shift_l_and_h_indent_and_outdent_with_the_subtasks() {
        use super::super::write::tests::clean_up;
        let text = "# Work\n- [ ] Design doc\n  - [ ] Research\n  - [ ] Outline\n\
                    - [ ] Weekly update\n  - [ ] Metrics\n# Personal\n- Go hiking\n";
        let (mut app, path) = tasks_app_with_file("tasks-indent", text);
        let read = || std::fs::read_to_string(&path).expect("read");
        let chosen = |app: &App| app.tasks_chosen().map(|item| item.title.clone());
        // "Design doc" is the first item, with nothing above it at the same level: L does nothing
        press(&mut app, KeyCode::Char('L'));
        assert_eq!(read(), text);
        // L on "Weekly update": indented together with "Metrics", becoming a subtask
        // of "Design doc"; "Design doc" expands and "Weekly update" stays chosen
        press(&mut app, KeyCode::Char('j'));
        press(&mut app, KeyCode::Char('L'));
        assert_eq!(
            read(),
            "# Work\n- [ ] Design doc\n  - [ ] Research\n  - [ ] Outline\n  - [ ] Weekly update\n    - [ ] Metrics\n\
             # Personal\n- Go hiking\n"
        );
        assert_eq!(
            shape(&app),
            [
                "#Work",
                "-Design doc",
                "  Research",
                "  Outline",
                "  +Weekly update",
                "#Personal",
                "Go hiking"
            ]
        );
        assert_eq!(chosen(&app).as_deref(), Some("Weekly update"));
        // u undoes; move up to "Research", H: aligned with "Design doc",
        // "Outline" below becomes its subtask, and it expands
        press(&mut app, KeyCode::Char('u'));
        assert_eq!(read(), text);
        press(&mut app, KeyCode::Char('k'));
        press(&mut app, KeyCode::Char('k'));
        assert_eq!(chosen(&app).as_deref(), Some("Research"));
        press(&mut app, KeyCode::Char('H'));
        assert_eq!(
            read(),
            "# Work\n- [ ] Design doc\n- [ ] Research\n  - [ ] Outline\n\
             - [ ] Weekly update\n  - [ ] Metrics\n# Personal\n- Go hiking\n"
        );
        assert_eq!(
            shape(&app),
            [
                "#Work",
                "Design doc",
                "-Research",
                "  Outline",
                "+Weekly update",
                "#Personal",
                "Go hiking"
            ]
        );
        // "Go hiking": top level (H does nothing) and the first item of
        // "Personal" ("Weekly update" above is in another group, so L does nothing)
        let before = read();
        for _ in 0..3 {
            press(&mut app, KeyCode::Char('j'));
        }
        assert_eq!(chosen(&app).as_deref(), Some("Go hiking"));
        press(&mut app, KeyCode::Char('H'));
        press(&mut app, KeyCode::Char('L'));
        assert_eq!(read(), before);
        clean_up(&path);
    }

    #[test]
    fn shift_j_and_k_move_the_item_with_its_subtasks() {
        use super::super::write::tests::clean_up;
        let text = "# Work\n- [ ] Design doc\n  - [ ] Research\n  - [ ] Outline\n\
                    - [ ] Weekly update\n# Personal\n- Go hiking\n";
        let (mut app, path) = tasks_app_with_file("tasks-swap", text);
        let read = || std::fs::read_to_string(&path).expect("read");
        let chosen = |app: &App| app.tasks_chosen().map(|item| item.title.clone());
        // J on "Design doc" (line 2, expanded): moves with its subtasks
        // below "Weekly update"; still chosen and still expanded (now line 3)
        app.expanded.insert(2);
        press(&mut app, KeyCode::Char('J'));
        let moved = "# Work\n- [ ] Weekly update\n- [ ] Design doc\n  - [ ] Research\n  - [ ] Outline\n\
                     # Personal\n- Go hiking\n";
        assert_eq!(read(), moved);
        assert_eq!(chosen(&app).as_deref(), Some("Design doc"));
        assert_eq!(app.expanded.iter().copied().collect::<Vec<_>>(), [3]);
        assert_eq!(
            shape(&app),
            [
                "#Work",
                "Weekly update",
                "-Design doc",
                "  Research",
                "  Outline",
                "#Personal",
                "Go hiking"
            ]
        );
        // Already the last item of "Work" ("Go hiking" below is in another group): J does nothing
        press(&mut app, KeyCode::Char('J'));
        assert_eq!(read(), moved);
        // K on "Outline": swaps with "Research"; K again: already the first subtask, nothing happens
        press(&mut app, KeyCode::Char('j'));
        press(&mut app, KeyCode::Char('j'));
        press(&mut app, KeyCode::Char('K'));
        let swapped = "# Work\n- [ ] Weekly update\n- [ ] Design doc\n  - [ ] Outline\n  - [ ] Research\n\
                       # Personal\n- Go hiking\n";
        assert_eq!(read(), swapped);
        assert_eq!(app.tasks_line, Some(4));
        press(&mut app, KeyCode::Char('K'));
        assert_eq!(read(), swapped);
        // u: swapped back, "Outline" still chosen (back on line 5)
        press(&mut app, KeyCode::Char('u'));
        assert_eq!(read(), moved);
        assert_eq!(app.tasks_line, Some(5));
        assert_eq!(chosen(&app).as_deref(), Some("Outline"));
        clean_up(&path);
    }

    #[test]
    fn l_reloads_when_the_item_above_changed_elsewhere() {
        use super::super::write::{Notice, tests::clean_up};
        // After choosing "Weekly update", "Design doc" above was changed
        // elsewhere (same line count): L does not write and reloads
        let text = "- [ ] Design doc\n- [ ] Weekly update\n";
        let (mut app, path) = tasks_app_with_file("tasks-indent-changed", text);
        app.tasks_line = Some(2);
        let elsewhere = "- [ ] Design doc (edited)\n- [ ] Weekly update\n";
        std::fs::write(&path, elsewhere).expect("write");
        press(&mut app, KeyCode::Char('L'));
        assert_eq!(std::fs::read_to_string(&path).expect("read"), elsewhere);
        assert_eq!(app.notice, Some(Notice::Reloaded));
        clean_up(&path);
    }

    #[test]
    fn indent_and_move_stay_inside_their_heading_section() {
        use super::super::write::tests::clean_up;
        // Two "Work" sections with the same name, with an empty "Personal" between them:
        // "Weekly update" in the second section has no item above it in the same section,
        // so L and K do nothing (no pointless write)
        let text = "# Work\n- [ ] Design doc\n# Personal\n# Work\n- [ ] Weekly update\n";
        let (mut app, path) = tasks_app_with_file("tasks-sections", text);
        let read = || std::fs::read_to_string(&path).expect("read");
        app.tasks_line = Some(5);
        for key in ['L', 'K'] {
            press(&mut app, KeyCode::Char(key));
            assert_eq!(read(), text, "{key}");
        }
        assert!(app.expanded.is_empty());
        // "Design doc" in the first section: no item below it in the same section either, so J does nothing
        app.tasks_line = Some(2);
        press(&mut app, KeyCode::Char('J'));
        assert_eq!(read(), text);
        clean_up(&path);
    }

    #[test]
    fn moving_one_of_two_same_named_items_keeps_the_choice_on_it() {
        use super::super::write::tests::clean_up;
        // Both items are called "Gym": J on the first one (line 1, expanded)
        // → moves to line 3, and the choice and the expanded state follow it
        let text = "- [ ] Gym\n  - [ ] Run 3 miles\n- [ ] Gym\n  - [ ] Swim laps\n";
        let (mut app, path) = tasks_app_with_file("tasks-same-name", text);
        let read = || std::fs::read_to_string(&path).expect("read");
        app.expanded.insert(1);
        press(&mut app, KeyCode::Char('J'));
        let moved = "- [ ] Gym\n  - [ ] Swim laps\n- [ ] Gym\n  - [ ] Run 3 miles\n";
        assert_eq!(read(), moved);
        assert_eq!(app.tasks_line, Some(3));
        assert_eq!(app.expanded.iter().copied().collect::<Vec<_>>(), [3]);
        // Already the last item: J again does nothing (does not swap back)
        press(&mut app, KeyCode::Char('J'));
        assert_eq!(read(), moved);
        // u: moved back; the choice and the expanded state go back to line 1 too
        press(&mut app, KeyCode::Char('u'));
        assert_eq!(read(), text);
        assert_eq!(app.tasks_line, Some(1));
        assert_eq!(app.expanded.iter().copied().collect::<Vec<_>>(), [1]);
        clean_up(&path);
    }

    #[test]
    fn deleting_every_subtask_does_not_leave_the_parent_expanded() {
        use super::super::write::tests::clean_up;
        // "Design doc" (line 3) and "Research" (line 4) are both expanded;
        // delete "Read RFCs" (line 5), "Research"'s only subtask
        let (mut app, path) = tasks_app_with_file("tasks-stale-expanded", TREE);
        app.expanded.extend([3, 4]);
        app.tasks_line = Some(5);
        press(&mut app, KeyCode::Char('d'));
        press(&mut app, KeyCode::Char('y'));
        // "Research" has no subtasks now: it does not count as expanded;
        // one h jumps to its parent "Design doc" (no need to press twice)
        app.tasks_line = Some(4);
        let row = app.tasks_cursor(&app.tasks_rows()).expect("chosen");
        assert!(matches!(
            app.tasks_rows()[row],
            TaskRow::Item {
                children: false,
                expanded: false,
                ..
            }
        ));
        press(&mut app, KeyCode::Char('h'));
        assert_eq!(app.tasks_line, Some(3));
        clean_up(&path);
    }

    #[test]
    fn a_conflict_on_the_tasks_page_keeps_the_keyboard_there() {
        use super::super::write::{Notice, tests::clean_up};
        // The file was changed elsewhere (a line added at the top) while editing, adding or deleting:
        // reload, and the keyboard stays on the tasks page instead of going to the hidden calendar
        let text = "- [ ] Triage bugs\n- [ ] Weekly update @2026-10-16\n";
        let (mut app, path) = tasks_app_with_file("tasks-conflict", text);
        // Add a different line at the top each time: the chosen item (line 1) never matches
        let mut extra = String::new();
        let mut change_elsewhere = || {
            extra = format!("- Added elsewhere {}\n{extra}", extra.lines().count() + 1);
            std::fs::write(&path, format!("{extra}{text}")).expect("write");
        };
        for keys in [&['e', '!'][..], &['a', 'x'], &['d']] {
            for &key in keys {
                press(&mut app, KeyCode::Char(key));
            }
            change_elsewhere();
            let confirm = if keys[0] == 'd' {
                KeyCode::Char('y')
            } else {
                KeyCode::Enter
            };
            press(&mut app, confirm);
            assert_eq!(app.notice, Some(Notice::Reloaded), "{keys:?}");
            assert_eq!(
                (app.view, app.focus),
                (View::Tasks, Focus::Tasks),
                "{keys:?}"
            );
        }
        clean_up(&path);
    }

    #[test]
    fn r_on_the_tasks_page_borrows_the_calendar() {
        use super::super::write::tests::clean_up;
        // "Triage bugs" has no date yet: r switches to layout 1's calendar, starting
        // from today; one day later, Enter → scheduled on 10/18, back to the tasks page
        let text = "- [ ] Triage bugs\n- [ ] Weekly update @2026-10-16\n";
        let (mut app, path) = tasks_app_with_file("tasks-move", text);
        press(&mut app, KeyCode::Char('r'));
        assert_eq!((app.view, app.focus), (View::Planner, Focus::Calendar));
        assert_eq!(app.selected, date(2026, 10, 17));
        press(&mut app, KeyCode::Char('l'));
        press(&mut app, KeyCode::Enter);
        assert_eq!(
            std::fs::read_to_string(&path).expect("read"),
            "- [ ] Triage bugs @2026-10-18\n- [ ] Weekly update @2026-10-16\n"
        );
        assert_eq!((app.view, app.focus), (View::Tasks, Focus::Tasks));
        assert_eq!(
            app.tasks_chosen().map(|item| item.title.as_str()),
            Some("Triage bugs")
        );
        // Esc cancels: also back to the tasks page
        press(&mut app, KeyCode::Char('r'));
        press(&mut app, KeyCode::Esc);
        assert_eq!((app.view, app.focus), (View::Tasks, Focus::Tasks));
        clean_up(&path);
    }

    #[test]
    fn expanded_items_follow_their_lines_after_a_delete_above() {
        use super::super::write::tests::clean_up;
        // b (line 2) is expanded; after deleting a above it, b becomes line 1 and is still expanded
        let text = "- [ ] a\n- [ ] b\n  - [ ] c\n";
        let (mut app, path) = tasks_app_with_file("tasks-remap", text);
        app.expanded.insert(2);
        press(&mut app, KeyCode::Char('d'));
        press(&mut app, KeyCode::Char('y'));
        assert_eq!(app.expanded.iter().copied().collect::<Vec<_>>(), [1]);
        assert_eq!(shape(&app), ["#", "-b", "  c"]);
        clean_up(&path);
    }

    #[test]
    fn esc_goes_back_to_the_calendar_and_tab_reaches_the_side_menu() {
        let mut app = tasks_app(TREE);
        // Wide enough: Tab moves to the side menu, Tab again back to the
        // tasks page; Esc goes from the side menu back to the tasks page
        app.screen = ratatui::layout::Rect::new(0, 0, 120, 30);
        press(&mut app, KeyCode::Tab);
        assert_eq!(app.focus, Focus::Side);
        press(&mut app, KeyCode::Tab);
        assert_eq!(app.focus, Focus::Tasks);
        press(&mut app, KeyCode::Tab);
        press(&mut app, KeyCode::Esc);
        assert_eq!(app.focus, Focus::Tasks);
        // Esc on the tasks page: back to layout 1's calendar (not quitting)
        press(&mut app, KeyCode::Esc);
        assert_eq!((app.view, app.focus), (View::Planner, Focus::Calendar));
        assert!(!app.should_quit);
    }
}
