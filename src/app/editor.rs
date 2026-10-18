//! 一行編輯框（像 tuxedo）：`e` / `Enter` 直接改右邊選到的那件事的那一行文字，下面即時顯示這行會讀成什麼；
//! `Enter` 存檔（寫錯的地方沒改好就存不了）、`Esc` 取消

use super::{App, Focus};
use crate::edit;
use crate::tasks::Item;
use ratatui::crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};
use tui_input::{Input, backend::crossterm::to_input_request};

/// 編輯框：打的字、在編哪件
#[derive(Debug, Clone)]
pub struct Editor {
    /// 打的字（那一行，不含縮排）和游標
    pub input: Input,
    /// 在編哪件（打開那時候讀到的樣子）
    pub item: Item,
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
        let Some(item) = self.chosen().map(|entry| entry.item.clone()) else {
            return;
        };
        let input = Input::new(item.line_text.trim().to_string());
        self.editor = Some(Editor { input, item });
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

    /// 存檔：那一行換成打的字（縮排照舊），寫回任務檔；右邊選著編過的那件
    ///
    /// 打的字有問題（不是一件事、日期寫錯）就不存，編輯框留著（畫面上有紅字說明）；什麼都沒改就不寫
    fn save_editor(&mut self) {
        if self
            .editor
            .as_ref()
            .is_none_or(|editor| editor.check().is_err())
        {
            return;
        }
        let Some(Editor { input, item }) = self.editor.take() else {
            return;
        };
        let new = input.value().trim();
        if new != item.line_text.trim() {
            let Some((path, text)) = self.read_tasks() else {
                return;
            };
            match edit::replace(&text, &item, new) {
                Ok(after) => self.write_tasks(&path, text, after, item.title.clone()),
                Err(edit::Changed) => {
                    self.reload(&text);
                    self.focus = Focus::Calendar;
                    return;
                }
            }
        }
        self.choose_line(item.line);
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
