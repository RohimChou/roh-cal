//! 畫面上的字：英文、繁體中文各一份；換語言 = 換一組 `Texts`

use jiff::civil::Date;

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

    /// 這個語言的那組字
    pub fn texts(self) -> &'static Texts {
        match self {
            Self::En => &EN,
            Self::ZhTw => &ZH_TW,
        }
    }
}

/// 一種語言的所有字
#[derive(Debug)]
pub struct Texts {
    /// 這組字是哪個語言（年 / 月的寫法要用）
    lang: Lang,
    /// 最下面一列的操作說明
    pub hints: &'static str,
    /// 最下面一列的操作說明：terminal 夠寬、有 side menu 的時候（多一個 `s`）
    pub hints_side: &'static str,
    /// 月曆表頭：週數那欄（4 格寬）
    pub header_week: &'static str,
    /// 月曆表頭的星期一到星期日：每個剛好 2 格寬
    pub weekdays: [&'static str; 7],
    /// 右邊事項框的表頭
    pub header_items: &'static str,
    /// side menu（日曆清單）的表頭
    pub header_side: &'static str,
    /// side menu：檔案開頭、還沒有標題的項目
    pub ungrouped: &'static str,
}

const EN: Texts = Texts {
    lang: Lang::En,
    hints: " hjkl move  Tab switch  [] month  t today  q quit",
    hints_side: " hjkl move  Tab switch  [] month  t today  s calendars  q quit",
    header_week: " Wk ",
    weekdays: ["Mo", "Tu", "We", "Th", "Fr", "Sa", "Su"],
    header_items: " Agenda",
    header_side: " Calendars",
    ungrouped: "Ungrouped",
};

const ZH_TW: Texts = Texts {
    lang: Lang::ZhTw,
    hints: " hjkl 移動  Tab 切換  [] 換月  t 今天  q 離開",
    hints_side: " hjkl 移動  Tab 切換  [] 換月  t 今天  s 日曆  q 離開",
    header_week: " 週 ",
    weekdays: ["一", "二", "三", "四", "五", "六", "日"],
    header_items: " 重要事項",
    header_side: " 日曆",
    ungrouped: "未分組",
};

/// 英文的月份縮寫，`[0]` 是一月
const EN_MONTHS: [&str; 12] = [
    "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
];
/// 英文的星期縮寫（標題列用；表頭用 2 個字母的 `weekdays`），`[0]` 是星期一
const EN_WEEKDAYS: [&str; 7] = ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"];

impl Texts {
    /// 月曆左邊欄的年份，例：`2027年` / `2027`
    pub fn year_label(&self, date: Date) -> String {
        match self.lang {
            Lang::En => date.year().to_string(),
            Lang::ZhTw => format!("{}年", date.year()),
        }
    }

    /// 月曆左邊欄的月份，例：`Oct` / `10月`
    pub fn month_label(&self, date: Date) -> String {
        match self.lang {
            Lang::En => EN_MONTHS[month_index(date)].to_string(),
            Lang::ZhTw => format!("{}月", date.month()),
        }
    }

    /// 一個月的事放不下時，那個月下面那條線上的提示，例：`23 more` / `還有 23 件`
    pub fn more_items(&self, count: usize) -> String {
        match self.lang {
            Lang::En => format!("{count} more"),
            Lang::ZhTw => format!("還有 {count} 件"),
        }
    }

    /// 選取的日期（右邊事項框表頭的右邊），例：`Sat Oct 17, 2026` / `2026/10/17（六）`
    pub fn date_title(&self, date: Date) -> String {
        // 星期一 = 0、星期日 = 6
        let weekday = date.weekday().to_monday_zero_offset() as usize;
        match self.lang {
            Lang::En => format!(
                "{} {} {}, {}",
                EN_WEEKDAYS[weekday],
                EN_MONTHS[month_index(date)],
                date.day(),
                date.year()
            ),
            Lang::ZhTw => format!(
                "{}/{}/{}（{}）",
                date.year(),
                date.month(),
                date.day(),
                self.weekdays[weekday]
            ),
        }
    }
}

/// 月份在陣列裡的位置：月份是 1～12，換成 0～11（`month()` 是 i8，轉成 usize 才能當位置）
fn month_index(date: Date) -> usize {
    usize::try_from(date.month() - 1).unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use jiff::civil::date;

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
    fn year_and_month_labels_in_both_languages() {
        let en = Lang::En.texts();
        assert_eq!(en.year_label(date(2027, 1, 1)), "2027");
        assert_eq!(en.month_label(date(2027, 1, 1)), "Jan");
        assert_eq!(en.month_label(date(2026, 10, 17)), "Oct");
        assert_eq!(en.month_label(date(2026, 12, 31)), "Dec");
        let zh = Lang::ZhTw.texts();
        assert_eq!(zh.year_label(date(2027, 1, 1)), "2027年");
        assert_eq!(zh.month_label(date(2026, 10, 17)), "10月");
    }

    #[test]
    fn more_items_in_both_languages() {
        assert_eq!(Lang::En.texts().more_items(23), "23 more");
        assert_eq!(Lang::ZhTw.texts().more_items(1), "還有 1 件");
    }

    #[test]
    fn date_title_in_both_languages() {
        let en = Lang::En.texts();
        assert_eq!(en.date_title(date(2026, 10, 17)), "Sat Oct 17, 2026");
        assert_eq!(en.date_title(date(2027, 1, 4)), "Mon Jan 4, 2027");
        let zh = Lang::ZhTw.texts();
        assert_eq!(zh.date_title(date(2026, 10, 17)), "2026/10/17（六）");
        assert_eq!(zh.date_title(date(2026, 10, 18)), "2026/10/18（日）");
    }
}
