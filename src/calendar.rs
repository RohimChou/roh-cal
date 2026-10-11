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

/// 捲動規則：選取的那週移動之後，畫面最上面那週要在哪
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Scroll {
    /// 選取的那週離上下邊緣至少保留幾週，不夠才捲（像 vim 的 scrolloff；0 = 掉出畫面才捲）
    Scrolloff(i64),
    /// 選取的那週固定在正中間那行（像 vim 的 scrolloff=999）
    Center,
}

/// 畫面最上面那週的新位置：畫面有 `weeks` 週、現在最上面是 `top`（星期一），選取的那週是 `week`（星期一）
///
/// 例：`Scrolloff(1)`、8 週 → 選取的那週只能在第 2～7 行；`Center`、6 週 → 永遠在第 3 行
pub fn scrolled_top(top: Date, week: Date, weeks: i64, scroll: Scroll) -> Date {
    match scroll {
        Scroll::Center => {
            // 中間那行的 index（0 起算）；偶數週數時偏上一行，下面多看一週
            let center = (weeks - 1) / 2;
            week.saturating_sub(center.weeks())
        }
        Scroll::Scrolloff(margin) => {
            // 選取的那週最上面 / 最下面可以在哪一行，換算成那行的星期一
            let last_row = weeks - 1 - margin;
            let min_week = top.saturating_add(margin.weeks());
            let max_week = top.saturating_add(last_row.weeks());
            if week < min_week {
                // 太靠上：往上捲，讓選取的那週剛好在第 margin + 1 行
                week.saturating_sub(margin.weeks())
            } else if week > max_week {
                // 太靠下：往下捲，讓選取的那週剛好在倒數第 margin + 1 行
                week.saturating_sub(last_row.weeks())
            } else {
                top
            }
        }
    }
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
    fn scrolled_top_keeps_the_margin_or_centers() {
        // 最上面 9/21，8 週：選取 10/5（第 3 行）在範圍內不動；
        // scrolloff 1 時選取 9/21（第 1 行）往上捲一週、11/9（第 8 行）往下捲一週；
        // scrolloff 0 時第 1 行、第 8 行都不捲，掉出去（11/16）才捲
        let top = date(2026, 9, 21);
        let off1 = Scroll::Scrolloff(1);
        assert_eq!(scrolled_top(top, date(2026, 10, 5), 8, off1), top);
        assert_eq!(
            scrolled_top(top, date(2026, 9, 21), 8, off1),
            date(2026, 9, 14)
        );
        assert_eq!(
            scrolled_top(top, date(2026, 11, 9), 8, off1),
            date(2026, 9, 28)
        );
        let off0 = Scroll::Scrolloff(0);
        assert_eq!(scrolled_top(top, date(2026, 9, 21), 8, off0), top);
        assert_eq!(scrolled_top(top, date(2026, 11, 9), 8, off0), top);
        assert_eq!(
            scrolled_top(top, date(2026, 11, 16), 8, off0),
            date(2026, 9, 28)
        );
        // 置中：8 週在第 4 行、6 週在第 3 行，跟原本的 top 無關
        assert_eq!(
            scrolled_top(top, date(2026, 10, 5), 8, Scroll::Center),
            date(2026, 9, 14)
        );
        assert_eq!(
            scrolled_top(top, date(2026, 10, 5), 6, Scroll::Center),
            date(2026, 9, 21)
        );
    }

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
