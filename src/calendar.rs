//! Pure date math: no screen code; takes dates, returns dates or numbers
//!
//! A week starts on `first` (the config file's `[calendar] first_day_of_week`), so the functions about weeks
//! take it as an argument

use jiff::{
    ToSpan,
    civil::{Date, Weekday},
};

/// The 7 days of a week, from its first day (`first`) on; days that are not in this month are `None`
pub type WeekRow = [Option<Date>; 7];

/// The 7 weekdays in the calendars' column order: a week starting on `first`
///
/// Example: `Weekday::Monday` → Monday, Tuesday, …, Sunday
pub fn weekdays(first: Weekday) -> [Weekday; 7] {
    std::array::from_fn(|column| first.wrapping_add(i64::try_from(column).unwrap_or(0)))
}

/// Saturday and Sunday: the days off that the calendars color as such (wherever they are in the week)
pub fn is_weekend(weekday: Weekday) -> bool {
    matches!(weekday, Weekday::Saturday | Weekday::Sunday)
}

/// Every week of `month`, one row per week starting on `first`; a week that crosses months keeps
/// only this month's days (the other half goes to the neighboring month)
///
/// Example: weeks starting on Monday, November 2026 has 6 rows; the first row has only 11/1 (Sunday), the
/// last only 11/30 (Monday)
pub fn month_rows(month: Date, first: Weekday) -> Vec<WeekRow> {
    let last = month.last_of_month();
    let mut rows = Vec::new();
    let mut row: WeekRow = [None; 7];
    for day in month
        .first_of_month()
        .series(1.day())
        .take_while(|&d| d <= last)
    {
        // The week's first day = 0, its last = 6
        let column = day.weekday().since(first) as usize;
        row[column] = Some(day);
        // The week's last day or the end of the month: this row ends
        if column == 6 || day == last {
            rows.push(row);
            row = [None; 7];
        }
    }
    rows
}

/// The first day (a `first`) of the week that `date` is in
pub fn week_start(date: Date, first: Weekday) -> Date {
    date.saturating_sub(i64::from(date.weekday().since(first)).days())
}

/// Which week of its year `date` is in: the week with 1/1 is week 1 (weeks start on `first`)
///
/// The week across the new year follows the year `date` falls in, e.g. weeks starting on Monday,
/// 2025-12-30 is week 53 of 2025 and 2026-01-02 is week 1 of 2026
pub fn week_of_year(date: Date, first: Weekday) -> i16 {
    // How many days come before 1/1 in 1/1's week (e.g. 1/1 is a Thursday, weeks start on Monday → 3 days
    // before it)
    let offset = date.first_of_year().weekday().since(first);
    (date.day_of_year() - 1 + i16::from(offset)) / 7 + 1
}

/// The number in the week-number column: each week shows its week of the year (`week_of_year`).
/// With `relative` (the config file's `[calendar] relative_weeks`), only the
/// selected week shows its week of the year; other weeks show how many weeks
/// they are from the selected one (either direction, like vim's relativenumber). Weeks start on `first`
///
/// Example (weeks starting on Monday): selected 10/17 (week 42) → the week of 9/28 shows 2, 10/5 shows 1,
/// 10/12 shows 42, 10/26 shows 2
pub fn week_label(date: Date, selected: Date, relative: bool, first: Weekday) -> i64 {
    let distance =
        i64::from((week_start(date, first) - week_start(selected, first)).get_days()).abs() / 7;
    if relative && distance > 0 {
        distance
    } else {
        i64::from(week_of_year(date, first))
    }
}

#[cfg(test)]
mod tests;
