use super::*;
use crate::test_utils::{mouse_event, press};
use jiff::civil::date;

#[test]
fn hjkl_and_arrows_move_by_a_day_or_a_week() {
    let mut app = App::new(Lang::En, date(2026, 10, 17), Vec::new());
    press(&mut app, KeyCode::Char('j'));
    assert_eq!(app.selected, date(2026, 10, 24));
    press(&mut app, KeyCode::Char('l'));
    assert_eq!(app.selected, date(2026, 10, 25));
    press(&mut app, KeyCode::Char('k'));
    assert_eq!(app.selected, date(2026, 10, 18));
    press(&mut app, KeyCode::Char('h'));
    assert_eq!(app.selected, date(2026, 10, 17));
    // Arrow keys do the same; moving across months and years works as usual
    press(&mut app, KeyCode::Down);
    press(&mut app, KeyCode::Down);
    assert_eq!(app.selected, date(2026, 10, 31));
    press(&mut app, KeyCode::Right);
    assert_eq!(app.selected, date(2026, 11, 1));
    press(&mut app, KeyCode::Up);
    press(&mut app, KeyCode::Left);
    assert_eq!(app.selected, date(2026, 10, 24));
    app.selected = date(2026, 12, 29);
    press(&mut app, KeyCode::Char('j'));
    assert_eq!(app.selected, date(2027, 1, 5));
}

#[test]
fn wheel_moves_a_week() {
    let mut app = App::new(Lang::En, date(2026, 10, 17), Vec::new());
    app.handle_mouse(mouse_event(MouseEventKind::ScrollDown, 0, 0));
    assert_eq!(app.selected, date(2026, 10, 24));
    app.handle_mouse(mouse_event(MouseEventKind::ScrollUp, 0, 0));
    app.handle_mouse(mouse_event(MouseEventKind::ScrollUp, 0, 0));
    assert_eq!(app.selected, date(2026, 10, 10));
}

#[test]
fn left_click_on_a_date_selects_it() {
    // 47 × 26 screen, October at the top: the 16th is at (28-31, 5), the 17th at (32-35, 5); see the screen
    // tests in ui/tests.rs
    let mut app = App::new(Lang::En, date(2026, 10, 17), Vec::new());
    app.screen = Rect::new(0, 0, 47, 26);
    app.calendar_top = Some(0);
    app.handle_mouse(mouse_event(MouseEventKind::Down(MouseButton::Left), 29, 5));
    assert_eq!(app.selected, date(2026, 10, 16));
    // Right click, click on the header: ignored
    app.handle_mouse(mouse_event(MouseEventKind::Down(MouseButton::Right), 33, 5));
    app.handle_mouse(mouse_event(MouseEventKind::Down(MouseButton::Left), 33, 1));
    assert_eq!(app.selected, date(2026, 10, 16));
}

#[test]
fn brackets_jump_to_the_first_of_the_previous_or_next_month() {
    let mut app = App::new(Lang::En, date(2026, 10, 17), Vec::new());
    app.calendar_top = Some(3);
    press(&mut app, KeyCode::Char(']'));
    assert_eq!(app.selected, date(2026, 11, 1));
    // After the jump, that month must scroll to the top (on the next draw)
    assert_eq!(app.calendar_top, None);
    press(&mut app, KeyCode::Char('['));
    press(&mut app, KeyCode::Char('['));
    assert_eq!(app.selected, date(2026, 9, 1));
    // Across a year
    app.selected = date(2026, 12, 15);
    press(&mut app, KeyCode::Char(']'));
    assert_eq!(app.selected, date(2027, 1, 1));
    app.selected = date(2027, 1, 20);
    press(&mut app, KeyCode::Char('['));
    assert_eq!(app.selected, date(2026, 12, 1));
}

/// An app with today = 2026/10/17 and `text` as the task file
fn app_with(text: &str) -> App {
    App::new(
        Lang::En,
        date(2026, 10, 17),
        crate::tasks::parse(text).items,
    )
}

/// The agenda in order: 10/10 a, 10/17 b, 10/17 c, 10/20 d (the undated one is not listed)
const ITEMS: &str =
    "- d @2026-10-20\n- c @2026-10-17 09:00\n- No date\n- b @2026-10-17\n- a @2026-10-10\n";

#[test]
fn tab_moves_into_the_agenda_and_back() {
    // 10/17 selected → moving to the agenda chooses the first item on 10/17 (b, with no time, comes first)
    let mut app = app_with(ITEMS);
    press(&mut app, KeyCode::Tab);
    assert_eq!(app.focus, Focus::Agenda);
    assert_eq!(
        app.chosen().map(|entry| entry.item.title.as_str()),
        Some("b")
    );
    press(&mut app, KeyCode::Tab);
    assert_eq!(app.focus, Focus::Calendar);
    assert_eq!(app.chosen(), None);
    // Nothing on or after the selected day: choose the last item, and the calendar jumps to its day
    app.selected = date(2026, 12, 1);
    press(&mut app, KeyCode::Tab);
    assert_eq!(
        app.chosen().map(|entry| entry.item.title.as_str()),
        Some("d")
    );
    assert_eq!(app.selected, date(2026, 10, 20));
}

#[test]
fn shift_tab_goes_through_the_panes_backwards() {
    // Wide enough for the side menu: calendar → side menu → agenda (10/17's first item, b) → calendar
    let mut app = app_with(ITEMS);
    app.screen = Rect::new(0, 0, 140, 30);
    let mut order = Vec::new();
    for _ in 0..3 {
        press(&mut app, KeyCode::BackTab);
        order.push(app.focus);
    }
    assert_eq!(order, [Focus::Side, Focus::Agenda, Focus::Calendar]);
    // Narrow (no side menu): calendar ⇄ agenda; with no items it stays on the calendar
    let mut app = app_with(ITEMS);
    press(&mut app, KeyCode::BackTab);
    assert_eq!(
        (
            app.focus,
            app.chosen().map(|entry| entry.item.title.as_str())
        ),
        (Focus::Agenda, Some("b"))
    );
    press(&mut app, KeyCode::BackTab);
    assert_eq!(app.focus, Focus::Calendar);
    let mut app = app_with("- No date\n");
    press(&mut app, KeyCode::BackTab);
    assert_eq!(app.focus, Focus::Calendar);
    // Side menu with no items: side menu → calendar (the agenda is skipped)
    app.screen = Rect::new(0, 0, 140, 30);
    press(&mut app, KeyCode::BackTab);
    press(&mut app, KeyCode::BackTab);
    assert_eq!(app.focus, Focus::Calendar);
    // Layout 2: calendar ⇄ side menu; the day popup ignores it
    let mut app = app_with(ITEMS);
    app.screen = Rect::new(0, 0, 140, 30);
    app.view = View::Month;
    press(&mut app, KeyCode::BackTab);
    assert_eq!(app.focus, Focus::Side);
    press(&mut app, KeyCode::BackTab);
    assert_eq!(app.focus, Focus::Calendar);
    press(&mut app, KeyCode::Enter);
    press(&mut app, KeyCode::BackTab);
    assert_eq!(app.focus, Focus::Day);
    // Tasks page: tasks page ⇄ side menu
    let mut app = app_with(ITEMS);
    app.screen = Rect::new(0, 0, 140, 30);
    press(&mut app, KeyCode::Char('3'));
    press(&mut app, KeyCode::BackTab);
    assert_eq!(app.focus, Focus::Side);
    press(&mut app, KeyCode::BackTab);
    assert_eq!(app.focus, Focus::Tasks);
}

#[test]
fn tab_stays_on_the_calendar_without_items() {
    let mut app = app_with("- No date\n");
    press(&mut app, KeyCode::Tab);
    assert_eq!(app.focus, Focus::Calendar);
}

#[test]
fn jk_in_the_agenda_choose_items_and_the_calendar_follows() {
    let mut app = app_with(ITEMS);
    press(&mut app, KeyCode::Tab);
    press(&mut app, KeyCode::Char('j'));
    assert_eq!(
        app.chosen().map(|entry| entry.item.title.as_str()),
        Some("c")
    );
    press(&mut app, KeyCode::Down);
    assert_eq!(
        app.chosen().map(|entry| entry.item.title.as_str()),
        Some("d")
    );
    assert_eq!(app.selected, date(2026, 10, 20));
    // Down from the last item: stays
    press(&mut app, KeyCode::Char('j'));
    assert_eq!(
        app.chosen().map(|entry| entry.item.title.as_str()),
        Some("d")
    );
    // Up to the first item, then up again: stays
    for _ in 0..5 {
        press(&mut app, KeyCode::Char('k'));
    }
    assert_eq!(
        app.chosen().map(|entry| entry.item.title.as_str()),
        Some("a")
    );
    assert_eq!(app.selected, date(2026, 10, 10));
    // h / l do nothing in the agenda
    press(&mut app, KeyCode::Char('l'));
    assert_eq!(app.selected, date(2026, 10, 10));
}

#[test]
fn esc_in_the_agenda_goes_back_to_the_calendar() {
    let mut app = app_with(ITEMS);
    press(&mut app, KeyCode::Tab);
    press(&mut app, KeyCode::Esc);
    assert_eq!(app.focus, Focus::Calendar);
    assert!(!app.should_quit);
    // Already on the calendar: Esc quits
    press(&mut app, KeyCode::Esc);
    assert!(app.should_quit);
}

#[test]
fn t_and_brackets_go_back_to_the_calendar() {
    for code in [KeyCode::Char('t'), KeyCode::Char('['), KeyCode::Char(']')] {
        let mut app = app_with(ITEMS);
        press(&mut app, KeyCode::Tab);
        press(&mut app, code);
        assert_eq!(app.focus, Focus::Calendar, "{code:?}");
    }
}

#[test]
fn wheel_in_the_agenda_moves_an_item() {
    let mut app = app_with(ITEMS);
    press(&mut app, KeyCode::Tab);
    app.handle_mouse(mouse_event(MouseEventKind::ScrollDown, 0, 0));
    assert_eq!(
        app.chosen().map(|entry| entry.item.title.as_str()),
        Some("c")
    );
    app.handle_mouse(mouse_event(MouseEventKind::ScrollUp, 0, 0));
    app.handle_mouse(mouse_event(MouseEventKind::ScrollUp, 0, 0));
    assert_eq!(
        app.chosen().map(|entry| entry.item.title.as_str()),
        Some("a")
    );
}

#[test]
fn clicking_an_item_moves_the_keyboard_there() {
    // 77 × 23, October at the top: the agenda's first row is 10/10 a (the inside of the agenda box starts
    // at x 43, y 3)
    let mut app = app_with(ITEMS);
    app.screen = Rect::new(0, 0, 77, 23);
    app.calendar_top = Some(0);
    app.handle_mouse(mouse_event(MouseEventKind::Down(MouseButton::Left), 47, 3));
    assert_eq!(app.focus, Focus::Agenda);
    assert_eq!(
        app.chosen().map(|entry| entry.item.title.as_str()),
        Some("a")
    );
    assert_eq!(app.selected, date(2026, 10, 10));
    // Then click a date on the calendar: the keyboard goes back to the calendar
    app.handle_mouse(mouse_event(MouseEventKind::Down(MouseButton::Left), 29, 5));
    assert_eq!(app.focus, Focus::Calendar);
    assert_eq!(app.selected, date(2026, 10, 16));
}

#[test]
fn t_goes_back_to_today() {
    let mut app = App::new(Lang::En, date(2026, 10, 17), Vec::new());
    app.selected = date(2027, 3, 1);
    app.calendar_top = Some(30);
    press(&mut app, KeyCode::Char('t'));
    assert_eq!(app.selected, date(2026, 10, 17));
    assert_eq!(app.calendar_top, None);
}

#[test]
fn groups_keep_file_order() {
    // Items before any heading are `None`; headings with the same name are one group; `##` is a group
    // too; a heading with no items under it yet is a calendar too (e.g. one just added)
    let mut app = app_with("");
    app.set_items(crate::tasks::parse(
        "- a\n# Work\n- b\n# No items\n## Personal\n- c\n# Work\n- d\n",
    ));
    let names: Vec<Option<&str>> = app
        .groups
        .iter()
        .map(|group| group.name.as_deref())
        .collect();
    assert_eq!(
        names,
        [None, Some("Work"), Some("No items"), Some("Personal")]
    );
    assert!(app.groups.iter().all(|group| group.shown));
    // d is under the second "# Work" and also counts as "Work"
    assert!(app.groups[1].includes(&app.items[3]));
    assert!(!app.groups[0].includes(&app.items[3]));
}

#[test]
fn calendars_nest_by_heading_level_and_list_as_a_tree() {
    // Work's second part (with B) comes after Personal in the file, but B is listed inside Work;
    // X inside Work and X inside Personal are two calendars
    let mut app = app_with("");
    app.set_items(crate::tasks::parse(
            "# Work\n## A\n- a @2026-10-10\n## X\n# Personal\n## X\n- p @2026-10-11\n# Work\n## B\n- b @2026-10-12\n",
        ));
    let paths: Vec<String> = app
        .groups
        .iter()
        .map(|group| group.path().join(" > "))
        .collect();
    assert_eq!(
        paths,
        [
            "Work",
            "Work > A",
            "Work > X",
            "Work > B",
            "Personal",
            "Personal > X"
        ]
    );
    // Only an item's own calendar counts: Work unchecked on its own (the calendars inside it stay checked)
    // hides only Work's own items, none here; A unchecked hides a
    let titles = |app: &App| -> Vec<String> {
        app.agenda()
            .iter()
            .map(|entry| entry.item.title.clone())
            .collect()
    };
    app.groups[0].shown = false;
    assert_eq!(titles(&app), ["a", "p", "b"]);
    app.groups[1].shown = false;
    assert_eq!(titles(&app), ["p", "b"]);
    // A checked again; B unchecked on its own hides only b
    app.groups[1].shown = true;
    app.groups[3].shown = false;
    assert_eq!(titles(&app), ["a", "p"]);
}

#[test]
fn unchecked_groups_leave_the_agenda() {
    // "Personal" unchecked: its b and c are not listed, and Tab into the agenda skips them too
    let mut app = app_with(
        "- a @2026-10-10\n# Personal\n- b @2026-10-17\n- c @2026-10-20\n# Work\n- d @2026-10-25\n",
    );
    app.groups[1].shown = false;
    let titles: Vec<&str> = app
        .agenda()
        .iter()
        .map(|entry| entry.item.title.as_str())
        .collect();
    assert_eq!(titles, ["a", "d"]);
    assert!(!app.is_shown(&app.items[1]));
    press(&mut app, KeyCode::Tab);
    assert_eq!(
        app.chosen().map(|entry| entry.item.title.as_str()),
        Some("d")
    );
    // Checked again: listed again
    app.groups[1].shown = true;
    assert_eq!(app.agenda().len(), 4);
}

#[test]
fn the_agenda_is_one_column_so_h_and_l_keep_the_choice() {
    // 107 wide, 11 items in October: one column however wide, one item per row (Item 9 is 6 rows below
    // Item 3); h, l and the arrows keep the chosen item
    let text: String = (1..=11)
        .map(|day| format!("- [ ] Item {day} @2026-10-{day:02}\n"))
        .collect();
    let mut app = app_with(&text);
    app.screen = Rect::new(0, 0, 107, 30);
    app.focus = Focus::Agenda;
    let mut terminal =
        ratatui::Terminal::new(ratatui::backend::TestBackend::new(107, 30)).expect("terminal");
    terminal
        .draw(|frame| crate::ui::render(frame, &mut app))
        .expect("draw");
    let buffer = terminal.backend().buffer();
    let row_of = |text: &str| {
        (0..30).find(|&y| {
            let row: String = (0..107).map(|x| buffer[(x, y)].symbol()).collect();
            row.contains(text)
        })
    };
    let row = row_of("Item 3 ").expect("Item 3 on screen");
    assert_eq!(row_of("Item 9 "), Some(row + 6));
    app.choose(2);
    let title = |app: &App| app.chosen().map(|entry| entry.item.title.clone());
    for code in [
        KeyCode::Char('l'),
        KeyCode::Right,
        KeyCode::Char('h'),
        KeyCode::Left,
    ] {
        press(&mut app, code);
        assert_eq!(title(&app).as_deref(), Some("Item 3"), "{code:?}");
    }
}

#[test]
fn hide_done_hides_done_tasks_but_not_their_open_subtasks() {
    // Only the done task itself is hidden from the agenda and the day's items: its open subtask stays, and
    // the day's open count matches what is listed; the tasks page keeps the done task too, for the tree
    // (collapsed: the subtask is not a row); open tasks and events stay, past events too
    let mut app = app_with(
        "- [x] Ship v1 @2026-10-16\n  - [ ] Write the blog post @2026-10-17\n- [ ] Pack @2026-10-17\n- Farmers market @2026-10-10\n",
    );
    let titles = |app: &App| -> Vec<String> {
        app.agenda()
            .iter()
            .map(|entry| entry.item.title.clone())
            .collect()
    };
    assert_eq!(titles(&app).len(), 4);
    app.settings.calendar_list.hide_done = true;
    assert_eq!(
        titles(&app),
        ["Farmers market", "Write the blog post", "Pack"]
    );
    assert_eq!(app.items_on(date(2026, 10, 17)).len(), 2);
    assert_eq!(app.day_counts(date(2026, 10, 17)), (2, 0));
    app.view = View::Tasks;
    let items: Vec<usize> = app
        .tasks_rows()
        .iter()
        .filter_map(|row| match row {
            TaskRow::Item { index, .. } => Some(*index),
            TaskRow::Heading { .. } => None,
        })
        .collect();
    assert_eq!(items, [0, 2, 3]);
    // Off again: back
    app.settings.calendar_list.hide_done = false;
    assert_eq!(titles(&app).len(), 4);
}

#[test]
fn dot_hides_done_tasks_from_anywhere_and_keeps_the_choice() {
    let chosen = |app: &App| app.chosen_item().map(|item| item.title.clone());
    // Agenda on 10/17: Ship v1 (done), Pack, then Call mom on 10/18; Ship v1 chosen
    let mut app =
        app_with("- [x] Ship v1 @2026-10-17\n- [ ] Pack @2026-10-17\n- [ ] Call mom @2026-10-18\n");
    let dir = crate::files::test_dir("dot-hide-done");
    let path = dir.join("config.toml");
    app.config_path = Some(path.clone());
    app.focus = Focus::Agenda;
    app.agenda_index = 0;
    // `.`: Ship v1 is hidden, the choice stays at the same position (Pack); written to the config file
    // right away
    press(&mut app, KeyCode::Char('.'));
    assert!(app.settings.calendar_list.hide_done);
    assert_eq!(chosen(&app).as_deref(), Some("Pack"));
    assert_eq!(
        std::fs::read_to_string(&path).expect("read"),
        "[calendar_list]\nhide_done = true\n"
    );
    // Again: shown, Pack stays chosen (now second)
    press(&mut app, KeyCode::Char('.'));
    assert!(!app.settings.calendar_list.hide_done);
    assert_eq!(
        (app.agenda_index, chosen(&app).as_deref()),
        (1, Some("Pack"))
    );
    app.config_path = None;
    let _ = std::fs::remove_dir_all(&dir);
    // The last item hidden: the new last one is chosen
    let mut app = app_with("- [ ] Pack @2026-10-17\n- [x] Ship v1 @2026-10-18\n");
    app.focus = Focus::Agenda;
    app.agenda_index = 1;
    press(&mut app, KeyCode::Char('.'));
    assert_eq!(
        (app.agenda_index, chosen(&app).as_deref()),
        (0, Some("Pack"))
    );
    // Layout 2's popup with only done tasks: nothing left, it closes
    let mut app = app_with("- [x] Ship v1 @2026-10-17\n");
    app.view = View::Month;
    app.focus = Focus::Day;
    press(&mut app, KeyCode::Char('.'));
    assert_eq!(app.focus, Focus::Calendar);
}

/// An app 120 cells wide (with a side menu) and `text` as the task file
fn wide_app_with(text: &str) -> App {
    let mut app = app_with(text);
    app.screen = Rect::new(0, 0, 120, 30);
    app
}

/// Three groups: ungrouped (a), Personal (b), Work (c)
const GROUPS: &str = "- a @2026-10-10\n# Personal\n- b @2026-10-17\n# Work\n- c @2026-10-20\n";

#[test]
fn tab_goes_through_the_side_menu_when_wide() {
    // Calendar → agenda → side menu → calendar
    let mut app = wide_app_with(GROUPS);
    for focus in [Focus::Agenda, Focus::Side, Focus::Calendar] {
        press(&mut app, KeyCode::Tab);
        assert_eq!(app.focus, focus);
    }
    // No items in the agenda: the calendar goes straight to the side menu
    let mut app = wide_app_with("- No date\n");
    press(&mut app, KeyCode::Tab);
    assert_eq!(app.focus, Focus::Side);
    // Hidden: same as when narrow, calendar ⇄ agenda
    let mut app = wide_app_with(GROUPS);
    app.side_hidden = true;
    press(&mut app, KeyCode::Tab);
    press(&mut app, KeyCode::Tab);
    assert_eq!(app.focus, Focus::Calendar);
}

#[test]
fn side_menu_keys_choose_and_check_calendars() {
    let mut app = wide_app_with(GROUPS);
    app.focus = Focus::Side;
    // j to "Personal", Space unchecks it: b is no longer listed
    press(&mut app, KeyCode::Char('j'));
    assert_eq!(app.side_row, SideRow::Calendar(1));
    press(&mut app, KeyCode::Char(' '));
    assert!(!app.groups[1].shown);
    assert_eq!(app.agenda().len(), 2);
    // Enter checks it again
    press(&mut app, KeyCode::Enter);
    assert!(app.groups[1].shown);
    // Down past the last calendar: stays there; up past the first one: the switch rows above the list,
    // 「隱藏做完的」 and then 「隱藏全部」, where it stops; Space switches 「隱藏做完的」; arrow keys and the
    // wheel do the same
    for _ in 0..3 {
        press(&mut app, KeyCode::Down);
    }
    assert_eq!(app.side_row, SideRow::Calendar(2));
    for _ in 0..3 {
        press(&mut app, KeyCode::Up);
    }
    assert_eq!(app.side_row, SideRow::HideDone);
    // Switching it writes that one line to the config file right away
    let dir = crate::files::test_dir("side-hide-done");
    let path = dir.join("config.toml");
    app.config_path = Some(path.clone());
    press(&mut app, KeyCode::Char(' '));
    assert!(app.settings.calendar_list.hide_done);
    assert_eq!(
        std::fs::read_to_string(&path).expect("read"),
        "[calendar_list]\nhide_done = true\n"
    );
    press(&mut app, KeyCode::Char(' '));
    assert!(!app.settings.calendar_list.hide_done);
    assert!(
        std::fs::read_to_string(&path)
            .expect("read")
            .contains("hide_done = false")
    );
    app.config_path = None;
    let _ = std::fs::remove_dir_all(&dir);
    app.handle_mouse(mouse_event(MouseEventKind::ScrollUp, 0, 0));
    assert_eq!(app.side_row, SideRow::HideAll);
    press(&mut app, KeyCode::Char('k'));
    assert_eq!(app.side_row, SideRow::HideAll);
    app.handle_mouse(mouse_event(MouseEventKind::ScrollDown, 0, 0));
    press(&mut app, KeyCode::Char('j'));
    assert_eq!(app.side_row, SideRow::Calendar(0));
    // h / l do not move the selected day; Esc goes back to the calendar without quitting
    press(&mut app, KeyCode::Char('l'));
    assert_eq!(app.selected, date(2026, 10, 17));
    press(&mut app, KeyCode::Esc);
    assert_eq!(app.focus, Focus::Calendar);
    assert!(!app.should_quit);
}

#[test]
fn gg_and_capital_g_jump_to_the_first_and_last_row() {
    let mut app = wide_app_with(GROUPS);
    app.focus = Focus::Side;
    // G: the last row, the last of the 3 calendars (index 2)
    press(&mut app, KeyCode::Char('G'));
    assert_eq!(app.side_row, SideRow::Calendar(2));
    // One g only waits; the second one jumps to the first row, the 「隱藏全部」 switch above the
    // calendars
    press(&mut app, KeyCode::Char('g'));
    assert_eq!(app.side_row, SideRow::Calendar(2));
    press(&mut app, KeyCode::Char('g'));
    assert_eq!(app.side_row, SideRow::HideAll);
    // g, another key, g: not gg (the other key works as usual)
    press(&mut app, KeyCode::Char('G'));
    press(&mut app, KeyCode::Char('g'));
    press(&mut app, KeyCode::Char('k'));
    press(&mut app, KeyCode::Char('g'));
    assert_eq!(app.side_row, SideRow::Calendar(1));
    // The wheel between the two g's: not gg either (the last g above is still waiting)
    app.handle_mouse(mouse_event(MouseEventKind::ScrollDown, 0, 0));
    press(&mut app, KeyCode::Char('g'));
    assert_eq!(app.side_row, SideRow::Calendar(2));
}

#[test]
fn reading_the_task_file_again_keeps_a_switch_row_chosen() {
    // 「隱藏全部」 chosen; the calendars go away (an empty task file) and come back: still chosen
    let mut app = wide_app_with(GROUPS);
    app.focus = Focus::Side;
    app.side_row = SideRow::HideAll;
    app.set_items(crate::tasks::parse(""));
    assert_eq!(app.chosen_side_row(), SideRow::HideAll);
    app.set_items(crate::tasks::parse(GROUPS));
    assert_eq!(app.chosen_side_row(), SideRow::HideAll);
    // A calendar chosen and no calendars left: 「隱藏做完的」, the last row then
    app.side_row = SideRow::Calendar(2);
    app.set_items(crate::tasks::parse(""));
    assert_eq!(app.chosen_side_row(), SideRow::HideDone);
}

#[test]
fn reading_the_task_file_again_keeps_the_chosen_calendar_chosen() {
    let chosen = |app: &App| app.chosen_group().and_then(|group| group.name.clone());
    // Work chosen: the third (ungrouped, Personal, Work)
    let mut app = wide_app_with(GROUPS);
    app.focus = Focus::Side;
    app.side_row = SideRow::Calendar(2);
    // Inbox added above it elsewhere: Work is the fourth now, and still chosen
    let ungrouped = "- a @2026-10-10\n";
    let inbox = "# Inbox\n- i @2026-10-11\n";
    let personal = "# Personal\n- b @2026-10-17\n";
    let work = "# Work\n- c @2026-10-20\n";
    app.set_items(crate::tasks::parse(&format!(
        "{ungrouped}{inbox}{personal}{work}"
    )));
    assert_eq!(app.side_row, SideRow::Calendar(3));
    assert_eq!(chosen(&app).as_deref(), Some("Work"));
    // Personal taken out: the third again
    app.set_items(crate::tasks::parse(&format!("{ungrouped}{inbox}{work}")));
    assert_eq!(app.side_row, SideRow::Calendar(2));
    // Work itself taken out: the one in its place, here the last one (Inbox)
    app.set_items(crate::tasks::parse(&format!("{ungrouped}{inbox}")));
    assert_eq!(chosen(&app).as_deref(), Some("Inbox"));
    // A calendar of the same name inside another one is not the same calendar: Work > Notes chosen,
    // Notes (top level) added above it; Work > Notes stays chosen
    app.set_items(crate::tasks::parse("# Work\n## Notes\n- n\n"));
    app.side_row = SideRow::Calendar(1);
    app.set_items(crate::tasks::parse("# Notes\n- x\n# Work\n## Notes\n- n\n"));
    assert_eq!(
        app.chosen_group().map(crate::app::Group::path),
        Some(vec!["Work".to_string(), "Notes".to_string()])
    );
}

#[test]
fn hide_all_unchecks_every_calendar_then_checks_them_all_again() {
    let mut app = wide_app_with(GROUPS);
    app.focus = Focus::Side;
    // gg to 「隱藏全部」, Space: every calendar unchecked, nothing left in the agenda
    press(&mut app, KeyCode::Char('g'));
    press(&mut app, KeyCode::Char('g'));
    press(&mut app, KeyCode::Char(' '));
    assert!(app.groups.iter().all(|group| !group.shown));
    assert!(app.all_hidden());
    assert_eq!(app.agenda().len(), 0);
    // Space again: they are all unchecked, so all checked again
    press(&mut app, KeyCode::Char(' '));
    assert!(app.groups.iter().all(|group| group.shown));
    assert!(!app.all_hidden());
    assert_eq!(app.agenda().len(), 3);
    // Hide all, then check one by hand: 「隱藏全部」 is off again, so Space hides all again (not shows)
    press(&mut app, KeyCode::Char(' '));
    press(&mut app, KeyCode::Char('j'));
    press(&mut app, KeyCode::Char('j'));
    press(&mut app, KeyCode::Char(' '));
    assert!(app.groups[0].shown);
    assert!(!app.all_hidden());
    press(&mut app, KeyCode::Char('g'));
    press(&mut app, KeyCode::Char('g'));
    press(&mut app, KeyCode::Char(' '));
    assert!(app.groups.iter().all(|group| !group.shown));
}

#[test]
fn checking_a_calendar_checks_the_calendars_inside_it_too() {
    // The list: Work, Work > SFAP, Work > SFAP > Bugs, Personal
    let mut app = wide_app_with(
        "# Work\n- a @2026-10-10\n## SFAP\n- b @2026-10-11\n### Bugs\n- c @2026-10-12\n# Personal\n- d @2026-10-13\n",
    );
    app.focus = Focus::Side;
    let shown = |app: &App| {
        app.groups
            .iter()
            .map(|group| group.shown)
            .collect::<Vec<_>>()
    };
    // Space on Work: it and everything inside it unchecked; Personal untouched
    press(&mut app, KeyCode::Char(' '));
    assert_eq!(shown(&app), [false, false, false, true]);
    // SFAP checked by itself: it and Bugs inside it; Work (around it) stays unchecked
    press(&mut app, KeyCode::Char('j'));
    press(&mut app, KeyCode::Char(' '));
    assert_eq!(shown(&app), [false, true, true, true]);
    // Space on Work again: checked, and so is everything inside it
    press(&mut app, KeyCode::Char('k'));
    press(&mut app, KeyCode::Char(' '));
    assert_eq!(shown(&app), [true, true, true, true]);
    // Hide all, then check Work (the first calendar, below the two switches): Work's calendars come back,
    // Personal stays hidden
    press(&mut app, KeyCode::Char('g'));
    press(&mut app, KeyCode::Char('g'));
    press(&mut app, KeyCode::Char(' '));
    assert_eq!(shown(&app), [false, false, false, false]);
    press(&mut app, KeyCode::Char('j'));
    press(&mut app, KeyCode::Char('j'));
    press(&mut app, KeyCode::Char(' '));
    assert_eq!(shown(&app), [true, true, true, false]);
}

#[test]
fn brackets_in_the_side_menu_jump_to_the_previous_or_next_top_level_calendar() {
    // The list: 0 Personal, 1 Work, 2 Work > SFAP, 3 Work > SFAP > Bugs, 4 Travel
    let mut app = wide_app_with(
        "# Personal\n- a @2026-10-10\n# Work\n- b @2026-10-11\n## SFAP\n- c @2026-10-12\n### Bugs\n- d @2026-10-13\n# Travel\n- e @2026-10-14\n",
    );
    app.focus = Focus::Side;
    // ] from Personal: Work, then Travel (skipping the two calendars inside Work); no calendar after
    // Travel: stays
    press(&mut app, KeyCode::Char(']'));
    assert_eq!(app.side_row, SideRow::Calendar(1));
    press(&mut app, KeyCode::Char(']'));
    assert_eq!(app.side_row, SideRow::Calendar(4));
    press(&mut app, KeyCode::Char(']'));
    assert_eq!(app.side_row, SideRow::Calendar(4));
    // [ back to Work, then Personal, then past the first calendar up to the 「隱藏全部」 switch (the top
    // row), where it stays
    press(&mut app, KeyCode::Char('['));
    assert_eq!(app.side_row, SideRow::Calendar(1));
    press(&mut app, KeyCode::Char('['));
    assert_eq!(app.side_row, SideRow::Calendar(0));
    press(&mut app, KeyCode::Char('['));
    assert_eq!(app.side_row, SideRow::HideAll);
    press(&mut app, KeyCode::Char('['));
    assert_eq!(app.side_row, SideRow::HideAll);
    // From Bugs (inside Work): ] = Travel, [ = the one before Work (Personal), like on the tasks page
    app.side_row = SideRow::Calendar(3);
    press(&mut app, KeyCode::Char(']'));
    assert_eq!(app.side_row, SideRow::Calendar(4));
    app.side_row = SideRow::Calendar(3);
    press(&mut app, KeyCode::Char('['));
    assert_eq!(app.side_row, SideRow::Calendar(0));
    // On 「隱藏做完的」: [ = 「隱藏全部」 above it, ] = the first calendar
    app.side_row = SideRow::HideDone;
    press(&mut app, KeyCode::Char('['));
    assert_eq!(app.side_row, SideRow::HideAll);
    app.side_row = SideRow::HideDone;
    press(&mut app, KeyCode::Char(']'));
    assert_eq!(app.side_row, SideRow::Calendar(0));
    // In the side menu they are not months: the selected day did not move
    assert_eq!(app.selected, date(2026, 10, 17));
    // Same on the tasks page
    app.view = View::Tasks;
    press(&mut app, KeyCode::Char(']'));
    assert_eq!(app.side_row, SideRow::Calendar(1));
}

#[test]
fn the_keyboard_leaves_the_side_menu_when_it_is_hidden_but_not_when_narrowed() {
    // Hide it with `s`
    let mut app = wide_app_with(GROUPS);
    app.focus = Focus::Side;
    press(&mut app, KeyCode::Char('s'));
    assert_eq!(app.focus, Focus::Calendar);
    // The terminal gets narrow: the keyboard stays in the side menu, which now shows over the left side
    let mut app = wide_app_with(GROUPS);
    app.focus = Focus::Side;
    app.resize(Rect::new(0, 0, 120, 20));
    assert_eq!(app.focus, Focus::Side);
    app.resize(Rect::new(0, 0, 119, 20));
    assert_eq!(app.focus, Focus::Side);
    assert!(ui::side_overlay(&app).is_some());
}

#[test]
fn clicking_a_calendar_selects_it_then_toggles_it() {
    // 120 × 30: inside the side menu, 「隱藏全部」 at y 3, 「隱藏做完的」 y 4, a rule, then one calendar per
    // row from y 6 (screen in ui/tests.rs's `side_menu_lists_the_calendars_when_wide`)
    let mut app = wide_app_with(GROUPS);
    let click = |app: &mut App, row| {
        app.handle_mouse(mouse_event(MouseEventKind::Down(MouseButton::Left), 5, row));
    };
    // First click: chooses "Personal", the keyboard moves to the side menu, not toggled yet
    click(&mut app, 7);
    assert_eq!(
        (app.focus, app.side_row),
        (Focus::Side, SideRow::Calendar(1))
    );
    assert!(app.groups[1].shown);
    // Click again: unchecks it; once more: checks it again
    click(&mut app, 7);
    assert!(!app.groups[1].shown);
    click(&mut app, 7);
    assert!(app.groups[1].shown);
    // Click another calendar: only chooses, does not toggle
    click(&mut app, 8);
    assert_eq!(app.side_row, SideRow::Calendar(2));
    assert!(app.groups[2].shown);
    // When the keyboard is not in the side menu, clicking the chosen one also just chooses it
    // (back to the side menu)
    app.focus = Focus::Calendar;
    click(&mut app, 8);
    assert_eq!(app.focus, Focus::Side);
    assert!(app.groups[2].shown);
    // 「隱藏全部」 switches on the first click (like 「隱藏做完的」) and gets chosen: all unchecked; again:
    // all checked
    click(&mut app, 3);
    assert_eq!(app.side_row, SideRow::HideAll);
    assert!(app.all_hidden());
    click(&mut app, 3);
    assert!(app.groups.iter().all(|group| group.shown));
}

#[test]
fn s_hides_the_side_menu_when_wide_and_calls_it_up_when_narrow() {
    let mut app = app_with(GROUPS);
    // Narrow: s calls it up (the keyboard moves to the side menu), and it works like when wide; s again
    // hides it, and so does Esc
    app.screen = Rect::new(0, 0, 119, 30);
    press(&mut app, KeyCode::Char('s'));
    assert_eq!(app.focus, Focus::Side);
    press(&mut app, KeyCode::Char('j'));
    press(&mut app, KeyCode::Char(' '));
    assert!(!app.groups[1].shown);
    press(&mut app, KeyCode::Char('s'));
    assert_eq!(app.focus, Focus::Calendar);
    press(&mut app, KeyCode::Char('s'));
    press(&mut app, KeyCode::Esc);
    assert_eq!(app.focus, Focus::Calendar);
    assert!(!app.side_hidden);
    // Wide enough: hides it, pressing again shows it
    app.screen = Rect::new(0, 0, 120, 30);
    press(&mut app, KeyCode::Char('s'));
    assert!(app.side_hidden);
    press(&mut app, KeyCode::Char('s'));
    assert!(!app.side_hidden);
}

#[test]
fn putting_the_narrow_side_menu_away_goes_back_to_the_pane_before() {
    // 60 cells wide, keyboard in the agenda with c (3rd item) chosen: s calls up the side menu; hiding it
    // with s / Esc / a click outside / q always goes back to the agenda with c still chosen
    // (q only hides it, does not quit)
    let mut app = app_with(ITEMS);
    app.screen = Rect::new(0, 0, 60, 20);
    app.choose(2);
    let left = MouseEventKind::Down(MouseButton::Left);
    for close in 0..4 {
        press(&mut app, KeyCode::Char('s'));
        assert_eq!(app.focus, Focus::Side);
        match close {
            0 => press(&mut app, KeyCode::Char('s')),
            1 => press(&mut app, KeyCode::Esc),
            2 => app.handle_mouse(mouse_event(left, 50, 10)),
            _ => press(&mut app, KeyCode::Char('q')),
        }
        assert_eq!((app.focus, app.agenda_index), (Focus::Agenda, 2), "{close}");
        assert!(!app.should_quit);
    }
    // Layout 2, the 10/17 popup with the 2nd item (c) chosen: call it up and hide it; the popup comes back
    // with c still chosen
    press(&mut app, KeyCode::Char('2'));
    press(&mut app, KeyCode::Enter);
    press(&mut app, KeyCode::Char('j'));
    press(&mut app, KeyCode::Char('s'));
    press(&mut app, KeyCode::Char('s'));
    assert_eq!((app.focus, app.day_index), (Focus::Day, 1));
    // Uncheck the only calendar while it is up: the day has no items left, so hiding it goes back to
    // the calendar
    press(&mut app, KeyCode::Char('s'));
    press(&mut app, KeyCode::Char(' '));
    press(&mut app, KeyCode::Char('s'));
    assert_eq!(app.focus, Focus::Calendar);
}

#[test]
fn the_narrow_side_menu_takes_clicks_and_a_click_outside_puts_it_away() {
    // 60 × 20: s calls it up over the left side (after the border, header, line, the two switches and
    // their rule, one calendar per row: ungrouped y 6, Personal y 7)
    let mut app = app_with(GROUPS);
    app.screen = Rect::new(0, 0, 60, 20);
    press(&mut app, KeyCode::Char('s'));
    let area = ui::side_overlay(&app).expect("overlay");
    assert_eq!(area, Rect::new(0, 0, 22, 19));
    // Click Personal: chooses it; click again: unchecks it
    let left = MouseEventKind::Down(MouseButton::Left);
    app.handle_mouse(mouse_event(left, 3, 7));
    assert_eq!(
        (app.focus, app.side_row),
        (Focus::Side, SideRow::Calendar(1))
    );
    app.handle_mouse(mouse_event(left, 3, 7));
    assert!(!app.groups[1].shown);
    // Click outside the box (10/16 on the calendar; with October at the top, it is at x 28-31, y 5):
    // only hides it, the selected day stays
    app.calendar_top = Some(0);
    let selected = app.selected;
    assert_eq!(
        ui::click_at(&app, 29, 5),
        Some(Click::Day(date(2026, 10, 16)))
    );
    app.handle_mouse(mouse_event(left, 29, 5));
    assert_eq!((app.focus, app.selected), (Focus::Calendar, selected));
    assert_eq!(ui::side_overlay(&app), None);
}

#[test]
fn widening_keeps_the_side_menu_unless_it_was_hidden() {
    // Called up when narrow, then the terminal is widened: side menu at the far left, keyboard still on it
    let mut app = app_with(GROUPS);
    app.resize(Rect::new(0, 0, 60, 20));
    press(&mut app, KeyCode::Char('s'));
    app.resize(Rect::new(0, 0, 120, 20));
    assert_eq!(app.focus, Focus::Side);
    // Hidden earlier while wide (side_hidden): after widening it is not shown, and the keyboard goes back
    // to the calendar
    let mut app = app_with(GROUPS);
    app.side_hidden = true;
    app.resize(Rect::new(0, 0, 60, 20));
    press(&mut app, KeyCode::Char('s'));
    assert_eq!(app.focus, Focus::Side);
    app.resize(Rect::new(0, 0, 120, 20));
    assert_eq!(app.focus, Focus::Calendar);
}

#[test]
fn dragging_the_side_menus_right_border_fixes_its_width_and_letting_go_saves_it() {
    // 130 × 20: the side menu is 22 wide (auto: the names are short), its right border at x 21, the
    // calendar box's left border right next to it at x 22
    let mut app = app_with(GROUPS);
    app.screen = Rect::new(0, 0, 130, 20);
    let dir = crate::files::test_dir("drag-side-border");
    let path = dir.join("config.toml");
    app.config_path = Some(path.clone());
    let left = MouseButton::Left;
    let read = |path: &std::path::Path| std::fs::read_to_string(path).expect("read");
    // Grab the calendar box's border, drag 5 cells right: 27 wide; nothing saved while dragging
    app.handle_mouse(mouse_event(MouseEventKind::Down(left), 22, 5));
    app.handle_mouse(mouse_event(MouseEventKind::Drag(left), 27, 5));
    assert_eq!(app.settings.calendar_list.width, Some(27));
    assert!(!path.exists());
    // 2 cells back (on another row): 25; letting go saves it
    app.handle_mouse(mouse_event(MouseEventKind::Drag(left), 25, 9));
    app.handle_mouse(mouse_event(MouseEventKind::Up(left), 25, 9));
    assert_eq!(app.settings.calendar_list.width, Some(25));
    assert_eq!(read(&path), "[calendar_list]\nwidth = 25\n");
    // Its own border is at x 24 now: dragged far left / right, it stops at 22 / 60
    app.handle_mouse(mouse_event(MouseEventKind::Down(left), 24, 5));
    app.handle_mouse(mouse_event(MouseEventKind::Drag(left), 0, 5));
    assert_eq!(app.settings.calendar_list.width, Some(22));
    app.handle_mouse(mouse_event(MouseEventKind::Drag(left), 125, 5));
    app.handle_mouse(mouse_event(MouseEventKind::Up(left), 125, 5));
    assert_eq!(read(&path), "[calendar_list]\nwidth = 60\n");
    // Dragging anywhere else leaves the width alone
    app.handle_mouse(mouse_event(MouseEventKind::Down(left), 90, 5));
    app.handle_mouse(mouse_event(MouseEventKind::Drag(left), 100, 5));
    app.handle_mouse(mouse_event(MouseEventKind::Up(left), 100, 5));
    assert_eq!(app.settings.calendar_list.width, Some(60));
    let _ = std::fs::remove_dir_all(&dir);
    // Narrow (60 × 20), called up with s over the left side: its own border (x 21) can be grabbed; right
    // next to it (x 22) is outside it, and a click there puts it away
    let mut app = app_with(GROUPS);
    app.screen = Rect::new(0, 0, 60, 20);
    press(&mut app, KeyCode::Char('s'));
    app.handle_mouse(mouse_event(MouseEventKind::Down(left), 21, 5));
    app.handle_mouse(mouse_event(MouseEventKind::Drag(left), 30, 5));
    app.handle_mouse(mouse_event(MouseEventKind::Up(left), 30, 5));
    assert_eq!(app.settings.calendar_list.width, Some(31));
    assert_eq!(ui::side_overlay(&app).map(|area| area.width), Some(31));
    app.handle_mouse(mouse_event(MouseEventKind::Down(left), 31, 5));
    assert_eq!(ui::side_overlay(&app), None);
    assert_eq!(app.settings.calendar_list.width, Some(31));
}

#[test]
fn double_clicking_the_side_menus_right_border_goes_back_to_the_auto_width() {
    // 130 × 20, fixed at 30 (auto would be 22: the names are short), right border at x 29
    let mut app = app_with(GROUPS);
    app.screen = Rect::new(0, 0, 130, 20);
    app.settings.calendar_list.width = Some(30);
    let dir = crate::files::test_dir("double-click-side-border");
    std::fs::create_dir_all(&dir).expect("dir");
    let path = dir.join("config.toml");
    std::fs::write(&path, "[calendar_list]\nwidth = 30\n").expect("write");
    app.config_path = Some(path.clone());
    let left = MouseButton::Left;
    let click = |app: &mut App, column| {
        app.handle_mouse(mouse_event(MouseEventKind::Down(left), column, 5));
        app.handle_mouse(mouse_event(MouseEventKind::Up(left), column, 5));
    };
    // Double-click: auto again, saved right away (the config file had only that line)
    click(&mut app, 29);
    assert_eq!(app.settings.calendar_list.width, Some(30));
    click(&mut app, 29);
    assert_eq!(app.settings.calendar_list.width, None);
    assert_eq!(std::fs::read_to_string(&path).expect("read"), "");
    let _ = std::fs::remove_dir_all(&dir);
    // Dragged 2 cells (22 → 24), then clicked right away: not a double click, it stays 24
    app.handle_mouse(mouse_event(MouseEventKind::Down(left), 21, 5));
    app.handle_mouse(mouse_event(MouseEventKind::Drag(left), 23, 5));
    app.handle_mouse(mouse_event(MouseEventKind::Up(left), 23, 5));
    click(&mut app, 23);
    assert_eq!(app.settings.calendar_list.width, Some(24));
    // Two clicks a second apart: not a double click either
    let start = Instant::now();
    app.press_side_border(23, start + Duration::from_secs(2));
    app.handle_mouse(mouse_event(MouseEventKind::Up(left), 23, 5));
    app.press_side_border(23, start + Duration::from_secs(3));
    app.handle_mouse(mouse_event(MouseEventKind::Up(left), 23, 5));
    assert_eq!(app.settings.calendar_list.width, Some(24));
}

#[test]
fn a_multi_day_item_is_listed_in_each_month_and_chosen_from_any_of_its_days() {
    // Manager OOO 10/28-11/3: one row in October and one in November; the November row is listed on
    // 11/1, before 11/1's "Write the monthly report"
    let mut app = app_with(
        "- [ ] Write the monthly report @2026-11-01\n- Manager OOO @2026-10-28..2026-11-03\n- [ ] Send the weekly update @2026-10-30\n",
    );
    let rows: Vec<(&str, Date)> = app
        .agenda()
        .iter()
        .map(|entry| (entry.item.title.as_str(), entry.date))
        .collect();
    assert_eq!(
        rows,
        [
            ("Manager OOO", date(2026, 10, 28)),
            ("Send the weekly update", date(2026, 10, 30)),
            ("Manager OOO", date(2026, 11, 1)),
            ("Write the monthly report", date(2026, 11, 1)),
        ]
    );
    // Select 11/2 (inside the OOO) and press Tab: chooses the November row; the selected day stays
    app.selected = date(2026, 11, 2);
    press(&mut app, KeyCode::Tab);
    assert_eq!(app.agenda_index, 2);
    assert_eq!(app.selected, date(2026, 11, 2));
    // j: "Write the monthly report", the calendar jumps to 11/1; k: back to the OOO; 11/1 is also
    // one of its days, so the calendar stays
    press(&mut app, KeyCode::Char('j'));
    assert_eq!(app.selected, date(2026, 11, 1));
    press(&mut app, KeyCode::Char('k'));
    assert_eq!((app.agenda_index, app.selected), (2, date(2026, 11, 1)));
    // Up again: "Send the weekly update" (10/30) → the OOO's October row (10/30 is also one of its
    // days, so the calendar stays)
    press(&mut app, KeyCode::Char('k'));
    press(&mut app, KeyCode::Char('k'));
    assert_eq!((app.agenda_index, app.selected), (0, date(2026, 10, 30)));
    // Tab on 10/29 (inside the OOO, in October): chooses the October row, ahead of 10/30's
    // "Send the weekly update"
    let mut app = app_with(
        "- [ ] Send the weekly update @2026-10-30\n- Manager OOO @2026-10-28..2026-11-03\n",
    );
    app.selected = date(2026, 10, 29);
    press(&mut app, KeyCode::Tab);
    assert_eq!(
        app.chosen().map(|entry| entry.date),
        Some(date(2026, 10, 28))
    );
    assert_eq!(app.selected, date(2026, 10, 29));
}

#[test]
fn enter_on_the_calendar_goes_into_that_days_items() {
    // 10/17 has b and c: Enter in layout 1 → the day popup opens with b chosen, drawn over the calendar
    // and the agenda (its top border names the day)
    let mut app = app_with(ITEMS);
    press(&mut app, KeyCode::Enter);
    assert_eq!((app.view, app.focus), (View::Planner, Focus::Day));
    assert_eq!(app.chosen_item().map(|item| item.title.as_str()), Some("b"));
    app.screen = Rect::new(0, 0, 70, 23);
    let mut terminal =
        ratatui::Terminal::new(ratatui::backend::TestBackend::new(70, 23)).expect("terminal");
    terminal
        .draw(|frame| crate::ui::render(frame, &mut app))
        .expect("draw");
    let popup = crate::ui::day_area(&app);
    let top: String = (popup.left()..popup.right())
        .map(|x| terminal.backend().buffer()[(x, popup.y)].symbol())
        .collect();
    assert!(top.contains("Sat Oct 17, 2026"), "{top}");
    // A day with no items (10/18): nothing happens
    let mut app = app_with(ITEMS);
    app.selected = date(2026, 10, 18);
    press(&mut app, KeyCode::Enter);
    assert_eq!(app.focus, Focus::Calendar);
    // Layout 2: the layout stays; the day popup opens with the first item (b) chosen
    let mut app = app_with(ITEMS);
    app.view = View::Month;
    press(&mut app, KeyCode::Enter);
    assert_eq!(
        (app.view, app.focus, app.day_index),
        (View::Month, Focus::Day, 0)
    );
    // With the mouse it takes a double click (70 × 23, October at the top: 10/17 is at x 32-35, y 5):
    // one click on the selected day only selects it, a second one right away opens the popup
    let mut app = app_with(ITEMS);
    app.screen = Rect::new(0, 0, 70, 23);
    app.calendar_top = Some(0);
    app.handle_mouse(mouse_event(MouseEventKind::Down(MouseButton::Left), 33, 5));
    assert_eq!(app.focus, Focus::Calendar);
    app.handle_mouse(mouse_event(MouseEventKind::Down(MouseButton::Left), 33, 5));
    assert_eq!(app.focus, Focus::Day);
}

#[test]
fn only_a_double_click_goes_into_the_day_in_layout_1() {
    let mut app = app_with(ITEMS);
    let start = Instant::now();
    let at = |ms: u64| start + Duration::from_millis(ms);
    let (oct16, oct17) = (date(2026, 10, 16), date(2026, 10, 17));
    // The selected day clicked again 0.9 s later: only selected, the keyboard stays on the calendar
    app.click_calendar_day(oct17, at(0));
    app.click_calendar_day(oct17, at(900));
    assert_eq!(app.focus, Focus::Calendar);
    // Another click 0.3 s after that one: a double click, the day popup with that day's first item (b)
    app.click_calendar_day(oct17, at(1200));
    assert_eq!(
        (app.focus, app.chosen_item().map(|item| item.title.as_str())),
        (Focus::Day, Some("b"))
    );
    // Two quick clicks on different days: the second day is selected, the keyboard back on the calendar
    app.click_calendar_day(oct16, at(2000));
    app.click_calendar_day(oct17, at(2200));
    assert_eq!((app.selected, app.focus), (oct17, Focus::Calendar));
}

#[test]
fn number_keys_switch_layouts() {
    // 2 switches to the month grid, 1 switches back; switching scrolls layout 1's calendar to the selected
    // month and puts the selected week (Sat 10/17: from Mon 10/12) on layout 2's row 2 (10/5 on top)
    let mut app = app_with(ITEMS);
    app.calendar_top = Some(5);
    app.month_top = Some(date(2026, 1, 5));
    press(&mut app, KeyCode::Char('2'));
    assert_eq!(app.view, View::Month);
    assert_eq!(
        (app.calendar_top, app.month_top),
        (None, Some(date(2026, 10, 5)))
    );
    press(&mut app, KeyCode::Char('1'));
    assert_eq!(app.view, View::Planner);
    // Switching to the month grid (no agenda) with the keyboard in the agenda: back to the calendar;
    // the side menu is not affected
    press(&mut app, KeyCode::Tab);
    assert_eq!(app.focus, Focus::Agenda);
    press(&mut app, KeyCode::Char('2'));
    assert_eq!(app.focus, Focus::Calendar);
    let mut app = wide_app_with(GROUPS);
    app.focus = Focus::Side;
    press(&mut app, KeyCode::Char('2'));
    assert_eq!(app.focus, Focus::Side);
}

#[test]
fn clicking_123_on_the_bottom_row_switches_layouts() {
    // 80 × 24: the bottom row is y 23, `1` `2` `3` are at x 1, 2, 3
    let mut app = app_with(ITEMS);
    app.screen = Rect::new(0, 0, 80, 24);
    let click = |app: &mut App, column| {
        app.handle_mouse(mouse_event(
            MouseEventKind::Down(MouseButton::Left),
            column,
            23,
        ));
    };
    click(&mut app, 3);
    assert_eq!((app.view, app.focus), (View::Tasks, Focus::Tasks));
    click(&mut app, 2);
    assert_eq!((app.view, app.focus), (View::Month, Focus::Calendar));
    click(&mut app, 1);
    assert_eq!(app.view, View::Planner);
}

#[test]
fn tab_skips_the_agenda_in_the_month_view() {
    // Month grid: calendar → side menu → calendar; when narrow (no side menu), Tab does nothing
    let mut app = wide_app_with(GROUPS);
    app.view = View::Month;
    for focus in [Focus::Side, Focus::Calendar] {
        press(&mut app, KeyCode::Tab);
        assert_eq!(app.focus, focus);
    }
    let mut app = app_with(ITEMS);
    app.view = View::Month;
    press(&mut app, KeyCode::Tab);
    assert_eq!(app.focus, Focus::Calendar);
}

#[test]
fn the_mouse_works_in_the_month_view() {
    // 80 × 30, 9/21 at the top (for cell positions see ui/tests.rs's
    // `month_view_click_positions_map_to_the_cells`)
    let mut app = app_with(ITEMS);
    app.view = View::Month;
    app.screen = Rect::new(0, 0, 80, 30);
    app.month_top = Some(date(2026, 9, 21));
    // Click the Tuesday cell of week 2 (not necessarily on the date): selects that day
    app.handle_mouse(mouse_event(MouseEventKind::Down(MouseButton::Left), 20, 9));
    assert_eq!(app.selected, date(2026, 9, 29));
    // The wheel scrolls the 6 weeks a week (9/28 on top); the selected day stays
    app.handle_mouse(mouse_event(MouseEventKind::ScrollDown, 20, 9));
    assert_eq!(app.selected, date(2026, 9, 29));
    assert_eq!(app.month_top, Some(date(2026, 9, 28)));
}

#[test]
fn brackets_put_the_month_on_top_in_the_month_view_too() {
    for code in [KeyCode::Char('['), KeyCode::Char(']')] {
        let mut app = app_with(ITEMS);
        app.view = View::Month;
        app.month_top = Some(date(2026, 1, 5));
        press(&mut app, code);
        assert_eq!(app.month_top, None, "{code:?}");
    }
}

#[test]
fn t_puts_todays_week_on_row_2_in_the_month_view() {
    // Today is Sat 10/17: its week starts Mon 10/12, so the week before (10/5) goes on top
    let mut app = App::new(Lang::En, date(2026, 10, 17), Vec::new());
    app.view = View::Month;
    app.selected = date(2027, 3, 1);
    app.month_top = Some(date(2027, 2, 22));
    press(&mut app, KeyCode::Char('t'));
    assert_eq!(app.selected, date(2026, 10, 17));
    assert_eq!(app.month_top, Some(date(2026, 10, 5)));
}

#[test]
fn quit_keys() {
    let keys = [
        KeyEvent::from(KeyCode::Char('q')),
        KeyEvent::from(KeyCode::Esc),
        KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL),
    ];
    for key in keys {
        let mut app = App::new(Lang::En, date(2026, 10, 17), Vec::new());
        app.handle_key(key);
        assert!(app.should_quit, "{key:?} should quit");
    }
}

#[test]
fn other_keys_do_not_quit() {
    // Plain c (no Ctrl), capital Q, and Enter do not quit
    let keys = [
        KeyEvent::from(KeyCode::Char('c')),
        KeyEvent::from(KeyCode::Char('Q')),
        KeyEvent::from(KeyCode::Enter),
    ];
    for key in keys {
        let mut app = App::new(Lang::En, date(2026, 10, 17), Vec::new());
        app.handle_key(key);
        assert!(!app.should_quit, "{key:?} should not quit");
    }
}

#[test]
fn in_progress_items_come_first_on_their_day() {
    // 10/17: Lunch (09:00), Email (no time), Report `[/]` (14:00), Slides `[/]` (no time); the multi-day
    // Trip started 10/16 and covers 10/17. 10/1: Prep talk `[/]`, and Conference carried over from 9/30
    let text = "- [ ] Lunch @2026-10-17 09:00\n- [ ] Email @2026-10-17\n\
                - [/] Report @2026-10-17 14:00\n- [/] Slides @2026-10-17\n- Trip @2026-10-16..2026-10-18\n\
                - Conference @2026-09-30..2026-10-02\n- [/] Prep talk @2026-10-01\n";
    let app = App::new(
        Lang::En,
        date(2026, 10, 17),
        crate::tasks::parse(text).items,
    );
    // Agenda (layout 1): in-progress first (before an item carried over to the 1st, too), each group still
    // by time (no time first)
    let agenda: Vec<(Date, &str)> = app
        .agenda()
        .iter()
        .map(|entry| (entry.date, entry.item.title.as_str()))
        .collect();
    let day = date(2026, 10, 17);
    assert_eq!(
        agenda,
        [
            (date(2026, 9, 30), "Conference"),
            (date(2026, 10, 1), "Prep talk"),
            (date(2026, 10, 1), "Conference"),
            (date(2026, 10, 16), "Trip"),
            (day, "Slides"),
            (day, "Report"),
            (day, "Email"),
            (day, "Lunch"),
        ]
    );
    // Layout 2's cell and popup: in-progress first, then the earlier start (Trip), then by time
    let on_day: Vec<&str> = app
        .items_on(day)
        .iter()
        .map(|item| item.title.as_str())
        .collect();
    assert_eq!(on_day, ["Slides", "Report", "Trip", "Email", "Lunch"]);
}

#[test]
fn day_counts_are_the_days_tasks_not_done_and_done() {
    // 10/17: two open tasks (one in progress), one done, an event, a multi-day task covering it, and a
    // task in the unchecked Personal calendar
    let text = "- [ ] Email Kevin @2026-10-17\n- [/] Draft the notes @2026-10-17\n- [x] Pay the fine @2026-10-17\n\
                - Standup @2026-10-17 09:00\n- [ ] Offsite prep @2026-10-16..2026-10-18\n\
                # Personal\n- [ ] Buy milk @2026-10-17\n";
    let mut app = App::new(
        Lang::En,
        date(2026, 10, 17),
        crate::tasks::parse(text).items,
    );
    app.set_items(crate::tasks::parse(text));
    let personal = app
        .groups
        .iter()
        .position(|group| group.name.as_deref() == Some("Personal"))
        .unwrap();
    app.groups[personal].shown = false;
    // Open: Email, Draft (in progress), Offsite prep; done: Pay the fine; the event and Buy milk do not count
    assert_eq!(app.day_counts(date(2026, 10, 17)), (3, 1));
    // Done tasks still count when 「Hide done」 hides them
    app.settings.calendar_list.hide_done = true;
    assert_eq!(app.day_counts(date(2026, 10, 17)), (3, 1));
    // A day with only the multi-day task; a day with none
    assert_eq!(app.day_counts(date(2026, 10, 18)), (1, 0));
    assert_eq!(app.day_counts(date(2026, 10, 19)), (0, 0));
}

#[test]
fn any_key_click_or_wheel_skips_the_intro_and_does_nothing_else() {
    let mut app = App::new(Lang::En, date(2026, 10, 17), Vec::new());
    // A mistake in the task file is noticed at start, before the intro
    app.notice = Some(Notice::Mistakes(1));
    app.play_splash();
    // `l` skips it without moving to the next day; the notice is still there to be seen
    press(&mut app, KeyCode::Char('l'));
    assert!(matches!(app.mode, Mode::Normal));
    assert_eq!(app.selected, date(2026, 10, 17));
    assert!(app.notice.is_some());
    // `q` only skips it, it does not quit
    app.play_splash();
    press(&mut app, KeyCode::Char('q'));
    assert!(matches!(app.mode, Mode::Normal) && !app.should_quit);
    // A click or the wheel: the same
    for kind in [
        MouseEventKind::Down(MouseButton::Left),
        MouseEventKind::ScrollDown,
        MouseEventKind::ScrollUp,
    ] {
        app.play_splash();
        app.handle_mouse(mouse_event(kind, 30, 5));
        assert!(matches!(app.mode, Mode::Normal), "{kind:?}");
        assert_eq!(app.selected, date(2026, 10, 17), "{kind:?}");
    }
    assert!(app.notice.is_some());
    // Moving the mouse does not skip it
    app.play_splash();
    app.handle_mouse(mouse_event(MouseEventKind::Moved, 30, 5));
    assert!(matches!(app.mode, Mode::Splash { .. }));
}

#[test]
fn the_intro_closes_by_itself_after_5_5_seconds() {
    let mut app = App::new(Lang::En, date(2026, 10, 17), Vec::new());
    app.play_splash();
    let Mode::Splash { opened } = app.mode else {
        panic!("the intro is not playing");
    };
    app.end_finished_splash(opened + Duration::from_millis(5_490));
    assert!(matches!(app.mode, Mode::Splash { .. }));
    app.end_finished_splash(opened + Duration::from_millis(5_500));
    assert!(matches!(app.mode, Mode::Normal));
    // Nothing else is closed by it
    app.mode = Mode::Help(Help::default());
    app.end_finished_splash(opened + Duration::from_secs(60));
    assert!(matches!(app.mode, Mode::Help(_)));
}
