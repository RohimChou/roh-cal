//! 畫面上所有的字；換語言 = 換一組 `Texts`

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

/// 畫面上要翻譯的字
///
/// 每種語言一個 function（`en()`、`zh_tw()`）；沒翻譯的欄位寫 `..Self::en()`，就會用英文
#[derive(Debug, Clone, Copy)]
pub struct Texts {
    /// 上邊線標題，例：`October 2026` / `2026 年 10 月`
    pub title: fn(year: i16, month: i8) -> String,
    /// 右邊的月份標記，例：`Nov` / `11 月`
    pub month_marker: fn(month: i8) -> String,
    /// 星期列，星期一到星期日
    pub weekdays: [&'static str; 7],
    /// 下邊線的操作說明
    pub hint: &'static str,
}

impl Texts {
    /// 英文；也是其他語言沒翻譯時的預設
    pub fn en() -> Self {
        Self {
            title: |year, month| format!("{} {year}", month_name(month)),
            // 月份縮寫：取全名前 3 個字母，例：November → Nov
            month_marker: |month| month_name(month)[..3].to_string(),
            weekdays: ["MON", "TUE", "WED", "THU", "FRI", "SAT", "SUN"],
            hint: "h/j/k/l move · t today · q quit",
        }
    }

    /// 繁體中文（台灣）；星期列沒翻譯，用英文
    pub fn zh_tw() -> Self {
        Self {
            title: |year, month| format!("{year} 年 {month} 月"),
            month_marker: |month| format!("{month:>2} 月"),
            hint: "h/j/k/l 移動 · t 今天 · q 離開",
            ..Self::en()
        }
    }
}

/// 英文月份全名；`month` 是 1～12
fn month_name(month: i8) -> &'static str {
    // jiff 的月份一定是 1～12，所以 month - 1 一定在 0～11
    MONTH_NAMES[usize::try_from(month - 1).unwrap_or(0)]
}
