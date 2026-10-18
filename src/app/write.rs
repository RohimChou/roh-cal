//! 寫任務檔：打勾、刪除、undo；寫之前一定先重讀，那一行在別處改過就不寫、改成重新載入

use super::{App, Focus};
use crate::edit;
use crate::files;
use crate::tasks::{self, Item, Kind};
use ratatui::crossterm::event::KeyCode;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};

/// undo 最多記幾步
const UNDO_LEVELS: usize = 50;

/// 最下面一列暫時換成的提示（寫檔的結果）；下一個按鍵、點擊就消失
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Notice {
    /// 任務檔在別處被改過（例：剛在 Obsidian 改了）：沒寫，已重新載入
    Reloaded,
    /// 讀 / 寫任務檔失敗（例：沒有權限、檔案被刪了）：沒寫
    Failed(ErrorKind),
    /// 想打勾的是行程（沒有框）：行程不能打勾
    NotATask,
    /// undo 成功，帶著被復原的那件事的標題，例：`週會`
    Undone(String),
    /// 沒有可以 undo 的（還沒寫過檔，或已經退到底）
    NothingToUndo,
}

/// 一次寫檔的紀錄（undo 用）：寫之前、寫之後的整個檔案，和改到的那件事的標題（提示用）
#[derive(Debug)]
pub(super) struct UndoStep {
    before: String,
    after: String,
    title: String,
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
            Ok(after) => self.write_tasks(&path, text, after, item.title),
            Err(edit::Changed) => self.reload(&text),
        }
    }

    /// `d`：先算刪掉選到的那件會刪掉幾件（含子任務），最下面一列問使用者（`y` 才刪）
    pub(super) fn ask_delete(&mut self) {
        let Some(item) = self.chosen().map(|entry| entry.item.clone()) else {
            return;
        };
        let Some((_, text)) = self.read_tasks() else {
            return;
        };
        match edit::delete(&text, &item) {
            Ok((_, count)) => self.confirm_delete = Some((item, count)),
            Err(edit::Changed) => self.reload(&text),
        }
    }

    /// 問「要刪嗎」的時候按的鍵：`y` 刪掉（連子任務），其他鍵都只是取消
    ///
    /// 問的時候算的件數跟現在算的不一樣（例：剛好在別處加了子任務）就不刪，改成重新載入
    pub(super) fn answer_delete(&mut self, code: KeyCode) {
        let Some((item, count)) = self.confirm_delete.take() else {
            return;
        };
        if code != KeyCode::Char('y') {
            return;
        }
        let Some((path, text)) = self.read_tasks() else {
            return;
        };
        match edit::delete(&text, &item) {
            Ok((after, deleted)) if deleted == count => {
                self.write_tasks(&path, text, after, item.title);
                // 右邊選下一件（原本的位置）；刪到沒有了就回到月曆
                match self.agenda().len().checked_sub(1) {
                    Some(last) => self.choose(self.agenda_index.min(last)),
                    None => self.focus = Focus::Calendar,
                }
            }
            _ => {
                self.reload(&text);
                self.focus = Focus::Calendar;
            }
        }
    }

    /// `u`：復原 roh-cal 最後一次寫檔，結果放在 `notice`
    ///
    /// 檔案要跟 roh-cal 最後寫的一模一樣才復原；在別處改過就不動檔案、改成重新載入，別人的改動不會被蓋掉
    pub(super) fn undo(&mut self) {
        let Some(path) = self.tasks_path.clone() else {
            self.notice = Some(Notice::NothingToUndo);
            return;
        };
        let Some(step) = self.undo.pop() else {
            self.notice = Some(Notice::NothingToUndo);
            return;
        };
        let current = match std::fs::read_to_string(&path) {
            Ok(text) => text,
            Err(error) => {
                // 沒復原成：這一步放回去，之後還能再試
                self.undo.push(step);
                self.notice = Some(Notice::Failed(error.kind()));
                return;
            }
        };
        if current != step.after {
            self.reload(&current);
            return;
        }
        if let Err(error) = files::write_atomically(&path, &step.before) {
            self.undo.push(step);
            self.notice = Some(Notice::Failed(error.kind()));
            return;
        }
        self.set_items(tasks::parse(&step.before).items);
        self.notice = Some(Notice::Undone(step.title));
    }

    /// 任務檔的位置和現在的內容（寫之前一定先重讀）；沒有任務檔（test）是 `None`，讀不到就記下提示、也是 `None`
    pub(super) fn read_tasks(&mut self) -> Option<(PathBuf, String)> {
        let path = self.tasks_path.clone()?;
        match std::fs::read_to_string(&path) {
            Ok(text) => Some((path, text)),
            Err(error) => {
                self.notice = Some(Notice::Failed(error.kind()));
                None
            }
        }
    }

    /// 新增用：任務檔的位置和現在的內容；檔案還不存在（例：預設位置第一次用）就當作空的，存的時候會建起來
    pub(super) fn read_tasks_for_adding(&mut self) -> Option<(PathBuf, String)> {
        let path = self.tasks_path.clone()?;
        match std::fs::read_to_string(&path) {
            Ok(text) => Some((path, text)),
            Err(error) if error.kind() == ErrorKind::NotFound => Some((path, String::new())),
            Err(error) => {
                self.notice = Some(Notice::Failed(error.kind()));
                None
            }
        }
    }

    /// 把 `after` 寫進任務檔，畫面的事跟著更新，記一步 undo（`before` = 寫之前的內容、`title` = 改到的那件事）；
    /// 失敗就記下提示（檔案、畫面都不動）
    pub(super) fn write_tasks(
        &mut self,
        path: &Path,
        before: String,
        after: String,
        title: String,
    ) {
        if let Err(error) = files::write_atomically(path, &after) {
            self.notice = Some(Notice::Failed(error.kind()));
            return;
        }
        self.set_items(tasks::parse(&after).items);
        self.undo.push(UndoStep {
            before,
            after,
            title,
        });
        // 只留最近的 UNDO_LEVELS 步
        if self.undo.len() > UNDO_LEVELS {
            self.undo.remove(0);
        }
    }

    /// 任務檔在別處被改過：照檔案現在的內容 `text` 重新載入，告訴使用者這次沒寫；之前記的 undo 都對不上了，清掉
    pub(super) fn reload(&mut self, text: &str) {
        self.set_items(tasks::parse(text).items);
        self.undo.clear();
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
    use crate::texts::Lang;
    use jiff::civil::date;
    use ratatui::crossterm::event::{
        KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
    };

    /// 暫存資料夾裡的任務檔（內容 `text`）和讀它的 app（今天 2026/10/17、鍵盤在右邊、選第一件）；
    /// 回傳 app 和檔案的位置。不會碰到真的任務檔，用完要刪掉 `path` 的資料夾
    pub(crate) fn app_with_task_file(name: &str, text: &str) -> (App, PathBuf) {
        let dir = files::test_dir(name);
        std::fs::create_dir_all(&dir).expect("dir");
        let path = dir.join("tasks.md");
        std::fs::write(&path, text).expect("write");
        let mut app = App::new(Lang::ZhTw, date(2026, 10, 17), tasks::parse(text).items);
        app.tasks_path = Some(path.clone());
        // 跟按 Tab 一樣選第一件：鍵盤換到右邊、月曆選取那件事的日子
        app.choose(0);
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
    fn u_undoes_the_last_writes_one_at_a_time_then_has_nothing_left() {
        let text = "- [ ] 早上 @2026-10-17 09:00\n- [ ] 下午 @2026-10-17 14:00\n";
        let (mut app, path) = app_with_task_file("undo", text);
        let read = || std::fs::read_to_string(&path).expect("read");
        // 兩件都打勾，再回月曆
        for code in [' ', 'j', ' '].map(KeyCode::Char) {
            press(&mut app, code);
        }
        press(&mut app, KeyCode::Esc);
        assert_eq!(
            read(),
            "- [x] 早上 @2026-10-17 09:00\n- [x] 下午 @2026-10-17 14:00\n"
        );
        // 在月曆上按 u：先復原最後一次（下午），再復原早上；畫面跟著回去
        press(&mut app, KeyCode::Char('u'));
        assert_eq!(
            read(),
            "- [x] 早上 @2026-10-17 09:00\n- [ ] 下午 @2026-10-17 14:00\n"
        );
        assert_eq!(app.notice, Some(Notice::Undone("下午".into())));
        press(&mut app, KeyCode::Char('u'));
        assert_eq!(read(), text);
        assert!(!app.items[0].done);
        press(&mut app, KeyCode::Char('u'));
        assert_eq!(app.notice, Some(Notice::NothingToUndo));
        clean_up(&path);
    }

    #[test]
    fn u_leaves_the_file_alone_when_it_changed_elsewhere() {
        let (mut app, path) = app_with_task_file("undo-changed", "- [ ] 早上 @2026-10-17 09:00\n");
        // 打勾再取消（記了兩步），之後在別處又改了檔案
        press(&mut app, KeyCode::Char(' '));
        press(&mut app, KeyCode::Char(' '));
        let edited = "- [ ] 早上 @2026-10-17 09:00\n- [ ] 新的 @2026-10-17 10:00\n";
        std::fs::write(&path, edited).expect("edit");
        press(&mut app, KeyCode::Char('u'));
        // 別人的改動留著，只重新載入；之前記的兩步都對不上了，全部清掉
        assert_eq!(std::fs::read_to_string(&path).expect("read"), edited);
        assert_eq!(app.items.len(), 2);
        assert_eq!(app.notice, Some(Notice::Reloaded));
        press(&mut app, KeyCode::Char('u'));
        assert_eq!(app.notice, Some(Notice::NothingToUndo));
        clean_up(&path);
    }

    #[test]
    fn undo_keeps_only_the_last_50_steps() {
        let (mut app, path) = app_with_task_file("undo-limit", "- [ ] 早上 @2026-10-17 09:00\n");
        // 打勾 / 取消 51 次（單數次 → 最後是打勾的）
        for _ in 0..=UNDO_LEVELS {
            press(&mut app, KeyCode::Char('x'));
        }
        // 一直按 u：只復原得了 50 次（多按幾次也不會超過）
        let undone = (0..UNDO_LEVELS + 5)
            .filter(|_| {
                press(&mut app, KeyCode::Char('u'));
                matches!(app.notice, Some(Notice::Undone(_)))
            })
            .count();
        assert_eq!(undone, UNDO_LEVELS);
        // 最早的那一步掉了：檔案停在第一次打勾之後
        assert_eq!(
            std::fs::read_to_string(&path).expect("read"),
            "- [x] 早上 @2026-10-17 09:00\n"
        );
        clean_up(&path);
    }

    /// 10/17 早上有一個子任務（沒日期，不在右邊）、下午沒有（刪除的 test 用）
    const WITH_SUBTASK: &str =
        "- [ ] 早上 @2026-10-17 09:00\n  - [ ] 準備 slides\n- [ ] 下午 @2026-10-17 14:00\n";

    #[test]
    fn d_then_y_deletes_with_subtasks_and_u_brings_them_back() {
        let (mut app, path) = app_with_task_file("delete", WITH_SUBTASK);
        let read = || std::fs::read_to_string(&path).expect("read");
        // d：先問，檔案還沒動；早上 + 子任務 = 2 件
        press(&mut app, KeyCode::Char('d'));
        assert_eq!(
            app.confirm_delete
                .as_ref()
                .map(|(item, count)| (item.title.as_str(), *count)),
            Some(("早上", 2))
        );
        assert_eq!(read(), WITH_SUBTASK);
        // y：刪掉早上和它的子任務，只剩下午；右邊選著下午
        press(&mut app, KeyCode::Char('y'));
        assert_eq!(read(), "- [ ] 下午 @2026-10-17 14:00\n");
        assert_eq!(app.confirm_delete, None);
        assert_eq!(
            app.chosen().map(|entry| entry.item.title.as_str()),
            Some("下午")
        );
        // u：整段救回來
        press(&mut app, KeyCode::Char('u'));
        assert_eq!(read(), WITH_SUBTASK);
        assert_eq!(app.notice, Some(Notice::Undone("早上".into())));
        clean_up(&path);
    }

    #[test]
    fn any_key_other_than_y_cancels_the_delete() {
        let (mut app, path) = app_with_task_file("delete-cancel", WITH_SUBTASK);
        for other in [KeyCode::Char('j'), KeyCode::Char('n'), KeyCode::Esc] {
            press(&mut app, KeyCode::Char('d'));
            // 取消用的那個鍵只拿來取消：還在右邊、還選著第一件
            press(&mut app, other);
            assert_eq!(app.confirm_delete, None, "{other:?}");
            assert_eq!(
                (app.focus, app.agenda_index),
                (Focus::Agenda, 0),
                "{other:?}"
            );
        }
        assert_eq!(std::fs::read_to_string(&path).expect("read"), WITH_SUBTASK);
        clean_up(&path);
    }

    #[test]
    fn y_does_not_delete_when_the_subtasks_changed_meanwhile() {
        let (mut app, path) = app_with_task_file("delete-changed", WITH_SUBTASK);
        press(&mut app, KeyCode::Char('d'));
        // 問的時候是 2 件；按 y 之前，在別處多加了一個子任務 → 會刪 3 件，跟問的不一樣
        let edited = "- [ ] 早上 @2026-10-17 09:00\n  - [ ] 準備 slides\n  - [ ] 印講義\n- [ ] 下午 @2026-10-17 14:00\n";
        std::fs::write(&path, edited).expect("edit");
        press(&mut app, KeyCode::Char('y'));
        assert_eq!(std::fs::read_to_string(&path).expect("read"), edited);
        assert_eq!(app.notice, Some(Notice::Reloaded));
        clean_up(&path);
    }

    #[test]
    fn deleting_the_last_item_goes_back_to_the_calendar() {
        let (mut app, path) = app_with_task_file("delete-last", "- [ ] 早上 @2026-10-17\n");
        press(&mut app, KeyCode::Char('d'));
        press(&mut app, KeyCode::Char('y'));
        assert_eq!(std::fs::read_to_string(&path).expect("read"), "");
        assert_eq!(app.focus, Focus::Calendar);
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
