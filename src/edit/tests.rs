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
        check_calendar(text, &taken).map(|heading| (heading.name, heading.level, heading.holiday))
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
    let path = |names: &[&str]| -> Vec<String> { names.iter().map(|&name| name.into()).collect() };
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
    let (after, _) = move_to_calendar(text, &item(text, 0), "- x", &path(&["B"])).expect("move");
    assert_eq!(after, "# A\r\n# B\r\n- y\r\n- x\r\n");
    let text = "# A\n- x\n# B\n- y";
    let (after, _) = move_to_calendar(text, &item(text, 0), "- x", &path(&["B"])).expect("move");
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
    let path = |names: &[&str]| -> Vec<String> { names.iter().map(|&name| name.into()).collect() };
    // Work's last item is the design doc (with its subtasks and the blank line between them)
    let (after, line) = add(GROUPS, "- [ ] New", &Place::InCalendar(path(&["Work"]))).expect("add");
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
    let (after, line) = add(text, "- new", &Place::InCalendar(path(&["Personal"]))).expect("add");
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
    let (after, line) = add(text, "- [ ] New task", &Place::InGroupOf(item(text, 0))).expect("add");
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
        swap("- a\r\n- b", &item("- a\r\n- b", 0), &item("- a\r\n- b", 1)).map(|(text, _)| text),
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

#[test]
fn add_calendar_in_goes_at_the_end_of_the_parent_with_one_more_hash() {
    // Inside Work > SFAP: after SFAP's last item, before the blank line in front of `# Personal`
    assert_eq!(
        add_calendar_in(
            "# Work\n- a\n## SFAP\n- b\n\n# Personal\n- c\n",
            &["Work".to_string(), "SFAP".to_string()],
            "Bugs"
        ),
        Ok((
            "# Work\n- a\n## SFAP\n- b\n\n### Bugs\n\n# Personal\n- c\n".to_string(),
            6
        ))
    );
    // Inside Work: after the calendars inside it too
    assert_eq!(
        add_calendar_in(
            "# Work\n## SFAP\n- b\n# Personal\n",
            &["Work".to_string()],
            "Bugs"
        ),
        Ok((
            "# Work\n## SFAP\n- b\n\n## Bugs\n# Personal\n".to_string(),
            5
        ))
    );
    // Work twice in the file: its last section; the last line without a newline gets one
    assert_eq!(
        add_calendar_in(
            "# Work\n- a\n# Home\n# Work\n- z",
            &["Work".to_string()],
            "Bugs"
        ),
        Ok((
            "# Work\n- a\n# Home\n# Work\n- z\n\n## Bugs\n".to_string(),
            7
        ))
    );
    // \r\n stays \r\n; no parent = the top level (add_calendar)
    assert_eq!(
        add_calendar_in("# Work\r\n- a\r\n", &["Work".to_string()], "Bugs"),
        Ok(("# Work\r\n- a\r\n\r\n## Bugs\r\n".to_string(), 4))
    );
    assert_eq!(
        add_calendar_in("# Work\n", &[], "Home"),
        Ok(add_calendar("# Work\n", "Home"))
    );
    // No such calendar (e.g. deleted elsewhere), or already 6 `#`: Changed
    assert_eq!(
        add_calendar_in("# Work\n", &["Home".to_string()], "Bugs"),
        Err(Changed)
    );
    assert_eq!(
        add_calendar_in("###### Deep\n", &["Deep".to_string()], "Deeper"),
        Err(Changed)
    );
}
