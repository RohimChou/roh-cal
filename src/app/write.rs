//! 寫任務檔：打勾；寫之前一定先重讀，那一行在別處改過就不寫、改成重新載入

use super::App;
use crate::edit;
use crate::files;
use crate::tasks::{self, Item, Kind};
use std::io::ErrorKind;
use std::path::PathBuf;

/// 最下面一列暫時換成的提示（寫檔的結果）；下一個按鍵、點擊就消失
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Notice {
    /// 任務檔在別處被改過（例：剛在 Obsidian 改了）：沒寫，已重新載入
    Reloaded,
    /// 讀 / 寫任務檔失敗（例：沒有權限、檔案被刪了）：沒寫
    Failed(ErrorKind),
    /// 想打勾的是行程（沒有框）：行程不能打勾
    NotATask,
}

impl App {
    /// 鍵盤在右邊時，選到的那件事打勾 / 取消，寫回任務檔；結果（失敗、重新載入）放在 `notice`
    ///
    /// 沒有任務檔（test）、沒選到事就什麼都不做
    pub(super) fn toggle_chosen(&mut self) {
        let Some(item) = self.chosen().map(|entry| entry.item.clone()) else {
            return;
        };
        if item.kind == Kind::Event {
            self.notice = Some(Notice::NotATask);
            return;
        }
        let Some((path, text)) = self.read_tasks() else {
            return;
        };
        match edit::toggle(&text, &item) {
            Ok(after) => self.write_tasks(&path, &after),
            Err(edit::Changed) => self.reload(&text),
        }
    }

    /// 任務檔的位置和現在的內容（寫之前一定先重讀）；沒有任務檔（test）是 `None`，讀不到就記下提示、也是 `None`
    fn read_tasks(&mut self) -> Option<(PathBuf, String)> {
        let path = self.tasks_path.clone()?;
        match std::fs::read_to_string(&path) {
            Ok(text) => Some((path, text)),
            Err(error) => {
                self.notice = Some(Notice::Failed(error.kind()));
                None
            }
        }
    }

    /// 把 `after` 寫進任務檔，畫面的事跟著更新；失敗就記下提示（檔案、畫面都不動）
    fn write_tasks(&mut self, path: &std::path::Path, after: &str) {
        match files::write_atomically(path, after) {
            Ok(()) => self.set_items(tasks::parse(after).items),
            Err(error) => self.notice = Some(Notice::Failed(error.kind())),
        }
    }

    /// 任務檔在別處被改過：照檔案現在的內容 `text` 重新載入，告訴使用者這次沒寫
    fn reload(&mut self, text: &str) {
        self.set_items(tasks::parse(text).items);
        self.notice = Some(Notice::Reloaded);
    }

    /// 換一份新的事（寫檔、重新載入之後）；side menu 的日曆照新的檔案重列，原本有的日曆保留勾選的狀態
    fn set_items(&mut self, items: Vec<Item>) {
        let mut groups = super::Group::list(&items);
        for group in &mut groups {
            if let Some(old) = self.groups.iter().find(|old| old.name == group.name) {
                group.shown = old.shown;
            }
        }
        self.groups = groups;
        self.group_index = self.group_index.min(self.groups.len().saturating_sub(1));
        self.items = items;
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::app::Focus;
    use crate::texts::Lang;
    use jiff::civil::date;
    use ratatui::crossterm::event::{
        KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
    };
    use std::path::Path;

    /// 暫存資料夾裡的任務檔（內容 `text`）和讀它的 app（今天 2026/10/17、鍵盤在右邊、選第一件）；
    /// 回傳 app 和檔案的位置。不會碰到真的任務檔，用完要刪掉 `path` 的資料夾
    pub(crate) fn app_with_task_file(name: &str, text: &str) -> (App, PathBuf) {
        let dir = files::test_dir(name);
        std::fs::create_dir_all(&dir).expect("dir");
        let path = dir.join("tasks.md");
        std::fs::write(&path, text).expect("write");
        let mut app = App::new(Lang::ZhTw, date(2026, 10, 17), tasks::parse(text).items);
        app.tasks_path = Some(path.clone());
        app.focus = Focus::Agenda;
        app.agenda_index = 0;
        (app, path)
    }

    /// 按一個鍵
    pub(crate) fn press(app: &mut App, code: KeyCode) {
        app.handle_key(KeyEvent::from(code));
    }

    /// 刪掉 test 的暫存資料夾
    pub(crate) fn clean_up(path: &Path) {
        std::fs::remove_dir_all(path.parent().expect("dir")).expect("clean up");
    }

    #[test]
    fn space_or_x_checks_the_chosen_item_off_in_the_file() {
        let text = "# 例行\n- [ ] 早上 @2026-10-17 09:00\n- [ ] 下午 @2026-10-17 14:00\n";
        let (mut app, path) = app_with_task_file("toggle", text);
        let read = || std::fs::read_to_string(&path).expect("read");
        // 選第二件（下午）按 Space：只改那一行
        press(&mut app, KeyCode::Char('j'));
        press(&mut app, KeyCode::Char(' '));
        assert_eq!(
            read(),
            "# 例行\n- [ ] 早上 @2026-10-17 09:00\n- [x] 下午 @2026-10-17 14:00\n"
        );
        // 畫面用的事也跟著更新；還選著同一件、沒有提示
        assert!(app.items[1].done);
        assert_eq!(app.agenda_index, 1);
        assert_eq!(app.notice, None);
        // x 也可以：再按一次就取消
        press(&mut app, KeyCode::Char('x'));
        assert_eq!(read(), text);
        assert!(!app.items[1].done);
        clean_up(&path);
    }

    #[test]
    fn an_event_cannot_be_checked_off() {
        let text = "- 看牙醫 @2026-10-17 10:30\n";
        let (mut app, path) = app_with_task_file("event", text);
        press(&mut app, KeyCode::Char(' '));
        assert_eq!(std::fs::read_to_string(&path).expect("read"), text);
        assert_eq!(app.notice, Some(Notice::NotATask));
        // 下一個鍵：提示消失
        press(&mut app, KeyCode::Char('j'));
        assert_eq!(app.notice, None);
        clean_up(&path);
    }

    #[test]
    fn toggle_reloads_instead_of_writing_when_the_file_changed_elsewhere() {
        let (mut app, path) = app_with_task_file("changed", "- [ ] 早上 @2026-10-17 09:00\n");
        // 在別處（例：Obsidian）上面多加了一行
        let edited = "- [ ] 新的 @2026-10-17 08:00\n- [ ] 早上 @2026-10-17 09:00\n";
        std::fs::write(&path, edited).expect("edit");
        press(&mut app, KeyCode::Char(' '));
        // 檔案不動、重新載入（現在有兩件）、提示出現
        assert_eq!(std::fs::read_to_string(&path).expect("read"), edited);
        assert_eq!(app.items.len(), 2);
        assert_eq!(app.notice, Some(Notice::Reloaded));
        clean_up(&path);
    }

    #[test]
    fn toggle_says_why_when_the_file_is_gone() {
        let (mut app, path) = app_with_task_file("gone", "- [ ] 早上 @2026-10-17 09:00\n");
        std::fs::remove_file(&path).expect("remove");
        press(&mut app, KeyCode::Char(' '));
        assert_eq!(app.notice, Some(Notice::Failed(ErrorKind::NotFound)));
        // 沒有偷偷建一個新檔案
        assert!(!path.exists());
        clean_up(&path);
    }

    #[test]
    fn toggle_without_a_task_file_does_nothing() {
        let mut app = App::new(
            Lang::ZhTw,
            date(2026, 10, 17),
            tasks::parse("- [ ] 早上 @2026-10-17\n").items,
        );
        app.focus = Focus::Agenda;
        press(&mut app, KeyCode::Char(' '));
        assert!(!app.items[0].done);
        assert_eq!(app.notice, None);
    }

    #[test]
    fn clicking_the_chosen_item_again_checks_it_off() {
        // 70 × 23、10 月在最上面：右邊第一列（y 3、從 x 36 開始）是 10/10 早上
        let text = "- [ ] 早上 @2026-10-10\n";
        let (mut app, path) = app_with_task_file("click", text);
        app.focus = Focus::Calendar;
        app.screen = ratatui::layout::Rect::new(0, 0, 70, 23);
        app.calendar_top = Some(0);
        let click = MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: 40,
            row: 3,
            modifiers: KeyModifiers::NONE,
        };
        let read = || std::fs::read_to_string(&path).expect("read");
        // 第一下：選它（鍵盤換到右邊），還沒打勾
        app.handle_mouse(click);
        assert_eq!(app.focus, Focus::Agenda);
        assert_eq!(read(), text);
        // 再點一次：打勾
        app.handle_mouse(click);
        assert_eq!(read(), "- [x] 早上 @2026-10-10\n");
        clean_up(&path);
    }

    #[test]
    fn rewriting_keeps_the_calendars_checked_state() {
        // 「生活」沒勾；打勾寫檔之後重列日曆，「生活」還是沒勾
        let text = "- [ ] 早上 @2026-10-17 09:00\n# 生活\n- [ ] 爬山 @2026-10-18\n";
        let (mut app, path) = app_with_task_file("groups", text);
        app.groups[1].shown = false;
        press(&mut app, KeyCode::Char(' '));
        assert!(app.items[0].done);
        assert!(app.groups[0].shown && !app.groups[1].shown);
        clean_up(&path);
    }
}
