//! Pure date math: no screen code; takes dates, returns dates or numbers

use jiff::{ToSpan, civil::Date};

/// The 7 days of a week (Monday to Sunday); days that are not in this month are `None`
pub type WeekRow = [Option<Date>; 7];

/// Every week of `month`, one row per week; a week that crosses months keeps
/// only this month's days (the other half goes to the neighboring month)
///
/// Example: November 2026 has 6 rows; the first row has only 11/1 (Sunday), the last only 11/30 (Monday)
pub fn month_rows(month: Date) -> Vec<WeekRow> {
    let last = month.last_of_month();
    let mut rows = Vec::new();
    let mut row: WeekRow = [None; 7];
    for day in month
        .first_of_month()
        .series(1.day())
        .take_while(|&d| d <= last)
    {
        // Monday = 0, Sunday = 6
        let column = day.weekday().to_monday_zero_offset() as usize;
        row[column] = Some(day);
        // Sunday or the end of the month: this row ends
        if column == 6 || day == last {
            rows.push(row);
            row = [None; 7];
        }
    }
    rows
}

/// The Monday of the week that `date` is in
pub fn monday_of(date: Date) -> Date {
    date.saturating_sub(i64::from(date.weekday().to_monday_zero_offset()).days())
}

/// Which week of its year `date` is in: the week with 1/1 is week 1 (weeks start on Monday)
///
/// The week across the new year follows the year `date` falls in, e.g.
/// 2025-12-30 is week 53 of 2025 and 2026-01-02 is week 1 of 2026
pub fn week_of_year(date: Date) -> i16 {
    // How many days come before 1/1 in 1/1's week (e.g. 1/1 is a Thursday → 3 days before it)
    let offset = date.first_of_year().weekday().to_monday_zero_offset();
    (date.day_of_year() - 1 + i16::from(offset)) / 7 + 1
}

/// The number in the week-number column: each week shows its week of the year (`week_of_year`).
/// With `relative` (the config file's `[calendar] relative_weeks`), only the
/// selected week shows its week of the year; other weeks show how many weeks
/// they are from the selected one (either direction, like vim's relativenumber)
///
/// Example: selected 10/17 (week 42) → the week of 9/28 shows 2, 10/5 shows 1, 10/12 shows 42, 10/26 shows 2
pub fn week_label(date: Date, selected: Date, relative: bool) -> i64 {
    let distance = i64::from((monday_of(date) - monday_of(selected)).get_days()).abs() / 7;
    if relative && distance > 0 {
        distance
    } else {
        i64::from(week_of_year(date))
    }
}

#[cfg(test)]
mod tests;
