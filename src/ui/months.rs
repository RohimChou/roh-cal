//! 版型 1 的排版：每個月佔幾列、每一列的編號、捲動到哪（只算數字，不碰畫面）
//!
//! 一個月那一塊 = 週數和這個月的事排成幾列（分欄之後）比較多的那個，但最多 `max_height` 列（至少是週數），
//! 再加上後面那條線；放不下的事在那個月的右邊自己捲（`window`）。
//! 每一列有一個編號：今天那個月的第一列是第 0 列，往下 1、2、3…，往上 -1、-2…

use crate::{
    app::{App, Entry},
    calendar::{self, WeekRow},
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
    /// 月和月之間的線；`year` = 下一個月是 1 月（跨年），畫粗線；
    /// `hidden` = 上面那個月的右邊，下面還有幾件沒顯示出來（要捲才看得到）
    Separator { year: bool, hidden: usize },
    /// 一個月裡的一列：左邊的年 / 月（可能沒有）、這一週屬於這個月的日子、右邊每一欄的一行（可能沒有）、
    /// 右框線的捲軸
    ///
    /// 這個月的事比週數多時，後面幾列只有右邊的事，`days` 是 `None`；
    /// 這個月的事放不下（右邊要捲）時才有捲軸：`thumb` 是 `Some(true)` = 這一列是捲軸的亮塊
    Week {
        label: Option<Label>,
        days: Option<WeekRow>,
        items: Vec<Option<Entry<'a>>>,
        thumb: Option<bool>,
    },
}

/// 排版要用的資料：哪個月是第 0 列、有日期的事（照日期、時間排好）、右邊分幾欄、一個月最多幾列、右邊捲到哪
pub struct Months<'a> {
    /// 第 0 列所在的月（今天那個月）
    origin: Date,
    /// 右邊的每一行（`App::agenda()`），照列在哪天、時間排好
    items: Vec<Entry<'a>>,
    /// 右邊的事分幾欄
    columns: usize,
    /// 一個月最多幾列（比那個月的週數少時，以週數為準）
    max_height: usize,
    /// 右邊捲到哪：(哪個月的 1 號, 那個月右邊最上面是排好的第幾列)；其他月從第一列開始
    pub window: Option<(Date, usize)>,
}

impl<'a> Months<'a> {
    pub fn new(app: &'a App, columns: usize, max_height: usize) -> Self {
        Self {
            origin: app.today,
            items: app.agenda(),
            columns,
            max_height,
            window: None,
        }
    }

    /// `month` 那個月列的事（`items` 已經照列在哪天排好，所以是連續的一段）
    fn month_items(&self, month: Date) -> &[Entry<'a>] {
        let first = month.first_of_month();
        let last = month.last_of_month();
        let start = self.items.partition_point(|entry| entry.date < first);
        let end = self.items.partition_point(|entry| entry.date <= last);
        &self.items[start..end]
    }

    /// `date` 那天的事（跨多天的：蓋到那天就算，列在那個月的那一行）
    pub fn day_items(&self, date: Date) -> Vec<Entry<'a>> {
        self.month_items(date)
            .iter()
            .filter(|entry| entry.is_on(date))
            .copied()
            .collect()
    }

    /// 這個月的事排成幾列（不限高；分欄時是每一欄的高度）：週數和事排成幾列，比較多的那個
    fn layout_height(&self, month: Date) -> usize {
        let weeks = calendar::month_rows(month).len();
        weeks.max(self.month_items(month).len().div_ceil(self.columns))
    }

    /// 一個月那一塊有幾列（不算後面那條線）：排成幾列，但最多 `max_height` 列（至少是週數）
    fn month_height(&self, month: Date) -> usize {
        let weeks = calendar::month_rows(month).len();
        self.layout_height(month).min(weeks.max(self.max_height))
    }

    /// `month` 的右邊捲到第幾列：選取的那個月照 `window`，其他月從 0 開始；最多捲到最後一列貼著那一塊的底
    fn offset(&self, month: Date) -> usize {
        let max = self.layout_height(month) - self.month_height(month);
        self.window
            .filter(|(window_month, _)| *window_month == month.first_of_month())
            .map_or(0, |(_, offset)| offset.min(max))
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
        // 上一個月下面還有幾件沒顯示出來（寫在這個月上面那條線上）
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
            // 1 月：第一列寫年份、第二列寫月份；其他月只在第一列寫月份
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
                    // 先排滿第一欄再排第二欄（每欄 `layout_height` 列），右邊捲了 `offset` 列：
                    // 第 c 欄的第 i 列 = 這個月的第 c × 每欄列數 + offset + i 件事
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

    /// `month` 的右邊，捲過之後下面還有幾件沒顯示出來（每一欄都算）
    fn hidden_below(&self, month: Date) -> usize {
        let count = self.month_items(month).len();
        let layout_height = self.layout_height(month);
        // 看得到的最後一列的下一列
        let end = self.offset(month) + self.month_height(month);
        (0..self.columns)
            .map(|c| {
                // 第 c 欄有幾件（每欄最多 `layout_height` 件）
                let in_column = count.saturating_sub(c * layout_height).min(layout_height);
                in_column.saturating_sub(end)
            })
            .sum()
    }

    /// `entry` 排在它那個月的哪一列（還沒捲；分欄時是在它那一欄的第幾列）：回傳那個月（1 號）和第幾列
    fn layout_row(&self, entry: Entry) -> Option<(Date, usize)> {
        let month = entry.date.first_of_month();
        let index = self
            .month_items(month)
            .iter()
            .position(|other| *other == entry)?;
        Some((month, index % self.layout_height(month)))
    }

    /// `month` 的右邊要捲到第幾列，才看得到 `targets` 那幾件事：從 `previous` 開始，
    /// 前後盡量多留 1 列（看得到前後還有什麼）；看得到就不捲，免得右邊跳來跳去；放不下時先顧最上面那件
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

    /// 捲動後畫面最上面是第幾列：選取那週離上下邊緣至少留 `margin` 列（見 `scrolled_top`），
    /// 再讓 `targets` 那幾件事（照 `window` 捲過之後的位置）也看得到（不用留邊）；放不下時先顧最上面那個
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
            // 那件事在那一塊裡畫在第幾列（右邊捲走、看不到的就不算）
            let shown = row.checked_sub(self.offset(month));
            if let Some(shown) = shown.filter(|&shown| shown < self.month_height(month)) {
                rows.push(self.block_start(month) + i64::try_from(shown).unwrap_or(0));
            }
        }
        let first = rows.iter().copied().min().unwrap_or(week);
        let last = rows.iter().copied().max().unwrap_or(week);
        scrolled_to_range(top, first, last, height, 0)
    }

    /// 畫面上看得到的列：從第 `top` 列開始的 `height` 列（畫畫面和滑鼠換算都用這個）
    pub fn visible_rows(&self, top: i64, height: usize) -> Vec<Row<'a>> {
        // 從最上面那列所在的那個月開始排，前 `skip` 列在畫面上面
        let (month, start) = self.block_at(top);
        let skip = usize::try_from(top - start).unwrap_or(0);
        let mut rows = self.rows(month, skip + height);
        rows.drain(..skip);
        rows.truncate(height);
        rows
    }
}

/// 捲軸的亮塊在那一塊的哪幾列：排起來 `layout` 列、看得到 `height` 列、捲了 `offset` 列；全部看得到就沒有捲軸
///
/// 亮塊長度 = `height` × 看得到的比例（至少 1 列）；捲到底時亮塊剛好貼底。例：8 件放 5 列、捲了 3 列 → 第 2～4 列
fn thumb_rows(layout: usize, height: usize, offset: usize) -> Option<std::ops::Range<usize>> {
    if layout <= height {
        return None;
    }
    let length = (height * height / layout).max(1);
    let start = offset * (height - length) / (layout - height);
    Some(start..start + length)
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

/// 捲動後畫面最上面是第幾列：讓第 `first`～`last` 列都看得到，上下盡量各留 `margin` 列
/// （連邊一起放不下就把邊縮小）；還是放不下就先顧 `first`
fn scrolled_to_range(top: i64, first: i64, last: i64, height: i64, margin: i64) -> i64 {
    let margin = margin.min((height - (last - first + 1)) / 2).max(0);
    scrolled_top(
        scrolled_top(top, last, height, margin),
        first,
        height,
        margin,
    )
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
        let months = Months::new(&app, 1, 100);
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
        let one = Months::new(&app, 1, 100);
        assert_eq!(one.month_items(date(2026, 10, 1)).len(), 7);
        assert_eq!(one.block_height(date(2026, 10, 1)), 8);
        assert_eq!(one.block_start(date(2026, 11, 1)), 8);
        let two = Months::new(&app, 2, 100);
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
        let months = Months::new(&app, 1, 100);
        let titles: Vec<&str> = months
            .month_items(date(2026, 10, 1))
            .iter()
            .map(|entry| entry.item.title.as_str())
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
        let months = Months::new(&app, 2, 100);
        let rows = months.rows(date(2026, 10, 1), 6);
        let titles: Vec<[&str; 2]> = rows[..5]
            .iter()
            .map(|row| match row {
                Row::Week { items, .. } => {
                    [0, 1].map(|c| items[c].map_or("", |entry| &entry.item.title))
                }
                Row::Separator { .. } => panic!("10 月的 5 列裡不該有線"),
            })
            .collect();
        assert_eq!(
            titles,
            [["1", "6"], ["2", "7"], ["3", ""], ["4", ""], ["5", ""]]
        );
        assert_eq!(
            rows[5],
            Row::Separator {
                year: false,
                hidden: 0
            }
        );
    }

    /// 每列的種類，方便比對：`-` = 細線、`=` = 粗線、`w` = 有日期的列、`.` = 只有事的列
    fn shape(rows: &[Row]) -> String {
        rows.iter()
            .map(|row| match row {
                Row::Separator { year: false, .. } => '-',
                Row::Separator { year: true, .. } => '=',
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
        let months = Months::new(&app, 1, 100);
        assert_eq!(
            shape(&months.rows(date(2026, 10, 1), 22)),
            "wwwww..-wwwwww-wwwww=wwwww"
        );
    }

    #[test]
    fn months_are_capped_and_scroll_inside() {
        // 10 月 20 件事（一欄）：排起來 20 列；最多 8 列 → 那一塊 8 列，11 月從第 9 列開始
        let text = (1..=20)
            .map(|day| format!("- [ ] {day} @2026-10-{day:02}\n"))
            .collect::<String>();
        let app = app_with(&text);
        let mut months = Months::new(&app, 1, 8);
        assert_eq!(months.month_height(date(2026, 10, 1)), 8);
        assert_eq!(months.block_start(date(2026, 11, 1)), 9);
        // 上限比週數少：至少是週數（10 月 5 週）
        assert_eq!(Months::new(&app, 1, 3).month_height(date(2026, 10, 1)), 5);
        let agenda = app.agenda();
        let october = date(2026, 10, 1);
        // 要看第 15 件（排在第 14 列）：右邊捲到第 8 列，下面留 1 列
        assert_eq!(months.window_for(october, 0, &[agenda[14]]), 8);
        // 已經看得到就不捲；沒有要看的就照舊；最多捲到最後一件貼著底（20 - 8 = 12）
        assert_eq!(months.window_for(october, 8, &[agenda[10]]), 8);
        assert_eq!(months.window_for(october, 8, &[]), 8);
        assert_eq!(months.window_for(october, 50, &[]), 12);
        // 捲了 8 列：10 月第一列右邊是第 9 件；別的月不受影響
        months.window = Some((october, 8));
        let rows = months.rows(october, 9);
        let Row::Week { items, .. } = &rows[0] else {
            panic!("第一列應該是 10 月");
        };
        assert_eq!(items[0].map(|entry| entry.item.title.as_str()), Some("9"));
        // 10 月下面那條線：還有 20 - 8 - 8 = 4 件沒顯示出來
        assert_eq!(
            rows[8],
            Row::Separator {
                year: false,
                hidden: 4
            }
        );
    }

    #[test]
    fn the_scrollbar_thumb_shows_how_much_and_where() {
        // 全部看得到：沒有捲軸
        assert_eq!(thumb_rows(5, 5, 0), None);
        // 8 件放 5 列：亮塊 5 × 5 / 8 = 3 列；沒捲在最上面、捲到底（3 列）貼底、捲一半在中間
        assert_eq!(thumb_rows(8, 5, 0), Some(0..3));
        assert_eq!(thumb_rows(8, 5, 3), Some(2..5));
        assert_eq!(thumb_rows(20, 8, 6), Some(2..5));
        // 很多件：亮塊至少 1 列
        assert_eq!(thumb_rows(100, 5, 95), Some(4..5));
        // 排出來的列也帶著捲軸：10 月 20 件、最多 8 列、沒捲 → 前 3 列是亮塊；11 月沒有捲軸
        let text = (1..=20)
            .map(|day| format!("- [ ] {day} @2026-10-{day:02}\n"))
            .collect::<String>();
        let app = app_with(&text);
        let months = Months::new(&app, 1, 8);
        let thumbs: Vec<Option<bool>> = months
            .rows(date(2026, 10, 1), 10)
            .iter()
            .filter_map(|row| match row {
                Row::Week { thumb, .. } => Some(*thumb),
                Row::Separator { .. } => None,
            })
            .collect();
        assert_eq!(
            &thumbs[..8],
            [true, true, true, false, false, false, false, false].map(Some)
        );
        assert_eq!(thumbs[8], None);
    }

    #[test]
    fn block_at_finds_the_month_of_every_row() {
        // 用另一個方向核對：每一天的列，找回來都是它自己那個月；前後 5 年（每 3 天一筆，星期幾都會輪到）
        // 用範例任務檔，有些月比週數高
        let app = app_with(include_str!("../../testdata/tasks.md"));
        let months = Months::new(&app, 1, 100);
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
