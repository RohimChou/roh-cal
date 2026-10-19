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
    app.handle_key(KeyEvent::from(KeyCode::Char('3')));
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
    let mut app = tasks_app("# A\n- a1\n# P\n## Q\n- q1\n# P\n- p1\n");
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
    // ] from a1 skips P's heading (nothing right under it) to Q's first item, then on to p1; [ back
    let chosen = |app: &App| app.tasks_chosen().map(|item| item.title.clone());
    press(&mut app, KeyCode::Char(']'));
    assert_eq!(chosen(&app).as_deref(), Some("q1"));
    press(&mut app, KeyCode::Char(']'));
    assert_eq!(chosen(&app).as_deref(), Some("p1"));
    press(&mut app, KeyCode::Char('['));
    press(&mut app, KeyCode::Char('['));
    assert_eq!(chosen(&app).as_deref(), Some("a1"));
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
