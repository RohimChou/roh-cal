//! 改任務檔的純 function：給檔案現在的內容，回傳改好的新內容（寫檔由呼叫的人用 `files::write_atomically`）
//!
//! 只動要改的那幾行，其他每個字（包括換行是 `\n` 還是 `\r\n`）都不動。
//! 改之前先比對：那一行跟讀進來時不一樣（例：剛在 Obsidian 改過）就不改，回傳 `Changed`

use super::{Item, checkbox, indent_width, parse};

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
    let start = item.line.checked_sub(1).ok_or(Changed)?;
    if lines.get(start) != Some(&item.line_text.as_str()) {
        return Err(Changed);
    }
    // 往下找：空行先跳過；遇到縮排沒有比它深的那行就停；`end` = 最後一行有內容的下一行
    let indent = indent_width(&item.line_text);
    let mut end = start + 1;
    for (index, line) in lines.iter().enumerate().skip(start + 1) {
        if line.trim().is_empty() {
            continue;
        }
        if indent_width(line) <= indent {
            break;
        }
        end = index + 1;
    }
    // 刪掉的那幾行裡有幾個項目（用 parser 數，跟畫面認的一樣）
    let count = parse(&segments[start..end].concat()).items.len();
    let mut result = segments[..start].concat();
    result.push_str(&segments[end..].concat());
    Ok((result, count))
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
}
