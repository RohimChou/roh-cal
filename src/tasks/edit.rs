//! 改任務檔的純 function：給檔案現在的內容，回傳改好的新內容（寫檔由呼叫的人用 `files::write_atomically`）
//!
//! 只改那一行，其他每個字（包括換行是 `\n` 還是 `\r\n`）都不動。
//! 改之前先比對：那一行跟讀進來時不一樣（例：剛在 Obsidian 改過）就不改，回傳 `Changed`

use super::{Item, checkbox};

/// 那一行跟讀進來時不一樣（被別人改過、上面多了一行、檔案變短了），不能改；要先重新讀檔
#[derive(Debug, PartialEq, Eq)]
pub struct Changed;

/// 打勾 / 取消：`- [ ] 週會` ↔ `- [x] 週會`
///
/// `text` = 任務檔現在的內容、`item` = 之前讀到的那個項目，回傳打勾後的整個檔案
pub fn toggle(text: &str, item: &Item) -> Result<String, Changed> {
    replace_line(text, item, &toggled(&item.line_text))
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
}
