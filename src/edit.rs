//! Pure functions that change the task file: given the file's current content, return the new content
//! (the caller writes the file with `files::write_atomically`)
//!
//! Only the part being changed is touched (check off, edit, reschedule: that line; delete, indent, move to
//! another calendar: that block; move up / down: those two blocks; add: one inserted line). Every other character stays as it is,
//! including whether line endings are `\n` or `\r\n`.
//! Every change checks first: if the line is not what was read (e.g. it was just edited in Obsidian),
//! nothing is changed and `Changed` is returned

use crate::tasks::{self, Heading, HeadingLine, Item, Mark, WarningKind};
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

/// Add a calendar inside the calendar with this `parent` path (empty = at the top level, `add_calendar`): the
/// heading `<one # more than the parent's> <name>` after a blank line, at the end of the parent's last
/// section (after its items and the calendars inside it, before the blank lines in front of the next heading
/// with as many `#` or fewer); return the new file content and the heading's line number (from 1).
/// `Changed` when there is no such calendar, or it already has 6 `#` (the editor does not offer that)
///
/// E.g. adding `Bugs` in `Work > SFAP` (`## SFAP`, `- b`, blank, `# Personal`) → `## SFAP`, `- b`, blank,
/// `### Bugs`, blank, `# Personal`
pub fn add_calendar_in(
    text: &str,
    parent: &[String],
    name: &str,
) -> Result<(String, usize), Changed> {
    if parent.is_empty() {
        return Ok(add_calendar(text, name));
    }
    let (at, level) = section_end(text, parent).ok_or(Changed)?;
    if level >= 6 {
        return Err(Changed);
    }
    let newline = if text.contains("\r\n") { "\r\n" } else { "\n" };
    let segments: Vec<&str> = text.split_inclusive('\n').collect();
    let mut result = segments[..at].concat();
    // The line before is the file's last line and has no newline: add one first
    if !result.ends_with('\n') {
        result.push_str(newline);
    }
    result.push_str(&format!(
        "{newline}{} {}{newline}",
        "#".repeat(level + 1),
        name.trim()
    ));
    result.push_str(&segments[at..].concat());
    Ok((result, at + 2))
}

/// Move a calendar inside the calendar with this `parent` path (empty = to the top level), renamed to `new` (what
/// was typed in the editor, checked with `check_calendar` first): each of its sections (`headings`; every heading
/// line of it, with everything under it up to the next heading with as many `#` or fewer: its items and the
/// calendars inside it) is taken out, and they go together, in file order and a blank line apart, to the end of
/// the parent's last section (after a blank line; the top level: the end of the file). Its own headings become
/// `<one # more than the parent's> <new>`, the headings inside it keep how much deeper they are.
/// Return the changed file, or `None` when it cannot move: into itself or a calendar inside it, or a heading
/// would get more than 6 `#`. `Changed` if one of its heading lines differs from when it was read, or the
/// parent is gone
///
/// E.g. moving `## SFAP` (with `### Bugs` inside) from `# Work` to `# Personal`: `## SFAP`, its items,
/// `### Bugs` and its items go after Personal's last item
pub fn move_calendar(
    text: &str,
    headings: &[HeadingLine],
    new: &str,
    parent: &[String],
) -> Result<Option<String>, Changed> {
    let lines: Vec<&str> = text.lines().collect();
    if headings
        .iter()
        .any(|heading| lines.get(heading.line - 1) != Some(&heading.text.as_str()))
    {
        return Err(Changed);
    }
    let Some(first) = headings.first().map(|line| &line.heading) else {
        return Err(Changed);
    };
    let own: Vec<&String> = first.parents.iter().chain([&first.name]).collect();
    if parent.len() >= own.len() && parent.iter().zip(&own).all(|(a, b)| a == *b) {
        return Ok(None);
    }
    let newline = if text.contains("\r\n") { "\r\n" } else { "\n" };
    // Whole segments including the newline, so the newlines (\n or \r\n) stay as they were
    let segments: Vec<&str> = text.split_inclusive('\n').collect();
    let all = tasks::parse(text).headings;
    // Each section: (first line, the line after it, its number of `#`), 0-based
    let sections: Vec<(usize, usize, usize)> = headings
        .iter()
        .map(|heading| {
            let end = all
                .iter()
                .find(|other| {
                    other.line > heading.line && other.heading.level <= heading.heading.level
                })
                .map_or(segments.len(), |other| other.line - 1);
            (heading.line - 1, end, heading.heading.level)
        })
        .collect();
    let mut moved = vec![false; segments.len()];
    for &(start, end, _) in &sections {
        moved[start..end].fill(true);
    }
    // The file without them; blank lines left at its end go too, unless the file already ended that way
    let mut rest: Vec<&str> = segments
        .iter()
        .zip(&moved)
        .filter(|&(_, &gone)| !gone)
        .map(|(segment, _)| *segment)
        .collect();
    if lines.last().is_some_and(|line| !line.trim().is_empty()) {
        while rest.last().is_some_and(|segment| segment.trim().is_empty()) {
            rest.pop();
        }
    }
    let rest_text = rest.concat();
    // Where it goes (the index of the line it goes before) and how many `#` its own headings get
    let (at, level) = if parent.is_empty() {
        let mut at = rest.len();
        while at > 0 && rest[at - 1].trim().is_empty() {
            at -= 1;
        }
        (at, 1)
    } else {
        let (at, parent_level) = section_end(&rest_text, parent).ok_or(Changed)?;
        (at, parent_level + 1)
    };
    // The sections as they will be, each without its trailing blank lines
    let mut block = String::new();
    for (index, &(start, end, own_level)) in sections.iter().enumerate() {
        if index > 0 {
            block.push_str(newline);
        }
        let mut last = end;
        while last > start + 1 && lines[last - 1].trim().is_empty() {
            last -= 1;
        }
        for (row, text) in lines.iter().enumerate().take(last).skip(start) {
            let line = if row == start {
                format!("{} {}", "#".repeat(level), new.trim())
            } else if let Some(inside) = all.iter().find(|other| other.line == row + 1) {
                let deeper = inside.heading.level - own_level;
                format!(
                    "{}{}",
                    "#".repeat(level + deeper),
                    text.trim_start_matches('#')
                )
            } else {
                (*text).to_string()
            };
            if line.starts_with("#######") {
                return Ok(None);
            }
            block.push_str(&line);
            block.push_str(newline);
        }
    }
    if level > 6 {
        return Ok(None);
    }
    let mut result = rest[..at].concat();
    // The line before is the file's last line and has no newline: add one first
    if !result.is_empty() && !result.ends_with('\n') {
        result.push_str(newline);
    }
    if !result.is_empty() {
        result.push_str(newline);
    }
    result.push_str(&block);
    result.push_str(&rest[at..].concat());
    Ok(Some(result))
}

/// Where the last section of the calendar with this `path` ends, and its number of `#`: the index of the line
/// right after its last line that is not blank, before the next heading with as many `#` or fewer (or the
/// end of the file); `None` when there is no such calendar
fn section_end(text: &str, path: &[String]) -> Option<(usize, usize)> {
    let lines: Vec<&str> = text.lines().collect();
    let headings = tasks::parse(text).headings;
    let last = headings.iter().rev().find(|line| {
        let heading = &line.heading;
        heading
            .parents
            .iter()
            .chain([&heading.name])
            .eq(path.iter())
    })?;
    let end = headings
        .iter()
        .find(|other| other.line > last.line && other.heading.level <= last.heading.level)
        .map_or(lines.len(), |other| other.line - 1);
    // Back over the blank lines in front of the next heading (the heading line itself stays)
    let mut at = end;
    while at > last.line && lines[at - 1].trim().is_empty() {
        at -= 1;
    }
    Some((at, last.heading.level))
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

/// Start / stop working on a task (`/`): `[ ]` or `[x]` → `[/]`, `[/]` → `[ ]`; only the box changes;
/// return the whole changed file
pub fn toggle_progress(text: &str, item: &Item) -> Result<String, Changed> {
    let next = |mark| {
        if mark == Mark::InProgress {
            "[ ]"
        } else {
            "[/]"
        }
    };
    replace_line(text, item, &with_box(&item.line_text, next))
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

/// Check / uncheck a line (`line` is a parsed item's line)
///
/// `- [ ] Team sync`, `- [/] Team sync` → `- [x] Team sync`; `- [x]`, `- [X]` → `- [ ]`
fn toggled(line: &str) -> String {
    with_box(line, |mark| if mark == Mark::Done { "[ ]" } else { "[x]" })
}

/// The line with its box replaced by what `next` gives for the box it has now; the indent and the rest of
/// the text stay as is; lines without a box (events) stay as is
fn with_box(line: &str, next: impl Fn(Mark) -> &'static str) -> String {
    let trimmed = line.trim_start();
    let indent = &line[..line.len() - trimmed.len()];
    let rest = trimmed.strip_prefix("- ").unwrap_or(trimmed);
    match tasks::checkbox(rest) {
        // Has a box: replace only the 3 characters of `[ ]` / `[/]` / `[x]`
        Some((mark, after)) => format!("{indent}- {}{after}", next(mark)),
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

/// Keeps an Obsidian block id at the very end of the line, the only place Obsidian (and other tools
/// reading the file) look for it: when nothing but a priority, a date or a time follows the id, it moves
/// behind them. Those count wherever they are in the line, so the item reads the same
///
/// E.g. `- [ ] Pay rent ^life-05 ! @2026-10-19` → `- [ ] Pay rent ! @2026-10-19 ^life-05`; with a title
/// word after it (`Read about ^regex today`) the line stays as it is
pub fn block_id_last(line: &str) -> String {
    let words = word_spans(line);
    let word = |&(from, to): &(usize, usize)| &line[from..to];
    let Some(position) = words
        .iter()
        .rposition(|span| tasks::is_block_id(word(span)))
    else {
        return line.to_string();
    };
    let after = &words[position + 1..];
    let is_field = |text: &str| {
        tasks::Priority::from_word(text).is_some()
            || is_date_word(text)
            || tasks::looks_like_time(text)
    };
    if after.is_empty() || !after.iter().all(|span| is_field(word(span))) {
        return line.to_string();
    }
    // Take the id out with the spaces before it, put it back at the end
    let (from, to) = words[position];
    format!(
        "{}{} {}",
        line[..from].trim_end(),
        line[to..].trim_end(),
        &line[from..to]
    )
}

#[cfg(test)]
mod tests;
