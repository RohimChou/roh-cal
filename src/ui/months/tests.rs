use super::*;
use crate::texts::Lang;
use jiff::civil::date;

/// An app whose today is 2026/10/17 and whose task file is `text`
fn app_with(text: &str) -> App {
    App::new(
        Lang::En,
        date(2026, 10, 17),
        crate::tasks::parse(text).items,
    )
}

#[test]
fn row_numbers_count_from_this_month() {
    // Today is in 2026/10: October's first row is row 0;
    // with no items, each month's block = its weeks + 1 line
    let app = app_with("");
    let months = Months::new(&app, 100);
    assert_eq!(months.block_start(date(2026, 10, 1)), 0);
    assert_eq!(months.block_start(date(2026, 11, 20)), 6);
    assert_eq!(months.block_start(date(2027, 1, 1)), 19);
    assert_eq!(months.block_start(date(2027, 2, 1)), 25);
    assert_eq!(months.block_start(date(2026, 9, 1)), -6);
    assert_eq!(months.block_start(date(2026, 8, 1)), -13);
    assert_eq!(months.row_of(date(2026, 10, 17)), 2);
    assert_eq!(months.row_of(date(2026, 11, 7)), 7);
    assert_eq!(months.row_of(date(2027, 1, 3)), 19);
    assert_eq!(months.row_of(date(2026, 9, 26)), -3);
    // Each block's last row is the line: October's line is row 5, September's is row -1
    assert_eq!(months.block_at(5), (date(2026, 10, 1), 0));
    assert_eq!(months.block_at(6), (date(2026, 11, 1), 6));
    assert_eq!(months.block_at(-1), (date(2026, 9, 1), -6));
    assert_eq!(months.block_at(-6), (date(2026, 9, 1), -6));
    assert_eq!(months.block_at(-7), (date(2026, 8, 1), -13));
}

#[test]
fn months_grow_with_their_items() {
    // October has 5 weeks and 7 items: 7 rows; September's item and the one with no date do not count
    // toward October
    let text = (1..=7)
        .map(|day| format!("- [ ] x @2026-10-{day:02}\n"))
        .collect::<String>()
        + "- [ ] Pay September rent @2026-09-30\n- [ ] Organize the bookshelf\n";
    let app = app_with(&text);
    let one = Months::new(&app, 100);
    assert_eq!(one.month_items(date(2026, 10, 1)).len(), 7);
    assert_eq!(one.block_height(date(2026, 10, 1)), 8);
    assert_eq!(one.block_start(date(2026, 11, 1)), 8);
    // The selected week is still among the month's top rows
    assert_eq!(one.row_of(date(2026, 11, 7)), 9);
}

#[test]
fn items_are_sorted_by_date_then_time_untimed_first() {
    // Same day: untimed items first, then timed ones by time; same date and time keeps file order
    let app = app_with(
        "- b @2026-10-16 17:30\n- a @2026-10-16 09:00\n- c @2026-10-16\n- d @2026-10-15\n\
             - e @2026-10-16 09:00\n",
    );
    let months = Months::new(&app, 100);
    let titles: Vec<&str> = months
        .month_items(date(2026, 10, 1))
        .iter()
        .map(|entry| entry.item.title.as_str())
        .collect();
    assert_eq!(titles, ["d", "c", "a", "e", "b"]);
}

#[test]
fn each_row_holds_the_next_item() {
    // October, 3 items: one per row from the top, the other weeks' rows have none, then the line
    let text = (1..=3)
        .map(|day| format!("- [ ] {day} @2026-10-{day:02}\n"))
        .collect::<String>();
    let app = app_with(&text);
    let months = Months::new(&app, 100);
    let rows = months.rows(date(2026, 10, 1), 6);
    let titles: Vec<&str> = rows[..5]
        .iter()
        .map(|row| match row {
            Row::Week { item, .. } => item.map_or("", |entry| entry.item.title.as_str()),
            Row::Separator { .. } => panic!("October's 5 week rows should have no line"),
        })
        .collect();
    assert_eq!(titles, ["1", "2", "3", "", ""]);
    assert_eq!(
        rows[5],
        Row::Separator {
            year: false,
            hidden: 0
        }
    );
}

/// Each row's kind, for easy comparison: `-` = thin line, `=` = thick line, `w` = row with dates,
/// `.` = row with only items
fn shape(rows: &[Row]) -> String {
    rows.iter()
        .map(|row| match row {
            Row::Separator { year: false, .. } => '-',
            Row::Separator { year: true, .. } => '=',
            Row::Week { days: Some(_), .. } => 'w',
            Row::Week { days: None, .. } => '.',
        })
        .collect()
}

#[test]
fn months_with_more_items_than_weeks_get_rows_without_dates() {
    // October has 7 items → 5 weeks + 2 item-only rows; November has none → 6 weeks;
    // December → 5 weeks; thick line at the new year;
    // asking for 22 rows still lays out all of the last month, January (5 weeks)
    let text = (1..=7)
        .map(|day| format!("- [ ] x @2026-10-{day:02}\n"))
        .collect::<String>();
    let app = app_with(&text);
    let months = Months::new(&app, 100);
    assert_eq!(
        shape(&months.rows(date(2026, 10, 1), 22)),
        "wwwww..-wwwwww-wwwww=wwwww"
    );
}

#[test]
fn months_are_capped_and_scroll_inside() {
    // October with 20 items: 20 laid-out rows; at most 8 → the block has 8 rows,
    // November starts at row 9
    let text = (1..=20)
        .map(|day| format!("- [ ] {day} @2026-10-{day:02}\n"))
        .collect::<String>();
    let app = app_with(&text);
    let mut months = Months::new(&app, 8);
    assert_eq!(months.month_height(date(2026, 10, 1)), 8);
    assert_eq!(months.block_start(date(2026, 11, 1)), 9);
    // A cap below the week count: still at least the weeks (October has 5)
    assert_eq!(Months::new(&app, 3).month_height(date(2026, 10, 1)), 5);
    let agenda = app.agenda();
    let october = date(2026, 10, 1);
    // To see the 15th item (laid out on row 14): the agenda scrolls to row 8, leaving 1 row below
    assert_eq!(months.window_for(october, 0, &[agenda[14]]), 8);
    // Already visible: no scroll; nothing to show: unchanged; at most until the last item touches
    // the bottom (20 - 8 = 12)
    assert_eq!(months.window_for(october, 8, &[agenda[10]]), 8);
    assert_eq!(months.window_for(october, 8, &[]), 8);
    assert_eq!(months.window_for(october, 50, &[]), 12);
    // Scrolled 8 rows: October's first row shows the 9th item in the agenda; other months are not affected
    months.window = Some((october, 8));
    let rows = months.rows(october, 9);
    let Row::Week { item, .. } = &rows[0] else {
        panic!("the first row should be October");
    };
    assert_eq!(item.map(|entry| entry.item.title.as_str()), Some("9"));
    // The line below October: 20 - 8 - 8 = 4 items are still hidden
    assert_eq!(
        rows[8],
        Row::Separator {
            year: false,
            hidden: 4
        }
    );
}

#[test]
fn the_scrollbar_thumb_shows_how_much_and_where() {
    // Everything visible: no scrollbar
    assert_eq!(thumb_rows(5, 5, 0), None);
    // 8 items in 5 rows: thumb 5 × 5 / 8 = 3 rows; at the top when not scrolled, touching the bottom
    // when scrolled to the end (3 rows), in the middle when scrolled halfway
    assert_eq!(thumb_rows(8, 5, 0), Some(0..3));
    assert_eq!(thumb_rows(8, 5, 3), Some(2..5));
    assert_eq!(thumb_rows(20, 8, 6), Some(2..5));
    // Many items: the thumb is at least 1 row
    assert_eq!(thumb_rows(100, 5, 95), Some(4..5));
    // The laid-out rows carry the scrollbar too: October has 20 items, at most 8 rows, not scrolled
    // → the first 3 rows are the thumb; November has no scrollbar
    let text = (1..=20)
        .map(|day| format!("- [ ] {day} @2026-10-{day:02}\n"))
        .collect::<String>();
    let app = app_with(&text);
    let months = Months::new(&app, 8);
    let thumbs: Vec<Option<bool>> = months
        .rows(date(2026, 10, 1), 10)
        .iter()
        .filter_map(|row| match row {
            Row::Week { thumb, .. } => Some(*thumb),
            Row::Separator { .. } => None,
        })
        .collect();
    assert_eq!(
        &thumbs[..8],
        [true, true, true, false, false, false, false, false].map(Some)
    );
    assert_eq!(thumbs[8], None);
}

#[test]
fn block_at_finds_the_month_of_every_row() {
    // Check from the other direction: each day's row maps back to its own month; 5 years each way
    // (one day every 3 days, so every weekday comes up)
    // Uses the sample task file, where some months are taller than their weeks
    let app = app_with(include_str!("../../../testdata/tasks.md"));
    let months = Months::new(&app, 100);
    let mut day = date(2021, 1, 1);
    while day < date(2032, 1, 1) {
        let (month, start) = months.block_at(months.row_of(day));
        assert_eq!(month, day.first_of_month(), "{day}");
        assert_eq!(start, months.block_start(day), "{day}");
        day = day.saturating_add(3.days());
    }
}

#[test]
fn scrolled_top_keeps_the_margin() {
    // 10 rows, margin 2: no scrolling while the selected row is on screen rows 3 to 8 (index 2 to 7)
    assert_eq!(scrolled_top(0, 5, 10, 2), 0);
    assert_eq!(scrolled_top(0, 2, 10, 2), 0);
    assert_eq!(scrolled_top(0, 7, 10, 2), 0);
    // Past the bottom: the selected row stops at index 7
    assert_eq!(scrolled_top(0, 8, 10, 2), 1);
    assert_eq!(scrolled_top(0, 30, 10, 2), 23);
    // Past the top: the selected row stops at index 2; it can scroll to negative rows (earlier months)
    assert_eq!(scrolled_top(20, 21, 10, 2), 19);
    assert_eq!(scrolled_top(0, 1, 10, 2), -1);
    assert_eq!(scrolled_top(0, -40, 10, 2), -42);
    // A screen of only 3 rows: the margin is at most 1
    assert_eq!(scrolled_top(0, 5, 3, 2), 4);
}
