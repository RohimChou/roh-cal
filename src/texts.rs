//! On-screen text: one set in English, one in Traditional Chinese; switching language = switching `Texts`

use crate::app::{Notice, Row};
use crate::edit::Problem;
use crate::tasks::{Item, Kind, WarningKind};
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
    /// The key hints on the bottom row (`s` = side menu: hides / shows it when wide enough, otherwise opens
    /// it over the left side)
    pub hints: &'static str,
    /// Calendar header: the week number column (4 cells wide)
    pub header_week: &'static str,
    /// Monday to Sunday in the calendar header: each exactly 2 cells wide
    pub weekdays: [&'static str; 7],
    /// The agenda's header
    pub header_items: &'static str,
    /// The side menu (calendar list) header
    pub header_side: &'static str,
    /// The tasks page header
    pub header_tasks: &'static str,
    /// The key hints on the tasks page's bottom row
    pub hints_tasks: &'static str,
    /// The key hints on the bottom row while layout 3's day popup is open
    pub hints_day: &'static str,
    /// Side menu: items at the top of the file, before any heading
    pub ungrouped: &'static str,
    /// The short version of the help page (`?`): the most used keys
    pub help_short: HelpPage,
    /// The full version of the help page (`Tab`): every key, including the editor, rescheduling and the mouse
    pub help_full: HelpPage,
    /// The `設定` in `roh-cal ∙ 設定` on the settings page's top border
    pub settings_title: &'static str,
    /// The key hints on the settings page's bottom border
    pub settings_hint: &'static str,
    /// After `R` on the settings page: the bottom border switches to this, asking whether to reset everything
    pub settings_reset_all: &'static str,
}

/// One version of the help page: the text on the top and bottom borders, and the sections in the two columns
#[derive(Debug, Clone, Copy)]
pub struct HelpPage {
    /// The `按鍵` in `roh-cal ∙ 按鍵` on the top border (differs between the two versions, so you can tell
    /// which one is showing)
    pub title: &'static str,
    /// The key hints on the bottom border
    pub hint: &'static str,
    /// Sections in the left column
    pub left: &'static [HelpSection],
    /// Sections in the right column
    pub right: &'static [HelpSection],
}

/// One help page section: its name + what each key does
#[derive(Debug, Clone, Copy)]
pub struct HelpSection {
    /// The section name, e.g. `Calendar` / `月曆`
    pub title: &'static str,
    /// (key, what it does), e.g. `("t", "today")`; the key column is the same in every language
    /// (pinned by a test)
    pub keys: &'static [(&'static str, &'static str)],
}

/// English text
const EN: Texts = Texts {
    lang: Lang::En,
    hints: " hjkl move  Tab pane  [] month  t today  s calendars  ? help  , settings  q quit",
    header_week: " Wk ",
    weekdays: ["Mo", "Tu", "We", "Th", "Fr", "Sa", "Su"],
    header_items: " Agenda",
    header_side: " Calendars",
    header_tasks: " Tasks",
    hints_tasks: " jk move  hl fold  Space check  e edit  a add  s calendars  Esc calendar  ? help  , settings  q quit",
    hints_day: " jk move  Space check  e edit  a add  r reschedule  d delete  Esc close",
    ungrouped: "Ungrouped",
    help_short: HelpPage {
        title: "keys",
        hint: "Tab all keys  ?/q/Esc close",
        left: &[
            HelpSection {
                title: "Move",
                keys: &[
                    ("hjkl", "move"),
                    ("[ / ]", "previous / next month"),
                    ("t", "today"),
                    ("Tab", "next pane"),
                ],
            },
            HelpSection {
                title: "Views",
                keys: &[
                    ("1", "calendar + agenda"),
                    ("3", "month grid"),
                    ("4", "tasks page"),
                    ("s", "calendar list"),
                ],
            },
        ],
        right: &[
            HelpSection {
                title: "Edit",
                keys: &[
                    ("Space", "check off"),
                    ("a", "add"),
                    ("e", "edit"),
                    ("r", "reschedule"),
                    ("d", "delete"),
                    ("u", "undo"),
                ],
            },
            HelpSection {
                title: "System",
                keys: &[("?", "help"), (",", "settings"), ("q", "quit")],
            },
        ],
    },
    help_full: HelpPage {
        title: "all keys",
        hint: "Tab fewer keys  ?/q/Esc close",
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
            },
            HelpSection {
                title: "Views",
                keys: &[
                    ("1", "calendar + agenda"),
                    ("3", "month grid"),
                    ("4", "tasks page"),
                    ("Tab / Shift+Tab", "next / previous pane"),
                    ("s", "hide / show calendars"),
                    ("Esc", "back to the calendar"),
                ],
            },
            HelpSection {
                title: "Tasks page",
                keys: &[
                    ("l / h", "expand / collapse"),
                    ("+ / -", "expand / collapse all"),
                    ("L / H", "indent / outdent"),
                    ("J / K", "move down / up"),
                ],
            },
            HelpSection {
                title: "Mouse",
                keys: &[
                    ("wheel", "a week / an item"),
                    ("click", "select; again: check"),
                    ("double-click", "open a day"),
                    ("click outside", "close the box"),
                ],
            },
            HelpSection {
                title: "System",
                keys: &[("?", "help"), (",", "settings"), ("q / Ctrl+C", "quit")],
            },
        ],
        right: &[
            HelpSection {
                title: "Agenda, day popup, tasks page",
                keys: &[
                    ("j / k", "next / previous item"),
                    ("Space / x", "check / uncheck"),
                    ("e / Enter", "edit the line"),
                    ("a / A", "add / add a subtask"),
                    ("r", "reschedule"),
                    ("d", "delete with subtasks"),
                    ("u", "undo"),
                ],
            },
            HelpSection {
                title: "Editor",
                keys: &[("Enter", "save"), ("Esc", "cancel"), ("Ctrl+U", "clear")],
            },
            HelpSection {
                title: "Reschedule",
                keys: &[
                    ("hjkl [ ] t", "pick the new day"),
                    ("Enter", "move it there"),
                    ("Esc", "cancel"),
                ],
            },
            HelpSection {
                title: "Calendar list",
                keys: &[("j / k", "choose"), ("Space / Enter", "check / uncheck")],
            },
            HelpSection {
                title: "Settings",
                keys: &[
                    ("j / k", "choose a row"),
                    ("h / l", "change it"),
                    ("r / R", "reset it / all"),
                ],
            },
        ],
    },
    settings_title: "settings",
    settings_hint: "jk choose  hl change  r reset  R reset all  Esc close",
    settings_reset_all: "Reset everything? y resets, any other key cancels",
};

/// Traditional Chinese text
const ZH_TW: Texts = Texts {
    lang: Lang::ZhTw,
    hints: " hjkl 移動  Tab 切換  [] 換月  t 今天  s 日曆  ? 說明  , 設定  q 離開",
    header_week: " 週 ",
    weekdays: ["一", "二", "三", "四", "五", "六", "日"],
    header_items: " 重要事項",
    header_side: " 日曆",
    header_tasks: " 任務",
    hints_tasks: " jk 移動  hl 收合  Space 打勾  e 編輯  a 新增  s 日曆  Esc 月曆  ? 說明  , 設定  q 離開",
    hints_day: " jk 移動  Space 打勾  e 編輯  a 新增  r 改期  d 刪除  Esc 關閉",
    ungrouped: "未分組",
    help_short: HelpPage {
        title: "按鍵",
        hint: "Tab 全部按鍵  ?/q/Esc 關閉",
        left: &[
            HelpSection {
                title: "移動",
                keys: &[
                    ("hjkl", "移動"),
                    ("[ / ]", "上個月 / 下個月"),
                    ("t", "今天"),
                    ("Tab", "換框"),
                ],
            },
            HelpSection {
                title: "畫面",
                keys: &[
                    ("1", "月曆 + 事項"),
                    ("3", "大月曆"),
                    ("4", "任務頁"),
                    ("s", "日曆清單"),
                ],
            },
        ],
        right: &[
            HelpSection {
                title: "編輯",
                keys: &[
                    ("Space", "打勾"),
                    ("a", "新增"),
                    ("e", "編輯"),
                    ("r", "改期"),
                    ("d", "刪除"),
                    ("u", "復原"),
                ],
            },
            HelpSection {
                title: "系統",
                keys: &[("?", "說明"), (",", "設定"), ("q", "離開")],
            },
        ],
    },
    help_full: HelpPage {
        title: "全部按鍵",
        hint: "Tab 精簡  ?/q/Esc 關閉",
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
            },
            HelpSection {
                title: "畫面",
                keys: &[
                    ("1", "月曆 + 事項"),
                    ("3", "大月曆"),
                    ("4", "任務頁"),
                    ("Tab / Shift+Tab", "換框 / 反方向換框"),
                    ("s", "收起 / 打開日曆清單"),
                    ("Esc", "回到月曆"),
                ],
            },
            HelpSection {
                title: "任務頁",
                keys: &[
                    ("l / h", "展開 / 收起"),
                    ("+ / -", "全部展開 / 全部收起"),
                    ("L / H", "縮排 / 取消縮排"),
                    ("J / K", "往下 / 往上搬"),
                ],
            },
            HelpSection {
                title: "滑鼠",
                keys: &[
                    ("wheel", "移一週 / 一件"),
                    ("click", "選；再點一次：打勾"),
                    ("double-click", "打開那天的事"),
                    ("click outside", "關掉跳窗"),
                ],
            },
            HelpSection {
                title: "系統",
                keys: &[("?", "說明"), (",", "設定"), ("q / Ctrl+C", "離開")],
            },
        ],
        right: &[
            HelpSection {
                title: "事項、跳窗、任務頁",
                keys: &[
                    ("j / k", "下一件 / 上一件"),
                    ("Space / x", "打勾 / 取消"),
                    ("e / Enter", "編輯那一行"),
                    ("a / A", "新增 / 新增子任務"),
                    ("r", "改期"),
                    ("d", "刪除（連子任務）"),
                    ("u", "復原"),
                ],
            },
            HelpSection {
                title: "編輯框",
                keys: &[("Enter", "存檔"), ("Esc", "取消"), ("Ctrl+U", "清掉")],
            },
            HelpSection {
                title: "改期",
                keys: &[
                    ("hjkl [ ] t", "選新的日期"),
                    ("Enter", "搬過去"),
                    ("Esc", "取消"),
                ],
            },
            HelpSection {
                title: "日曆清單",
                keys: &[("j / k", "選日曆"), ("Space / Enter", "勾選 / 取消")],
            },
            HelpSection {
                title: "設定頁",
                keys: &[
                    ("j / k", "選一列"),
                    ("h / l", "改值"),
                    ("r / R", "還原一列 / 全部"),
                ],
            },
        ],
    },
    settings_title: "設定",
    settings_hint: "jk 選  hl 改  r 還原  R 全部還原  Esc 關閉",
    settings_reset_all: "全部還原成預設？y 還原，其他鍵取消",
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
                    " The task file has {count} mistake(s): press 4 to see them on the tasks page"
                )
            }
            (Lang::ZhTw, Notice::Mistakes(count)) => {
                format!(" 任務檔有 {count} 處寫錯：按 4 到任務頁看是哪幾行")
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

    /// The title on the editor's top border, e.g. ` 編輯第 6 行 ` / ` Edit line 6 `
    pub fn edit_title(&self, line: usize) -> String {
        match self.lang {
            Lang::En => format!(" Edit line {line} "),
            Lang::ZhTw => format!(" 編輯第 {line} 行 "),
        }
    }

    /// The title on the editor's top border when adding: ` 新增 ` / ` 新增子任務 `
    pub fn add_title(&self, subtask: bool) -> &'static str {
        match (self.lang, subtask) {
            (Lang::En, false) => " New item ",
            (Lang::En, true) => " New subtask ",
            (Lang::ZhTw, false) => " 新增 ",
            (Lang::ZhTw, true) => " 新增子任務 ",
        }
    }

    /// The editor's bottom border: how to save, how to cancel
    pub fn editor_keys(&self) -> &'static str {
        match self.lang {
            Lang::En => " Enter save  Esc cancel ",
            Lang::ZhTw => " Enter 存檔  Esc 取消 ",
        }
    }

    /// What the line typed in the editor reads as, e.g. `任務 · 2026/10/16（五） 17:30 · 「Send the weekly update」`
    pub fn preview(&self, item: &Item) -> String {
        let kind = match (self.lang, item.kind, item.done) {
            (Lang::En, Kind::Task, false) => "Task",
            (Lang::En, Kind::Task, true) => "Done task",
            (Lang::En, Kind::Event, _) => "Event",
            (Lang::ZhTw, Kind::Task, false) => "任務",
            (Lang::ZhTw, Kind::Task, true) => "做完的任務",
            (Lang::ZhTw, Kind::Event, _) => "行程",
        };
        let when = match item.date {
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
        };
        match self.lang {
            Lang::En => format!(" {kind} · {when} · \"{}\"", item.title),
            Lang::ZhTw => format!(" {kind} · {when} · 「{}」", item.title),
        }
    }

    /// What is wrong with the line typed in the editor (it cannot be saved), e.g. `@2026-13-01：沒有這一天`
    pub fn problem(&self, problem: &Problem) -> String {
        match (self.lang, problem) {
            (Lang::En, Problem::NotAnItem) => {
                " Start with \"- \" (\"- [ ] \" for a task, \"- \" for an event)".into()
            }
            (Lang::ZhTw, Problem::NotAnItem) => {
                " 要用「- 」開頭才是一件事（「- [ ] 」是任務、「- 」是行程）".into()
            }
            (Lang::En, Problem::EmptyTitle) => " The title is empty".into(),
            (Lang::ZhTw, Problem::EmptyTitle) => " 標題是空的".into(),
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
    /// e.g. ` 刪除「Write the design doc」和底下 2 件子任務？y 刪除，其他鍵取消`
    pub fn confirm_delete(&self, title: &str, count: usize) -> String {
        let subtasks = count.saturating_sub(1);
        match (self.lang, subtasks) {
            (Lang::En, 0) => format!(" Delete \"{title}\"? y deletes, any other key cancels"),
            (Lang::En, 1) => {
                format!(" Delete \"{title}\" and its subtask? y deletes, any other key cancels")
            }
            (Lang::En, n) => {
                format!(
                    " Delete \"{title}\" and its {n} subtasks? y deletes, any other key cancels"
                )
            }
            (Lang::ZhTw, 0) => format!(" 刪除「{title}」？y 刪除，其他鍵取消"),
            (Lang::ZhTw, n) => format!(" 刪除「{title}」和底下 {n} 件子任務？y 刪除，其他鍵取消"),
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

    /// Bottom of the settings page: what the chosen row does
    pub fn setting_description(&self, row: Row) -> &'static str {
        match (self.lang, row) {
            (Lang::En, Row::Theme) => "Colors: l the next theme, h the previous one",
            (Lang::ZhTw, Row::Theme) => "顏色：l 換下一套 theme，h 換上一套",
            (Lang::En, Row::RelativeWeeks) => {
                "Week column: the others count from the selected week"
            }
            (Lang::ZhTw, Row::RelativeWeeks) => "週數欄：選取那週寫週數，其他週寫差幾週",
            (Lang::En, Row::GridLines) => "Month grid: faint lines between the cells",
            (Lang::ZhTw, Row::GridLines) => "大月曆：格子之間畫淡淡的格線",
            (Lang::En, Row::ItemIcons) => "Month grid: a dot before each item",
            (Lang::ZhTw, Row::ItemIcons) => "大月曆：每件事前面加一個點",
            (Lang::En, Row::CenterCursor) => "Month grid: the selected week stays in the middle",
            (Lang::ZhTw, Row::CenterCursor) => "大月曆：選取的那週固定在正中間",
            (Lang::En, Row::ListWidth) => "Calendar list: max width (h / l narrower / wider)",
            (Lang::ZhTw, Row::ListWidth) => "日曆清單：最寬幾格（h / l：窄 / 寬一格）",
            (Lang::En, Row::Color(_)) => "Color: l the next in the palette, r the theme's",
            (Lang::ZhTw, Row::Color(_)) => "顏色：l 換色票的下一個，r 回到 theme 的",
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
            " 任務檔有 2 處寫錯：按 4 到任務頁看是哪幾行"
        );
        assert_eq!(
            zh.notice(&Notice::Failed(ErrorKind::PermissionDenied)),
            " 寫不進任務檔（permission denied）"
        );
    }

    #[test]
    fn preview_says_what_a_line_reads_as() {
        let item = |line: &str| crate::tasks::parse(line).items[0].clone();
        let zh = Lang::ZhTw.texts();
        assert_eq!(
            zh.preview(&item("- [ ] Send the weekly update @2026-10-16 17:30")),
            " 任務 · 2026/10/16（五） 17:30 · 「Send the weekly update」"
        );
        assert_eq!(
            zh.preview(&item("- Business trip to NYC @2026-10-20..2026-10-25")),
            " 行程 · 2026/10/20（二）～2026/10/25（日） · 「Business trip to NYC」"
        );
        assert_eq!(
            zh.preview(&item("- [x] Organize the bookshelf")),
            " 做完的任務 · 沒有日期 · 「Organize the bookshelf」"
        );
        let en = Lang::En.texts();
        assert_eq!(
            en.preview(&item("- Dentist appointment @2026-10-14 10:30")),
            " Event · Wed Oct 14, 2026 10:30 · \"Dentist appointment\""
        );
        assert_eq!(
            en.problem(&Problem::Mistake(WarningKind::NoSuchDate(
                "@2026-13-01".into()
            ))),
            " @2026-13-01: no such day"
        );
    }

    #[test]
    fn confirm_delete_counts_the_subtasks() {
        let zh = Lang::ZhTw.texts();
        assert_eq!(
            zh.confirm_delete("Go to the gym", 1),
            " 刪除「Go to the gym」？y 刪除，其他鍵取消"
        );
        assert_eq!(
            zh.confirm_delete("Write the design doc", 3),
            " 刪除「Write the design doc」和底下 2 件子任務？y 刪除，其他鍵取消"
        );
        assert_eq!(
            Lang::En.texts().confirm_delete("a", 2),
            " Delete \"a\" and its subtask? y deletes, any other key cancels"
        );
    }

    #[test]
    fn help_pages_list_the_same_keys_in_both_languages() {
        // The key column is the same in every language (only the descriptions are translated): a key missing
        // from or extra in either language is caught
        let keys = |page: HelpPage| -> Vec<Vec<&str>> {
            page.left
                .iter()
                .chain(page.right)
                .map(|section| section.keys.iter().map(|&(key, _)| key).collect())
                .collect()
        };
        let (en, zh) = (Lang::En.texts(), Lang::ZhTw.texts());
        assert_eq!(keys(en.help_short), keys(zh.help_short));
        assert_eq!(keys(en.help_full), keys(zh.help_full));
        assert_eq!(en.help_full.left.len(), zh.help_full.left.len());
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
