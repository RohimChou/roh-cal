//! App 的狀態和按鍵：今天、選取的日期、捲到哪

use crate::{
    mock::{self, Item},
    planner,
    texts::Lang,
    theme::Theme,
};
use color_eyre::Result;
use jiff::{ToSpan, civil::Date};
use ratatui::{
    DefaultTerminal,
    crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers},
};

/// 鍵盤在操作哪一邊
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Focus {
    /// 左邊的月曆
    Calendar,
    /// 右邊的事項
    Agenda,
}

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
    /// 畫面的語言，按 `L` 切換
    pub lang: Lang,
    /// 鍵盤在操作哪一邊，按 `Tab` 切換
    pub focus: Focus,
    /// 右邊選到第幾件事（`items` 的 index）；`focus` 是 `Agenda` 時才用
    pub selected_item: usize,
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
            lang: Lang::Zh,
            focus: Focus::Calendar,
            selected_item: 0,
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
            KeyCode::Char('q') => self.should_quit = true,
            // Esc：在右邊就回月曆，在月曆就離開
            KeyCode::Esc if self.focus == Focus::Agenda => self.focus = Focus::Calendar,
            KeyCode::Esc => self.should_quit = true,
            KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.should_quit = true;
            }
            KeyCode::Tab => self.toggle_focus(),
            KeyCode::Char('t') => {
                self.selected = self.today;
                self.focus = Focus::Calendar;
                self.jump_to_month = true;
            }
            KeyCode::Char('L') => self.lang = self.lang.toggled(),
            _ if self.focus == Focus::Agenda => self.handle_agenda_key(key.code),
            KeyCode::Char('h') | KeyCode::Left => self.move_by(-1),
            KeyCode::Char('l') | KeyCode::Right => self.move_by(1),
            KeyCode::Char('j') | KeyCode::Down => self.move_by(7),
            KeyCode::Char('k') | KeyCode::Up => self.move_by(-7),
            _ => {}
        }
    }

    /// 右邊的事項：`j` / `k` 上下一件，月曆選取的日期跟著那件事走
    fn handle_agenda_key(&mut self, code: KeyCode) {
        let last = self.items.len().saturating_sub(1);
        match code {
            KeyCode::Char('j') | KeyCode::Down => {
                self.selected_item = (self.selected_item + 1).min(last);
            }
            KeyCode::Char('k') | KeyCode::Up => {
                self.selected_item = self.selected_item.saturating_sub(1);
            }
            _ => return,
        }
        self.selected = self.items[self.selected_item].date;
    }

    /// `Tab`：月曆 ↔ 右邊的事項；切到右邊時選「選取那天或之後」的第一件事（都沒有就最後一件）
    fn toggle_focus(&mut self) {
        match self.focus {
            Focus::Agenda => self.focus = Focus::Calendar,
            Focus::Calendar => {
                let Some(last) = self.items.len().checked_sub(1) else {
                    return;
                };
                self.selected_item = self
                    .items
                    .iter()
                    .position(|item| item.date >= self.selected)
                    .unwrap_or(last);
                self.selected = self.items[self.selected_item].date;
                self.focus = Focus::Agenda;
            }
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
        // L 切換語言，再按一次切回來
        press(&mut app, KeyCode::Char('L'));
        assert_eq!(app.lang, Lang::En);
        press(&mut app, KeyCode::Char('L'));
        assert_eq!(app.lang, Lang::Zh);
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
