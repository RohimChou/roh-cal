//! 版型 1 的排版：每個月佔幾列、每一列的編號、捲動到哪（只算數字，不碰畫面）
//!
//! 一個月那一塊 = 週數和這個月的事排成幾列（分欄之後）比較多的那個，再加上後面那條線；
//! 每一列有一個編號：今天那個月的第一列是第 0 列，往下 1、2、3…，往上 -1、-2…

use crate::{
    app::App,
    calendar::{self, WeekRow},
    tasks::Item,
};
use jiff::{ToSpan, civil::Date};

/// 左邊欄寫什麼
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Label {
    /// 年份，例：`2027年`
    Year,
    /// 月份，例：`10月`
    Month,
}

/// 畫面上的一列（左右兩個框的同一列）
#[derive(Debug, PartialEq)]
pub enum Row<'a> {
    /// 月和月之間的線；`year` = 下一個月是 1 月（跨年），畫粗線
    Separator { year: bool },
    /// 一個月裡的一列：左邊的年 / 月（可能沒有）、這一週屬於這個月的日子、右邊每一欄的一件事（可能沒有）
    ///
    /// 這個月的事比週數多時，後面幾列只有右邊的事，`days` 是 `None`
    Week {
        label: Option<Label>,
        days: Option<WeekRow>,
        items: Vec<Option<&'a Item>>,
    },
}

/// 排版要用的資料：哪個月是第 0 列、有日期的事（照日期、時間排好）、右邊分幾欄
pub struct Months<'a> {
    /// 第 0 列所在的月（今天那個月）
    origin: Date,
    /// 有日期的事，照日期、時間排好（同一天沒有時間的在前；日期、時間一樣的照檔案裡的順序）
    items: Vec<&'a Item>,
    /// 右邊的事分幾欄
    columns: usize,
}

impl<'a> Months<'a> {
    pub fn new(app: &'a App, columns: usize) -> Self {
        Self {
            origin: app.today,
            items: app.agenda(),
            columns,
        }
    }

    /// `month` 那個月的事（`items` 已經照日期排好，所以是連續的一段）
    fn month_items(&self, month: Date) -> &[&'a Item] {
        let first = Some(month.first_of_month());
        let last = Some(month.last_of_month());
        let start = self.items.partition_point(|item| item.date < first);
        let end = self.items.partition_point(|item| item.date <= last);
        &self.items[start..end]
    }

    /// 一個月那一塊有幾列（不算後面那條線）：週數和事排成幾列，比較多的那個
    fn month_height(&self, month: Date) -> usize {
        let weeks = calendar::month_rows(month).len();
        weeks.max(self.month_items(month).len().div_ceil(self.columns))
    }

    /// 一個月那一塊有幾列，加上後面那條線
    fn block_height(&self, month: Date) -> i64 {
        i64::try_from(self.month_height(month)).unwrap_or(0) + 1
    }

    /// `month` 那一塊的第一列是第幾列
    pub fn block_start(&self, month: Date) -> i64 {
        let mut start = 0;
        let mut current = self.origin.first_of_month();
        let month = month.first_of_month();
        // 往後：一路加上前面每個月的高度；往前：一路減掉
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

    /// `date` 那一週是第幾列
    pub fn row_of(&self, date: Date) -> i64 {
        // 這一週是那個月的第幾週（從 0 開始）；週一定排在那個月最上面幾列
        let week = calendar::month_rows(date)
            .iter()
            .position(|row| row.contains(&Some(date)))
            .unwrap_or(0);
        self.block_start(date) + i64::try_from(week).unwrap_or(0)
    }

    /// 第 `row` 列落在哪個月那一塊：回傳那個月（1 號）和那一塊的第一列是第幾列
    pub fn block_at(&self, row: i64) -> (Date, i64) {
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

    /// 從 `first` 那個月開始一個月一個月往下排，排到至少 `height` 列（最後一個月可能超出去）
    pub fn rows(&self, first: Date, height: usize) -> Vec<Row<'a>> {
        let mut rows = Vec::new();
        let mut month = first.first_of_month();
        while rows.len() < height {
            if !rows.is_empty() {
                rows.push(Row::Separator {
                    year: month.month() == 1,
                });
            }
            let weeks = calendar::month_rows(month);
            let items = self.month_items(month);
            let month_height = self.month_height(month);
            // 1 月：第一列寫年份、第二列寫月份；其他月只在第一列寫月份
            let january = month.month() == 1;
            for i in 0..month_height {
                let label = match (january, i) {
                    (true, 0) => Some(Label::Year),
                    (true, 1) | (false, 0) => Some(Label::Month),
                    _ => None,
                };
                rows.push(Row::Week {
                    label,
                    days: weeks.get(i).copied(),
                    // 先排滿第一欄再排第二欄：第 c 欄的第 i 列 = 這個月的第 c × 高度 + i 件事
                    items: (0..self.columns)
                        .map(|c| items.get(c * month_height + i).copied())
                        .collect(),
                });
            }
            month = month.saturating_add(1.month());
        }
        rows
    }

    /// 右邊的 `item` 排在第幾列（鍵盤在右邊時，捲動要讓選到的那件事看得到）
    pub fn item_row(&self, item: &Item) -> Option<i64> {
        let month = item.date?.first_of_month();
        let index = self
            .month_items(month)
            .iter()
            .position(|other| std::ptr::eq(*other, item))?;
        // 先排滿第一欄再排第二欄：第 index 件在那一塊的第 index % 高度 列
        let row = index % self.month_height(month);
        Some(self.block_start(month) + i64::try_from(row).ok()?)
    }

    /// 第 `row` 列是什麼（滑鼠點的時候用）
    pub fn row_at(&self, row: i64) -> Option<Row<'a>> {
        let (month, start) = self.block_at(row);
        let skip = usize::try_from(row - start).ok()?;
        self.rows(month, skip + 1).into_iter().nth(skip)
    }
}

/// 捲動後畫面最上面是第幾列：畫面有 `height` 列、現在最上面是 `top`、選取的是第 `selected` 列
///
/// 選取的那列離上下邊緣至少留 `margin` 列，不夠才捲；例：10 列、留 2 列 → 選取的只能在畫面的第 3～8 列
pub fn scrolled_top(top: i64, selected: i64, height: i64, margin: i64) -> i64 {
    if height <= 0 {
        return selected;
    }
    // 畫面太矮時，留的列數最多到一半
    let margin = margin.min((height - 1) / 2);
    if selected < top + margin {
        selected - margin
    } else if selected + margin >= top + height {
        selected + margin + 1 - height
    } else {
        top
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::texts::Lang;
    use jiff::civil::date;

    /// 今天 2026/10/17、任務檔是 `text` 的 app
    fn app_with(text: &str) -> App {
        App::new(
            Lang::En,
            date(2026, 10, 17),
            crate::tasks::parse(text).items,
        )
    }

    #[test]
    fn row_numbers_count_from_this_month() {
        // 今天是 2026/10：10 月第一列是第 0 列；沒有事時，每個月那一塊 = 週數 + 1 條線
        let app = app_with("");
        let months = Months::new(&app, 1);
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
        // 每一塊最後一列是線：10 月的線是第 5 列、9 月的線是第 -1 列
        assert_eq!(months.block_at(5), (date(2026, 10, 1), 0));
        assert_eq!(months.block_at(6), (date(2026, 11, 1), 6));
        assert_eq!(months.block_at(-1), (date(2026, 9, 1), -6));
        assert_eq!(months.block_at(-6), (date(2026, 9, 1), -6));
        assert_eq!(months.block_at(-7), (date(2026, 8, 1), -13));
    }

    #[test]
    fn months_grow_with_their_items() {
        // 10 月 5 週、7 件事：一欄要 7 列，兩欄要 4 列（比 5 週少，所以還是 5 列）；9 月的事不算在 10 月
        let text = (1..=7)
            .map(|day| format!("- [ ] x @2026-10-{day:02}\n"))
            .collect::<String>()
            + "- [ ] 九月 @2026-09-30\n- [ ] 沒日期\n";
        let app = app_with(&text);
        let one = Months::new(&app, 1);
        assert_eq!(one.month_items(date(2026, 10, 1)).len(), 7);
        assert_eq!(one.block_height(date(2026, 10, 1)), 8);
        assert_eq!(one.block_start(date(2026, 11, 1)), 8);
        let two = Months::new(&app, 2);
        assert_eq!(two.block_height(date(2026, 10, 1)), 6);
        // 選取的那週還是在那個月最上面幾列
        assert_eq!(one.row_of(date(2026, 11, 7)), 9);
    }

    #[test]
    fn items_are_sorted_by_date_then_time_untimed_first() {
        // 同一天：沒時間的在前、有時間的照時間；日期時間一樣的照檔案順序
        let app = app_with(
            "- b @2026-10-16 17:30\n- a @2026-10-16 09:00\n- c @2026-10-16\n- d @2026-10-15\n\
             - e @2026-10-16 09:00\n",
        );
        let months = Months::new(&app, 1);
        let titles: Vec<&str> = months
            .month_items(date(2026, 10, 1))
            .iter()
            .map(|item| item.title.as_str())
            .collect();
        assert_eq!(titles, ["d", "c", "a", "e", "b"]);
    }

    #[test]
    fn rows_fill_the_first_column_before_the_second() {
        // 10 月 7 件事、兩欄：高度 5（週數比較多），第一欄 5 件、第二欄 2 件
        let text = (1..=7)
            .map(|day| format!("- [ ] {day} @2026-10-{day:02}\n"))
            .collect::<String>();
        let app = app_with(&text);
        let months = Months::new(&app, 2);
        let rows = months.rows(date(2026, 10, 1), 6);
        let titles: Vec<[&str; 2]> = rows[..5]
            .iter()
            .map(|row| match row {
                Row::Week { items, .. } => [0, 1].map(|c| items[c].map_or("", |item| &item.title)),
                Row::Separator { .. } => panic!("10 月的 5 列裡不該有線"),
            })
            .collect();
        assert_eq!(
            titles,
            [["1", "6"], ["2", "7"], ["3", ""], ["4", ""], ["5", ""]]
        );
        assert_eq!(rows[5], Row::Separator { year: false });
    }

    /// 每列的種類，方便比對：`-` = 細線、`=` = 粗線、`w` = 有日期的列、`.` = 只有事的列
    fn shape(rows: &[Row]) -> String {
        rows.iter()
            .map(|row| match row {
                Row::Separator { year: false } => '-',
                Row::Separator { year: true } => '=',
                Row::Week { days: Some(_), .. } => 'w',
                Row::Week { days: None, .. } => '.',
            })
            .collect()
    }

    #[test]
    fn months_with_more_items_than_weeks_get_rows_without_dates() {
        // 一欄：10 月 7 件事 → 5 週 + 2 列只有事；11 月沒事 → 6 週；12 月 → 5 週；跨年粗線；
        // 要 22 列時，最後的 1 月也整個排完（5 週）
        let text = (1..=7)
            .map(|day| format!("- [ ] x @2026-10-{day:02}\n"))
            .collect::<String>();
        let app = app_with(&text);
        let months = Months::new(&app, 1);
        assert_eq!(
            shape(&months.rows(date(2026, 10, 1), 22)),
            "wwwww..-wwwwww-wwwww=wwwww"
        );
    }

    #[test]
    fn block_at_finds_the_month_of_every_row() {
        // 用另一個方向核對：每一天的列，找回來都是它自己那個月；前後 5 年（每 3 天一筆，星期幾都會輪到）
        // 用範例任務檔，有些月比週數高
        let app = app_with(include_str!("../../testdata/tasks.md"));
        let months = Months::new(&app, 1);
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
        // 10 列、留 2 列：選取的在畫面的第 3～8 列（index 2～7）就不捲
        assert_eq!(scrolled_top(0, 5, 10, 2), 0);
        assert_eq!(scrolled_top(0, 2, 10, 2), 0);
        assert_eq!(scrolled_top(0, 7, 10, 2), 0);
        // 往下超過：選取的那列停在 index 7
        assert_eq!(scrolled_top(0, 8, 10, 2), 1);
        assert_eq!(scrolled_top(0, 30, 10, 2), 23);
        // 往上超過：選取的那列停在 index 2；可以捲到負的列（以前的月）
        assert_eq!(scrolled_top(20, 21, 10, 2), 19);
        assert_eq!(scrolled_top(0, 1, 10, 2), -1);
        assert_eq!(scrolled_top(0, -40, 10, 2), -42);
        // 畫面只有 3 列：最多留 1 列
        assert_eq!(scrolled_top(0, 5, 3, 2), 4);
    }
}
