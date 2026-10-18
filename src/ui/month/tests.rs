use super::*;
use jiff::civil::date;

#[test]
fn grid_splits_the_days_evenly_and_the_weeks_equally() {
    // Inside 78 × 28 (an 80 × 30 box): day width 78 − 5 − 7 = 66, split over 7 columns = 9 remainder 3
    // → Mon, Tue, Wed get 10 cells, the others 9. With grid lines the weeks shown do not matter
    let top = date(2026, 9, 28);
    let grid = Grid::new(Rect::new(1, 1, 78, 28), true, top);
    let widths: Vec<u16> = grid.columns.iter().map(|&(_, width)| width).collect();
    assert_eq!(widths, [5, 10, 10, 10, 9, 9, 9, 9]);
    // 1 cell for a vertical line between columns: Monday starts at 1 + 5 + 1 = 7, Sunday's last cell
    // touches the right edge of the inside (78)
    assert_eq!(grid.columns[1].0, 7);
    let (x, width) = grid.columns[7];
    assert_eq!(x + width, 1 + 78);
    // Height: 28 − header 2 − lines 5 = 21, 3 rows for each of 6 weeks; week 1 starts at 1 + 2 = 3,
    // the next week one line further
    assert_eq!(grid.week_height, 3);
    assert_eq!(grid.week_y(0), 3);
    assert_eq!(grid.week_y(1), 7);
    // 3 rows left over at the bottom (y 27-29): the line before the next week and 2 rows of it, cut off
    assert_eq!(grid.weeks, 7);
    assert_eq!(grid.cell(6, 1), Rect::new(7, 27, 10, 2));
    // Not evenly divisible: 27 rows → 20 ÷ 6 = 3 remainder 2: the line and the next week's first row;
    // remainder 1 (26 rows) has no room for the next week: the leftover row belongs to the 6th week
    let grid = Grid::new(Rect::new(1, 1, 78, 27), true, top);
    assert_eq!((grid.week_height, grid.weeks), (3, 7));
    assert_eq!(grid.cell(6, 1).height, 1);
    let grid = Grid::new(Rect::new(1, 1, 78, 26), true, top);
    assert_eq!((grid.week_height, grid.weeks), (3, 6));
    // Too small: cannot be drawn
    assert!(Grid::new(Rect::new(1, 1, 18, 28), true, top).too_small());
    assert!(Grid::new(Rect::new(1, 1, 78, 12), true, top).too_small());
    assert!(!Grid::new(Rect::new(1, 1, 19, 13), true, top).too_small());
}

#[test]
fn without_grid_lines_the_cells_touch() {
    // 78 × 28 without grid lines: day width 78 − 5 = 73, split over 7 columns = 10 remainder 3
    // → Mon, Tue, Wed get 11 cells, the others 10; columns touch
    let top = date(2027, 2, 1);
    let grid = Grid::new(Rect::new(1, 1, 78, 28), false, top);
    let widths: Vec<u16> = grid.columns.iter().map(|&(_, width)| width).collect();
    assert_eq!(widths, [5, 11, 11, 11, 10, 10, 10, 10]);
    assert_eq!(grid.columns[1].0, 1 + 5);
    let (x, width) = grid.columns[7];
    assert_eq!(x + width, 1 + 78);
    // Weeks from 2027/2/1: only March starts on screen (3/1, a Monday, the 5th week), so only one row
    // for a month's edge, between the weeks of 2/22 and 3/1. Height: 28 − header 1 − that row = 26,
    // 4 rows for each of 6 weeks; week 1 sits right below the header, the next week right after; the
    // 5th week one row further down
    assert_eq!(grid.week_gaps, [0, 0, 0, 1, 0, 0]);
    assert_eq!(grid.week_height, 4);
    assert_eq!(grid.week_y(0), 2);
    assert_eq!(grid.week_y(1), 6);
    assert_eq!(grid.week_y(3), 14);
    assert_eq!(grid.week_y(4), 19);
    // 2 rows left over (y 27-28) and no month's edge before the 7th week: 2 rows of it show
    assert_eq!(grid.weeks, 7);
    assert_eq!(grid.cell(6, 1), Rect::new(6, 27, 11, 2));
    // Weeks from 9/28: October starts in the first week (10/1, a Thursday), November on 11/1 (a
    // Sunday): its edge needs a row above the week of 10/26 (above 11/1) and one below (under 10/26-31).
    // 28 − 1 − 3 = 24 = 6 × 4: no rows left over, so no 7th week
    let grid = Grid::new(Rect::new(1, 1, 78, 28), false, date(2026, 9, 28));
    assert_eq!(grid.week_gaps, [1, 0, 0, 1, 1]);
    assert_eq!(grid.weeks, 6);
    // A height too short with grid lines can be drawn without them: 12 − 1 − 1 = 10 → 1 row per week
    assert!(!Grid::new(Rect::new(1, 1, 78, 12), false, top).too_small());
}

#[test]
fn multi_day_items_keep_one_line_across_the_row() {
    // The row 10/19 (Mon) - 10/25 (Sun): A 10/19-21, B 10/20-23 (overlaps A: the next line),
    // C 10/22-28 (A is over by then: C takes A's line again); D and E are one-day items
    let text = "- A @2026-10-19..2026-10-21\n- B @2026-10-20..2026-10-23\n\
                    - C @2026-10-22..2026-10-28\n- D @2026-10-19\n- E @2026-10-22\n- F @2026-10-24\n";
    let app = App::new(
        crate::texts::Lang::En,
        date(2026, 10, 17),
        crate::tasks::parse(text).items,
    );
    let days: Vec<Date> = date(2026, 10, 19).series(1.day()).take(7).collect();
    let (bars, slots) = layout_row(&days, &app);
    let bars: Vec<(&str, usize, usize, usize)> = bars
        .iter()
        .map(|bar| (bar.item.title.as_str(), bar.lane, bar.first, bar.last))
        .collect();
    assert_eq!(bars, [("A", 0, 0, 2), ("B", 1, 1, 4), ("C", 0, 3, 6)]);
    // Each day's lines: bars on their own line (`=`), one-day items fill free lines from the top
    // (F on Saturday takes the line B left free), then go below (E on Thursday)
    let lines: Vec<String> = slots
        .iter()
        .map(|day| {
            day.iter()
                .map(|slot| match slot {
                    Slot::Bar(item) => format!("={}", item.title),
                    Slot::Item(item) => item.title.clone(),
                    Slot::Empty => "_".to_string(),
                })
                .collect::<Vec<_>>()
                .join(" ")
        })
        .collect();
    assert_eq!(
        lines,
        ["=A D", "=A =B", "=A =B", "=C =B E", "=C =B", "=C F", "=C"]
    );
    // The next row, 10/26-11/1: C started a row before, so it does not write the time there; C ends on
    // Wednesday and nothing else is there, so no line is left free
    let later: Vec<Date> = date(2026, 10, 26).series(1.day()).take(7).collect();
    let (bars, slots) = layout_row(&later, &app);
    assert_eq!(
        (bars[0].item.title.as_str(), bars[0].first, bars[0].starts),
        ("C", 0, false)
    );
    assert_eq!(
        slots.iter().map(Vec::len).collect::<Vec<_>>(),
        [1, 1, 1, 0, 0, 0, 0]
    );
}

#[test]
fn week_column_text_lines_up_with_the_week_numbers() {
    // Week numbers are 「 {:>3} 」: 「  42 」; labels end in the same cell when they fit in 3 cells
    assert_eq!(week_column_text("Nov"), " Nov");
    assert_eq!(week_column_text("1月"), " 1月");
    assert_eq!(week_column_text("11月"), " 11月");
    assert_eq!(week_column_text("2027"), " 2027");
}

#[test]
fn a_cell_shows_all_its_lines_or_one_fewer_and_plus_n() {
    // 3 rows tall = the date + 2 lines: 2 fit; 3 do not, so 1 line and `+N` on the other
    assert_eq!(shown_lines(2, 3), 2);
    assert_eq!(shown_lines(3, 3), 1);
    // 1 row tall: only the date (dots instead)
    assert_eq!(shown_lines(2, 1), 0);
}

#[test]
fn scrolling_moves_a_week_only_when_the_selection_leaves_the_six_weeks() {
    // Top is 9/28: weeks 9/28, 10/5, 10/12, 10/19, 10/26, 11/2 (the week of 9/28 holds 10/1 too, the
    // week of 10/26 holds 11/1); no scrolling inside them
    let top = date(2026, 9, 28);
    assert_eq!(scrolled(top, date(2026, 9, 28), false), top);
    assert_eq!(scrolled(top, date(2026, 11, 8), false), top);
    // One week further down (11/9): the top moves down one week (to 10/5); one week up (9/27, in the
    // week of 9/21): that week becomes the top
    assert_eq!(scrolled(top, date(2026, 11, 9), false), date(2026, 10, 5));
    assert_eq!(scrolled(top, date(2026, 9, 27), false), date(2026, 9, 21));
    // A stored top that is not a Monday counts as the week it is in
    assert_eq!(
        scrolled(date(2026, 10, 3), date(2026, 10, 17), false),
        date(2026, 9, 28)
    );
    // Centered: the selected week (10/12) is always row 3, so the top is two weeks up (9/28)
    for top in [date(2026, 9, 21), date(2026, 10, 12), date(2027, 1, 4)] {
        assert_eq!(scrolled(top, date(2026, 10, 17), true), date(2026, 9, 28));
    }
    // Just opened: the week with the 1st is at the top (10/1 is a Thursday, so 9/28; 2027/1/1 is a
    // Friday, so 12/28; 2027/2/1 is a Monday, so 2/1 itself)
    assert_eq!(month_top(date(2026, 10, 17)), date(2026, 9, 28));
    assert_eq!(month_top(date(2027, 1, 31)), date(2026, 12, 28));
    assert_eq!(month_top(date(2027, 2, 14)), date(2027, 2, 1));
}
