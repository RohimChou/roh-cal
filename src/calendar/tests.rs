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
