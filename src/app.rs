//! 程式的狀態與主迴圈：畫畫面 → 等輸入 → 改狀態，一直重複到離開

use crate::{tasks::Item, texts::Lang, theme::Theme, ui};
use color_eyre::Result;
use jiff::{ToSpan, civil::Date};
use ratatui::{
    DefaultTerminal,
    crossterm::event::{
        self, KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
    },
    layout::Rect,
};

/// 整個程式的狀態
#[derive(Debug)]
pub struct App {
    /// 畫面的語言
    pub lang: Lang,
    /// 今天（打開程式那天；test 可以指定任何一天）
    pub today: Date,
    /// 選取的日期：月曆上加括號 `[17]` 的那天
    pub selected: Date,
    /// 任務檔裡的所有項目
    pub items: Vec<Item>,
    /// 月曆捲到哪：畫面最上面是第幾列（`None` = 下一次畫的時候把選取那天的那個月放在最上面）
    pub calendar_top: Option<i64>,
    /// 畫面的顏色
    pub theme: Theme,
    /// terminal 的大小（每一輪畫之前更新），滑鼠點的位置要用它換算
    pub screen: Rect,
    /// `true` = 這一輪畫完就結束主迴圈
    should_quit: bool,
}

impl App {
    /// 剛打開時的狀態：選取今天、theme 是 Catppuccin Mocha
    pub fn new(lang: Lang, today: Date, items: Vec<Item>) -> Self {
        Self {
            lang,
            today,
            selected: today,
            items,
            calendar_top: None,
            theme: Theme::catppuccin_mocha(),
            screen: Rect::default(),
            should_quit: false,
        }
    }

    /// 主迴圈：每一輪畫一次畫面、等一個輸入，直到要離開
    pub fn run(mut self, terminal: &mut DefaultTerminal) -> Result<()> {
        while !self.should_quit {
            // 先記下 terminal 的大小（視窗拉大縮小也會跟著變），畫面和滑鼠換算位置才一致
            let size = terminal.size()?;
            self.screen = Rect::new(0, 0, size.width, size.height);
            terminal.draw(|frame| ui::render(frame, &mut self))?;
            // event::read() 會 block 到有輸入為止；按鍵只處理按下（有的 terminal 放開也會送一次）
            let event = event::read()?;
            if let Some(key) = event.as_key_press_event() {
                self.handle_key(key);
            } else if let Some(mouse) = event.as_mouse_event() {
                self.handle_mouse(mouse);
            }
        }
        Ok(())
    }

    /// 按鍵：`hjkl` / 方向鍵移動選取的日期、`t` 回今天、`q` / `Esc` / `Ctrl+C` 離開
    fn handle_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Char('q') | KeyCode::Esc => self.should_quit = true,
            // raw mode 下 Ctrl+C 不會送 SIGINT 讓程式結束，要自己處理
            KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.should_quit = true;
            }
            // 左右 = 前一天 / 後一天，上下 = 上一週 / 下一週（像 vim）
            KeyCode::Char('h') | KeyCode::Left => self.move_by(-1),
            KeyCode::Char('l') | KeyCode::Right => self.move_by(1),
            KeyCode::Char('k') | KeyCode::Up => self.move_by(-7),
            KeyCode::Char('j') | KeyCode::Down => self.move_by(7),
            // 回今天，並把今天那個月捲到最上面
            KeyCode::Char('t') => {
                self.selected = self.today;
                self.calendar_top = None;
            }
            _ => {}
        }
    }

    /// 滑鼠：滾輪上下移一週、左鍵點月曆上的日期 = 選那天
    fn handle_mouse(&mut self, mouse: MouseEvent) {
        match mouse.kind {
            // 往下滾（看後面的日期）：下一週；往上滾：上一週
            MouseEventKind::ScrollDown => self.move_by(7),
            MouseEventKind::ScrollUp => self.move_by(-7),
            // 點在日期上才選；點在表頭、線、空格上不理
            MouseEventKind::Down(MouseButton::Left) => {
                if let Some(date) = ui::date_at(self, mouse.column, mouse.row) {
                    self.selected = date;
                }
            }
            // 其他（右鍵、拖曳、移動）不理
            _ => {}
        }
    }

    /// 選取的日期往後（負的 = 往前）移幾天
    fn move_by(&mut self, days: i64) {
        self.selected = self.selected.saturating_add(days.days());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use jiff::civil::date;

    /// 按一個鍵（沒有 Ctrl 之類的 modifier）
    fn press(app: &mut App, code: KeyCode) {
        app.handle_key(KeyEvent::from(code));
    }

    #[test]
    fn hjkl_and_arrows_move_by_a_day_or_a_week() {
        let mut app = App::new(Lang::En, date(2026, 10, 17), Vec::new());
        press(&mut app, KeyCode::Char('j'));
        assert_eq!(app.selected, date(2026, 10, 24));
        press(&mut app, KeyCode::Char('l'));
        assert_eq!(app.selected, date(2026, 10, 25));
        press(&mut app, KeyCode::Char('k'));
        assert_eq!(app.selected, date(2026, 10, 18));
        press(&mut app, KeyCode::Char('h'));
        assert_eq!(app.selected, date(2026, 10, 17));
        // 方向鍵一樣；跨月、跨年都照常移
        press(&mut app, KeyCode::Down);
        press(&mut app, KeyCode::Down);
        assert_eq!(app.selected, date(2026, 10, 31));
        press(&mut app, KeyCode::Right);
        assert_eq!(app.selected, date(2026, 11, 1));
        press(&mut app, KeyCode::Up);
        press(&mut app, KeyCode::Left);
        assert_eq!(app.selected, date(2026, 10, 24));
        app.selected = date(2026, 12, 29);
        press(&mut app, KeyCode::Char('j'));
        assert_eq!(app.selected, date(2027, 1, 5));
    }

    /// 滑鼠在 (`column`, `row`) 的一個動作
    fn mouse(kind: MouseEventKind, column: u16, row: u16) -> MouseEvent {
        MouseEvent {
            kind,
            column,
            row,
            modifiers: KeyModifiers::NONE,
        }
    }

    #[test]
    fn wheel_moves_a_week() {
        let mut app = App::new(Lang::En, date(2026, 10, 17), Vec::new());
        app.handle_mouse(mouse(MouseEventKind::ScrollDown, 0, 0));
        assert_eq!(app.selected, date(2026, 10, 24));
        app.handle_mouse(mouse(MouseEventKind::ScrollUp, 0, 0));
        app.handle_mouse(mouse(MouseEventKind::ScrollUp, 0, 0));
        assert_eq!(app.selected, date(2026, 10, 10));
    }

    #[test]
    fn left_click_on_a_date_selects_it() {
        // 40 × 27 的畫面、10 月在最上面：16 號在 (24～26, 6)，見 ui.rs 的畫面 test
        let mut app = App::new(Lang::En, date(2026, 10, 17), Vec::new());
        app.screen = Rect::new(0, 0, 40, 27);
        app.calendar_top = Some(0);
        app.handle_mouse(mouse(MouseEventKind::Down(MouseButton::Left), 25, 6));
        assert_eq!(app.selected, date(2026, 10, 16));
        // 右鍵、點在表頭上：不理
        app.handle_mouse(mouse(MouseEventKind::Down(MouseButton::Right), 28, 6));
        app.handle_mouse(mouse(MouseEventKind::Down(MouseButton::Left), 28, 2));
        assert_eq!(app.selected, date(2026, 10, 16));
    }

    #[test]
    fn t_goes_back_to_today() {
        let mut app = App::new(Lang::En, date(2026, 10, 17), Vec::new());
        app.selected = date(2027, 3, 1);
        app.calendar_top = Some(30);
        press(&mut app, KeyCode::Char('t'));
        assert_eq!(app.selected, date(2026, 10, 17));
        assert_eq!(app.calendar_top, None);
    }

    #[test]
    fn quit_keys() {
        let keys = [
            KeyEvent::from(KeyCode::Char('q')),
            KeyEvent::from(KeyCode::Esc),
            KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL),
        ];
        for key in keys {
            let mut app = App::new(Lang::En, date(2026, 10, 17), Vec::new());
            app.handle_key(key);
            assert!(app.should_quit, "{key:?} 應該要離開");
        }
    }

    #[test]
    fn other_keys_do_not_quit() {
        // 只按 c（沒有 Ctrl）、大寫 Q、Enter 都不離開
        let keys = [
            KeyEvent::from(KeyCode::Char('c')),
            KeyEvent::from(KeyCode::Char('Q')),
            KeyEvent::from(KeyCode::Enter),
        ];
        for key in keys {
            let mut app = App::new(Lang::En, date(2026, 10, 17), Vec::new());
            app.handle_key(key);
            assert!(!app.should_quit, "{key:?} 不應該離開");
        }
    }
}
