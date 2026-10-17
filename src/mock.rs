//! mock 用的假資料：寫死在程式裡，不讀任務檔

use jiff::civil::{Date, date};

/// 一件事：哪天、標題、是不是假日（假日用另一個顏色）
pub struct Item {
    pub date: Date,
    pub title: &'static str,
    pub holiday: bool,
}

/// 2026 年 8 月到 2027 年 2 月的假資料（照日期排好）：假日 + 工作、生活上的事
///
/// 10 月特別多（比 10 月的週數還多），用來看一個月被撐高的樣子
pub fn items() -> Vec<Item> {
    let holiday = |date, title| Item {
        date,
        title,
        holiday: true,
    };
    let task = |date, title| Item {
        date,
        title,
        holiday: false,
    };
    vec![
        task(date(2026, 8, 8), "父親節聚餐"),
        task(date(2026, 8, 19), "繳信用卡費"),
        task(date(2026, 8, 28), "牙醫回診"),
        task(date(2026, 9, 7), "開學"),
        holiday(date(2026, 9, 25), "中秋節"),
        holiday(date(2026, 9, 28), "教師節"),
        task(date(2026, 9, 30), "拆 ticket、估人天"),
        task(date(2026, 10, 8), "跟 PM 開需求會議"),
        holiday(date(2026, 10, 9), "國慶日補假"),
        holiday(date(2026, 10, 10), "國慶日"),
        task(date(2026, 10, 12), "Code review：Amy 的 PR #482"),
        task(date(2026, 10, 14), "實作退款 API"),
        task(date(2026, 10, 16), "交週報"),
        task(date(2026, 10, 17), "看 roh-cal v2 的 mock"),
        task(date(2026, 10, 22), "準備上線清冊"),
        task(date(2026, 10, 23), "上線 freeze"),
        holiday(date(2026, 10, 25), "光復節"),
        holiday(date(2026, 10, 26), "光復節補假"),
        task(date(2026, 10, 27), "Release v2.3"),
        task(date(2026, 10, 29), "團隊 offsite"),
        task(date(2026, 11, 2), "期中考週"),
        task(date(2026, 11, 6), "AI 課作業繳交"),
        task(date(2026, 11, 20), "部門月會"),
        task(date(2026, 11, 27), "健康檢查"),
        task(date(2026, 12, 1), "年度考核自評"),
        task(date(2026, 12, 18), "期末考"),
        task(date(2026, 12, 24), "平安夜聚餐"),
        holiday(date(2026, 12, 25), "行憲紀念日"),
        task(date(2026, 12, 31), "跨年"),
        holiday(date(2027, 1, 1), "元旦"),
        task(date(2027, 1, 15), "交期末報告"),
        task(date(2027, 1, 29), "尾牙"),
        holiday(date(2027, 2, 5), "除夕"),
        holiday(date(2027, 2, 6), "春節"),
        holiday(date(2027, 2, 7), "初二"),
        task(date(2027, 2, 22), "開學"),
    ]
}
