//! 寫任務檔：打勾、刪除、新增、編輯、undo；寫之前一定先重讀，檔案在別處改過就不寫、改成重新載入

use super::{App, DayList};
use crate::files;
use crate::tasks::{self, Item, edit};
use crate::ui;
use jiff::civil::{Date, Time};
use std::io::ErrorKind;
use std::path::{Path, PathBuf};

/// undo 最多記幾步
const UNDO_LEVELS: usize = 50;

/// 下框線暫時換成的提示（寫檔、undo 的結果）；下一個按鍵就消失
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Notice {
    /// 檔案在別處被改過（例：剛在 Obsidian 改了）：沒寫，已重新載入
    Reloaded,
    /// 讀 / 寫任務檔失敗（例：沒有權限、檔案被刪了）：沒寫
    Failed(std::io::ErrorKind),
    /// undo 成功，帶著被復原的項目標題，例：`週會`
    Undone(String),
    /// 沒有可以 undo 的（還沒寫過檔，或已經退到底）
    NothingToUndo,
}

/// 一次寫檔的紀錄（undo 用）：寫之前、寫之後的整個檔案，和改到的項目標題（提示用）
#[derive(Debug)]
pub(super) struct UndoStep {
    before: String,
    after: String,
    title: String,
}

impl App {
    /// 清單選到的項目、任務檔的位置和檔案現在的內容（寫之前一定先重讀）。
    /// 沒有任務檔（test）或那天沒有項目是 `Ok(None)`；讀不到檔案是 `Err(提示)`
    fn selected_and_file(&self, list: DayList) -> Result<Option<(Item, PathBuf, String)>, Notice> {
        let Some(path) = self.tasks_path.clone() else {
            return Ok(None);
        };
        let Some(item) = ui::items_on(&self.items, list.date)
            .get(list.selected)
            .copied()
            .cloned()
        else {
            return Ok(None);
        };
        match std::fs::read_to_string(&path) {
            Ok(text) => Ok(Some((item, path, text))),
            Err(error) => Err(Notice::Failed(error.kind())),
        }
    }

    /// 當天清單選到的項目打勾 / 取消，寫回任務檔；寫成功（或沒事可做）回傳 `None`，沒寫成回傳要顯示的提示
    ///
    /// 那一行在別處被改過（`Changed`）就不寫，改成重新載入
    pub(super) fn toggle_selected(&mut self, list: DayList) -> Option<Notice> {
        let (item, path, text) = match self.selected_and_file(list) {
            Ok(found) => found?,
            Err(notice) => return Some(notice),
        };
        match edit::toggle(&text, &item) {
            Ok(new_text) => self.write_tasks(&path, text, new_text, item.title),
            Err(edit::Changed) => Some(self.reload(&text)),
        }
    }

    /// `d`：先算一次刪掉選到的項目會刪掉幾筆（含子任務），回傳要問的筆數（沒事可做是 `None`）；
    /// 算不出來（檔案在別處改過、讀不到）回傳提示
    pub(super) fn ask_delete(&mut self, list: DayList) -> Result<Option<usize>, Notice> {
        let Some((item, _, text)) = self.selected_and_file(list)? else {
            return Ok(None);
        };
        match edit::delete(&text, &item) {
            Ok((_, count)) => Ok(Some(count)),
            Err(edit::Changed) => Err(self.reload(&text)),
        }
    }

    /// `y`：刪掉選到的項目（連子任務），寫回任務檔；回傳值跟 `toggle_selected` 一樣
    ///
    /// `count` = 剛剛問使用者的筆數；現在算出來不一樣（例：剛好在別處加了子任務）就不刪，改成重新載入
    pub(super) fn delete_selected(&mut self, list: DayList, count: usize) -> Option<Notice> {
        let (item, path, text) = match self.selected_and_file(list) {
            Ok(found) => found?,
            Err(notice) => return Some(notice),
        };
        match edit::delete(&text, &item) {
            Ok((new_text, deleted)) if deleted == count => {
                self.write_tasks(&path, text, new_text, item.title)
            }
            _ => Some(self.reload(&text)),
        }
    }

    /// 新項目 `- [ ] 標題 @日期 時間` 寫進任務檔（`time` 是 `None` 就不寫時間），放在 `near`（清單選到的項目）同一組；
    /// 回傳新項目在檔案的第幾行，沒有任務檔（test）是 `Ok(None)`；讀寫失敗、檔案在別處改過回傳提示
    pub(super) fn add_item(
        &mut self,
        title: String,
        date: Date,
        time: Option<Time>,
        near: Option<&Item>,
    ) -> Result<Option<usize>, Notice> {
        let Some(path) = self.tasks_path.clone() else {
            return Ok(None);
        };
        // 任務檔還不存在（例：預設位置第一次用）：當作空的，存的時候會建起來
        let text = match std::fs::read_to_string(&path) {
            Ok(text) => text,
            Err(error) if error.kind() == ErrorKind::NotFound => String::new(),
            Err(error) => return Err(Notice::Failed(error.kind())),
        };
        match edit::add(&text, &title, date, time, near) {
            Ok((new_text, line)) => {
                if let Some(notice) = self.write_tasks(&path, text, new_text, title) {
                    return Err(notice);
                }
                Ok(Some(line))
            }
            Err(edit::Changed) => Err(self.reload(&text)),
        }
    }

    /// 編輯：`item` 那一行改成新的標題、日期、時間（`None` = 拿掉時間），寫進任務檔；什麼都沒改就不寫（也不記 undo）。
    /// 回傳那個項目在檔案的第幾行（編輯不會換行），沒有任務檔（test）是 `Ok(None)`；讀寫失敗、那一行在別處改過回傳提示
    pub(super) fn update_item(
        &mut self,
        item: &Item,
        title: String,
        date: Date,
        time: Option<Time>,
    ) -> Result<Option<usize>, Notice> {
        let Some(path) = self.tasks_path.clone() else {
            return Ok(None);
        };
        let text = match std::fs::read_to_string(&path) {
            Ok(text) => text,
            Err(error) => return Err(Notice::Failed(error.kind())),
        };
        match edit::update(&text, item, &title, date, time) {
            Ok(new_text) if new_text == text => Ok(Some(item.line)),
            Ok(new_text) => {
                // undo 的提示用原本的標題：復原之後回來的是它
                if let Some(notice) = self.write_tasks(&path, text, new_text, item.title.clone()) {
                    return Err(notice);
                }
                Ok(Some(item.line))
            }
            Err(edit::Changed) => Err(self.reload(&text)),
        }
    }

    /// 把 `after` 寫進任務檔，畫面的項目跟著更新，記一步 undo（`before` = 寫之前的內容、`title` = 改到的項目）。
    /// 寫成功回傳 `None`，失敗回傳提示
    fn write_tasks(
        &mut self,
        path: &Path,
        before: String,
        after: String,
        title: String,
    ) -> Option<Notice> {
        if let Err(error) = files::write_atomically(path, &after) {
            return Some(Notice::Failed(error.kind()));
        }
        self.items = tasks::parse(&after).items;
        self.undo.push(UndoStep {
            before,
            after,
            title,
        });
        // 只留最近的 UNDO_LEVELS 步
        if self.undo.len() > UNDO_LEVELS {
            self.undo.remove(0);
        }
        None
    }

    /// 檔案在別處被改過：照檔案現在的內容 `text` 重新載入；之前記的 undo 都對不上了，清掉
    fn reload(&mut self, text: &str) -> Notice {
        self.items = tasks::parse(text).items;
        self.undo.clear();
        Notice::Reloaded
    }

    /// 復原 roh-cal 最後一次寫檔（`u`），回傳要顯示的提示
    ///
    /// 檔案要跟 roh-cal 最後寫的一模一樣才復原；在別處改過就不動檔案、改成重新載入，別人的改動不會被蓋掉
    pub(super) fn undo(&mut self) -> Notice {
        let Some(path) = self.tasks_path.clone() else {
            return Notice::NothingToUndo;
        };
        let Some(step) = self.undo.pop() else {
            return Notice::NothingToUndo;
        };
        let current = match std::fs::read_to_string(&path) {
            Ok(text) => text,
            Err(error) => {
                // 沒復原成：這一步放回去，之後還能再試
                self.undo.push(step);
                return Notice::Failed(error.kind());
            }
        };
        if current != step.after {
            return self.reload(&current);
        }
        if let Err(error) = files::write_atomically(&path, &step.before) {
            self.undo.push(step);
            return Notice::Failed(error.kind());
        }
        self.items = tasks::parse(&step.before).items;
        Notice::Undone(step.title)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::Overlay;
    use crate::app::tests::{app_with_task_file, app_with_two_items, key};
    use ratatui::crossterm::event::KeyCode;

    #[test]
    fn space_or_x_checks_the_selected_item_off_in_the_file() {
        let text = "# 例行\n- [ ] 早上 @2026-10-10 09:00\n- 下午 @2026-10-10 14:00\n";
        let (mut app, path) = app_with_task_file("toggle", text);
        let read = || std::fs::read_to_string(&path).expect("read");
        // 選第二個（下午，沒寫 checkbox）按 Space：補上打勾的 checkbox，只改那一行
        app.handle_key(key(KeyCode::Char('j')));
        app.handle_key(key(KeyCode::Char(' ')));
        assert_eq!(
            read(),
            "# 例行\n- [ ] 早上 @2026-10-10 09:00\n- [x] 下午 @2026-10-10 14:00\n"
        );
        // 畫面用的項目也跟著更新；清單還開著、還選著同一個、沒有提示
        assert!(app.items[1].done);
        assert_eq!(app.notice, None);
        // x 也可以：再按一次就取消
        app.handle_key(key(KeyCode::Char('x')));
        assert_eq!(
            read(),
            "# 例行\n- [ ] 早上 @2026-10-10 09:00\n- [ ] 下午 @2026-10-10 14:00\n"
        );
        assert!(!app.items[1].done);
        std::fs::remove_dir_all(path.parent().expect("dir")).expect("clean up");
    }

    #[test]
    fn toggle_reloads_instead_of_writing_when_the_file_changed_elsewhere() {
        let (mut app, path) = app_with_task_file("changed", "- [ ] 早上 @2026-10-10 09:00\n");
        // 在別處（例：Obsidian）上面多加了一行
        let edited = "- [ ] 新的 @2026-10-10 08:00\n- [ ] 早上 @2026-10-10 09:00\n";
        std::fs::write(&path, edited).expect("edit");
        app.handle_key(key(KeyCode::Char(' ')));
        // 檔案不動、重新載入（現在有兩個項目）、提示出現；下一個按鍵提示就消失
        assert_eq!(std::fs::read_to_string(&path).expect("read"), edited);
        assert_eq!(app.items.len(), 2);
        assert_eq!(app.notice, Some(Notice::Reloaded));
        app.handle_key(key(KeyCode::Char('j')));
        assert_eq!(app.notice, None);
        std::fs::remove_dir_all(path.parent().expect("dir")).expect("clean up");
    }

    #[test]
    fn toggle_says_why_when_the_file_is_gone() {
        let (mut app, path) = app_with_task_file("gone", "- [ ] 早上 @2026-10-10 09:00\n");
        std::fs::remove_file(&path).expect("remove");
        app.handle_key(key(KeyCode::Char(' ')));
        assert_eq!(
            app.notice,
            Some(Notice::Failed(std::io::ErrorKind::NotFound))
        );
        // 沒有偷偷建一個新檔案
        assert!(!path.exists());
        std::fs::remove_dir_all(path.parent().expect("dir")).expect("clean up");
    }

    #[test]
    fn toggle_without_a_task_file_does_nothing() {
        let mut app = app_with_two_items();
        app.handle_key(key(KeyCode::Enter));
        app.handle_key(key(KeyCode::Char(' ')));
        assert!(!app.items[0].done);
        assert_eq!(app.notice, None);
        app.handle_key(key(KeyCode::Char('u')));
        assert_eq!(app.notice, Some(Notice::NothingToUndo));
    }

    #[test]
    fn u_undoes_the_last_check_and_then_has_nothing_left() {
        let text = "- [ ] 早上 @2026-10-10 09:00\n- 下午 @2026-10-10 14:00\n";
        let (mut app, path) = app_with_task_file("undo", text);
        let read = || std::fs::read_to_string(&path).expect("read");
        app.handle_key(key(KeyCode::Char('j')));
        app.handle_key(key(KeyCode::Char(' ')));
        assert_ne!(read(), text);
        // u：檔案（連沒寫 checkbox 的寫法）和畫面都回到打勾之前，提示被復原的項目
        app.handle_key(key(KeyCode::Char('u')));
        assert_eq!(read(), text);
        assert!(!app.items[1].done);
        assert_eq!(app.notice, Some(Notice::Undone("下午".into())));
        app.handle_key(key(KeyCode::Char('u')));
        assert_eq!(app.notice, Some(Notice::NothingToUndo));
        std::fs::remove_dir_all(path.parent().expect("dir")).expect("clean up");
    }

    #[test]
    fn u_on_the_calendar_steps_back_one_write_at_a_time() {
        let text = "- [ ] 早上 @2026-10-10 09:00\n- [ ] 下午 @2026-10-10 14:00\n";
        let (mut app, path) = app_with_task_file("undo-steps", text);
        let read = || std::fs::read_to_string(&path).expect("read");
        // 兩個都打勾，關掉清單
        for code in [' ', 'j', ' ', 'q'].map(KeyCode::Char) {
            app.handle_key(key(code));
        }
        assert_eq!(
            read(),
            "- [x] 早上 @2026-10-10 09:00\n- [x] 下午 @2026-10-10 14:00\n"
        );
        // 在月曆上按 u：先復原最後一次（下午），再復原早上
        app.handle_key(key(KeyCode::Char('u')));
        assert_eq!(app.overlay, None);
        assert_eq!(
            read(),
            "- [x] 早上 @2026-10-10 09:00\n- [ ] 下午 @2026-10-10 14:00\n"
        );
        app.handle_key(key(KeyCode::Char('u')));
        assert_eq!(read(), text);
        assert_eq!(app.notice, Some(Notice::Undone("早上".into())));
        std::fs::remove_dir_all(path.parent().expect("dir")).expect("clean up");
    }

    #[test]
    fn u_leaves_the_file_alone_when_it_changed_elsewhere() {
        let (mut app, path) = app_with_task_file("undo-changed", "- [ ] 早上 @2026-10-10 09:00\n");
        // 打勾再取消（記了兩步），之後在別處又改了檔案
        app.handle_key(key(KeyCode::Char(' ')));
        app.handle_key(key(KeyCode::Char(' ')));
        let edited = "- [ ] 早上 @2026-10-10 09:00\n- [ ] 新的 @2026-10-10 10:00\n";
        std::fs::write(&path, edited).expect("edit");
        app.handle_key(key(KeyCode::Char('u')));
        // 別人的改動留著，只重新載入；之前記的兩步都對不上了，全部清掉
        assert_eq!(std::fs::read_to_string(&path).expect("read"), edited);
        assert_eq!(app.items.len(), 2);
        assert_eq!(app.notice, Some(Notice::Reloaded));
        app.handle_key(key(KeyCode::Char('u')));
        assert_eq!(app.notice, Some(Notice::NothingToUndo));
        std::fs::remove_dir_all(path.parent().expect("dir")).expect("clean up");
    }

    #[test]
    fn undo_keeps_only_the_last_50_steps() {
        let (mut app, path) = app_with_task_file("undo-limit", "- [ ] 早上 @2026-10-10 09:00\n");
        // 打勾 / 取消 51 次（單數次 → 最後是打勾的）
        for _ in 0..=UNDO_LEVELS {
            app.handle_key(key(KeyCode::Char('x')));
        }
        // 一直按 u：只復原得了 50 次（多按幾次也不會超過）
        let undone = (0..UNDO_LEVELS + 5)
            .filter(|_| {
                app.handle_key(key(KeyCode::Char('u')));
                matches!(app.notice, Some(Notice::Undone(_)))
            })
            .count();
        assert_eq!(undone, UNDO_LEVELS);
        // 最早的那一步掉了：檔案停在第一次打勾之後
        assert_eq!(
            std::fs::read_to_string(&path).expect("read"),
            "- [x] 早上 @2026-10-10 09:00\n"
        );
        std::fs::remove_dir_all(path.parent().expect("dir")).expect("clean up");
    }

    /// 10/10 早上有一個子任務（沒日期，不在清單上）、下午沒有（刪除的 test 用）
    const WITH_SUBTASK: &str =
        "- [ ] 早上 @2026-10-10 09:00\n  - [ ] 準備 slides\n- [ ] 下午 @2026-10-10 14:00\n";

    /// 清單正在問的刪除筆數（test 用）
    fn confirm_delete(app: &App) -> Option<usize> {
        match app.overlay {
            Some(Overlay::DayList(list)) => list.confirm_delete,
            _ => None,
        }
    }

    #[test]
    fn d_then_y_deletes_with_subtasks_and_u_brings_them_back() {
        let (mut app, path) = app_with_task_file("delete", WITH_SUBTASK);
        let read = || std::fs::read_to_string(&path).expect("read");
        // d：先問，檔案還沒動；早上 + 子任務 = 2 筆
        app.handle_key(key(KeyCode::Char('d')));
        assert_eq!(confirm_delete(&app), Some(2));
        assert_eq!(read(), WITH_SUBTASK);
        // y：刪掉早上和它的子任務，只剩下午
        app.handle_key(key(KeyCode::Char('y')));
        assert_eq!(read(), "- [ ] 下午 @2026-10-10 14:00\n");
        assert_eq!(app.items.len(), 1);
        assert_eq!(confirm_delete(&app), None);
        // u：整段救回來
        app.handle_key(key(KeyCode::Char('u')));
        assert_eq!(read(), WITH_SUBTASK);
        assert_eq!(app.notice, Some(Notice::Undone("早上".into())));
        std::fs::remove_dir_all(path.parent().expect("dir")).expect("clean up");
    }

    #[test]
    fn any_key_other_than_y_cancels_the_delete() {
        let (mut app, path) = app_with_task_file("delete-cancel", WITH_SUBTASK);
        for other in [KeyCode::Char('j'), KeyCode::Char('n'), KeyCode::Esc] {
            app.handle_key(key(KeyCode::Char('d')));
            // 取消用的那個鍵只拿來取消：清單還開著、選的還是第一個
            app.handle_key(key(other));
            assert_eq!(confirm_delete(&app), None, "{other:?}");
            assert!(matches!(
                app.overlay,
                Some(Overlay::DayList(DayList { selected: 0, .. }))
            ));
        }
        assert_eq!(std::fs::read_to_string(&path).expect("read"), WITH_SUBTASK);
        std::fs::remove_dir_all(path.parent().expect("dir")).expect("clean up");
    }

    #[test]
    fn y_does_not_delete_when_the_subtasks_changed_meanwhile() {
        let (mut app, path) = app_with_task_file("delete-changed", WITH_SUBTASK);
        app.handle_key(key(KeyCode::Char('d')));
        assert_eq!(confirm_delete(&app), Some(2));
        // 問的時候是 2 筆；按 y 之前，在別處多加了一個子任務 → 會刪 3 筆，跟問的不一樣
        let edited = "- [ ] 早上 @2026-10-10 09:00\n  - [ ] 準備 slides\n  - [ ] 印講義\n- [ ] 下午 @2026-10-10 14:00\n";
        std::fs::write(&path, edited).expect("edit");
        app.handle_key(key(KeyCode::Char('y')));
        assert_eq!(std::fs::read_to_string(&path).expect("read"), edited);
        assert_eq!(app.notice, Some(Notice::Reloaded));
        std::fs::remove_dir_all(path.parent().expect("dir")).expect("clean up");
    }
}
