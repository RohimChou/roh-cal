//! 程式的狀態與主迴圈：畫畫面 → 等輸入 → 改狀態，一直重複到離開

use color_eyre::Result;
use ratatui::{
    DefaultTerminal, Frame,
    crossterm::event::{self, KeyCode, KeyEvent, KeyModifiers},
    widgets::{Block, BorderType},
};

/// 整個程式的狀態
#[derive(Debug, Default)]
pub struct App {
    /// `true` = 這一輪畫完就結束主迴圈
    should_quit: bool,
}

impl App {
    /// 主迴圈：每一輪畫一次畫面、等一個輸入，直到要離開
    pub fn run(mut self, terminal: &mut DefaultTerminal) -> Result<()> {
        while !self.should_quit {
            terminal.draw(|frame| self.render(frame))?;
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

    /// 畫面：整個 terminal 一個圓角框，左上角寫 `roh-cal`
    fn render(&self, frame: &mut Frame) {
        let block = Block::bordered()
            .border_type(BorderType::Rounded)
            .title(" roh-cal ");
        frame.render_widget(block, frame.area());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::{Terminal, backend::TestBackend};

    #[test]
    fn quit_keys() {
        let keys = [
            KeyEvent::from(KeyCode::Char('q')),
            KeyEvent::from(KeyCode::Esc),
            KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL),
        ];
        for key in keys {
            let mut app = App::default();
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
            let mut app = App::default();
            app.handle_key(key);
            assert!(!app.should_quit, "{key:?} 不應該離開");
        }
    }

    #[test]
    fn render_frame_with_title() {
        let mut terminal = Terminal::new(TestBackend::new(20, 3)).unwrap();
        let app = App::default();
        terminal.draw(|frame| app.render(frame)).unwrap();
        terminal.backend().assert_buffer_lines([
            "╭ roh-cal ─────────╮",
            "│                  │",
            "╰──────────────────╯",
        ]);
    }
}
