//! 改任務檔的純 function：給檔案現在的內容，回傳改好的新內容（寫檔由呼叫的人用 `files::write_atomically`）
//!
//! 只動要改的那一行，其他每個字（包括換行是 `\n` 還是 `\r\n`）都不動。
//! 改之前先比對：那一行跟讀進來時不一樣（例：剛在 Obsidian 改過）就不改，回傳 `Changed`

use crate::tasks::{self, Item, WarningKind};
use jiff::{ToSpan, civil::Date};

/// 那一行跟讀進來時不一樣（被別人改過、上面多了一行、檔案變短了），不能改；要先重新讀檔
#[derive(Debug, PartialEq, Eq)]
pub struct Changed;

/// 編輯框裡打的一行有什麼問題（存不了）
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Problem {
    /// 不是 `- ` 開頭（不是一件事）
    NotAnItem,
    /// 寫錯的地方，例：`@2026-13-01`（parser 會把它當成一般文字，存了就看不到日期）
    Mistake(WarningKind),
}

/// 編輯框裡打的一行（不含縮排）讀起來是什麼：正好是一件事、沒有寫錯的地方才算數
///
/// 例：`- [ ] 交週報 @2026-10-16 17:30` → 那件事；`交週報` → `NotAnItem`；`- 出差 @2026-13-01` → `Mistake`
pub fn check(line: &str) -> Result<Item, Problem> {
    let line = line.trim();
    if !line.starts_with("- ") && line != "-" {
        return Err(Problem::NotAnItem);
    }
    let parsed = tasks::parse(line);
    if let Some(warning) = parsed.warnings.into_iter().next() {
        return Err(Problem::Mistake(warning.kind));
    }
    parsed.items.into_iter().next().ok_or(Problem::NotAnItem)
}

/// 編輯：`item` 那一行換成 `new`（編輯框裡打的那一行，不含縮排），縮排照原本的；回傳改好的整個檔案
///
/// 呼叫的人要先用 `check` 確認 `new` 沒問題
pub fn replace(text: &str, item: &Item, new: &str) -> Result<String, Changed> {
    let indent = &item.line_text[..item.line_text.len() - item.line_text.trim_start().len()];
    replace_line(text, item, &format!("{indent}{}", new.trim()))
}

/// 任務打勾 / 取消：`- [ ] 週會` ↔ `- [x] 週會`，回傳打勾後的整個檔案
///
/// `text` = 任務檔現在的內容、`item` = 之前讀到的那個項目。行程沒有框、不能打勾：那一行照原樣（呼叫的人要先擋掉）
pub fn toggle(text: &str, item: &Item) -> Result<String, Changed> {
    replace_line(text, item, &toggled(&item.line_text))
}

/// 改期：只換掉那一行的日期，其他字一字不動，回傳改好的整個檔案
///
/// 時間照留；跨多天的整段一起搬、天數不變（例：`@2026-10-20..2026-10-25` 改到 11/2 → `@2026-11-02..2026-11-07`）。
/// 還沒排日期的：日期加在行尾（行尾有 Obsidian 的 `^id` 就加在它前面）
pub fn reschedule(text: &str, item: &Item, to: Date) -> Result<String, Changed> {
    replace_line(text, item, &rescheduled(&item.line_text, item, to))
}

/// 一行的日期換成 `to`（見 `reschedule`）
fn rescheduled(line: &str, item: &Item, to: Date) -> String {
    let Some(from) = item.date else {
        // 還沒排日期：加在行尾，`^id` 要留在最後面
        let (body, block_id) = match line.rsplit_once(' ') {
            Some((body, last)) if tasks::is_block_id(last) => (body, format!(" {last}")),
            _ => (line, String::new()),
        };
        return format!("{} @{to}{block_id}", body.trim_end());
    };
    // 新的日期：跨多天的最後一天一起搬
    let new_token = match item.end {
        Some(end) => {
            let days = (to - from).get_days();
            format!("@{to}..{}", end.saturating_add(days.days()))
        }
        None => format!("@{to}"),
    };
    // 檔案裡原本的寫法：parser 認的是第一個合法的日期，寫出來一定是標準的 YYYY-MM-DD；
    // 頭尾同一天的範圍（`@10-20..10-20`）parser 當成一天，兩種寫法都找
    let old_tokens = match item.end {
        Some(end) => vec![format!("@{from}..{end}")],
        None => vec![format!("@{from}"), format!("@{from}..{from}")],
    };
    for old in old_tokens {
        if let Some(start) = whole_word(line, &old) {
            return format!(
                "{}{new_token}{}",
                &line[..start],
                &line[start + old.len()..]
            );
        }
    }
    // 找不到（不會發生：parser 讀得到日期就一定在這一行裡）：照原樣
    line.to_string()
}

/// `word` 第一次以「整個字」出現在 `line` 的哪裡（前後是空白或行頭行尾）
fn whole_word(line: &str, word: &str) -> Option<usize> {
    line.match_indices(word)
        .map(|(start, _)| start)
        .find(|&start| {
            let before = line[..start].chars().next_back();
            let after = line[start + word.len()..].chars().next();
            before.is_none_or(char::is_whitespace) && after.is_none_or(char::is_whitespace)
        })
}

/// 一行打勾 / 取消，縮排和後面的文字一字不動（`line` 是 parser 認過的項目那一行）
///
/// `- [ ] 週會` → `- [x] 週會`；`- [x]`、`- [X]` → `- [ ]`；沒有框的（行程）照原樣
fn toggled(line: &str) -> String {
    let trimmed = line.trim_start();
    let indent = &line[..line.len() - trimmed.len()];
    let rest = trimmed.strip_prefix("- ").unwrap_or(trimmed);
    match tasks::checkbox(rest) {
        // 有框：只換 `[ ]` / `[x]` 那 3 個字
        Some((done, after)) => {
            let mark = if done { "[ ]" } else { "[x]" };
            format!("{indent}- {mark}{after}")
        }
        None => line.to_string(),
    }
}

/// 把 `item` 那一行換成 `new`；那一行現在跟 `item.line_text` 不一樣（或已經沒有那一行）就回傳 `Changed`
fn replace_line(text: &str, item: &Item, new: &str) -> Result<String, Changed> {
    let mut result = String::with_capacity(text.len() + new.len());
    let mut replaced = false;
    // lines() 拿內容（跟 parser 同一種切法），split_inclusive 拿連換行的整段，換行（\n 或 \r\n）才能照原樣留著
    for (index, (content, segment)) in text.lines().zip(text.split_inclusive('\n')).enumerate() {
        if index + 1 != item.line {
            result.push_str(segment);
            continue;
        }
        if content != item.line_text {
            return Err(Changed);
        }
        result.push_str(new);
        result.push_str(&segment[content.len()..]);
        replaced = true;
    }
    if replaced { Ok(result) } else { Err(Changed) }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tasks::parse;
    use jiff::civil::date;

    /// `text` 裡的第 `index` 個項目
    fn item(text: &str, index: usize) -> Item {
        parse(text).items[index].clone()
    }

    #[test]
    fn check_accepts_exactly_one_item_without_mistakes() {
        let item = check("  - [ ] 交週報 @2026-10-16 17:30  ").expect("一件事");
        assert_eq!(
            (item.title.as_str(), item.date),
            ("交週報", Some(date(2026, 10, 16)))
        );
        // 不是 `- ` 開頭：標題、清單的別種寫法、空的
        for line in ["交週報", "# 例行", "* 交週報", ""] {
            assert_eq!(check(line), Err(Problem::NotAnItem), "{line:?}");
        }
        // 寫錯的日期、時間、範圍
        assert_eq!(
            check("- 出差 @2026-13-01"),
            Err(Problem::Mistake(WarningKind::NoSuchDate(
                "@2026-13-01".into()
            )))
        );
        assert_eq!(
            check("- 會議 @2026-10-12 25:00"),
            Err(Problem::Mistake(WarningKind::NoSuchTime("25:00".into())))
        );
    }

    #[test]
    fn replace_keeps_the_indentation() {
        let text = "- [ ] 打包\n  - [ ] 書房 @2026-11-14\n";
        assert_eq!(
            replace(text, &item(text, 1), "- [x] 書房和客廳 @2026-11-15 "),
            Ok("- [ ] 打包\n  - [x] 書房和客廳 @2026-11-15\n".into())
        );
        // 那一行在別處改過：不改
        assert_eq!(
            replace("- [ ] 打包\n  - [ ] 臥室\n", &item(text, 1), "- [ ] x"),
            Err(Changed)
        );
    }

    #[test]
    fn reschedule_changes_only_the_date() {
        let text = "# 例行\n- [ ] 交週報 @2026-10-16 17:30 ^w-01\n- 開會 @2026-10-12 跟客戶\n";
        // 時間、行尾的 ^id 都照留
        assert_eq!(
            reschedule(text, &item(text, 0), date(2026, 10, 19)),
            Ok("# 例行\n- [ ] 交週報 @2026-10-19 17:30 ^w-01\n- 開會 @2026-10-12 跟客戶\n".into())
        );
        // 日期在中間：只換那個字，位置不動
        assert_eq!(
            reschedule(text, &item(text, 1), date(2026, 11, 2)),
            Ok("# 例行\n- [ ] 交週報 @2026-10-16 17:30 ^w-01\n- 開會 @2026-11-02 跟客戶\n".into())
        );
    }

    #[test]
    fn reschedule_moves_a_multi_day_item_as_a_whole() {
        // 6 天的出差改到 10/29 開始：最後一天跟著搬到 11/3（跨月）；往前搬也一樣
        let text = "- 東京出差 @2026-10-20..2026-10-25 09:00\n";
        assert_eq!(
            reschedule(text, &item(text, 0), date(2026, 10, 29)),
            Ok("- 東京出差 @2026-10-29..2026-11-03 09:00\n".into())
        );
        assert_eq!(
            reschedule(text, &item(text, 0), date(2026, 10, 1)),
            Ok("- 東京出差 @2026-10-01..2026-10-06 09:00\n".into())
        );
        // 頭尾同一天的寫法（parser 當成一天）：換成一般的日期
        let text = "- 開會 @2026-10-20..2026-10-20\n";
        assert_eq!(
            reschedule(text, &item(text, 0), date(2026, 10, 21)),
            Ok("- 開會 @2026-10-21\n".into())
        );
    }

    #[test]
    fn reschedule_skips_text_that_only_looks_like_the_date() {
        // 第一個 @2026-13-01 不存在（parser 當文字）、`x@2026-10-16` 不是整個字：換的是真正的那個日期
        let text = "- [ ] 繳稅 @2026-13-01 x@2026-10-16 @2026-10-16\n";
        assert_eq!(
            reschedule(text, &item(text, 0), date(2026, 10, 20)),
            Ok("- [ ] 繳稅 @2026-13-01 x@2026-10-16 @2026-10-20\n".into())
        );
    }

    #[test]
    fn reschedule_adds_a_date_to_an_unscheduled_item() {
        let text = "- [ ] 整理書櫃\n- [ ] 回信 ^r-02\n";
        assert_eq!(
            reschedule(text, &item(text, 0), date(2026, 10, 18)),
            Ok("- [ ] 整理書櫃 @2026-10-18\n- [ ] 回信 ^r-02\n".into())
        );
        // 行尾有 ^id：日期加在它前面
        assert_eq!(
            reschedule(text, &item(text, 1), date(2026, 10, 18)),
            Ok("- [ ] 整理書櫃\n- [ ] 回信 @2026-10-18 ^r-02\n".into())
        );
    }

    #[test]
    fn toggle_flips_only_the_box_of_that_line() {
        let text = "# 例行\n- [ ] 週報 @2026-10-16\n  - [x] 子任務\n- [X] 大寫 @2026-10-17\n";
        assert_eq!(
            toggle(text, &item(text, 0)),
            Ok("# 例行\n- [x] 週報 @2026-10-16\n  - [x] 子任務\n- [X] 大寫 @2026-10-17\n".into())
        );
        // 縮排照留；`[x]` 和 `[X]` 都變回 `[ ]`
        assert_eq!(
            toggle(text, &item(text, 1)),
            Ok("# 例行\n- [ ] 週報 @2026-10-16\n  - [ ] 子任務\n- [X] 大寫 @2026-10-17\n".into())
        );
        assert_eq!(
            toggle(text, &item(text, 2)),
            Ok("# 例行\n- [ ] 週報 @2026-10-16\n  - [x] 子任務\n- [ ] 大寫 @2026-10-17\n".into())
        );
    }

    #[test]
    fn toggle_keeps_crlf_and_a_missing_last_newline() {
        let text = "- [ ] a\r\n- [ ] b";
        assert_eq!(
            toggle(text, &item(text, 0)),
            Ok("- [x] a\r\n- [ ] b".into())
        );
        assert_eq!(
            toggle(text, &item(text, 1)),
            Ok("- [ ] a\r\n- [x] b".into())
        );
    }

    #[test]
    fn toggle_leaves_an_event_as_it_is() {
        let text = "- 看牙醫 @2026-10-14\n";
        assert_eq!(toggle(text, &item(text, 0)), Ok(text.into()));
    }

    #[test]
    fn toggle_refuses_when_the_line_changed() {
        let text = "- [ ] a\n- [ ] b\n";
        let b = item(text, 1);
        // 在別處：上面多了一行（b 往下移）、b 被改了字、檔案被砍短
        for changed in [
            "- [ ] new\n- [ ] a\n- [ ] b\n",
            "- [ ] a\n- [ ] b!\n",
            "- [ ] a\n",
        ] {
            assert_eq!(toggle(changed, &b), Err(Changed), "{changed:?}");
        }
    }
}
