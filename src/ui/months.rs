//! Layout 1's layout math: how many rows each month takes, each row's number, and the scroll position
//! (numbers only, no drawing)
//!
//! A month's block = the larger of its week count and the rows its items need (after splitting into
//! columns), but at most `max_height` rows (never fewer than its weeks),
//! plus the line after it; items that do not fit scroll on their own in that month's agenda (`window`).
//! Every row has a number: the first row of today's month is row 0, going down 1, 2, 3…, going up -1, -2…

use crate::{
    app::{App, Entry},
    calendar::{self, WeekRow},
};
use jiff::{ToSpan, civil::Date};

/// What the left column shows
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Label {
    /// The year, e.g. `2027年`
    Year,
    /// The month, e.g. `10月`
    Month,
}

/// One row on screen (the same row in both the left and right box)
#[derive(Debug, PartialEq)]
pub enum Row<'a> {
    /// The line between months; `year` = the next month is January (new year), drawn as a thick line;
    /// `hidden` = how many of the month above's agenda items are still hidden below (scroll to see them)
    Separator {
        /// The next month is January (new year): draw a thick line
        year: bool,
        /// How many agenda items of the month above are not shown
        hidden: usize,
    },
    /// One row inside a month: the year / month on the left (may be absent), this week's days that belong
    /// to this month, one line for each agenda column (may be absent),
    /// and the scrollbar on the right border
    ///
    /// When a month has more items than weeks, the extra rows only have agenda items, and `days` is `None`;
    /// there is a scrollbar only when the month's items do not fit (the agenda has to scroll): `thumb` is
    /// `Some(true)` = this row is part of the scrollbar thumb
    Week {
        /// The year / month on the left (only on the month's first and second rows)
        label: Option<Label>,
        /// This week's days that belong to this month; `None` on rows with only agenda items
        days: Option<WeekRow>,
        /// One line per agenda column (`None` when that column has no item on this row)
        items: Vec<Option<Entry<'a>>>,
        /// Scrollbar on the right border: `None` = this month needs no scrolling, `Some(true)` = this row
        /// is part of the thumb
        thumb: Option<bool>,
    },
}

/// Data for the layout: which month holds row 0, the dated items (sorted by date and time), how many
/// agenda columns, the most rows per month, and where the agenda is scrolled to
pub struct Months<'a> {
    /// The month that holds row 0 (today's month)
    origin: Date,
    /// Every agenda line (`App::agenda()`), sorted by the day it is listed on, then time
    items: Vec<Entry<'a>>,
    /// How many columns the agenda items are split into
    columns: usize,
    /// The most rows a month can take (if fewer than the month's weeks, the week count wins)
    max_height: usize,
    /// Where the agenda is scrolled: (the 1st of which month, which laid-out row is at the top of that
    /// month's agenda); other months start from their first row
    pub window: Option<(Date, usize)>,
}

impl<'a> Months<'a> {
    /// Lays out from `app`: today's month is row 0; the agenda has `columns` columns, a month takes at most
    /// `max_height` rows; the agenda's scroll position follows `app.agenda_window`
    pub fn new(app: &'a App, columns: usize, max_height: usize) -> Self {
        Self {
            origin: app.today,
            items: app.agenda(),
            columns,
            max_height,
            window: None,
        }
    }

    /// The items listed in `month` (`items` is already sorted by listed day, so they form one contiguous slice)
    fn month_items(&self, month: Date) -> &[Entry<'a>] {
        let first = month.first_of_month();
        let last = month.last_of_month();
        let start = self.items.partition_point(|entry| entry.date < first);
        let end = self.items.partition_point(|entry| entry.date <= last);
        &self.items[start..end]
    }

    /// The items on `date` (a multi-day item counts if it covers that day; it is the line listed in
    /// that month)
    pub fn day_items(&self, date: Date) -> Vec<Entry<'a>> {
        self.month_items(date)
            .iter()
            .filter(|entry| entry.is_on(date))
            .copied()
            .collect()
    }

    /// How many rows this month's layout needs (no cap; with columns, the height of each column): the
    /// larger of the week count and the rows the items need
    fn layout_height(&self, month: Date) -> usize {
        let weeks = calendar::month_rows(month).len();
        weeks.max(self.month_items(month).len().div_ceil(self.columns))
    }

    /// Rows in a month's block (not counting the line after it): the layout rows, but at most `max_height`
    /// (never fewer than its weeks)
    fn month_height(&self, month: Date) -> usize {
        let weeks = calendar::month_rows(month).len();
        self.layout_height(month).min(weeks.max(self.max_height))
    }

    /// Which row `month`'s agenda is scrolled to: the selected month follows `window`, other months start
    /// at 0; at most until the last row touches the bottom of the block
    fn offset(&self, month: Date) -> usize {
        let max = self.layout_height(month) - self.month_height(month);
        self.window
            .filter(|(window_month, _)| *window_month == month.first_of_month())
            .map_or(0, |(_, offset)| offset.min(max))
    }

    /// Rows in a month's block, plus the line after it
    fn block_height(&self, month: Date) -> i64 {
        i64::try_from(self.month_height(month)).unwrap_or(0) + 1
    }

    /// The row number of the first row of `month`'s block
    pub fn block_start(&self, month: Date) -> i64 {
        let mut start = 0;
        let mut current = self.origin.first_of_month();
        let month = month.first_of_month();
        // Forward: add up the height of each month before it; backward: subtract them
        while current < month {
            start += self.block_height(current);
            current = current.saturating_add(1.month());
        }
        while current > month {
            current = current.saturating_sub(1.month());
            start -= self.block_height(current);
        }
        start
    }

    /// The row number of the week containing `date`
    fn row_of(&self, date: Date) -> i64 {
        // Which week of the month this is (from 0); weeks always take the month's top rows
        let week = calendar::month_rows(date)
            .iter()
            .position(|row| row.contains(&Some(date)))
            .unwrap_or(0);
        self.block_start(date) + i64::try_from(week).unwrap_or(0)
    }

    /// Which month's block row `row` falls in: returns that month (its 1st) and the row number of the
    /// block's first row
    fn block_at(&self, row: i64) -> (Date, i64) {
        let mut month = self.origin.first_of_month();
        let mut start = 0;
        while row < start {
            month = month.saturating_sub(1.month());
            start -= self.block_height(month);
        }
        while row >= start + self.block_height(month) {
            start += self.block_height(month);
            month = month.saturating_add(1.month());
        }
        (month, start)
    }

    /// Lays out month by month from `first`'s month down until there are at least `height` rows (the
    /// last month may run past it)
    fn rows(&self, first: Date, height: usize) -> Vec<Row<'a>> {
        let mut rows = Vec::new();
        let mut month = first.first_of_month();
        // How many items of the previous month are still hidden below (written on the line above this month)
        let mut hidden = 0;
        while rows.len() < height {
            if !rows.is_empty() {
                rows.push(Row::Separator {
                    year: month.month() == 1,
                    hidden,
                });
            }
            hidden = self.hidden_below(month);
            let weeks = calendar::month_rows(month);
            let items = self.month_items(month);
            let layout_height = self.layout_height(month);
            let offset = self.offset(month);
            let thumb = thumb_rows(layout_height, self.month_height(month), offset);
            // January: the year on the first row and the month on the second;
            // other months only have the month on the first row
            let january = month.month() == 1;
            for i in 0..self.month_height(month) {
                let label = match (january, i) {
                    (true, 0) => Some(Label::Year),
                    (true, 1) | (false, 0) => Some(Label::Month),
                    _ => None,
                };
                rows.push(Row::Week {
                    label,
                    days: weeks.get(i).copied(),
                    // Fill the first column before the second (`layout_height` rows each); the agenda is
                    // scrolled by `offset` rows:
                    // row i of column c = the month's item at c × rows per column + offset + i
                    items: (0..self.columns)
                        .map(|c| items.get(c * layout_height + offset + i).copied())
                        .collect(),
                    thumb: thumb.as_ref().map(|thumb| thumb.contains(&i)),
                });
            }
            month = month.saturating_add(1.month());
        }
        rows
    }

    /// How many of `month`'s agenda items are still hidden below after scrolling (counting every column)
    fn hidden_below(&self, month: Date) -> usize {
        let count = self.month_items(month).len();
        let layout_height = self.layout_height(month);
        // The row after the last visible row
        let end = self.offset(month) + self.month_height(month);
        (0..self.columns)
            .map(|c| {
                // How many items column c has (at most `layout_height` per column)
                let in_column = count.saturating_sub(c * layout_height).min(layout_height);
                in_column.saturating_sub(end)
            })
            .sum()
    }

    /// Which row of its month `entry` is laid out on (before scrolling; with columns, the row within
    /// its column): returns that month (its 1st) and the row
    fn layout_row(&self, entry: Entry) -> Option<(Date, usize)> {
        let month = entry.date.first_of_month();
        let index = self
            .month_items(month)
            .iter()
            .position(|other| *other == entry)?;
        Some((month, index % self.layout_height(month)))
    }

    /// Which row `month`'s agenda must scroll to so the `targets` items are visible: starting from
    /// `previous`, keep 1 extra row before and after if possible (to see what comes before and after);
    /// do not scroll if they are already visible, so the agenda does not jump around; if they do not
    /// fit, favor the top one
    pub fn window_for(&self, month: Date, previous: usize, targets: &[Entry]) -> usize {
        let month = month.first_of_month();
        let height = i64::try_from(self.month_height(month)).unwrap_or(0);
        let max = i64::try_from(self.layout_height(month)).unwrap_or(0) - height;
        let previous = i64::try_from(previous).unwrap_or(0).min(max);
        let rows: Vec<i64> = targets
            .iter()
            .filter_map(|&entry| self.layout_row(entry))
            .filter(|(item_month, _)| *item_month == month)
            .filter_map(|(_, row)| i64::try_from(row).ok())
            .collect();
        let window = match (rows.iter().min(), rows.iter().max()) {
            (Some(&first), Some(&last)) => scrolled_to_range(previous, first, last, height, 1),
            _ => previous,
        };
        usize::try_from(window.clamp(0, max)).unwrap_or(0)
    }

    /// The top row after scrolling: keep the selected week at least `margin` rows from the top and
    /// bottom edges (see `scrolled_top`), then make the `targets` items visible too (at their position
    /// after the `window` scroll, no margin); if they do not fit, favor the top one
    pub fn scrolled_to(
        &self,
        top: i64,
        selected: Date,
        targets: &[Entry],
        height: i64,
        margin: i64,
    ) -> i64 {
        let week = self.row_of(selected);
        let top = scrolled_top(top, week, height, margin);
        let mut rows = vec![week];
        for (month, row) in targets.iter().filter_map(|&entry| self.layout_row(entry)) {
            // Which row of the block that item is drawn on (items scrolled out of the agenda do not count)
            let shown = row.checked_sub(self.offset(month));
            if let Some(shown) = shown.filter(|&shown| shown < self.month_height(month)) {
                rows.push(self.block_start(month) + i64::try_from(shown).unwrap_or(0));
            }
        }
        let first = rows.iter().copied().min().unwrap_or(week);
        let last = rows.iter().copied().max().unwrap_or(week);
        scrolled_to_range(top, first, last, height, 0)
    }

    /// The rows visible on screen: `height` rows starting at row `top` (used both for drawing and
    /// for mouse mapping)
    pub fn visible_rows(&self, top: i64, height: usize) -> Vec<Row<'a>> {
        // Lay out from the month that holds the top row; the first `skip` rows are above the screen
        let (month, start) = self.block_at(top);
        let skip = usize::try_from(top - start).unwrap_or(0);
        let mut rows = self.rows(month, skip + height);
        rows.drain(..skip);
        rows.truncate(height);
        rows
    }
}

/// Which rows of the block hold the scrollbar thumb: `layout` rows laid out, `height` rows visible,
/// scrolled `offset` rows; no scrollbar when everything is visible
///
/// Thumb length = `height` × the visible fraction (at least 1 row); scrolled to the end, the thumb
/// just touches the bottom. E.g. 8 items in 5 rows, scrolled 3 rows → rows 2 to 4
fn thumb_rows(layout: usize, height: usize, offset: usize) -> Option<std::ops::Range<usize>> {
    if layout <= height {
        return None;
    }
    let length = (height * height / layout).max(1);
    let start = offset * (height - length) / (layout - height);
    Some(start..start + length)
}

/// The top row after scrolling: the screen has `height` rows, the current top is `top`, the selected
/// row is `selected`
///
/// Keep the selected row at least `margin` rows from the top and bottom edges, scrolling only when
/// needed; e.g. 10 rows with a margin of 2 → the selected row can only be on screen rows 3 to 8
pub fn scrolled_top(top: i64, selected: i64, height: i64, margin: i64) -> i64 {
    if height <= 0 {
        return selected;
    }
    // On a very short screen, the margin is at most half
    let margin = margin.min((height - 1) / 2);
    if selected < top + margin {
        selected - margin
    } else if selected + margin >= top + height {
        selected + margin + 1 - height
    } else {
        top
    }
}

/// The top row after scrolling: make rows `first` to `last` visible, with up to `margin` rows above
/// and below (the margin shrinks if it does not fit with them);
/// if they still do not fit, favor `first`
fn scrolled_to_range(top: i64, first: i64, last: i64, height: i64, margin: i64) -> i64 {
    let margin = margin.min((height - (last - first + 1)) / 2).max(0);
    scrolled_top(
        scrolled_top(top, last, height, margin),
        first,
        height,
        margin,
    )
}

#[cfg(test)]
mod tests {
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
        let months = Months::new(&app, 1, 100);
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
        // October has 5 weeks and 7 items: one column needs 7 rows, two columns need 4 (fewer than 5 weeks,
        // so still 5 rows); September's item and the one with no date do not count toward October
        let text = (1..=7)
            .map(|day| format!("- [ ] x @2026-10-{day:02}\n"))
            .collect::<String>()
            + "- [ ] Pay September rent @2026-09-30\n- [ ] Organize the bookshelf\n";
        let app = app_with(&text);
        let one = Months::new(&app, 1, 100);
        assert_eq!(one.month_items(date(2026, 10, 1)).len(), 7);
        assert_eq!(one.block_height(date(2026, 10, 1)), 8);
        assert_eq!(one.block_start(date(2026, 11, 1)), 8);
        let two = Months::new(&app, 2, 100);
        assert_eq!(two.block_height(date(2026, 10, 1)), 6);
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
        let months = Months::new(&app, 1, 100);
        let titles: Vec<&str> = months
            .month_items(date(2026, 10, 1))
            .iter()
            .map(|entry| entry.item.title.as_str())
            .collect();
        assert_eq!(titles, ["d", "c", "a", "e", "b"]);
    }

    #[test]
    fn rows_fill_the_first_column_before_the_second() {
        // October, 7 items, two columns: height 5 (weeks win), 5 items in the first column, 2 in the second
        let text = (1..=7)
            .map(|day| format!("- [ ] {day} @2026-10-{day:02}\n"))
            .collect::<String>();
        let app = app_with(&text);
        let months = Months::new(&app, 2, 100);
        let rows = months.rows(date(2026, 10, 1), 6);
        let titles: Vec<[&str; 2]> = rows[..5]
            .iter()
            .map(|row| match row {
                Row::Week { items, .. } => {
                    [0, 1].map(|c| items[c].map_or("", |entry| &entry.item.title))
                }
                Row::Separator { .. } => panic!("October's 5 week rows should have no line"),
            })
            .collect();
        assert_eq!(
            titles,
            [["1", "6"], ["2", "7"], ["3", ""], ["4", ""], ["5", ""]]
        );
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
        // One column: October has 7 items → 5 weeks + 2 item-only rows; November has none → 6 weeks;
        // December → 5 weeks; thick line at the new year;
        // asking for 22 rows still lays out all of the last month, January (5 weeks)
        let text = (1..=7)
            .map(|day| format!("- [ ] x @2026-10-{day:02}\n"))
            .collect::<String>();
        let app = app_with(&text);
        let months = Months::new(&app, 1, 100);
        assert_eq!(
            shape(&months.rows(date(2026, 10, 1), 22)),
            "wwwww..-wwwwww-wwwww=wwwww"
        );
    }

    #[test]
    fn months_are_capped_and_scroll_inside() {
        // October with 20 items (one column): 20 laid-out rows; at most 8 → the block has 8 rows,
        // November starts at row 9
        let text = (1..=20)
            .map(|day| format!("- [ ] {day} @2026-10-{day:02}\n"))
            .collect::<String>();
        let app = app_with(&text);
        let mut months = Months::new(&app, 1, 8);
        assert_eq!(months.month_height(date(2026, 10, 1)), 8);
        assert_eq!(months.block_start(date(2026, 11, 1)), 9);
        // A cap below the week count: still at least the weeks (October has 5)
        assert_eq!(Months::new(&app, 1, 3).month_height(date(2026, 10, 1)), 5);
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
        let Row::Week { items, .. } = &rows[0] else {
            panic!("the first row should be October");
        };
        assert_eq!(items[0].map(|entry| entry.item.title.as_str()), Some("9"));
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
        let months = Months::new(&app, 1, 8);
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
        let app = app_with(include_str!("../../testdata/tasks.md"));
        let months = Months::new(&app, 1, 100);
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
}
