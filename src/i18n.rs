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
}

impl Texts {
    /// 英文；也是其他語言沒翻譯時的預設
    pub fn en() -> Self {
        Self {
            title: |year, month| format!("{} {year}", month_name(month)),
            // 月份縮寫：取全名前 3 個字母，例：November → Nov
            month_label: |month| month_name(month)[..3].to_string(),
            weekdays: ["MON", "TUE", "WED", "THU", "FRI", "SAT", "SUN"],
            hint: "hjkl move · o open · t today · v view · q quit",
            day_title: |date| date.strftime("%Y-%m-%d %a").to_string(),
            day_empty: "no items",
            day_hint: "j/k select · Space/x check · q/Esc close",
            file_reloaded: "file changed elsewhere, reloaded",
            file_failed: |kind| format!("can't save the task file: {kind}"),
        }
    }

    /// 繁體中文（台灣）；星期列沒翻譯，用英文
    pub fn zh_tw() -> Self {
        Self {
            title: |year, month| format!("{year} 年 {month} 月"),
            month_label: |month| format!("{month:>2} 月"),
            hint: "hjkl 移動 · o 開啟 · t 今天 · v 切換 · q 離開",
            day_title: |date| format!("{date}（{}）", zh_weekday(date)),
            day_empty: "這天沒有項目",
            day_hint: "j/k 選擇 · Space/x 打勾 · q/Esc 關閉",
            file_reloaded: "檔案在別處改過，已重新載入",
            file_failed: |kind| format!("任務檔存不了：{kind}"),
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
}
