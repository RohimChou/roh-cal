//! 改期（`r`）：月曆就是選日期的地方。按 `r` 之後鍵盤換到月曆，用平常的鍵（`hjkl`、`[` `]`、`t`、滑鼠）選新的日期，
//! `Enter`（或再點一次同一天）寫回任務檔，`Esc` 取消
//!
//! 像拖曳：選取的日期移了幾天，那件事就往後（往前）搬幾天；跨多天的整段一起搬

use super::{App, Focus, View};
use crate::edit;
use crate::tasks::Item;
use jiff::{ToSpan, civil::Date};
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

/// 改期中：要搬的那件事，和搬完（或取消）要回去的版型、框
#[derive(Debug, Clone)]
pub struct Moving {
    /// 要搬的那件事（按 `r` 那時候讀到的樣子）
    pub item: Item,
    /// 按 `r` 那時候選取的日期：選取的日期移了幾天，那件事就搬幾天
    from: Date,
    /// 按 `r` 之前在哪個版型、哪個框
    back: (View, Focus),
}

impl App {
    /// `r`：開始搬右邊選到的那件事（鍵盤換到月曆，選取的日期還是那件事的日子）
    pub(super) fn start_moving(&mut self) {
        let Some(item) = self.chosen_item().cloned() else {
            return;
        };
        let back = (self.view, self.focus);
        // 任務頁沒有月曆：借版型 1 的月曆來選，從那件事的日子（沒日期就從今天）開始
        if self.view == View::Tasks {
            self.view = View::Planner;
            self.selected = item.date.unwrap_or(self.today);
            self.scroll_to_month();
        }
        self.moving = Some(Moving {
            item,
            from: self.selected,
            back,
        });
        self.focus = Focus::Calendar;
    }

    /// 改期中的按鍵：移動選取的日期跟平常一樣；`Enter` 確定、`Esc` 取消、`Ctrl+C` 離開，其他鍵不理
    pub(super) fn handle_moving_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.should_quit = true;
            }
            KeyCode::Enter => self.finish_moving(),
            KeyCode::Esc => self.cancel_moving(),
            KeyCode::Char('t') => {
                self.selected = self.today;
                self.scroll_to_month();
            }
            KeyCode::Char('[') => self.jump_month(-1),
            KeyCode::Char(']') => self.jump_month(1),
            // 只收移動日期的鍵（月曆上的 `a` 新增之類的不理）
            code @ (KeyCode::Char('h' | 'j' | 'k' | 'l')
            | KeyCode::Left
            | KeyCode::Right
            | KeyCode::Up
            | KeyCode::Down) => self.handle_calendar_key(code),
            _ => {}
        }
    }

    /// 改期中點到月曆上的一天：選那天；再點一次已經選的那天 = 確定
    pub(super) fn click_while_moving(&mut self, day: Date) {
        if day == self.selected {
            self.finish_moving();
        } else {
            self.selected = day;
        }
    }

    /// 確定：選取的日期移了幾天，那件事就搬幾天，寫回任務檔；回到按 `r` 之前的框（右邊選著搬過去的那件）
    fn finish_moving(&mut self) {
        let Some(Moving { item, from, back }) = self.moving.take() else {
            return;
        };
        let to = self.selected;
        // 新的日子：選取的日期移了幾天就搬幾天（沒動就不寫）；還沒排日期的就是選的那天
        let new_first = match item.date {
            Some(first) if to != from => Some(first.saturating_add((to - from).get_days().days())),
            Some(_) => None,
            None => Some(to),
        };
        // 搬成了（或不用搬）：寫檔失敗、在別處改過就是 `false`
        let mut moved = true;
        if let Some(new_first) = new_first {
            let Some((path, text)) = self.read_tasks() else {
                self.go_back(back);
                return;
            };
            moved = match edit::reschedule(&text, &item, new_first) {
                Ok(after) => self.write_tasks(&path, text, after, item.title.clone()),
                Err(edit::Changed) => {
                    self.reload(&text);
                    false
                }
            };
        }
        self.go_back(back);
        // 右邊選著搬過去的那件（只改那一行，行號不變）；沒搬成、找不到就留在月曆
        if back.1 == Focus::Agenda {
            let index = self
                .agenda()
                .iter()
                .position(|entry| entry.item.line == item.line && entry.is_on(to));
            match index {
                Some(index) if moved => self.agenda_index = index,
                _ => self.focus = Focus::Calendar,
            }
        }
    }

    /// 取消：選取的日期回到按 `r` 那時候，回到按 `r` 之前的框
    fn cancel_moving(&mut self) {
        if let Some(Moving { from, back, .. }) = self.moving.take() {
            self.selected = from;
            self.go_back(back);
        }
    }

    /// 回到按 `r` 之前的版型和框
    fn go_back(&mut self, (view, focus): (View, Focus)) {
        self.view = view;
        self.focus = focus;
    }
}

#[cfg(test)]
mod tests {
    use super::super::write::Notice;
    use super::super::write::tests::{app_with_task_file, clean_up, press};
    use super::*;
    use jiff::civil::date;
    use ratatui::crossterm::event::{MouseButton, MouseEvent, MouseEventKind};

    #[test]
    fn r_then_hjkl_and_enter_moves_the_chosen_item() {
        let text = "- [ ] 交週報 @2026-10-16 17:30\n- [ ] 早餐會 @2026-10-17 08:00\n";
        let (mut app, path) = app_with_task_file("move", text);
        // 選交週報、按 r：鍵盤換到月曆，提示寫著在搬哪件
        press(&mut app, KeyCode::Char('r'));
        assert_eq!(app.focus, Focus::Calendar);
        assert_eq!(
            app.moving.as_ref().map(|m| m.item.title.as_str()),
            Some("交週報")
        );
        // 往後一週再往後一天（10/24），Enter：只改日期，時間照留；回到右邊、選著搬過去的那件
        press(&mut app, KeyCode::Char('j'));
        press(&mut app, KeyCode::Char('l'));
        press(&mut app, KeyCode::Enter);
        assert_eq!(
            std::fs::read_to_string(&path).expect("read"),
            "- [ ] 交週報 @2026-10-24 17:30\n- [ ] 早餐會 @2026-10-17 08:00\n"
        );
        assert!(app.moving.is_none());
        assert_eq!(app.focus, Focus::Agenda);
        assert_eq!(
            app.chosen()
                .map(|entry| (entry.item.title.as_str(), entry.date)),
            Some(("交週報", date(2026, 10, 24)))
        );
        // u 復原
        press(&mut app, KeyCode::Char('u'));
        assert_eq!(std::fs::read_to_string(&path).expect("read"), text);
        clean_up(&path);
    }

    #[test]
    fn esc_cancels_the_move() {
        let text = "- [ ] 交週報 @2026-10-16 17:30\n";
        let (mut app, path) = app_with_task_file("move-cancel", text);
        press(&mut app, KeyCode::Char('r'));
        press(&mut app, KeyCode::Char(']'));
        assert_eq!(app.selected, date(2026, 11, 1));
        // Esc：檔案不動，選取的日期回到 10/16，回到右邊
        press(&mut app, KeyCode::Esc);
        assert_eq!(std::fs::read_to_string(&path).expect("read"), text);
        assert_eq!(app.selected, date(2026, 10, 16));
        assert!(app.moving.is_none());
        assert_eq!(app.focus, Focus::Agenda);
        // 沒改日期就 Enter：不寫、不記 undo
        press(&mut app, KeyCode::Char('r'));
        press(&mut app, KeyCode::Enter);
        press(&mut app, KeyCode::Char('u'));
        assert_eq!(app.notice, Some(Notice::NothingToUndo));
        clean_up(&path);
    }

    #[test]
    fn other_keys_do_nothing_while_moving() {
        let text = "- [ ] 交週報 @2026-10-16\n";
        let (mut app, path) = app_with_task_file("move-keys", text);
        press(&mut app, KeyCode::Char('r'));
        // q、Tab、Space、u 都不理（不會離開、不會換框、不會打勾）
        for code in ['q', ' ', 'u', 's', '3', 'a', 'e']
            .map(KeyCode::Char)
            .into_iter()
            .chain([KeyCode::Tab])
        {
            press(&mut app, code);
            assert!(app.moving.is_some(), "{code:?}");
            assert!(app.editor.is_none(), "{code:?}");
        }
        assert!(!app.should_quit);
        assert_eq!(app.view, View::Planner);
        assert_eq!(std::fs::read_to_string(&path).expect("read"), text);
        clean_up(&path);
    }

    #[test]
    fn clicking_a_day_picks_it_and_clicking_it_again_moves_there() {
        // 70 × 23、10 月在最上面：10/20（二）在 x 15～17、y 6
        let text = "- [ ] 交週報 @2026-10-16\n";
        let (mut app, path) = app_with_task_file("move-click", text);
        app.screen = ratatui::layout::Rect::new(0, 0, 70, 23);
        app.calendar_top = Some(0);
        press(&mut app, KeyCode::Char('r'));
        let click = MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: 16,
            row: 6,
            modifiers: KeyModifiers::NONE,
        };
        app.handle_mouse(click);
        assert_eq!(app.selected, date(2026, 10, 20));
        assert!(app.moving.is_some());
        app.handle_mouse(click);
        assert_eq!(
            std::fs::read_to_string(&path).expect("read"),
            "- [ ] 交週報 @2026-10-20\n"
        );
        assert!(app.moving.is_none());
        clean_up(&path);
    }

    #[test]
    fn moving_a_multi_day_item_keeps_its_length() {
        // 主管出差 10/28～11/3：從 11 月那一行按 r（選取 11/2），往後一週 → 整段往後一週，天數不變
        let text = "- 主管出差 @2026-10-28..2026-11-03\n";
        let (mut app, path) = app_with_task_file("move-range", text);
        app.agenda_index = 1;
        app.selected = date(2026, 11, 2);
        press(&mut app, KeyCode::Char('r'));
        press(&mut app, KeyCode::Char('j'));
        press(&mut app, KeyCode::Enter);
        assert_eq!(
            std::fs::read_to_string(&path).expect("read"),
            "- 主管出差 @2026-11-04..2026-11-10\n"
        );
        // 回到右邊：選著搬過去之後 11 月那一行（11/9 也是它的日子）
        assert_eq!(app.selected, date(2026, 11, 9));
        assert_eq!(
            app.chosen().map(|entry| entry.date),
            Some(date(2026, 11, 4))
        );
        clean_up(&path);
    }
}
