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
    // Simplified Chinese, Japanese, an empty string, something like en that is not en
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
fn notices_in_both_languages() {
    use std::io::ErrorKind;
    let en = Lang::En.texts();
    assert_eq!(
        en.notice(&Notice::Failed(ErrorKind::NotFound)),
        " Could not write the task file (entity not found)"
    );
    let zh = Lang::ZhTw.texts();
    assert_eq!(zh.notice(&Notice::NotATask), " 行程沒有框，不能打勾");
    assert_eq!(
        zh.notice(&Notice::Mistakes(2)),
        " 任務檔有 2 處寫錯：按 3 到任務頁看是哪幾行"
    );
    assert_eq!(
        zh.notice(&Notice::Failed(ErrorKind::PermissionDenied)),
        " 寫不進任務檔（permission denied）"
    );
}

#[test]
fn when_says_the_date_time_and_range() {
    let item = |line: &str| crate::tasks::parse(line).items[0].clone();
    let zh = Lang::ZhTw.texts();
    assert_eq!(
        zh.when(&item("- [ ] Send the weekly update @2026-10-16 17:30")),
        "2026/10/16（五） 17:30"
    );
    assert_eq!(
        zh.when(&item("- Business trip to NYC @2026-10-20..2026-10-25")),
        "2026/10/20（二）～2026/10/25（日）"
    );
    assert_eq!(zh.when(&item("- [x] Organize the bookshelf")), "沒有日期");
    let en = Lang::En.texts();
    assert_eq!(
        en.when(&item("- Dentist appointment @2026-10-14 10:30")),
        "Wed Oct 14, 2026 10:30"
    );
    assert_eq!(
        en.problem(&Problem::Mistake(WarningKind::NoSuchDate(
            "@2026-13-01".into()
        ))),
        " @2026-13-01: no such day"
    );
}

#[test]
fn confirm_delete_calendar_counts_the_items() {
    let (en, zh) = (Lang::En.texts(), Lang::ZhTw.texts());
    assert_eq!(
        zh.confirm_delete_calendar("Moving", 0),
        " 刪除日曆「Moving」？d 或 y 刪除，其他鍵取消"
    );
    assert_eq!(
        zh.confirm_delete_calendar("Moving", 4),
        " 刪除日曆「Moving」和裡面的 4 件事？d 或 y 刪除，其他鍵取消"
    );
    assert_eq!(
        en.confirm_delete_calendar("Moving", 1),
        " Delete the calendar \"Moving\" and its item? d or y deletes, any other key cancels"
    );
    assert_eq!(
        en.confirm_delete_calendar("Moving", 4),
        " Delete the calendar \"Moving\" and its 4 items? d or y deletes, any other key cancels"
    );
}

#[test]
fn confirm_delete_counts_the_subtasks() {
    let zh = Lang::ZhTw.texts();
    assert_eq!(
        zh.confirm_delete("Go to the gym", 1),
        " 刪除「Go to the gym」？d 或 y 刪除，其他鍵取消"
    );
    assert_eq!(
        zh.confirm_delete("Write the design doc", 3),
        " 刪除「Write the design doc」和底下 2 件子任務？d 或 y 刪除，其他鍵取消"
    );
    assert_eq!(
        Lang::En.texts().confirm_delete("a", 2),
        " Delete \"a\" and its subtask? d or y deletes, any other key cancels"
    );
}

#[test]
fn help_pages_list_the_same_keys_in_both_languages() {
    // The key column is the same in every language (only the descriptions are translated): a key missing
    // from or extra in either language is caught
    let keys = |page: HelpPage| -> Vec<Vec<&str>> {
        page.short
            .iter()
            .chain(page.left)
            .chain(page.right)
            .map(|section| section.keys.iter().map(|&(key, _)| key).collect())
            .collect()
    };
    let (en, zh) = (Lang::En.texts(), Lang::ZhTw.texts());
    assert_eq!(keys(en.help), keys(zh.help));
    assert_eq!(en.help.left.len(), zh.help.left.len());
    // Where each section works is the same too
    let places = |page: HelpPage| -> Vec<&[Focus]> {
        page.short
            .iter()
            .chain(page.left)
            .chain(page.right)
            .map(|section| section.places)
            .collect()
    };
    assert_eq!(places(en.help), places(zh.help));
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
