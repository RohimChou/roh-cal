use super::super::{Focus, View};
use super::*;
use crate::texts::Lang;
use jiff::civil::date;
use ratatui::crossterm::event::{KeyEvent, KeyModifiers};

/// An app on the tasks page (today 2026/10/17) whose task file is `text`
fn tasks_app(text: &str) -> App {
    let mut app = App::new(
        Lang::ZhTw,
        date(2026, 10, 17),
        crate::tasks::parse(text).items,
    );
    app.handle_key(KeyEvent::from(KeyCode::Char('3')));
    app
}

/// Same as `tasks_app`, but the heading lines are read too (as when reading the real file), so heading
/// rows can be chosen
fn tasks_app_with_headings(text: &str) -> App {
    let mut app = App::new(Lang::ZhTw, date(2026, 10, 17), Vec::new());
    app.set_items(crate::tasks::parse(text));
    app.handle_key(KeyEvent::from(KeyCode::Char('3')));
    app
}

/// Presses one key
fn press(app: &mut App, code: KeyCode) {
    app.handle_key(KeyEvent::from(code));
}

#[test]
fn brackets_jump_to_the_previous_or_next_calendar() {
    // Lines: 1 # A, 2 a1, 3 a2, 4 # B, 5 b1, 6 ## C, 7 c1, 8 # Empty
    let mut app = tasks_app_with_headings("# A\n- a1\n- a2\n# B\n- b1\n## C\n- c1\n# Empty\n");
    // From a2 (A's second item; the page opens on the first item, a1): ] = B's heading row, then C's (a
    // `##` heading is a calendar too), then Empty's (no items, still a calendar)
    press(&mut app, KeyCode::Char('j'));
    assert_eq!(app.tasks_line, Some(3));
    press(&mut app, KeyCode::Char(']'));
    assert_eq!(app.tasks_line, Some(4));
    press(&mut app, KeyCode::Char(']'));
    assert_eq!(app.tasks_line, Some(6));
    press(&mut app, KeyCode::Char(']'));
    assert_eq!(app.tasks_line, Some(8));
    // No calendar after Empty: stays
    press(&mut app, KeyCode::Char(']'));
    assert_eq!(app.tasks_line, Some(8));
    // [ goes back one calendar at a time, to its heading row; stays at the first calendar
    press(&mut app, KeyCode::Char('['));
    assert_eq!(app.tasks_line, Some(6));
    // From an item (c1), [ = the calendar before C (B), like [ on the calendar goes to the previous month
    press(&mut app, KeyCode::Char('j'));
    press(&mut app, KeyCode::Char('['));
    assert_eq!(app.tasks_line, Some(4));
    press(&mut app, KeyCode::Char('['));
    press(&mut app, KeyCode::Char('['));
    assert_eq!(app.tasks_line, Some(1));
    // The calendar's date did not move (on the tasks page [ ] are not months)
    assert_eq!(app.selected, date(2026, 10, 17));
}

#[test]
fn gg_and_capital_g_choose_the_first_and_last_item() {
    let mut app = tasks_app(TREE);
    let chosen = |app: &App| app.tasks_chosen().map(|item| item.title.clone());
    // G: the last item (headings skipped); gg: the first one
    press(&mut app, KeyCode::Char('G'));
    assert_eq!(chosen(&app).as_deref(), Some("Go hiking"));
    press(&mut app, KeyCode::Char('g'));
    assert_eq!(chosen(&app).as_deref(), Some("Go hiking"));
    press(&mut app, KeyCode::Char('g'));
    assert_eq!(chosen(&app).as_deref(), Some("Renew passport"));
    // g, another key, g: not gg (j moves as usual)
    press(&mut app, KeyCode::Char('g'));
    press(&mut app, KeyCode::Char('j'));
    press(&mut app, KeyCode::Char('g'));
    assert_eq!(chosen(&app).as_deref(), Some("Design doc"));
}

#[test]
fn a_calendars_heading_row_can_be_chosen_and_a_adds_there() {
    use super::super::write::tests::clean_up;
    let text = "# Work\n- [ ] Review PRs\n\n# Empty\n\n# Later\n- [ ] Book flights\n";
    let (mut app, path) = tasks_app_with_file("tasks-heading-add", text);
    let read = || std::fs::read_to_string(&path).expect("read");
    // The first item is chosen at start; j goes on to the Empty heading row (no item is chosen then)
    assert_eq!(
        app.tasks_chosen().map(|item| item.title.as_str()),
        Some("Review PRs")
    );
    press(&mut app, KeyCode::Char('j'));
    assert_eq!(app.tasks_chosen(), None);
    assert_eq!(app.tasks_chosen_calendar(), Some(vec!["Empty".to_string()]));
    // Keys for an item do nothing there (Space, e)
    press(&mut app, KeyCode::Char(' '));
    press(&mut app, KeyCode::Char('e'));
    assert!(app.editor().is_none());
    assert_eq!(read(), text);
    // a: adds at the end of Empty (not of the file); the new item is chosen
    press(&mut app, KeyCode::Char('a'));
    type_text(&mut app, "Plan the week");
    press(&mut app, KeyCode::Enter);
    assert_eq!(
        read(),
        "# Work\n- [ ] Review PRs\n\n# Empty\n- [ ] Plan the week\n\n# Later\n- [ ] Book flights\n"
    );
    assert_eq!(
        app.tasks_chosen().map(|item| item.title.as_str()),
        Some("Plan the week")
    );
    // k goes back up through the Empty heading row to Review PRs, then the Work heading row
    press(&mut app, KeyCode::Char('k'));
    assert_eq!(app.tasks_chosen_calendar(), Some(vec!["Empty".to_string()]));
    press(&mut app, KeyCode::Char('k'));
    press(&mut app, KeyCode::Char('k'));
    assert_eq!(app.tasks_chosen_calendar(), Some(vec!["Work".to_string()]));
    // A click on a heading row chooses it (rows: Work, Review PRs, Empty, Plan the week, Later, …)
    app.click_task(2, false, Instant::now());
    assert_eq!(app.tasks_chosen_calendar(), Some(vec!["Empty".to_string()]));
    clean_up(&path);
}

#[test]
fn every_checked_calendar_is_listed_even_with_no_items() {
    let mut app = App::new(Lang::ZhTw, date(2026, 10, 17), Vec::new());
    app.set_items(crate::tasks::parse(
        "# Work\n- [ ] a\n# Empty\n# Done\n- [x] d\n",
    ));
    press(&mut app, KeyCode::Char('3'));
    assert_eq!(shape(&app), ["#Work", "a", "#Empty", "#Done", "d"]);
    // Done tasks hidden: Done keeps its heading row
    app.settings.calendar_list.hide_done = true;
    assert_eq!(shape(&app), ["#Work", "a", "#Empty", "#Done"]);
    // Unchecked in the side menu: left out
    app.groups[1].shown = false;
    assert_eq!(shape(&app), ["#Work", "a", "#Done"]);
}

#[test]
fn calendars_inside_calendars_list_their_outer_calendars_first() {
    // P has no items of its own: it still gets a heading row, right above Q's; back in P later, P again
    let app = tasks_app("# A\n- a1\n# P\n## Q\n- q1\n# P\n- p1\n");
    assert_eq!(shape(&app), ["#A", "a1", "#P", "#Q", "q1", "#P", "p1"]);
    let depths: Vec<usize> = app
        .tasks_rows()
        .iter()
        .filter_map(|row| match row {
            TaskRow::Heading { depth, .. } => Some(*depth),
            TaskRow::Item { .. } => None,
        })
        .collect();
    assert_eq!(depths, [0, 0, 1, 0]);
}

#[test]
fn a_checked_calendar_inside_an_unchecked_one_is_listed() {
    // Work unchecked on its own, SFAP (no items) inside it checked: Work's own item w is left out, SFAP's
    // heading is listed, with a Work row above it for the tree
    let mut app = tasks_app_with_headings("# Work\n- w\n## SFAP\n# Life\n- l\n");
    app.groups[0].shown = false;
    assert_eq!(shape(&app), ["#Work", "#SFAP", "#Life", "l"]);
}

#[test]
fn brackets_skip_heading_rows_without_a_line_of_their_own() {
    // Lines: 1 a0 (「未分組」), 2 # Work, 3 ## SFAP, 4 s1, 5 # Life, 6 l1. 「未分組」 has no heading line, so
    // ] from a0 goes to the first real one (Work); Work's own heading counts even with nothing right
    // under it
    let mut app = tasks_app_with_headings("- a0\n# Work\n## SFAP\n- s1\n# Life\n- l1\n");
    let chosen = |app: &App| app.tasks_chosen().map(|item| item.title.clone());
    assert_eq!(chosen(&app).as_deref(), Some("a0"));
    press(&mut app, KeyCode::Char('['));
    assert_eq!(chosen(&app).as_deref(), Some("a0"));
    for line in [2, 3, 5] {
        press(&mut app, KeyCode::Char(']'));
        assert_eq!(app.tasks_line, Some(line));
    }
    for line in [3, 2] {
        press(&mut app, KeyCode::Char('['));
        assert_eq!(app.tasks_line, Some(line));
    }
}

/// What each row looks like: heading `#name` (`+#name` when collapsed), item `title` (two spaces per level
/// in front; an item with subtasks gets `+` when collapsed, `-` when expanded)
fn shape(app: &App) -> Vec<String> {
    app.tasks_rows()
        .iter()
        .map(|row| match row {
            TaskRow::Heading {
                name, collapsed, ..
            } => format!(
                "{}#{}",
                if *collapsed { "+" } else { "" },
                name.as_deref().unwrap_or("")
            ),
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
fn shift_arrows_expand_and_collapse_the_whole_subtree() {
    // a has a1 (with a11) and a2; b has b1. The page opens on a, everything collapsed
    let mut app = tasks_app("# W\n- a\n  - a1\n    - a11\n  - a2\n- b\n  - b1\n");
    let chosen = |app: &App| app.tasks_chosen().map(|item| item.title.clone());
    let shift = |app: &mut App, code| app.handle_key(KeyEvent::new(code, KeyModifiers::SHIFT));
    assert_eq!(shape(&app), ["#W", "+a", "+b"]);
    // Shift+→: a and every level under it open; b stays closed
    shift(&mut app, KeyCode::Right);
    assert_eq!(shape(&app), ["#W", "-a", "  -a1", "    a11", "  a2", "+b"]);
    // Shift+←: all of it closes, so → (one level, like l) opens just a again (a1 closed)
    shift(&mut app, KeyCode::Left);
    assert_eq!(shape(&app), ["#W", "+a", "+b"]);
    press(&mut app, KeyCode::Right);
    assert_eq!(shape(&app), ["#W", "-a", "  +a1", "  a2", "+b"]);
    // Shift+← on an item without subtasks (a2): the item it is under closes, and gets chosen
    press(&mut app, KeyCode::Char('j'));
    press(&mut app, KeyCode::Char('j'));
    assert_eq!(chosen(&app).as_deref(), Some("a2"));
    shift(&mut app, KeyCode::Left);
    assert_eq!(shape(&app), ["#W", "+a", "+b"]);
    assert_eq!(chosen(&app).as_deref(), Some("a"));
}

#[test]
fn calendars_collapse_and_expand_like_items() {
    // Lines: 1 # Work, 2 a, 3 a1 (a's subtask), 4 ## SFAP, 5 s, 6 # Life, 7 l, 8 # Empty
    let mut app =
        tasks_app_with_headings("# Work\n- a\n  - a1\n## SFAP\n- s\n# Life\n- l\n# Empty\n");
    let shift = |app: &mut App, code| app.handle_key(KeyEvent::new(code, KeyModifiers::SHIFT));
    let open = ["#Work", "+a", "#SFAP", "s", "#Life", "l", "#Empty"];
    assert_eq!(shape(&app), open);
    // The page opens on a; h (a top-level item, nothing open) goes to Work's heading; h there closes Work
    press(&mut app, KeyCode::Char('h'));
    assert_eq!(app.tasks_line, Some(1));
    press(&mut app, KeyCode::Char('h'));
    assert_eq!(shape(&app), ["+#Work", "#Life", "l", "#Empty"]);
    // l opens it again; l once more goes to the first row inside it (a)
    press(&mut app, KeyCode::Char('l'));
    assert_eq!(shape(&app), open);
    press(&mut app, KeyCode::Char('l'));
    assert_eq!(app.tasks_line, Some(2));
    // Shift+→ on Work's heading: everything inside opens, a's subtasks too
    press(&mut app, KeyCode::Char('k'));
    shift(&mut app, KeyCode::Right);
    assert_eq!(
        shape(&app),
        ["#Work", "-a", "  a1", "#SFAP", "s", "#Life", "l", "#Empty"]
    );
    // Shift+←: Work and SFAP inside it close; l then opens only Work (SFAP and a stay closed)
    shift(&mut app, KeyCode::Left);
    assert_eq!(shape(&app), ["+#Work", "#Life", "l", "#Empty"]);
    press(&mut app, KeyCode::Char('l'));
    assert_eq!(
        shape(&app),
        ["#Work", "+a", "+#SFAP", "#Life", "l", "#Empty"]
    );
    // Empty has nothing inside: h there does not close it, and there is no calendar around it to go to
    press(&mut app, KeyCode::Char('G'));
    press(&mut app, KeyCode::Char('h'));
    assert_eq!(app.tasks_line, Some(8));
    assert_eq!(
        shape(&app),
        ["#Work", "+a", "+#SFAP", "#Life", "l", "#Empty"]
    );
    // Mouse: a click on Work's ▾ (row 0) closes it, another opens it; a right click closes everything
    // inside it (it is open), another opens everything (SFAP and a's subtasks too)
    app.click_task(0, true, Instant::now());
    assert_eq!(shape(&app)[0], "+#Work");
    app.click_task(0, true, Instant::now());
    assert_eq!(shape(&app)[0], "#Work");
    app.right_click_task(0);
    assert_eq!(shape(&app), ["+#Work", "#Life", "l", "#Empty"]);
    app.right_click_task(0);
    assert_eq!(
        shape(&app),
        ["#Work", "-a", "  a1", "#SFAP", "s", "#Life", "l", "#Empty"]
    );
    // After a write, the item it chose shows: with Work, SFAP and Life closed, s (in Work > SFAP) opens
    // Work and SFAP; Life stays closed
    app.right_click_task(0);
    app.collapsed.insert(vec!["Life".to_string()]);
    app.choose_task_line(5);
    assert_eq!(
        app.collapsed,
        std::collections::BTreeSet::from([vec!["Life".to_string()]])
    );
    assert_eq!(
        app.tasks_chosen().map(|item| item.title.as_str()),
        Some("s")
    );
}

#[test]
fn a_collapsed_calendar_counts_the_items_inside_it() {
    // Work: a (with subtask a1) and done d; SFAP inside Work: s; Life: l
    let mut app = tasks_app_with_headings(
        "# Work\n- [ ] a\n  - [ ] a1\n- [x] d\n## SFAP\n- [ ] s\n# Life\n- l\n",
    );
    let work = ["Work".to_string()];
    // The top-level items in Work and in SFAP inside it: a, d, s (a1 is a subtask)
    assert_eq!(app.calendar_item_count(&work), 3);
    // Done tasks hidden: d no longer counts; SFAP unchecked: s neither
    app.settings.calendar_list.hide_done = true;
    assert_eq!(app.calendar_item_count(&work), 2);
    app.groups[1].shown = false;
    assert_eq!(app.calendar_item_count(&work), 1);
}

#[test]
fn equals_and_minus_expand_and_collapse_everything() {
    // Lines: 1 Renew passport, 2 # Work, 3 Design doc, 4 Research, 5 Read RFCs, 6 Outline, 7 Triage bugs,
    // 8 # Personal, 9 Go hiking
    let mut app = tasks_app_with_headings(TREE);
    // =: every level expands, and the same item ("Renew passport") stays chosen
    press(&mut app, KeyCode::Char('='));
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
    // Move down to the deepest item, "Read RFCs" (past Work's heading row); -: everything collapses, the
    // calendars too, and the top-level calendar it was in (Work's heading) is chosen instead
    for _ in 0..4 {
        press(&mut app, KeyCode::Char('j'));
    }
    assert_eq!(
        app.tasks_chosen().map(|item| item.title.as_str()),
        Some("Read RFCs")
    );
    press(&mut app, KeyCode::Char('-'));
    assert_eq!(shape(&app), ["#", "Renew passport", "+#Work", "+#Personal"]);
    assert_eq!(app.tasks_line, Some(2));
    // + (= with Shift, on the same key) expands everything too
    press(&mut app, KeyCode::Char('+'));
    assert_eq!(shape(&app)[3], "-Design doc");
}

#[test]
fn j_and_k_skip_heading_rows_without_a_line_and_stop_at_the_ends() {
    // Only items given, no heading lines: the heading rows cannot be chosen (see
    // `a_calendars_heading_row_can_be_chosen_and_a_adds_there` for ones that can)
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
    press(&mut app, KeyCode::Char('3'));
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
fn only_a_double_click_checks_a_task_off() {
    use super::super::write::tests::clean_up;
    use std::time::Duration;
    let text = "# Work\n- [ ] Design doc\n  - [ ] Research\n- [ ] Weekly update\n";
    let (mut app, path) = tasks_app_with_file("tasks-double-click", text);
    let read = || std::fs::read_to_string(&path).expect("read");
    let start = Instant::now();
    let at = |ms: u64| start + Duration::from_millis(ms);
    // Rows: Work, Design doc (Research collapsed under it), Weekly update.
    // Weekly update clicked, then again 0.9 s later: only chosen, not checked off
    app.click_task(2, false, at(0));
    app.click_task(2, false, at(900));
    assert_eq!(
        app.tasks_chosen().map(|item| item.title.as_str()),
        Some("Weekly update")
    );
    assert_eq!(read(), text);
    // Another click 0.3 s after that one: a double click, checked off
    app.click_task(2, false, at(1200));
    assert!(read().contains("- [x] Weekly update\n"));
    // Two quick clicks on Design doc's ▸: expand, then collapse; it is not checked off
    app.click_task(1, true, at(2000));
    app.click_task(1, true, at(2200));
    assert!(read().contains("- [ ] Design doc\n"));
    assert!(!app.expanded.contains(&2));
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
    // "Personal" ("Weekly update" above is in another group, so L does nothing); j stops on the Personal
    // heading row on the way
    let before = read();
    for _ in 0..4 {
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
    // (A calendar heading, so adding goes into the chosen item's calendar, not into a new 預設 one)
    let text = "# Work\n- [ ] Triage bugs\n- [ ] Weekly update @2026-10-16\n";
    let (mut app, path) = tasks_app_with_file("tasks-conflict", text);
    // Add a different line at the top each time: the chosen item (line 2) never matches
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

#[test]
fn o_opens_the_editor_on_the_chosen_line() {
    // Line 1 is the heading, a1 is on line 2, a2 on line 3
    let mut app = tasks_app("# A\n- a1\n- a2\n");
    press(&mut app, KeyCode::Char('j'));
    press(&mut app, KeyCode::Char('o'));
    assert!(app.open_editor);
    assert_eq!(app.tasks_chosen_line(), Some(3));
    // An empty file: nothing to choose, so the editor starts at the top
    assert_eq!(tasks_app("").tasks_chosen_line(), None);
    // Only on the tasks page: on the calendar `o` does nothing
    let mut app = tasks_app("- a1\n");
    press(&mut app, KeyCode::Char('1'));
    press(&mut app, KeyCode::Char('o'));
    assert!(!app.open_editor);
}
