use color_eyre::Result;
use ratatui::{
    DefaultTerminal, Frame,
    crossterm::event::{self, KeyCode},
    widgets::{Block, Paragraph},
};

fn main() -> Result<()> {
    // 出錯時印出好讀的錯誤訊息與 backtrace
    color_eyre::install()?;
    // 進入 TUI 模式（raw mode + alternate screen），closure 結束後自動還原 terminal
    ratatui::run(|terminal| App::default().run(terminal))
}

/// 整個 app 的狀態
#[derive(Debug, Default)]
struct App {
    /// true 時離開主迴圈
    should_quit: bool,
}

impl App {
    /// 主迴圈：畫畫面 → 等按鍵 → 更新狀態，直到要離開
    fn run(mut self, terminal: &mut DefaultTerminal) -> Result<()> {
        while !self.should_quit {
            terminal.draw(|frame| self.render(frame))?;
            self.handle_events()?;
        }
        Ok(())
    }

    /// 依目前狀態畫出整個畫面
    fn render(&self, frame: &mut Frame) {
        let block = Block::bordered().title(" roh-cal ");
        let body = Paragraph::new("Hello, ratatui!  按 q 或 Esc 離開").block(block);
        frame.render_widget(body, frame.area());
    }

    /// 處理一個輸入事件；event::read() 會 block 到有輸入為止
    fn handle_events(&mut self) -> Result<()> {
        // 只理會「按下」，忽略放開 / 長按重複
        if let Some(key) = event::read()?.as_key_press_event()
            && matches!(key.code, KeyCode::Char('q') | KeyCode::Esc)
        {
            self.should_quit = true;
        }
        Ok(())
    }
}
