use super::super::write::Notice;
use super::super::write::tests::{app_with_task_file, clean_up, press};
use super::*;
use jiff::civil::date;

/// Types text into the editor one character at a time
fn type_text(app: &mut App, text: &str) {
    for c in text.chars() {
        press(app, KeyCode::Char(c));
    }
}

#[test]
fn a_in_the_calendar_list_adds_a_calendar_at_the_end() {
    let text = "# Work\n- [ ] Review PRs @2026-10-10\n";
    let (mut app, path) = app_with_task_file("new-calendar", text);
    app.focus = Focus::Side;
    press(&mut app, KeyCode::Char('a'));
    // A name some calendar already has cannot be saved: the editor stays open
    type_text(&mut app, "Work");
    assert_eq!(
        app.editor().map(Editor::check),
        Some(Err(edit::Problem::NameTaken))
    );
    press(&mut app, KeyCode::Enter);
    assert!(app.editor().is_some());
    // A new name: written last, after a blank line; the calendar list shows it, chosen
    for _ in 0..4 {
        press(&mut app, KeyCode::Backspace);
    }
    type_text(&mut app, "Side projects");
    press(&mut app, KeyCode::Enter);
    assert!(app.editor().is_none());
    assert_eq!(
        std::fs::read_to_string(&path).expect("read"),
        "# Work\n- [ ] Review PRs @2026-10-10\n\n# Side projects\n"
    );
    let names: Vec<Option<&str>> = app
        .groups
        .iter()
        .map(|group| group.name.as_deref())
        .collect();
    assert_eq!(names, [Some("Work"), Some("Side projects")]);
    assert_eq!((app.focus, app.group_index), (Focus::Side, 1));
    // u takes it back out
    press(&mut app, KeyCode::Char('u'));
    assert_eq!(std::fs::read_to_string(&path).expect("read"), text);
    assert_eq!(app.groups.len(), 1);
    assert_eq!(app.notice, Some(Notice::Undone("Side projects".into())));
    clean_up(&path);
}

#[test]
fn capital_a_in_the_calendar_list_adds_a_calendar_inside_the_chosen_one() {
    let text = "# Work\n- [ ] Review PRs @2026-10-10\n## SFAP\n- [ ] Fix the login bug\n\n# Personal\n- Gym\n";
    let (mut app, path) = app_with_task_file("new-subcalendar", text);
    app.focus = Focus::Side;
    // Work chosen: A opens the editor for a calendar inside it; the top border says where it goes
    press(&mut app, KeyCode::Char('A'));
    let editor = app.editor().expect("editor");
    assert!(matches!(&editor.target, Target::AddCalendar { parent, .. } if *parent == ["Work"]));
    // A name already inside Work cannot be saved; one that is only taken elsewhere (Personal) can
    type_text(&mut app, "SFAP");
    assert_eq!(
        app.editor().map(Editor::check),
        Some(Err(edit::Problem::NameTaken))
    );
    for _ in 0..4 {
        press(&mut app, KeyCode::Backspace);
    }
    type_text(&mut app, "Personal");
    assert!(app.editor().is_some_and(|editor| editor.check().is_ok()));
    for _ in 0..8 {
        press(&mut app, KeyCode::Backspace);
    }
    // Saved: `## CAIS` at the end of Work (after SFAP, before the blank line in front of Personal); the
    // calendar list chooses it (Work, Work > SFAP, Work > CAIS, Personal)
    type_text(&mut app, "CAIS");
    press(&mut app, KeyCode::Enter);
    assert!(app.editor().is_none());
    assert_eq!(
        std::fs::read_to_string(&path).expect("read"),
        "# Work\n- [ ] Review PRs @2026-10-10\n## SFAP\n- [ ] Fix the login bug\n\n## CAIS\n\n# Personal\n- Gym\n"
    );
    let paths: Vec<Vec<String>> = app.groups.iter().map(crate::app::Group::path).collect();
    assert_eq!(app.group_index, 2);
    assert_eq!(paths[2], ["Work", "CAIS"]);
    // On 「Hide done」 (below the calendars): A does nothing
    app.group_index = app.groups.len();
    press(&mut app, KeyCode::Char('A'));
    assert!(app.editor().is_none());
    clean_up(&path);
}

#[test]
fn hash_picks_the_calendar_a_new_calendar_goes_inside() {
    let text = "# Work\n## SFAP\n- [ ] Fix the login bug\n\n# Personal\n- Gym\n";
    let (mut app, path) = app_with_task_file("new-calendar-hash", text);
    app.focus = Focus::Side;
    // a (the top level): just `#` lists 「最外層」 first, then every calendar
    press(&mut app, KeyCode::Char('a'));
    type_text(&mut app, "Bugs #");
    let names = |app: &App| -> Vec<String> {
        let editor = app.editor().expect("editor");
        let names = editor.calendar_names();
        editor
            .suggestions()
            .unwrap_or_default()
            .iter()
            .map(|found| names[found.index].clone())
            .collect()
    };
    assert_eq!(names(&app), ["最外層", "Work", "Work > SFAP", "Personal"]);
    // `#sf` → Work > SFAP; Enter picks it: the `#` word goes, the calendar goes inside SFAP
    type_text(&mut app, "sf");
    press(&mut app, KeyCode::Enter);
    let editor = app.editor().expect("editor");
    assert_eq!(editor.input.value(), "Bugs ");
    assert!(
        matches!(&editor.target, Target::AddCalendar { parent, .. } if *parent == ["Work", "SFAP"])
    );
    press(&mut app, KeyCode::Enter);
    assert_eq!(
        std::fs::read_to_string(&path).expect("read"),
        "# Work\n## SFAP\n- [ ] Fix the login bug\n\n### Bugs\n\n# Personal\n- Gym\n"
    );
    clean_up(&path);
    // A inside Work, then 「最外層」 picked: the top level, where SFAP is not taken (it is inside Work)
    let (mut app, path) = app_with_task_file("new-calendar-top", text);
    app.focus = Focus::Side;
    press(&mut app, KeyCode::Char('A'));
    type_text(&mut app, "SFAP");
    assert_eq!(
        app.editor().map(Editor::check),
        Some(Err(edit::Problem::NameTaken))
    );
    type_text(&mut app, " #最");
    press(&mut app, KeyCode::Enter);
    assert!(app.editor().is_some_and(|editor| editor.check().is_ok()));
    press(&mut app, KeyCode::Enter);
    assert_eq!(
        std::fs::read_to_string(&path).expect("read"),
        format!("{text}\n# SFAP\n")
    );
    clean_up(&path);
}

#[test]
fn hash_in_the_rename_editor_moves_the_calendar_inside_another() {
    let text = "# Work\n- a\n\n## SFAP\n- b\n### Bugs\n- c\n\n# Personal\n- d\n";
    let (mut app, path) = app_with_task_file("move-calendar", text);
    app.focus = Focus::Side;
    // Work, Work > SFAP, Work > SFAP > Bugs, Personal: Bugs unchecked, SFAP chosen
    app.groups[2].shown = false;
    app.group_index = 1;
    press(&mut app, KeyCode::Char('e'));
    // The `#` list: 「最外層」 and the calendars SFAP can go inside (not itself, not Bugs inside it)
    type_text(&mut app, " #");
    let editor = app.editor().expect("editor");
    let names = editor.calendar_names();
    let listed: Vec<&str> = editor
        .suggestions()
        .unwrap_or_default()
        .iter()
        .map(|found| names[found.index].as_str())
        .collect();
    assert_eq!(listed, ["最外層", "Work", "Personal"]);
    // Personal picked and saved: SFAP and Bugs go to the end of Personal; still chosen, Bugs still unchecked
    type_text(&mut app, "pers");
    press(&mut app, KeyCode::Enter);
    press(&mut app, KeyCode::Enter);
    assert!(app.editor().is_none());
    assert_eq!(
        std::fs::read_to_string(&path).expect("read"),
        "# Work\n- a\n\n# Personal\n- d\n\n## SFAP\n- b\n### Bugs\n- c\n"
    );
    let chosen = app.groups[app.group_index].path();
    assert_eq!(chosen, ["Personal", "SFAP"]);
    let bugs = app
        .groups
        .iter()
        .find(|group| group.name.as_deref() == Some("Bugs"))
        .expect("Bugs");
    assert_eq!(bugs.path(), ["Personal", "SFAP", "Bugs"]);
    assert!(!bugs.shown);
    // u puts it back in one step
    press(&mut app, KeyCode::Char('u'));
    assert_eq!(std::fs::read_to_string(&path).expect("read"), text);
    clean_up(&path);
    // A calendar whose heading would get more than 6 `#` there is not listed: X (with Y inside) can go
    // inside D (4 `#`: X 5, Y 6), not E (5 `#`)
    let text = "# A\n## B\n### C\n#### D\n##### E\n# X\n## Y\n- y\n";
    let (mut app, path) = app_with_task_file("move-calendar-deep", text);
    app.focus = Focus::Side;
    app.group_index = 5;
    press(&mut app, KeyCode::Char('e'));
    type_text(&mut app, " #");
    let editor = app.editor().expect("editor");
    assert!(
        editor
            .calendar_names()
            .contains(&"A > B > C > D".to_string())
    );
    assert!(
        !editor
            .calendar_names()
            .contains(&"A > B > C > D > E".to_string())
    );
    clean_up(&path);
}

#[test]
fn e_in_the_calendar_list_renames_every_heading_of_the_calendar() {
    // Work appears twice (one calendar) and has SFAP inside it
    let text = "# Work\n- [ ] Review PRs @2026-10-10\n## SFAP\n- [ ] Fix the login bug @2026-10-11\n\
                    # Days off type:holiday\n- Long weekend @2026-10-12\n# Work\n- [ ] Deploy @2026-10-20\n";
    let (mut app, path) = app_with_task_file("rename-calendar", text);
    let names = |app: &App| -> Vec<(String, bool)> {
        app.groups
            .iter()
            .map(|group| (group.path().join(" > "), group.shown))
            .collect()
    };
    app.focus = Focus::Side;
    // The holiday calendar (third in the tree), unchecked: the editor holds its heading text,
    // type:holiday too
    app.group_index = 2;
    app.groups[2].shown = false;
    press(&mut app, KeyCode::Char('e'));
    let typed = "Days off type:holiday";
    assert_eq!(app.editor().map(|editor| editor.input.value()), Some(typed));
    // The name of another calendar at the same level cannot be taken
    for _ in typed.chars() {
        press(&mut app, KeyCode::Backspace);
    }
    type_text(&mut app, "Work");
    press(&mut app, KeyCode::Enter);
    assert!(app.editor().is_some());
    // A new name: its heading is rewritten, the calendar stays chosen and unchecked
    for _ in 0..4 {
        press(&mut app, KeyCode::Backspace);
    }
    type_text(&mut app, "Holidays type:holiday");
    press(&mut app, KeyCode::Enter);
    let holidays = text.replace("# Days off", "# Holidays");
    assert_eq!(std::fs::read_to_string(&path).expect("read"), holidays);
    assert_eq!(app.group_index, 2);
    assert_eq!(names(&app)[2], ("Holidays".to_string(), false));
    // Work, with SFAP unchecked: both Work headings are renamed, and SFAP (now inside Day job) stays
    // unchecked
    app.groups[1].shown = false;
    app.group_index = 0;
    press(&mut app, KeyCode::Char('e'));
    for _ in 0..4 {
        press(&mut app, KeyCode::Backspace);
    }
    type_text(&mut app, "Day job");
    press(&mut app, KeyCode::Enter);
    let renamed = holidays.replace("# Work", "# Day job");
    assert_eq!(std::fs::read_to_string(&path).expect("read"), renamed);
    assert_eq!(
        names(&app),
        [
            ("Day job".to_string(), true),
            ("Day job > SFAP".to_string(), false),
            ("Holidays".to_string(), false),
        ]
    );
    assert_eq!(app.group_index, 0);
    // Enter without changing anything: nothing written (no undo step)
    let steps = app.undo.len();
    press(&mut app, KeyCode::Char('e'));
    press(&mut app, KeyCode::Enter);
    assert!(app.editor().is_none());
    assert_eq!(app.undo.len(), steps);
    // u brings the old name back
    press(&mut app, KeyCode::Char('u'));
    assert_eq!(std::fs::read_to_string(&path).expect("read"), holidays);
    clean_up(&path);
}

#[test]
fn a_failed_write_keeps_the_editor_and_what_was_typed() {
    use std::os::unix::fs::PermissionsExt;
    let text = "- [ ] Review PRs @2026-10-10\n";
    let (mut app, path) = app_with_task_file("editor-failed", text);
    press(&mut app, KeyCode::Char('e'));
    type_text(&mut app, " for Amy");
    // Make the folder read-only: the temp file cannot be written (reading still works)
    let dir = path.parent().expect("dir");
    let mode = |mode| std::fs::set_permissions(dir, std::fs::Permissions::from_mode(mode));
    mode(0o555).expect("read-only");
    press(&mut app, KeyCode::Enter);
    // The editor is still open, the typed text is still there (the selected day's date left out), and
    // the bottom row says why
    assert_eq!(
        app.editor().map(|editor| editor.input.value()),
        Some("- [ ] Review PRs for Amy")
    );
    assert_eq!(
        app.notice,
        Some(Notice::Failed(std::io::ErrorKind::PermissionDenied))
    );
    assert_eq!(std::fs::read_to_string(&path).expect("read"), text);
    // Make it writable again: pressing Enter once more saves and closes the editor
    mode(0o755).expect("writable");
    press(&mut app, KeyCode::Enter);
    assert!(app.editor().is_none());
    assert_eq!(
        std::fs::read_to_string(&path).expect("read"),
        "- [ ] Review PRs for Amy @2026-10-10\n"
    );
    clean_up(&path);
}

#[test]
fn e_opens_the_line_and_enter_saves_what_was_typed() {
    let text = "# Recurring\n  - [ ] Send the weekly update @2026-10-16 17:30\n";
    let (mut app, path) = app_with_task_file("edit", text);
    // e: the editor holds that line (without indentation), cursor at the end; the item is on the
    // selected day (10/16), so its date is left out and the time stays as @17:30
    press(&mut app, KeyCode::Char('e'));
    let editor = app.editor().expect("editor");
    assert_eq!(editor.input.value(), "- [ ] Send the weekly update @17:30");
    // Change 17:30 to 18:00: the cursor is at the end, press Backspace 5 times, then type; saving puts
    // the day back
    for _ in 0..5 {
        press(&mut app, KeyCode::Backspace);
    }
    type_text(&mut app, "18:00 to my manager");
    press(&mut app, KeyCode::Enter);
    // Indentation kept; "to my manager" follows the time and is part of the title
    assert_eq!(
        std::fs::read_to_string(&path).expect("read"),
        "# Recurring\n  - [ ] Send the weekly update @2026-10-16 18:00 to my manager\n"
    );
    assert!(app.editor().is_none());
    assert_eq!(
        app.chosen().map(|entry| entry.item.title.as_str()),
        Some("Send the weekly update to my manager")
    );
    // u undoes
    press(&mut app, KeyCode::Char('u'));
    assert_eq!(std::fs::read_to_string(&path).expect("read"), text);
    clean_up(&path);
}

#[test]
fn a_in_the_agenda_adds_to_the_chosen_items_group() {
    let text =
        "# Work\n- [ ] Write the design doc @2026-10-16\n# Personal\n- Go hiking @2026-10-18\n";
    let (mut app, path) = app_with_task_file("add", text);
    // a: only the box is prefilled, cursor where the title goes (the selected day is not shown: it is
    // put on when saving)
    press(&mut app, KeyCode::Char('a'));
    let editor = app.editor().expect("editor");
    assert_eq!(editor.input.value(), "- [ ] ");
    assert_eq!(editor.input.cursor(), 6);
    type_text(&mut app, "Send the weekly update");
    press(&mut app, KeyCode::Enter);
    // Added at the end of "Work", on the selected day; the agenda chooses the new item
    assert_eq!(
        std::fs::read_to_string(&path).expect("read"),
        "# Work\n- [ ] Write the design doc @2026-10-16\n- [ ] Send the weekly update @2026-10-16\n# Personal\n- Go hiking @2026-10-18\n"
    );
    assert_eq!(
        app.chosen().map(|entry| entry.item.title.as_str()),
        Some("Send the weekly update")
    );
    // u: removes the whole line; the notice names the added item
    press(&mut app, KeyCode::Char('u'));
    assert_eq!(std::fs::read_to_string(&path).expect("read"), text);
    assert_eq!(
        app.notice,
        Some(Notice::Undone("Send the weekly update".into()))
    );
    clean_up(&path);
}

#[test]
fn with_no_calendar_yet_a_new_item_goes_into_a_default_calendar() {
    // An empty task file: 「預設」 is written first, the item under it, in one step (u takes both back)
    let (mut app, path) = app_with_task_file("default-calendar", "");
    let read = || std::fs::read_to_string(&path).expect("read");
    app.focus = Focus::Calendar;
    press(&mut app, KeyCode::Char('a'));
    type_text(&mut app, "Get a flu shot");
    press(&mut app, KeyCode::Enter);
    assert_eq!(read(), "# 預設\n- [ ] Get a flu shot @2026-10-17\n");
    let names: Vec<Option<&str>> = app
        .groups
        .iter()
        .map(|group| group.name.as_deref())
        .collect();
    assert_eq!(names, [Some("預設")]);
    // Once there is a calendar, adding works as usual (the end of the file is inside 預設)
    app.focus = Focus::Calendar;
    app.selected = date(2026, 10, 20);
    press(&mut app, KeyCode::Char('a'));
    type_text(&mut app, "Renew passport");
    press(&mut app, KeyCode::Enter);
    assert_eq!(
        read(),
        "# 預設\n- [ ] Get a flu shot @2026-10-17\n- [ ] Renew passport @2026-10-20\n"
    );
    press(&mut app, KeyCode::Char('u'));
    press(&mut app, KeyCode::Char('u'));
    assert_eq!(read(), "");
    clean_up(&path);
    // Only items before any heading (no calendar yet): 預設 goes at the end; a subtask (A) does not
    // make one
    let (mut app, path) = app_with_task_file("default-calendar-loose", "- [ ] Loose @2026-10-17\n");
    press(&mut app, KeyCode::Char('A'));
    type_text(&mut app, "Sub");
    press(&mut app, KeyCode::Enter);
    assert_eq!(
        std::fs::read_to_string(&path).expect("read"),
        "- [ ] Loose @2026-10-17\n  - [ ] Sub\n"
    );
    app.focus = Focus::Calendar;
    press(&mut app, KeyCode::Char('a'));
    type_text(&mut app, "New");
    press(&mut app, KeyCode::Enter);
    assert_eq!(
        std::fs::read_to_string(&path).expect("read"),
        "- [ ] Loose @2026-10-17\n  - [ ] Sub\n\n# 預設\n- [ ] New @2026-10-17\n"
    );
    clean_up(&path);
}

/// The calendar a new item goes into, e.g. `Work > SFAP`
fn calendar(app: &App) -> Option<String> {
    app.editor()
        .and_then(Editor::calendar)
        .map(|path| path.join(" > "))
}

/// What the `#` list shows, in order (empty when it is closed)
fn listed(app: &App) -> Vec<String> {
    app.editor()
        .and_then(|editor| {
            let names = editor.calendar_names();
            let found = editor.suggestions()?;
            Some(
                found
                    .into_iter()
                    .map(|found| names[found.index].clone())
                    .collect(),
            )
        })
        .unwrap_or_default()
}

#[test]
fn hash_picks_the_calendar_a_new_item_goes_into() {
    // Selected 10/20 (nothing that day): the first calendar that is not a holiday calendar (Work, not
    // the holidays listed before it)
    let text = "# Holidays type:holiday\n- Day off @2026-10-09\n# Work\n- [ ] Write the design doc @2026-10-16\n## SFAP\n- [ ] Estimate @2026-10-12\n# Personal\n";
    let (mut app, path) = app_with_task_file("add-hash", text);
    press(&mut app, KeyCode::Esc);
    app.selected = date(2026, 10, 20);
    press(&mut app, KeyCode::Char('a'));
    assert_eq!(calendar(&app).as_deref(), Some("Work"));
    // Just `#`: every calendar, in the calendar list's order; `pe` leaves Personal
    type_text(&mut app, "#");
    assert_eq!(
        listed(&app),
        ["Holidays", "Work", "Work > SFAP", "Personal"]
    );
    type_text(&mut app, "pe");
    assert_eq!(listed(&app), ["Personal"]);
    // Enter picks it: the #pe word is taken out, the list closes
    press(&mut app, KeyCode::Enter);
    assert_eq!(calendar(&app).as_deref(), Some("Personal"));
    assert_eq!(
        app.editor().map(|editor| editor.input.value()),
        Some("- [ ] ")
    );
    assert_eq!(listed(&app), [] as [&str; 0]);
    // Personal has no items yet: the new one goes right under its heading
    type_text(&mut app, "Call the bank");
    press(&mut app, KeyCode::Enter);
    assert_eq!(
        std::fs::read_to_string(&path).expect("read"),
        format!("{text}- [ ] Call the bank @2026-10-20\n")
    );
    // Now 10/20 has an item (in Personal): `a` starts there. `#w` after the title: both Works match
    // equally well (list order); down / up / Ctrl+N move (not past the ends), Tab picks Work > SFAP, where
    // it goes after the last item, before the next heading
    press(&mut app, KeyCode::Esc);
    press(&mut app, KeyCode::Char('a'));
    assert_eq!(calendar(&app).as_deref(), Some("Personal"));
    type_text(&mut app, "Review #w");
    assert_eq!(listed(&app), ["Work", "Work > SFAP"]);
    let index = |app: &App| app.editor().map(|editor| editor.hash_list.index);
    press(&mut app, KeyCode::Up);
    assert_eq!(index(&app), Some(0));
    press(&mut app, KeyCode::Down);
    press(&mut app, KeyCode::Down);
    assert_eq!(index(&app), Some(1));
    press(&mut app, KeyCode::Up);
    app.handle_key(KeyEvent::new(KeyCode::Char('n'), KeyModifiers::CONTROL));
    assert_eq!(index(&app), Some(1));
    press(&mut app, KeyCode::Tab);
    assert_eq!(calendar(&app).as_deref(), Some("Work > SFAP"));
    assert_eq!(
        app.editor().map(|editor| editor.input.value()),
        Some("- [ ] Review ")
    );
    press(&mut app, KeyCode::Enter);
    assert!(
        std::fs::read_to_string(&path)
            .expect("read")
            .contains("- [ ] Estimate @2026-10-12\n- [ ] Review @2026-10-20\n# Personal\n")
    );
    // A subtask (A) stays under its item: `#` is only text, no list
    press(&mut app, KeyCode::Char('A'));
    type_text(&mut app, "#w");
    assert_eq!(listed(&app), [] as [&str; 0]);
    assert_eq!(calendar(&app), None);
    clean_up(&path);
}

#[test]
fn a_closed_hash_list_leaves_the_word_as_a_tag() {
    let text = "# Work\n- [ ] Write the design doc @2026-10-16\n## SFAP\n";
    let (mut app, path) = app_with_task_file("add-hash-tag", text);
    press(&mut app, KeyCode::Char('a'));
    // A word in the middle of the text opens the list too; picking takes the word and the space after it
    type_text(&mut app, "#sf Ship it");
    assert_eq!(listed(&app), [] as [&str; 0]);
    for _ in 0.." Ship it".len() {
        press(&mut app, KeyCode::Left);
    }
    press(&mut app, KeyCode::Backspace);
    type_text(&mut app, "f");
    assert_eq!(listed(&app), ["Work > SFAP"]);
    press(&mut app, KeyCode::Enter);
    assert_eq!(
        app.editor().map(|editor| editor.input.value()),
        Some("- [ ] Ship it")
    );
    assert_eq!(calendar(&app).as_deref(), Some("Work > SFAP"));
    // The cursor is where the word was
    assert_eq!(app.editor().map(|editor| editor.input.cursor()), Some(6));
    // Esc closes only the list: the editor stays open, the word stays, and Enter saves it as part of the
    // title (an Obsidian tag) in the calendar it already had
    press(&mut app, KeyCode::End);
    type_text(&mut app, " #wo");
    assert_eq!(listed(&app), ["Work", "Work > SFAP"]);
    press(&mut app, KeyCode::Esc);
    assert_eq!(listed(&app), [] as [&str; 0]);
    assert!(app.editor().is_some());
    // Moving the cursor keeps it closed; typing in the word opens it again, a space closes it
    press(&mut app, KeyCode::Left);
    press(&mut app, KeyCode::Right);
    assert_eq!(listed(&app), [] as [&str; 0]);
    type_text(&mut app, "r");
    assert_eq!(listed(&app), ["Work", "Work > SFAP"]);
    type_text(&mut app, " ");
    assert_eq!(listed(&app), [] as [&str; 0]);
    // Nothing matches: no list, so Enter saves
    type_text(&mut app, "#zzz");
    assert_eq!(listed(&app), [] as [&str; 0]);
    press(&mut app, KeyCode::Enter);
    assert_eq!(
        std::fs::read_to_string(&path).expect("read"),
        format!("{text}- [ ] Ship it #wor #zzz @2026-10-16\n")
    );
    clean_up(&path);
}

#[test]
fn hash_moves_an_edited_item_with_its_subtasks() {
    let text = "# Trip\n- [ ] Book flights @2026-10-16\n  - [ ] Compare prices\n# Work\n- [ ] Write the design doc\n";
    let (mut app, path) = app_with_task_file("edit-hash", text);
    // The top border names the calendar it is in
    press(&mut app, KeyCode::Char('e'));
    assert_eq!(calendar(&app).as_deref(), Some("Trip"));
    assert_eq!(
        app.editor().map(|editor| editor.input.value()),
        Some("- [ ] Book flights")
    );
    // Pick Work: it moves there on save, as Work's last item, with its subtask; still chosen
    type_text(&mut app, " #wo");
    assert_eq!(listed(&app), ["Work"]);
    press(&mut app, KeyCode::Enter);
    assert_eq!(calendar(&app).as_deref(), Some("Work"));
    press(&mut app, KeyCode::Enter);
    assert!(app.editor().is_none());
    assert_eq!(
        std::fs::read_to_string(&path).expect("read"),
        "# Trip\n# Work\n- [ ] Write the design doc\n- [ ] Book flights @2026-10-16\n  - [ ] Compare prices\n"
    );
    assert_eq!(
        app.chosen()
            .map(|entry| (entry.item.title.as_str(), entry.item.line)),
        Some(("Book flights", 4))
    );
    // u puts it back in one step
    press(&mut app, KeyCode::Char('u'));
    assert_eq!(std::fs::read_to_string(&path).expect("read"), text);
    // Picking the calendar it is already in, with nothing else changed: nothing is written
    let steps = app.undo.len();
    press(&mut app, KeyCode::Char('e'));
    type_text(&mut app, " #tr");
    press(&mut app, KeyCode::Enter);
    assert_eq!(calendar(&app).as_deref(), Some("Trip"));
    press(&mut app, KeyCode::Enter);
    assert!(app.editor().is_none());
    assert_eq!(app.undo.len(), steps);
    assert_eq!(std::fs::read_to_string(&path).expect("read"), text);
    clean_up(&path);
}

#[test]
fn a_on_the_calendar_adds_on_the_selected_day() {
    // Selected 10/20 (nothing that day): added at the end of
    // the file; the keyboard moves to the agenda with it chosen
    let text = "# Work\n- [ ] Write the design doc @2026-10-16\n";
    let (mut app, path) = app_with_task_file("add-calendar", text);
    press(&mut app, KeyCode::Esc);
    app.selected = date(2026, 10, 20);
    press(&mut app, KeyCode::Char('a'));
    type_text(&mut app, "Get a flu shot");
    press(&mut app, KeyCode::Enter);
    assert_eq!(
        std::fs::read_to_string(&path).expect("read"),
        "# Work\n- [ ] Write the design doc @2026-10-16\n- [ ] Get a flu shot @2026-10-20\n"
    );
    assert_eq!(app.focus, Focus::Agenda);
    // Selected 10/16 ("Write the design doc" is that day): added to its
    // group; layout 2 has no agenda, so the keyboard stays on the calendar
    app.view = crate::app::View::Month;
    app.focus = Focus::Calendar;
    app.selected = date(2026, 10, 16);
    press(&mut app, KeyCode::Char('a'));
    type_text(&mut app, "Share it with the team");
    press(&mut app, KeyCode::Enter);
    assert!(
        std::fs::read_to_string(&path).expect("read").contains(
            "- [ ] Get a flu shot @2026-10-20\n- [ ] Share it with the team @2026-10-16\n"
        )
    );
    assert_eq!(app.focus, Focus::Calendar);
    clean_up(&path);
}

#[test]
fn shift_a_adds_a_subtask_and_keeps_the_parent_chosen() {
    let text = "- [ ] Write the design doc @2026-10-16\n- [ ] Send the weekly update @2026-10-16\n";
    let (mut app, path) = app_with_task_file("add-subtask", text);
    press(&mut app, KeyCode::Char('A'));
    assert_eq!(
        app.editor().map(|editor| editor.input.value()),
        Some("- [ ] ")
    );
    type_text(&mut app, "Gather requirements");
    press(&mut app, KeyCode::Enter);
    assert_eq!(
        std::fs::read_to_string(&path).expect("read"),
        "- [ ] Write the design doc @2026-10-16\n  - [ ] Gather requirements\n- [ ] Send the weekly update @2026-10-16\n"
    );
    // The subtask has no date and is not in the agenda: "Write the design doc" stays chosen
    assert_eq!(
        app.chosen().map(|entry| entry.item.title.as_str()),
        Some("Write the design doc")
    );
    clean_up(&path);
}

#[test]
fn adding_creates_a_missing_task_file_and_needs_a_title() {
    let (mut app, path) = app_with_task_file("add-new-file", "");
    std::fs::remove_file(&path).expect("remove");
    app.focus = Focus::Calendar;
    // Enter without a title: cannot save, the editor stays open
    press(&mut app, KeyCode::Char('a'));
    press(&mut app, KeyCode::Enter);
    assert_eq!(
        app.editor().map(Editor::check),
        Some(Err(edit::Problem::EmptyTitle))
    );
    assert!(!path.exists());
    // Title typed: the file is created, the item in a new 預設 calendar (the file had none)
    type_text(&mut app, "First task");
    press(&mut app, KeyCode::Enter);
    assert_eq!(
        std::fs::read_to_string(&path).expect("read"),
        "# 預設\n- [ ] First task @2026-10-17\n"
    );
    clean_up(&path);
}

#[test]
fn a_line_with_a_mistake_is_not_saved() {
    let text = "- [ ] Send the weekly update @2026-10-16\n";
    let (mut app, path) = app_with_task_file("edit-mistake", text);
    press(&mut app, KeyCode::Enter);
    // Type a date that does not exist (10/32; the item's own date is left out in the editor): Enter
    // does not save, the editor stays open, the check result is an error
    type_text(&mut app, " @2026-10-32");
    press(&mut app, KeyCode::Enter);
    let editor = app.editor().expect("still open");
    assert!(editor.check().is_err());
    assert_eq!(std::fs::read_to_string(&path).expect("read"), text);
    // Remove the leading `- `: not an item, not saved either
    press(&mut app, KeyCode::Home);
    press(&mut app, KeyCode::Delete);
    press(&mut app, KeyCode::Delete);
    assert_eq!(
        app.editor().map(Editor::check),
        Some(Err(edit::Problem::NotAnItem))
    );
    // Esc: not saved, closed
    press(&mut app, KeyCode::Esc);
    assert!(app.editor().is_none());
    assert_eq!(std::fs::read_to_string(&path).expect("read"), text);
    clean_up(&path);
}

#[test]
fn keys_go_to_the_editor_while_it_is_open() {
    // q, u, j, Space are all typing: they do not quit, undo, change the chosen item or check it off
    let text = "- [ ] Send the weekly update @2026-10-16\n";
    let (mut app, path) = app_with_task_file("edit-keys", text);
    press(&mut app, KeyCode::Char('e'));
    type_text(&mut app, " qu j");
    assert!(!app.should_quit);
    assert_eq!(
        app.editor().map(|editor| editor.input.value()),
        Some("- [ ] Send the weekly update qu j")
    );
    // Saving with no change: nothing is written, no undo is recorded
    press(&mut app, KeyCode::Esc);
    press(&mut app, KeyCode::Char('e'));
    press(&mut app, KeyCode::Enter);
    press(&mut app, KeyCode::Char('u'));
    assert_eq!(app.notice, Some(Notice::NothingToUndo));
    clean_up(&path);
}

#[test]
fn taking_the_date_away_leaves_the_agenda() {
    // A multi-day item shows its whole range in the editor; range removed: after saving, the agenda no
    // longer has it and the keyboard goes back to the calendar
    let text = "- Conference @2026-10-16..2026-10-18\n";
    let (mut app, path) = app_with_task_file("edit-undated", text);
    press(&mut app, KeyCode::Char('e'));
    assert_eq!(
        app.editor().map(|editor| editor.input.value()),
        Some("- Conference @2026-10-16..2026-10-18")
    );
    for _ in 0.." @2026-10-16..2026-10-18".chars().count() {
        press(&mut app, KeyCode::Backspace);
    }
    press(&mut app, KeyCode::Enter);
    assert_eq!(
        std::fs::read_to_string(&path).expect("read"),
        "- Conference\n"
    );
    // The selected day stays: 10/17 was one of the conference's days, so choosing it did not move it
    assert_eq!(app.focus, Focus::Calendar);
    assert_eq!(app.selected, date(2026, 10, 17));
    clean_up(&path);
}

#[test]
fn saving_an_untouched_line_writes_nothing() {
    // The date is written before the title: the editor shows 「- [ ] Pack」, and putting the day back
    // would move it to the end. Opened and saved without a change: the file stays as it is
    let text = "- [ ] @2026-10-16 Pack the bag\n";
    let (mut app, path) = app_with_task_file("edit-untouched", text);
    press(&mut app, KeyCode::Char('e'));
    assert_eq!(
        app.editor().map(|editor| editor.input.value()),
        Some("- [ ] Pack the bag")
    );
    press(&mut app, KeyCode::Enter);
    assert!(app.editor().is_none());
    assert_eq!(std::fs::read_to_string(&path).expect("read"), text);
    // A change is saved, with the day put back at the end
    press(&mut app, KeyCode::Char('e'));
    type_text(&mut app, "s");
    press(&mut app, KeyCode::Enter);
    assert_eq!(
        std::fs::read_to_string(&path).expect("read"),
        "- [ ] Pack the bags @2026-10-16\n"
    );
    clean_up(&path);
}

#[test]
fn the_tasks_page_edits_and_adds_the_whole_line() {
    // The tasks page has no selected day: the editor shows the line with its date, a new item has no
    // date, and taking the date away makes the item undated
    let text = "- [ ] Send the weekly update @2026-10-16 17:30\n";
    let (mut app, path) = app_with_task_file("edit-tasks-page", text);
    press(&mut app, KeyCode::Char('3'));
    press(&mut app, KeyCode::Char('e'));
    assert_eq!(
        app.editor().map(|editor| editor.input.value()),
        Some("- [ ] Send the weekly update @2026-10-16 17:30")
    );
    for _ in 0.." @2026-10-16 17:30".chars().count() {
        press(&mut app, KeyCode::Backspace);
    }
    press(&mut app, KeyCode::Enter);
    assert_eq!(
        std::fs::read_to_string(&path).expect("read"),
        "- [ ] Send the weekly update\n"
    );
    clean_up(&path);
}
