//! 版型 3 的「那天的事」跳窗：月曆上 `Enter`（或再點一次選取的那天）打開，列出選取那天所有的事
//! （格子放不下、寫成 `+N` 的也看得到）；按鍵跟版型 1 右邊的事項框一樣（畫面在 `ui::day`）

use super::{App, Focus};
use crate::ui;
use ratatui::{crossterm::event::KeyCode, layout::Position};

impl App {
    /// 打開跳窗：選第一件；那天沒事就不開
    pub(super) fn open_day_popup(&mut self) {
        if self.items_on(self.selected).is_empty() {
            return;
        }
        self.day_index = 0;
        self.day_clicked = false;
        self.focus = Focus::Day;
    }

    /// 鍵盤在跳窗：`j` / `k` 上下一件；寫檔的鍵跟右邊的事項框一樣：`Space` / `x` 打勾、`r` 改期、`e` / `Enter` 編輯、
    /// `a` 新增、`A` 新增子任務、`d` 刪除（`Esc` / `q` 關掉在 `handle_normal_key`）
    pub(super) fn handle_day_key(&mut self, code: KeyCode) {
        match code {
            KeyCode::Char('j') | KeyCode::Down => self.choose_day(1),
            KeyCode::Char('k') | KeyCode::Up => self.choose_day(-1),
            KeyCode::Char(' ' | 'x') => self.toggle_chosen(),
            KeyCode::Char('r') => self.start_moving(),
            KeyCode::Char('e') | KeyCode::Enter => self.start_editing(),
            KeyCode::Char('a') => self.start_adding(),
            KeyCode::Char('A') => self.start_adding_subtask(),
            KeyCode::Char('d') => self.ask_delete(),
            _ => {}
        }
    }

    /// 跳窗選往後（負的 = 往前）第 `step` 件；到第一件 / 最後一件就停住
    pub(super) fn choose_day(&mut self, step: isize) {
        let last = self.items_on(self.selected).len().saturating_sub(1);
        self.day_index = self.day_index.saturating_add_signed(step).min(last);
    }

    /// 跳窗開著時點左鍵：點一件 = 選它，再點一次已經選到的 = 打勾（跟右邊的事項框一樣；打開之後第一次點只算選，見 `day_clicked`）；
    /// 點框外 = 關掉跳窗（跟 `Esc` 一樣），點框線、空白的地方不理
    pub(super) fn click_day(&mut self, column: u16, row: u16) {
        if !ui::day_area(self).contains(Position::new(column, row)) {
            self.focus = Focus::Calendar;
            return;
        }
        match ui::day_item_at(self, column, row) {
            Some(index) if index == self.day_index && self.day_clicked => self.toggle_chosen(),
            Some(index) => {
                self.day_index = index;
                self.day_clicked = true;
            }
            None => {}
        }
    }

    /// 寫檔之後，跳窗選檔案第 `line` 行那件；它不在選取那天了（例：改了日期）就留在原本的位置
    pub(super) fn choose_day_line(&mut self, line: usize) {
        if let Some(index) = self
            .items_on(self.selected)
            .iter()
            .position(|item| item.line == line)
        {
            self.day_index = index;
        }
        self.keep_day_choice();
    }

    /// 那天的事變少了（刪除、復原、改了日期）：選的位置超出範圍就選最後一件，一件都沒有了就關掉跳窗
    pub(super) fn keep_day_choice(&mut self) {
        match self.items_on(self.selected).len().checked_sub(1) {
            Some(last) => self.day_index = self.day_index.min(last),
            None => self.focus = Focus::Calendar,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::View;
    use super::super::write::tests::{app_with_task_file, clean_up, press};
    use super::*;
    use crate::texts::Lang;
    use jiff::civil::date;
    use ratatui::crossterm::event::{KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
    use ratatui::layout::Rect;

    /// 10/17 那天有 4 件，跳窗的順序：出差（10/16 開始）、整理書櫃（沒時間）、週會 10:00、交週報 17:30；10/20 還有一件
    const DAY: &str = "# 工作\n- [ ] 交週報 @2026-10-17 17:30\n- 週會 @2026-10-17 10:00\n\
                       - [x] 整理書櫃 @2026-10-17\n- 出差 @2026-10-16..2026-10-18\n- [ ] 別天 @2026-10-20\n";

    /// 版型 3、選取 10/17、鍵盤在月曆的 app（沒有任務檔）
    fn month_app() -> App {
        let mut app = App::new(
            Lang::ZhTw,
            date(2026, 10, 17),
            crate::tasks::parse(DAY).items,
        );
        app.view = View::Month;
        app.screen = Rect::new(0, 0, 60, 20);
        app
    }

    /// 跳窗選到的那件的標題
    fn chosen(app: &App) -> Option<String> {
        app.chosen_item().map(|item| item.title.clone())
    }

    /// 在 (`column`, `row`) 按一下左鍵 / 滾一下滾輪
    fn mouse(app: &mut App, kind: MouseEventKind, column: u16, row: u16) {
        app.handle_mouse(MouseEvent {
            kind,
            column,
            row,
            modifiers: KeyModifiers::NONE,
        });
    }

    #[test]
    fn enter_opens_the_days_items_and_esc_or_q_closes_them() {
        let mut app = month_app();
        press(&mut app, KeyCode::Enter);
        assert_eq!((app.view, app.focus), (View::Month, Focus::Day));
        assert_eq!(chosen(&app).as_deref(), Some("出差"));
        // j 到最後一件就停住；k 往回一件；Tab 不理
        for _ in 0..10 {
            press(&mut app, KeyCode::Char('j'));
        }
        assert_eq!(chosen(&app).as_deref(), Some("交週報"));
        press(&mut app, KeyCode::Up);
        press(&mut app, KeyCode::Tab);
        assert_eq!(
            (app.focus, chosen(&app).as_deref()),
            (Focus::Day, Some("週會"))
        );
        // Esc 關掉，回到月曆；再打開選第一件；q 也只關掉、不離開
        press(&mut app, KeyCode::Esc);
        assert_eq!((app.view, app.focus), (View::Month, Focus::Calendar));
        press(&mut app, KeyCode::Enter);
        assert_eq!(chosen(&app).as_deref(), Some("出差"));
        press(&mut app, KeyCode::Char('q'));
        assert_eq!(app.focus, Focus::Calendar);
        assert!(!app.should_quit);
        // 那天沒事（10/19）：不開
        app.selected = date(2026, 10, 19);
        press(&mut app, KeyCode::Enter);
        assert_eq!(app.focus, Focus::Calendar);
    }

    #[test]
    fn clicks_and_the_wheel_choose_in_the_popup() {
        let mut app = month_app();
        press(&mut app, KeyCode::Enter);
        let area = ui::day_area(&app);
        // 點第 3 件（框線下面第 3 列）：選週會；點框線不理
        mouse(
            &mut app,
            MouseEventKind::Down(MouseButton::Left),
            area.x + 3,
            area.y + 3,
        );
        assert_eq!(chosen(&app).as_deref(), Some("週會"));
        mouse(
            &mut app,
            MouseEventKind::Down(MouseButton::Left),
            area.x,
            area.y + 3,
        );
        assert_eq!(app.focus, Focus::Day);
        // 滾輪往下一件，到最後一件停住
        mouse(&mut app, MouseEventKind::ScrollDown, 0, 0);
        mouse(&mut app, MouseEventKind::ScrollDown, 0, 0);
        assert_eq!(chosen(&app).as_deref(), Some("交週報"));
        // 點框外：關掉
        mouse(&mut app, MouseEventKind::Down(MouseButton::Left), 0, 0);
        assert_eq!(app.focus, Focus::Calendar);
    }

    /// 有任務檔、版型 3、選取 10/17、跳窗開著的 app
    fn popup_with_file(name: &str) -> (App, std::path::PathBuf) {
        let (mut app, path) = app_with_task_file(name, DAY);
        app.view = View::Month;
        app.focus = Focus::Calendar;
        app.selected = date(2026, 10, 17);
        app.screen = Rect::new(0, 0, 60, 20);
        press(&mut app, KeyCode::Enter);
        (app, path)
    }

    /// 一個字一個字打進編輯框
    fn type_text(app: &mut App, text: &str) {
        for c in text.chars() {
            press(app, KeyCode::Char(c));
        }
    }

    #[test]
    fn checking_and_editing_keep_the_popup_on_the_same_item() {
        let (mut app, path) = popup_with_file("day-popup-edit");
        let read = || std::fs::read_to_string(&path).expect("read");
        // 整理書櫃（第 2 件）Space：取消打勾
        press(&mut app, KeyCode::Char('j'));
        press(&mut app, KeyCode::Char(' '));
        assert!(read().contains("- [ ] 整理書櫃 @2026-10-17\n"));
        assert_eq!(
            (app.focus, chosen(&app).as_deref()),
            (Focus::Day, Some("整理書櫃"))
        );
        // 點選到的那件：打開之後第一次點只算選（連點兩下選取的那天，第二下不會順手打勾）；再點一次才打勾，檔案回到原本的樣子
        let area = ui::day_area(&app);
        let click = |app: &mut App| {
            mouse(
                app,
                MouseEventKind::Down(MouseButton::Left),
                area.x + 3,
                area.y + 2,
            );
        };
        click(&mut app);
        assert!(read().contains("- [ ] 整理書櫃 @2026-10-17\n"));
        click(&mut app);
        assert_eq!(read(), DAY);
        // e 加上 18:00：排到最後面，跳窗還選著它
        press(&mut app, KeyCode::Char('e'));
        type_text(&mut app, " 18:00");
        press(&mut app, KeyCode::Enter);
        assert!(read().contains("- [x] 整理書櫃 @2026-10-17 18:00\n"));
        assert_eq!(
            (app.focus, app.day_index, chosen(&app).as_deref()),
            (Focus::Day, 3, Some("整理書櫃"))
        );
        clean_up(&path);
    }

    #[test]
    fn deleting_moving_and_adding_from_the_popup() {
        let (mut app, path) = popup_with_file("day-popup-write");
        let read = || std::fs::read_to_string(&path).expect("read");
        // 最後一件（交週報）r → l → Enter：搬到 10/18；跳窗換成 10/18，選著它（出差、交週報）
        for _ in 0..3 {
            press(&mut app, KeyCode::Char('j'));
        }
        press(&mut app, KeyCode::Char('r'));
        assert_eq!(app.focus, Focus::Calendar);
        press(&mut app, KeyCode::Char('l'));
        press(&mut app, KeyCode::Enter);
        assert!(read().contains("- [ ] 交週報 @2026-10-18 17:30\n"));
        assert_eq!(app.selected, date(2026, 10, 18));
        assert_eq!(
            (app.focus, app.day_index, chosen(&app).as_deref()),
            (Focus::Day, 1, Some("交週報"))
        );
        // u：交週報回到 10/17；10/18 只剩出差，選的位置跟著縮
        press(&mut app, KeyCode::Char('u'));
        assert_eq!(read(), DAY);
        assert_eq!(
            (app.focus, chosen(&app).as_deref()),
            (Focus::Day, Some("出差"))
        );
        // a：預先填 10/18，加在「工作」最後面；跳窗選著新的那件
        press(&mut app, KeyCode::Char('a'));
        type_text(&mut app, "買票");
        press(&mut app, KeyCode::Enter);
        assert!(read().ends_with("- [ ] 別天 @2026-10-20\n- [ ] 買票 @2026-10-18\n"));
        assert_eq!(
            (app.focus, chosen(&app).as_deref()),
            (Focus::Day, Some("買票"))
        );
        // d、y 刪掉買票：選最後一件（出差）；再刪出差：那天沒事了，跳窗關掉、還是版型 3
        press(&mut app, KeyCode::Char('d'));
        press(&mut app, KeyCode::Char('y'));
        assert_eq!(
            (app.focus, chosen(&app).as_deref()),
            (Focus::Day, Some("出差"))
        );
        press(&mut app, KeyCode::Char('d'));
        press(&mut app, KeyCode::Char('y'));
        assert!(!read().contains("出差"));
        assert_eq!((app.view, app.focus), (View::Month, Focus::Calendar));
        clean_up(&path);
    }
}
