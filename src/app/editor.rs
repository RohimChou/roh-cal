//! 一行編輯框（像 tuxedo）：直接打那一行的文字，下面即時顯示這行會讀成什麼；`Enter` 存檔（寫錯的地方沒改好就存不了）、
//! `Esc` 取消
//!
//! - `e` / `Enter`：編輯右邊選到的那件事的那一行
//! - `a`：新增一件（右邊選到的那件那一組；在月曆上是選取那天第一件事的那一組，那天沒事就加在檔案最後），預先填好選取的日期
//! - `A`：新增右邊選到的那件事的子任務

use super::{App, Focus};
use crate::edit::{self, Place};
use crate::tasks::Item;
use ratatui::crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};
use tui_input::{Input, backend::crossterm::to_input_request};

/// 編輯框：打的字、要做什麼
#[derive(Debug, Clone)]
pub struct Editor {
    /// 打的字（那一行，不含縮排）和游標
    pub input: Input,
    /// 編輯哪件，或新增到哪裡
    pub target: Target,
}

/// 編輯框存檔時要做什麼
#[derive(Debug, Clone)]
pub enum Target {
    /// 編輯這件（打開那時候讀到的樣子）
    Edit(Item),
    /// 新增一件，放在這裡
    Add(Place),
}

impl Editor {
    /// 現在打的這一行讀起來是什麼（畫面上即時顯示；有問題就存不了）
    pub fn check(&self) -> Result<Item, edit::Problem> {
        edit::check(self.input.value())
    }
}

impl App {
    /// `e` / `Enter`：打開編輯框，裡面是右邊選到的那件事的那一行（不含縮排），游標在最後面
    pub(super) fn start_editing(&mut self) {
        let Some(item) = self.chosen_item().cloned() else {
            return;
        };
        let input = Input::new(item.line_text.trim().to_string());
        self.editor = Some(Editor {
            input,
            target: Target::Edit(item),
        });
    }

    /// `a`：新增一件，預先填好 `- [ ]  @選取的日期`，游標在標題的位置（任務頁不填日期）
    ///
    /// 鍵盤在右邊、任務頁：放在選到的那件那一組；在月曆：放在選取那天第一件事那一組，那天沒事就放在檔案最後
    pub(super) fn start_adding(&mut self) {
        let near = match self.focus {
            Focus::Agenda => self.chosen().map(|entry| entry.item.clone()),
            Focus::Tasks => self.tasks_chosen().cloned(),
            Focus::Calendar | Focus::Side => self
                .items_on(self.selected)
                .first()
                .map(|item| (*item).clone()),
        };
        let place = near.map_or(Place::End, Place::InGroupOf);
        let prefix = "- [ ] ";
        // 任務頁沒有選取的日期：不填日期
        let input = if self.view == super::View::Tasks {
            Input::new(prefix.to_string())
        } else {
            Input::new(format!("{prefix} @{}", self.selected)).with_cursor(prefix.chars().count())
        };
        self.editor = Some(Editor {
            input,
            target: Target::Add(place),
        });
    }

    /// `A`：新增右邊選到的那件事的子任務（預先填好 `- [ ] `，子任務常常沒有自己的日期）
    pub(super) fn start_adding_subtask(&mut self) {
        let Some(item) = self.chosen_item().cloned() else {
            return;
        };
        self.editor = Some(Editor {
            input: Input::new("- [ ] ".to_string()),
            target: Target::Add(Place::ChildOf(item)),
        });
    }

    /// 編輯框開著時的按鍵：`Enter` 存檔、`Esc` 取消、`Ctrl+C` 離開，其他鍵都是打字（游標移動、刪字見 tui-input）
    pub(super) fn handle_editor_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.should_quit = true;
            }
            KeyCode::Enter => self.save_editor(),
            KeyCode::Esc => self.editor = None,
            _ => {
                if let Some(editor) = &mut self.editor
                    && let Some(request) = to_input_request(&Event::Key(key))
                {
                    editor.input.handle(request);
                }
            }
        }
    }

    /// 存檔：編輯 = 那一行換成打的字（縮排照舊）；新增 = 打的字放到該放的地方。寫回任務檔，右邊選著那件
    ///
    /// 打的字有問題（不是一件事、日期寫錯、沒有標題）就不存，編輯框留著（畫面上有紅字說明）；編輯什麼都沒改就不寫
    fn save_editor(&mut self) {
        if self
            .editor
            .as_ref()
            .is_none_or(|editor| editor.check().is_err())
        {
            return;
        }
        let Some(Editor { input, target }) = self.editor.take() else {
            return;
        };
        let new = input.value().trim();
        match target {
            Target::Edit(item) => {
                if new != item.line_text.trim() {
                    let Some((path, text)) = self.read_tasks() else {
                        return;
                    };
                    match edit::replace(&text, &item, new) {
                        Ok(after) => self.write_tasks(&path, text, after, item.title.clone()),
                        Err(edit::Changed) => {
                            // 在別處改過：重新載入，鍵盤回到這個版型的主框（任務頁還是任務頁）
                            self.reload(&text);
                            self.focus = self.home_focus();
                            return;
                        }
                    }
                }
                self.choose_line(item.line);
            }
            Target::Add(place) => {
                let Some((path, text)) = self.read_tasks_for_adding() else {
                    return;
                };
                let title = edit::check(new).map(|item| item.title).unwrap_or_default();
                match edit::add(&text, new, &place) {
                    Ok((after, line)) => {
                        self.write_tasks(&path, text, after, title);
                        // 右邊選著新的那件（任務頁一定看得到）；它不在右邊（例：沒有日期的子任務）就還是選著原本那件
                        // （它的行號沒變）
                        let shown = self.view == super::View::Tasks
                            || self.agenda().iter().any(|entry| entry.item.line == line);
                        match &place {
                            _ if shown => self.choose_line(line),
                            Place::InGroupOf(item) | Place::ChildOf(item)
                                if self.focus == Focus::Agenda =>
                            {
                                self.choose_line(item.line);
                            }
                            _ => {}
                        }
                    }
                    Err(edit::Changed) => {
                        self.reload(&text);
                        self.focus = self.home_focus();
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::write::Notice;
    use super::super::write::tests::{app_with_task_file, clean_up, press};
    use super::*;
    use jiff::civil::date;

    /// 一個字一個字打進編輯框
    fn type_text(app: &mut App, text: &str) {
        for c in text.chars() {
            press(app, KeyCode::Char(c));
        }
    }

    #[test]
    fn e_opens_the_line_and_enter_saves_what_was_typed() {
        let text = "# 例行\n  - [ ] 交週報 @2026-10-16 17:30\n";
        let (mut app, path) = app_with_task_file("edit", text);
        // e：編輯框裡是那一行（不含縮排），游標在最後
        press(&mut app, KeyCode::Char('e'));
        let editor = app.editor.as_ref().expect("編輯框");
        assert_eq!(editor.input.value(), "- [ ] 交週報 @2026-10-16 17:30");
        // 往前移到時間前面，把 17:30 改成 18:00：Backspace 5 次再打
        for _ in 0..5 {
            press(&mut app, KeyCode::Backspace);
        }
        type_text(&mut app, "18:00 給主管");
        press(&mut app, KeyCode::Enter);
        // 縮排照留；「給主管」跟在時間後面，是標題的一部分
        assert_eq!(
            std::fs::read_to_string(&path).expect("read"),
            "# 例行\n  - [ ] 交週報 @2026-10-16 18:00 給主管\n"
        );
        assert!(app.editor.is_none());
        assert_eq!(
            app.chosen().map(|entry| entry.item.title.as_str()),
            Some("交週報 給主管")
        );
        // u 復原
        press(&mut app, KeyCode::Char('u'));
        assert_eq!(std::fs::read_to_string(&path).expect("read"), text);
        clean_up(&path);
    }

    #[test]
    fn a_in_the_agenda_adds_to_the_chosen_items_group() {
        let text = "# 工作\n- [ ] 寫報告 @2026-10-16\n# 生活\n- 爬山 @2026-10-18\n";
        let (mut app, path) = app_with_task_file("add", text);
        // a：預先填好選取的日期，游標在標題的位置；上框線寫「新增」
        press(&mut app, KeyCode::Char('a'));
        let editor = app.editor.as_ref().expect("編輯框");
        assert_eq!(editor.input.value(), "- [ ]  @2026-10-16");
        assert_eq!(editor.input.cursor(), 6);
        type_text(&mut app, "交週報");
        press(&mut app, KeyCode::Enter);
        // 加在「工作」的最後；右邊選著新的那件
        assert_eq!(
            std::fs::read_to_string(&path).expect("read"),
            "# 工作\n- [ ] 寫報告 @2026-10-16\n- [ ] 交週報 @2026-10-16\n# 生活\n- 爬山 @2026-10-18\n"
        );
        assert_eq!(
            app.chosen().map(|entry| entry.item.title.as_str()),
            Some("交週報")
        );
        // u：整行拿掉，提示寫的是新增的那件
        press(&mut app, KeyCode::Char('u'));
        assert_eq!(std::fs::read_to_string(&path).expect("read"), text);
        assert_eq!(app.notice, Some(Notice::Undone("交週報".into())));
        clean_up(&path);
    }

    #[test]
    fn a_on_the_calendar_adds_on_the_selected_day() {
        // 選取 10/20（那天沒事）：加在檔案最後，鍵盤換到右邊、選著它
        let text = "# 工作\n- [ ] 寫報告 @2026-10-16\n";
        let (mut app, path) = app_with_task_file("add-calendar", text);
        press(&mut app, KeyCode::Esc);
        app.selected = date(2026, 10, 20);
        press(&mut app, KeyCode::Char('a'));
        type_text(&mut app, "看醫生");
        press(&mut app, KeyCode::Enter);
        assert_eq!(
            std::fs::read_to_string(&path).expect("read"),
            "# 工作\n- [ ] 寫報告 @2026-10-16\n- [ ] 看醫生 @2026-10-20\n"
        );
        assert_eq!(app.focus, Focus::Agenda);
        // 選取 10/16（那天有寫報告）：加在寫報告那一組；版型 3 沒有右邊，鍵盤留在月曆
        app.view = crate::app::View::Month;
        app.focus = Focus::Calendar;
        app.selected = date(2026, 10, 16);
        press(&mut app, KeyCode::Char('a'));
        type_text(&mut app, "印出來");
        press(&mut app, KeyCode::Enter);
        assert!(
            std::fs::read_to_string(&path)
                .expect("read")
                .contains("- [ ] 看醫生 @2026-10-20\n- [ ] 印出來 @2026-10-16\n")
        );
        assert_eq!(app.focus, Focus::Calendar);
        clean_up(&path);
    }

    #[test]
    fn shift_a_adds_a_subtask_and_keeps_the_parent_chosen() {
        let text = "- [ ] 寫報告 @2026-10-16\n- [ ] 交週報 @2026-10-16\n";
        let (mut app, path) = app_with_task_file("add-subtask", text);
        press(&mut app, KeyCode::Char('A'));
        assert_eq!(
            app.editor.as_ref().map(|editor| editor.input.value()),
            Some("- [ ] ")
        );
        type_text(&mut app, "找資料");
        press(&mut app, KeyCode::Enter);
        assert_eq!(
            std::fs::read_to_string(&path).expect("read"),
            "- [ ] 寫報告 @2026-10-16\n  - [ ] 找資料\n- [ ] 交週報 @2026-10-16\n"
        );
        // 子任務沒有日期、不在右邊：還是選著寫報告
        assert_eq!(
            app.chosen().map(|entry| entry.item.title.as_str()),
            Some("寫報告")
        );
        clean_up(&path);
    }

    #[test]
    fn adding_creates_a_missing_task_file_and_needs_a_title() {
        let (mut app, path) = app_with_task_file("add-new-file", "");
        std::fs::remove_file(&path).expect("remove");
        app.focus = Focus::Calendar;
        // 沒打標題就 Enter：存不了，編輯框還開著
        press(&mut app, KeyCode::Char('a'));
        press(&mut app, KeyCode::Enter);
        assert_eq!(
            app.editor.as_ref().map(Editor::check),
            Some(Err(edit::Problem::EmptyTitle))
        );
        assert!(!path.exists());
        // 打了標題：檔案建起來
        type_text(&mut app, "第一件事");
        press(&mut app, KeyCode::Enter);
        assert_eq!(
            std::fs::read_to_string(&path).expect("read"),
            "- [ ] 第一件事 @2026-10-17\n"
        );
        clean_up(&path);
    }

    #[test]
    fn a_line_with_a_mistake_is_not_saved() {
        let text = "- [ ] 交週報 @2026-10-16\n";
        let (mut app, path) = app_with_task_file("edit-mistake", text);
        press(&mut app, KeyCode::Enter);
        // 日期改成不存在的 10/32：Enter 不存，編輯框還開著，檢查結果是錯的
        for _ in 0..2 {
            press(&mut app, KeyCode::Backspace);
        }
        type_text(&mut app, "32");
        press(&mut app, KeyCode::Enter);
        let editor = app.editor.as_ref().expect("還開著");
        assert!(editor.check().is_err());
        assert_eq!(std::fs::read_to_string(&path).expect("read"), text);
        // 拿掉開頭的「- 」：不是一件事，也不存
        press(&mut app, KeyCode::Home);
        press(&mut app, KeyCode::Delete);
        press(&mut app, KeyCode::Delete);
        assert_eq!(
            app.editor.as_ref().map(Editor::check),
            Some(Err(edit::Problem::NotAnItem))
        );
        // Esc：不存、關掉
        press(&mut app, KeyCode::Esc);
        assert!(app.editor.is_none());
        assert_eq!(std::fs::read_to_string(&path).expect("read"), text);
        clean_up(&path);
    }

    #[test]
    fn keys_go_to_the_editor_while_it_is_open() {
        // q、u、j、Space 都是打字，不會離開、復原、換選的事、打勾
        let text = "- [ ] 交週報 @2026-10-16\n";
        let (mut app, path) = app_with_task_file("edit-keys", text);
        press(&mut app, KeyCode::Char('e'));
        type_text(&mut app, " qu j");
        assert!(!app.should_quit);
        assert_eq!(
            app.editor.as_ref().map(|editor| editor.input.value()),
            Some("- [ ] 交週報 @2026-10-16 qu j")
        );
        // 沒改就存：不寫、不記 undo
        press(&mut app, KeyCode::Esc);
        press(&mut app, KeyCode::Char('e'));
        press(&mut app, KeyCode::Enter);
        press(&mut app, KeyCode::Char('u'));
        assert_eq!(app.notice, Some(Notice::NothingToUndo));
        clean_up(&path);
    }

    #[test]
    fn taking_the_date_away_leaves_the_agenda() {
        // 日期拿掉：存了之後右邊沒有它了，鍵盤回到月曆
        let text = "- [ ] 交週報 @2026-10-16\n";
        let (mut app, path) = app_with_task_file("edit-undated", text);
        press(&mut app, KeyCode::Char('e'));
        for _ in 0.." @2026-10-16".chars().count() {
            press(&mut app, KeyCode::Backspace);
        }
        press(&mut app, KeyCode::Enter);
        assert_eq!(
            std::fs::read_to_string(&path).expect("read"),
            "- [ ] 交週報\n"
        );
        assert_eq!(app.focus, Focus::Calendar);
        assert_eq!(app.selected, date(2026, 10, 16));
        clean_up(&path);
    }
}
