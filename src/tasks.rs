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
    /// `- [ ] text` / `- [/] text` / `- [x] text`: a task, which you check off yourself
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
    /// Whether the task is done: `[x]` / `[X]` is done, `[ ]` and `[/]` are not yet; always `false` for an
    /// event (events cannot be checked off)
    pub done: bool,
    /// Whether the task is being worked on: `[/]` (never done at the same time); always `false` for an
    /// event. Its title is drawn in bold
    pub in_progress: bool,
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
    /// How urgent it is (`!` / `!!` / `!!!` in the text); `None` without one
    pub priority: Option<Priority>,
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
    /// its last day). It is drawn in the normal text color (a holiday in the day-off blue); finished ones
    /// (done tasks, past events) turn gray
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

/// How urgent an item is (rule 6 of the spec): more `!` = more urgent; the screen colors the title by it
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Priority {
    /// `!`
    Low,
    /// `!!`
    Medium,
    /// `!!!`
    High,
}

impl Priority {
    /// The priority a word stands for: exactly `!`, `!!` or `!!!`; anything else (`!!!!`, `wow!`) is `None`
    pub(crate) fn from_word(word: &str) -> Option<Self> {
        match word {
            "!" => Some(Self::Low),
            "!!" => Some(Self::Medium),
            "!!!" => Some(Self::High),
            _ => None,
        }
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
    /// How many `#`: fewer `#` = a level up (`#` Work → `##` SFAP is a calendar inside Work)
    pub level: usize,
    /// The heading text (without `type:holiday`)
    pub name: String,
    /// Holiday calendar: the heading has `type:holiday`, so the days of the items under it are holidays
    pub holiday: bool,
    /// The names of the calendars it is inside, outermost first: the nearest heading above with fewer `#`,
    /// then that one's, and so on (empty for a top-level calendar). A calendar is its whole path
    /// (`parents` + `name`): `Work > SFAP` and `Personal > SFAP` are two calendars
    pub parents: Vec<String>,
}

impl Heading {
    /// The outermost calendar it is in, or itself at the top level: `CAIS` for `CAIS > 2603_…`
    pub fn outermost(&self) -> &str {
        self.parents.first().unwrap_or(&self.name)
    }
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
    // (level, name) of the headings the current one is inside, outermost first, and the current one
    let mut outline: Vec<(usize, String)> = Vec::new();
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
        if let Some(mut heading) = parse_heading(raw) {
            // Its parents: the headings above with fewer `#`
            outline.retain(|&(level, _)| level < heading.level);
            heading.parents = outline.iter().map(|(_, name)| name.clone()).collect();
            outline.push((heading.level, heading.name.clone()));
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

        let Some((kind, mark, rest)) = parse_item_start(trimmed) else {
            continue;
        };

        // Find the parent: pop the ones not indented less than this item; the last one left is the parent
        let indent = indent_width(raw);
        while ancestors.last().is_some_and(|&(width, _)| width >= indent) {
            ancestors.pop();
        }
        let parent = ancestors.last().map(|&(_, position)| position);
        let depth = ancestors.len();

        let (title, when, priority) = parse_title(rest, line, &mut parsed.warnings);
        ancestors.push((indent, parsed.items.len()));
        parsed.items.push(Item {
            kind,
            done: mark == Mark::Done,
            in_progress: mark == Mark::InProgress,
            title,
            date: when.date,
            end: when.end,
            time: when.time,
            priority,
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
        parents: Vec::new(),
    })
}

/// The word in a heading that marks a holiday calendar (the spec's `key:value` form)
const HOLIDAY: &str = "type:holiday";

/// What a task's box says (rule 1 of the spec)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Mark {
    /// `[ ]`: not started
    Open,
    /// `[/]`: being worked on (the "in progress" of Obsidian's Tasks plugin and many themes)
    InProgress,
    /// `[x]` / `[X]`
    Done,
}

/// Whether this line is an item: returns (task or event, what its box says, the remaining text);
/// `None` if it does not start with `- `
///
/// E.g. `- [x] Pack` → (task, done, `Pack`); `- [/] Pack` → (task, in progress, ...);
/// `- Dentist` → (event, open, ...)
fn parse_item_start(trimmed: &str) -> Option<(Kind, Mark, &str)> {
    // Must be "- " with the space; `-Pack` does not count
    let rest = trimmed.strip_prefix("- ")?;
    match checkbox(rest) {
        Some((mark, after)) => Some((Kind::Task, mark, after.trim_start())),
        // No box: an event (never done or in progress)
        None => Some((Kind::Event, Mark::Open, rest)),
    }
}

/// The checkbox after `- `: returns (what it says, the text after the checkbox with its spaces kept);
/// `None` if there is no checkbox
///
/// `[ ]`, `[/]`, `[x]`, `[X]` must be followed by a space or the end. E.g. `[x] Pack` → (done, ` Pack`);
/// `[ ]Pack` is not a checkbox
pub(crate) fn checkbox(rest: &str) -> Option<(Mark, &str)> {
    let marks = [
        ("[ ]", Mark::Open),
        ("[/]", Mark::InProgress),
        ("[x]", Mark::Done),
        ("[X]", Mark::Done),
    ];
    for (text, mark) in marks {
        if let Some(after) = rest.strip_prefix(text)
            && (after.is_empty() || after.starts_with(' '))
        {
            return Some((mark, after));
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

/// Take the first `@date` or `@date..date` (and the time right after it) and the first `!` / `!!` / `!!!`
/// out of an item's text; what is left is the title
///
/// E.g. `Implement the refund API @2026-10-14` → (`Implement the refund API`, 10/14, no time, no priority);
/// `Business trip to NYC @2026-10-20..2026-10-25 09:00` → (`Business trip to NYC`, 10/20 to 10/25,
/// 9 o'clock); `Fix the audit log !!! @2026-10-21` → (`Fix the audit log`, 10/21, high)
fn parse_title(
    text: &str,
    line: usize,
    warnings: &mut Vec<Warning>,
) -> (String, When, Option<Priority>) {
    let mut words = Vec::new();
    let mut when = When::default();
    let mut priority = None;
    let mut tokens = text.split_whitespace().peekable();

    while let Some(token) = tokens.next() {
        // `!` / `!!` / `!!!`: only the first counts, a later one stays text
        if priority.is_none()
            && let Some(found) = Priority::from_word(token)
        {
            priority = Some(found);
            continue;
        }
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
    (words.join(" "), when, priority)
}

/// Whether it is an Obsidian block id: `^` followed by ASCII letters, digits, `-`, e.g. `^csfds-01`
pub(crate) fn is_block_id(word: &str) -> bool {
    word.strip_prefix('^').is_some_and(|id| {
        !id.is_empty() && id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
    })
}

/// Whether it looks like `YYYY-MM-DD` (checks the format only, not whether the date exists)
pub(crate) fn looks_like_date(text: &str) -> bool {
    let bytes = text.as_bytes();
    bytes.len() == 10
        && bytes.iter().enumerate().all(|(i, &b)| match i {
            4 | 7 => b == b'-',
            _ => b.is_ascii_digit(),
        })
}

/// Whether it looks like `YYYY-MM-DD..YYYY-MM-DD` (multi-day; format only, not whether the dates exist)
pub(crate) fn looks_like_range(text: &str) -> bool {
    text.split_once("..")
        .is_some_and(|(first, last)| looks_like_date(first) && looks_like_date(last))
}

/// Whether it looks like `HH:MM` (checks the format only, not whether the time exists)
pub(crate) fn looks_like_time(text: &str) -> bool {
    let bytes = text.as_bytes();
    bytes.len() == 5
        && bytes.iter().enumerate().all(|(i, &b)| match i {
            2 => b == b':',
            _ => b.is_ascii_digit(),
        })
}

#[cfg(test)]
mod tests;
