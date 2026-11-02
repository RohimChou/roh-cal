use super::*;
use jiff::civil::{Weekday, date};

/// Weeks starting on Monday
const MONDAY: Weekday = Weekday::Monday;

#[test]
fn week_label_counts_weeks_from_the_selected_one_when_relative() {
    let selected = date(2026, 10, 17);
    // Normal: every week shows its week of the year
    assert_eq!(week_label(date(2026, 10, 5), selected, false, MONDAY), 41);
    // Relative: the selected week (any day 10/12 to 18) shows its week of
    // the year, other weeks show the distance; both directions are positive
    assert_eq!(week_label(date(2026, 10, 12), selected, true, MONDAY), 42);
    assert_eq!(week_label(date(2026, 10, 18), selected, true, MONDAY), 42);
    assert_eq!(week_label(date(2026, 10, 11), selected, true, MONDAY), 1);
    assert_eq!(week_label(date(2026, 10, 26), selected, true, MONDAY), 2);
    // Works across the new year too: the week of 12/28 is 11 weeks from the week of 10/12
    assert_eq!(week_label(date(2027, 1, 1), selected, true, MONDAY), 11);
}

#[test]
fn month_rows_split_the_weeks_at_both_ends() {
    // 2026/11/1 is a Sunday and 11/30 a Monday: the first and last rows each have only one day
    let november = month_rows(date(2026, 11, 15), MONDAY);
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
    let december = month_rows(date(2026, 12, 1), MONDAY);
    assert_eq!(december.len(), 5);
    assert_eq!(december[0][0], None);
    assert_eq!(december[0][1], Some(date(2026, 12, 1)));
    assert_eq!(december[4][3], Some(date(2026, 12, 31)));
    assert_eq!(december[4][4], None);
    // 2021/2/1 is a Monday and the month has only 28 days: exactly 4 full rows
    let february = month_rows(date(2021, 2, 1), MONDAY);
    assert_eq!(february.len(), 4);
    assert!(february.iter().flatten().all(Option::is_some));
}

#[test]
fn month_rows_put_every_day_once_under_its_weekday() {
    // Checked another way: each day appears exactly once, in its weekday's cell, and each row is one week
    let mut month = date(2000, 1, 1);
    while month < date(2040, 1, 1) {
        let rows = month_rows(month, MONDAY);
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
            let weeks: Vec<i16> = row
                .iter()
                .flatten()
                .map(|&d| week_of_year(d, MONDAY))
                .collect();
            assert!(weeks.windows(2).all(|w| w[0] == w[1]), "{month}");
        }
        month = month.saturating_add(1.month());
    }
}

#[test]
fn week_start_goes_back_to_the_monday() {
    // 2026/10/17 is a Saturday → 10/12; a Monday is its own Monday; across months and years
    assert_eq!(week_start(date(2026, 10, 17), MONDAY), date(2026, 10, 12));
    assert_eq!(week_start(date(2026, 10, 12), MONDAY), date(2026, 10, 12));
    assert_eq!(week_start(date(2026, 11, 1), MONDAY), date(2026, 10, 26));
    assert_eq!(week_start(date(2027, 1, 3), MONDAY), date(2026, 12, 28));
    // Checked another way: every day's Monday is a Monday, and at most 6 days earlier
    let mut day = date(2026, 1, 1);
    while day < date(2028, 1, 1) {
        let monday = week_start(day, MONDAY);
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
    assert_eq!(week_of_year(date(2025, 12, 30), MONDAY), 53);
    assert_eq!(week_of_year(date(2026, 1, 1), MONDAY), 1);
    assert_eq!(week_of_year(date(2026, 1, 2), MONDAY), 1);
    assert_eq!(week_of_year(date(2026, 12, 31), MONDAY), 53);
    assert_eq!(week_of_year(date(2027, 1, 1), MONDAY), 1);
    // An ordinary date
    assert_eq!(week_of_year(date(2026, 10, 10), MONDAY), 41);
    // Leap year that also starts on Sunday: 12/31 is week 54
    assert_eq!(week_of_year(date(2012, 12, 31), MONDAY), 54);
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
            usize::try_from(week_of_year(day, MONDAY)).ok(),
            Some(mondays + 1),
            "{day}"
        );
        day = day.saturating_add(1.day());
    }
}

/// Weeks starting on Sunday (the default)
const SUNDAY: Weekday = Weekday::Sunday;

#[test]
fn weekdays_start_from_the_first_day() {
    use Weekday::{Friday, Monday, Saturday, Sunday, Thursday, Tuesday, Wednesday};
    assert_eq!(
        weekdays(SUNDAY),
        [
            Sunday, Monday, Tuesday, Wednesday, Thursday, Friday, Saturday
        ]
    );
    assert_eq!(
        weekdays(MONDAY),
        [
            Monday, Tuesday, Wednesday, Thursday, Friday, Saturday, Sunday
        ]
    );
    // The weekend is the same two days wherever they sit
    let weekend: Vec<Weekday> = weekdays(SUNDAY)
        .into_iter()
        .filter(|&day| is_weekend(day))
        .collect();
    assert_eq!(weekend, [Sunday, Saturday]);
}

#[test]
fn month_rows_starting_on_sunday() {
    // Expected rows from Python's calendar.monthcalendar with setfirstweekday(SUNDAY).
    // 2026/11/1 is a Sunday: the first row is a whole week, 11/1 to 11/7; 5 rows, the last 11/29 and 11/30
    let november = month_rows(date(2026, 11, 15), SUNDAY);
    assert_eq!(november.len(), 5);
    let days =
        |from: i8| std::array::from_fn(|i| Some(date(2026, 11, from + i8::try_from(i).unwrap())));
    assert_eq!(november[0], days(1));
    assert_eq!(november[3], days(22));
    assert_eq!(
        november[4],
        [
            Some(date(2026, 11, 29)),
            Some(date(2026, 11, 30)),
            None,
            None,
            None,
            None,
            None
        ]
    );
    // 2026/12/1 is a Tuesday: the first row starts at the 3rd cell; 12/31 (Thursday) is the 5th cell of
    // the 5th row
    let december = month_rows(date(2026, 12, 1), SUNDAY);
    assert_eq!(december.len(), 5);
    assert_eq!(december[0][1], None);
    assert_eq!(december[0][2], Some(date(2026, 12, 1)));
    assert_eq!(december[4][4], Some(date(2026, 12, 31)));
    assert_eq!(december[4][5], None);
    // 2026/2/1 is a Sunday and the month has only 28 days: exactly 4 full rows
    let february = month_rows(date(2026, 2, 1), SUNDAY);
    assert_eq!(february.len(), 4);
    assert!(february.iter().flatten().all(Option::is_some));
}

#[test]
fn month_rows_starting_on_sunday_put_every_day_once_under_its_weekday() {
    // Same check as for Monday: each day once, in its weekday's cell (cell 0 = Sunday), one week per row
    let mut month = date(2000, 1, 1);
    while month < date(2040, 1, 1) {
        let rows = month_rows(month, SUNDAY);
        let days: Vec<Date> = rows.iter().flatten().flatten().copied().collect();
        let expected: Vec<Date> = month
            .series(1.day())
            .take(usize::try_from(month.days_in_month()).unwrap())
            .collect();
        assert_eq!(days, expected, "{month}");
        for row in &rows {
            for (column, day) in row.iter().enumerate() {
                if let Some(day) = day {
                    let weekday = Weekday::Sunday.wrapping_add(i64::try_from(column).unwrap());
                    assert_eq!(day.weekday(), weekday, "{day}");
                }
            }
            let weeks: Vec<i16> = row
                .iter()
                .flatten()
                .map(|&d| week_of_year(d, SUNDAY))
                .collect();
            assert!(weeks.windows(2).all(|w| w[0] == w[1]), "{month}");
        }
        month = month.saturating_add(1.month());
    }
}

#[test]
fn week_start_goes_back_to_the_sunday() {
    // 2026/10/17 is a Saturday → 10/11; a Sunday is its own start; across months and years
    assert_eq!(week_start(date(2026, 10, 17), SUNDAY), date(2026, 10, 11));
    assert_eq!(week_start(date(2026, 10, 11), SUNDAY), date(2026, 10, 11));
    assert_eq!(week_start(date(2026, 11, 1), SUNDAY), date(2026, 11, 1));
    assert_eq!(week_start(date(2027, 1, 2), SUNDAY), date(2026, 12, 27));
    // Checked another way: every day's start is a Sunday, and at most 6 days earlier
    let mut day = date(2026, 1, 1);
    while day < date(2028, 1, 1) {
        let start = week_start(day, SUNDAY);
        assert_eq!(start.weekday(), Weekday::Sunday, "{day}");
        assert!(
            start <= day && day <= start.saturating_add(6.days()),
            "{day}"
        );
        day = day.saturating_add(1.day());
    }
}

#[test]
fn week_of_year_starting_on_sunday() {
    // Expected values from Python's strftime("%U") (+ 1 when 1/1 is not a Sunday: %U counts the days
    // before the first Sunday as week 0). 2027/1/1 is a Friday: 1/1 and 1/2 are week 1, Sunday 1/3 week 2
    assert_eq!(week_of_year(date(2026, 12, 31), SUNDAY), 53);
    assert_eq!(week_of_year(date(2027, 1, 1), SUNDAY), 1);
    assert_eq!(week_of_year(date(2027, 1, 2), SUNDAY), 1);
    assert_eq!(week_of_year(date(2027, 1, 3), SUNDAY), 2);
    // A Sunday starts the next week: Saturday 10/10 is week 41, Sunday 10/11 week 42 (with Monday both 41)
    assert_eq!(week_of_year(date(2026, 10, 10), SUNDAY), 41);
    assert_eq!(week_of_year(date(2026, 10, 11), SUNDAY), 42);
    assert_eq!(week_of_year(date(2026, 10, 11), MONDAY), 41);
    // A year that starts on a Sunday: 1/1 is week 1
    assert_eq!(week_of_year(date(2023, 1, 1), SUNDAY), 1);
    // Leap year starting on a Saturday: 12/31 (a Sunday) is week 54
    assert_eq!(week_of_year(date(2028, 12, 31), SUNDAY), 54);
}

#[test]
fn week_of_year_starting_on_sunday_matches_counting_sundays() {
    // Checked with another method: week number = 1 + how many Sundays came after 1/1, up to this day
    let mut day = date(2000, 1, 1);
    while day < date(2040, 1, 1) {
        let sundays = day
            .first_of_year()
            .series(1.day())
            .skip(1)
            .take_while(|&d| d <= day)
            .filter(|d| d.weekday() == Weekday::Sunday)
            .count();
        assert_eq!(
            usize::try_from(week_of_year(day, SUNDAY)).ok(),
            Some(sundays + 1),
            "{day}"
        );
        day = day.saturating_add(1.day());
    }
}

#[test]
fn week_label_starting_on_sunday_counts_from_the_selected_sunday() {
    let selected = date(2026, 10, 17);
    // The selected week is 10/11 (Sunday) to 10/17: Sunday 10/11 shows its week of the year, 42; Sunday
    // 10/18 is already the next week (with Monday it would be the selected one)
    assert_eq!(week_label(date(2026, 10, 11), selected, true, SUNDAY), 42);
    assert_eq!(week_label(date(2026, 10, 18), selected, true, SUNDAY), 1);
    assert_eq!(week_label(date(2026, 10, 18), selected, true, MONDAY), 42);
    assert_eq!(week_label(date(2026, 10, 10), selected, true, SUNDAY), 1);
    // Across the new year: the week of 2027/1/1 starts on 12/27, 11 weeks after 10/11
    assert_eq!(week_label(date(2027, 1, 1), selected, true, SUNDAY), 11);
    // Not relative: its week of the year
    assert_eq!(week_label(date(2026, 10, 18), selected, false, SUNDAY), 43);
}
