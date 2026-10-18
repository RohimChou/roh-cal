//! The task file parser: plain text in, items out (format in `docs/task-file-format.md`)
//!
//! Each item records its line number and the line's original text, so that when writing back to the file
//! we know which line to change and whether someone else has changed that line

use jiff::civil::{Date, Time};

/// The result of parsing
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Parsed {
    /// All items, in file order
    pub items: Vec<Item>,
    /// Mistakes that do not stop the file from being read, e.g. `@2026-13-01`
    pub warnings: Vec<Warning>,
    /// Every heading, in file order (also ones with nothing under them yet: an empty calendar)
    pub headings: Vec<HeadingLine>,
}

/// One heading line of the file, e.g. line 3 `# Payments API v2`
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HeadingLine {
    /// Line number (from 1)
    pub line: usize,
    /// The line exactly as it is in the file (to check it was not changed elsewhere before rewriting it)
    pub text: String,
    /// What it reads as
    pub heading: Heading,
}

/// The kind of item: with or without a box (rules 1 and 2 of the spec)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// `- [ ] text` / `- [x] text`: a task, which you check off yourself
    Task,
    /// `- text` (no box): an event (meeting, holiday, milestone); it cannot be checked off,
    /// and it is over once its day has passed
    Event,
}

/// One item in the task file (a line starting with `- `)
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Item {
    /// Task (with a box) or event (no box)
    pub kind: Kind,
    /// Whether the task is done: `[x]` / `[X]` is done, `[ ]` is not yet; always `false` for an event
    /// (events cannot be checked off)
    pub done: bool,
    /// Title: the text left after removing `- [ ] `, the date (and time) and a trailing `^id`, with runs of
    /// spaces collapsed to one, e.g. `Implement the refund API`
    pub title: String,
    /// The day it shows on in the calendar; none means not scheduled yet
    pub date: Option<Date>,
    /// The last day of a multi-day item (both ends included), e.g. 10/25 for `@2026-10-20..2026-10-25`;
    /// `None` for a single day
    pub end: Option<Date>,
    /// The time (only when a date is written; for a multi-day item, the time on the first day)
    pub time: Option<Time>,
    /// Which group it belongs to (the nearest heading above); `None` before the file's first heading
    pub group: Option<Heading>,
    /// The number of the heading it is under in the file (0 before the first heading): two headings with the
    /// same name give the same `group` but differ here
    pub section: usize,
    /// Nesting level: 0 = outermost, 1 = subtask, 2 = subtask of a subtask…
    pub depth: usize,
    /// The parent's index in `items`; `None` at the outermost level
    pub parent: Option<usize>,
    /// The line number in the file (from 1, as editors show it)
    pub line: usize,
    /// The line's original text (the whole line, with indent, without the newline),
    /// e.g. `  - [ ] Write integration tests @2026-10-13`
    pub line_text: String,
}

impl Item {
    /// Not over yet: an unfinished task, or an event not yet past (today counts; a multi-day one goes by
    /// its last day). Its day is orange in the calendar,
    /// and it uses that day's color in the agenda; finished ones (done tasks, past events) turn gray
    pub fn is_open(&self, today: Date) -> bool {
        match self.kind {
            Kind::Task => !self.done,
            Kind::Event => self.last_day().is_some_and(|last| last >= today),
        }
    }

    /// The last day: the end day for a multi-day item, otherwise its one day; `None` without a date
    pub fn last_day(&self) -> Option<Date> {
        self.end.or(self.date)
    }

    /// Whether `day` is one of this item's days (multi-day: the first, the last and every day between)
    pub fn covers(&self, day: Date) -> bool {
        self.date.is_some_and(|first| first <= day)
            && self.last_day().is_some_and(|last| day <= last)
    }
}

/// The date and time taken from an item's text (the first `@date` or `@date..date`,
/// and the time right after it)
#[derive(Debug, Default)]
struct When {
    /// The day (the first day for a multi-day item)
    date: Option<Date>,
    /// The last day of a multi-day item; `None` for a single day
    end: Option<Date>,
    /// The time
    time: Option<Time>,
}

/// A group heading, e.g. `# Payments API v2`, `# US holidays type:holiday`
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Heading {
    /// How many `#`: the tasks page indents the heading by this. Groups are not nested yet;
    /// when nested groups come later, existing task files won't need to change
    pub level: usize,
    /// The heading text (without `type:holiday`)
    pub name: String,
    /// Holiday calendar: the heading has `type:holiday`, so the days of the items under it are holidays
    pub holiday: bool,
}

/// A problem found while parsing that does not stop the file from being read
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Warning {
    /// The line number in the file (from 1)
    pub line: usize,
    /// Which kind of problem
    pub kind: WarningKind,
}

/// Kinds of problem; the screen picks the message text by language, this only records what the problem is
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WarningKind {
    /// Looks like a date but does not exist, e.g. `@2026-13-01`, `@2026-02-30`; treated as plain text
    NoSuchDate(String),
    /// Looks like a time but does not exist, e.g. `25:00` in `@2026-10-12 25:00`; treated as plain text
    NoSuchTime(String),
    /// A multi-day range that ends before it starts, e.g. `@2026-10-25..2026-10-20`; treated as plain text
    EndBeforeStart(String),
}

/// The line where the front matter (an Obsidian note's properties) at the top of the file ends: from a
/// first line of `---` to the next `---` (inclusive);
/// `None` without front matter (the first line is not `---`, or there is no closing `---`).
/// A `- work` inside is a YAML list, not an item
///
/// E.g. `---` / `tags:` / `  - work` / `---` / `- [ ] Send the weekly update` → `Some(4)`; reading starts
/// at line 5
fn front_matter_end(text: &str) -> Option<usize> {
    let mut lines = text.lines();
    if lines.next()?.trim_end() != "---" {
        return None;
    }
    // Line 1 is the opening `---`; find how many lines after it the closing `---` is
    lines
        .position(|line| line.trim_end() == "---")
        .map(|index| index + 2)
}

/// Parse the whole task file
pub fn parse(text: &str) -> Parsed {
    let mut parsed = Parsed::default();
    let mut group = None;
    // The number of the heading we are under (0 before the first heading)
    let mut section = 0;
    // (indent width, index in items) of the current chain of items, used to find the parent
    let mut ancestors: Vec<(usize, usize)> = Vec::new();
    // Whether we are inside a ``` code block (nothing inside counts)
    let mut in_code_block = false;
    // The line where the front matter at the top of the file ends (nothing inside counts)
    let front_matter = front_matter_end(text).unwrap_or(0);

    // lines() treats both \n and \r\n as line breaks
    for (index, raw) in text.lines().enumerate() {
        let line = index + 1;
        let trimmed = raw.trim_start();
        if line <= front_matter {
            continue;
        }

        // ``` or ~~~ starts / ends a code block
        if trimmed.starts_with("```") || trimmed.starts_with("~~~") {
            in_code_block = !in_code_block;
            continue;
        }
        if in_code_block {
            continue;
        }

        // A heading: switch group; in the new group, items start again from the outermost level
        if let Some(heading) = parse_heading(raw) {
            parsed.headings.push(HeadingLine {
                line,
                text: raw.to_string(),
                heading: heading.clone(),
            });
            group = Some(heading);
            section += 1;
            ancestors.clear();
            continue;
        }

        let Some((kind, done, rest)) = parse_item_start(trimmed) else {
            continue;
        };

        // Find the parent: pop the ones not indented less than this item; the last one left is the parent
        let indent = indent_width(raw);
        while ancestors.last().is_some_and(|&(width, _)| width >= indent) {
            ancestors.pop();
        }
        let parent = ancestors.last().map(|&(_, position)| position);
        let depth = ancestors.len();

        let (title, when) = parse_title(rest, line, &mut parsed.warnings);
        ancestors.push((indent, parsed.items.len()));
        parsed.items.push(Item {
            kind,
            done,
            title,
            date: when.date,
            end: when.end,
            time: when.time,
            group: group.clone(),
            section,
            depth,
            parent,
            line,
            line_text: raw.to_string(),
        });
    }
    parsed
}

/// `# Name` to `###### Name` (`#` at the line start, followed by a space); `None` if it is not a heading
fn parse_heading(line: &str) -> Option<Heading> {
    let level = line.chars().take_while(|&c| c == '#').count();
    if !(1..=6).contains(&level) {
        return None;
    }
    // Each `#` is 1 byte, so level can be used directly to slice the string
    let text = line[level..].strip_prefix(' ')?;
    // `type:holiday` is not part of the name; after removing it, join the remaining words with one space
    let words: Vec<&str> = text.split_whitespace().collect();
    let holiday = words.contains(&HOLIDAY);
    let name = words
        .into_iter()
        .filter(|&word| word != HOLIDAY)
        .collect::<Vec<_>>()
        .join(" ");
    Some(Heading {
        level,
        name,
        holiday,
    })
}

/// The word in a heading that marks a holiday calendar (the spec's `key:value` form)
const HOLIDAY: &str = "type:holiday";

/// Whether this line is an item: returns (task or event, done or not, the remaining text);
/// `None` if it does not start with `- `
///
/// E.g. `- [x] Pack` → (task, true, `Pack`); `- [ ] Pack` → (task, false, ...);
/// `- Dentist` → (event, false, ...)
fn parse_item_start(trimmed: &str) -> Option<(Kind, bool, &str)> {
    // Must be "- " with the space; `-Pack` does not count
    let rest = trimmed.strip_prefix("- ")?;
    match checkbox(rest) {
        Some((done, after)) => Some((Kind::Task, done, after.trim_start())),
        // No box: an event
        None => Some((Kind::Event, false, rest)),
    }
}

/// The checkbox after `- `: returns (done or not, the text after the checkbox with its spaces kept);
/// `None` if there is no checkbox
///
/// `[ ]`, `[x]`, `[X]` must be followed by a space or the end. E.g. `[x] Pack` → (true, ` Pack`);
/// `[ ]Pack` is not a checkbox
pub(crate) fn checkbox(rest: &str) -> Option<(bool, &str)> {
    let marks = [("[ ]", false), ("[x]", true), ("[X]", true)];
    for (mark, done) in marks {
        if let Some(after) = rest.strip_prefix(mark)
            && (after.is_empty() || after.starts_with(' '))
        {
            return Some((done, after));
        }
    }
    None
}

/// Indent width: a space counts 1, a tab 4 (so mixed indents can still be compared for depth)
pub(crate) fn indent_width(line: &str) -> usize {
    line.chars()
        .take_while(|c| c.is_whitespace())
        .map(|c| if c == '\t' { 4 } else { 1 })
        .sum()
}

/// Take the first `@date` or `@date..date` (and the time right after it) out of an item's text;
/// what is left is the title
///
/// E.g. `Implement the refund API @2026-10-14` → (`Implement the refund API`, 10/14, no time);
/// `Business trip to NYC @2026-10-20..2026-10-25 09:00` → (`Business trip to NYC`, 10/20 to 10/25,
/// 9 o'clock)
fn parse_title(text: &str, line: usize, warnings: &mut Vec<Warning>) -> (String, When) {
    let mut words = Vec::new();
    let mut when = When::default();
    let mut tokens = text.split_whitespace().peekable();

    while let Some(token) = tokens.next() {
        // Already has a date (only the first counts), or not shaped like `@date` / `@date..date` → plain text
        let candidate = token
            .strip_prefix('@')
            .filter(|rest| looks_like_date(rest) || looks_like_range(rest));
        let Some(candidate) = candidate.filter(|_| when.date.is_none()) else {
            words.push(token);
            continue;
        };
        // A single day is treated as a range with the same first and last day
        let (first, last) = candidate.split_once("..").unwrap_or((candidate, candidate));
        let (Ok(first), Ok(last)) = (first.parse::<Date>(), last.parse::<Date>()) else {
            warnings.push(Warning {
                line,
                kind: WarningKind::NoSuchDate(token.to_string()),
            });
            words.push(token);
            continue;
        };
        if last < first {
            warnings.push(Warning {
                line,
                kind: WarningKind::EndBeforeStart(token.to_string()),
            });
            words.push(token);
            continue;
        }
        when.date = Some(first);
        // Same first and last day (`@2026-10-20..2026-10-20`) means a one-day item
        when.end = (last > first).then_some(last);
        // `HH:MM` right after the date is the time
        if let Some(&next) = tokens.peek()
            && looks_like_time(next)
        {
            tokens.next();
            match next.parse::<Time>() {
                Ok(parsed_time) => when.time = Some(parsed_time),
                Err(_) => {
                    warnings.push(Warning {
                        line,
                        kind: WarningKind::NoSuchTime(next.to_string()),
                    });
                    words.push(next);
                }
            }
        }
    }
    // An Obsidian block id at the end of the line (`^csfds-01`, used to link to this line) stays in the file
    // but is not part of the title
    if words.last().is_some_and(|word| is_block_id(word)) {
        words.pop();
    }
    (words.join(" "), when)
}

/// Whether it is an Obsidian block id: `^` followed by ASCII letters, digits, `-`, e.g. `^csfds-01`
pub(crate) fn is_block_id(word: &str) -> bool {
    word.strip_prefix('^').is_some_and(|id| {
        !id.is_empty() && id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
    })
}

/// Whether it looks like `YYYY-MM-DD` (checks the format only, not whether the date exists)
fn looks_like_date(text: &str) -> bool {
    let bytes = text.as_bytes();
    bytes.len() == 10
        && bytes.iter().enumerate().all(|(i, &b)| match i {
            4 | 7 => b == b'-',
            _ => b.is_ascii_digit(),
        })
}

/// Whether it looks like `YYYY-MM-DD..YYYY-MM-DD` (multi-day; format only, not whether the dates exist)
fn looks_like_range(text: &str) -> bool {
    text.split_once("..")
        .is_some_and(|(first, last)| looks_like_date(first) && looks_like_date(last))
}

/// Whether it looks like `HH:MM` (checks the format only, not whether the time exists)
fn looks_like_time(text: &str) -> bool {
    let bytes = text.as_bytes();
    bytes.len() == 5
        && bytes.iter().enumerate().all(|(i, &b)| match i {
            2 => b == b':',
            _ => b.is_ascii_digit(),
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use jiff::civil::{date, time};

    /// Parse a single line and return its item (for tests)
    fn one(line: &str) -> Item {
        let parsed = parse(line);
        assert_eq!(parsed.items.len(), 1, "{line}");
        parsed.items[0].clone()
    }

    #[test]
    fn front_matter_at_the_top_is_skipped() {
        // Obsidian properties: `- work` under tags is not an item; reading goes on as usual after the closing
        // `---`, and line numbers follow the file
        let text = "---\ntags:\n  - work\n---\n- [ ] Send the weekly update @2026-10-16\n";
        let items = parse(text).items;
        assert_eq!(items.len(), 1);
        assert_eq!(
            (items[0].title.as_str(), items[0].line),
            ("Send the weekly update", 5)
        );
        // `---` not on line 1 (a Markdown divider), or no closing `---`: not front matter, read as usual
        assert_eq!(parse("- a\n---\n- b\n---\n").items.len(), 2);
        assert_eq!(parse("---\n- a\n- b\n").items.len(), 2);
    }

    #[test]
    fn rule1_tasks_open_and_done() {
        assert!(!one("- [ ] Pack").done);
        assert!(one("- [x] Pack").done);
        assert!(one("- [X] Pack").done);
        // Nothing after the checkbox still counts (an empty task)
        assert_eq!(one("- [ ]").title, "");
    }

    #[test]
    fn rule2_item_without_checkbox_is_an_event() {
        let item = one("- Dentist @2026-10-14");
        assert_eq!((item.kind, item.done), (Kind::Event, false));
        assert_eq!(item.title, "Dentist");
        assert_eq!(one("- [ ] Pack").kind, Kind::Task);
        assert_eq!(one("- [x] Pack").kind, Kind::Task);
        // No space after `[ ]` means no checkbox: the whole thing is text, and it is an event
        let item = one("- [ ]Pack");
        assert_eq!(item.kind, Kind::Event);
        assert_eq!(item.title, "[ ]Pack");
    }

    #[test]
    fn open_means_an_unfinished_task_or_an_event_not_yet_past() {
        let today = date(2026, 10, 17);
        // Tasks: only unfinished ones count (even past their date; that is overdue)
        assert!(one("- [ ] Send the weekly update @2026-10-16").is_open(today));
        assert!(!one("- [x] Send the weekly update @2026-10-16").is_open(today));
        assert!(one("- [ ] Organize the bookshelf").is_open(today));
        // Events: only today's and later ones count; once past, they are over
        assert!(one("- Dinner with friends @2026-10-17").is_open(today));
        assert!(one("- Go hiking @2026-10-18").is_open(today));
        assert!(!one("- Farmers market @2026-10-10").is_open(today));
        // An event with no date: there is no day to compare, so it does not count
        assert!(!one("- Restaurants to try").is_open(today));
    }

    #[test]
    fn rule3_date_and_time_are_removed_from_title() {
        let item = one("- [ ] Code review @2026-10-12 14:00");
        assert_eq!(item.title, "Code review");
        assert_eq!(item.date, Some(date(2026, 10, 12)));
        assert_eq!(item.time, Some(time(14, 0, 0, 0)));
        // The date can also be in the middle
        let item = one("- Send @2026-10-16 the weekly update");
        assert_eq!(
            (item.title.as_str(), item.date),
            ("Send the weekly update", Some(date(2026, 10, 16)))
        );
        // No date = not scheduled yet
        assert_eq!(one("- [ ] Organize the bookshelf").date, None);
    }

    #[test]
    fn rule3_only_the_first_date_counts() {
        let item = one("- [ ] Prepare the release checklist @2026-10-22 @2026-10-23");
        assert_eq!(item.date, Some(date(2026, 10, 22)));
        assert_eq!(item.title, "Prepare the release checklist @2026-10-23");
    }

    #[test]
    fn a_block_id_at_the_end_is_not_part_of_the_title() {
        // An Obsidian `^id` at the end of the line: not part of the title (it stays in the file);
        // a date before it is still read
        let item = one("- [ ] Confirm the LDAP account with IT @2026-10-20 ^csfds-01");
        assert_eq!(item.title, "Confirm the LDAP account with IT");
        assert_eq!(item.date, Some(date(2026, 10, 20)));
        assert_eq!(
            item.line_text,
            "- [ ] Confirm the LDAP account with IT @2026-10-20 ^csfds-01"
        );
        // Not at the end of the line, nothing after `^`, other symbols: all plain text
        assert_eq!(one("- Read the ^abc part").title, "Read the ^abc part");
        assert_eq!(one("- Exponent 2^").title, "Exponent 2^");
        assert_eq!(one("- Power ^2.5").title, "Power ^2.5");
    }

    #[test]
    fn rule3_at_mentions_and_other_text_stay_text() {
        let item = one("- [ ] Sync with @Kevin on frontend error codes");
        assert_eq!(
            (item.title.as_str(), item.date),
            ("Sync with @Kevin on frontend error codes", None)
        );
        // A URL, key:value, #number are all just text
        let item = one("- [ ] See https://example.com due:2026-10-15 PR #482");
        assert_eq!(item.title, "See https://example.com due:2026-10-15 PR #482");
        // The time counts only right after the date
        assert_eq!(one("- Meeting 14:00 @2026-10-12").time, None);
    }

    #[test]
    fn rule3_invalid_dates_stay_text_with_warning() {
        let parsed = parse("- [ ] Pay property tax @2026-13-01\n- [ ] Pay rent @2026-02-30");
        assert_eq!(parsed.items[0].title, "Pay property tax @2026-13-01");
        assert_eq!(parsed.items[0].date, None);
        let expected = vec![
            Warning {
                line: 1,
                kind: WarningKind::NoSuchDate("@2026-13-01".into()),
            },
            Warning {
                line: 2,
                kind: WarningKind::NoSuchDate("@2026-02-30".into()),
            },
        ];
        assert_eq!(parsed.warnings, expected);
        // A time that does not exist: the date still counts, the time is text
        let parsed = parse("- Meeting @2026-10-12 25:00");
        assert_eq!(parsed.items[0].date, Some(date(2026, 10, 12)));
        assert_eq!(
            (parsed.items[0].time, parsed.items[0].title.as_str()),
            (None, "Meeting 25:00")
        );
        assert_eq!(
            parsed.warnings[0].kind,
            WarningKind::NoSuchTime("25:00".into())
        );
    }

    #[test]
    fn rule3_multi_day_items_from_the_first_day_to_the_last() {
        let item = one("- Business trip to NYC @2026-10-20..2026-10-25");
        assert_eq!(item.title, "Business trip to NYC");
        // The first, the last and every day between count; the day before and the day after do not
        assert!(item.covers(date(2026, 10, 20)) && item.covers(date(2026, 10, 25)));
        assert!(item.covers(date(2026, 10, 22)));
        assert!(!item.covers(date(2026, 10, 19)) && !item.covers(date(2026, 10, 26)));
        assert_eq!(
            (item.date, item.end, item.last_day()),
            (
                Some(date(2026, 10, 20)),
                Some(date(2026, 10, 25)),
                Some(date(2026, 10, 25))
            )
        );
        // A time after it = the time on the first day; ranges may cross months and years
        let item = one("- [ ] Training @2026-12-30..2027-01-02 09:00");
        assert_eq!(item.time, Some(time(9, 0, 0, 0)));
        assert_eq!(item.end, Some(date(2027, 1, 2)));
        // Same first and last day = a one-day item
        let item = one("- Meeting @2026-10-20..2026-10-20");
        assert_eq!((item.date, item.end), (Some(date(2026, 10, 20)), None));
        // A one-day item: its last day is that day
        assert_eq!(
            one("- Team sync @2026-10-12").last_day(),
            Some(date(2026, 10, 12))
        );
    }

    #[test]
    fn rule3_bad_ranges_stay_text_with_warning() {
        // Ends before it starts, or one end does not exist: the whole token is text, with one warning
        let parsed = parse(
            "- Trip @2026-10-25..2026-10-20
- Trip @2026-10-20..2026-13-01",
        );
        assert_eq!(parsed.items[0].title, "Trip @2026-10-25..2026-10-20");
        assert_eq!((parsed.items[0].date, parsed.items[0].end), (None, None));
        assert_eq!(
            parsed.warnings,
            [
                Warning {
                    line: 1,
                    kind: WarningKind::EndBeforeStart("@2026-10-25..2026-10-20".into()),
                },
                Warning {
                    line: 2,
                    kind: WarningKind::NoSuchDate("@2026-10-20..2026-13-01".into()),
                },
            ]
        );
        // Not shaped like a date range (one end missing, only the day written): plain text, no warning
        let parsed = parse(
            "- Trip @2026-10-20..
- Trip @2026-10-20..25",
        );
        assert_eq!(parsed.items[0].date, None);
        assert_eq!(parsed.items[1].title, "Trip @2026-10-20..25");
        assert!(parsed.warnings.is_empty());
    }

    #[test]
    fn multi_day_events_stay_open_until_their_last_day() {
        let today = date(2026, 10, 17);
        assert!(one("- Trip @2026-10-15..2026-10-17").is_open(today));
        assert!(!one("- Trip @2026-10-14..2026-10-16").is_open(today));
    }

    #[test]
    fn every_heading_is_listed_with_its_line_and_text() {
        // Also a heading with nothing under it; not one inside a code block
        let parsed = parse(
            "- a\n# Work\n\n## Empty\n```\n# Not a heading\n```\n# Days off type:holiday\n- b\n",
        );
        let headings: Vec<(usize, &str, &str)> = parsed
            .headings
            .iter()
            .map(|line| (line.line, line.text.as_str(), line.heading.name.as_str()))
            .collect();
        assert_eq!(
            headings,
            [
                (2, "# Work", "Work"),
                (4, "## Empty", "Empty"),
                (8, "# Days off type:holiday", "Days off"),
            ]
        );
    }

    #[test]
    fn rule4_headings_set_the_group_and_keep_their_level() {
        let parsed =
            parse("- No group\n# Personal\n- [ ] Pay the credit card bill\n## Moving\n- [ ] Pack");
        let groups: Vec<_> = parsed.items.iter().map(|item| item.group.clone()).collect();
        let heading = |level, name: &str| {
            Some(Heading {
                level,
                name: name.into(),
                holiday: false,
            })
        };
        assert_eq!(
            groups,
            vec![None, heading(1, "Personal"), heading(2, "Moving")]
        );
        // `#tag` (no space) and 7 #s are not headings
        assert_eq!(parse("#tag\n- a").items[0].group, None);
        assert_eq!(parse("####### x\n- a").items[0].group, None);
    }

    #[test]
    fn rule4b_type_holiday_makes_a_holiday_calendar() {
        // `type:holiday` can go anywhere and is not part of the name;
        // any other `key:value` is still part of the name
        let group = |text: &str| parse(text).items[0].group.clone().expect("has a heading");
        let heading = group("# US holidays type:holiday\n- Long weekend @2026-10-10");
        assert_eq!(
            (heading.name.as_str(), heading.holiday),
            ("US holidays", true)
        );
        let heading = group("## type:holiday 2026 long weekends\n- a");
        assert_eq!(
            (heading.name.as_str(), heading.holiday),
            ("2026 long weekends", true)
        );
        let heading = group("# Work type:work\n- a");
        assert_eq!(
            (heading.name.as_str(), heading.holiday),
            ("Work type:work", false)
        );
        // Upper case or a missing colon does not count
        assert!(!group("# Days off Type:Holiday\n- a").holiday);
        assert!(!group("# Days off holiday\n- a").holiday);
    }

    #[test]
    fn section_counts_headings_so_same_named_ones_differ() {
        // 0 before any heading; the two "Work" sections have the same group but a different section
        let items = parse("- a\n# Work\n- b\n# Personal\n# Work\n- c\n").items;
        let sections: Vec<_> = items.iter().map(|item| item.section).collect();
        assert_eq!(sections, [0, 1, 3]);
        assert_eq!(items[1].group, items[2].group);
    }

    #[test]
    fn rule5_indentation_makes_children() {
        let text = "- Tokyo, 5 days\n  - [ ] Book flights\n    - [ ] Compare prices\n  - [ ] Book the hotel\n- Fly out";
        let items = parse(text).items;
        let shape: Vec<_> = items.iter().map(|item| (item.depth, item.parent)).collect();
        assert_eq!(
            shape,
            vec![
                (0, None),
                (1, Some(0)),
                (2, Some(1)),
                (1, Some(0)),
                (0, None)
            ]
        );
        // The original line is kept with its indent
        assert_eq!(items[2].line_text, "    - [ ] Compare prices");
        // A tab counts as indent too
        let items = parse("- [ ] Pack\n\t- [ ] Office").items;
        assert_eq!(items[1].parent, Some(0));
        // Under a new heading, counting starts again from the outermost level
        let items = parse("- a\n# g\n  - b").items;
        assert_eq!(items[1].parent, None);
    }

    #[test]
    fn other_lines_are_ignored() {
        let text = "Some notes\n\n-No space after the dash\n```\n- [ ] Inside a code block\n```\n- [ ] A real task";
        let parsed = parse(text);
        assert_eq!(parsed.items.len(), 1);
        assert_eq!(
            (parsed.items[0].title.as_str(), parsed.items[0].line),
            ("A real task", 7)
        );
    }

    #[test]
    fn crlf_line_endings_work() {
        let items = parse("- [ ] a @2026-10-12\r\n- [x] b\r\n").items;
        assert_eq!(items.len(), 2);
        assert_eq!(
            (items[0].title.as_str(), items[1].title.as_str()),
            ("a", "b")
        );
        // The original line does not include \r
        assert_eq!(items[1].line_text, "- [x] b");
    }

    #[test]
    fn sample_file_parses_like_a_real_one() {
        let parsed = parse(include_str!("../testdata/tasks.md"));
        let count = |f: fn(&Item) -> bool| parsed.items.iter().filter(|item| f(item)).count();
        assert_eq!(parsed.items.len(), 57);
        // Tasks: 6 `[x]` / `[X]`, 32 `[ ]`;
        // 19 events without a box (meetings, holidays, birthdays, milestones, trips)
        assert_eq!(count(|item| item.kind == Kind::Task && item.done), 6);
        assert_eq!(count(|item| item.kind == Kind::Task && !item.done), 32);
        assert_eq!(count(|item| item.kind == Kind::Event), 19);
        assert_eq!(count(|item| item.date.is_none()), 11);
        // 3 multi-day items (Manager OOO, Tokyo, the long weekend)
        assert_eq!(count(|item| item.end.is_some()), 3);
        // 1 holiday calendar with 2 items
        assert_eq!(
            count(|item| item.group.as_ref().is_some_and(|group| group.holiday)),
            2
        );
        // These numbers were counted separately by another small program, not copied from the parser's output
        // The only wrong date
        assert_eq!(parsed.warnings.len(), 1);
        assert_eq!(
            parsed.warnings[0].kind,
            WarningKind::NoSuchDate("@2026-13-01".into())
        );
    }
}
