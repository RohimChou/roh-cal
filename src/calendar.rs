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
mod tests {
    use super::*;
    use jiff::civil::{Weekday, date};

    #[test]
    fn week_label_counts_weeks_from_the_selected_one_when_relative() {
        let selected = date(2026, 10, 17);
        // Normal: every week shows its week of the year
        assert_eq!(week_label(date(2026, 10, 5), selected, false), 41);
        // Relative: the selected week (any day 10/12 to 18) shows its week of
        // the year, other weeks show the distance; both directions are positive
        assert_eq!(week_label(date(2026, 10, 12), selected, true), 42);
        assert_eq!(week_label(date(2026, 10, 18), selected, true), 42);
        assert_eq!(week_label(date(2026, 10, 11), selected, true), 1);
        assert_eq!(week_label(date(2026, 10, 26), selected, true), 2);
        // Works across the new year too: the week of 12/28 is 11 weeks from the week of 10/12
        assert_eq!(week_label(date(2027, 1, 1), selected, true), 11);
    }

    #[test]
    fn month_rows_split_the_weeks_at_both_ends() {
        // 2026/11/1 is a Sunday and 11/30 a Monday: the first and last rows each have only one day
        let november = month_rows(date(2026, 11, 15));
        assert_eq!(november.len(), 6);
        assert_eq!(
            november[0],
            [None, None, None, None, None, None, Some(date(2026, 11, 1))]
        );
        assert_eq!(
            november[5],
            [Some(date(2026, 11, 30)), None, None, None, None, None, None]
        );
        // 2026/12/1 is a Tuesday: the first row starts at the 2nd cell
        let december = month_rows(date(2026, 12, 1));
        assert_eq!(december.len(), 5);
        assert_eq!(december[0][0], None);
        assert_eq!(december[0][1], Some(date(2026, 12, 1)));
        assert_eq!(december[4][3], Some(date(2026, 12, 31)));
        assert_eq!(december[4][4], None);
        // 2021/2/1 is a Monday and the month has only 28 days: exactly 4 full rows
        let february = month_rows(date(2021, 2, 1));
        assert_eq!(february.len(), 4);
        assert!(february.iter().flatten().all(Option::is_some));
    }

    #[test]
    fn month_rows_put_every_day_once_under_its_weekday() {
        // Checked another way: each day appears exactly once, in its weekday's cell, and each row is one week
        let mut month = date(2000, 1, 1);
        while month < date(2040, 1, 1) {
            let rows = month_rows(month);
            let days: Vec<Date> = rows.iter().flatten().flatten().copied().collect();
            let expected: Vec<Date> = month
                .series(1.day())
                .take(usize::try_from(month.days_in_month()).unwrap())
                .collect();
            assert_eq!(days, expected, "{month}");
            for row in &rows {
                for (column, day) in row.iter().enumerate() {
                    if let Some(day) = day {
                        let weekday = Weekday::Monday.wrapping_add(i64::try_from(column).unwrap());
                        assert_eq!(day.weekday(), weekday, "{day}");
                    }
                }
                let weeks: Vec<i16> = row.iter().flatten().map(|&d| week_of_year(d)).collect();
                assert!(weeks.windows(2).all(|w| w[0] == w[1]), "{month}");
            }
            month = month.saturating_add(1.month());
        }
    }

    #[test]
    fn monday_of_goes_back_to_the_monday() {
        // 2026/10/17 is a Saturday → 10/12; a Monday is its own Monday; across months and years
        assert_eq!(monday_of(date(2026, 10, 17)), date(2026, 10, 12));
        assert_eq!(monday_of(date(2026, 10, 12)), date(2026, 10, 12));
        assert_eq!(monday_of(date(2026, 11, 1)), date(2026, 10, 26));
        assert_eq!(monday_of(date(2027, 1, 3)), date(2026, 12, 28));
        // Checked another way: every day's Monday is a Monday, and at most 6 days earlier
        let mut day = date(2026, 1, 1);
        while day < date(2028, 1, 1) {
            let monday = monday_of(day);
            assert_eq!(monday.weekday(), Weekday::Monday, "{day}");
            assert!(
                monday <= day && day <= monday.saturating_add(6.days()),
                "{day}"
            );
            day = day.saturating_add(1.day());
        }
    }

    #[test]
    fn week_of_year_cross_year_follows_the_date() {
        // The week across the new year: follows the year the date falls in
        assert_eq!(week_of_year(date(2025, 12, 30)), 53);
        assert_eq!(week_of_year(date(2026, 1, 1)), 1);
        assert_eq!(week_of_year(date(2026, 1, 2)), 1);
        assert_eq!(week_of_year(date(2026, 12, 31)), 53);
        assert_eq!(week_of_year(date(2027, 1, 1)), 1);
        // An ordinary date
        assert_eq!(week_of_year(date(2026, 10, 10)), 41);
        // Leap year that also starts on Sunday: 12/31 is week 54
        assert_eq!(week_of_year(date(2012, 12, 31)), 54);
    }

    #[test]
    fn week_of_year_matches_counting_mondays() {
        // Checked with another method: week number = 1 + how many Mondays came after 1/1, up to this day
        let mut day = date(2000, 1, 1);
        while day < date(2040, 1, 1) {
            let mondays = day
                .first_of_year()
                .series(1.day())
                .skip(1)
                .take_while(|&d| d <= day)
                .filter(|d| d.weekday() == Weekday::Monday)
                .count();
            assert_eq!(
                usize::try_from(week_of_year(day)).ok(),
                Some(mondays + 1),
                "{day}"
            );
            day = day.saturating_add(1.day());
        }
    }
}
