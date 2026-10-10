use color_eyre::Result;
use jiff::{
    ToSpan, Zoned,
    civil::{Date, Weekday},
};
use ratatui::{
    DefaultTerminal, Frame,
    crossterm::event::{self, KeyCode},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Paragraph},
};

/// 月曆固定顯示幾週（一個月最多佔 6 週，再加前後各 1 週）
const WEEKS: i64 = 8;
/// 選取的那週離上下邊緣至少保留幾週，不夠就捲動（像 vim 的 scrolloff）
const SCROLLOFF: i64 = 1;

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
    /// 月曆最上面那一週的星期一；選取的日期太靠近上下邊緣時才會跟著捲動
    top: Date,
}

impl App {
    /// 建立 app，記下今天的日期；一開始選取今天
    fn new() -> Self {
        let today = Zoned::now().date();
        let mut app = Self {
            should_quit: false,
            today,
            selected: today,
            // 一開始：這個月 1 號那週的前一週放在最上面
            top: monday_of(today.first_of_month()).saturating_sub(1.week()),
        };
        // 今天如果太靠近上下邊緣，先捲到保留 SCROLLOFF 的位置
        // （WEEKS = 8、SCROLLOFF = 1 時本月一定在第 2～7 行，不會捲；改了這兩個值才可能捲）
        app.scroll_to_selected();
        app
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
        let mut lines = calendar_lines(self.top, self.selected, self.today);
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
            KeyCode::Char('h') => self.move_selected(-1),
            KeyCode::Char('l') => self.move_selected(1),
            // k / j：上一週 / 下一週（同一欄往上 / 往下）
            KeyCode::Char('k') => self.move_selected(-7),
            KeyCode::Char('j') => self.move_selected(7),
            // 其他按鍵不理
            _ => {}
        }
        Ok(())
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

/// `date` 那一週的星期一
fn monday_of(date: Date) -> Date {
    date.saturating_sub(date.weekday().to_monday_zero_offset().days())
}

/// 最上面那週是 `top` 時，畫面上最後一天（最下面那週的星期日）
fn last_visible(top: Date) -> Date {
    top.saturating_add((WEEKS * 7 - 1).days())
}

/// 組出月曆：標題、星期列，接著從 `top` 那週開始一週一行，共 `WEEKS` 週
///
/// 標題是選取日期的年月，不是那個月的日期用灰色；
/// 選取的日期反白、今天的日期加底線，兩個是同一天就兩種都套
///
/// 每格 5 個字元寬，日期靠右對齊在星期名稱下面，例：選 2026-10-10
/// ```text
///  MON  TUE  WED  THU  FRI  SAT  SUN
///   21   22   23   24   25   26   27    ← 9 月，灰色
///   28   29   30    1    2    3    4    ← 28–30 灰色
///    5    6    7    8    9   10   11
///  ...
///   26   27   28   29   30   31    1    ← 1 灰色
///    2    3    4    5    6    7    8    ← 11 月，灰色
/// ```
fn calendar_lines(top: Date, selected: Date, today: Date) -> Vec<Line<'static>> {
    let month = selected.first_of_month();
    let last = last_visible(top);

    let mut lines = vec![
        Line::from(format!("{} 年 {} 月", month.year(), month.month())),
        Line::from(" MON  TUE  WED  THU  FRI  SAT  SUN"),
    ];
    let mut week = Line::default();
    // 從 top 開始一天一天往後，到畫面最後一天為止
    for date in top.series(1.day()).take_while(|&date| date <= last) {
        // 一格 = 2 個空白 + 2 位數日期 + 1 個空白；只有日期數字套 style
        let mut style = Style::new();
        // 不是選取日期的那個月：灰色
        if date.first_of_month() != month {
            style = style.fg(Color::DarkGray);
        }
        // 今天：加底線
        if date == today {
            style = style.add_modifier(Modifier::UNDERLINED);
        }
        // 選取：反白（前景色和背景色對調）
        if date == selected {
            style = style.add_modifier(Modifier::REVERSED);
        }
        week.push_span("  ");
        week.push_span(Span::styled(format!("{:>2}", date.day()), style));
        week.push_span(" ");
        // 排到星期日就換下一行
        if date.weekday() == Weekday::Sunday {
            lines.push(week);
            week = Line::default();
        }
    }
    // 最後一週沒排滿 7 天（只有 9999-12-31 這種日期上限才會發生），也要放進去
    if !week.spans.is_empty() {
        lines.push(week);
    }
    lines
}
