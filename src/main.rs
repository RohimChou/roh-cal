use color_eyre::Result;
use jiff::{Zoned, civil::Date};
use ratatui::{
    DefaultTerminal, Frame,
    crossterm::event::{self, KeyCode},
    widgets::{Block, Paragraph},
};

fn main() -> Result<()> {
    // 出錯時印出好讀的錯誤訊息與 backtrace
    color_eyre::install()?;
    // 進入 TUI 模式（raw mode + alternate screen），closure 結束後自動還原 terminal
    ratatui::run(|terminal| App::new().run(terminal))
}

/// 整個 app 的狀態
#[derive(Debug)]
struct App {
    /// true 時離開主迴圈
    should_quit: bool,
    /// 今天的日期（啟動時取一次，用本機時區）
    today: Date,
}

impl App {
    /// 建立 app，記下今天的日期
    fn new() -> Self {
        Self {
            should_quit: false,
            today: Zoned::now().date(),
        }
    }

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
        // 月曆下面空一行，再放操作說明
        let mut lines = month_lines(self.today);
        lines.push(String::new());
        lines.push("按 q 或 Esc 離開".to_string());
        let body = Paragraph::new(lines.join("\n")).block(block);
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

/// 組出 `today` 所在月份的月曆文字：標題、星期列，接著一週一行
///
/// 每格 5 個字元寬，日期靠右對齊在星期名稱下面，例：2026 年 10 月（1 號是星期四）
/// ```text
///  MON  TUE  WED  THU  FRI  SAT  SUN
///                   1    2    3    4
///    5    6    7    8    9   10   11
/// ```
fn month_lines(today: Date) -> Vec<String> {
    let first = today.first_of_month();
    // 1 號前面要空幾格（星期一 = 0 … 星期日 = 6）
    let offset = first.weekday().to_monday_zero_offset();

    let mut lines = vec![
        format!("{} 年 {} 月", today.year(), today.month()),
        " MON  TUE  WED  THU  FRI  SAT  SUN".to_string(),
    ];
    let mut week = "     ".repeat(offset as usize);
    for day in 1..=first.days_in_month() {
        week.push_str(&format!("{day:>4} "));
        // 排到星期日就換下一行
        if (offset + day) % 7 == 0 {
            lines.push(week);
            week = String::new();
        }
    }
    // 最後一週沒排滿 7 天，也要放進去
    if !week.is_empty() {
        lines.push(week);
    }
    lines
}
