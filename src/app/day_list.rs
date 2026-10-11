//! 當天清單：哪一天、選到哪個項目，和清單開著時的按鍵（畫面在 `ui::day_list`）

use super::{App, Form, Overlay};
use crate::ui;
use jiff::civil::Date;
use ratatui::crossterm::event::{KeyCode, KeyEvent};

/// 當天清單：哪一天、選到第幾個項目
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DayList {
    /// 清單是哪一天的（打開時選取的那天）
    pub date: Date,
    /// 選到的項目：`ui::items_on(items, date)` 的 index
    pub selected: usize,
    /// 按了 `d`，正在等確認：會刪掉幾筆（含子任務）；下一個鍵是 `y` 才刪，其他鍵都是取消
    pub confirm_delete: Option<usize>,
}

impl DayList {
    /// 打開某一天的清單，選第一個項目
    pub(super) fn open(date: Date) -> Self {
        Self {
            date,
            selected: 0,
            confirm_delete: None,
        }
    }

    /// 往下（`step` = +1）/ 往上（−1）選一個，頭尾相接；`count` = 那天有幾個項目，0 個就不動
    fn select(&mut self, step: isize, count: usize) {
        if let Some(index) = (self.selected as isize + step).checked_rem_euclid(count as isize) {
            self.selected = index as usize;
        }
    }
}

impl App {
    /// 當天清單開著時的按鍵：j / k 選項目、a 新增、Space / x 打勾、d 刪除、u 復原、q / Esc 關閉，月曆不動
    pub(super) fn handle_day_list_key(&mut self, mut list: DayList, key: KeyEvent) {
        if let Some(count) = list.confirm_delete.take() {
            // 正在問「刪除嗎？」：只有 y 會刪，其他鍵都只是取消（按錯不會有事）
            if key.code == KeyCode::Char('y') {
                self.notice = self.delete_selected(list, count);
            }
        } else {
            let count = ui::items_on(&self.items, list.date).len();
            match key.code {
                // q / Esc：關閉清單（q 在這裡不是離開 app）
                KeyCode::Char('q') | KeyCode::Esc => {
                    self.overlay = None;
                    return;
                }
                KeyCode::Char('j') | KeyCode::Down => list.select(1, count),
                KeyCode::Char('k') | KeyCode::Up => list.select(-1, count),
                // Space / x：打勾 / 取消，寫回任務檔
                KeyCode::Char(' ') | KeyCode::Char('x') => {
                    self.notice = self.toggle_selected(list);
                }
                // a：新增表單，存好 / 取消之後回到清單
                KeyCode::Char('a') => {
                    self.overlay = Some(Overlay::Form(Form::open(list)));
                    return;
                }
                // d：先問一次（顯示會刪掉幾筆），按 y 才刪
                KeyCode::Char('d') => match self.ask_delete(list) {
                    Ok(count) => list.confirm_delete = count,
                    Err(notice) => self.notice = Some(notice),
                },
                // u：復原 roh-cal 最後一次寫檔
                KeyCode::Char('u') => self.notice = Some(self.undo()),
                // ?：說明頁，關掉之後回到這個清單
                KeyCode::Char('?') => {
                    self.overlay = Some(Overlay::Help { back: Some(list) });
                    return;
                }
                _ => {}
            }
        }
        // 重新載入後項目可能變少：選的不能超過最後一個
        let count = ui::items_on(&self.items, list.date).len();
        list.selected = list.selected.min(count.saturating_sub(1));
        self.overlay = Some(Overlay::DayList(list));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::View;
    use crate::app::tests::{app_with_two_items, key};
    use jiff::civil::date;
    use ratatui::crossterm::event::{KeyModifiers, MouseEvent, MouseEventKind};

    #[test]
    fn enter_or_o_opens_the_day_list_and_q_or_esc_closes_it() {
        let mut app = app_with_two_items();
        for open in [KeyCode::Enter, KeyCode::Char('o')] {
            for close in [KeyCode::Char('q'), KeyCode::Esc] {
                app.handle_key(key(open));
                assert_eq!(
                    app.overlay,
                    Some(Overlay::DayList(DayList {
                        date: date(2026, 10, 10),
                        selected: 0,
                        confirm_delete: None
                    }))
                );
                // 清單開著時 q 只關清單，不離開 app
                app.handle_key(key(close));
                assert_eq!(app.overlay, None);
                assert!(!app.should_quit);
            }
        }
    }

    #[test]
    fn day_list_j_k_wrap_around_and_the_calendar_stays_put() {
        let mut app = app_with_two_items();
        app.handle_key(key(KeyCode::Enter));
        let selected = |app: &App| match app.overlay {
            Some(Overlay::DayList(list)) => Some(list.selected),
            _ => None,
        };
        app.handle_key(key(KeyCode::Char('j')));
        assert_eq!(selected(&app), Some(1));
        // 兩個項目：再往下回到第一個，往上又到最後一個
        app.handle_key(key(KeyCode::Down));
        assert_eq!(selected(&app), Some(0));
        app.handle_key(key(KeyCode::Char('k')));
        assert_eq!(selected(&app), Some(1));
        // 月曆的鍵、Option+q、滑鼠都不理：日期、畫面、跳窗都不變
        for code in ['h', 'l', 't', 'v', 'œ'].map(KeyCode::Char) {
            app.handle_key(key(code));
        }
        app.handle_mouse(MouseEvent {
            kind: MouseEventKind::ScrollDown,
            column: 0,
            row: 0,
            modifiers: KeyModifiers::NONE,
        });
        assert_eq!(app.selected, date(2026, 10, 10));
        assert_eq!(app.view, View::Compact);
        assert_eq!(selected(&app), Some(1));
    }

    #[test]
    fn day_list_of_an_empty_day_ignores_j_k() {
        let mut app = app_with_two_items();
        app.handle_key(key(KeyCode::Char('l')));
        app.handle_key(key(KeyCode::Char('o')));
        // 10/11 沒有項目：j / k 不會出錯，選的還是 0
        app.handle_key(key(KeyCode::Char('j')));
        app.handle_key(key(KeyCode::Char('k')));
        assert_eq!(
            app.overlay,
            Some(Overlay::DayList(DayList {
                date: date(2026, 10, 11),
                selected: 0,
                confirm_delete: None
            }))
        );
    }
}
