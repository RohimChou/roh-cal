//! 程式的狀態與主迴圈：畫畫面 → 等輸入 → 改狀態，一直重複到離開

use crate::{texts::Lang, ui};
use color_eyre::Result;
use jiff::civil::Date;
use ratatui::{
    DefaultTerminal,
    crossterm::event::{self, KeyCode, KeyEvent, KeyModifiers},
};

/// 整個程式的狀態
#[derive(Debug)]
pub struct App {
    /// 畫面的語言
    pub lang: Lang,
    /// 今天（打開程式那天；test 可以指定任何一天）
    pub today: Date,
    /// `true` = 這一輪畫完就結束主迴圈
    should_quit: bool,
}

impl App {
    /// 剛打開時的狀態
    pub fn new(lang: Lang, today: Date) -> Self {
        Self {
            lang,
            today,
            should_quit: false,
        }
    }

    /// 主迴圈：每一輪畫一次畫面、等一個輸入，直到要離開
    pub fn run(mut self, terminal: &mut DefaultTerminal) -> Result<()> {
        while !self.should_quit {
            terminal.draw(|frame| ui::render(frame, &self))?;
            // event::read() 會 block 到有輸入為止；只處理按下（有的 terminal 放開也會送一次）
            if let Some(key) = event::read()?.as_key_press_event() {
                self.handle_key(key);
            }
        }
        Ok(())
    }

    /// 按鍵：`q` / `Esc` / `Ctrl+C` 離開，其他鍵先不理
    fn handle_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Char('q') | KeyCode::Esc => self.should_quit = true,
            // raw mode 下 Ctrl+C 不會送 SIGINT 讓程式結束，要自己處理
            KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.should_quit = true;
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use jiff::civil::date;

    #[test]
    fn quit_keys() {
        let keys = [
            KeyEvent::from(KeyCode::Char('q')),
            KeyEvent::from(KeyCode::Esc),
            KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL),
        ];
        for key in keys {
            let mut app = App::new(Lang::En, date(2026, 10, 17));
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
            let mut app = App::new(Lang::En, date(2026, 10, 17));
            app.handle_key(key);
            assert!(!app.should_quit, "{key:?} 不應該離開");
        }
    }
}
