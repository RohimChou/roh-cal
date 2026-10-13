//! 任務 view：整份任務檔照分組列出來（含沒日期的），子任務可以展開 / 收起（畫面在 `ui::tasks_view`）

use super::App;
use crate::tasks::Item;
use crate::ui;
use ratatui::crossterm::event::{KeyCode, KeyEvent};
use std::collections::BTreeSet;

/// 任務 view 的狀態：選到哪個項目、哪些展開了；切到別的 view 再回來還是一樣
#[derive(Debug, Default)]
pub struct TasksView {
    /// 選到的項目：`ui::tasks_view::item_rows` 的 index（分組標題不算）
    pub selected: usize,
    /// 展開的項目（記行號）
    pub expanded: BTreeSet<usize>,
    /// 按了 `d`，正在等確認：會刪掉幾筆（含子任務）；下一個鍵是 `y` 才刪，其他鍵都是取消
    pub confirm_delete: Option<usize>,
}

impl App {
    /// 任務 view 的按鍵（沒有跳窗時）：j / k 選、l / h 展開 / 收起、Space / x 打勾、d 刪除；
    /// 回傳 `false` = 這個鍵交給月曆的按鍵處理（`u` 復原、`v` / `V` 換 view、`?` 說明、`,` 設定、`q` 離開），
    /// 其他月曆的鍵（移動日期、`Enter` 打開當天清單…）在這裡都不理
    pub(super) fn handle_tasks_key(&mut self, key: KeyEvent) -> bool {
        // 正在問「刪除嗎？」：只有 y 會刪，其他鍵都只是取消（按錯不會有事）
        if let Some(count) = self.tasks_view.confirm_delete.take() {
            if key.code == KeyCode::Char('y') {
                self.notice = self.delete_item(self.tasks_item(), count);
                self.clamp_tasks_selection();
            }
            return true;
        }
        let count = self.tasks_len();
        let view = &mut self.tasks_view;
        match key.code {
            // j / k：往下 / 往上選，頭尾相接
            KeyCode::Char('j') | KeyCode::Down if count > 0 => {
                view.selected = (view.selected + 1) % count;
            }
            KeyCode::Char('k') | KeyCode::Up if count > 0 => {
                view.selected = (view.selected + count - 1) % count;
            }
            // l / →：展開；已經展開就跳到第一個子任務
            KeyCode::Char('l') | KeyCode::Right => self.expand_tasks_item(),
            // h / ←：收起；已經收起（或沒有子任務）就跳到父項目
            KeyCode::Char('h') | KeyCode::Left => self.collapse_tasks_item(),
            // Space / x：打勾 / 取消，寫回任務檔
            KeyCode::Char(' ') | KeyCode::Char('x') => {
                self.notice = self.toggle_item(self.tasks_item());
            }
            // d：先問一次（顯示會刪掉幾筆），按 y 才刪
            KeyCode::Char('d') => match self.ask_delete(self.tasks_item()) {
                Ok(count) => self.tasks_view.confirm_delete = count,
                Err(notice) => self.notice = Some(notice),
            },
            // 交給月曆的按鍵處理
            KeyCode::Char('u' | 'v' | 'V' | '?' | ',' | 'q') | KeyCode::Esc => return false,
            // 其他月曆的鍵不理
            _ => {}
        }
        self.clamp_tasks_selection();
        true
    }

    /// 任務 view 現在有幾個項目（含展開的子任務，不含分組標題）
    fn tasks_len(&self) -> usize {
        ui::tasks_view::item_rows(&self.items, &self.tasks_view.expanded).len()
    }

    /// 任務 view 選到的項目；任務檔沒有項目回傳 `None`
    fn tasks_item(&self) -> Option<Item> {
        ui::tasks_view::item_rows(&self.items, &self.tasks_view.expanded)
            .get(self.tasks_view.selected)
            .map(|row| row.item.clone())
    }

    /// 重新載入、刪除後項目可能變少：選的不能超過最後一個
    fn clamp_tasks_selection(&mut self) {
        let count = self.tasks_len();
        self.tasks_view.selected = self.tasks_view.selected.min(count.saturating_sub(1));
    }

    /// `l`：選到的項目有子任務、還沒展開就展開；已經展開就選它的第一個子任務（就在下一行）
    fn expand_tasks_item(&mut self) {
        let Some(item) = self.tasks_item() else {
            return;
        };
        if ui::day_list::children(&self.items, &item).is_empty() {
            return;
        }
        if !self.tasks_view.expanded.insert(item.line) {
            self.tasks_view.selected += 1;
        }
    }

    /// `h`：選到的項目展開了就收起；沒展開、又是展開出來的子任務，就選它的父項目
    fn collapse_tasks_item(&mut self) {
        let view = &self.tasks_view;
        let rows = ui::tasks_view::item_rows(&self.items, &view.expanded);
        let Some(row) = rows.get(view.selected) else {
            return;
        };
        let line = row.item.line;
        // 父項目：往上第一個縮排比較少的
        let parent = rows[..view.selected]
            .iter()
            .rposition(|other| other.level < row.level);
        if !self.tasks_view.expanded.remove(&line)
            && let Some(index) = parent
        {
            self.tasks_view.selected = index;
        }
    }
}
