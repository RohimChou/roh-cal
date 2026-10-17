//! 畫面用的項目：任務檔裡有日期的事，照日期、時間排好

use crate::tasks;
use jiff::civil::{Date, Time};

/// 一件事：哪天、幾點（可以沒有）、哪個日曆（分組）、標題、是不是假日、做完了沒
pub struct Item {
    pub date: Date,
    pub time: Option<Time>,
    /// 分組名稱；檔案開頭還沒有標題的項目是空字串
    pub group: String,
    pub title: String,
    pub holiday: bool,
    pub done: bool,
}

/// 任務檔裡的分組名稱（side menu 的日曆），照第一次出現的順序
pub fn groups(parsed: &[tasks::Item]) -> Vec<String> {
    let mut groups: Vec<String> = Vec::new();
    for name in parsed.iter().map(group_name) {
        if !groups.contains(&name) {
            groups.push(name);
        }
    }
    groups
}

/// 有日期的項目，照日期、時間排好（同一天沒有時間的在前；日期、時間一樣的照檔案裡的順序）
///
/// 沒有日期的項目先不顯示（月曆上沒有位置放）
pub fn from_tasks(parsed: &[tasks::Item]) -> Vec<Item> {
    let mut items: Vec<Item> = parsed
        .iter()
        .filter_map(|item| {
            Some(Item {
                date: item.date?,
                time: item.time,
                group: group_name(item),
                title: item.title.clone(),
                holiday: false,
                done: item.done,
            })
        })
        .collect();
    // sort_by_key 是 stable sort：key 一樣的保持原本的順序；None 排在 Some 前面
    items.sort_by_key(|item| (item.date, item.time));
    items
}

/// 項目的分組名稱；沒有分組是空字串
fn group_name(item: &tasks::Item) -> String {
    item.group
        .as_ref()
        .map(|heading| heading.name.clone())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use jiff::civil::{date, time};

    #[test]
    fn dated_items_are_sorted_and_groups_keep_file_order() {
        let parsed = tasks::parse(
            "- [ ] 沒分組 @2026-10-20\n\
             # 工作\n\
             - [x] 下午的會 @2026-10-12 14:00\n\
             - [ ] 整天的事 @2026-10-12\n\
             - [ ] 沒有日期\n\
             # 生活\n\
             - [ ] 早上的事 @2026-10-12 09:00\n",
        );
        assert_eq!(groups(&parsed.items), ["", "工作", "生活"]);
        let items = from_tasks(&parsed.items);
        // 沒有日期的不算；同一天：沒時間的在前，有時間的照時間
        let order: Vec<(&str, Date, Option<Time>)> = items
            .iter()
            .map(|item| (item.title.as_str(), item.date, item.time))
            .collect();
        assert_eq!(
            order,
            [
                ("整天的事", date(2026, 10, 12), None),
                ("早上的事", date(2026, 10, 12), Some(time(9, 0, 0, 0))),
                ("下午的會", date(2026, 10, 12), Some(time(14, 0, 0, 0))),
                ("沒分組", date(2026, 10, 20), None),
            ]
        );
        assert!(items[2].done);
        assert_eq!(items[2].group, "工作");
    }

    #[test]
    fn sample_file_has_no_warnings_and_every_item_has_a_date() {
        // 預期數字是另外用 Python 數範例檔得到的
        let parsed = tasks::parse(include_str!("../testdata/sample.md"));
        assert!(parsed.warnings.is_empty(), "{:?}", parsed.warnings);
        assert_eq!(parsed.items.len(), 100);
        assert_eq!(from_tasks(&parsed.items).len(), 100);
        assert_eq!(groups(&parsed.items), ["工作", "學校", "生活", "假日"]);
    }
}
