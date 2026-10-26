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
fn rule1_in_progress_is_a_task_not_done_yet() {
    let item = one("- [/] Draft the release notes @2026-10-16");
    assert_eq!(
        (item.kind, item.done, item.in_progress),
        (Kind::Task, false, true)
    );
    assert_eq!(item.title, "Draft the release notes");
    // The other boxes and events are not in progress
    assert!(!one("- [ ] Pack").in_progress);
    assert!(!one("- [x] Pack").in_progress);
    assert!(!one("- Dentist").in_progress);
    // No space after `[/]` means no checkbox: the whole thing is an event's text
    let item = one("- [/]Pack");
    assert_eq!(
        (item.kind, item.in_progress, item.title.as_str()),
        (Kind::Event, false, "[/]Pack")
    );
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
fn rule6_exclamation_marks_set_the_priority() {
    // `!` / `!!` / `!!!` anywhere in the item, and they are not part of the title; no mark = no priority
    let item = one("- [ ] Fix the audit log !!! @2026-10-21 14:00 ^cais-09");
    assert_eq!(item.priority, Some(Priority::High));
    assert_eq!(item.title, "Fix the audit log");
    assert_eq!(
        (item.date, item.time),
        (Some(date(2026, 10, 21)), Some(time(14, 0, 0, 0)))
    );
    assert_eq!(
        one("- [ ] !! Call the bank").priority,
        Some(Priority::Medium)
    );
    assert_eq!(
        one("- Code freeze ! @2026-10-19").priority,
        Some(Priority::Low)
    );
    assert_eq!(one("- [ ] Pack").priority, None);
    // Only the first counts, a later one stays text
    let item = one("- [ ] Pay the fine !! !!!");
    assert_eq!(
        (item.priority, item.title.as_str()),
        (Some(Priority::Medium), "Pay the fine !!!")
    );
    // Only a word made of 1 to 3 `!`: four of them, or `!` stuck to a word, stay text
    let item = one("- [ ] Ship it!!! !!!!");
    assert_eq!(
        (item.priority, item.title.as_str()),
        (None, "Ship it!!! !!!!")
    );
    // A `!` before a block id: the id is still taken off the end
    let item = one("- [ ] Renew the passport ! ^life-05");
    assert_eq!(
        (item.priority, item.title.as_str()),
        (Some(Priority::Low), "Renew the passport")
    );
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
    let parsed =
        parse("- a\n# Work\n\n## Empty\n```\n# Not a heading\n```\n# Days off type:holiday\n- b\n");
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
fn rule4_fewer_hashes_is_a_level_up() {
    // A heading is inside the nearest heading above with fewer `#` (a skipped level counts the same);
    // SFAP under Work and SFAP under Personal are two calendars
    let parsed =
        parse("# Work\n## SFAP\n### Bugs\n## 11RC\n# Personal\n## SFAP\n#### Deep\n# Later\n");
    let paths: Vec<String> = parsed
        .headings
        .iter()
        .map(|line| {
            let heading = &line.heading;
            heading
                .parents
                .iter()
                .chain([&heading.name])
                .cloned()
                .collect::<Vec<_>>()
                .join(" > ")
        })
        .collect();
    assert_eq!(
        paths,
        [
            "Work",
            "Work > SFAP",
            "Work > SFAP > Bugs",
            "Work > 11RC",
            "Personal",
            "Personal > SFAP",
            "Personal > SFAP > Deep",
            "Later"
        ]
    );
}

#[test]
fn rule4_headings_set_the_group_and_keep_their_level() {
    let parsed =
        parse("- No group\n# Personal\n- [ ] Pay the credit card bill\n## Moving\n- [ ] Pack");
    let groups: Vec<_> = parsed.items.iter().map(|item| item.group.clone()).collect();
    let heading = |level, name: &str, parents: &[&str]| {
        Some(Heading {
            level,
            name: name.into(),
            holiday: false,
            archived: None,
            parents: parents.iter().map(|&parent| parent.into()).collect(),
        })
    };
    assert_eq!(
        groups,
        vec![
            None,
            heading(1, "Personal", &[]),
            heading(2, "Moving", &["Personal"])
        ]
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
fn rule4c_archived_and_a_date_archives_a_calendar() {
    // `archived:<date>` can go anywhere, also next to `type:holiday`, and is not part of the name
    let group = |text: &str| parse(text).items[0].group.clone().expect("has a heading");
    let heading = group("## Q3 report archived:2026-10-26\n- a");
    assert_eq!(
        (heading.name.as_str(), heading.archived),
        ("Q3 report", Some(date(2026, 10, 26)))
    );
    let heading = group("# archived:2025-01-05 2024 holidays type:holiday\n- a");
    assert_eq!(
        (heading.name.as_str(), heading.holiday, heading.archived),
        ("2024 holidays", true, Some(date(2025, 1, 5)))
    );
    // No date, a date that does not exist, upper case: part of the name, not archived
    for text in [
        "# Q3 archived:yes",
        "# Q3 archived:",
        "# Q3 archived:2026-13-01",
        "# Q3 Archived:2026-10-26",
    ] {
        let heading = group(&format!("{text}\n- a"));
        assert_eq!(heading.archived, None, "{text}");
        assert_eq!(heading.name, text["# ".len()..], "{text}");
    }
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
    let parsed = parse(include_str!("../../testdata/tasks.md"));
    let count = |f: fn(&Item) -> bool| parsed.items.iter().filter(|item| f(item)).count();
    assert_eq!(parsed.items.len(), 57);
    // Tasks: 6 `[x]` / `[X]`, 32 not done (31 `[ ]`, 1 `[/]`: Draft the release notes);
    // 19 events without a box (meetings, holidays, birthdays, milestones, trips)
    assert_eq!(count(|item| item.kind == Kind::Task && item.done), 6);
    assert_eq!(count(|item| item.kind == Kind::Task && !item.done), 32);
    assert_eq!(count(|item| item.in_progress), 1);
    assert_eq!(count(|item| item.kind == Kind::Event), 19);
    assert_eq!(count(|item| item.date.is_none()), 11);
    // 3 multi-day items (Manager OOO, Tokyo, the long weekend)
    assert_eq!(count(|item| item.end.is_some()), 3);
    // Priorities: 2 `!!!` (Release v2.3, the credit card bill), 1 `!!`, 1 `!`
    assert_eq!(count(|item| item.priority == Some(Priority::High)), 2);
    assert_eq!(count(|item| item.priority == Some(Priority::Medium)), 1);
    assert_eq!(count(|item| item.priority == Some(Priority::Low)), 1);
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
