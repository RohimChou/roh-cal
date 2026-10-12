//! 新增 / 編輯表單（月曆或當天清單按 `a` 新增，清單按 `Enter` / `o` / `e` 編輯）：標題、日期、時間三欄，
//! 和表單開著時的按鍵、按 Enter 的檢查（畫面在 `ui::form`、寫檔在 `write`）

use super::{App, DayList, Notice, Overlay};
use crate::tasks::{self, Item};
use crate::ui;
use jiff::{
    ToSpan,
    civil::{Date, Time},
};
use ratatui::crossterm::event::{Event, KeyCode, KeyEvent};
use tui_input::{Input, InputRequest, backend::crossterm::to_input_request};

/// 新增 / 編輯表單：標題、日期、時間三欄
#[derive(Debug, Clone)]
pub struct Form {
    /// 標題欄
    pub title: Input,
    /// 日期欄（`2026-10-12`），一開始是打開表單的那一天
    pub date: Input,
    /// 時間欄（`14:00`），可以空著 = 沒有時間；也收 `9:00`、`1400` 這種寫法，存檔時寫成 `09:00`、`14:00`
    pub time: Input,
    /// 小月曆選到的日期：日期欄最後一次是完整日期時的那天（打到一半時不變），`h/j/k/l` 從這天開始換
    pub picked: Date,
    /// 游標在哪一欄
    pub focus: FormField,
    /// 從哪裡打開的：當天清單（存好 / 取消之後回到清單，新項目放在清單選到的項目同一組），`None` = 月曆
    pub back: Option<DayList>,
    /// 在編輯哪個項目（打開時讀到的樣子，存檔時用來確認那一行沒被別人改過）；`None` = 新增
    pub editing: Option<Item>,
    /// 按 Enter 時沒通過的檢查（顯示在下框線），下一個按鍵就消失
    pub problem: Option<FormProblem>,
}

impl Form {
    /// 打開表單：標題、時間空的，日期是 `date`，游標在標題；`back` = 從哪個清單打開的（`None` = 月曆）
    pub(super) fn open(date: Date, back: Option<DayList>) -> Self {
        Self {
            title: Input::default(),
            date: Input::new(date.to_string()),
            time: Input::default(),
            picked: date,
            focus: FormField::Title,
            back,
            editing: None,
            problem: None,
        }
    }

    /// 打開表單編輯 `item`（從清單 `back` 打開）：標題、日期、時間是那個項目的，游標在標題最後面
    pub(super) fn edit(item: &Item, back: DayList) -> Self {
        let date = item.date.unwrap_or(back.date);
        let time = item.time.map(|time| time.strftime("%H:%M").to_string());
        Self {
            title: Input::new(item.title.clone()),
            time: Input::new(time.unwrap_or_default()),
            editing: Some(item.clone()),
            ..Self::open(date, Some(back))
        }
    }

    /// 小月曆選到的日期移動 `days` 天（負數 = 往前），日期欄換成新的日期（打到一半的字會被蓋掉）
    fn move_date(&mut self, days: i64) {
        // 超出 jiff 能表示的範圍（例：9999-12-31 再往後）就不動
        let Ok(date) = self.picked.checked_add(days.days()) else {
            return;
        };
        self.picked = date;
        self.date = Input::new(date.to_string());
    }
}

// tui-input 的 `Input` 沒有實作 `PartialEq`，自己比：字和游標位置一樣就算一樣（test 比對 `overlay` 用）
impl PartialEq for Form {
    fn eq(&self, other: &Self) -> bool {
        let same = |a: &Input, b: &Input| a.value() == b.value() && a.cursor() == b.cursor();
        same(&self.title, &other.title)
            && same(&self.date, &other.date)
            && same(&self.time, &other.time)
            && self.picked == other.picked
            && self.focus == other.focus
            && self.back == other.back
            && self.editing == other.editing
            && self.problem == other.problem
    }
}

impl Eq for Form {}

/// 表單的欄位（照畫面上由上到下的順序）
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FormField {
    Title,
    Date,
    Time,
}

impl FormField {
    /// `Tab` 換到的下一欄（最後一欄再按就回到第一欄）
    fn next(self) -> Self {
        match self {
            Self::Title => Self::Date,
            Self::Date => Self::Time,
            Self::Time => Self::Title,
        }
    }

    /// `Shift+Tab` 換到的上一欄（第一欄再按就到最後一欄）
    fn previous(self) -> Self {
        match self {
            Self::Title => Self::Time,
            Self::Date => Self::Title,
            Self::Time => Self::Date,
        }
    }

    /// 這一欄收不收打的這個字：標題什麼都收，日期欄只收數字和 `-`，時間欄只收數字和 `:`
    fn accepts(self, c: char) -> bool {
        match self {
            Self::Title => true,
            Self::Date => c.is_ascii_digit() || c == '-',
            Self::Time => c.is_ascii_digit() || c == ':',
        }
    }
}

/// 表單按 Enter 時沒通過的檢查
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FormProblem {
    /// 標題是空的
    EmptyTitle,
    /// 標題裡有 `@日期`（例：`延到 @2026-10-20`）：parser 只認第一個 `@日期`，存進去會變成項目的日期
    DateInTitle,
    /// 日期不是 `2026-10-12` 這種寫法，或不存在（例：`2026-13-01`）
    BadDate,
    /// 時間不是 `14:00`、`9:00`、`1400`、`930` 這幾種寫法，或不存在（例：`25:00`）
    BadTime,
}

impl FormProblem {
    /// 這個問題出在哪一欄（按 Enter 沒存成時，游標跳過去）
    fn field(self) -> FormField {
        match self {
            Self::EmptyTitle | Self::DateInTitle => FormField::Title,
            Self::BadDate => FormField::Date,
            Self::BadTime => FormField::Time,
        }
    }
}

impl App {
    /// 表單開著時的按鍵：打字打進游標那一欄；`Tab` / `Shift+Tab` 換欄、`Enter` 存、`Esc` 取消（都回到打開表單的地方）；
    /// 游標在日期欄時 `h/j/k/l`（或方向鍵）換日期；游標在不收字母的欄（日期、時間）時 `q` 也是取消
    pub(super) fn handle_form_key(&mut self, key: KeyEvent) {
        // 表單裡有字串，不能複製：先拿出來改，最後再放回去
        let Some(Overlay::Form(mut form)) = self.overlay.take() else {
            return;
        };
        form.problem = None;
        // 取消：Esc，或在不收字母的欄（日期、時間）按 q，跟其他跳窗一樣；標題欄的 q 是打字
        let cancel = key.code == KeyCode::Esc
            || (key.code == KeyCode::Char('q') && !form.focus.accepts('q'));
        if cancel {
            self.overlay = form.back.map(Overlay::DayList);
            return;
        }
        match key.code {
            KeyCode::Tab => form.focus = form.focus.next(),
            KeyCode::BackTab => form.focus = form.focus.previous(),
            KeyCode::Enter => {
                if let Some(list) = self.save_form(&mut form) {
                    if form.back.is_some() {
                        // 從清單打開的：回到新項目那天的清單，選到新項目
                        self.overlay = Some(Overlay::DayList(list));
                    } else {
                        // 從月曆打開的：回到月曆，選取移到新項目那天（看得到新的點點）
                        self.selected = list.date;
                        self.scroll_to_selected();
                    }
                    return;
                }
                // 欄位沒通過檢查：游標跳到有錯的那一欄
                if let Some(problem) = form.problem {
                    form.focus = problem.field();
                }
                // 編輯時檔案在別處改過（已重新載入）：這個表單對不上了，回到清單，要改再打開一次
                if form.editing.is_some()
                    && self.notice == Some(Notice::Reloaded)
                    && let Some(mut list) = form.back
                {
                    let count = ui::items_on(&self.items, list.date).len();
                    list.selected = list.selected.min(count.saturating_sub(1));
                    self.overlay = Some(Overlay::DayList(list));
                    return;
                }
            }
            // 日期欄：h / l（← / →）前一天 / 後一天、k / j（↑ / ↓）上一週 / 下一週，跟月曆一樣
            KeyCode::Char('h') | KeyCode::Left if form.focus == FormField::Date => {
                form.move_date(-1)
            }
            KeyCode::Char('l') | KeyCode::Right if form.focus == FormField::Date => {
                form.move_date(1)
            }
            KeyCode::Char('k') | KeyCode::Up if form.focus == FormField::Date => form.move_date(-7),
            KeyCode::Char('j') | KeyCode::Down if form.focus == FormField::Date => {
                form.move_date(7)
            }
            // 其他鍵交給 tui-input（打字、Backspace、←/→、Ctrl+U…）；日期、時間欄不收字母（`FormField::accepts`）
            _ => {
                let request = to_input_request(&Event::Key(key));
                let input = match form.focus {
                    FormField::Title => &mut form.title,
                    FormField::Date => &mut form.date,
                    FormField::Time => &mut form.time,
                };
                let refused =
                    matches!(request, Some(InputRequest::InsertChar(c)) if !form.focus.accepts(c));
                if let Some(request) = request
                    && !refused
                {
                    input.handle(request);
                }
                // 日期打完整了：小月曆跟著跳過去
                if form.focus == FormField::Date
                    && let Ok(date) = form.date.value().parse::<Date>()
                {
                    form.picked = date;
                }
            }
        }
        self.overlay = Some(Overlay::Form(form));
    }

    /// 表單按 Enter：檢查標題、日期、時間，寫進任務檔（新增一行，或改編輯的那一行）。存好了回傳那個項目那天的清單（選到它）；
    /// 沒存成回傳 `None`，原因放在 `form.problem`（欄位寫錯）或 `self.notice`（檔案的問題），表單留著讓使用者改
    fn save_form(&mut self, form: &mut Form) -> Option<DayList> {
        let title = form.title.value().trim().to_string();
        if title.is_empty() {
            form.problem = Some(FormProblem::EmptyTitle);
            return None;
        }
        if tasks::has_date(&title) {
            form.problem = Some(FormProblem::DateInTitle);
            return None;
        }
        let Ok(date) = form.date.value().parse::<Date>() else {
            form.problem = Some(FormProblem::BadDate);
            return None;
        };
        // 時間欄空著 = 沒有時間
        let typed = form.time.value().trim();
        let time = if typed.is_empty() {
            None
        } else {
            let Some(time) = typed_time(typed) else {
                form.problem = Some(FormProblem::BadTime);
                return None;
            };
            Some(time)
        };
        let saved = match form.editing.clone() {
            // 編輯：那一行改成新的標題、日期、時間
            Some(item) => self.update_item(&item, title, date, time),
            // 新增：放在清單選到的那個項目同一組；從月曆打開的，跟打開那天的清單再按 a 一樣（選第一個）。
            // 表單開著時月曆不會動，所以 `self.selected` 還是打開表單的那天
            None => {
                let list = form.back.unwrap_or_else(|| DayList::open(self.selected));
                let near = ui::items_on(&self.items, list.date)
                    .get(list.selected)
                    .copied()
                    .cloned();
                self.add_item(title, date, time, near.as_ref())
            }
        };
        let line = match saved {
            Ok(line) => line?,
            Err(notice) => {
                self.notice = Some(notice);
                return None;
            }
        };
        let selected = ui::items_on(&self.items, date)
            .iter()
            .position(|item| item.line == line)
            .unwrap_or(0);
        Some(DayList {
            date,
            selected,
            confirm_delete: None,
        })
    }
}

/// 時間欄打的字 → 時間：`14:00`、`9:00`（冒號前 1～2 位、後 2 位）或 `1400`、`930`（3～4 位數字，最後兩位是分鐘）。
/// 寫法不對或時間不存在（例：`9`、`9:5`、`25:00`）回傳 `None`
fn typed_time(text: &str) -> Option<Time> {
    let (hour, minute) = match text.split_once(':') {
        Some(parts) => parts,
        // 沒有冒號：最後兩位是分鐘，前面是小時（`1400` → `14` + `00`）
        None => text.split_at_checked(text.len().checked_sub(2)?)?,
    };
    let digits = |part: &str| part.bytes().all(|byte| byte.is_ascii_digit());
    if !((1..=2).contains(&hour.len()) && minute.len() == 2 && digits(hour) && digits(minute)) {
        return None;
    }
    // 小時、分鐘超出範圍（例：`25`、`60`）時 jiff 回傳錯誤
    Time::new(hour.parse().ok()?, minute.parse().ok()?, 0, 0).ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::Notice;
    use crate::app::tests::{app_with_task_file, app_with_two_items, key};
    use jiff::civil::{date, time};
    use ratatui::crossterm::event::{KeyModifiers, MouseEvent, MouseEventKind};

    /// 一個一個字打進去（test 用）
    fn type_text(app: &mut App, text: &str) {
        for c in text.chars() {
            app.handle_key(key(KeyCode::Char(c)));
        }
    }

    /// 表單現在的狀態（test 用）
    fn form(app: &App) -> &Form {
        match &app.overlay {
            Some(Overlay::Form(form)) => form,
            other => panic!("not a form: {other:?}"),
        }
    }

    #[test]
    fn a_opens_the_form_and_enter_adds_the_item() {
        let text = "- [ ] 早上 @2026-10-10 09:00\n- [ ] 下午 @2026-10-10 14:00\n";
        let (mut app, path) = app_with_task_file("add", text);
        let read = || std::fs::read_to_string(&path).expect("read");
        // a：表單打開，標題空的、日期是清單那天、游標在標題
        app.handle_key(key(KeyCode::Char('a')));
        assert_eq!(form(&app).title.value(), "");
        assert_eq!(form(&app).date.value(), "2026-10-10");
        assert_eq!(form(&app).focus, FormField::Title);
        // 打字（q、d 這些在表單裡就是字，不是指令）→ Enter：寫進檔案，回到清單、選到新項目（沒時間的排最後）
        type_text(&mut app, "估 q d");
        app.handle_key(key(KeyCode::Enter));
        assert_eq!(read(), format!("{text}- [ ] 估 q d @2026-10-10\n"));
        assert_eq!(
            app.overlay,
            Some(Overlay::DayList(DayList {
                date: date(2026, 10, 10),
                selected: 2,
                confirm_delete: None
            }))
        );
        // u：拿掉
        app.handle_key(key(KeyCode::Char('u')));
        assert_eq!(read(), text);
        assert_eq!(app.notice, Some(Notice::Undone("估 q d".into())));
        std::fs::remove_dir_all(path.parent().expect("dir")).expect("clean up");
    }

    #[test]
    fn tab_switches_fields_and_the_date_field_takes_only_digits() {
        let (mut app, path) = app_with_task_file("add-date", "");
        app.handle_key(key(KeyCode::Char('a')));
        // Tab 到日期欄：字母不收；刪掉兩個字改成 15 號
        app.handle_key(key(KeyCode::Tab));
        assert_eq!(form(&app).focus, FormField::Date);
        type_text(&mut app, "xz");
        app.handle_key(key(KeyCode::Backspace));
        app.handle_key(key(KeyCode::Backspace));
        type_text(&mut app, "15");
        assert_eq!(form(&app).date.value(), "2026-10-15");
        // 標題是空的：不存，表單留著、下框線說原因，游標自己跳回標題
        app.handle_key(key(KeyCode::Enter));
        assert_eq!(form(&app).problem, Some(FormProblem::EmptyTitle));
        assert_eq!(form(&app).focus, FormField::Title);
        // 直接打字，原因就消失；Enter 存到 15 號，回到 15 號的清單
        type_text(&mut app, "繳卡費");
        assert_eq!(form(&app).problem, None);
        app.handle_key(key(KeyCode::Enter));
        assert_eq!(
            std::fs::read_to_string(&path).expect("read"),
            "- [ ] 繳卡費 @2026-10-15\n"
        );
        assert!(matches!(
            app.overlay,
            Some(Overlay::DayList(DayList { date: day, selected: 0, .. })) if day == date(2026, 10, 15)
        ));
        std::fs::remove_dir_all(path.parent().expect("dir")).expect("clean up");
    }

    #[test]
    fn bad_date_keeps_the_form_and_esc_cancels() {
        let (mut app, path) = app_with_task_file("add-cancel", "");
        app.handle_key(key(KeyCode::Char('a')));
        type_text(&mut app, "x");
        // 日期改成 2026-10-1（不完整）→ 不存
        app.handle_key(key(KeyCode::Tab));
        app.handle_key(key(KeyCode::Backspace));
        app.handle_key(key(KeyCode::Enter));
        assert_eq!(form(&app).problem, Some(FormProblem::BadDate));
        // Option+q、滑鼠都不理；Esc：什麼都不存，回到原本的清單
        app.handle_key(key(KeyCode::Char('œ')));
        app.handle_mouse(MouseEvent {
            kind: MouseEventKind::ScrollDown,
            column: 0,
            row: 0,
            modifiers: KeyModifiers::NONE,
        });
        assert!(matches!(app.overlay, Some(Overlay::Form(_))));
        app.handle_key(key(KeyCode::Esc));
        assert!(matches!(app.overlay, Some(Overlay::DayList(_))));
        assert_eq!(std::fs::read_to_string(&path).expect("read"), "");
        std::fs::remove_dir_all(path.parent().expect("dir")).expect("clean up");
    }

    #[test]
    fn hjkl_or_arrows_in_the_date_field_change_the_date() {
        let (mut app, path) = app_with_task_file("add-pick", "");
        app.handle_key(key(KeyCode::Char('a')));
        app.handle_key(key(KeyCode::Tab));
        // 從 10/10 開始：l 後一天、j 下一週、k 上一週、h 前一天；先用字母走一圈，再用方向鍵走一圈
        let steps = [
            (KeyCode::Char('l'), KeyCode::Right, date(2026, 10, 11)),
            (KeyCode::Char('j'), KeyCode::Down, date(2026, 10, 18)),
            (KeyCode::Char('k'), KeyCode::Up, date(2026, 10, 11)),
            (KeyCode::Char('h'), KeyCode::Left, date(2026, 10, 10)),
        ];
        for arrows in [false, true] {
            for (letter, arrow, expected) in steps {
                let code = if arrows { arrow } else { letter };
                app.handle_key(key(code));
                assert_eq!(form(&app).date.value(), expected.to_string(), "{code:?}");
                assert_eq!(form(&app).picked, expected, "{code:?}");
            }
        }
        // 跨月：10/10 + 3 週 = 10/31，再 1 天 = 11/1
        type_text(&mut app, "jjjl");
        assert_eq!(form(&app).date.value(), "2026-11-01");
        // 標題欄的 h / j / k / l 就是打字，日期不變；Enter 存到換好的日期
        app.handle_key(KeyEvent::new(KeyCode::BackTab, KeyModifiers::SHIFT));
        type_text(&mut app, "hjkl");
        assert_eq!(form(&app).title.value(), "hjkl");
        assert_eq!(form(&app).date.value(), "2026-11-01");
        app.handle_key(key(KeyCode::Enter));
        assert_eq!(
            std::fs::read_to_string(&path).expect("read"),
            "- [ ] hjkl @2026-11-01\n"
        );
        std::fs::remove_dir_all(path.parent().expect("dir")).expect("clean up");
    }

    #[test]
    fn typing_a_full_date_moves_the_calendar_and_hjkl_starts_from_the_last_full_one() {
        let mut app = app_with_two_items();
        for code in [KeyCode::Enter, KeyCode::Char('a'), KeyCode::Tab] {
            app.handle_key(key(code));
        }
        // 刪掉兩個字改成 25 號：打到一半（2026-10-2）小月曆不動，打完整了才跳過去
        app.handle_key(key(KeyCode::Backspace));
        app.handle_key(key(KeyCode::Backspace));
        type_text(&mut app, "2");
        assert_eq!(form(&app).picked, date(2026, 10, 10));
        type_text(&mut app, "5");
        assert_eq!(form(&app).picked, date(2026, 10, 25));
        // 又打到一半就按 l：從上一個完整的日期（25 號）往後一天，打到一半的字被蓋掉
        app.handle_key(key(KeyCode::Backspace));
        assert_eq!(form(&app).date.value(), "2026-10-2");
        app.handle_key(key(KeyCode::Char('l')));
        assert_eq!(form(&app).date.value(), "2026-10-26");
        assert_eq!(form(&app).picked, date(2026, 10, 26));
    }

    #[test]
    fn a_on_the_calendar_adds_to_the_selected_day_and_goes_back_to_the_calendar() {
        let text = "# 工作\n- [ ] 早上 @2026-10-10 09:00\n# 個人\n- [ ] 散步 @2026-10-11\n";
        let (mut app, path) = app_with_task_file("add-calendar", text);
        let read = || std::fs::read_to_string(&path).expect("read");
        // 先關掉清單，回到月曆（選取在 10/10）；a：表單直接打開，日期是 10/10
        app.handle_key(key(KeyCode::Char('q')));
        app.handle_key(key(KeyCode::Char('a')));
        assert_eq!(form(&app).back, None);
        assert_eq!(form(&app).date.value(), "2026-10-10");
        // 存好：放在那天第一個項目（早上）那一組，回到月曆
        type_text(&mut app, "估價");
        app.handle_key(key(KeyCode::Enter));
        assert_eq!(
            read(),
            "# 工作\n- [ ] 早上 @2026-10-10 09:00\n- [ ] 估價 @2026-10-10\n# 個人\n- [ ] 散步 @2026-10-11\n"
        );
        assert_eq!(app.overlay, None);
        // 10/11 按 a，日期換成下一週（10/18）：放在散步那一組，月曆的選取跳到 10/18
        app.handle_key(key(KeyCode::Char('l')));
        app.handle_key(key(KeyCode::Char('a')));
        app.handle_key(key(KeyCode::Tab));
        app.handle_key(key(KeyCode::Char('j')));
        app.handle_key(KeyEvent::new(KeyCode::BackTab, KeyModifiers::SHIFT));
        type_text(&mut app, "跑步");
        app.handle_key(key(KeyCode::Enter));
        assert!(read().ends_with("- [ ] 散步 @2026-10-11\n- [ ] 跑步 @2026-10-18\n"));
        assert_eq!(app.overlay, None);
        assert_eq!(app.selected, date(2026, 10, 18));
        // Esc：什麼都不存，回到月曆，選取不動
        let before = read();
        app.handle_key(key(KeyCode::Char('a')));
        type_text(&mut app, "x");
        app.handle_key(key(KeyCode::Esc));
        assert_eq!(app.overlay, None);
        assert_eq!(read(), before);
        assert_eq!(app.selected, date(2026, 10, 18));
        std::fs::remove_dir_all(path.parent().expect("dir")).expect("clean up");
    }

    #[test]
    fn a_title_with_an_at_date_is_refused() {
        let (mut app, path) = app_with_task_file("add-at-date", "");
        let read = || std::fs::read_to_string(&path).expect("read");
        app.handle_key(key(KeyCode::Char('a')));
        // 標題裡有 @日期：存了會跑到 10/20，所以不存，表單留著、下框線說原因
        type_text(&mut app, "延到 @2026-10-20");
        app.handle_key(key(KeyCode::Enter));
        assert_eq!(form(&app).problem, Some(FormProblem::DateInTitle));
        assert_eq!(read(), "");
        // 拿掉「 @2026-10-20」（12 個字）就存得進去，日期是表單上的 10/10
        for _ in 0..12 {
            app.handle_key(key(KeyCode::Backspace));
        }
        app.handle_key(key(KeyCode::Enter));
        assert_eq!(read(), "- [ ] 延到 @2026-10-10\n");
        std::fs::remove_dir_all(path.parent().expect("dir")).expect("clean up");
    }

    /// 10/10 早上有子任務、下午已經打勾（編輯的 test 用）
    const MORNING_AND_DONE_AFTERNOON: &str =
        "- [ ] 早上 @2026-10-10 09:00\n  - [ ] 子任務\n- [x] 下午 @2026-10-10 14:00\n";

    #[test]
    fn enter_o_or_e_in_the_day_list_opens_the_item_for_editing() {
        let text = MORNING_AND_DONE_AFTERNOON;
        let (mut app, path) = app_with_task_file("edit-open", text);
        // 選第二個（下午，第 3 行）：三個鍵都打開它，標題、日期是它的、游標在標題；Esc 回到清單、還選著它
        app.handle_key(key(KeyCode::Char('j')));
        for open in [KeyCode::Enter, KeyCode::Char('o'), KeyCode::Char('e')] {
            app.handle_key(key(open));
            assert_eq!(form(&app).title.value(), "下午", "{open:?}");
            assert_eq!(form(&app).date.value(), "2026-10-10");
            assert_eq!(form(&app).focus, FormField::Title);
            let line = form(&app).editing.as_ref().map(|item| item.line);
            assert_eq!(line, Some(3));
            app.handle_key(key(KeyCode::Esc));
            assert!(
                matches!(
                    app.overlay,
                    Some(Overlay::DayList(DayList { selected: 1, .. }))
                ),
                "{open:?}"
            );
        }
        // 沒有項目的那天（10/11）：e 不會打開表單
        for code in [
            KeyCode::Char('q'),
            KeyCode::Char('l'),
            KeyCode::Enter,
            KeyCode::Char('e'),
        ] {
            app.handle_key(key(code));
        }
        assert!(matches!(app.overlay, Some(Overlay::DayList(_))));
        assert_eq!(std::fs::read_to_string(&path).expect("read"), text);
        std::fs::remove_dir_all(path.parent().expect("dir")).expect("clean up");
    }

    #[test]
    fn editing_rewrites_the_line_and_u_undoes_it() {
        let text = MORNING_AND_DONE_AFTERNOON;
        let (mut app, path) = app_with_task_file("edit-save", text);
        let read = || std::fs::read_to_string(&path).expect("read");
        app.handle_key(key(KeyCode::Char('j')));
        app.handle_key(key(KeyCode::Char('e')));
        // 標題改成「晚上」、日期往後一天：打勾狀態和時間照舊、子任務不動；回到 10/11 的清單、選到它
        app.handle_key(key(KeyCode::Backspace));
        app.handle_key(key(KeyCode::Backspace));
        type_text(&mut app, "晚上");
        app.handle_key(key(KeyCode::Tab));
        app.handle_key(key(KeyCode::Char('l')));
        app.handle_key(key(KeyCode::Enter));
        assert_eq!(
            read(),
            "- [ ] 早上 @2026-10-10 09:00\n  - [ ] 子任務\n- [x] 晚上 @2026-10-11 14:00\n"
        );
        assert_eq!(
            app.overlay,
            Some(Overlay::DayList(DayList {
                date: date(2026, 10, 11),
                selected: 0,
                confirm_delete: None
            }))
        );
        // u：整行回到原本的樣子，提示原本的標題
        app.handle_key(key(KeyCode::Char('u')));
        assert_eq!(read(), text);
        assert_eq!(app.notice, Some(Notice::Undone("下午".into())));
        std::fs::remove_dir_all(path.parent().expect("dir")).expect("clean up");
    }

    #[test]
    fn saving_an_unchanged_item_writes_nothing() {
        let text = MORNING_AND_DONE_AFTERNOON;
        let (mut app, path) = app_with_task_file("edit-same", text);
        app.handle_key(key(KeyCode::Char('e')));
        app.handle_key(key(KeyCode::Enter));
        // 檔案不動、沒記 undo；回到清單、還選著它
        assert_eq!(std::fs::read_to_string(&path).expect("read"), text);
        assert!(matches!(
            app.overlay,
            Some(Overlay::DayList(DayList { selected: 0, .. }))
        ));
        app.handle_key(key(KeyCode::Char('u')));
        assert_eq!(app.notice, Some(Notice::NothingToUndo));
        std::fs::remove_dir_all(path.parent().expect("dir")).expect("clean up");
    }

    #[test]
    fn editing_after_the_file_changed_elsewhere_goes_back_to_the_list() {
        let (mut app, path) = app_with_task_file("edit-changed", MORNING_AND_DONE_AFTERNOON);
        app.handle_key(key(KeyCode::Char('e')));
        type_text(&mut app, "（改）");
        // 表單開著的時候，在別處（例：Obsidian）改了這一行
        let edited = MORNING_AND_DONE_AFTERNOON.replace("早上", "早上九點");
        std::fs::write(&path, &edited).expect("edit");
        app.handle_key(key(KeyCode::Enter));
        // 不寫、重新載入、回到清單、提示；別人的改動留著
        assert_eq!(std::fs::read_to_string(&path).expect("read"), edited);
        assert_eq!(app.notice, Some(Notice::Reloaded));
        assert!(matches!(app.overlay, Some(Overlay::DayList(_))));
        assert_eq!(app.items[0].title, "早上九點");
        std::fs::remove_dir_all(path.parent().expect("dir")).expect("clean up");
    }

    #[test]
    fn a_problem_moves_the_cursor_to_that_field() {
        let mut app = app_with_two_items();
        for code in [KeyCode::Enter, KeyCode::Char('a'), KeyCode::Tab] {
            app.handle_key(key(code));
        }
        // 游標在日期、標題是空的 → 跳到標題
        app.handle_key(key(KeyCode::Enter));
        assert_eq!(form(&app).problem, Some(FormProblem::EmptyTitle));
        assert_eq!(form(&app).focus, FormField::Title);
        // 標題打好、日期刪掉一個字（2026-10-1），游標在標題 → 跳到日期（小月曆也跟著出現）
        type_text(&mut app, "x");
        app.handle_key(key(KeyCode::Tab));
        app.handle_key(key(KeyCode::Backspace));
        app.handle_key(KeyEvent::new(KeyCode::BackTab, KeyModifiers::SHIFT));
        app.handle_key(key(KeyCode::Enter));
        assert_eq!(form(&app).problem, Some(FormProblem::BadDate));
        assert_eq!(form(&app).focus, FormField::Date);
        // 游標在日期、標題裡有 @日期 → 跳到標題（標題的問題先檢查）
        app.handle_key(KeyEvent::new(KeyCode::BackTab, KeyModifiers::SHIFT));
        type_text(&mut app, " @2026-10-20");
        app.handle_key(key(KeyCode::Tab));
        app.handle_key(key(KeyCode::Enter));
        assert_eq!(form(&app).problem, Some(FormProblem::DateInTitle));
        assert_eq!(form(&app).focus, FormField::Title);
    }

    #[test]
    fn tab_and_shift_tab_go_around_the_three_fields() {
        let mut app = app_with_two_items();
        app.handle_key(key(KeyCode::Char('a')));
        let shift_tab = KeyEvent::new(KeyCode::BackTab, KeyModifiers::SHIFT);
        // Tab：標題 → 日期 → 時間 → 回到標題；Shift+Tab 反過來
        for (event, expected) in [
            (key(KeyCode::Tab), FormField::Date),
            (key(KeyCode::Tab), FormField::Time),
            (key(KeyCode::Tab), FormField::Title),
            (shift_tab, FormField::Time),
            (shift_tab, FormField::Date),
            (shift_tab, FormField::Title),
        ] {
            app.handle_key(event);
            assert_eq!(form(&app).focus, expected);
        }
    }

    #[test]
    fn typed_time_takes_a_few_ways_of_writing() {
        let cases = [
            ("14:00", Some(time(14, 0, 0, 0))),
            ("9:00", Some(time(9, 0, 0, 0))),
            ("09:05", Some(time(9, 5, 0, 0))),
            ("1400", Some(time(14, 0, 0, 0))),
            ("930", Some(time(9, 30, 0, 0))),
            ("0000", Some(time(0, 0, 0, 0))),
            ("23:59", Some(time(23, 59, 0, 0))),
            // 只有小時、分鐘只有 1 位、位數不對、多的冒號、不存在的時間：都不收
            ("9", None),
            ("14", None),
            ("9:5", None),
            ("12345", None),
            ("123:45", None),
            (":30", None),
            ("1:2:3", None),
            ("24:00", None),
            ("9:60", None),
            ("2500", None),
            ("", None),
        ];
        for (text, expected) in cases {
            assert_eq!(typed_time(text), expected, "{text}");
        }
    }

    #[test]
    fn the_time_field_takes_digits_and_colons_and_saves_the_time() {
        let (mut app, path) = app_with_task_file("add-time", "");
        let read = || std::fs::read_to_string(&path).expect("read");
        app.handle_key(key(KeyCode::Char('a')));
        type_text(&mut app, "早餐");
        // Shift+Tab 從標題直接到時間欄：字母、`-` 不收
        app.handle_key(KeyEvent::new(KeyCode::BackTab, KeyModifiers::SHIFT));
        type_text(&mut app, "25z-:00");
        assert_eq!(form(&app).time.value(), "25:00");
        // 25 點不存在：不存；從標題按 Enter，游標也跳回時間欄
        app.handle_key(key(KeyCode::Tab));
        app.handle_key(key(KeyCode::Enter));
        assert_eq!(form(&app).problem, Some(FormProblem::BadTime));
        assert_eq!(form(&app).focus, FormField::Time);
        assert_eq!(read(), "");
        // 改成 930：存成兩位數的 09:30
        for _ in 0..5 {
            app.handle_key(key(KeyCode::Backspace));
        }
        type_text(&mut app, "930");
        app.handle_key(key(KeyCode::Enter));
        assert_eq!(read(), "- [ ] 早餐 @2026-10-10 09:30\n");
        std::fs::remove_dir_all(path.parent().expect("dir")).expect("clean up");
    }

    #[test]
    fn editing_changes_or_clears_the_time() {
        let text = MORNING_AND_DONE_AFTERNOON;
        let (mut app, path) = app_with_task_file("edit-time", text);
        let read = || std::fs::read_to_string(&path).expect("read");
        let shift_tab = KeyEvent::new(KeyCode::BackTab, KeyModifiers::SHIFT);
        // 打開「早上」：時間欄是它的 09:00；改成 1530
        app.handle_key(key(KeyCode::Char('e')));
        assert_eq!(form(&app).time.value(), "09:00");
        app.handle_key(shift_tab);
        for _ in 0..5 {
            app.handle_key(key(KeyCode::Backspace));
        }
        type_text(&mut app, "1530");
        app.handle_key(key(KeyCode::Enter));
        assert_eq!(read(), text.replace("09:00", "15:30"));
        // 清單照時間排：下午（14:00）在前，早上（15:30）在後、還選著它
        assert!(matches!(
            app.overlay,
            Some(Overlay::DayList(DayList { selected: 1, .. }))
        ));
        // 再打開它，把時間清掉：那一行就沒有時間
        app.handle_key(key(KeyCode::Char('e')));
        app.handle_key(shift_tab);
        for _ in 0..5 {
            app.handle_key(key(KeyCode::Backspace));
        }
        app.handle_key(key(KeyCode::Enter));
        assert_eq!(read(), text.replace(" 09:00", ""));
        std::fs::remove_dir_all(path.parent().expect("dir")).expect("clean up");
    }

    #[test]
    fn q_cancels_the_form_outside_the_title() {
        let (mut app, path) = app_with_task_file("form-q", MORNING_AND_DONE_AFTERNOON);
        // 標題欄：q 是打字
        app.handle_key(key(KeyCode::Char('a')));
        type_text(&mut app, "q");
        assert_eq!(form(&app).title.value(), "q");
        // 日期欄、時間欄：q = Esc，什麼都不存，回到清單
        for tabs in [1, 2] {
            if tabs == 2 {
                app.handle_key(key(KeyCode::Char('a')));
            }
            for _ in 0..tabs {
                app.handle_key(key(KeyCode::Tab));
            }
            app.handle_key(key(KeyCode::Char('q')));
            assert!(matches!(app.overlay, Some(Overlay::DayList(_))), "{tabs}");
        }
        // 編輯表單一樣；從月曆打開的，q 回到月曆
        app.handle_key(key(KeyCode::Char('e')));
        app.handle_key(key(KeyCode::Tab));
        app.handle_key(key(KeyCode::Char('q')));
        assert!(matches!(app.overlay, Some(Overlay::DayList(_))));
        app.handle_key(key(KeyCode::Char('q')));
        app.handle_key(key(KeyCode::Char('a')));
        app.handle_key(key(KeyCode::Tab));
        app.handle_key(key(KeyCode::Char('q')));
        assert_eq!(app.overlay, None);
        assert!(!app.should_quit);
        assert_eq!(
            std::fs::read_to_string(&path).expect("read"),
            MORNING_AND_DONE_AFTERNOON
        );
        std::fs::remove_dir_all(path.parent().expect("dir")).expect("clean up");
    }
}
