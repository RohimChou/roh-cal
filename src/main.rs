use color_eyre::Result;
use jiff::{ToSpan, Zoned, civil::Date};
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
            top: month_top(today),
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
        lines.push(Line::from("h / j / k / l 移動  t 今天  q 或 Esc 離開"));
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
            // t：跳回今天
            KeyCode::Char('t') => self.jump_to_today(),
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

/// 顯示 `date` 那個月時，畫面最上面那週的星期一：1 號那週的前一週
fn month_top(date: Date) -> Date {
    monday_of(date.first_of_month()).saturating_sub(1.week())
}

/// `date` 那一週的星期一
fn monday_of(date: Date) -> Date {
    date.saturating_sub(date.weekday().to_monday_zero_offset().days())
}

/// `date` 是它那一年的第幾週：1/1 所在的那週是第 1 週（每週從星期一開始）
///
/// 跨年那週看 `date` 落在哪一年，例：2025-12-30 是 2025 年第 53 週、2026-01-02 是 2026 年第 1 週
fn week_of_year(date: Date) -> i16 {
    // 1/1 那週裡，1/1 前面還有幾天（例：1/1 是星期四 → 前面有 3 天）
    let offset = date.first_of_year().weekday().to_monday_zero_offset();
    (date.day_of_year() - 1 + i16::from(offset)) / 7 + 1
}

/// 組出月曆：標題、星期列，接著從 `top` 那週開始一週一行，共 `WEEKS` 週
///
/// 每行最左邊像 vim 的 relativenumber：選取的那行顯示選取日期的週數，其他行顯示跟選取那行差幾行。
/// 標題是選取日期的年月，不是那個月的日期用灰色；
/// 選取的日期反白、今天的日期加底線，兩個是同一天就兩種都套
///
/// 左邊欄 4 格，每格日期 5 個字元寬、靠右對齊在星期名稱下面，例：選 2026-10-10
/// ```text
///      MON  TUE  WED  THU  FRI  SAT  SUN
///   2   21   22   23   24   25   26   27    ← 9 月，灰色
///   1   28   29   30    1    2    3    4    ← 28–30 灰色
/// 41     5    6    7    8    9   10   11    ← 選取的那週：第 41 週
///   1   12   13   14   15   16   17   18
///  ...
/// ```
fn calendar_lines(top: Date, selected: Date, today: Date) -> Vec<Line<'static>> {
    let month = selected.first_of_month();
    // 畫面上每一行的星期一
    let mondays: Vec<Date> = top.series(1.week()).take(WEEKS as usize).collect();
    // 選取的那週在第幾行（0 = 最上面）；選取一定在畫面上，找不到只是保險
    let selected_row = mondays
        .iter()
        .position(|&monday| monday == monday_of(selected))
        .unwrap_or(0);

    let mut lines = vec![
        Line::from(format!("    {} 年 {} 月", month.year(), month.month())),
        Line::from("     MON  TUE  WED  THU  FRI  SAT  SUN"),
    ];
    for (row, monday) in mondays.into_iter().enumerate() {
        let mut week = Line::default();
        // 左邊欄：選取那行顯示選取日期的週數（靠左、黃色粗體），其他行顯示差幾行（靠右、灰色）
        if row == selected_row {
            let style = Style::new().fg(Color::Yellow).add_modifier(Modifier::BOLD);
            week.push_span(Span::styled(
                format!("{:<3}", week_of_year(selected)),
                style,
            ));
        } else {
            let distance = row.abs_diff(selected_row);
            week.push_span(Span::styled(format!("{distance:>3}"), Color::DarkGray));
        }
        week.push_span(" ");

        // 這一週的 7 天（只有 9999-12-31 這種日期上限才會不滿 7 天）
        for date in monday.series(1.day()).take(7) {
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
        }
        lines.push(week);
    }
    lines
}
