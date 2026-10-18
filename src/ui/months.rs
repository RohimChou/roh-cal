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
pub(super) fn scrolled_to_range(top: i64, first: i64, last: i64, height: i64, margin: i64) -> i64 {
    let margin = margin.min((height - (last - first + 1)) / 2).max(0);
    scrolled_top(
        scrolled_top(top, last, height, margin),
        first,
        height,
        margin,
    )
}

#[cfg(test)]
mod tests;
