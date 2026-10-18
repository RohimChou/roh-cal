//! 純日期計算：不碰畫面，只吃日期、回傳日期或數字

use jiff::{ToSpan, civil::Date};

/// 一週的 7 天（星期一到星期日）；不屬於這個月的那幾天是 `None`
pub type WeekRow = [Option<Date>; 7];

/// `month` 那個月的每一週，一週一列；跨月的那一週只留這個月的日子（另一半放在隔壁的月）
///
/// 例：2026 年 11 月有 6 列，第一列只有 11/1（星期日）、最後一列只有 11/30（星期一）
pub fn month_rows(month: Date) -> Vec<WeekRow> {
    let last = month.last_of_month();
    let mut rows = Vec::new();
    let mut row: WeekRow = [None; 7];
    for day in month
        .first_of_month()
        .series(1.day())
        .take_while(|&d| d <= last)
    {
        // 星期一 = 0、星期日 = 6
        let column = day.weekday().to_monday_zero_offset() as usize;
        row[column] = Some(day);
        // 星期日或月底：這一列結束
        if column == 6 || day == last {
            rows.push(row);
            row = [None; 7];
        }
    }
    rows
}

/// `date` 那一週的星期一
pub fn monday_of(date: Date) -> Date {
    date.saturating_sub(i64::from(date.weekday().to_monday_zero_offset()).days())
}

/// `date` 是它那一年的第幾週：1/1 所在的那週是第 1 週（每週從星期一開始）
///
/// 跨年那週看 `date` 落在哪一年，例：2025-12-30 是 2025 年第 53 週、2026-01-02 是 2026 年第 1 週
pub fn week_of_year(date: Date) -> i16 {
    // 1/1 那週裡，1/1 前面還有幾天（例：1/1 是星期四 → 前面有 3 天）
    let offset = date.first_of_year().weekday().to_monday_zero_offset();
    (date.day_of_year() - 1 + i16::from(offset)) / 7 + 1
}

/// 週數欄寫的數字：每週寫第幾週（`week_of_year`）；`relative`（設定檔的 `[calendar] relative_weeks`）時只有選取那週
/// 寫第幾週，其他週寫跟選取那週差幾週（不分前後，像 vim 的 relativenumber）
///
/// 例：選取 10/17（第 42 週）→ 10/5 那週寫 2、10/12 那週寫 42、10/26 那週寫 2
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
        // 平常：每週都寫第幾週
        assert_eq!(week_label(date(2026, 10, 5), selected, false), 41);
        // 相對：選取那週（10/12～18 都算）寫第幾週，其他週寫差幾週，前後都是正的
        assert_eq!(week_label(date(2026, 10, 12), selected, true), 42);
        assert_eq!(week_label(date(2026, 10, 18), selected, true), 42);
        assert_eq!(week_label(date(2026, 10, 11), selected, true), 1);
        assert_eq!(week_label(date(2026, 10, 26), selected, true), 2);
        // 跨年也照算：12/28 那週離 10/12 那週 11 週
        assert_eq!(week_label(date(2027, 1, 1), selected, true), 11);
    }

    #[test]
    fn month_rows_split_the_weeks_at_both_ends() {
        // 2026/11/1 是星期日、11/30 是星期一：頭尾兩列都只有一天
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
        // 2026/12/1 是星期二：第一列從第 2 格開始
        let december = month_rows(date(2026, 12, 1));
        assert_eq!(december.len(), 5);
        assert_eq!(december[0][0], None);
        assert_eq!(december[0][1], Some(date(2026, 12, 1)));
        assert_eq!(december[4][3], Some(date(2026, 12, 31)));
        assert_eq!(december[4][4], None);
        // 2021/2/1 是星期一、只有 28 天：剛好 4 列都是滿的
        let february = month_rows(date(2021, 2, 1));
        assert_eq!(february.len(), 4);
        assert!(february.iter().flatten().all(Option::is_some));
    }

    #[test]
    fn month_rows_put_every_day_once_under_its_weekday() {
        // 用另一種方式核對：每天剛好出現一次、在對的星期那格，而且每列都是同一週
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
        // 2026/10/17 是星期六 → 10/12；星期一自己就是；跨月、跨年
        assert_eq!(monday_of(date(2026, 10, 17)), date(2026, 10, 12));
        assert_eq!(monday_of(date(2026, 10, 12)), date(2026, 10, 12));
        assert_eq!(monday_of(date(2026, 11, 1)), date(2026, 10, 26));
        assert_eq!(monday_of(date(2027, 1, 3)), date(2026, 12, 28));
        // 用另一種方式核對：每天的星期一都是星期一、而且在 6 天以內
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
        // 跨年那週：看日期落在哪一年
        assert_eq!(week_of_year(date(2025, 12, 30)), 53);
        assert_eq!(week_of_year(date(2026, 1, 1)), 1);
        assert_eq!(week_of_year(date(2026, 1, 2)), 1);
        assert_eq!(week_of_year(date(2026, 12, 31)), 53);
        assert_eq!(week_of_year(date(2027, 1, 1)), 1);
        // 一般日期
        assert_eq!(week_of_year(date(2026, 10, 10)), 41);
        // 閏年又 1/1 是星期日，12/31 是第 54 週
        assert_eq!(week_of_year(date(2012, 12, 31)), 54);
    }

    #[test]
    fn week_of_year_matches_counting_mondays() {
        // 用另一種算法核對：週數 = 1 + 1/1 之後到這天為止出現過幾個星期一
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
