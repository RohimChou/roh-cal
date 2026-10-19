//! The sample task file built into the program, for `--sample`

use crate::tasks;
use jiff::ToSpan;
use jiff::civil::{Date, date};

/// The sample task file `testdata/tasks.md`, copied into the program when it is compiled, so `--sample` also
/// works without the repo (e.g. after `cargo install`). The tests read the same file, so changing it changes
/// both
const TEXT: &str = include_str!("../testdata/tasks.md");

/// The sample is written as if today were in the week starting Monday 2026-10-12 (the tests use days of
/// that week as today)
const WEEK: Date = date(2026, 10, 12);

/// The sample with every date moved by whole weeks, so that its week (`WEEK`) becomes the week of `today`:
/// whenever it is opened, it has things overdue, today and coming up, and each item keeps its weekday
/// (the Monday team sync stays on a Monday)
pub fn text(today: Date) -> String {
    let days = WEEK.until(today).map_or(0, |span| span.get_days());
    move_dates(TEXT, days.div_euclid(7) * 7)
}

/// Moves every date written as `YYYY-MM-DD` in `text` by `days`, wherever it is (`@2026-10-12`, both ends
/// of `@2026-10-28..2026-11-03`, `due:2026-10-15`); one that does not exist, like `@2026-13-01`, stays as
/// it is, so the sample keeps its mistake on purpose
fn move_dates(text: &str, days: i32) -> String {
    let is_digit = |index: usize| text.as_bytes().get(index).is_some_and(u8::is_ascii_digit);
    let mut moved = String::with_capacity(text.len());
    // text[..copied] is in `moved` already
    let mut copied = 0;
    let mut start = 0;
    while start + 10 <= text.len() {
        let end = start + 10;
        // A whole `YYYY-MM-DD` (no digit right before or after it) that is a real date. `get` is `None` when
        // `start` falls inside a character like 🎂
        let found = text
            .get(start..end)
            .filter(|word| tasks::looks_like_date(word))
            .filter(|_| (start == 0 || !is_digit(start - 1)) && !is_digit(end))
            .and_then(|word| word.parse::<Date>().ok())
            .and_then(|date| date.checked_add(days.days()).ok());
        if let Some(date) = found {
            moved.push_str(&text[copied..start]);
            moved.push_str(&date.to_string());
            copied = end;
            start = end;
        } else {
            start += 1;
        }
    }
    moved.push_str(&text[copied..]);
    moved
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn move_dates_moves_every_real_date_and_nothing_else() {
        let cases = [
            // A day, a day with a time, both ends of a range (into the next month)
            (
                "- Team sync @2026-10-12 10:00",
                "- Team sync @2026-10-19 10:00",
            ),
            (
                "- OOO @2026-10-28..2026-11-03",
                "- OOO @2026-11-04..2026-11-10",
            ),
            // Into the next year; written after other words
            ("- [ ] Fly home @2026-12-28", "- [ ] Fly home @2027-01-04"),
            (
                "- [ ] Self-assessment due:2026-10-15",
                "- [ ] Self-assessment due:2026-10-22",
            ),
            // After a character wider than one byte
            (
                "- Mom's birthday 🎂 @2026-10-29",
                "- Mom's birthday 🎂 @2026-11-05",
            ),
            // Not a real date, or not a whole YYYY-MM-DD: left as it is
            (
                "- [ ] Pay property tax @2026-13-01",
                "- [ ] Pay property tax @2026-13-01",
            ),
            (
                "- 12026-10-12 2026-10-123 2026-1-12",
                "- 12026-10-12 2026-10-123 2026-1-12",
            ),
            (
                "- Code review: Amy's PR #482",
                "- Code review: Amy's PR #482",
            ),
        ];
        for (text, expected) in cases {
            assert_eq!(move_dates(text, 7), expected, "{text}");
        }
        // Backward; the whole text at once, line breaks kept
        assert_eq!(
            move_dates("@2026-01-02\n@2026-03-01..2026-03-02\n", -7),
            "@2025-12-26\n@2026-02-22..2026-02-23\n"
        );
    }

    #[test]
    fn the_sample_week_becomes_the_week_of_today() {
        // Today inside the sample's week (Mon 2026-10-12 to Sun 2026-10-18): the sample as it is
        for day in [12, 15, 18] {
            assert_eq!(text(date(2026, 10, day)), TEXT, "{day}");
        }
        // The first Monday team sync (2026-10-12 in the sample) lands on the Monday of today's week
        for (today, monday) in [
            (date(2026, 10, 20), "@2026-10-19 10:00"), // Tuesday, one week later
            (date(2027, 3, 3), "@2027-03-01 10:00"),   // Wednesday, 20 weeks later
            (date(2026, 1, 1), "@2025-12-29 10:00"),   // Thursday, 41 weeks earlier
        ] {
            let moved = text(today);
            let first_sync = moved
                .lines()
                .find(|line| line.starts_with("- Team sync"))
                .expect("team sync");
            assert_eq!(first_sync, format!("- Team sync {monday}"), "{today}");
            // Still the same items, and still only the one mistake (`@2026-13-01` is not moved)
            let parsed = tasks::parse(&moved);
            assert_eq!(
                (parsed.items.len(), parsed.warnings.len()),
                (57, 1),
                "{today}"
            );
        }
    }
}
