//! App 的狀態、按鍵 / 滑鼠處理、捲動規則（畫面在 `ui`）

use crate::calendar::{monday_of, month_top};
use crate::i18n::Texts;
use crate::tasks::Item;
use crate::theme::Theme;
use crate::ui::compact;
use color_eyre::Result;
use jiff::{ToSpan, Zoned, civil::Date};
use ratatui::{
    DefaultTerminal,
    crossterm::event::{self, KeyCode, MouseButton, MouseEvent, MouseEventKind},
};

/// 月曆固定顯示幾週（一個月最多佔 6 週，再加前後各 1 週）；捲動規則和畫面都用
pub const WEEKS: i64 = 8;
/// 選取的那週離上下邊緣至少保留幾週，不夠就捲動（像 vim 的 scrolloff）
pub const SCROLLOFF: i64 = 1;

/// 整個 app 的狀態；畫面（`ui`）拿 `&App` 唯讀，所以畫面要用的欄位是 `pub`
#[derive(Debug)]
pub struct App {
    /// true 時離開主迴圈
    should_quit: bool,
    /// 今天的日期（啟動時取一次，用本機時區）
    pub today: Date,
    /// 目前選取的日期，用 h / j / k / l 或方向鍵移動
    pub selected: Date,
    /// 月曆最上面那一週的星期一；選取的日期太靠近上下邊緣時才會跟著捲動
    pub top: Date,
    /// 畫面的顏色與樣式
    pub theme: Theme,
    /// 畫面上的字（語言）
    pub texts: Texts,
    /// 任務檔裡的項目（任務、事件）
    pub items: Vec<Item>,
}

impl App {
    /// 建立 app，今天用系統時間（本機時區）
    pub fn new(texts: Texts, items: Vec<Item>) -> Self {
        Self::on_date(Zoned::now().date(), texts, items)
    }

    /// 建立 app，指定「今天」是哪天；一開始選取今天（test 用這個指定日期）
    pub fn on_date(today: Date, texts: Texts, items: Vec<Item>) -> Self {
        let mut app = Self {
            should_quit: false,
            today,
            selected: today,
            top: month_top(today),
            theme: Theme::default(),
            texts,
            items,
        };
        // 今天如果太靠近上下邊緣，先捲到保留 SCROLLOFF 的位置
        // （WEEKS = 8、SCROLLOFF = 1 時本月一定在第 2～7 行，不會捲；改了這兩個值才可能捲）
        app.scroll_to_selected();
        app
    }

    /// 主迴圈：畫畫面 → 等按鍵 → 更新狀態，直到要離開
    pub fn run(mut self, terminal: &mut DefaultTerminal) -> Result<()> {
        while !self.should_quit {
            terminal.draw(|frame| compact::render(frame, &self))?;
            self.handle_events()?;
        }
        Ok(())
    }

    /// 處理一個輸入事件（鍵盤或滑鼠）；event::read() 會 block 到有輸入為止
    fn handle_events(&mut self) -> Result<()> {
        let event = event::read()?;
        // 鍵盤只理會「按下」，忽略放開 / 長按重複
        if let Some(key) = event.as_key_press_event() {
            self.handle_key(key.code);
        } else if let Some(mouse) = event.as_mouse_event() {
            self.handle_mouse(mouse);
        }
        Ok(())
    }

    /// 處理一個按鍵
    fn handle_key(&mut self, code: KeyCode) {
        match code {
            KeyCode::Char('q') | KeyCode::Esc => self.should_quit = true,
            // h / l（或 ← / →）：前一天 / 後一天
            KeyCode::Char('h') | KeyCode::Left => self.move_selected(-1),
            KeyCode::Char('l') | KeyCode::Right => self.move_selected(1),
            // k / j（或 ↑ / ↓）：上一週 / 下一週（同一欄往上 / 往下）
            KeyCode::Char('k') | KeyCode::Up => self.move_selected(-7),
            KeyCode::Char('j') | KeyCode::Down => self.move_selected(7),
            // t：跳回今天
            KeyCode::Char('t') => self.jump_to_today(),
            // 其他按鍵不理
            _ => {}
        }
    }

    /// 處理一個滑鼠事件：滾輪滾一格 = 按一次 j / k；左鍵點日期 = 選取那天
    fn handle_mouse(&mut self, mouse: MouseEvent) {
        match mouse.kind {
            // 往下滾（看後面的日期）：下一週
            MouseEventKind::ScrollDown => self.move_selected(7),
            // 往上滾：上一週
            MouseEventKind::ScrollUp => self.move_selected(-7),
            // 左鍵按下：點在日期上才選取；點到最上 / 最下那行時照 SCROLLOFF 捲一行
            MouseEventKind::Down(MouseButton::Left) => {
                if let Some(date) = compact::date_at(self.top, mouse.column, mouse.row) {
                    self.selected = date;
                    self.scroll_to_selected();
                }
            }
            // 其他（右鍵、拖曳、移動）不理
            _ => {}
        }
    }

    /// 選取的日期移動 `days` 天（負數 = 往前），需要時捲動月曆
    fn move_selected(&mut self, days: i64) {
        // 超出 jiff 能表示的範圍（例：9999-12-31 再往後）就不動
        let Ok(date) = self.selected.checked_add(days.days()) else {
            return;
        };
        self.selected = date;
        self.scroll_to_selected();
    }

    /// 跳回今天：今天還在畫面上就只移動選取（畫面不動），不在畫面上就回到剛打開 app 的畫面
    fn jump_to_today(&mut self) {
        self.selected = self.today;
        let week = monday_of(self.today);
        let last_week = self.top.saturating_add((WEEKS - 1).weeks());
        if week < self.top || week > last_week {
            self.top = month_top(self.today);
        }
        // 今天在畫面最上 / 最下那行時，照 SCROLLOFF 再捲一行
        self.scroll_to_selected();
    }

    /// 捲動月曆，讓選取的那週離上下邊緣至少 `SCROLLOFF` 週
    ///
    /// 例：`WEEKS = 8`、`SCROLLOFF = 1` → 選取的那週只能在第 2～7 行（共 8 行）
    fn scroll_to_selected(&mut self) {
        let week = monday_of(self.selected);
        // 選取的那週最上面 / 最下面可以在哪一行，換算成那行的星期一
        let min_week = self.top.saturating_add(SCROLLOFF.weeks());
        let max_week = self.top.saturating_add((WEEKS - 1 - SCROLLOFF).weeks());
        if week < min_week {
            // 太靠上：往上捲，讓選取的那週剛好在第 SCROLLOFF + 1 行
            self.top = week.saturating_sub(SCROLLOFF.weeks());
        } else if week > max_week {
            // 太靠下：往下捲，讓選取的那週剛好在倒數第 SCROLLOFF + 1 行
            self.top = week.saturating_sub((WEEKS - 1 - SCROLLOFF).weeks());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use jiff::civil::{Weekday, date};
    use ratatui::crossterm::event::KeyModifiers;

    /// 選取的那週在畫面第幾行（0 = 最上面）；不在畫面上回傳 `None`
    fn selected_row(app: &App) -> Option<i64> {
        let week = monday_of(app.selected);
        (0..WEEKS).find(|&row| app.top.saturating_add(row.weeks()) == week)
    }

    #[test]
    fn opening_screen_shows_week_before_the_1st_on_top() {
        let app = App::on_date(date(2026, 10, 10), Texts::en(), Vec::new());
        // 10/1 是星期四 → 那週的星期一是 9/28 → 前一週是 9/21
        assert_eq!(app.top, date(2026, 9, 21));
        assert_eq!(selected_row(&app), Some(2));
    }

    #[test]
    fn scrolling_keeps_selected_row_inside_scrolloff() {
        let min_row = SCROLLOFF;
        let max_row = WEEKS - 1 - SCROLLOFF;
        let starts = [
            date(2026, 10, 10),
            date(2026, 3, 1),
            date(2027, 2, 1),
            date(2028, 2, 29),
            date(2026, 12, 31),
        ];
        // h / l / k / j 各按 400 次（約 1 年 / 8 年）
        for start in starts {
            for days in [-1, 1, -7, 7] {
                let mut app = App::on_date(start, Texts::en(), Vec::new());
                let mut previous = selected_row(&app).expect("selected on screen");
                for _ in 0..400 {
                    app.move_selected(days);
                    let row = selected_row(&app).expect("selected on screen");
                    assert!(
                        (min_row..=max_row).contains(&row),
                        "{start} {days}: row {row}"
                    );
                    // 每按一次最多移 1 行，畫面不會跳
                    assert!((row - previous).abs() <= 1, "{start} {days}: jumped");
                    assert_eq!(app.top.weekday(), Weekday::Monday);
                    previous = row;
                }
            }
        }
    }

    #[test]
    fn jump_to_today_keeps_screen_when_today_is_visible() {
        let mut app = App::on_date(date(2026, 10, 10), Texts::en(), Vec::new());
        app.move_selected(7);
        let top = app.top;
        app.jump_to_today();
        assert_eq!(app.selected, date(2026, 10, 10));
        assert_eq!(app.top, top);
    }

    #[test]
    fn jump_to_today_resets_screen_when_far_away() {
        let mut app = App::on_date(date(2026, 10, 10), Texts::en(), Vec::new());
        for _ in 0..52 {
            app.move_selected(7);
        }
        app.jump_to_today();
        assert_eq!(app.selected, date(2026, 10, 10));
        assert_eq!(app.top, month_top(date(2026, 10, 10)));
    }

    #[test]
    fn arrow_keys_move_like_hjkl() {
        // 同一個方向，按字母鍵跟按方向鍵要走到同一天
        let pairs = [
            (KeyCode::Char('h'), KeyCode::Left),
            (KeyCode::Char('l'), KeyCode::Right),
            (KeyCode::Char('k'), KeyCode::Up),
            (KeyCode::Char('j'), KeyCode::Down),
        ];
        for (letter, arrow) in pairs {
            let mut by_letter = App::on_date(date(2026, 10, 10), Texts::en(), Vec::new());
            let mut by_arrow = App::on_date(date(2026, 10, 10), Texts::en(), Vec::new());
            by_letter.handle_key(letter);
            by_arrow.handle_key(arrow);
            assert_ne!(
                by_arrow.selected,
                date(2026, 10, 10),
                "{arrow:?} did nothing"
            );
            assert_eq!(
                by_arrow.selected, by_letter.selected,
                "{arrow:?} != {letter:?}"
            );
        }
    }

    #[test]
    fn mouse_wheel_moves_selected_by_one_week() {
        let mut app = App::on_date(date(2026, 10, 10), Texts::en(), Vec::new());
        let wheel = |kind| MouseEvent {
            kind,
            column: 0,
            row: 0,
            modifiers: KeyModifiers::NONE,
        };
        app.handle_mouse(wheel(MouseEventKind::ScrollDown));
        assert_eq!(app.selected, date(2026, 10, 17));
        app.handle_mouse(wheel(MouseEventKind::ScrollUp));
        app.handle_mouse(wheel(MouseEventKind::ScrollUp));
        assert_eq!(app.selected, date(2026, 10, 3));
    }

    #[test]
    fn left_click_selects_the_clicked_date() {
        let mut app = App::on_date(date(2026, 10, 10), Texts::en(), Vec::new());
        let click = |column, row| MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column,
            row,
            modifiers: KeyModifiers::NONE,
        };
        // 第 4 週星期二 = 10/13
        app.handle_mouse(click(12, 6));
        assert_eq!(app.selected, date(2026, 10, 13));
        // 點在左邊欄：不動
        app.handle_mouse(click(4, 6));
        assert_eq!(app.selected, date(2026, 10, 13));
        // 點最上面那行（9/22）：選取後照 scrolloff 往上捲一行，選取變成在第 2 行
        app.handle_mouse(click(12, 3));
        assert_eq!(app.selected, date(2026, 9, 22));
        assert_eq!(selected_row(&app), Some(SCROLLOFF));
    }

    #[test]
    fn moving_past_the_last_date_stays_put() {
        let mut app = App::on_date(Date::MAX, Texts::en(), Vec::new());
        app.move_selected(1);
        assert_eq!(app.selected, Date::MAX);
    }
}
