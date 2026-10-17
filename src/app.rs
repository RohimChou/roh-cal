//! App 的狀態和按鍵：今天、選取的日期、捲到哪

use crate::{
    mock::{self, Item},
    planner,
    theme::Theme,
};
use color_eyre::Result;
use jiff::{ToSpan, civil::Date};
use ratatui::{
    DefaultTerminal,
    crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers},
};

pub struct App {
    pub today: Date,
    pub selected: Date,
    /// 畫面上第一個月（1 號）：今天往前 12 個月
    pub first_month: Date,
    /// 畫面上最後一個月（1 號）：今天往後 24 個月
    pub last_month: Date,
    /// 捲動位置：畫面最上面是第幾列；畫的時候依選取的日期調整
    pub scroll: usize,
    /// 下次畫的時候，把選取的那個月捲到最上面（剛打開、按 `t` 的時候）
    pub jump_to_month: bool,
    pub items: Vec<Item>,
    pub theme: Theme,
    should_quit: bool,
}

impl App {
    pub fn new(today: Date) -> Self {
        let this_month = today.first_of_month();
        Self {
            today,
            selected: today,
            first_month: this_month.saturating_sub(12.months()),
            last_month: this_month.saturating_add(24.months()),
            scroll: 0,
            jump_to_month: true,
            items: mock::items(),
            theme: Theme::catppuccin_mocha(),
            should_quit: false,
        }
    }

    /// 主迴圈：畫畫面 → 等一個輸入 → 處理，直到按了離開
    pub fn run(mut self, terminal: &mut DefaultTerminal) -> Result<()> {
        while !self.should_quit {
            terminal.draw(|frame| planner::render(frame, &mut self))?;
            // event::read() 會 block 到有輸入為止
            if let Event::Key(key) = event::read()?
                && key.kind == KeyEventKind::Press
            {
                self.handle_key(key);
            }
        }
        Ok(())
    }

    fn handle_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Char('q') | KeyCode::Esc => self.should_quit = true,
            KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.should_quit = true;
            }
            KeyCode::Char('h') | KeyCode::Left => self.move_by(-1),
            KeyCode::Char('l') | KeyCode::Right => self.move_by(1),
            KeyCode::Char('j') | KeyCode::Down => self.move_by(7),
            KeyCode::Char('k') | KeyCode::Up => self.move_by(-7),
            KeyCode::Char('t') => {
                self.selected = self.today;
                self.jump_to_month = true;
            }
            _ => {}
        }
    }

    /// 選取的日期往後（負的 = 往前）移幾天；超出畫面上的月份就停在頭 / 尾那天
    fn move_by(&mut self, days: i64) {
        let moved = self.selected.saturating_add(days.days());
        self.selected = moved.clamp(self.first_month, self.last_month.last_of_month());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use jiff::civil::date;

    fn press(app: &mut App, code: KeyCode) {
        app.handle_key(KeyEvent::from(code));
    }

    #[test]
    fn hjkl_move_by_a_day_or_a_week_and_t_jumps_back() {
        let mut app = App::new(date(2026, 10, 17));
        press(&mut app, KeyCode::Char('j'));
        assert_eq!(app.selected, date(2026, 10, 24));
        press(&mut app, KeyCode::Char('l'));
        assert_eq!(app.selected, date(2026, 10, 25));
        press(&mut app, KeyCode::Char('k'));
        press(&mut app, KeyCode::Char('h'));
        assert_eq!(app.selected, date(2026, 10, 17));
        // 往後一個多月再按 t：回到今天，而且要把那個月捲到最上面
        for _ in 0..6 {
            press(&mut app, KeyCode::Down);
        }
        app.jump_to_month = false;
        press(&mut app, KeyCode::Char('t'));
        assert_eq!(app.selected, date(2026, 10, 17));
        assert!(app.jump_to_month);
    }

    #[test]
    fn moving_stops_at_the_first_and_last_day_on_screen() {
        // 畫面上是 2025/10 ～ 2028/10
        let mut app = App::new(date(2026, 10, 17));
        app.selected = date(2025, 10, 3);
        press(&mut app, KeyCode::Char('k'));
        assert_eq!(app.selected, date(2025, 10, 1));
        app.selected = date(2028, 10, 28);
        press(&mut app, KeyCode::Char('j'));
        assert_eq!(app.selected, date(2028, 10, 31));
    }
}
