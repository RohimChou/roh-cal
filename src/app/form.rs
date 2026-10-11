//! 新增表單：標題、日期兩欄，和表單開著時的按鍵、按 Enter 的檢查（畫面在 `ui::form`、寫檔在 `write`）

use super::{App, DayList, Overlay};
use crate::ui;
use jiff::{ToSpan, civil::Date};
use ratatui::crossterm::event::{Event, KeyCode, KeyEvent};
use tui_input::{Input, InputRequest, backend::crossterm::to_input_request};

/// 新增表單：標題、日期兩欄
#[derive(Debug, Clone)]
pub struct Form {
    /// 標題欄
    pub title: Input,
    /// 日期欄（`2026-10-12`），一開始是清單那一天
    pub date: Input,
    /// 小月曆選到的日期：日期欄最後一次是完整日期時的那天（打到一半時不變），`h/j/k/l` 從這天開始換
    pub picked: Date,
    /// 游標在哪一欄
    pub focus: FormField,
    /// 存好 / 取消之後回到這個清單；新項目放在這個清單選到的項目同一組
    pub back: DayList,
    /// 按 Enter 時沒通過的檢查（顯示在下框線），下一個按鍵就消失
    pub problem: Option<FormProblem>,
}

impl Form {
    /// 從清單打開：標題空的、日期是清單那一天，游標在標題
    pub(super) fn open(back: DayList) -> Self {
        Self {
            title: Input::default(),
            date: Input::new(back.date.to_string()),
            picked: back.date,
            focus: FormField::Title,
            back,
            problem: None,
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
            && self.picked == other.picked
            && self.focus == other.focus
            && self.back == other.back
            && self.problem == other.problem
    }
}

impl Eq for Form {}

/// 表單的欄位
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FormField {
    Title,
    Date,
}

impl FormField {
    /// `Tab` / `Shift+Tab` 換到的另一欄（只有兩欄，往前往後都一樣）
    fn other(self) -> Self {
        match self {
            Self::Title => Self::Date,
            Self::Date => Self::Title,
        }
    }
}

/// 表單按 Enter 時沒通過的檢查
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FormProblem {
    /// 標題是空的
    EmptyTitle,
    /// 日期不是 `2026-10-12` 這種寫法，或不存在（例：`2026-13-01`）
    BadDate,
}

impl App {
    /// 表單開著時的按鍵：打字打進游標那一欄；`Tab` / `Shift+Tab` 換欄、`Enter` 存、`Esc` 取消（都回到清單）；
    /// 游標在日期欄時 `h/j/k/l`（或方向鍵）換日期
    pub(super) fn handle_form_key(&mut self, key: KeyEvent) {
        // 表單裡有字串，不能複製：先拿出來改，最後再放回去
        let Some(Overlay::Form(mut form)) = self.overlay.take() else {
            return;
        };
        form.problem = None;
        match key.code {
            KeyCode::Esc => {
                self.overlay = Some(Overlay::DayList(form.back));
                return;
            }
            KeyCode::Tab | KeyCode::BackTab => form.focus = form.focus.other(),
            KeyCode::Enter => {
                if let Some(list) = self.save_form(&mut form) {
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
            // 其他鍵交給 tui-input（打字、Backspace、←/→、Ctrl+U…）；日期欄只收數字和 `-`
            _ => {
                let request = to_input_request(&Event::Key(key));
                let input = match form.focus {
                    FormField::Title => &mut form.title,
                    FormField::Date => &mut form.date,
                };
                let typed_letter = matches!(request, Some(InputRequest::InsertChar(c)) if !(c.is_ascii_digit() || c == '-'));
                if let Some(request) = request
                    && !(form.focus == FormField::Date && typed_letter)
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

    /// 表單按 Enter：檢查標題、日期，寫進任務檔。存好了回傳要回去的清單（新項目那天，選到新項目）；
    /// 沒存成回傳 `None`，原因放在 `form.problem`（欄位寫錯）或 `self.notice`（檔案的問題），表單留著讓使用者改
    fn save_form(&mut self, form: &mut Form) -> Option<DayList> {
        let title = form.title.value().trim().to_string();
        if title.is_empty() {
            form.problem = Some(FormProblem::EmptyTitle);
            return None;
        }
        let Ok(date) = form.date.value().parse::<Date>() else {
            form.problem = Some(FormProblem::BadDate);
            return None;
        };
        // 新項目放在清單選到的那個項目同一組
        let near = ui::items_on(&self.items, form.back.date)
            .get(form.back.selected)
            .copied()
            .cloned();
        let line = match self.add_item(title, date, near.as_ref()) {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::Notice;
    use crate::app::tests::{app_with_task_file, app_with_two_items, key};
    use jiff::civil::date;
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
        type_text(&mut app, "xq");
        app.handle_key(key(KeyCode::Backspace));
        app.handle_key(key(KeyCode::Backspace));
        type_text(&mut app, "15");
        assert_eq!(form(&app).date.value(), "2026-10-15");
        // 標題是空的：不存，表單留著、下框線說原因
        app.handle_key(key(KeyCode::Enter));
        assert_eq!(form(&app).problem, Some(FormProblem::EmptyTitle));
        // Shift+Tab 回標題，打字之後原因就消失；Enter 存到 15 號，回到 15 號的清單
        app.handle_key(KeyEvent::new(KeyCode::BackTab, KeyModifiers::SHIFT));
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
}
