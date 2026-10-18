//! 改任務檔的純 function：給檔案現在的內容，回傳改好的新內容（寫檔由呼叫的人用 `files::write_atomically`）
//!
//! 只動要改的地方（打勾、編輯、改期：那一行；刪除：那一塊；新增：插一行），其他每個字（包括換行是 `\n` 還是 `\r\n`）都不動。
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
    /// 標題是空的（例：只打了 `- [ ] @2026-10-17`）
    EmptyTitle,
}

/// 新增的那一行放在檔案的哪裡
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Place {
    /// `item` 那一組（同一個標題底下）的最後一件和它的子任務下面
    InGroupOf(Item),
    /// `item` 的子任務：放在它和它原本的子任務下面，縮排比它多 2 格
    ChildOf(Item),
    /// 檔案最後面
    End,
}

/// 編輯框裡打的一行（不含縮排）讀起來是什麼：正好是一件事、沒有寫錯的地方才算數
///
/// 例：`- [ ] 交週報 @2026-10-16 17:30` → 那件事；`交週報` → `NotAnItem`；`- 出差 @2026-13-01` → `Mistake`
pub fn check(line: &str) -> Result<Item, Problem> {
    let line = line.trim();
    // 只打了 `- `（去掉頭尾空白剩 `-`）：是一件事，只是還沒有標題
    if line == "-" {
        return Err(Problem::EmptyTitle);
    }
    if !line.starts_with("- ") {
        return Err(Problem::NotAnItem);
    }
    let parsed = tasks::parse(line);
    if let Some(warning) = parsed.warnings.into_iter().next() {
        return Err(Problem::Mistake(warning.kind));
    }
    let item = parsed.items.into_iter().next().ok_or(Problem::NotAnItem)?;
    if item.title.is_empty() {
        return Err(Problem::EmptyTitle);
    }
    Ok(item)
}

/// 新增：`new`（編輯框裡打的那一行）放到 `place`，回傳新的檔案內容和新的那一行是第幾行（從 1 開始）
///
/// 換行照檔案原本用的（`\n` 或 `\r\n`）；檔案最後一行沒有換行就先補一個。呼叫的人要先用 `check` 確認 `new` 沒問題
pub fn add(text: &str, new: &str, place: &Place) -> Result<(String, usize), Changed> {
    let lines: Vec<&str> = text.lines().collect();
    let segments: Vec<&str> = text.split_inclusive('\n').collect();
    // 新的一行插在第幾段前面、縮排多少
    let (at, indent) = match place {
        Place::InGroupOf(item) => {
            find_line(&lines, item)?;
            // 同一組的最後一件（檔案現在的內容重新 parse，跟畫面認的一樣）
            let items = tasks::parse(text).items;
            let mut last = items
                .iter()
                .skip_while(|other| other.line != item.line)
                .take_while(|other| other.group == item.group)
                .last()
                .ok_or(Changed)?;
            // 最後一件是子任務：往上找到它最外層的那件，從那件算整塊（子任務、底下的備註都在裡面），
            // 新的才不會插在備註前面、把備註搶過去
            while let Some(parent) = last.parent {
                last = &items[parent];
            }
            // 跟那一組最外層的事一樣，不縮排
            (block_end(&lines, last.line - 1), String::new())
        }
        Place::ChildOf(item) => {
            let start = find_line(&lines, item)?;
            let indent =
                &item.line_text[..item.line_text.len() - item.line_text.trim_start().len()];
            (block_end(&lines, start), format!("{indent}  "))
        }
        Place::End => (segments.len(), String::new()),
    };
    let newline = if text.contains("\r\n") { "\r\n" } else { "\n" };
    let mut result = segments[..at].concat();
    // 前一行是檔案最後一行、沒有換行：先補一個
    if !result.is_empty() && !result.ends_with('\n') {
        result.push_str(newline);
    }
    result.push_str(&format!("{indent}{}{newline}", new.trim()));
    result.push_str(&segments[at..].concat());
    Ok((result, at + 1))
}

/// 刪除：`item` 連同底下縮排比它深的連續幾行（子任務、備註，中間的空行也算；後面的空行不算）一起刪掉；
/// 回傳刪掉之後的整個檔案，和刪掉幾件（含它自己）
///
/// 例：刪 `- 東京 5 天` → 底下的 `  - [ ] 訂機票`、`    - [ ] 比價` 一起刪，共 3 件
pub fn delete(text: &str, item: &Item) -> Result<(String, usize), Changed> {
    let lines: Vec<&str> = text.lines().collect();
    // 連換行的整段，換行（\n 或 \r\n）才能照原樣留著
    let segments: Vec<&str> = text.split_inclusive('\n').collect();
    let start = find_line(&lines, item)?;
    let end = block_end(&lines, start);
    // 刪掉的那幾行裡有幾件（用 parser 數，跟畫面認的一樣）
    let count = tasks::parse(&segments[start..end].concat()).items.len();
    let mut result = segments[..start].concat();
    result.push_str(&segments[end..].concat());
    Ok((result, count))
}

/// 寫檔前後的行號對照（從 1 開始），畫面記著的行號（選到的那件、展開的子任務）跟著搬
///
/// 每次寫檔只改連續的一段（打勾、編輯、改期、新增、刪除、復原都是）：前面沒動的行號不變、後面的跟著加減；
/// 改到的那一段：行數沒變（換掉一行）就照舊，變了（新增、刪除）就對不到（`None`）
pub fn line_map(before: &str, after: &str) -> impl Fn(usize) -> Option<usize> + use<> {
    let old: Vec<&str> = before.lines().collect();
    let new: Vec<&str> = after.lines().collect();
    // 前面一樣的有幾行、後面一樣的有幾行（不跟前面那段重疊）
    let prefix = old.iter().zip(&new).take_while(|(a, b)| a == b).count();
    let suffix = old
        .iter()
        .rev()
        .zip(new.iter().rev())
        .take_while(|(a, b)| a == b)
        .count()
        .min(old.len().min(new.len()) - prefix);
    let (old_len, new_len) = (old.len(), new.len());
    move |line| {
        if line <= prefix {
            // 改到的那段前面：沒動
            Some(line)
        } else if line > old_len - suffix {
            // 改到的那段後面：跟著多出來 / 少掉的行數加減
            (line + new_len).checked_sub(old_len)
        } else if old_len == new_len {
            // 改到的那段裡面、行數沒變（換掉一行）：照舊
            Some(line)
        } else {
            // 改到的那段裡面、行數變了：對不到
            None
        }
    }
}

/// `item` 在第幾行（從 0 開始）；那一行跟讀進來時不一樣（或已經沒有那一行）回傳 `Changed`
fn find_line(lines: &[&str], item: &Item) -> Result<usize, Changed> {
    let index = item.line.checked_sub(1).ok_or(Changed)?;
    if lines.get(index) == Some(&item.line_text.as_str()) {
        Ok(index)
    } else {
        Err(Changed)
    }
}

/// 第 `start` 行（從 0 開始）連同底下縮排比它深的連續幾行（子任務、備註，中間的空行也算）結束在哪：
/// 回傳最後一行有內容的下一行。最後面的空行不算（那是跟下一件之間的空行）
fn block_end(lines: &[&str], start: usize) -> usize {
    let indent = tasks::indent_width(lines[start]);
    let mut end = start + 1;
    for (index, line) in lines.iter().enumerate().skip(start + 1) {
        // 空行先跳過；遇到縮排沒有比它深的那行就停
        if line.trim().is_empty() {
            continue;
        }
        if tasks::indent_width(line) <= indent {
            break;
        }
        end = index + 1;
    }
    end
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
        // 還沒排日期：加在行尾，`^id` 要留在最後面（行尾的空白不算；`^id` 前面是 tab 也認得）
        let (body, block_id) = match line.trim_end().rsplit_once(char::is_whitespace) {
            Some((body, last)) if tasks::is_block_id(last) => (body, format!(" {last}")),
            _ => (line.trim_end(), String::new()),
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
    // 頭尾同一天的範圍（`@2026-10-20..2026-10-20`）parser 當成一天，兩種寫法都找
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
    fn check_refuses_an_empty_title() {
        assert_eq!(check("- [ ]  @2026-10-17"), Err(Problem::EmptyTitle));
        assert_eq!(check("- [ ]"), Err(Problem::EmptyTitle));
        // 只打了 `- `：也是標題空的（不是「要用 `- ` 開頭」）
        assert_eq!(check("- "), Err(Problem::EmptyTitle));
        assert_eq!(check("  -  "), Err(Problem::EmptyTitle));
    }

    /// 新增的 test 用的檔案：「工作」有一件帶子任務和空行，後面接「生活」
    const GROUPS: &str = "# 工作\n- [ ] 寫報告 @2026-10-16\n  - [ ] 找資料\n\n  - [x] 列大綱\n\n# 生活\n- 爬山 @2026-10-18\n";

    #[test]
    fn add_goes_after_the_last_item_of_the_group() {
        // 「工作」的最後一件（寫報告）連子任務、中間的空行一起算；後面的空行不算
        let (after, line) = add(
            GROUPS,
            "- [ ] 交週報 @2026-10-16",
            &Place::InGroupOf(item(GROUPS, 0)),
        )
        .expect("add");
        assert_eq!(
            after,
            "# 工作\n- [ ] 寫報告 @2026-10-16\n  - [ ] 找資料\n\n  - [x] 列大綱\n- [ ] 交週報 @2026-10-16\n\n# 生活\n- 爬山 @2026-10-18\n"
        );
        assert_eq!(line, 6);
        // 選的是子任務也一樣：加在那一組最後面，不縮排
        let (after, _) = add(GROUPS, "- x", &Place::InGroupOf(item(GROUPS, 1))).expect("add");
        assert!(after.contains("  - [x] 列大綱\n- x\n\n# 生活"));
        // 「生活」是最後一組：加在檔案最後
        let (after, line) = add(
            GROUPS,
            "- 聚餐 @2026-10-17",
            &Place::InGroupOf(item(GROUPS, 3)),
        )
        .expect("add");
        assert!(after.ends_with("- 爬山 @2026-10-18\n- 聚餐 @2026-10-17\n"));
        assert_eq!(line, 9);
    }

    #[test]
    fn add_keeps_the_last_items_notes_with_it() {
        // 寫報告的備註（跟子任務一樣縮排）在最後一個子任務後面：新的一件放在備註後面，備註還是寫報告的
        let text = "# 工作\n- [ ] 寫報告\n  - [ ] 找資料\n  備註：下週交\n# 生活\n";
        let (after, line) = add(text, "- [ ] 新的", &Place::InGroupOf(item(text, 0))).expect("add");
        assert_eq!(
            after,
            "# 工作\n- [ ] 寫報告\n  - [ ] 找資料\n  備註：下週交\n- [ ] 新的\n# 生活\n"
        );
        assert_eq!(line, 5);
    }

    #[test]
    fn add_a_subtask_right_under_its_parent() {
        // 寫報告的子任務：加在它原本的子任務下面，縮排多 2 格
        let (after, line) =
            add(GROUPS, "- [ ] 校稿", &Place::ChildOf(item(GROUPS, 0))).expect("add");
        assert!(after.contains("  - [x] 列大綱\n  - [ ] 校稿\n\n# 生活"));
        assert_eq!(line, 6);
        // 子任務的子任務：再多 2 格
        let (after, _) =
            add(GROUPS, "- [ ] 查文獻", &Place::ChildOf(item(GROUPS, 1))).expect("add");
        assert!(after.contains("  - [ ] 找資料\n    - [ ] 查文獻\n\n  - [x] 列大綱"));
    }

    #[test]
    fn add_at_the_end_keeps_the_newlines() {
        // 最後一行沒有換行：先補一個；\r\n 的檔案用 \r\n
        assert_eq!(add("- a", "- b", &Place::End), Ok(("- a\n- b\n".into(), 2)));
        assert_eq!(
            add("- a\r\n", "- b", &Place::End),
            Ok(("- a\r\n- b\r\n".into(), 2))
        );
        // 空的檔案（還不存在的任務檔）
        assert_eq!(add("", "- b", &Place::End), Ok(("- b\n".into(), 1)));
        // 選的那件在別處改過：不加
        assert_eq!(
            add("- [ ] 別的\n", "- b", &Place::InGroupOf(item(GROUPS, 0))),
            Err(Changed)
        );
    }

    #[test]
    fn delete_takes_the_subtasks_along() {
        // 寫報告 + 2 件子任務（中間的空行也刪）= 3 件；後面跟下一組之間的空行留著
        assert_eq!(
            delete(GROUPS, &item(GROUPS, 0)),
            Ok(("# 工作\n\n# 生活\n- 爬山 @2026-10-18\n".into(), 3))
        );
        // 刪子任務：只有它自己
        assert_eq!(
            delete(GROUPS, &item(GROUPS, 2)),
            Ok((
                "# 工作\n- [ ] 寫報告 @2026-10-16\n  - [ ] 找資料\n\n\n# 生活\n- 爬山 @2026-10-18\n".into(),
                1
            ))
        );
        // 最後一行沒有換行、\r\n：其他行照原樣
        assert_eq!(
            delete("- a\r\n- b", &item("- a\r\n- b", 1)),
            Ok(("- a\r\n".into(), 1))
        );
        // 在別處改過：不刪
        assert_eq!(delete("- [ ] 別的\n", &item(GROUPS, 0)), Err(Changed));
    }

    #[test]
    fn line_map_follows_the_lines_that_moved() {
        // 換掉一行：每一行都照舊
        let map = line_map("a\nb\nc\n", "a\nB\nc\n");
        assert_eq!([1, 2, 3].map(&map), [Some(1), Some(2), Some(3)]);
        // 在第 2 行插一行：後面的往下一行
        let map = line_map("a\nb\nc\n", "a\nnew\nb\nc\n");
        assert_eq!([1, 2, 3].map(&map), [Some(1), Some(3), Some(4)]);
        // 刪掉第 2、3 行：刪掉的對不到，後面的往上兩行
        let map = line_map("a\nb\nc\nd\n", "a\nd\n");
        assert_eq!([1, 2, 3, 4].map(&map), [Some(1), None, None, Some(2)]);
        // 加在最後面、刪掉最後面
        let map = line_map("a\nb\n", "a\nb\nc\n");
        assert_eq!([1, 2].map(&map), [Some(1), Some(2)]);
        let map = line_map("a\nb\n", "a\n");
        assert_eq!([1, 2].map(&map), [Some(1), None]);
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
        // ^id 後面還有空白、^id 前面是 tab：一樣加在 ^id 前面（Obsidian 連到這一行的連結才不會斷）
        for text in ["- [ ] 回信 ^r-02  \n", "- [ ] 回信\t^r-02\n"] {
            assert_eq!(
                reschedule(text, &item(text, 0), date(2026, 10, 18)),
                Ok("- [ ] 回信 @2026-10-18 ^r-02\n".into()),
                "{text:?}"
            );
        }
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
