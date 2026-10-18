//! Pure functions that change the task file: given the file's current content, return the new content
//! (the caller writes the file with `files::write_atomically`)
//!
//! Only the part being changed is touched (check off, edit, reschedule: that line; delete, indent: that
//! block; move up / down: those two blocks; add: one inserted line). Every other character stays as it is,
//! including whether line endings are `\n` or `\r\n`.
//! Every change checks first: if the line is not what was read (e.g. it was just edited in Obsidian),
//! nothing is changed and `Changed` is returned

use crate::tasks::{self, Item, WarningKind};
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
}

/// Where in the file the added line goes
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Place {
    /// Below the last item of `item`'s group (under the same heading) and that item's subtasks
    InGroupOf(Item),
    /// A subtask of `item`: below it and its existing subtasks, indented 2 spaces more than it
    ChildOf(Item),
    /// At the end of the file
    End,
}

/// What the line typed in the editor (without indent) reads as: it counts only if it is exactly one item
/// with no mistakes
///
/// E.g. `- [ ] 交週報 @2026-10-16 17:30` → that item; `交週報` → `NotAnItem`; `- 出差 @2026-13-01` → `Mistake`
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

/// Delete: remove `item` together with the following lines indented deeper than it (subtasks, note lines,
/// blank lines in between too; trailing blank lines are not included);
/// return the whole file after deleting and how many items were deleted (itself included)
///
/// E.g. deleting `- 東京 5 天` → `  - [ ] 訂機票` and `    - [ ] 比價` under it go too, 3 items in total
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
/// E.g. `to` = `  `: `- 東京 5 天`, `  - [ ] 訂機票` → `  - 東京 5 天`, `    - [ ] 訂機票`.
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
/// E.g. `- 寫報告`, `備註：週五前`, `  - 找資料`, `- 交週報` → the block of 寫報告 is the first 3 lines
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

/// Check / uncheck a task: `- [ ] 週會` ↔ `- [x] 週會`; return the whole file after the change
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
/// `- [ ] 週會` → `- [x] 週會`; `- [x]`, `- [X]` → `- [ ]`; lines without a box (events) stay as is
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
    fn check_accepts_exactly_one_item_without_mistakes() {
        let item = check("  - [ ] 交週報 @2026-10-16 17:30  ").expect("一件事");
        assert_eq!(
            (item.title.as_str(), item.date),
            ("交週報", Some(date(2026, 10, 16)))
        );
        // Not starting with `- `: a heading, another list style, empty
        for line in ["交週報", "# 例行", "* 交週報", ""] {
            assert_eq!(check(line), Err(Problem::NotAnItem), "{line:?}");
        }
        // A wrong date, time, range
        assert_eq!(
            check("- 出差 @2026-13-01"),
            Err(Problem::Mistake(WarningKind::NoSuchDate(
                "@2026-13-01".into()
            )))
        );
        assert_eq!(
            check("- 會議 @2026-10-12 25:00"),
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

    /// The file for the add tests: 「工作」 has one item with subtasks and blank lines, followed by 「生活」
    const GROUPS: &str = "# 工作\n- [ ] 寫報告 @2026-10-16\n  - [ ] 找資料\n\n  - [x] 列大綱\n\n# 生活\n- 爬山 @2026-10-18\n";

    #[test]
    fn add_goes_after_the_last_item_of_the_group() {
        // The last item of 「工作」 (寫報告) counts together with its subtasks and the blank line between them;
        // the trailing blank line does not
        let (after, line) = add(
            GROUPS,
            "- [ ] 交週報 @2026-10-16",
            &Place::InGroupOf(item(GROUPS, 0)),
        )
        .expect("add");
        assert_eq!(
            after,
            "# 工作\n- [ ] 寫報告 @2026-10-16\n  - [ ] 找資料\n\n  - [x] 列大綱\n- [ ] 交週報 @2026-10-16\n\n# 生活\n- 爬山 @2026-10-18\n"
        );
        assert_eq!(line, 6);
        // Same when a subtask is chosen: added at the end of that group, not indented
        let (after, _) = add(GROUPS, "- x", &Place::InGroupOf(item(GROUPS, 1))).expect("add");
        assert!(after.contains("  - [x] 列大綱\n- x\n\n# 生活"));
        // 「生活」 is the last group: added at the end of the file
        let (after, line) = add(
            GROUPS,
            "- 聚餐 @2026-10-17",
            &Place::InGroupOf(item(GROUPS, 3)),
        )
        .expect("add");
        assert!(after.ends_with("- 爬山 @2026-10-18\n- 聚餐 @2026-10-17\n"));
        assert_eq!(line, 9);
    }

    #[test]
    fn add_keeps_the_last_items_notes_with_it() {
        // The note line of 寫報告 (indented like its subtasks) comes after the last subtask: the new item goes
        // after the note line, which still belongs to 寫報告
        let text = "# 工作\n- [ ] 寫報告\n  - [ ] 找資料\n  備註：下週交\n# 生活\n";
        let (after, line) = add(text, "- [ ] 新的", &Place::InGroupOf(item(text, 0))).expect("add");
        assert_eq!(
            after,
            "# 工作\n- [ ] 寫報告\n  - [ ] 找資料\n  備註：下週交\n- [ ] 新的\n# 生活\n"
        );
        assert_eq!(line, 5);
    }

    #[test]
    fn add_a_subtask_right_under_its_parent() {
        // A subtask of 寫報告: added below its existing subtasks, indented 2 more spaces
        let (after, line) =
            add(GROUPS, "- [ ] 校稿", &Place::ChildOf(item(GROUPS, 0))).expect("add");
        assert!(after.contains("  - [x] 列大綱\n  - [ ] 校稿\n\n# 生活"));
        assert_eq!(line, 6);
        // A subtask of a subtask: 2 more spaces again
        let (after, _) =
            add(GROUPS, "- [ ] 查文獻", &Place::ChildOf(item(GROUPS, 1))).expect("add");
        assert!(after.contains("  - [ ] 找資料\n    - [ ] 查文獻\n\n  - [x] 列大綱"));
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
            add("- [ ] 別的\n", "- b", &Place::InGroupOf(item(GROUPS, 0))),
            Err(Changed)
        );
    }

    #[test]
    fn delete_takes_the_subtasks_along() {
        // 寫報告 + 2 subtasks (the blank line between is deleted too) = 3 items;
        // the blank line before the next group stays
        assert_eq!(
            delete(GROUPS, &item(GROUPS, 0)),
            Ok(("# 工作\n\n# 生活\n- 爬山 @2026-10-18\n".into(), 3))
        );
        // Deleting a subtask: only itself
        assert_eq!(
            delete(GROUPS, &item(GROUPS, 2)),
            Ok((
                "# 工作\n- [ ] 寫報告 @2026-10-16\n  - [ ] 找資料\n\n\n# 生活\n- 爬山 @2026-10-18\n".into(),
                1
            ))
        );
        // No newline on the last line, \r\n: the other lines stay as they are
        assert_eq!(
            delete("- a\r\n- b", &item("- a\r\n- b", 1)),
            Ok(("- a\r\n".into(), 1))
        );
        // Changed elsewhere: nothing deleted
        assert_eq!(delete("- [ ] 別的\n", &item(GROUPS, 0)), Err(Changed));
    }

    #[test]
    fn reindent_moves_the_whole_block() {
        // Indent 寫報告 by 2 spaces: its subtasks get 2 more as well, the blank line between stays;
        // the trailing blank line and the next group do not change
        assert_eq!(
            reindent(GROUPS, &item(GROUPS, 0), &item(GROUPS, 0), "  "),
            Ok(
                "# 工作\n  - [ ] 寫報告 @2026-10-16\n    - [ ] 找資料\n\n    - [x] 列大綱\n\n# 生活\n- 爬山 @2026-10-18\n"
                    .into()
            )
        );
        // Outdent 列大綱: line up with the parent (no indent)
        assert_eq!(
            reindent(GROUPS, &item(GROUPS, 2), &item(GROUPS, 0), ""),
            Ok(
                "# 工作\n- [ ] 寫報告 @2026-10-16\n  - [ ] 找資料\n\n- [x] 列大綱\n\n# 生活\n- 爬山 @2026-10-18\n"
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
            reindent("- [ ] 別的\n", &item(GROUPS, 0), &item(GROUPS, 0), "  "),
            Err(Changed)
        );
        // The item the indent is based on (the parent 寫報告) was changed elsewhere: not changed either
        let changed = item("# 工作\n- [ ] 寫報告（改過）\n", 0);
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
        // Under 寫報告 comes an unindented note line first, then the subtask 找資料 (the parser still counts
        // 找資料 as a subtask of 寫報告)
        let text = "- [ ] 寫報告\n備註：週五前\n  - [ ] 找資料\n- [ ] 交週報\n";
        // Move up / down: the note line and 找資料 go along with 寫報告
        assert_eq!(
            swap(text, &item(text, 0), &item(text, 2)).map(|(text, _)| text),
            Ok("- [ ] 交週報\n- [ ] 寫報告\n備註：週五前\n  - [ ] 找資料\n".into())
        );
        // Delete: 找資料 goes too, 2 items in total
        assert_eq!(
            delete(text, &item(text, 0)),
            Ok(("- [ ] 交週報\n".into(), 2))
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
        let text = "- [ ] 寫報告\n\t- [ ] 找資料\n\t\t- [ ] 查文獻\n  - [ ] 交週報\n";
        let (swapped, _) = swap(text, &item(text, 1), &item(text, 3)).expect("swap");
        assert_eq!(
            swapped,
            "- [ ] 寫報告\n\t- [ ] 交週報\n  - [ ] 找資料\n  \t- [ ] 查文獻\n"
        );
        // Read back: 交週報 and 找資料 are still subtasks of 寫報告, and 查文獻 is still under 找資料
        let parents: Vec<_> = parse(&swapped)
            .items
            .iter()
            .map(|item| item.parent)
            .collect();
        assert_eq!(parents, [None, Some(0), Some(0), Some(2)]);
    }

    #[test]
    fn swap_map_follows_the_moved_lines_even_when_they_read_the_same() {
        // Both items are called 「運動」: lines 1-2 (運動, 跑步) and lines 3-4 (運動, 游泳) swap places
        let text = "- [ ] 運動\n  - [ ] 跑步\n- [ ] 運動\n  - [ ] 游泳\n- [ ] 睡覺\n";
        let (after, swap) = swap(text, &item(text, 0), &item(text, 2)).expect("swap");
        assert_eq!(
            after,
            "- [ ] 運動\n  - [ ] 游泳\n- [ ] 運動\n  - [ ] 跑步\n- [ ] 睡覺\n"
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
                "# 工作\n- [ ] 寫報告 @2026-10-16\n  - [x] 列大綱\n\n  - [ ] 找資料\n\n# 生活\n- 爬山 @2026-10-18\n"
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
            swap("- [ ] 別的\n", &item(GROUPS, 0), &item(GROUPS, 1)),
            Err(Changed)
        );
    }

    #[test]
    fn replace_keeps_the_indentation() {
        let text = "- [ ] 打包\n  - [ ] 書房 @2026-11-14\n";
        assert_eq!(
            replace(text, &item(text, 1), "- [x] 書房和客廳 @2026-11-15 "),
            Ok("- [ ] 打包\n  - [x] 書房和客廳 @2026-11-15\n".into())
        );
        // The line was changed elsewhere: not changed
        assert_eq!(
            replace("- [ ] 打包\n  - [ ] 臥室\n", &item(text, 1), "- [ ] x"),
            Err(Changed)
        );
    }

    #[test]
    fn reschedule_changes_only_the_date() {
        let text = "# 例行\n- [ ] 交週報 @2026-10-16 17:30 ^w-01\n- 開會 @2026-10-12 跟客戶\n";
        // The time and the ^id at the end of the line are both kept
        assert_eq!(
            reschedule(text, &item(text, 0), date(2026, 10, 19)),
            Ok("# 例行\n- [ ] 交週報 @2026-10-19 17:30 ^w-01\n- 開會 @2026-10-12 跟客戶\n".into())
        );
        // The date in the middle: only that word changes, in the same place
        assert_eq!(
            reschedule(text, &item(text, 1), date(2026, 11, 2)),
            Ok("# 例行\n- [ ] 交週報 @2026-10-16 17:30 ^w-01\n- 開會 @2026-11-02 跟客戶\n".into())
        );
    }

    #[test]
    fn reschedule_moves_a_multi_day_item_as_a_whole() {
        // A 6-day trip moved to start on 10/29: the last day moves along to 11/3 (into the next month);
        // moving it earlier works the same
        let text = "- 東京出差 @2026-10-20..2026-10-25 09:00\n";
        assert_eq!(
            reschedule(text, &item(text, 0), date(2026, 10, 29)),
            Ok("- 東京出差 @2026-10-29..2026-11-03 09:00\n".into())
        );
        assert_eq!(
            reschedule(text, &item(text, 0), date(2026, 10, 1)),
            Ok("- 東京出差 @2026-10-01..2026-10-06 09:00\n".into())
        );
        // Same first and last day (one day to the parser): becomes a plain date
        let text = "- 開會 @2026-10-20..2026-10-20\n";
        assert_eq!(
            reschedule(text, &item(text, 0), date(2026, 10, 21)),
            Ok("- 開會 @2026-10-21\n".into())
        );
    }

    #[test]
    fn reschedule_skips_text_that_only_looks_like_the_date() {
        // The first @2026-13-01 does not exist (text to the parser) and `x@2026-10-16` is not a whole word:
        // the real date is the one that changes
        let text = "- [ ] 繳稅 @2026-13-01 x@2026-10-16 @2026-10-16\n";
        assert_eq!(
            reschedule(text, &item(text, 0), date(2026, 10, 20)),
            Ok("- [ ] 繳稅 @2026-13-01 x@2026-10-16 @2026-10-20\n".into())
        );
    }

    #[test]
    fn reschedule_adds_a_date_to_an_unscheduled_item() {
        let text = "- [ ] 整理書櫃\n- [ ] 回信 ^r-02\n";
        assert_eq!(
            reschedule(text, &item(text, 0), date(2026, 10, 18)),
            Ok("- [ ] 整理書櫃 @2026-10-18\n- [ ] 回信 ^r-02\n".into())
        );
        // A ^id at the end of the line: the date goes before it
        assert_eq!(
            reschedule(text, &item(text, 1), date(2026, 10, 18)),
            Ok("- [ ] 整理書櫃\n- [ ] 回信 @2026-10-18 ^r-02\n".into())
        );
        // Spaces after ^id, a tab before ^id: still added before ^id
        // (so Obsidian links to this line do not break)
        for text in ["- [ ] 回信 ^r-02  \n", "- [ ] 回信\t^r-02\n"] {
            assert_eq!(
                reschedule(text, &item(text, 0), date(2026, 10, 18)),
                Ok("- [ ] 回信 @2026-10-18 ^r-02\n".into()),
                "{text:?}"
            );
        }
    }

    #[test]
    fn toggle_flips_only_the_box_of_that_line() {
        let text = "# 例行\n- [ ] 週報 @2026-10-16\n  - [x] 子任務\n- [X] 大寫 @2026-10-17\n";
        assert_eq!(
            toggle(text, &item(text, 0)),
            Ok("# 例行\n- [x] 週報 @2026-10-16\n  - [x] 子任務\n- [X] 大寫 @2026-10-17\n".into())
        );
        // The indent is kept; both `[x]` and `[X]` go back to `[ ]`
        assert_eq!(
            toggle(text, &item(text, 1)),
            Ok("# 例行\n- [ ] 週報 @2026-10-16\n  - [ ] 子任務\n- [X] 大寫 @2026-10-17\n".into())
        );
        assert_eq!(
            toggle(text, &item(text, 2)),
            Ok("# 例行\n- [ ] 週報 @2026-10-16\n  - [x] 子任務\n- [ ] 大寫 @2026-10-17\n".into())
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
        let text = "- 看牙醫 @2026-10-14\n";
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
}
