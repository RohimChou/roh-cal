//! 任務檔的 parser：純文字進去、項目出來（格式見 `docs/task-file-format.md`）
//!
//! 每個項目都記下行號和那一行原本的文字，寫回檔案時（`edit`）才知道要改哪一行、那一行有沒有被別人改過

pub mod edit;

use jiff::civil::{Date, Time};

/// 解析的結果
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Parsed {
    /// 所有項目，照檔案裡的順序
    pub items: Vec<Item>,
    /// 寫錯但不影響讀取的地方，例：`@2026-13-01`
    pub warnings: Vec<Warning>,
}

/// 任務檔裡的一個項目（`- ` 開頭的那一行）
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Item {
    /// 完成了沒：`[x]` / `[X]` 是完成；`[ ]` 或沒寫 checkbox 都是未完成（每一行都可以打勾）
    pub done: bool,
    /// 去掉 `- [ ] ` 和日期之後的文字，例：`實作退款 API`
    pub title: String,
    /// 出現在月曆的哪一天；沒寫就是還沒排日期
    pub date: Option<Date>,
    /// 幾點（只有寫了日期才會有）
    pub time: Option<Time>,
    /// 屬於哪個分組（上面最近的標題）；檔案開頭還沒有標題時是 `None`
    pub group: Option<Heading>,
    /// 第幾層：0 = 最外層，子任務 1、子任務的子任務 2…
    pub depth: usize,
    /// 父項目在 `items` 裡的位置；最外層是 `None`
    pub parent: Option<usize>,
    /// 在檔案的第幾行（從 1 開始，跟編輯器顯示的一樣）
    pub line: usize,
    /// 那一行原本的文字（整行，含縮排，不含換行），例：`  - [ ] 寫 integration test @2026-10-13`
    pub line_text: String,
}

/// 分組標題，例：`# 付款 API 改版`
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Heading {
    /// 幾個 `#`；v0 不分層，但先記下來，之後做多層分組不用改檔案
    pub level: usize,
    /// 標題文字
    pub name: String,
}

/// 解析時發現、但不影響讀取的問題
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Warning {
    /// 在檔案的第幾行（從 1 開始）
    pub line: usize,
    /// 哪一種問題
    pub kind: WarningKind,
}

/// 問題的種類；訊息文字由畫面依語言決定，這裡只記是什麼問題
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WarningKind {
    /// 長得像日期但不存在，例：`@2026-13-01`、`@2026-02-30`；當成一般文字
    InvalidDate(String),
    /// 長得像時間但不存在，例：`@2026-10-12 25:00` 的 `25:00`；當成一般文字
    InvalidTime(String),
}

/// 解析整個任務檔
pub fn parse(text: &str) -> Parsed {
    let mut parsed = Parsed::default();
    let mut group = None;
    // 目前這一串項目的 (縮排寬度, 在 items 的位置)，用來找父項目
    let mut ancestors: Vec<(usize, usize)> = Vec::new();
    // 是否在 ``` code block 裡面（裡面的東西都不算）
    let mut in_code_block = false;

    // lines() 會把 \n 和 \r\n 都當成換行
    for (index, raw) in text.lines().enumerate() {
        let line = index + 1;
        let trimmed = raw.trim_start();

        // ``` 或 ~~~ 開始 / 結束 code block
        if trimmed.starts_with("```") || trimmed.starts_with("~~~") {
            in_code_block = !in_code_block;
            continue;
        }
        if in_code_block {
            continue;
        }

        // 標題：換分組；新的分組裡，項目重新從最外層算起
        if let Some(heading) = parse_heading(raw) {
            group = Some(heading);
            ancestors.clear();
            continue;
        }

        let Some((done, rest)) = parse_item_start(trimmed) else {
            continue;
        };

        // 找父項目：往回丟掉縮排沒有比自己少的，剩下最後一個就是父項目
        let indent = indent_width(raw);
        while ancestors.last().is_some_and(|&(width, _)| width >= indent) {
            ancestors.pop();
        }
        let parent = ancestors.last().map(|&(_, position)| position);
        let depth = ancestors.len();

        let (title, date, time) = parse_title(rest, line, &mut parsed.warnings);
        ancestors.push((indent, parsed.items.len()));
        parsed.items.push(Item {
            done,
            title,
            date,
            time,
            group: group.clone(),
            depth,
            parent,
            line,
            line_text: raw.to_string(),
        });
    }
    parsed
}

/// `# 名稱` ～ `###### 名稱`（`#` 要在行首、後面要有空白）；不是標題回傳 `None`
fn parse_heading(line: &str) -> Option<Heading> {
    let level = line.chars().take_while(|&c| c == '#').count();
    if !(1..=6).contains(&level) {
        return None;
    }
    // `#` 都是 1 個 byte，所以可以直接用 level 切字串
    let name = line[level..].strip_prefix(' ')?;
    Some(Heading {
        level,
        name: name.trim().to_string(),
    })
}

/// 看這行是不是項目：回傳（完成了沒, 剩下的文字）；不是 `- ` 開頭就回傳 `None`
///
/// 例：`- [x] 打包` → (true, `打包`)；`- [ ] 打包` 和 `- 搬家` → (false, ...)
fn parse_item_start(trimmed: &str) -> Option<(bool, &str)> {
    // 要「- 」加空白；`-這樣` 不算
    let rest = trimmed.strip_prefix("- ")?;
    match checkbox(rest) {
        Some((done, after)) => Some((done, after.trim_start())),
        // 沒有 checkbox：也是未完成的項目
        None => Some((false, rest)),
    }
}

/// `- ` 後面的 checkbox：回傳（完成了沒, checkbox 後面的文字，空白照留）；沒有 checkbox 回傳 `None`
///
/// `[ ]`、`[x]`、`[X]` 後面要接空白或直接結束。例：`[x] 打包` → (true, ` 打包`)；`[ ]打包` 不算 checkbox
fn checkbox(rest: &str) -> Option<(bool, &str)> {
    let marks = [("[ ]", false), ("[x]", true), ("[X]", true)];
    for (mark, done) in marks {
        if let Some(after) = rest.strip_prefix(mark)
            && (after.is_empty() || after.starts_with(' '))
        {
            return Some((done, after));
        }
    }
    None
}

/// 縮排寬度：空白算 1、tab 算 4（混用時才比得出誰比較深）
fn indent_width(line: &str) -> usize {
    line.chars()
        .take_while(|c| c.is_whitespace())
        .map(|c| if c == '\t' { 4 } else { 1 })
        .sum()
}

/// 從項目的文字裡拿出第一個 `@日期`（和緊接的時間），剩下的就是標題
///
/// 例：`實作退款 API @2026-10-14` → (`實作退款 API`, 10/14, 沒有時間)
fn parse_title(
    text: &str,
    line: usize,
    warnings: &mut Vec<Warning>,
) -> (String, Option<Date>, Option<Time>) {
    let mut words = Vec::new();
    let mut date = None;
    let mut time = None;
    let mut tokens = text.split_whitespace().peekable();

    while let Some(token) = tokens.next() {
        // 已經有日期了（只認第一個），或不是 `@日期` 的樣子 → 一般文字
        let candidate = token.strip_prefix('@').filter(|rest| looks_like_date(rest));
        let Some(candidate) = candidate.filter(|_| date.is_none()) else {
            words.push(token);
            continue;
        };
        let Ok(parsed_date) = candidate.parse::<Date>() else {
            warnings.push(Warning {
                line,
                kind: WarningKind::InvalidDate(token.to_string()),
            });
            words.push(token);
            continue;
        };
        date = Some(parsed_date);
        // 日期後面緊接 `HH:MM` 就是時間
        if let Some(&next) = tokens.peek()
            && looks_like_time(next)
        {
            tokens.next();
            match next.parse::<Time>() {
                Ok(parsed_time) => time = Some(parsed_time),
                Err(_) => {
                    warnings.push(Warning {
                        line,
                        kind: WarningKind::InvalidTime(next.to_string()),
                    });
                    words.push(next);
                }
            }
        }
    }
    (words.join(" "), date, time)
}

/// 是不是 `YYYY-MM-DD` 的樣子（只看格式，不管日期存不存在）
fn looks_like_date(text: &str) -> bool {
    let bytes = text.as_bytes();
    bytes.len() == 10
        && bytes.iter().enumerate().all(|(i, &b)| match i {
            4 | 7 => b == b'-',
            _ => b.is_ascii_digit(),
        })
}

/// 是不是 `HH:MM` 的樣子（只看格式，不管時間存不存在）
fn looks_like_time(text: &str) -> bool {
    let bytes = text.as_bytes();
    bytes.len() == 5
        && bytes.iter().enumerate().all(|(i, &b)| match i {
            2 => b == b':',
            _ => b.is_ascii_digit(),
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use jiff::civil::{date, time};

    /// 解析單一行，回傳那個項目（測試用）
    fn one(line: &str) -> Item {
        let parsed = parse(line);
        assert_eq!(parsed.items.len(), 1, "{line}");
        parsed.items[0].clone()
    }

    #[test]
    fn rule1_tasks_open_and_done() {
        assert!(!one("- [ ] 打包").done);
        assert!(one("- [x] 打包").done);
        assert!(one("- [X] 打包").done);
        // checkbox 後面沒有內容也算（空的任務）
        assert_eq!(one("- [ ]").title, "");
    }

    #[test]
    fn rule2_item_without_checkbox_is_an_open_item() {
        let item = one("- 看牙醫 @2026-10-14");
        assert!(!item.done);
        assert_eq!(item.title, "看牙醫");
        // `[ ]` 後面沒有空白就不是 checkbox：整段當文字，一樣是未完成
        let item = one("- [ ]打包");
        assert!(!item.done);
        assert_eq!(item.title, "[ ]打包");
    }

    #[test]
    fn rule3_date_and_time_are_removed_from_title() {
        let item = one("- [ ] Code review @2026-10-12 14:00");
        assert_eq!(item.title, "Code review");
        assert_eq!(item.date, Some(date(2026, 10, 12)));
        assert_eq!(item.time, Some(time(14, 0, 0, 0)));
        // 日期放中間也可以
        let item = one("- 交 @2026-10-16 週報");
        assert_eq!(
            (item.title.as_str(), item.date),
            ("交 週報", Some(date(2026, 10, 16)))
        );
        // 沒寫日期 = 還沒排
        assert_eq!(one("- [ ] 整理書櫃").date, None);
    }

    #[test]
    fn rule3_only_the_first_date_counts() {
        let item = one("- [ ] 準備上線清冊 @2026-10-22 @2026-10-23");
        assert_eq!(item.date, Some(date(2026, 10, 22)));
        assert_eq!(item.title, "準備上線清冊 @2026-10-23");
    }

    #[test]
    fn rule3_at_mentions_and_other_text_stay_text() {
        let item = one("- [ ] 跟前端 @Kevin 對 error code");
        assert_eq!(
            (item.title.as_str(), item.date),
            ("跟前端 @Kevin 對 error code", None)
        );
        // URL、key:value、#編號都只是文字
        let item = one("- [ ] 參考 https://example.com due:2026-10-15 PR #482");
        assert_eq!(
            item.title,
            "參考 https://example.com due:2026-10-15 PR #482"
        );
        // 時間要緊接在日期後面才算
        assert_eq!(one("- 會議 14:00 @2026-10-12").time, None);
    }

    #[test]
    fn rule3_invalid_dates_stay_text_with_warning() {
        let parsed = parse("- [ ] 繳房屋稅 @2026-13-01\n- [ ] 交 @2026-02-30");
        assert_eq!(parsed.items[0].title, "繳房屋稅 @2026-13-01");
        assert_eq!(parsed.items[0].date, None);
        let expected = vec![
            Warning {
                line: 1,
                kind: WarningKind::InvalidDate("@2026-13-01".into()),
            },
            Warning {
                line: 2,
                kind: WarningKind::InvalidDate("@2026-02-30".into()),
            },
        ];
        assert_eq!(parsed.warnings, expected);
        // 不存在的時間：日期照算，時間當文字
        let parsed = parse("- 會議 @2026-10-12 25:00");
        assert_eq!(parsed.items[0].date, Some(date(2026, 10, 12)));
        assert_eq!(
            (parsed.items[0].time, parsed.items[0].title.as_str()),
            (None, "會議 25:00")
        );
        assert_eq!(
            parsed.warnings[0].kind,
            WarningKind::InvalidTime("25:00".into())
        );
    }

    #[test]
    fn rule4_headings_set_the_group_and_keep_their_level() {
        let parsed = parse("- 沒分組\n# 個人\n- [ ] 繳信用卡\n## 搬家\n- [ ] 打包");
        let groups: Vec<_> = parsed.items.iter().map(|item| item.group.clone()).collect();
        let heading = |level, name: &str| {
            Some(Heading {
                level,
                name: name.into(),
            })
        };
        assert_eq!(groups, vec![None, heading(1, "個人"), heading(2, "搬家")]);
        // `#tag`（沒有空白）、7 個 # 都不是標題
        assert_eq!(parse("#tag\n- a").items[0].group, None);
        assert_eq!(parse("####### x\n- a").items[0].group, None);
    }

    #[test]
    fn rule5_indentation_makes_children() {
        let text = "- 東京 5 天\n  - [ ] 訂機票\n    - [ ] 比價\n  - [ ] 訂飯店\n- 出發";
        let items = parse(text).items;
        let shape: Vec<_> = items.iter().map(|item| (item.depth, item.parent)).collect();
        assert_eq!(
            shape,
            vec![
                (0, None),
                (1, Some(0)),
                (2, Some(1)),
                (1, Some(0)),
                (0, None)
            ]
        );
        // 原本那一行連縮排一起記
        assert_eq!(items[2].line_text, "    - [ ] 比價");
        // tab 也算縮排
        let items = parse("- [ ] 打包\n\t- [ ] 書房").items;
        assert_eq!(items[1].parent, Some(0));
        // 新的標題底下重新從最外層算
        let items = parse("- a\n# g\n  - b").items;
        assert_eq!(items[1].parent, None);
    }

    #[test]
    fn other_lines_are_ignored() {
        let text = "說明文字\n\n-沒有空白\n```\n- [ ] code block 裡的\n```\n- [ ] 真的任務";
        let parsed = parse(text);
        assert_eq!(parsed.items.len(), 1);
        assert_eq!(
            (parsed.items[0].title.as_str(), parsed.items[0].line),
            ("真的任務", 7)
        );
    }

    #[test]
    fn crlf_line_endings_work() {
        let items = parse("- [ ] a @2026-10-12\r\n- [x] b\r\n").items;
        assert_eq!(items.len(), 2);
        assert_eq!(
            (items[0].title.as_str(), items[1].title.as_str()),
            ("a", "b")
        );
        // 原本那一行不含 \r
        assert_eq!(items[1].line_text, "- [x] b");
    }

    #[test]
    fn sample_file_parses_like_a_real_one() {
        let parsed = parse(include_str!("../testdata/tasks.md"));
        let count = |f: fn(&Item) -> bool| parsed.items.iter().filter(|item| f(item)).count();
        assert_eq!(parsed.items.len(), 56);
        // 7 個 `[x]`；31 個 `[ ]` + 18 個沒 checkbox 的 = 49 個未完成
        assert_eq!(count(|item| item.done), 7);
        assert_eq!(count(|item| !item.done), 49);
        assert_eq!(count(|item| item.date.is_none()), 12);
        // 數字是用另一支小程式獨立數出來的，不是看 parser 的輸出抄的
        // 唯一一個寫錯的日期
        assert_eq!(parsed.warnings.len(), 1);
        assert_eq!(
            parsed.warnings[0].kind,
            WarningKind::InvalidDate("@2026-13-01".into())
        );
    }
}
