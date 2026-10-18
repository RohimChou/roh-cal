//! Pure functions that change the task file: given the file's current content, return the new content
//! (the caller writes the file with `files::write_atomically`)
//!
//! Only the part being changed is touched (check off, edit, reschedule: that line; delete, indent, move to
//! another calendar: that block; move up / down: those two blocks; add: one inserted line). Every other character stays as it is,
//! including whether line endings are `\n` or `\r\n`.
//! Every change checks first: if the line is not what was read (e.g. it was just edited in Obsidian),
//! nothing is changed and `Changed` is returned

use crate::tasks::{self, Heading, HeadingLine, Item, WarningKind};
use jiff::{ToSpan, civil::Date};

/// The line differs from when it was read (edited elsewhere, a line added above, the file got shorter),
/// so it cannot be changed; read the file again first
#[derive(Debug, PartialEq, Eq)]
pub struct Changed;

/// What is wrong with the line typed in the editor (it cannot be saved)
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Problem {
    /// Does not start with `- ` (not an item)
    NotAnItem,
    /// A mistake, e.g. `@2026-13-01` (the parser treats it as plain text, so once saved the date is gone)
    Mistake(WarningKind),
    /// The title is empty (e.g. only `- [ ] @2026-10-17` was typed)
    EmptyTitle,
    /// A calendar's name is empty
    EmptyName,
    /// Another calendar already has this name (headings with the same name count as one calendar)
    NameTaken,
}

/// Where in the file the added line goes
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Place {
    /// Below the last item of `item`'s group (under the same heading) and that item's subtasks
    InGroupOf(Item),
    /// A subtask of `item`: below it and its existing subtasks, indented 2 spaces more than it
    ChildOf(Item),
    /// At the end of the calendar with this path (its parents' names, then its own, e.g. `Work`, `SFAP`):
    /// below its last item (and that item's subtasks and notes); right under its first heading when it has
    /// no items of its own yet
    InCalendar(Vec<String>),
    /// At the end of the file
    End,
}

/// What the line typed in the editor (without indent) reads as: it counts only if it is exactly one item
/// with no mistakes
///
/// E.g. `- [ ] Send the weekly update @2026-10-16 17:30` → that item; `Send the weekly update` →
/// `NotAnItem`; `- Trip @2026-13-01` → `Mistake`
pub fn check(line: &str) -> Result<Item, Problem> {
    let line = line.trim();
    // Only `- ` was typed (`-` after trimming): it is an item, it just has no title yet
    if line == "-" {
        return Err(Problem::EmptyTitle);
    }
    if !line.starts_with("- ") {
        return Err(Problem::NotAnItem);
    }
    let parsed = tasks::parse(line);
    if let Some(warning) = parsed.warnings.into_iter().next() {
        return Err(Problem::Mistake(warning.kind));
    }
    let item = parsed.items.into_iter().next().ok_or(Problem::NotAnItem)?;
    if item.title.is_empty() {
        return Err(Problem::EmptyTitle);
    }
    Ok(item)
}

/// What the text typed as a calendar's name reads as: the heading it becomes (with `type:holiday` in it:
/// a holiday calendar); empty → `EmptyName`; a name in `taken` (the calendars there already are) →
/// `NameTaken`
///
/// E.g. `Side projects` → `# Side projects`; `US holidays type:holiday` → a holiday calendar named
/// `US holidays`
pub fn check_calendar(text: &str, taken: &[String]) -> Result<Heading, Problem> {
    let heading = tasks::parse(&format!("# {}", text.trim()))
        .headings
        .into_iter()
        .next()
        .map(|line| line.heading)
        .ok_or(Problem::EmptyName)?;
    if heading.name.is_empty() {
        return Err(Problem::EmptyName);
    }
    if taken.contains(&heading.name) {
        return Err(Problem::NameTaken);
    }
    Ok(heading)
}

/// Add a calendar: the heading `# <name>` (`name` = what was typed in the editor, checked with
/// `check_calendar` first) as the file's last line, after a blank line (no blank line in an empty file or
/// after one that is already there); return the new file content and the heading's line number (from 1)
///
/// Line endings follow the file's own (`\n` or `\r\n`); if the last line has no newline, one is added first
pub fn add_calendar(text: &str, name: &str) -> (String, usize) {
    let newline = if text.contains("\r\n") { "\r\n" } else { "\n" };
    let mut result = text.to_string();
    if !result.is_empty() && !result.ends_with('\n') {
        result.push_str(newline);
    }
    if result
        .lines()
        .last()
        .is_some_and(|line| !line.trim().is_empty())
    {
        result.push_str(newline);
    }
    result.push_str(&format!("# {}{newline}", name.trim()));
    let line = result.lines().count();
    (result, line)
}

/// Rename a calendar: each of its heading lines (`headings`; headings with the same name are one calendar)
/// becomes `<its own #s> <new>` (`new` = what was typed in the editor, checked with `check_calendar` first);
/// return the whole changed file. `Changed` if one of those lines now differs from when it was read
///
/// E.g. `## Moving` renamed to `Moving house` → `## Moving house`
pub fn rename_calendar(text: &str, headings: &[HeadingLine], new: &str) -> Result<String, Changed> {
    let mut result = String::with_capacity(text.len());
    let mut renamed = 0;
    // Whole segments including the newline, so the newlines (\n or \r\n) stay as they were
    for (index, (content, segment)) in text.lines().zip(text.split_inclusive('\n')).enumerate() {
        let Some(heading) = headings.iter().find(|heading| heading.line == index + 1) else {
            result.push_str(segment);
            continue;
        };
        if content != heading.text {
            return Err(Changed);
        }
        result.push_str(&format!(
            "{} {}",
            "#".repeat(heading.heading.level),
            new.trim()
        ));
        result.push_str(&segment[content.len()..]);
        renamed += 1;
    }
    if renamed == headings.len() {
        Ok(result)
    } else {
        Err(Changed)
    }
}

/// Delete a calendar: each of its heading lines (`headings`) and everything under it, the calendars inside
/// it too, up to the next heading with as many `#` or fewer (a `#` line in a code block does not count) or the
/// end of the file; return the whole file
/// after deleting and how many items were deleted. `Changed` if one of the heading lines now differs from
/// when it was read
///
/// E.g. deleting `Moving` from `# Personal`, `- a`, `## Moving`, `- b`, `- c`, `# Work`, `- d` leaves
/// `# Personal`, `- a`, `# Work`, `- d` (2 items deleted)
pub fn delete_calendar(text: &str, headings: &[HeadingLine]) -> Result<(String, usize), Changed> {
    let lines: Vec<&str> = text.lines().collect();
    // Whole segments including the newline, so the newlines (\n or \r\n) stay as they were
    let segments: Vec<&str> = text.split_inclusive('\n').collect();
    if headings
        .iter()
        .any(|heading| lines.get(heading.line - 1) != Some(&heading.text.as_str()))
    {
        return Err(Changed);
    }
    // Where every heading starts (0-based) and its number of `#`, as the parser sees them
    let starts: Vec<(usize, usize)> = tasks::parse(text)
        .headings
        .iter()
        .map(|heading| (heading.line - 1, heading.heading.level))
        .collect();
    let mut deleted = vec![false; segments.len()];
    for heading in headings {
        let start = heading.line - 1;
        let end = starts
            .iter()
            .find(|&&(next, level)| next > start && level <= heading.heading.level)
            .map_or(segments.len(), |&(next, _)| next);
        deleted[start..end].fill(true);
    }
    let kept = |keep: bool| -> String {
        segments
            .iter()
            .zip(&deleted)
            .filter(|&(_, &gone)| gone != keep)
            .map(|(segment, _)| *segment)
            .collect()
    };
    // How many items the deleted lines hold (counted by the parser, so it matches what the screen sees)
    let count = tasks::parse(&kept(false)).items.len();
    Ok((kept(true), count))
}

/// Move a calendar one level in (`deeper`) / out: each of its heading lines (`headings`) and the headings
/// of the calendars inside it (up to the next heading with as many `#` or fewer) get one `#` more / fewer;
/// only those `#`s change. Return the changed file, or `None` when it cannot move: going in needs a calendar
/// right above it at the same level to go inside (and at most 6 `#`), going out needs at least 2 `#`.
/// `Changed` if one of its heading lines now differs from when it was read
///
/// E.g. in `# Work`, `# SFAP`, `## Bugs`, moving SFAP in gives `# Work`, `## SFAP`, `### Bugs`
pub fn shift_calendar(
    text: &str,
    headings: &[HeadingLine],
    deeper: bool,
) -> Result<Option<String>, Changed> {
    let lines: Vec<&str> = text.lines().collect();
    if headings
        .iter()
        .any(|heading| lines.get(heading.line - 1) != Some(&heading.text.as_str()))
    {
        return Err(Changed);
    }
    let all = tasks::parse(text).headings;
    // Line numbers (from 1) of the headings that get one `#` more / fewer
    let mut shifted: Vec<usize> = Vec::new();
    for heading in headings {
        let (start, level) = (heading.line, heading.heading.level);
        if deeper {
            // The nearest heading above at this level or higher must be at this level: it goes inside that one
            let above = all
                .iter()
                .rev()
                .find(|other| other.line < start && other.heading.level <= level);
            if above.is_none_or(|other| other.heading.level != level) {
                return Ok(None);
            }
        } else if level < 2 {
            return Ok(None);
        }
        let end = all
            .iter()
            .find(|other| other.line > start && other.heading.level <= level)
            .map_or(usize::MAX, |other| other.line);
        for other in all
            .iter()
            .filter(|other| (start..end).contains(&other.line))
        {
            if deeper && other.heading.level >= 6 {
                return Ok(None);
            }
            if !shifted.contains(&other.line) {
                shifted.push(other.line);
            }
        }
    }
    let mut result = String::with_capacity(text.len() + shifted.len());
    for (index, segment) in text.split_inclusive('\n').enumerate() {
        match (shifted.contains(&(index + 1)), deeper) {
            (true, true) => {
                result.push('#');
                result.push_str(segment);
            }
            // A heading line starts with `#` (1 byte)
            (true, false) => result.push_str(&segment[1..]),
            (false, _) => result.push_str(segment),
        }
    }
    Ok(Some(result))
}

/// Add: put `new` (the line typed in the editor) at `place`; return the new file content and the new line's
/// line number (from 1)
///
/// Line endings follow the file's own (`\n` or `\r\n`); if the last line has no newline, one is added first.
/// The caller must check `new` with `check` first
pub fn add(text: &str, new: &str, place: &Place) -> Result<(String, usize), Changed> {
    let lines: Vec<&str> = text.lines().collect();
    let segments: Vec<&str> = text.split_inclusive('\n').collect();
    // Which segment the new line goes before, and its indent
    let (at, indent) = match place {
        Place::InGroupOf(item) => {
            find_line(&lines, item)?;
            // The group's last item (re-parse the current file content, so it matches what the screen sees)
            let items = tasks::parse(text).items;
            let mut last = items
                .iter()
                .skip_while(|other| other.line != item.line)
                .take_while(|other| other.group == item.group)
                .last()
                .ok_or(Changed)?;
            // The last item is a subtask: go up to its outermost item and count that item's whole block
            // (with its subtasks and the note lines below), so the new line does not go in front of a note
            // line and take it over
            while let Some(parent) = last.parent {
                last = &items[parent];
            }
            // No indent, same as the group's outermost items
            (block_end(text, &lines, last.line - 1), String::new())
        }
        Place::ChildOf(item) => {
            let start = find_line(&lines, item)?;
            let indent =
                &item.line_text[..item.line_text.len() - item.line_text.trim_start().len()];
            (block_end(text, &lines, start), format!("{indent}  "))
        }
        Place::InCalendar(path) => (calendar_end(text, &lines, path)?, String::new()),
        Place::End => (segments.len(), String::new()),
    };
    let newline = if text.contains("\r\n") { "\r\n" } else { "\n" };
    let mut result = segments[..at].concat();
    // The line before is the file's last line and has no newline: add one first
    if !result.is_empty() && !result.ends_with('\n') {
        result.push_str(newline);
    }
    result.push_str(&format!("{indent}{}{newline}", new.trim()));
    result.push_str(&segments[at..].concat());
    Ok((result, at + 1))
}

/// Where a line goes at the end of the calendar with this `path` (see `Place::InCalendar`): the index of the
/// line it goes before (`lines` = `text`'s lines). Below the calendar's last item (counting its outermost
/// item's whole block, with subtasks and note lines); right under its first heading when it has no items of
/// its own yet; `Changed` when there is no such calendar
fn calendar_end(text: &str, lines: &[&str], path: &[String]) -> Result<usize, Changed> {
    let parsed = tasks::parse(text);
    let is_it = |heading: &Heading| {
        heading.parents.len() + 1 == path.len()
            && heading
                .parents
                .iter()
                .chain([&heading.name])
                .eq(path.iter())
    };
    let last = parsed
        .items
        .iter()
        .rfind(|item| item.group.as_ref().is_some_and(is_it));
    match last {
        // Its last item: count its outermost item's whole block
        Some(mut last) => {
            while let Some(parent) = last.parent {
                last = &parsed.items[parent];
            }
            Ok(block_end(text, lines, last.line - 1))
        }
        // No items yet: right under its heading (the heading's line number = the next line's index)
        None => parsed
            .headings
            .iter()
            .find(|line| is_it(&line.heading))
            .map(|heading| heading.line)
            .ok_or(Changed),
    }
}

/// Move to another calendar (an item edited with another calendar picked in the `#` list): `item`'s line
/// becomes `new` (the line typed in the editor) and its whole block (with subtasks and note lines) goes to
/// the end of the calendar with this `path` (where `Place::InCalendar` adds), as an outermost item: `item`'s
/// indent is taken off every line, the subtasks keep their extra indent. Return the whole changed file and
/// `item`'s new line number (from 1). `Changed` if `item`'s line differs from when it was read, or there is
/// no such calendar
///
/// E.g. moving the subtask `  - [ ] Book flights` (with `    - [ ] Compare prices` under it) to `Work`:
/// `- [ ] Book flights` and `  - [ ] Compare prices` go below Work's last item
pub fn move_to_calendar(
    text: &str,
    item: &Item,
    new: &str,
    path: &[String],
) -> Result<(String, usize), Changed> {
    let lines: Vec<&str> = text.lines().collect();
    // Whole segments including the newline, so the newlines (\n or \r\n) stay as they were
    let segments: Vec<&str> = text.split_inclusive('\n').collect();
    let start = find_line(&lines, item)?;
    let end = block_end(text, &lines, start);
    let newline = if text.contains("\r\n") { "\r\n" } else { "\n" };
    // The block as it will be: the typed line, then the lines under it without `item`'s indent
    let mut block = format!("{}{newline}", new.trim());
    block.push_str(&reindented(
        &segments[start + 1..end],
        indent_of(&item.line_text),
        "",
    ));
    if !block.ends_with('\n') {
        block.push_str(newline);
    }
    // The file without the block; the block goes at the end of the calendar there
    let rest = [segments[..start].concat(), segments[end..].concat()].concat();
    let rest_lines: Vec<&str> = rest.lines().collect();
    let at = calendar_end(&rest, &rest_lines, path)?;
    let rest_segments: Vec<&str> = rest.split_inclusive('\n').collect();
    let mut result = rest_segments[..at].concat();
    // The line before is the file's last line and has no newline: add one first
    if !result.is_empty() && !result.ends_with('\n') {
        result.push_str(newline);
    }
    result.push_str(&block);
    result.push_str(&rest_segments[at..].concat());
    Ok((result, at + 1))
}

/// Delete: remove `item` together with the following lines indented deeper than it (subtasks, note lines,
/// blank lines in between too; trailing blank lines are not included);
/// return the whole file after deleting and how many items were deleted (itself included)
///
/// E.g. deleting `- Tokyo, 5 days` → `  - [ ] Book flights` and `    - [ ] Compare prices` under it go too,
/// 3 items in total
pub fn delete(text: &str, item: &Item) -> Result<(String, usize), Changed> {
    let lines: Vec<&str> = text.lines().collect();
    // Whole segments including the newline, so the newlines (\n or \r\n) stay as they were
    let segments: Vec<&str> = text.split_inclusive('\n').collect();
    let start = find_line(&lines, item)?;
    let end = block_end(text, &lines, start);
    // How many items the deleted lines hold (counted by the parser, so it matches what the screen sees)
    let count = tasks::parse(&segments[start..end].concat()).items.len();
    let mut result = segments[..start].concat();
    result.push_str(&segments[end..].concat());
    Ok((result, count))
}

/// Indent / outdent: in `item`'s block (with subtasks and note lines), replace `item`'s indent at the
/// start of each line with `to`, and return the whole changed file;
/// the subtasks' extra indent is kept
///
/// E.g. `to` = `  `: `- Tokyo, 5 days`, `  - [ ] Book flights` → `  - Tokyo, 5 days`,
/// `    - [ ] Book flights`.
/// If a line in the block starts with an indent written differently from `item`'s (tabs and spaces mixed),
/// it is replaced with spaces by width, so the parent / child relations stay the same.
/// `to` is worked out from the `anchor` line (the item above, or the parent): that line must also match
/// what was read, otherwise `Changed` is returned
pub fn reindent(text: &str, item: &Item, anchor: &Item, to: &str) -> Result<String, Changed> {
    let lines: Vec<&str> = text.lines().collect();
    // Whole segments including the newline, so the newlines (\n or \r\n) stay as they were
    let segments: Vec<&str> = text.split_inclusive('\n').collect();
    let start = find_line(&lines, item)?;
    find_line(&lines, anchor)?;
    let end = block_end(text, &lines, start);
    let mut result = segments[..start].concat();
    result.push_str(&reindented(
        &segments[start..end],
        indent_of(&item.line_text),
        to,
    ));
    result.push_str(&segments[end..].concat());
    Ok(result)
}

/// In a block (whole segments with newlines), replace the indent `from` at the start of each line with `to`;
/// the subtasks' extra indent is kept, and so are blank lines in between.
/// Lines that do not start with `from` (tabs and spaces mixed) get spaces by width, so the parent / child
/// relations stay the same
fn reindented(segments: &[&str], from: &str, to: &str) -> String {
    let mut result = String::new();
    for segment in segments {
        if segment.trim().is_empty() {
            result.push_str(segment);
            continue;
        }
        match segment.strip_prefix(from) {
            Some(rest) => {
                result.push_str(to);
                result.push_str(rest);
            }
            None => {
                let width = (tasks::indent_width(segment) + tasks::indent_width(to))
                    .saturating_sub(tasks::indent_width(from));
                result.push_str(&" ".repeat(width));
                result.push_str(segment.trim_start());
            }
        }
    }
    result
}

/// Move up / down: swap the block of `upper` with the block of `lower`, on the same level below it (both
/// with subtasks and note lines), and return the whole changed file; whatever lies between the two blocks
/// (blank lines, note lines) stays in the middle. Indents stay where they were: the block moved up gets
/// `upper`'s indent, the block moved down gets `lower`'s (when the two items write their indent
/// differently, e.g. one with a tab and one with two spaces, moving them as they are could turn one into
/// a subtask of the other)
///
/// E.g. `- a`, `  - a1`, `- b` → `- b`, `- a`, `  - a1`
pub fn swap(text: &str, upper: &Item, lower: &Item) -> Result<(String, Swap), Changed> {
    let lines: Vec<&str> = text.lines().collect();
    // Whole segments including the newline, so the newlines (\n or \r\n) stay as they were
    let segments: Vec<&str> = text.split_inclusive('\n').collect();
    let upper_start = find_line(&lines, upper)?;
    let lower_start = find_line(&lines, lower)?;
    let upper_end = block_end(text, &lines, upper_start);
    let lower_end = block_end(text, &lines, lower_start);
    // The lower block must start after the upper block ends (if not, the file does not match the screen)
    if lower_start < upper_end {
        return Err(Changed);
    }
    let (upper_indent, lower_indent) = (indent_of(&upper.line_text), indent_of(&lower.line_text));
    let mut upper_block = reindented(
        &segments[upper_start..upper_end],
        upper_indent,
        lower_indent,
    );
    let mut lower_block = reindented(
        &segments[lower_start..lower_end],
        lower_indent,
        upper_indent,
    );
    // The lower block is the file's last line with no newline: it needs one when moved up, and the block
    // moved to the end gets no newline (same as before)
    if !lower_block.ends_with('\n') {
        lower_block.push_str(if text.contains("\r\n") { "\r\n" } else { "\n" });
        upper_block.truncate(upper_block.trim_end_matches(['\r', '\n']).len());
    }
    let result = [
        segments[..upper_start].concat(),
        lower_block,
        segments[upper_end..lower_start].concat(),
        upper_block,
        segments[lower_end..].concat(),
    ]
    .concat();
    let swap = Swap {
        start: upper_start + 1,
        upper: upper_end - upper_start,
        gap: lower_start - upper_end,
        lower: lower_end - lower_start,
    };
    Ok((result, swap))
}

/// Which two blocks a move up / down swapped (line numbers from 1): after writing, the line numbers the
/// screen keeps are moved with `map`; undo uses `reversed`
///
/// Counts the lines actually moved, not their text (so it works even when two items read the same)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Swap {
    /// First line of the upper block
    pub start: usize,
    /// Number of lines in the upper block
    pub upper: usize,
    /// Number of lines between the two blocks (blank lines, note lines)
    pub gap: usize,
    /// Number of lines in the lower block
    pub lower: usize,
}

impl Swap {
    /// Where line `line` (before the swap) ends up after it: the upper block moves down, the lower block
    /// moves up, the lines between shift along, the rest stay
    ///
    /// E.g. lines 1-2 swapped with line 3 (`start` 1, `upper` 2, `gap` 0, `lower` 1): 1 → 2, 2 → 3, 3 → 1
    pub fn map(self, line: usize) -> usize {
        let upper_end = self.start + self.upper;
        let lower_start = upper_end + self.gap;
        let lower_end = lower_start + self.lower;
        if line < self.start || line >= lower_end {
            line
        } else if line < upper_end {
            line + self.gap + self.lower
        } else if line < lower_start {
            line + self.lower - self.upper
        } else {
            line - self.upper - self.gap
        }
    }

    /// The swap back (for undo): the block that was below is now on top
    pub fn reversed(self) -> Self {
        Self {
            upper: self.lower,
            lower: self.upper,
            ..self
        }
    }
}

/// The indent at the start of a line (spaces, tabs)
pub fn indent_of(line: &str) -> &str {
    &line[..line.len() - line.trim_start().len()]
}

/// Maps line numbers (from 1) from before a write to after it, so the line numbers the screen keeps
/// (the chosen item, expanded subtasks) move along
///
/// Each write changes only one run of lines (check off, edit, reschedule, add, delete, indent, undo all do):
/// line numbers before it stay, those after it shift by the change;
/// inside the changed run: if the line count did not change (one line replaced), they stay; if it did
/// (add, delete), they have no match (`None`). Move up / down does not use this; it uses `Swap::map`
pub fn line_map(before: &str, after: &str) -> impl Fn(usize) -> Option<usize> + use<> {
    let old: Vec<&str> = before.lines().collect();
    let new: Vec<&str> = after.lines().collect();
    // How many lines match at the start, and how many at the end (not overlapping the start)
    let prefix = old.iter().zip(&new).take_while(|(a, b)| a == b).count();
    let suffix = old
        .iter()
        .rev()
        .zip(new.iter().rev())
        .take_while(|(a, b)| a == b)
        .count()
        .min(old.len().min(new.len()) - prefix);
    let (old_len, new_len) = (old.len(), new.len());
    move |line| {
        if line <= prefix {
            // Before the changed run: unchanged
            Some(line)
        } else if line > old_len - suffix {
            // After the changed run: shift by the lines added / removed
            (line + new_len).checked_sub(old_len)
        } else if old_len == new_len {
            // Inside the changed run, same line count (one line replaced): unchanged
            Some(line)
        } else {
            // Inside the changed run, line count changed: no match
            None
        }
    }
}

/// Which line `item` is on (from 0); `Changed` if the line differs from when it was read (or is gone)
fn find_line(lines: &[&str], item: &Item) -> Result<usize, Changed> {
    let index = item.line.checked_sub(1).ok_or(Changed)?;
    if lines.get(index) == Some(&item.line_text.as_str()) {
        Ok(index)
    } else {
        Err(Changed)
    }
}

/// Where the whole block of the item on line `start` (from 0) ends: with the following lines indented
/// deeper than it (subtasks, note lines, blank lines in between too),
/// it returns the line after the last non-blank one; trailing blank lines are not included (they separate
/// it from the next item). If a note line that is not indented deeper sits in between
/// (Obsidian treats it as text of the same item) and the parser still counts later items as its subtasks,
/// the block reaches those too (`text` = the whole file, for the parser)
///
/// E.g. `- Write the design doc`, `Note: due Friday`, `  - Gather requirements`, `- Send the weekly update`
/// → the design doc's block is the first 3 lines
fn block_end(text: &str, lines: &[&str], start: usize) -> usize {
    let items = tasks::parse(text).items;
    let mut end = indented_end(lines, start);
    if let Some(index) = items.iter().position(|item| item.line == start + 1) {
        // Subtasks = the items right after it that are deeper; take the last one together with its own block
        let depth = items[index].depth;
        let descendants = items[index + 1..]
            .iter()
            .take_while(|item| item.depth > depth);
        if let Some(last) = descendants.last() {
            end = end.max(indented_end(lines, last.line - 1));
        }
    }
    end
}

/// Where line `start` (from 0) and the following lines indented deeper than it (blank lines in between
/// too) end: returns the line after the last non-blank one
fn indented_end(lines: &[&str], start: usize) -> usize {
    let indent = tasks::indent_width(lines[start]);
    let mut end = start + 1;
    for (index, line) in lines.iter().enumerate().skip(start + 1) {
        // Skip blank lines; stop at the first line not indented deeper
        if line.trim().is_empty() {
            continue;
        }
        if tasks::indent_width(line) <= indent {
            break;
        }
        end = index + 1;
    }
    end
}

/// Edit: replace `item`'s line with `new` (the line typed in the editor, without indent), keeping the
/// original indent; return the whole changed file
///
/// The caller must check `new` with `check` first
pub fn replace(text: &str, item: &Item, new: &str) -> Result<String, Changed> {
    let indent = &item.line_text[..item.line_text.len() - item.line_text.trim_start().len()];
    replace_line(text, item, &format!("{indent}{}", new.trim()))
}

/// Check / uncheck a task: `- [ ] Team sync` ↔ `- [x] Team sync`; return the whole file after the change
///
/// `text` = the task file's current content, `item` = the item read earlier. An event has no box and
/// cannot be checked off: its line stays as is (the caller must block it first)
pub fn toggle(text: &str, item: &Item) -> Result<String, Changed> {
    replace_line(text, item, &toggled(&item.line_text))
}

/// Reschedule: replace only the date on that line, nothing else; return the whole changed file
///
/// The time is kept; a multi-day range moves as a whole with the same number of days
/// (e.g. `@2026-10-20..2026-10-25` moved to 11/2 → `@2026-11-02..2026-11-07`).
/// Not yet scheduled: the date is added at the end of the line (before an Obsidian `^id` if there is one)
pub fn reschedule(text: &str, item: &Item, to: Date) -> Result<String, Changed> {
    replace_line(text, item, &rescheduled(&item.line_text, item, to))
}

/// Change a line's date to `to` (see `reschedule`)
fn rescheduled(line: &str, item: &Item, to: Date) -> String {
    let Some(from) = item.date else {
        // Not yet scheduled: add it at the end of the line, keeping `^id` last (trailing spaces do not count;
        // a tab before `^id` is recognized too)
        let (body, block_id) = match line.trim_end().rsplit_once(char::is_whitespace) {
            Some((body, last)) if tasks::is_block_id(last) => (body, format!(" {last}")),
            _ => (line.trim_end(), String::new()),
        };
        return format!("{} @{to}{block_id}", body.trim_end());
    };
    // The new date: for a multi-day item, the last day moves along
    let new_token = match item.end {
        Some(end) => {
            let days = (to - from).get_days();
            format!("@{to}..{}", end.saturating_add(days.days()))
        }
        None => format!("@{to}"),
    };
    // How it is written in the file: the parser takes the first valid date, which always prints as the
    // standard YYYY-MM-DD;
    // a range with the same first and last day (`@2026-10-20..2026-10-20`) is one day to the parser,
    // so look for both forms
    let old_tokens = match item.end {
        Some(end) => vec![format!("@{from}..{end}")],
        None => vec![format!("@{from}"), format!("@{from}..{from}")],
    };
    for old in old_tokens {
        if let Some(start) = whole_word(line, &old) {
            return format!(
                "{}{new_token}{}",
                &line[..start],
                &line[start + old.len()..]
            );
        }
    }
    // Not found (cannot happen: a date the parser read must be on this line): leave the line as is
    line.to_string()
}

/// Where `word` first appears in `line` as a whole word (whitespace or line start / end on both sides)
fn whole_word(line: &str, word: &str) -> Option<usize> {
    line.match_indices(word)
        .map(|(start, _)| start)
        .find(|&start| {
            let before = line[..start].chars().next_back();
            let after = line[start + word.len()..].chars().next();
            before.is_none_or(char::is_whitespace) && after.is_none_or(char::is_whitespace)
        })
}

/// Check / uncheck a line; the indent and the rest of the text stay as is (`line` is a parsed item's line)
///
/// `- [ ] Team sync` → `- [x] Team sync`; `- [x]`, `- [X]` → `- [ ]`; lines without a box (events) stay as is
fn toggled(line: &str) -> String {
    let trimmed = line.trim_start();
    let indent = &line[..line.len() - trimmed.len()];
    let rest = trimmed.strip_prefix("- ").unwrap_or(trimmed);
    match tasks::checkbox(rest) {
        // Has a box: replace only the 3 characters of `[ ]` / `[x]`
        Some((done, after)) => {
            let mark = if done { "[ ]" } else { "[x]" };
            format!("{indent}- {mark}{after}")
        }
        None => line.to_string(),
    }
}

/// Replace `item`'s line with `new`; `Changed` if that line now differs from `item.line_text` (or is gone)
fn replace_line(text: &str, item: &Item, new: &str) -> Result<String, Changed> {
    let mut result = String::with_capacity(text.len() + new.len());
    let mut replaced = false;
    // lines() gives the content (split the same way as the parser); split_inclusive gives whole segments
    // with the newline, so the newlines (\n or \r\n) stay as they were
    for (index, (content, segment)) in text.lines().zip(text.split_inclusive('\n')).enumerate() {
        if index + 1 != item.line {
            result.push_str(segment);
            continue;
        }
        if content != item.line_text {
            return Err(Changed);
        }
        result.push_str(new);
        result.push_str(&segment[content.len()..]);
        replaced = true;
    }
    if replaced { Ok(result) } else { Err(Changed) }
}

/// Each whitespace-separated word of `text` with where it starts and ends (byte offsets)
fn word_spans(text: &str) -> Vec<(usize, usize)> {
    let mut spans = Vec::new();
    let mut start = None;
    for (index, c) in text.char_indices() {
        match (c.is_whitespace(), start) {
            (true, Some(from)) => {
                spans.push((from, index));
                start = None;
            }
            (false, None) => start = Some(index),
            _ => {}
        }
    }
    if let Some(from) = start {
        spans.push((from, text.len()));
    }
    spans
}

/// Whether the word is shaped like an item's date: `@date` or `@date..date`
fn is_date_word(word: &str) -> bool {
    word.strip_prefix('@')
        .is_some_and(|rest| tasks::looks_like_date(rest) || tasks::looks_like_range(rest))
}

/// The line as the editor shows it when editing on `day`: the item's own `@day` is left out (it is that
/// day anyway) and a time right after it stays as `@HH:MM`, e.g. `- Team sync @2026-10-19 10:00` →
/// `- Team sync @10:00`. Anything else (another day, a range, more than one date-shaped word) stays as it
/// is. `show_day` puts the day back
pub fn hide_day(line: &str, day: Date) -> String {
    let words = word_spans(line);
    let dates: Vec<usize> = (0..words.len())
        .filter(|&index| is_date_word(&line[words[index].0..words[index].1]))
        .collect();
    let [position] = dates[..] else {
        return line.to_string();
    };
    let (from, to) = words[position];
    if line[from..to] != format!("@{day}") {
        return line.to_string();
    }
    match words.get(position + 1) {
        Some(&(time_from, time_to)) if tasks::looks_like_time(&line[time_from..time_to]) => {
            format!(
                "{}@{}{}",
                &line[..from],
                &line[time_from..time_to],
                &line[time_to..]
            )
        }
        // Take the word out with the spaces before it
        _ => format!("{}{}", line[..from].trim_end(), &line[to..]),
    }
}

/// The editor's text back to a task-file line on `day` (see `hide_day`): without a date it gets `@day`,
/// in place of `@HH:MM` if it has one (`@day HH:MM`), otherwise at the end (before an Obsidian block
/// id); a text with its own date stays as it is
///
/// E.g. `- Team sync @10:00` → `- Team sync @2026-10-19 10:00`; `- [ ] Pack` → `- [ ] Pack @2026-10-19`
pub fn show_day(text: &str, day: Date) -> String {
    let text = text.trim_end();
    let words = word_spans(text);
    let word = |&(from, to): &(usize, usize)| &text[from..to];
    if words.iter().any(|span| is_date_word(word(span))) {
        return text.to_string();
    }
    if let Some(&(from, to)) = words.iter().find(|span| {
        word(span)
            .strip_prefix('@')
            .is_some_and(tasks::looks_like_time)
    }) {
        return format!(
            "{}@{day} {}{}",
            &text[..from],
            &text[from + 1..to],
            &text[to..]
        );
    }
    match words.last() {
        Some(&(from, _)) if tasks::is_block_id(&text[from..]) => {
            format!("{} @{day} {}", text[..from].trim_end(), &text[from..])
        }
        _ => format!("{text} @{day}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tasks::parse;
    use jiff::civil::date;

    /// The item at `index` in `text`
    fn item(text: &str, index: usize) -> Item {
        parse(text).items[index].clone()
    }

    #[test]
    fn a_calendar_name_reads_as_a_heading() {
        let taken = ["Work".to_string()];
        let read = |text: &str| {
            check_calendar(text, &taken)
                .map(|heading| (heading.name, heading.level, heading.holiday))
        };
        assert_eq!(
            read("  Side projects "),
            Ok(("Side projects".into(), 1, false))
        );
        // type:holiday makes a holiday calendar and is not part of the name
        assert_eq!(
            read("US holidays type:holiday"),
            Ok(("US holidays".into(), 1, true))
        );
        // Nothing typed, only spaces, only type:holiday: no name
        for text in ["", "   ", "type:holiday"] {
            assert_eq!(read(text), Err(Problem::EmptyName), "{text:?}");
        }
        // A calendar called Work is already there (also with extra spaces or type:holiday)
        for text in ["Work", " Work ", "Work type:holiday"] {
            assert_eq!(read(text), Err(Problem::NameTaken), "{text:?}");
        }
    }

    #[test]
    fn renaming_a_calendar_rewrites_each_of_its_headings() {
        // Two Work headings (one calendar) at different levels, \r\n endings: each keeps its own #s
        let text = "# Work\r\n- a\r\n## Moving\r\n- b\r\n## Work\r\n- c\r\n";
        let work: Vec<HeadingLine> = parse(text)
            .headings
            .into_iter()
            .filter(|line| line.heading.name == "Work")
            .collect();
        assert_eq!(
            rename_calendar(text, &work, " Day job "),
            Ok("# Day job\r\n- a\r\n## Moving\r\n- b\r\n## Day job\r\n- c\r\n".to_string())
        );
        // One of its lines was changed elsewhere, or is gone: nothing is renamed
        assert_eq!(
            rename_calendar(&text.replace("## Work", "## Wrok"), &work, "Day job"),
            Err(Changed)
        );
        assert_eq!(
            rename_calendar("# Work\n- a\n", &work, "Day job"),
            Err(Changed)
        );
    }

    #[test]
    fn deleting_a_calendar_removes_its_headings_and_everything_under_them() {
        // Work has two headings; under Personal, a `#` line in a code block is not a heading, so the part
        // under the first Work does not end there
        let text = "- loose\n# Work\n- a\n  - a1\n\nNotes about work\n# Personal\n```\n# not a heading\n```\n\
                    - b\n## Work\n- c\n# Moving\n- d\n";
        let calendar = |text: &str, name: &str| -> Vec<HeadingLine> {
            parse(text)
                .headings
                .into_iter()
                .filter(|line| line.heading.name == name)
                .collect()
        };
        let work = calendar(text, "Work");
        assert_eq!(
            delete_calendar(text, &work),
            Ok((
                "- loose\n# Personal\n```\n# not a heading\n```\n- b\n# Moving\n- d\n".into(),
                3
            ))
        );
        // The last calendar: up to the end of the file
        assert_eq!(
            delete_calendar(text, &calendar(text, "Moving")),
            Ok((text.replace("# Moving\n- d\n", ""), 1))
        );
        // A heading changed elsewhere: nothing is deleted
        assert_eq!(
            delete_calendar(&text.replace("## Work", "## Wrok"), &work),
            Err(Changed)
        );
        // A calendar with one inside it: both go, up to the next heading with as many `#` or fewer
        let nested = "# A\n- a\n## B\n- b\n### C\n- c\n## D\n- d\n# E\n- e\n";
        assert_eq!(
            delete_calendar(nested, &calendar(nested, "B")),
            Ok(("# A\n- a\n## D\n- d\n# E\n- e\n".into(), 2))
        );
        assert_eq!(
            delete_calendar(nested, &calendar(nested, "A")),
            Ok(("# E\n- e\n".into(), 4))
        );
        // \r\n endings stay
        let crlf = "# A\r\n- a\r\n# B\r\n- b\r\n";
        assert_eq!(
            delete_calendar(crlf, &calendar(crlf, "A")),
            Ok(("# B\r\n- b\r\n".into(), 1))
        );
    }

    #[test]
    fn a_calendar_moves_in_and_out_with_the_ones_inside_it() {
        let calendar = |text: &str, name: &str| -> Vec<HeadingLine> {
            parse(text)
                .headings
                .into_iter()
                .filter(|line| line.heading.name == name)
                .collect()
        };
        // SFAP goes inside Work (the calendar right above at its level); Bugs inside SFAP goes along;
        // Personal after it stays
        let text = "# Work\n- a\n# SFAP\n- b\n## Bugs\n- c\n# Personal\n- d\n";
        let moved = shift_calendar(text, &calendar(text, "SFAP"), true);
        assert_eq!(
            moved,
            Ok(Some(
                "# Work\n- a\n## SFAP\n- b\n### Bugs\n- c\n# Personal\n- d\n".to_string()
            ))
        );
        // And back out
        let moved = moved.expect("moved").expect("moved");
        assert_eq!(
            shift_calendar(&moved, &calendar(&moved, "SFAP"), false),
            Ok(Some(text.to_string()))
        );
        // Cannot move: the first calendar has nothing above to go inside; a top-level one cannot go out;
        // Bugs right below its parent SFAP has no calendar at its own level above it
        assert_eq!(
            shift_calendar(text, &calendar(text, "Work"), true),
            Ok(None)
        );
        assert_eq!(
            shift_calendar(text, &calendar(text, "Work"), false),
            Ok(None)
        );
        assert_eq!(
            shift_calendar(text, &calendar(text, "Bugs"), true),
            Ok(None)
        );
        // At most 6 #s
        let deep = "# A\n###### B\n# C\n###### D\n";
        assert_eq!(shift_calendar(deep, &calendar(deep, "C"), true), Ok(None));
        // A heading changed elsewhere
        assert_eq!(
            shift_calendar(
                &text.replace("# SFAP", "# SFA"),
                &calendar(text, "SFAP"),
                true
            ),
            Err(Changed)
        );
    }

    #[test]
    fn a_new_calendar_goes_last_after_a_blank_line() {
        // Empty file: just the heading
        assert_eq!(
            add_calendar("", "Side projects"),
            ("# Side projects\n".into(), 1)
        );
        // After the last line, with a blank line between; the last line gets its missing newline first
        for text in ["- a\n", "- a"] {
            assert_eq!(
                add_calendar(text, " Side projects "),
                ("- a\n\n# Side projects\n".into(), 3),
                "{text:?}"
            );
        }
        // Already ends with a blank line: no second one
        assert_eq!(add_calendar("- a\n\n", "B"), ("- a\n\n# B\n".into(), 3));
        // \r\n files stay \r\n
        assert_eq!(
            add_calendar("# A\r\n- a\r\n", "B"),
            ("# A\r\n- a\r\n\r\n# B\r\n".into(), 4)
        );
    }

    #[test]
    fn check_accepts_exactly_one_item_without_mistakes() {
        let item = check("  - [ ] Send the weekly update @2026-10-16 17:30  ").expect("one item");
        assert_eq!(
            (item.title.as_str(), item.date),
            ("Send the weekly update", Some(date(2026, 10, 16)))
        );
        // Not starting with `- `: a heading, another list style, empty
        for line in [
            "Send the weekly update",
            "# Recurring",
            "* Send the weekly update",
            "",
        ] {
            assert_eq!(check(line), Err(Problem::NotAnItem), "{line:?}");
        }
        // A wrong date, time, range
        assert_eq!(
            check("- Trip @2026-13-01"),
            Err(Problem::Mistake(WarningKind::NoSuchDate(
                "@2026-13-01".into()
            )))
        );
        assert_eq!(
            check("- Meeting @2026-10-12 25:00"),
            Err(Problem::Mistake(WarningKind::NoSuchTime("25:00".into())))
        );
    }

    #[test]
    fn check_refuses_an_empty_title() {
        assert_eq!(check("- [ ]  @2026-10-17"), Err(Problem::EmptyTitle));
        assert_eq!(check("- [ ]"), Err(Problem::EmptyTitle));
        // Only `- ` typed: also an empty title (not "start with `- `")
        assert_eq!(check("- "), Err(Problem::EmptyTitle));
        assert_eq!(check("  -  "), Err(Problem::EmptyTitle));
    }

    /// The file for the add tests: "Work" has one item with subtasks and blank lines, followed by "Personal"
    const GROUPS: &str = "# Work\n- [ ] Write the design doc @2026-10-16\n  - [ ] Gather requirements\n\n  - [x] Draft the outline\n\n# Personal\n- Go hiking @2026-10-18\n";

    #[test]
    fn add_goes_after_the_last_item_of_the_group() {
        // The last item of "Work" (the design doc) counts together with its subtasks and the blank line
        // between them; the trailing blank line does not
        let (after, line) = add(
            GROUPS,
            "- [ ] Send the weekly update @2026-10-16",
            &Place::InGroupOf(item(GROUPS, 0)),
        )
        .expect("add");
        assert_eq!(
            after,
            "# Work\n- [ ] Write the design doc @2026-10-16\n  - [ ] Gather requirements\n\n  - [x] Draft the outline\n- [ ] Send the weekly update @2026-10-16\n\n# Personal\n- Go hiking @2026-10-18\n"
        );
        assert_eq!(line, 6);
        // Same when a subtask is chosen: added at the end of that group, not indented
        let (after, _) = add(GROUPS, "- x", &Place::InGroupOf(item(GROUPS, 1))).expect("add");
        assert!(after.contains("  - [x] Draft the outline\n- x\n\n# Personal"));
        // "Personal" is the last group: added at the end of the file
        let (after, line) = add(
            GROUPS,
            "- Dinner with friends @2026-10-17",
            &Place::InGroupOf(item(GROUPS, 3)),
        )
        .expect("add");
        assert!(after.ends_with("- Go hiking @2026-10-18\n- Dinner with friends @2026-10-17\n"));
        assert_eq!(line, 9);
    }

    #[test]
    fn move_to_calendar_takes_the_whole_block_to_the_end_of_the_calendar() {
        let path =
            |names: &[&str]| -> Vec<String> { names.iter().map(|&name| name.into()).collect() };
        // The design doc with its subtasks (and the blank line between them) goes below Personal's last item;
        // its line becomes the typed one; the blank line after the block stays in Work
        let (after, line) = move_to_calendar(
            GROUPS,
            &item(GROUPS, 0),
            "- [ ] Write the design doc v2 @2026-10-16",
            &path(&["Personal"]),
        )
        .expect("move");
        assert_eq!(
            after,
            "# Work\n\n# Personal\n- Go hiking @2026-10-18\n- [ ] Write the design doc v2 @2026-10-16\n  - [ ] Gather requirements\n\n  - [x] Draft the outline\n"
        );
        assert_eq!(line, 5);
        // A subtask comes out as an outermost item; its own subtask keeps the 2 extra spaces
        let text = "# Trip\n- Tokyo\n  - [ ] Book flights\n    - [ ] Compare prices\n  - [ ] Pack\n# Work\n- a\n";
        let (after, line) = move_to_calendar(
            text,
            &item(text, 1),
            "- [ ] Book flights @2026-11-01",
            &path(&["Work"]),
        )
        .expect("move");
        assert_eq!(
            after,
            "# Trip\n- Tokyo\n  - [ ] Pack\n# Work\n- a\n- [ ] Book flights @2026-11-01\n  - [ ] Compare prices\n"
        );
        assert_eq!(line, 6);
        // Up to a calendar with no items of its own: right under its heading
        let text = "# Personal\n## SFAP\n- c\n# Work\n- a\n  - a1\n- b\n";
        let (after, line) =
            move_to_calendar(text, &item(text, 1), "- a", &path(&["Personal"])).expect("move");
        assert_eq!(
            after,
            "# Personal\n- a\n  - a1\n## SFAP\n- c\n# Work\n- b\n"
        );
        assert_eq!(line, 2);
        // \r\n line endings stay; a last line with no newline gets one
        let text = "# A\r\n- x\r\n# B\r\n- y\r\n";
        let (after, _) =
            move_to_calendar(text, &item(text, 0), "- x", &path(&["B"])).expect("move");
        assert_eq!(after, "# A\r\n# B\r\n- y\r\n- x\r\n");
        let text = "# A\n- x\n# B\n- y";
        let (after, _) =
            move_to_calendar(text, &item(text, 0), "- x", &path(&["B"])).expect("move");
        assert_eq!(after, "# A\n# B\n- y\n- x\n");
        // The line changed elsewhere, or the calendar is gone: nothing moves
        let changed = item("# A\n- x2\n", 0);
        assert_eq!(
            move_to_calendar(text, &Item { line: 2, ..changed }, "- x", &path(&["B"])),
            Err(Changed)
        );
        assert_eq!(
            move_to_calendar(text, &item(text, 0), "- x", &path(&["Home"])),
            Err(Changed)
        );
    }

    #[test]
    fn add_into_a_calendar_by_its_path() {
        let path =
            |names: &[&str]| -> Vec<String> { names.iter().map(|&name| name.into()).collect() };
        // Work's last item is the design doc (with its subtasks and the blank line between them)
        let (after, line) =
            add(GROUPS, "- [ ] New", &Place::InCalendar(path(&["Work"]))).expect("add");
        assert!(after.contains("  - [x] Draft the outline\n- [ ] New\n\n# Personal"));
        assert_eq!(line, 6);
        // A calendar with calendars inside: after its own last item, before the first one inside; the one
        // inside by its whole path
        let text = "# Work\n- a\n## SFAP\n- b\n# Personal\n## SFAP\n- c\n";
        let (after, _) = add(text, "- new", &Place::InCalendar(path(&["Work"]))).expect("add");
        assert_eq!(
            after,
            "# Work\n- a\n- new\n## SFAP\n- b\n# Personal\n## SFAP\n- c\n"
        );
        let (after, _) = add(
            text,
            "- new",
            &Place::InCalendar(path(&["Personal", "SFAP"])),
        )
        .expect("add");
        assert_eq!(
            after,
            "# Work\n- a\n## SFAP\n- b\n# Personal\n## SFAP\n- c\n- new\n"
        );
        // No items of its own yet (Personal holds only a calendar): right under its heading
        let (after, line) =
            add(text, "- new", &Place::InCalendar(path(&["Personal"]))).expect("add");
        assert_eq!(
            after,
            "# Work\n- a\n## SFAP\n- b\n# Personal\n- new\n## SFAP\n- c\n"
        );
        assert_eq!(line, 6);
        // Gone (renamed or deleted elsewhere): nothing added
        assert_eq!(
            add(text, "- new", &Place::InCalendar(path(&["Home"]))),
            Err(Changed)
        );
    }

    #[test]
    fn add_keeps_the_last_items_notes_with_it() {
        // The design doc's note line (indented like its subtasks) comes after the last subtask: the new item
        // goes after the note line, which still belongs to the design doc
        let text = "# Work\n- [ ] Write the design doc\n  - [ ] Gather requirements\n  Note: due next week\n# Personal\n";
        let (after, line) =
            add(text, "- [ ] New task", &Place::InGroupOf(item(text, 0))).expect("add");
        assert_eq!(
            after,
            "# Work\n- [ ] Write the design doc\n  - [ ] Gather requirements\n  Note: due next week\n- [ ] New task\n# Personal\n"
        );
        assert_eq!(line, 5);
    }

    #[test]
    fn add_a_subtask_right_under_its_parent() {
        // A subtask of the design doc: added below its existing subtasks, indented 2 more spaces
        let (after, line) =
            add(GROUPS, "- [ ] Proofread", &Place::ChildOf(item(GROUPS, 0))).expect("add");
        assert!(after.contains("  - [x] Draft the outline\n  - [ ] Proofread\n\n# Personal"));
        assert_eq!(line, 6);
        // A subtask of a subtask: 2 more spaces again
        let (after, _) = add(
            GROUPS,
            "- [ ] Read old tickets",
            &Place::ChildOf(item(GROUPS, 1)),
        )
        .expect("add");
        assert!(after.contains(
            "  - [ ] Gather requirements\n    - [ ] Read old tickets\n\n  - [x] Draft the outline"
        ));
    }

    #[test]
    fn add_at_the_end_keeps_the_newlines() {
        // The last line has no newline: add one first; a \r\n file gets \r\n
        assert_eq!(add("- a", "- b", &Place::End), Ok(("- a\n- b\n".into(), 2)));
        assert_eq!(
            add("- a\r\n", "- b", &Place::End),
            Ok(("- a\r\n- b\r\n".into(), 2))
        );
        // An empty file (a task file that does not exist yet)
        assert_eq!(add("", "- b", &Place::End), Ok(("- b\n".into(), 1)));
        // The chosen item was changed elsewhere: nothing added
        assert_eq!(
            add(
                "- [ ] Something else\n",
                "- b",
                &Place::InGroupOf(item(GROUPS, 0))
            ),
            Err(Changed)
        );
    }

    #[test]
    fn delete_takes_the_subtasks_along() {
        // The design doc + 2 subtasks (the blank line between is deleted too) = 3 items;
        // the blank line before the next group stays
        assert_eq!(
            delete(GROUPS, &item(GROUPS, 0)),
            Ok(("# Work\n\n# Personal\n- Go hiking @2026-10-18\n".into(), 3))
        );
        // Deleting a subtask: only itself
        assert_eq!(
            delete(GROUPS, &item(GROUPS, 2)),
            Ok((
                "# Work\n- [ ] Write the design doc @2026-10-16\n  - [ ] Gather requirements\n\n\n# Personal\n- Go hiking @2026-10-18\n".into(),
                1
            ))
        );
        // No newline on the last line, \r\n: the other lines stay as they are
        assert_eq!(
            delete("- a\r\n- b", &item("- a\r\n- b", 1)),
            Ok(("- a\r\n".into(), 1))
        );
        // Changed elsewhere: nothing deleted
        assert_eq!(
            delete("- [ ] Something else\n", &item(GROUPS, 0)),
            Err(Changed)
        );
    }

    #[test]
    fn reindent_moves_the_whole_block() {
        // Indent the design doc by 2 spaces: its subtasks get 2 more as well, the blank line between stays;
        // the trailing blank line and the next group do not change
        assert_eq!(
            reindent(GROUPS, &item(GROUPS, 0), &item(GROUPS, 0), "  "),
            Ok(
                "# Work\n  - [ ] Write the design doc @2026-10-16\n    - [ ] Gather requirements\n\n    - [x] Draft the outline\n\n# Personal\n- Go hiking @2026-10-18\n"
                    .into()
            )
        );
        // Outdent "Draft the outline": line up with the parent (no indent)
        assert_eq!(
            reindent(GROUPS, &item(GROUPS, 2), &item(GROUPS, 0), ""),
            Ok(
                "# Work\n- [ ] Write the design doc @2026-10-16\n  - [ ] Gather requirements\n\n- [x] Draft the outline\n\n# Personal\n- Go hiking @2026-10-18\n"
                    .into()
            )
        );
        // Tab indent: the subtask's extra tab is kept; the line not starting with a tab gets spaces by width
        // (6 - 4 = 2 spaces); \r\n is kept
        let tabs = "- a\r\n\t- b\r\n\t\t- c\r\n      - d\r\n";
        assert_eq!(
            reindent(tabs, &item(tabs, 1), &item(tabs, 0), ""),
            Ok("- a\r\n- b\r\n\t- c\r\n  - d\r\n".into())
        );
        // Changed elsewhere: not changed
        assert_eq!(
            reindent(
                "- [ ] Something else\n",
                &item(GROUPS, 0),
                &item(GROUPS, 0),
                "  "
            ),
            Err(Changed)
        );
        // The item the indent is based on (the parent, the design doc) was changed elsewhere: not changed
        // either
        let changed = item("# Work\n- [ ] Write the design doc (edited)\n", 0);
        assert_eq!(
            reindent(GROUPS, &item(GROUPS, 2), &changed, ""),
            Err(Changed)
        );
    }

    #[test]
    fn line_map_follows_the_lines_that_moved() {
        // One line replaced: every line keeps its number
        let map = line_map("a\nb\nc\n", "a\nB\nc\n");
        assert_eq!([1, 2, 3].map(&map), [Some(1), Some(2), Some(3)]);
        // A line inserted at line 2: the ones after move down one line
        let map = line_map("a\nb\nc\n", "a\nnew\nb\nc\n");
        assert_eq!([1, 2, 3].map(&map), [Some(1), Some(3), Some(4)]);
        // Lines 2 and 3 deleted: the deleted ones have no match, the ones after move up two lines
        let map = line_map("a\nb\nc\nd\n", "a\nd\n");
        assert_eq!([1, 2, 3, 4].map(&map), [Some(1), None, None, Some(2)]);
        // Added at the end, deleted at the end
        let map = line_map("a\nb\n", "a\nb\nc\n");
        assert_eq!([1, 2].map(&map), [Some(1), Some(2)]);
        let map = line_map("a\nb\n", "a\n");
        assert_eq!([1, 2].map(&map), [Some(1), None]);
    }

    #[test]
    fn a_shallow_note_line_does_not_cut_the_subtasks_off_the_block() {
        // Under the design doc comes an unindented note line first, then the subtask "Gather requirements"
        // (the parser still counts it as a subtask of the design doc)
        let text = "- [ ] Write the design doc\nNote: due Friday\n  - [ ] Gather requirements\n- [ ] Send the weekly update\n";
        // Move up / down: the note line and "Gather requirements" go along with the design doc
        assert_eq!(
            swap(text, &item(text, 0), &item(text, 2)).map(|(text, _)| text),
            Ok("- [ ] Send the weekly update\n- [ ] Write the design doc\nNote: due Friday\n  - [ ] Gather requirements\n".into())
        );
        // Delete: "Gather requirements" goes too, 2 items in total
        assert_eq!(
            delete(text, &item(text, 0)),
            Ok(("- [ ] Send the weekly update\n".into(), 2))
        );
        // Indent: the note line under b and b1 are indented along with it; b1 is still a subtask of b
        let text = "- a\n- b\nnote\n  - b1\n";
        assert_eq!(
            reindent(text, &item(text, 1), &item(text, 0), "  "),
            Ok("- a\n  - b\n  note\n    - b1\n".into())
        );
    }

    #[test]
    fn swap_leaves_each_places_indent_where_it_was() {
        // Two siblings write their indent differently (a tab, two spaces): after the swap each one gets
        // the indent its new position had, and the subtasks' extra indent is kept
        let text = "- [ ] Write the design doc\n\t- [ ] Gather requirements\n\t\t- [ ] Read old tickets\n  - [ ] Send the weekly update\n";
        let (swapped, _) = swap(text, &item(text, 1), &item(text, 3)).expect("swap");
        assert_eq!(
            swapped,
            "- [ ] Write the design doc\n\t- [ ] Send the weekly update\n  - [ ] Gather requirements\n  \t- [ ] Read old tickets\n"
        );
        // Read back: the weekly update and "Gather requirements" are still subtasks of the design doc,
        // and "Read old tickets" is still under "Gather requirements"
        let parents: Vec<_> = parse(&swapped)
            .items
            .iter()
            .map(|item| item.parent)
            .collect();
        assert_eq!(parents, [None, Some(0), Some(0), Some(2)]);
    }

    #[test]
    fn swap_map_follows_the_moved_lines_even_when_they_read_the_same() {
        // Both items are called "Gym": lines 1-2 (Gym, Run) and lines 3-4 (Gym, Swim) swap places
        let text = "- [ ] Gym\n  - [ ] Run\n- [ ] Gym\n  - [ ] Swim\n- [ ] Sleep\n";
        let (after, swap) = swap(text, &item(text, 0), &item(text, 2)).expect("swap");
        assert_eq!(
            after,
            "- [ ] Gym\n  - [ ] Swim\n- [ ] Gym\n  - [ ] Run\n- [ ] Sleep\n"
        );
        // The original lines 1, 2 move to lines 3, 4, and lines 3, 4 move to lines 1, 2; the rest stay
        assert_eq!([1, 2, 3, 4, 5].map(|line| swap.map(line)), [3, 4, 1, 2, 5]);
        // A blank line between, blocks of different lengths: line 1 (1 line) swaps with lines 3-5 (3 lines),
        // and the blank line moves from line 2 to line 4
        let swap = Swap {
            start: 1,
            upper: 1,
            gap: 1,
            lower: 3,
        };
        assert_eq!(
            [1, 2, 3, 4, 5, 6].map(|line| swap.map(line)),
            [5, 4, 1, 2, 3, 6]
        );
        // Undo: moving back puts every line in its original place
        assert!((1..=6).all(|line| swap.reversed().map(swap.map(line)) == line));
    }

    #[test]
    fn swap_moves_the_blocks_with_their_subtasks() {
        // Two blocks with subtasks swap places; the rest stay
        let text = "- a\n  - a1\n- b\n  - b1\n- c\n";
        assert_eq!(
            swap(text, &item(text, 0), &item(text, 2)),
            Ok((
                "- b\n  - b1\n- a\n  - a1\n- c\n".into(),
                Swap {
                    start: 1,
                    upper: 2,
                    gap: 0,
                    lower: 2
                }
            ))
        );
        // The blank line between stays between; the one before the next group does not move either
        assert_eq!(
            swap(GROUPS, &item(GROUPS, 1), &item(GROUPS, 2)).map(|(text, _)| text),
            Ok(
                "# Work\n- [ ] Write the design doc @2026-10-16\n  - [x] Draft the outline\n\n  - [ ] Gather requirements\n\n# Personal\n- Go hiking @2026-10-18\n"
                    .into()
            )
        );
        // The lower item is the last line, without a newline, \r\n:
        // after the swap the last line has no newline either
        assert_eq!(
            swap("- a\r\n- b", &item("- a\r\n- b", 0), &item("- a\r\n- b", 1))
                .map(|(text, _)| text),
            Ok("- b\r\n- a".into())
        );
        // Changed elsewhere: not moved
        assert_eq!(
            swap("- [ ] Something else\n", &item(GROUPS, 0), &item(GROUPS, 1)),
            Err(Changed)
        );
    }

    #[test]
    fn replace_keeps_the_indentation() {
        let text = "- [ ] Pack\n  - [ ] Office @2026-11-14\n";
        assert_eq!(
            replace(
                text,
                &item(text, 1),
                "- [x] Office and living room @2026-11-15 "
            ),
            Ok("- [ ] Pack\n  - [x] Office and living room @2026-11-15\n".into())
        );
        // The line was changed elsewhere: not changed
        assert_eq!(
            replace("- [ ] Pack\n  - [ ] Bedroom\n", &item(text, 1), "- [ ] x"),
            Err(Changed)
        );
    }

    #[test]
    fn reschedule_changes_only_the_date() {
        let text = "# Recurring\n- [ ] Send the weekly update @2026-10-16 17:30 ^w-01\n- Meeting @2026-10-12 with the client\n";
        // The time and the ^id at the end of the line are both kept
        assert_eq!(
            reschedule(text, &item(text, 0), date(2026, 10, 19)),
            Ok("# Recurring\n- [ ] Send the weekly update @2026-10-19 17:30 ^w-01\n- Meeting @2026-10-12 with the client\n".into())
        );
        // The date in the middle: only that word changes, in the same place
        assert_eq!(
            reschedule(text, &item(text, 1), date(2026, 11, 2)),
            Ok("# Recurring\n- [ ] Send the weekly update @2026-10-16 17:30 ^w-01\n- Meeting @2026-11-02 with the client\n".into())
        );
    }

    #[test]
    fn reschedule_moves_a_multi_day_item_as_a_whole() {
        // A 6-day trip moved to start on 10/29: the last day moves along to 11/3 (into the next month);
        // moving it earlier works the same.
        // 🗼 is a multi-byte character before the date: the date is still found and replaced by byte position
        let text = "- Trip to Tokyo 🗼 @2026-10-20..2026-10-25 09:00\n";
        assert_eq!(
            reschedule(text, &item(text, 0), date(2026, 10, 29)),
            Ok("- Trip to Tokyo 🗼 @2026-10-29..2026-11-03 09:00\n".into())
        );
        assert_eq!(
            reschedule(text, &item(text, 0), date(2026, 10, 1)),
            Ok("- Trip to Tokyo 🗼 @2026-10-01..2026-10-06 09:00\n".into())
        );
        // Same first and last day (one day to the parser): becomes a plain date
        let text = "- Meeting @2026-10-20..2026-10-20\n";
        assert_eq!(
            reschedule(text, &item(text, 0), date(2026, 10, 21)),
            Ok("- Meeting @2026-10-21\n".into())
        );
    }

    #[test]
    fn reschedule_skips_text_that_only_looks_like_the_date() {
        // The first @2026-13-01 does not exist (text to the parser) and `x@2026-10-16` is not a whole word:
        // the real date is the one that changes
        let text = "- [ ] Pay taxes @2026-13-01 x@2026-10-16 @2026-10-16\n";
        assert_eq!(
            reschedule(text, &item(text, 0), date(2026, 10, 20)),
            Ok("- [ ] Pay taxes @2026-13-01 x@2026-10-16 @2026-10-20\n".into())
        );
    }

    #[test]
    fn reschedule_adds_a_date_to_an_unscheduled_item() {
        let text = "- [ ] Organize the bookshelf\n- [ ] Reply to Kevin ^r-02\n";
        assert_eq!(
            reschedule(text, &item(text, 0), date(2026, 10, 18)),
            Ok("- [ ] Organize the bookshelf @2026-10-18\n- [ ] Reply to Kevin ^r-02\n".into())
        );
        // A ^id at the end of the line: the date goes before it
        assert_eq!(
            reschedule(text, &item(text, 1), date(2026, 10, 18)),
            Ok("- [ ] Organize the bookshelf\n- [ ] Reply to Kevin @2026-10-18 ^r-02\n".into())
        );
        // Spaces after ^id, a tab before ^id: still added before ^id
        // (so Obsidian links to this line do not break)
        for text in [
            "- [ ] Reply to Kevin ^r-02  \n",
            "- [ ] Reply to Kevin\t^r-02\n",
        ] {
            assert_eq!(
                reschedule(text, &item(text, 0), date(2026, 10, 18)),
                Ok("- [ ] Reply to Kevin @2026-10-18 ^r-02\n".into()),
                "{text:?}"
            );
        }
    }

    #[test]
    fn toggle_flips_only_the_box_of_that_line() {
        let text = "# Recurring\n- [ ] Weekly update @2026-10-16\n  - [x] Subtask\n- [X] Capital X @2026-10-17\n";
        assert_eq!(
            toggle(text, &item(text, 0)),
            Ok("# Recurring\n- [x] Weekly update @2026-10-16\n  - [x] Subtask\n- [X] Capital X @2026-10-17\n".into())
        );
        // The indent is kept; both `[x]` and `[X]` go back to `[ ]`
        assert_eq!(
            toggle(text, &item(text, 1)),
            Ok("# Recurring\n- [ ] Weekly update @2026-10-16\n  - [ ] Subtask\n- [X] Capital X @2026-10-17\n".into())
        );
        assert_eq!(
            toggle(text, &item(text, 2)),
            Ok("# Recurring\n- [ ] Weekly update @2026-10-16\n  - [x] Subtask\n- [ ] Capital X @2026-10-17\n".into())
        );
    }

    #[test]
    fn toggle_keeps_crlf_and_a_missing_last_newline() {
        let text = "- [ ] a\r\n- [ ] b";
        assert_eq!(
            toggle(text, &item(text, 0)),
            Ok("- [x] a\r\n- [ ] b".into())
        );
        assert_eq!(
            toggle(text, &item(text, 1)),
            Ok("- [ ] a\r\n- [x] b".into())
        );
    }

    #[test]
    fn toggle_leaves_an_event_as_it_is() {
        let text = "- Dentist @2026-10-14\n";
        assert_eq!(toggle(text, &item(text, 0)), Ok(text.into()));
    }

    #[test]
    fn toggle_refuses_when_the_line_changed() {
        let text = "- [ ] a\n- [ ] b\n";
        let b = item(text, 1);
        // Elsewhere: a line added above (b moves down), b's text changed, the file cut short
        for changed in [
            "- [ ] new\n- [ ] a\n- [ ] b\n",
            "- [ ] a\n- [ ] b!\n",
            "- [ ] a\n",
        ] {
            assert_eq!(toggle(changed, &b), Err(Changed), "{changed:?}");
        }
    }

    #[test]
    fn the_day_is_left_out_in_the_editor_and_put_back_when_saving() {
        let day = date(2026, 10, 19);
        // (line in the file, what the editor shows on 10/19)
        for (line, shown) in [
            ("- Team sync @2026-10-19 10:00", "- Team sync @10:00"),
            ("- [ ] Pack the bag @2026-10-19", "- [ ] Pack the bag"),
            ("- [ ] Pack @2026-10-19 ^abc-1", "- [ ] Pack ^abc-1"),
            ("- [x] Done @2026-10-19 08:30 ^id", "- [x] Done @08:30 ^id"),
        ] {
            assert_eq!(hide_day(line, day), shown, "{line}");
            assert_eq!(show_day(shown, day), line, "{shown}");
        }
        // Left as they are: another day, a range, no date, two date-shaped words
        for line in [
            "- Team sync @2026-10-20 10:00",
            "- Trip @2026-10-19..2026-10-21",
            "- [ ] Someday",
            "- Moved @2026-10-19 from @2026-10-12",
        ] {
            assert_eq!(hide_day(line, day), line);
        }
        // Saving: a text with its own date keeps it; the date of a new item goes at the end; trailing
        // spaces do not count; only the prefix typed: the date still goes on (the title is still missing)
        assert_eq!(show_day("- Trip @2026-10-20", day), "- Trip @2026-10-20");
        assert_eq!(
            show_day("- [ ] Call Mom  ", day),
            "- [ ] Call Mom @2026-10-19"
        );
        assert_eq!(show_day("- [ ] ", day), "- [ ] @2026-10-19");
    }
}
