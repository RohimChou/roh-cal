//! 純日期計算：不碰畫面、不碰 App，只吃日期、回傳日期或數字

use jiff::{ToSpan, civil::Date};

/// 顯示 `date` 那個月時，畫面最上面那週的星期一：1 號那週的前一週
pub fn month_top(date: Date) -> Date {
    monday_of(date.first_of_month()).saturating_sub(1.week())
}

/// `date` 那一週的星期一
pub fn monday_of(date: Date) -> Date {
    date.saturating_sub(date.weekday().to_monday_zero_offset().days())
}

/// `date` 是它那一年的第幾週：1/1 所在的那週是第 1 週（每週從星期一開始）
///
/// 跨年那週看 `date` 落在哪一年，例：2025-12-30 是 2025 年第 53 週、2026-01-02 是 2026 年第 1 週
pub fn week_of_year(date: Date) -> i16 {
    // 1/1 那週裡，1/1 前面還有幾天（例：1/1 是星期四 → 前面有 3 天）
    let offset = date.first_of_year().weekday().to_monday_zero_offset();
    (date.day_of_year() - 1 + i16::from(offset)) / 7 + 1
}

#[cfg(test)]
mod tests {
    use super::*;
    use jiff::civil::{Weekday, date};

    #[test]
    fn week_of_year_cross_year_follows_selected_date() {
        // 跨年那週：看選取的日期落在哪一年
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
