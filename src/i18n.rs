//! 畫面上所有的字；換語言 = 換一組 `Texts`

use jiff::civil::Date;
use std::io::ErrorKind;

/// 英文月份全名，`[0]` 是一月
const MONTH_NAMES: [&str; 12] = [
    "January",
    "February",
    "March",
    "April",
    "May",
    "June",
    "July",
    "August",
    "September",
    "October",
    "November",
    "December",
];

/// 中文的星期，`[0]` 是星期一
const ZH_WEEKDAYS: [&str; 7] = ["一", "二", "三", "四", "五", "六", "日"];

/// 支援的語言
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Lang {
    /// 英文
    En,
    /// 繁體中文（台灣）
    ZhTw,
}

impl Lang {
    /// 從語言代碼判斷語言，不分大小寫，`_` 和 `-` 都可以；不支援的回傳 `None`
    ///
    /// 例：`en`、`en-US` → 英文；`zh-TW`、`zh_TW`、`zh-Hant`、`zh-Hant-TW`、`zh-HK` → 繁體中文
    pub fn from_code(code: &str) -> Option<Self> {
        let code = code.to_lowercase().replace('_', "-");
        if code == "en" || code.starts_with("en-") {
            Some(Self::En)
        } else if ["zh-tw", "zh-hant", "zh-hk", "zh-mo"]
            .iter()
            .any(|prefix| code.starts_with(prefix))
        {
            Some(Self::ZhTw)
        } else {
            None
        }
    }

    /// 系統的語言；讀不到或不支援就用英文
    pub fn from_system() -> Self {
        sys_locale::get_locale()
            .and_then(|code| Self::from_code(&code))
            .unwrap_or(Self::En)
    }

    /// 這個語言要用的那組字
    pub fn texts(self) -> Texts {
        match self {
            Self::En => Texts::en(),
            Self::ZhTw => Texts::zh_tw(),
        }
    }
}

/// 說明頁（`?`）的內容：左右兩欄的按鍵（照 tuxedo 的排法）
#[derive(Debug, Clone, Copy)]
pub struct Help {
    /// 上框線 `roh-cal · keys` 的 `keys`
    pub title: &'static str,
    /// 下框線的操作說明
    pub hint: &'static str,
    /// 左欄的段落
    pub left: &'static [HelpSection],
    /// 右欄的段落
    pub right: &'static [HelpSection],
}

/// 說明頁的一段：標題 + 每個按鍵做什麼
#[derive(Debug, Clone, Copy)]
pub struct HelpSection {
    /// 段落標題，例：`Calendar` / `月曆`
    pub title: &'static str,
    /// (按鍵, 做什麼)，例：`("t", "today")`；按鍵那欄每種語言都一樣（有 test 釘住）
    pub keys: &'static [(&'static str, &'static str)],
}

/// 畫面上要翻譯的字
///
/// 每種語言一個 function（`en()`、`zh_tw()`）；沒翻譯的欄位寫 `..Self::en()`，就會用英文
#[derive(Debug, Clone, Copy)]
pub struct Texts {
    /// 上邊線標題，例：`October 2026` / `2026 年 10 月`
    pub title: fn(year: i16, month: i8) -> String,
    /// 右邊的月份標記，例：`Nov` / `11 月`
    pub month_label: fn(month: i8) -> String,
    /// 星期列，星期一到星期日
    pub weekdays: [&'static str; 7],
    /// 下邊線的操作說明
    pub hint: &'static str,
    /// 當天清單的標題，例：`2026-10-12 Mon` / `2026-10-12（一）`
    pub day_title: fn(date: Date) -> String,
    /// 當天清單：那天沒有項目時顯示的字
    pub day_empty: &'static str,
    /// 當天清單最下面的操作說明
    pub day_hint: &'static str,
    /// 提示：寫檔前發現那一行在別處被改過，沒寫、已重新載入（暫時顯示在操作說明的位置）
    pub file_reloaded: &'static str,
    /// 提示：讀 / 寫任務檔失敗，例：`can't save the task file: permission denied`
    pub file_failed: fn(kind: ErrorKind) -> String,
    /// 提示：undo 成功，例：`undone: 週會`
    pub undone: fn(title: &str) -> String,
    /// 提示：沒有可以 undo 的
    pub nothing_to_undo: &'static str,
    /// 當天清單按了 `d`，問要不要刪；`subtasks` = 會一起刪掉的子任務數，例：`delete with 2 subtasks? y = yes, other = no`
    pub delete_confirm: fn(subtasks: usize) -> String,
    /// 說明頁（`?`）
    pub help: Help,
    /// 新增表單的標題（上框線）
    pub form_title: &'static str,
    /// 編輯表單的標題（上框線）
    pub form_edit_title: &'static str,
    /// 新增表單的欄位名：標題、日期、時間
    pub form_labels: [&'static str; 3],
    /// 新增表單最下面的操作說明
    pub form_hint: &'static str,
    /// 新增表單：游標在日期欄時，日期右邊的操作說明
    pub form_date_hint: &'static str,
    /// 新增表單：游標在時間欄時，時間右邊的說明
    pub form_time_hint: &'static str,
    /// 新增表單：標題是空的
    pub form_empty_title: &'static str,
    /// 新增表單：標題裡有 `@日期`
    pub form_date_in_title: &'static str,
    /// 新增表單：日期寫錯
    pub form_bad_date: &'static str,
    /// 新增表單：時間寫錯
    pub form_bad_time: &'static str,
}

impl Texts {
    /// 英文；也是其他語言沒翻譯時的預設
    pub fn en() -> Self {
        Self {
            title: |year, month| format!("{} {year}", month_name(month)),
            // 月份縮寫：取全名前 3 個字母，例：November → Nov
            month_label: |month| month_name(month)[..3].to_string(),
            weekdays: ["MON", "TUE", "WED", "THU", "FRI", "SAT", "SUN"],
            hint: "hjkl move · o open · v view · ? help · q quit",
            day_title: |date| date.strftime("%Y-%m-%d %a").to_string(),
            day_empty: "no items",
            day_hint: "a add · Space check · d delete · ? help",
            file_reloaded: "file changed elsewhere, reloaded",
            file_failed: |kind| format!("can't save the task file: {kind}"),
            undone: |title| format!("undone: {title}"),
            nothing_to_undo: "nothing to undo",
            delete_confirm: |subtasks| match subtasks {
                0 => "delete? y = yes, other = no".to_string(),
                1 => "delete with 1 subtask? y = yes, other = no".to_string(),
                n => format!("delete with {n} subtasks? y = yes, other = no"),
            },
            help: Help {
                title: "keys",
                hint: "?/q/Esc close",
                left: &[HelpSection {
                    title: "NAVIGATION",
                    keys: &[
                        ("h j k l", "move (arrow keys too)"),
                        ("t", "jump to today"),
                        ("o / Enter", "open detail"),
                        ("v", "switch view"),
                        (",", "open settings"),
                        ("Ctrl+C", "quit from anywhere"),
                    ],
                }],
                right: &[HelpSection {
                    title: "EDITING",
                    keys: &[
                        ("a", "add an item"),
                        ("e", "edit an item"),
                        ("Space / x", "check off / uncheck"),
                        ("d", "delete (with subtasks)"),
                        ("u", "undo"),
                    ],
                }],
            },
            form_title: "new item",
            form_edit_title: "edit item",
            form_labels: ["title", "date", "time"],
            form_hint: "Tab next · Enter save · Esc cancel",
            form_date_hint: "hjkl change date",
            form_time_hint: "optional",
            form_empty_title: "the title is empty",
            form_date_in_title: "the title can't contain an @date",
            form_bad_date: "date should look like 2026-10-12",
            form_bad_time: "time should look like 14:00 or 1400",
        }
    }

    /// 繁體中文（台灣）；星期列沒翻譯，用英文
    pub fn zh_tw() -> Self {
        Self {
            title: |year, month| format!("{year} 年 {month} 月"),
            month_label: |month| format!("{month:>2} 月"),
            hint: "hjkl 移動 · o 開啟 · v 切換 · ? 說明 · q 離開",
            day_title: |date| format!("{date}（{}）", zh_weekday(date)),
            day_empty: "這天沒有項目",
            day_hint: "a 新增 · Space 打勾 · d 刪除 · ? 說明",
            file_reloaded: "檔案在別處改過，已重新載入",
            file_failed: |kind| format!("任務檔存不了：{kind}"),
            undone: |title| format!("已復原：{title}"),
            nothing_to_undo: "沒有可以復原的",
            delete_confirm: |subtasks| match subtasks {
                0 => "刪除？y = 是，其他鍵 = 否".to_string(),
                n => format!("刪除（含 {n} 個子任務）？y = 是，其他鍵 = 否"),
            },
            help: Help {
                title: "按鍵",
                hint: "?/q/Esc 關閉",
                left: &[HelpSection {
                    title: "移動",
                    keys: &[
                        ("h j k l", "移動（方向鍵也可以）"),
                        ("t", "跳回今天"),
                        ("o / Enter", "打開當天清單"),
                        ("v", "切換大小月曆"),
                        (",", "打開設定頁"),
                        ("Ctrl+C", "直接離開（哪裡都可以）"),
                    ],
                }],
                right: &[HelpSection {
                    title: "編輯",
                    keys: &[
                        ("a", "新增項目"),
                        ("e", "編輯項目"),
                        ("Space / x", "打勾 / 取消"),
                        ("d", "刪除（連子任務）"),
                        ("u", "復原"),
                    ],
                }],
            },
            form_title: "新增項目",
            form_edit_title: "編輯項目",
            form_labels: ["標題", "日期", "時間"],
            form_hint: "Tab 換欄 · Enter 存 · Esc 取消",
            form_date_hint: "hjkl 換日期",
            form_time_hint: "可以空著",
            form_empty_title: "標題是空的",
            form_date_in_title: "標題不能有 @日期，請用日期欄",
            form_bad_date: "日期要像 2026-10-12",
            form_bad_time: "時間要像 14:00 或 1400",
            ..Self::en()
        }
    }
}

/// 英文月份全名；`month` 是 1～12
fn month_name(month: i8) -> &'static str {
    // jiff 的月份一定是 1～12，所以 month - 1 一定在 0～11
    MONTH_NAMES[usize::try_from(month - 1).unwrap_or(0)]
}

/// 中文的星期，例：星期一 → `一`
fn zh_weekday(date: Date) -> &'static str {
    // 星期一 = 0 … 星期日 = 6，一定在 0～6
    ZH_WEEKDAYS[usize::try_from(date.weekday().to_monday_zero_offset()).unwrap_or(0)]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_code_accepts_common_spellings() {
        for code in ["en", "en-US", "EN_gb"] {
            assert_eq!(Lang::from_code(code), Some(Lang::En), "{code}");
        }
        for code in ["zh-TW", "zh_TW", "zh-Hant", "zh-Hant-TW", "zh-HK"] {
            assert_eq!(Lang::from_code(code), Some(Lang::ZhTw), "{code}");
        }
    }

    #[test]
    fn from_code_rejects_unsupported() {
        // 簡體中文、日文、空字串、像 en 但不是 en 的
        for code in ["zh-CN", "zh-Hans", "ja-JP", "", "eng"] {
            assert_eq!(Lang::from_code(code), None, "{code}");
        }
    }

    #[test]
    fn english_month_names() {
        assert_eq!((Texts::en().title)(2026, 10), "October 2026");
        assert_eq!((Texts::en().month_label)(11), "Nov");
        assert_eq!((Texts::zh_tw().title)(2026, 10), "2026 年 10 月");
    }

    #[test]
    fn day_titles_show_the_date_and_weekday() {
        let monday = jiff::civil::date(2026, 10, 12);
        let sunday = jiff::civil::date(2026, 10, 18);
        assert_eq!((Texts::en().day_title)(monday), "2026-10-12 Mon");
        assert_eq!((Texts::zh_tw().day_title)(monday), "2026-10-12（一）");
        assert_eq!((Texts::zh_tw().day_title)(sunday), "2026-10-18（日）");
    }

    #[test]
    fn file_failed_says_why() {
        let denied = ErrorKind::PermissionDenied;
        assert_eq!(
            (Texts::en().file_failed)(denied),
            "can't save the task file: permission denied"
        );
        assert_eq!(
            (Texts::zh_tw().file_failed)(denied),
            "任務檔存不了：permission denied"
        );
    }

    #[test]
    fn undone_names_the_item() {
        assert_eq!((Texts::en().undone)("週會"), "undone: 週會");
        assert_eq!((Texts::zh_tw().undone)("週會"), "已復原：週會");
    }

    #[test]
    fn help_lists_the_same_keys_in_every_language() {
        // 只看按鍵那欄：每種語言的段落數、每段的按鍵和順序都一樣（翻譯時不會漏掉或多出一個鍵）
        let keys = |texts: Texts| -> Vec<Vec<&str>> {
            let help = texts.help;
            help.left
                .iter()
                .chain(help.right)
                .map(|section| section.keys.iter().map(|&(key, _)| key).collect())
                .collect()
        };
        assert_eq!(keys(Texts::en()), keys(Texts::zh_tw()));
        assert_eq!(keys(Texts::en()).len(), 2);
    }

    #[test]
    fn delete_confirm_counts_the_subtasks() {
        let en = Texts::en().delete_confirm;
        assert_eq!(en(0), "delete? y = yes, other = no");
        assert_eq!(en(1), "delete with 1 subtask? y = yes, other = no");
        assert_eq!(en(3), "delete with 3 subtasks? y = yes, other = no");
        assert_eq!(
            (Texts::zh_tw().delete_confirm)(2),
            "刪除（含 2 個子任務）？y = 是，其他鍵 = 否"
        );
    }
}
