use color_eyre::Result;
use jiff::{ToSpan, Zoned, civil::Date};
use ratatui::{
    DefaultTerminal, Frame,
    crossterm::event::{self, KeyCode},
    style::{Modifier, Style},
    text::{Line, Span},
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
    /// 目前選取的日期，用 h / j / k / l 移動
    selected: Date,
}

impl App {
    /// 建立 app，記下今天的日期；一開始選取今天
    fn new() -> Self {
        let today = Zoned::now().date();
        Self {
            should_quit: false,
            today,
            selected: today,
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
        let mut lines = month_lines(self.selected, self.today);
        lines.push(Line::default());
        lines.push(Line::from("h / j / k / l 移動  q 或 Esc 離開"));
        let body = Paragraph::new(lines).block(block);
        frame.render_widget(body, frame.area());
    }

    /// 處理一個輸入事件；event::read() 會 block 到有輸入為止
    fn handle_events(&mut self) -> Result<()> {
        // 只理會「按下」，忽略放開 / 長按重複
        let Some(key) = event::read()?.as_key_press_event() else {
            return Ok(());
        };
        match key.code {
            KeyCode::Char('q') | KeyCode::Esc => self.should_quit = true,
            // h / l：前一天 / 後一天
            KeyCode::Char('h') => self.selected = self.selected.checked_sub(1.day())?,
            KeyCode::Char('l') => self.selected = self.selected.checked_add(1.day())?,
            // k / j：上一週 / 下一週（同一欄往上 / 往下）
            KeyCode::Char('k') => self.selected = self.selected.checked_sub(1.week())?,
            KeyCode::Char('j') => self.selected = self.selected.checked_add(1.week())?,
            // 其他按鍵不理
            _ => {}
        }
        Ok(())
    }
}

/// 組出 `selected` 所在月份的月曆：標題、星期列，接著一週一行
///
/// 選取的日期反白、今天的日期加底線；兩個是同一天就兩種都套
///
/// 每格 5 個字元寬，日期靠右對齊在星期名稱下面，例：2026 年 10 月（1 號是星期四）
/// ```text
///  MON  TUE  WED  THU  FRI  SAT  SUN
///                   1    2    3    4
///    5    6    7    8    9   10   11
/// ```
fn month_lines(selected: Date, today: Date) -> Vec<Line<'static>> {
    let first = selected.first_of_month();
    // 今天在不在畫的這個月（例：選到 11 月時，10 月的今天不用標）
    let today_in_month = today.first_of_month() == first;
    // 1 號前面要空幾格（星期一 = 0 … 星期日 = 6）
    let offset = first.weekday().to_monday_zero_offset();

    let mut lines = vec![
        Line::from(format!("{} 年 {} 月", first.year(), first.month())),
        Line::from(" MON  TUE  WED  THU  FRI  SAT  SUN"),
    ];
    let mut week = Line::from("     ".repeat(offset as usize));
    for day in 1..=first.days_in_month() {
        // 一格 = 2 個空白 + 2 位數日期 + 1 個空白；只有日期數字套 style
        let mut style = Style::new();
        // 今天：加底線
        if today_in_month && day == today.day() {
            style = style.add_modifier(Modifier::UNDERLINED);
        }
        // 反白：前景色和背景色對調
        if day == selected.day() {
            style = style.add_modifier(Modifier::REVERSED);
        }
        week.push_span("  ");
        week.push_span(Span::styled(format!("{day:>2}"), style));
        week.push_span(" ");
        // 排到星期日就換下一行
        if (offset + day) % 7 == 0 {
            lines.push(week);
            week = Line::default();
        }
    }
    // 最後一週沒排滿 7 天，也要放進去
    if !week.spans.is_empty() {
        lines.push(week);
    }
    lines
}
