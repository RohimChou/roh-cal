//! 改任務檔的純 function：給檔案現在的內容，回傳改好的新內容（寫檔由呼叫的人用 `files::write_atomically`）
//!
//! 只動要改的那幾行，其他每個字（包括換行是 `\n` 還是 `\r\n`）都不動。
//! 改之前先比對：那一行跟讀進來時不一樣（例：剛在 Obsidian 改過）就不改，回傳 `Changed`

use super::{Item, checkbox, indent_width, parse};
use jiff::civil::{Date, Time};

/// 那一行跟讀進來時不一樣（被別人改過、上面多了一行、檔案變短了），不能改；要先重新讀檔
#[derive(Debug, PartialEq, Eq)]
pub struct Changed;

/// 打勾 / 取消：`- [ ] 週會` ↔ `- [x] 週會`
///
/// `text` = 任務檔現在的內容、`item` = 之前讀到的那個項目，回傳打勾後的整個檔案
pub fn toggle(text: &str, item: &Item) -> Result<String, Changed> {
    replace_line(text, item, &toggled(&item.line_text))
}

/// 刪掉項目，連同底下縮排比它深的連續幾行（子任務、備註，中間的空行也算）；
/// 回傳刪掉之後的整個檔案，和刪掉幾個項目（含它自己）
///
/// 例：刪 `- 東京 5 天` → 底下的 `  - [ ] 訂機票`、`    - [ ] 比價` 一起刪，共 3 個。
/// 最後面的空行不算進去（那是跟下一個項目之間的空行）
pub fn delete(text: &str, item: &Item) -> Result<(String, usize), Changed> {
    let lines: Vec<&str> = text.lines().collect();
    // 連換行的整段，換行（\n 或 \r\n）才能照原樣留著
    let segments: Vec<&str> = text.split_inclusive('\n').collect();
    let start = find_line(&lines, item)?;
    let end = block_end(&lines, start);
    // 刪掉的那幾行裡有幾個項目（用 parser 數，跟畫面認的一樣）
    let count = parse(&segments[start..end].concat()).items.len();
    let mut result = segments[..start].concat();
    result.push_str(&segments[end..].concat());
    Ok((result, count))
}

/// 新增一個項目 `- [ ] 標題 @日期`；回傳新的檔案內容，和新項目在第幾行（從 1 開始）
///
/// 放在 `near`（清單上選到的項目）那一組（同一個標題底下）的最後一個項目和它的子任務下面；
/// `near` 是 `None`（那天還沒有項目）就加在檔案最後面。換行照檔案原本用的（`\n` 或 `\r\n`）
pub fn add(
    text: &str,
    title: &str,
    date: Date,
    near: Option<&Item>,
) -> Result<(String, usize), Changed> {
    let lines: Vec<&str> = text.lines().collect();
    let segments: Vec<&str> = text.split_inclusive('\n').collect();
    // 新的一行插在第幾段前面
    let at = match near {
        Some(item) => {
            find_line(&lines, item)?;
            // 同一組的最後一個項目（檔案現在的內容重新 parse，跟畫面認的一樣）
            let items = parse(text).items;
            let last = items
                .iter()
                .skip_while(|other| other.line != item.line)
                .take_while(|other| other.group == item.group)
                .last()
                .ok_or(Changed)?;
            block_end(&lines, last.line - 1)
        }
        None => segments.len(),
    };
    let newline = if text.contains("\r\n") { "\r\n" } else { "\n" };
    let mut result = segments[..at].concat();
    // 前一行是檔案最後一行、沒有換行：先補一個
    if !result.is_empty() && !result.ends_with('\n') {
        result.push_str(newline);
    }
    result.push_str(&format!("- [ ] {} @{date}{newline}", title.trim()));
    result.push_str(&segments[at..].concat());
    Ok((result, at + 1))
}

/// 編輯項目：那一行重寫成標準的樣子 `- [ ] 標題 @日期 時間`，回傳整個檔案
///
/// 縮排和打勾狀態（`[ ]` / `[x]` / `[X]` / 沒寫 checkbox）照原本那一行，日期放最後面，`time` 是 `None` 就不寫時間。
/// 標題、日期、時間都跟原本一樣就一個字都不動。例：`- [ ] 會議 @2026-10-12 跟客戶` 改成 10/13 →
/// `- [ ] 會議 跟客戶 @2026-10-13`
///
/// `title` 裡不能有 `@日期`：parser 只認第一個 `@日期`，日期放最後面的話會被標題裡的搶走，呼叫的人要先擋掉
pub fn update(
    text: &str,
    item: &Item,
    title: &str,
    date: Date,
    time: Option<Time>,
) -> Result<String, Changed> {
    let title = title.trim();
    // 什麼都沒改：那一行照原樣（還是要確認那一行沒被別人改過）
    if title == item.title && Some(date) == item.date && time == item.time {
        return replace_line(text, item, &item.line_text);
    }
    replace_line(text, item, &rewritten(&item.line_text, title, date, time))
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
/// 回傳最後一行有內容的下一行。最後面的空行不算（那是跟下一個項目之間的空行）
fn block_end(lines: &[&str], start: usize) -> usize {
    let indent = indent_width(lines[start]);
    let mut end = start + 1;
    for (index, line) in lines.iter().enumerate().skip(start + 1) {
        // 空行先跳過；遇到縮排沒有比它深的那行就停
        if line.trim().is_empty() {
            continue;
        }
        if indent_width(line) <= indent {
            break;
        }
        end = index + 1;
    }
    end
}

/// 一行打勾 / 取消，縮排和後面的文字一字不動（`line` 是 parser 認過的項目那一行）
///
/// - `- [ ] 週會` → `- [x] 週會`；`- [x]`、`- [X]` → `- [ ]`
/// - 沒有 checkbox 的 `- 看牙醫` → `- [x] 看牙醫`（再取消就是 `- [ ] 看牙醫`）
fn toggled(line: &str) -> String {
    let trimmed = line.trim_start();
    let indent = &line[..line.len() - trimmed.len()];
    let rest = trimmed.strip_prefix("- ").unwrap_or(trimmed);
    match checkbox(rest) {
        // 有 checkbox：只換 `[ ]` / `[x]` 那 3 個字
        Some((done, after)) => {
            let mark = if done { "[ ]" } else { "[x]" };
            format!("{indent}- {mark}{after}")
        }
        // 沒有 checkbox：補一個打勾的（空的項目 `- ` 後面就不多留空白）
        None if rest.is_empty() => format!("{indent}- [x]"),
        None => format!("{indent}- [x] {rest}"),
    }
}

/// 一行重寫成 `- [ ] 標題 @日期 時間`，縮排和 checkbox 照原本那一行（`line` 是 parser 認過的項目那一行）
///
/// 例：`  - [x] 準備 slides @2026-10-12` → `  - [x] 準備投影片 @2026-10-12 09:00`
fn rewritten(line: &str, title: &str, date: Date, time: Option<Time>) -> String {
    let trimmed = line.trim_start();
    let indent = &line[..line.len() - trimmed.len()];
    let rest = trimmed.strip_prefix("- ").unwrap_or(trimmed);
    // checkbox 照原本的寫法（`[ ]`、`[x]`、`[X]` 都是 3 個 byte）；沒寫 checkbox 就不補
    let mark = match checkbox(rest) {
        Some(_) => format!("{} ", &rest[..3]),
        None => String::new(),
    };
    // 標題是空的：日期緊接在 checkbox 後面，不多留空白
    let title = if title.is_empty() {
        String::new()
    } else {
        format!("{title} ")
    };
    let time = time
        .map(|time| format!(" {}", time.strftime("%H:%M")))
        .unwrap_or_default();
    format!("{indent}- {mark}{title}@{date}{time}")
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
    use jiff::civil::{date, time};

    /// 第 `line` 行的項目打勾 / 取消，回傳新的檔案內容（test 用）
    fn toggle_line(text: &str, line: usize) -> Result<String, Changed> {
        let items = parse(text).items;
        let item = items.iter().find(|item| item.line == line).expect("item");
        toggle(text, item)
    }

    #[test]
    fn toggle_flips_the_checkbox_and_keeps_the_rest_of_the_line() {
        let cases = [
            (
                "- [ ] 週會 @2026-10-12 10:00",
                "- [x] 週會 @2026-10-12 10:00",
            ),
            ("- [x] 週會", "- [ ] 週會"),
            ("- [X] 週會", "- [ ] 週會"),
            // 沒有 checkbox：補一個打勾的
            (
                "- 看牙醫 @2026-10-14 10:30",
                "- [x] 看牙醫 @2026-10-14 10:30",
            ),
            // 縮排（空白、tab）、多的空白、@人名、網址都照留
            (
                "  - [ ]  跟 @Kevin 對 https://x.io",
                "  - [x]  跟 @Kevin 對 https://x.io",
            ),
            ("\t- [x] 書房", "\t- [ ] 書房"),
            // 空的項目
            ("- [ ]", "- [x]"),
            ("- ", "- [x]"),
        ];
        for (before, after) in cases {
            assert_eq!(toggle_line(before, 1), Ok(after.to_string()), "{before}");
        }
    }

    #[test]
    fn toggle_changes_only_that_line() {
        // 標題、說明、空行、\r\n、最後一行沒換行，都原封不動
        let text = "# 例行\r\n說明文字\r\n\r\n- [ ] 週會\r\n  - [ ] 準備投影片\r\n- 交週報";
        assert_eq!(
            toggle_line(text, 4),
            Ok(
                "# 例行\r\n說明文字\r\n\r\n- [x] 週會\r\n  - [ ] 準備投影片\r\n- 交週報"
                    .to_string()
            )
        );
        assert_eq!(
            toggle_line(text, 6),
            Ok(
                "# 例行\r\n說明文字\r\n\r\n- [ ] 週會\r\n  - [ ] 準備投影片\r\n- [x] 交週報"
                    .to_string()
            )
        );
        // \n、最後有換行
        assert_eq!(
            toggle_line("- a\n- b\n", 2),
            Ok("- a\n- [x] b\n".to_string())
        );
    }

    #[test]
    fn toggle_refuses_when_the_line_changed() {
        let item = parse("- [ ] 週會\n- [ ] 交週報\n").items[1].clone();
        // 那一行被改過（例：剛在 Obsidian 改了字）
        assert_eq!(
            toggle("- [ ] 週會\n- [ ] 交週報（改）\n", &item),
            Err(Changed)
        );
        // 上面多了一行，第 2 行變成別的
        assert_eq!(
            toggle("- [ ] 新的\n- [ ] 週會\n- [ ] 交週報\n", &item),
            Err(Changed)
        );
        // 檔案變短，沒有第 2 行了
        assert_eq!(toggle("- [ ] 週會\n", &item), Err(Changed));
        // 改的是別行：照樣打勾，別人的改動也留著
        assert_eq!(
            toggle("- [x] 週會\n- [ ] 交週報\n- [ ] 新的\n", &item),
            Ok("- [x] 週會\n- [x] 交週報\n- [ ] 新的\n".to_string())
        );
    }

    #[test]
    fn toggle_every_item_of_the_sample_file() {
        let text = include_str!("../../testdata/tasks.md");
        let items = parse(text).items;
        for (index, item) in items.iter().enumerate() {
            let new_text = toggle(text, item).expect("file not changed");
            let new_items = parse(&new_text).items;
            // 只有這個項目的完成狀態反過來；其他項目、這個項目的標題 / 日期 / 分組 / 層級都一樣
            let mut expected = items.clone();
            expected[index].done = !item.done;
            expected[index].line_text = new_items[index].line_text.clone();
            assert_eq!(new_items, expected, "line {}", item.line);
            // 整個檔案只差這一行
            let changed = text
                .lines()
                .zip(new_text.lines())
                .filter(|(old, new)| old != new);
            assert_eq!(changed.count(), 1, "line {}", item.line);
            assert_eq!(new_text.lines().count(), text.lines().count());
        }
    }

    /// 刪掉第 `line` 行的項目（test 用）
    fn delete_line(text: &str, line: usize) -> Result<(String, usize), Changed> {
        let items = parse(text).items;
        let item = items.iter().find(|item| item.line == line).expect("item");
        delete(text, item)
    }

    #[test]
    fn delete_takes_subtasks_and_notes_along() {
        let text = "# 旅行\n- 東京 5 天 @2026-11-01\n  - [ ] 訂機票\n    - [ ] 比價\n  備註：記得帶護照\n\n  - [ ] 訂飯店\n\n- [ ] 換日幣\n";
        // 刪「東京 5 天」：底下縮排比較深的（子任務、備註、中間的空行）都一起刪，最後面的空行留著
        assert_eq!(
            delete_line(text, 2),
            Ok(("# 旅行\n\n- [ ] 換日幣\n".to_string(), 4))
        );
        // 刪子任務「訂機票」：只帶走它底下的「比價」
        assert_eq!(
            delete_line(text, 3),
            Ok((
                "# 旅行\n- 東京 5 天 @2026-11-01\n  備註：記得帶護照\n\n  - [ ] 訂飯店\n\n- [ ] 換日幣\n"
                    .to_string(),
                2
            ))
        );
        // 沒有子任務：只刪那一行
        assert_eq!(
            delete_line(text, 9),
            Ok((
                "# 旅行\n- 東京 5 天 @2026-11-01\n  - [ ] 訂機票\n    - [ ] 比價\n  備註：記得帶護照\n\n  - [ ] 訂飯店\n\n"
                    .to_string(),
                1
            ))
        );
    }

    #[test]
    fn delete_keeps_line_endings() {
        // \r\n 照留；刪最後一行（沒有換行）時，前一行的換行留著
        let text = "- [ ] a\r\n  - [ ] a1\r\n- [ ] b";
        assert_eq!(delete_line(text, 1), Ok(("- [ ] b".to_string(), 2)));
        assert_eq!(
            delete_line(text, 3),
            Ok(("- [ ] a\r\n  - [ ] a1\r\n".to_string(), 1))
        );
    }

    #[test]
    fn delete_refuses_when_the_line_changed() {
        let item = parse("- [ ] 週會\n- [ ] 交週報\n").items[1].clone();
        assert_eq!(
            delete("- [ ] 週會\n- [ ] 交週報（改）\n", &item),
            Err(Changed)
        );
        assert_eq!(delete("- [ ] 週會\n", &item), Err(Changed));
    }

    #[test]
    fn delete_every_item_of_the_sample_file() {
        let text = include_str!("../../testdata/tasks.md");
        let items = parse(text).items;
        for (index, item) in items.iter().enumerate() {
            let (new_text, count) = delete(text, item).expect("file not changed");
            // 刪掉的 = 它自己 + parser 認的所有後代（後面緊接著、層級比它深的項目）
            let descendants = items[index + 1..]
                .iter()
                .take_while(|other| other.depth > item.depth)
                .count();
            assert_eq!(count, 1 + descendants, "line {}", item.line);
            assert_eq!(parse(&new_text).items.len(), items.len() - count);
        }
    }

    /// 第 `line` 行的項目旁邊新增（`line` 是 0 就是那天沒有項目，加在最後面；test 用）
    fn add_near(text: &str, line: usize, title: &str) -> Result<(String, usize), Changed> {
        let items = parse(text).items;
        let near = items.iter().find(|item| item.line == line);
        add(text, title, jiff::civil::date(2026, 10, 12), near)
    }

    #[test]
    fn add_goes_after_the_last_item_of_the_same_group() {
        let text = "# 工作\n- [ ] 週會 @2026-10-12\n- [ ] 寫 test @2026-10-13\n  - [ ] 單元測試\n\n# 個人\n- [ ] 看牙醫 @2026-10-14\n";
        // 選到「週會」：加在「工作」這組最後一個項目（寫 test）和它的子任務下面，空行和下一組都不動
        assert_eq!(
            add_near(text, 2, "估人天"),
            Ok((
                "# 工作\n- [ ] 週會 @2026-10-12\n- [ ] 寫 test @2026-10-13\n  - [ ] 單元測試\n- [ ] 估人天 @2026-10-12\n\n# 個人\n- [ ] 看牙醫 @2026-10-14\n"
                    .to_string(),
                5
            ))
        );
        // 選到「看牙醫」（最後一組）：加在檔案最後面；標題前後的空白拿掉
        assert_eq!(
            add_near(text, 7, "  繳卡費 "),
            Ok((format!("{text}- [ ] 繳卡費 @2026-10-12\n"), 8))
        );
    }

    #[test]
    fn add_without_a_neighbour_goes_to_the_end() {
        // 那天沒有項目：加在最後面；最後一行沒換行就先補；空檔案也可以；\r\n 的檔案用 \r\n
        assert_eq!(
            add_near("- [ ] a\n", 0, "新的"),
            Ok(("- [ ] a\n- [ ] 新的 @2026-10-12\n".to_string(), 2))
        );
        assert_eq!(
            add_near("- [ ] a", 0, "新的"),
            Ok(("- [ ] a\n- [ ] 新的 @2026-10-12\n".to_string(), 2))
        );
        assert_eq!(
            add_near("", 0, "新的"),
            Ok(("- [ ] 新的 @2026-10-12\n".to_string(), 1))
        );
        assert_eq!(
            add_near("- [ ] a\r\n", 0, "新的"),
            Ok(("- [ ] a\r\n- [ ] 新的 @2026-10-12\r\n".to_string(), 2))
        );
    }

    #[test]
    fn add_refuses_when_the_neighbour_changed() {
        let item = parse("- [ ] 週會\n- [ ] 交週報\n").items[1].clone();
        let date = jiff::civil::date(2026, 10, 12);
        assert_eq!(
            add("- [ ] 週會\n- [ ] 交週報（改）\n", "x", date, Some(&item)),
            Err(Changed)
        );
    }

    #[test]
    fn add_near_every_item_of_the_sample_file() {
        let text = include_str!("../../testdata/tasks.md");
        let items = parse(text).items;
        for item in &items {
            let (new_text, line) = add_near(text, item.line, "新的").expect("file not changed");
            let new_items = parse(&new_text).items;
            // 多一個項目，就在回傳的那一行、跟選到的項目同一組、日期和標題對
            assert_eq!(new_items.len(), items.len() + 1);
            let added = new_items
                .iter()
                .find(|other| other.line == line)
                .expect("added");
            assert_eq!(added.group, item.group, "line {}", item.line);
            assert_eq!(
                (added.title.as_str(), added.date),
                ("新的", Some(jiff::civil::date(2026, 10, 12)))
            );
            assert_eq!(added.depth, 0, "line {}", item.line);
        }
    }

    /// 編輯第 `line` 行的項目（test 用）
    fn update_line(
        text: &str,
        line: usize,
        title: &str,
        day: Date,
        at: Option<Time>,
    ) -> Result<String, Changed> {
        let items = parse(text).items;
        let item = items.iter().find(|item| item.line == line).expect("item");
        update(text, item, title, day, at)
    }

    #[test]
    fn update_rewrites_the_line_in_the_standard_form() {
        let cases = [
            // 日期移到最後面
            (
                "- [ ] 會議 @2026-10-12 跟客戶",
                "會議 跟客戶",
                date(2026, 10, 13),
                None,
                "- [ ] 會議 跟客戶 @2026-10-13",
            ),
            // 縮排、[x] 照舊；改標題
            (
                "  - [x] 準備 slides @2026-10-12 09:00",
                "準備投影片",
                date(2026, 10, 12),
                Some(time(9, 0, 0, 0)),
                "  - [x] 準備投影片 @2026-10-12 09:00",
            ),
            // [X] 照舊；加時間
            (
                "- [X] 週報 @2026-10-16",
                "週報",
                date(2026, 10, 16),
                Some(time(17, 30, 0, 0)),
                "- [X] 週報 @2026-10-16 17:30",
            ),
            // 沒寫 checkbox 就不補；拿掉時間
            (
                "- 看牙醫 @2026-10-14 10:30",
                "看牙醫",
                date(2026, 10, 14),
                None,
                "- 看牙醫 @2026-10-14",
            ),
            // tab 縮排照留；多的空白收成一個，@人名、網址照留；標題前後的空白去掉
            (
                "\t- [ ]  跟 @Kevin 對   https://x.io @2026-10-12",
                "  跟 @Kevin 對 https://x.io ",
                date(2026, 10, 12),
                Some(time(14, 0, 0, 0)),
                "\t- [ ] 跟 @Kevin 對 https://x.io @2026-10-12 14:00",
            ),
        ];
        for (line, title, day, at, expected) in cases {
            assert_eq!(
                update_line(line, 1, title, day, at),
                Ok(expected.to_string()),
                "{line}"
            );
        }
    }

    #[test]
    fn update_without_changes_leaves_the_line_alone() {
        // 標題（前後多的空白不算）、日期、時間都沒改：多的空白、日期的位置都照原樣
        let text = "- [ ]  會議   @2026-10-12 14:00 跟客戶\n";
        assert_eq!(
            update_line(
                text,
                1,
                " 會議 跟客戶 ",
                date(2026, 10, 12),
                Some(time(14, 0, 0, 0))
            ),
            Ok(text.to_string())
        );
    }

    #[test]
    fn update_changes_only_that_line() {
        // 標題、說明、子任務、\r\n、最後一行沒換行，都原封不動
        let text = "# 例行\r\n說明文字\r\n- [ ] 週會 @2026-10-12\r\n  - [ ] 準備投影片\r\n- 交週報 @2026-10-16";
        assert_eq!(
            update_line(text, 3, "週會（改）", date(2026, 10, 19), None),
            Ok(
                "# 例行\r\n說明文字\r\n- [ ] 週會（改） @2026-10-19\r\n  - [ ] 準備投影片\r\n- 交週報 @2026-10-16"
                    .to_string()
            )
        );
        assert_eq!(
            update_line(text, 5, "交週報", date(2026, 10, 16), Some(time(17, 0, 0, 0))),
            Ok(
                "# 例行\r\n說明文字\r\n- [ ] 週會 @2026-10-12\r\n  - [ ] 準備投影片\r\n- 交週報 @2026-10-16 17:00"
                    .to_string()
            )
        );
    }

    #[test]
    fn update_refuses_when_the_line_changed() {
        let item = parse("- [ ] 週會 @2026-10-12\n").items[0].clone();
        // 那一行被改過：有改東西、沒改東西都不寫
        for title in ["週會（改）", "週會"] {
            assert_eq!(
                update(
                    "- [ ] 週會 @2026-10-13\n",
                    &item,
                    title,
                    date(2026, 10, 12),
                    None
                ),
                Err(Changed),
                "{title}"
            );
        }
        // 檔案變短，沒有那一行了
        assert_eq!(
            update("", &item, "週會", date(2026, 10, 12), None),
            Err(Changed)
        );
    }

    #[test]
    fn update_every_item_of_the_sample_file() {
        let text = include_str!("../../testdata/tasks.md");
        let items = parse(text).items;
        let (new_date, new_time) = (date(2026, 11, 3), Some(time(8, 15, 0, 0)));
        for (index, item) in items.iter().enumerate() {
            let new_text =
                update(text, item, "改過的標題", new_date, new_time).expect("file not changed");
            let new_items = parse(&new_text).items;
            // 只有這個項目的標題 / 日期 / 時間變了；完成狀態、分組、層級、父項目和其他項目都一樣
            let mut expected = items.clone();
            expected[index].title = "改過的標題".to_string();
            expected[index].date = Some(new_date);
            expected[index].time = new_time;
            expected[index].line_text = new_items[index].line_text.clone();
            assert_eq!(new_items, expected, "line {}", item.line);
            // 整個檔案只差這一行
            let changed = text
                .lines()
                .zip(new_text.lines())
                .filter(|(old, new)| old != new);
            assert_eq!(changed.count(), 1, "line {}", item.line);
            assert_eq!(new_text.lines().count(), text.lines().count());
        }
    }
}
