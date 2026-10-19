//! On-screen text: one set in English, one in Traditional Chinese; switching language = switching `Texts`

use crate::app::{Focus, Notice, Row, View};
use crate::edit::Problem;
use crate::tasks::{Item, WarningKind};
use jiff::civil::Date;

/// Supported languages
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Lang {
    /// English
    En,
    /// Traditional Chinese (Taiwan)
    ZhTw,
}

impl Lang {
    /// Work out the language from a language code, ignoring case, with `_` or `-`; `None` if unsupported
    ///
    /// E.g. `en`, `en-US` → English; `zh-TW`, `zh_TW`, `zh-Hant`, `zh-Hant-TW`, `zh-HK` → Traditional Chinese
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

    /// The system language; English if it cannot be read or is not supported
    pub fn from_system() -> Self {
        sys_locale::get_locale()
            .and_then(|code| Self::from_code(&code))
            .unwrap_or(Self::En)
    }

    /// The text set for this language
    pub fn texts(self) -> &'static Texts {
        match self {
            Self::En => &EN,
            Self::ZhTw => &ZH_TW,
        }
    }
}

/// All the text for one language
#[derive(Debug)]
pub struct Texts {
    /// Which language this set is in (needed to write years / months)
    lang: Lang,
    /// The key hints on the bottom row, after `123` and `views` (`s` = side menu: hides / shows it when wide
    /// enough, otherwise opens it over the left side)
    pub hints: &'static str,
    /// After the layout keys `123` at the left end of the bottom row (calendar views and the tasks page)
    pub views: &'static str,
    /// Calendar header: the week number column (4 cells wide)
    pub header_week: &'static str,
    /// Monday to Sunday in the calendar header: each exactly 2 cells wide
    pub weekdays: [&'static str; 7],
    /// The agenda's header
    pub header_items: &'static str,
    /// The side menu (calendar list) header
    pub header_side: &'static str,
    /// The switch at the bottom of the side menu: hide done tasks (`[calendar_list] hide_done`)
    pub hide_done: &'static str,
    /// The tasks page header
    pub header_tasks: &'static str,
    /// The key hints on the tasks page's bottom row, after `123` and `views`
    pub hints_tasks: &'static str,
    /// The key hints on the bottom row while layout 2's day popup is open
    pub hints_day: &'static str,
    /// The key hints on the bottom row while the keyboard is in the side menu (calendar list)
    pub hints_side: &'static str,
    /// The keys on the bottom border of the editor's `#` list
    pub hash_list_keys: &'static str,
    /// The editor's words: the fields of what the line reads as, and the hint row (what can be typed)
    pub editor_words: EditorWords,
    /// The title on the editor's top border when adding a calendar
    pub add_calendar_title: &'static str,
    /// The calendar a new item goes into when the task file has no calendar yet (`# 預設`)
    pub default_calendar: &'static str,
    /// The title on the editor's top border when renaming a calendar
    pub rename_calendar_title: &'static str,
    /// The top level in the `#` list when picking where a calendar goes (outside every calendar)
    pub top_level: &'static str,
    /// Side menu: items at the top of the file, before any heading
    pub ungrouped: &'static str,
    /// The help page (`?`): the short version lists the keys that work where the keyboard is, the full
    /// version (`Tab`) every key, including the editor, rescheduling and the mouse
    pub help: HelpPage,
    /// The `設定` in `roh-cal ∙ 設定` on the settings page's top border
    pub settings_title: &'static str,
    /// The key hints on the settings page's bottom border
    pub settings_hint: &'static str,
    /// After `R` on the settings page: the bottom border switches to this, asking whether to reset everything
    pub settings_reset_all: &'static str,
    /// The color picker's top border, before the role's name (`自訂顏色 ∙ today`)
    pub picker_title: &'static str,
    /// The color picker's key hints on its bottom border
    pub picker_hint: &'static str,
    /// The color picker's last line: a color code can be typed too
    pub picker_typing: &'static str,
}

/// The editor's words: the labels of the fields under the typed line (what it reads as), and the hint row
/// (what can be typed: each with the same label, then what to type)
#[derive(Debug, Clone, Copy)]
pub struct EditorWords {
    /// In front of the hint row: 「可以打」
    pub can_type: &'static str,
    /// An item's title
    pub title: &'static str,
    /// The title field while nothing is typed after `- [ ] ` yet
    pub title_placeholder: &'static str,
    /// An item's date (`@2026-10-19`)
    pub date: &'static str,
    /// Its time (`@10:00`)
    pub time: &'static str,
    /// Its priority (`! !! !!!`)
    pub priority: &'static str,
    /// The priorities: high, medium, low
    pub levels: [&'static str; 3],
    /// The calendar an item is in (`#name` picks another)
    pub calendar: &'static str,
    /// The item a subtask goes under
    pub under: &'static str,
    /// A calendar's name
    pub name: &'static str,
    /// The name field while nothing is typed yet
    pub name_placeholder: &'static str,
    /// A calendar's kind: shown only for a holiday calendar
    pub kind: &'static str,
    /// A holiday calendar (`type:holiday`)
    pub holiday: &'static str,
    /// The calendar a calendar is inside (`#name` picks another)
    pub inside: &'static str,
    /// What to type before part of a calendar's name: `#name`
    pub name_key: &'static str,
}

/// The help page: the short version (what `?` opens: the sections of `short` that work where the keyboard
/// is), the full version (`Tab`: every key, in the two columns `left` and `right`) and the text on the borders
#[derive(Debug, Clone, Copy)]
pub struct HelpPage {
    /// The short version's `按鍵` in `roh-cal ∙ 按鍵 ∙ 月曆` on the top border (followed by where the
    /// keyboard is, `Texts::help_place`)
    pub short_title: &'static str,
    /// The short version's key hints on the bottom border
    pub short_hint: &'static str,
    /// The full version's top border, after `roh-cal ∙ `; it also says what the grayed-out sections mean
    pub full_title: &'static str,
    /// The full version's key hints on the bottom border
    pub full_hint: &'static str,
    /// The short version's sections, each with the few keys used most where it works; the ones for one
    /// place go on the left, the ones that work anywhere on the right
    pub short: &'static [HelpSection],
    /// The full version's sections in the left column
    pub left: &'static [HelpSection],
    /// The full version's sections in the right column
    pub right: &'static [HelpSection],
}

/// One help page section: its name + what each key does, and where it works
#[derive(Debug, Clone, Copy)]
pub struct HelpSection {
    /// The section name, e.g. `Calendar` / `月曆`
    pub title: &'static str,
    /// (key, what it does), e.g. `("t", "today")`; the key column is the same in every language
    /// (pinned by a test)
    pub keys: &'static [(&'static str, &'static str)],
    /// Where the keyboard has to be for these keys to work; empty = anywhere (pinned to be the same in
    /// every language by a test)
    pub places: &'static [Focus],
}

impl HelpSection {
    /// Whether these keys work with the keyboard in `focus`
    pub fn works_in(&self, focus: Focus) -> bool {
        self.places.is_empty() || self.places.contains(&focus)
    }
}

/// English text
const EN: Texts = Texts {
    lang: Lang::En,
    hints: " hjkl[] move  Tab pane  t today  s calendars  ? help  , settings  q quit",
    views: "view",
    header_week: " Wk ",
    weekdays: ["Mo", "Tu", "We", "Th", "Fr", "Sa", "Su"],
    header_items: " Agenda",
    header_side: " Calendars",
    hide_done: "Hide done",
    header_tasks: " Tasks",
    hints_tasks: " jk move  hl fold  Space check  e edit  a add  s calendars  ? help  , settings  q quit",
    hints_day: " jk move  Space check  e edit  a add  r reschedule  d delete  Esc close",
    hints_side: " jk move  Space show / hide  a/A add calendar  e rename  d delete  Esc back",
    hash_list_keys: " Enter pick  Esc close ",
    editor_words: EditorWords {
        can_type: "Add",
        title: "Title",
        title_placeholder: "(type it after - [ ])",
        date: "Date",
        time: "Time",
        priority: "Priority",
        levels: ["high", "medium", "low"],
        calendar: "Calendar",
        under: "Under",
        name: "Name",
        name_placeholder: "(type its name)",
        kind: "Kind",
        holiday: "holiday calendar",
        inside: "Inside",
        name_key: "#name",
    },
    add_calendar_title: " New calendar ",
    default_calendar: "Default",
    rename_calendar_title: " Rename calendar ",
    top_level: "Top level",
    ungrouped: "Ungrouped",
    help: HelpPage {
        short_title: "keys",
        short_hint: "?/Tab all keys  q/Esc close",
        full_title: "all keys, gray: not here",
        full_hint: "Tab keys for here  ?/q/Esc close",
        short: &[
            HelpSection {
                title: "Calendar",
                keys: &[
                    ("hjkl", "move"),
                    ("[ / ]", "previous / next month"),
                    ("t", "today"),
                    ("Enter", "the day's items"),
                    ("a", "add on that day"),
                ],
                places: &[Focus::Calendar],
            },
            HelpSection {
                title: "Items",
                keys: &[
                    ("j / k", "choose"),
                    ("Space", "check off"),
                    ("/", "in progress"),
                    ("e", "edit"),
                    ("a / A", "add / add a subtask"),
                    ("r", "reschedule"),
                    ("d", "delete"),
                    ("u", "undo"),
                ],
                places: &[Focus::Agenda, Focus::Day],
            },
            HelpSection {
                title: "Tasks page",
                keys: &[
                    ("j / k", "choose"),
                    ("gg / G", "first / last"),
                    ("l / h", "expand / collapse"),
                    ("Space", "check off"),
                    ("/", "in progress"),
                    ("e", "edit"),
                    ("a / A", "add / add a subtask"),
                    ("r", "reschedule"),
                    ("d", "delete"),
                    ("u", "undo"),
                    ("o", "open in editor"),
                ],
                places: &[Focus::Tasks],
            },
            HelpSection {
                title: "Calendar list",
                keys: &[
                    ("j / k", "choose"),
                    ("gg / G", "first / last"),
                    ("Space", "show / hide"),
                    ("a / A", "add / add inside"),
                    ("e", "rename"),
                    ("d", "delete"),
                    ("L / H", "one level in / out"),
                ],
                places: &[Focus::Side],
            },
            HelpSection {
                title: "Views",
                keys: &[
                    ("1", "calendar + agenda"),
                    ("2", "month grid"),
                    ("3", "tasks page"),
                    ("s", "calendar list"),
                    ("Tab", "next pane"),
                ],
                places: &[],
            },
            HelpSection {
                title: "System",
                keys: &[("?", "help"), (",", "settings"), ("q", "quit")],
                places: &[],
            },
        ],
        left: &[
            HelpSection {
                title: "Calendar",
                keys: &[
                    ("hjkl / arrows", "a day / a week"),
                    ("[ / ]", "previous / next month"),
                    ("t", "today"),
                    ("Enter", "go into the day's items"),
                    ("a", "add on that day"),
                ],
                places: &[Focus::Calendar],
            },
            HelpSection {
                title: "Views",
                keys: &[
                    ("1", "calendar + agenda"),
                    ("2", "month grid"),
                    ("3", "tasks page"),
                    ("Tab / Shift+Tab", "next / previous pane"),
                    ("s", "hide / show calendars"),
                    ("Esc", "back to the calendar"),
                ],
                places: &[],
            },
            HelpSection {
                title: "Tasks page",
                keys: &[
                    ("gg / G", "first / last item"),
                    ("l / h", "expand / collapse"),
                    ("+ / -", "expand / collapse all"),
                    ("L / H", "indent / outdent"),
                    ("J / K", "move down / up"),
                    ("[ / ]", "previous / next calendar"),
                    ("o", "open in editor"),
                ],
                places: &[Focus::Tasks],
            },
            HelpSection {
                title: "Mouse",
                keys: &[
                    ("wheel", "a week / an item"),
                    ("click", "select"),
                    ("double-click", "open a day / check"),
                    ("click outside", "close the box"),
                ],
                places: &[],
            },
            HelpSection {
                title: "System",
                keys: &[("?", "help"), (",", "settings"), ("q / Ctrl+C", "quit")],
                places: &[],
            },
        ],
        right: &[
            HelpSection {
                title: "Agenda, day popup, tasks page",
                keys: &[
                    ("j / k", "next / previous item"),
                    ("Space / x", "check / uncheck"),
                    ("/", "in progress / back to open"),
                    ("e / Enter", "edit the line"),
                    ("a / A", "add / add a subtask"),
                    ("r", "reschedule"),
                    ("d", "delete with subtasks"),
                    ("u", "undo"),
                ],
                places: &[Focus::Agenda, Focus::Day, Focus::Tasks],
            },
            HelpSection {
                title: "Editor",
                keys: &[
                    ("Enter", "save"),
                    ("Esc", "cancel"),
                    ("Ctrl+U", "clear"),
                    ("#", "pick a calendar"),
                ],
                places: &[],
            },
            HelpSection {
                title: "Reschedule",
                keys: &[
                    ("hjkl [ ] t", "pick the new day"),
                    ("Enter", "move it there"),
                    ("Esc", "cancel"),
                ],
                places: &[Focus::Agenda, Focus::Day, Focus::Tasks],
            },
            HelpSection {
                title: "Calendar list",
                keys: &[
                    ("j / k", "choose"),
                    ("gg / G", "first / last one"),
                    ("Space / Enter", "check / uncheck"),
                    ("a / A", "add / add inside"),
                    ("e", "rename it"),
                    ("d", "delete it, items too"),
                    ("L / H", "one level in / out"),
                ],
                places: &[Focus::Side],
            },
            HelpSection {
                title: "Settings",
                keys: &[
                    ("j / k", "choose a row"),
                    ("h / l", "change it"),
                    ("e", "custom color"),
                    ("r / R", "reset it / all"),
                ],
                places: &[],
            },
        ],
    },
    settings_title: "settings",
    settings_hint: "jk pick  hl change  r/R reset  Esc close",
    settings_reset_all: "Reset all? y yes, other keys cancel",
    picker_title: "Custom color",
    picker_hint: "jk pick  hl HL change  Enter keep  Esc undo",
    picker_typing: "Or type a color code: a6e3a1",
};

/// Traditional Chinese text
const ZH_TW: Texts = Texts {
    lang: Lang::ZhTw,
    hints: " hjkl[] 移動  Tab 切換  t 今天  s 日曆  ? 說明  , 設定  q 離開",
    views: "版型",
    header_week: " 週 ",
    weekdays: ["一", "二", "三", "四", "五", "六", "日"],
    header_items: " 重要事項",
    header_side: " 日曆",
    hide_done: "隱藏做完的",
    header_tasks: " 任務",
    hints_tasks: " jk 移動  hl 收合  Space 打勾  e 編輯  a 新增  s 日曆  ? 說明  , 設定  q 離開",
    hints_day: " jk 移動  Space 打勾  e 編輯  a 新增  r 改期  d 刪除  Esc 關閉",
    hints_side: " jk 移動  Space 勾選  a/A 新增日曆  e 改名  d 刪除  Esc 回去",
    hash_list_keys: " Enter 選定  Esc 收起 ",
    editor_words: EditorWords {
        can_type: "可以打",
        title: "標題",
        title_placeholder: "（打在 - [ ] 後面）",
        date: "日期",
        time: "時間",
        priority: "優先度",
        levels: ["高", "中", "低"],
        calendar: "日曆",
        under: "上層事項",
        name: "名字",
        name_placeholder: "（打日曆的名字）",
        kind: "類型",
        holiday: "假日日曆",
        inside: "上層日曆",
        name_key: "#名字",
    },
    add_calendar_title: " 新增日曆 ",
    default_calendar: "預設",
    rename_calendar_title: " 日曆改名 ",
    top_level: "最外層",
    ungrouped: "未分組",
    help: HelpPage {
        short_title: "按鍵",
        short_hint: "?/Tab 全部按鍵  q/Esc 關閉",
        full_title: "全部按鍵，灰的這裡用不到",
        full_hint: "Tab 這裡的按鍵  ?/q/Esc 關閉",
        short: &[
            HelpSection {
                title: "月曆",
                keys: &[
                    ("hjkl", "移動"),
                    ("[ / ]", "上個月 / 下個月"),
                    ("t", "今天"),
                    ("Enter", "那天的事"),
                    ("a", "在那天新增"),
                ],
                places: &[Focus::Calendar],
            },
            HelpSection {
                title: "事項",
                keys: &[
                    ("j / k", "選"),
                    ("Space", "打勾"),
                    ("/", "進行中"),
                    ("e", "編輯"),
                    ("a / A", "新增 / 新增子任務"),
                    ("r", "改期"),
                    ("d", "刪除"),
                    ("u", "復原"),
                ],
                places: &[Focus::Agenda, Focus::Day],
            },
            HelpSection {
                title: "任務頁",
                keys: &[
                    ("j / k", "選"),
                    ("gg / G", "第一件 / 最後一件"),
                    ("l / h", "展開 / 收起"),
                    ("Space", "打勾"),
                    ("/", "進行中"),
                    ("e", "編輯"),
                    ("a / A", "新增 / 新增子任務"),
                    ("r", "改期"),
                    ("d", "刪除"),
                    ("u", "復原"),
                    ("o", "編輯器"),
                ],
                places: &[Focus::Tasks],
            },
            HelpSection {
                title: "日曆清單",
                keys: &[
                    ("j / k", "選"),
                    ("gg / G", "第一個 / 最後一個"),
                    ("Space", "勾選"),
                    ("a / A", "新增 / 新增子日曆"),
                    ("e", "改名"),
                    ("d", "刪除"),
                    ("L / H", "往內 / 往外一層"),
                ],
                places: &[Focus::Side],
            },
            HelpSection {
                title: "畫面",
                keys: &[
                    ("1", "月曆 + 事項"),
                    ("2", "大月曆"),
                    ("3", "任務頁"),
                    ("s", "日曆清單"),
                    ("Tab", "換框"),
                ],
                places: &[],
            },
            HelpSection {
                title: "系統",
                keys: &[("?", "說明"), (",", "設定"), ("q", "離開")],
                places: &[],
            },
        ],
        left: &[
            HelpSection {
                title: "月曆",
                keys: &[
                    ("hjkl / arrows", "移一天 / 一週"),
                    ("[ / ]", "上個月 / 下個月"),
                    ("t", "跳回今天"),
                    ("Enter", "進到那天的事"),
                    ("a", "在那天新增"),
                ],
                places: &[Focus::Calendar],
            },
            HelpSection {
                title: "畫面",
                keys: &[
                    ("1", "月曆 + 事項"),
                    ("2", "大月曆"),
                    ("3", "任務頁"),
                    ("Tab / Shift+Tab", "換框 / 反方向換框"),
                    ("s", "收起 / 打開日曆清單"),
                    ("Esc", "回到月曆"),
                ],
                places: &[],
            },
            HelpSection {
                title: "任務頁",
                keys: &[
                    ("gg / G", "第一件 / 最後一件"),
                    ("l / h", "展開 / 收起"),
                    ("+ / -", "全部展開 / 全部收起"),
                    ("L / H", "縮排 / 取消縮排"),
                    ("J / K", "往下 / 往上搬"),
                    ("[ / ]", "上一個 / 下一個日曆"),
                    ("o", "編輯器"),
                ],
                places: &[Focus::Tasks],
            },
            HelpSection {
                title: "滑鼠",
                keys: &[
                    ("wheel", "移一週 / 一件"),
                    ("click", "選"),
                    ("double-click", "打開那天的事 / 打勾"),
                    ("click outside", "關掉跳窗"),
                ],
                places: &[],
            },
            HelpSection {
                title: "系統",
                keys: &[("?", "說明"), (",", "設定"), ("q / Ctrl+C", "離開")],
                places: &[],
            },
        ],
        right: &[
            HelpSection {
                title: "事項、跳窗、任務頁",
                keys: &[
                    ("j / k", "下一件 / 上一件"),
                    ("Space / x", "打勾 / 取消"),
                    ("/", "進行中 / 取消"),
                    ("e / Enter", "編輯那一行"),
                    ("a / A", "新增 / 新增子任務"),
                    ("r", "改期"),
                    ("d", "刪除（連子任務）"),
                    ("u", "復原"),
                ],
                places: &[Focus::Agenda, Focus::Day, Focus::Tasks],
            },
            HelpSection {
                title: "編輯框",
                keys: &[
                    ("Enter", "存檔"),
                    ("Esc", "取消"),
                    ("Ctrl+U", "清掉"),
                    ("#", "換日曆"),
                ],
                places: &[],
            },
            HelpSection {
                title: "改期",
                keys: &[
                    ("hjkl [ ] t", "選新的日期"),
                    ("Enter", "搬過去"),
                    ("Esc", "取消"),
                ],
                places: &[Focus::Agenda, Focus::Day, Focus::Tasks],
            },
            HelpSection {
                title: "日曆清單",
                keys: &[
                    ("j / k", "選日曆"),
                    ("gg / G", "第一個 / 最後一個"),
                    ("Space / Enter", "勾選 / 取消"),
                    ("a / A", "新增 / 新增子日曆"),
                    ("e", "改名"),
                    ("d", "刪除（連裡面的事）"),
                    ("L / H", "往內 / 往外一層"),
                ],
                places: &[Focus::Side],
            },
            HelpSection {
                title: "設定頁",
                keys: &[
                    ("j / k", "選一列"),
                    ("h / l", "改值"),
                    ("e", "自訂顏色"),
                    ("r / R", "還原一列 / 全部"),
                ],
                places: &[],
            },
        ],
    },
    settings_title: "設定",
    settings_hint: "jk 選  hl 改  r 還原  R 全部  Esc 關閉",
    settings_reset_all: "全部還原成預設？y 還原，其他鍵取消",
    picker_title: "自訂顏色",
    picker_hint: "jk 選  hl HL 調  Enter 存  Esc 取消",
    picker_typing: "也可以直接打色碼：a6e3a1",
};

/// English month abbreviations; `[0]` is January
const EN_MONTHS: [&str; 12] = [
    "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
];
/// English weekday abbreviations (for title rows; the header uses the 2-letter `weekdays`); `[0]` is Monday
const EN_WEEKDAYS: [&str; 7] = ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"];

impl Texts {
    /// The year in the calendar's left column, e.g. `2027年` / `2027`
    pub fn year_label(&self, date: Date) -> String {
        match self.lang {
            Lang::En => date.year().to_string(),
            Lang::ZhTw => format!("{}年", date.year()),
        }
    }

    /// The month in the calendar's left column, e.g. `Oct` / `10月`
    pub fn month_label(&self, date: Date) -> String {
        match self.lang {
            Lang::En => EN_MONTHS[month_index(date)].to_string(),
            Lang::ZhTw => format!("{}月", date.month()),
        }
    }

    /// A day's tasks not done yet, on the right of the day popup's top border, e.g. `3 open` / `未完成 3`
    pub fn day_open(&self, count: usize) -> String {
        match self.lang {
            Lang::En => format!("{count} open"),
            Lang::ZhTw => format!("未完成 {count}"),
        }
    }

    /// A day's tasks done, after `day_open`, e.g. `2 done` / `已完成 2`
    pub fn day_done(&self, count: usize) -> String {
        match self.lang {
            Lang::En => format!("{count} done"),
            Lang::ZhTw => format!("已完成 {count}"),
        }
    }

    /// The hint on the line below a month when its items do not fit, e.g. `23 more` / `還有 23 件`
    pub fn more_items(&self, count: usize) -> String {
        match self.lang {
            Lang::En => format!("{count} more"),
            Lang::ZhTw => format!("還有 {count} 件"),
        }
    }

    /// The notice that briefly replaces the bottom row (the result of writing the file),
    /// e.g. `寫不進任務檔（permission denied）`
    pub fn notice(&self, notice: &Notice) -> String {
        match (self.lang, notice) {
            (Lang::En, Notice::Reloaded) => {
                " The task file changed elsewhere: reloaded, nothing written".into()
            }
            (Lang::ZhTw, Notice::Reloaded) => {
                " 任務檔在別的地方改過了：已重新載入，這次沒寫".into()
            }
            (Lang::En, Notice::Failed(kind)) => format!(" Could not write the task file ({kind})"),
            (Lang::ZhTw, Notice::Failed(kind)) => format!(" 寫不進任務檔（{kind}）"),
            (Lang::En, Notice::NotATask) => " An event has no box to check".into(),
            (Lang::ZhTw, Notice::NotATask) => " 行程沒有框，不能打勾".into(),
            (Lang::En, Notice::Mistakes(count)) => {
                format!(
                    " The task file has {count} mistake(s): press 3 to see them on the tasks page"
                )
            }
            (Lang::ZhTw, Notice::Mistakes(count)) => {
                format!(" 任務檔有 {count} 處寫錯：按 3 到任務頁看是哪幾行")
            }
            (Lang::En, Notice::Undone(title)) => format!(" Undone: {title}"),
            (Lang::ZhTw, Notice::Undone(title)) => format!(" 已復原：{title}"),
            (Lang::En, Notice::NothingToUndo) => " Nothing to undo".into(),
            (Lang::ZhTw, Notice::NothingToUndo) => " 沒有可以復原的".into(),
            (Lang::En, Notice::SettingsFailed(kind)) => {
                format!(" Could not save the settings to the config file ({kind})")
            }
            (Lang::ZhTw, Notice::SettingsFailed(kind)) => {
                format!(" 設定存不回設定檔（{kind}）")
            }
            (Lang::En, Notice::NoEditor(name)) => {
                format!(" Could not start the editor {name}: set $EDITOR, e.g. export EDITOR=nvim")
            }
            (Lang::ZhTw, Notice::NoEditor(name)) => {
                format!(" 開不了編輯器 {name}：設定 $EDITOR，例：export EDITOR=nvim")
            }
        }
    }

    /// Right of the tasks page header: how many mistakes the task file has, e.g. `1 處寫錯` / `1 mistake(s)`
    pub fn mistakes(&self, count: usize) -> String {
        match self.lang {
            Lang::En => format!("{count} mistake(s) "),
            Lang::ZhTw => format!("{count} 處寫錯 "),
        }
    }

    /// The tasks page's bottom row: what is wrong with the chosen line, e.g. ` 第 44 行：@2026-13-01：沒有這一天`
    pub fn tasks_mistake(&self, line: usize, kind: &WarningKind) -> String {
        match self.lang {
            Lang::En => format!(" Line {line}: {}", self.mistake(kind)),
            Lang::ZhTw => format!(" 第 {line} 行：{}", self.mistake(kind)),
        }
    }

    /// The tasks page's bottom row: the chosen line's line number in the file and its original text,
    /// e.g. ` 第 6 行  - [ ] Implement the refund API @2026-10-14`
    pub fn tasks_detail(&self, line: usize, text: &str) -> String {
        match self.lang {
            Lang::En => format!(" Line {line}  {text}"),
            Lang::ZhTw => format!(" 第 {line} 行  {text}"),
        }
    }

    /// The title on the editor's top border when editing an item: ` 編輯事項 ` / ` Edit item `
    pub fn edit_title(&self) -> &'static str {
        match self.lang {
            Lang::En => " Edit item ",
            Lang::ZhTw => " 編輯事項 ",
        }
    }

    /// The title on the editor's top border when adding: ` 新增事項 ` / ` 新增子任務 `
    pub fn add_title(&self, subtask: bool) -> &'static str {
        match (self.lang, subtask) {
            (Lang::En, false) => " New item ",
            (Lang::En, true) => " New subtask ",
            (Lang::ZhTw, false) => " 新增事項 ",
            (Lang::ZhTw, true) => " 新增子任務 ",
        }
    }

    /// The editor's bottom border: how to save, how to cancel (what can be typed is on the hint row inside)
    pub fn editor_keys(&self) -> &'static str {
        match self.lang {
            Lang::En => " Enter save  Esc cancel ",
            Lang::ZhTw => " Enter 存檔  Esc 取消 ",
        }
    }

    /// When an item is, e.g. `2026/10/16（五） 17:30`, `2026/10/20（二）～2026/10/25（日）`, `沒有日期`
    pub fn when(&self, item: &Item) -> String {
        match item.date {
            None => match self.lang {
                Lang::En => "no date".to_string(),
                Lang::ZhTw => "沒有日期".to_string(),
            },
            Some(date) => {
                let mut when = self.date_title(date);
                if let Some(time) = item.time {
                    when.push_str(&format!(" {}", time.strftime("%H:%M")));
                }
                if let Some(end) = item.end {
                    when.push_str(&format!("～{}", self.date_title(end)));
                }
                when
            }
        }
    }

    /// What is wrong with the line typed in the editor (it cannot be saved), e.g. `@2026-13-01：沒有這一天`
    pub fn problem(&self, problem: &Problem) -> String {
        match (self.lang, problem) {
            (Lang::En, Problem::NotAnItem) => {
                " Start with \"- [ ] \" for a task, \"- \" for an event".into()
            }
            (Lang::ZhTw, Problem::NotAnItem) => " 開頭寫「- [ ] 」是任務，寫「- 」是行程".into(),
            (Lang::En, Problem::EmptyTitle) => " The title is empty".into(),
            (Lang::ZhTw, Problem::EmptyTitle) => " 標題是空的".into(),
            (Lang::En, Problem::EmptyName) => " The name is empty".into(),
            (Lang::ZhTw, Problem::EmptyName) => " 名字是空的".into(),
            (Lang::En, Problem::NameTaken) => " There is already a calendar with this name".into(),
            (Lang::ZhTw, Problem::NameTaken) => " 已經有這個名字的日曆了".into(),
            (_, Problem::Mistake(kind)) => format!(" {}", self.mistake(kind)),
        }
    }

    /// A mistake in the task file, e.g. `@2026-13-01：沒有這一天` / `@2026-13-01: no such day`
    pub fn mistake(&self, kind: &WarningKind) -> String {
        match (self.lang, kind) {
            (Lang::En, WarningKind::NoSuchDate(token)) => format!("{token}: no such day"),
            (Lang::ZhTw, WarningKind::NoSuchDate(token)) => format!("{token}：沒有這一天"),
            (Lang::En, WarningKind::NoSuchTime(token)) => format!("{token}: no such time"),
            (Lang::ZhTw, WarningKind::NoSuchTime(token)) => format!("{token}：沒有這個時間"),
            (Lang::En, WarningKind::EndBeforeStart(token)) => {
                format!("{token}: ends before it starts")
            }
            (Lang::ZhTw, WarningKind::EndBeforeStart(token)) => format!("{token}：結束在開始之前"),
        }
    }

    /// After `d`, the bottom row asks whether to delete,
    /// e.g. ` 刪除「Write the design doc」和底下 2 件子任務？d 或 y 刪除，其他鍵取消`
    pub fn confirm_delete(&self, title: &str, count: usize) -> String {
        let subtasks = count.saturating_sub(1);
        match (self.lang, subtasks) {
            (Lang::En, 0) => format!(" Delete \"{title}\"? d or y deletes, any other key cancels"),
            (Lang::En, 1) => {
                format!(
                    " Delete \"{title}\" and its subtask? d or y deletes, any other key cancels"
                )
            }
            (Lang::En, n) => {
                format!(
                    " Delete \"{title}\" and its {n} subtasks? d or y deletes, any other key cancels"
                )
            }
            (Lang::ZhTw, 0) => format!(" 刪除「{title}」？d 或 y 刪除，其他鍵取消"),
            (Lang::ZhTw, n) => {
                format!(" 刪除「{title}」和底下 {n} 件子任務？d 或 y 刪除，其他鍵取消")
            }
        }
    }

    /// After `d` in the calendar list, the bottom row asks whether to delete the calendar,
    /// e.g. ` 刪除日曆「Moving」和裡面的 4 件事？d 或 y 刪除，其他鍵取消`
    pub fn confirm_delete_calendar(&self, name: &str, count: usize) -> String {
        match (self.lang, count) {
            (Lang::En, 0) => {
                format!(" Delete the calendar \"{name}\"? d or y deletes, any other key cancels")
            }
            (Lang::En, 1) => format!(
                " Delete the calendar \"{name}\" and its item? d or y deletes, any other key cancels"
            ),
            (Lang::En, n) => format!(
                " Delete the calendar \"{name}\" and its {n} items? d or y deletes, any other key cancels"
            ),
            (Lang::ZhTw, 0) => format!(" 刪除日曆「{name}」？d 或 y 刪除，其他鍵取消"),
            (Lang::ZhTw, n) => {
                format!(" 刪除日曆「{name}」和裡面的 {n} 件事？d 或 y 刪除，其他鍵取消")
            }
        }
    }

    /// While rescheduling (after `r`), the hint on the bottom row,
    /// e.g. ` 改期「Send the weekly update」：移到新的日期  Enter 確定  Esc 取消`
    pub fn moving(&self, title: &str) -> String {
        match self.lang {
            Lang::En => {
                format!(" Reschedule \"{title}\": move to the new day  Enter done  Esc cancel")
            }
            Lang::ZhTw => format!(" 改期「{title}」：移到新的日期  Enter 確定  Esc 取消"),
        }
    }

    /// Below the description on the settings page, when the chosen setting does nothing on the current view:
    /// which views it is for, e.g. `這個畫面用不到：2 大月曆`
    pub fn setting_elsewhere(&self, views: &[View]) -> String {
        let name = |view: &View| match (self.lang, view) {
            (Lang::En, View::Planner) => "1 calendar",
            (Lang::En, View::Month) => "2 month grid",
            (Lang::En, View::Tasks) => "3 tasks page",
            (Lang::ZhTw, View::Planner) => "1 月曆",
            (Lang::ZhTw, View::Month) => "2 大月曆",
            (Lang::ZhTw, View::Tasks) => "3 任務頁",
        };
        let names: Vec<&str> = views.iter().map(name).collect();
        match self.lang {
            Lang::En => format!("Only in {}", names.join(", ")),
            Lang::ZhTw => format!("這個畫面用不到：{}", names.join("、")),
        }
    }

    /// Where the keyboard is, after the short help page's title: `roh-cal ∙ 按鍵 ∙ 任務頁`
    pub fn help_place(&self, focus: Focus) -> &'static str {
        match (self.lang, focus) {
            (Lang::En, Focus::Calendar) => "calendar",
            (Lang::En, Focus::Agenda) => "agenda",
            (Lang::En, Focus::Day) => "day popup",
            (Lang::En, Focus::Side) => "calendar list",
            (Lang::En, Focus::Tasks) => "tasks page",
            (Lang::ZhTw, Focus::Calendar) => "月曆",
            (Lang::ZhTw, Focus::Agenda) => "事項",
            (Lang::ZhTw, Focus::Day) => "那天的事",
            (Lang::ZhTw, Focus::Side) => "日曆清單",
            (Lang::ZhTw, Focus::Tasks) => "任務頁",
        }
    }

    /// Bottom of the settings page: what the chosen row does
    pub fn setting_description(&self, row: Row) -> &'static str {
        match (self.lang, row) {
            (Lang::En, Row::Theme) => "Theme: l the next, h the previous",
            (Lang::ZhTw, Row::Theme) => "顏色：l 換下一套 theme，h 換上一套",
            (Lang::En, Row::RelativeWeeks) => "Weeks: count from the selected week",
            (Lang::ZhTw, Row::RelativeWeeks) => "週數欄：其他週寫跟選取那週差幾週",
            (Lang::En, Row::GridLines) => "Month grid: lines between the cells",
            (Lang::ZhTw, Row::GridLines) => "大月曆：格子之間畫淡淡的格線",
            (Lang::En, Row::CenterCursor) => "Month grid: centers the selected week",
            (Lang::ZhTw, Row::CenterCursor) => "大月曆：選取的那週固定在正中間",
            (Lang::En, Row::ListWidth) => "Calendar list max width: h -1, l +1",
            (Lang::ZhTw, Row::ListWidth) => "日曆清單最寬幾格：h 窄一格、l 寬一格",
            (Lang::En, Row::HideDone) => "Hide done tasks everywhere",
            (Lang::ZhTw, Row::HideDone) => "做完的任務藏起來（日曆清單也能切）",
            (Lang::En, Row::Color(_)) => "Color: l next, e custom, r reset",
            (Lang::ZhTw, Row::Color(_)) => "顏色：l 換下一個，e 自訂，r 還原",
        }
    }

    /// The selected date (on the right of the agenda's header), e.g. `Sat Oct 17, 2026` / `2026/10/17（六）`
    pub fn date_title(&self, date: Date) -> String {
        // Monday = 0, Sunday = 6
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

/// The month's index in an array: months are 1-12, turned into 0-11 (`month()` is i8; it must become
/// usize to be used as an index)
fn month_index(date: Date) -> usize {
    usize::try_from(date.month() - 1).unwrap_or(0)
}

#[cfg(test)]
mod tests;
