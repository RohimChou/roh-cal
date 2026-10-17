//! 畫面上的字：中文、英文各一份，按 `L` 切換

use jiff::civil::Date;

/// 畫面用的語言
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Lang {
    Zh,
    En,
}

impl Lang {
    /// 另一個語言（按 `L` 切換）
    pub fn toggled(self) -> Self {
        match self {
            Self::Zh => Self::En,
            Self::En => Self::Zh,
        }
    }

    /// 這個語言的字
    pub fn texts(self) -> &'static Texts {
        match self {
            Self::Zh => &ZH,
            Self::En => &EN,
        }
    }
}

/// 一種語言的所有字
pub struct Texts {
    lang: Lang,
    /// 上框線左邊的標題
    pub title: &'static str,
    /// 下框線的操作說明
    pub hints: &'static str,
    /// 表頭：年月那欄（最多 6 格寬）
    pub header_month: &'static str,
    /// 表頭：週數那欄（4 格寬）
    pub header_week: &'static str,
    /// 表頭：右邊事項那欄
    pub header_items: &'static str,
    /// 表頭：side menu（日曆清單）
    pub header_side: &'static str,
    /// 表頭：版型 2 右邊那欄（從選取那天開始）
    pub header_upcoming: &'static str,
    /// 版型 2：今天那段標題後面加的字
    pub today: &'static str,
    /// 版型 2：選取那天沒有事
    pub no_items: &'static str,
    /// 表頭的星期一到星期日：一天 3 格寬，名字要剛好 2 格（前面補一個空白）
    pub weekdays: [&'static str; 7],
}

const ZH: Texts = Texts {
    lang: Lang::Zh,
    title: " roh-cal v2 mock：行事曆 ",
    hints: " hjkl 移動  Tab 切換  Space 勾選  s 側欄  1/2 版型  t 今天  L English  q 離開 ",
    header_month: "年月",
    header_week: " 週 ",
    header_items: " 重要事項",
    header_side: " 日曆",
    header_upcoming: " 當天起",
    today: " 今天",
    no_items: "沒有事",
    weekdays: ["一", "二", "三", "四", "五", "六", "日"],
};

const EN: Texts = Texts {
    lang: Lang::En,
    title: " roh-cal v2 mock: planner ",
    hints: " hjkl move  Tab switch  Space toggle  s sidebar  1/2 layout  t today  L 中文  q quit ",
    header_month: "",
    header_week: " Wk ",
    header_items: " Agenda",
    header_side: " Calendars",
    header_upcoming: " From this day",
    today: " today",
    no_items: "Nothing",
    weekdays: ["Mo", "Tu", "We", "Th", "Fr", "Sa", "Su"],
};

/// 英文的月份縮寫（左邊欄、標題用）
const EN_MONTHS: [&str; 12] = [
    "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
];
/// 英文的星期縮寫（標題用；表頭用 2 個字母的 `weekdays`）
const EN_WEEKDAYS: [&str; 7] = ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"];

impl Texts {
    /// 左邊欄的年份，例：`2027年` / `2027`
    pub fn year_label(&self, date: Date) -> String {
        match self.lang {
            Lang::Zh => format!("{}年", date.year()),
            Lang::En => date.year().to_string(),
        }
    }

    /// 左邊欄的月份，例：`10月` / `Oct`
    pub fn month_label(&self, date: Date) -> String {
        match self.lang {
            Lang::Zh => format!("{}月", date.month()),
            Lang::En => EN_MONTHS[month_index(date)].to_string(),
        }
    }

    /// 上框線右邊的選取日期，例：`2026/10/17（六）` / `Sat Oct 17, 2026`
    pub fn date_title(&self, date: Date) -> String {
        let weekday = date.weekday().to_monday_zero_offset() as usize;
        match self.lang {
            Lang::Zh => format!(
                "{}/{}/{}（{}）",
                date.year(),
                date.month(),
                date.day(),
                self.weekdays[weekday]
            ),
            Lang::En => format!(
                "{} {} {}, {}",
                EN_WEEKDAYS[weekday],
                EN_MONTHS[month_index(date)],
                date.day(),
                date.year()
            ),
        }
    }
}

impl Texts {
    /// 版型 2 每一天的標題，例：`10/17（六）` / `Sat Oct 17`
    pub fn day_heading(&self, date: Date) -> String {
        let weekday = date.weekday().to_monday_zero_offset() as usize;
        match self.lang {
            Lang::Zh => format!(
                "{}/{}（{}）",
                date.month(),
                date.day(),
                self.weekdays[weekday]
            ),
            Lang::En => format!(
                "{} {} {}",
                EN_WEEKDAYS[weekday],
                EN_MONTHS[month_index(date)],
                date.day()
            ),
        }
    }
}

/// 月份在陣列裡的位置：1 月 = 0
fn month_index(date: Date) -> usize {
    usize::try_from(date.month() - 1).unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use jiff::civil::date;

    #[test]
    fn labels_and_title_in_both_languages() {
        let day = date(2026, 10, 17);
        let zh = Lang::Zh.texts();
        assert_eq!(zh.year_label(day), "2026年");
        assert_eq!(zh.month_label(day), "10月");
        assert_eq!(zh.date_title(day), "2026/10/17（六）");
        let en = Lang::En.texts();
        assert_eq!(en.year_label(day), "2026");
        assert_eq!(en.month_label(day), "Oct");
        assert_eq!(en.month_label(date(2027, 1, 1)), "Jan");
        assert_eq!(en.date_title(day), "Sat Oct 17, 2026");
        assert_eq!(Lang::Zh.toggled(), Lang::En);
        assert_eq!(Lang::En.toggled(), Lang::Zh);
    }
}
