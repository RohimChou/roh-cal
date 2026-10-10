mod i18n;
mod theme;

use color_eyre::{Result, eyre::bail};
use i18n::{Lang, Texts};
use jiff::{
    ToSpan, Zoned,
    civil::{Date, Weekday},
};
use ratatui::{
    DefaultTerminal, Frame,
    crossterm::event::{self, KeyCode},
    layout::Rect,
    style::Style,
    text::{Line, Span},
    widgets::{Block, BorderType, Padding, Paragraph},
};
use theme::Theme;

/// 月曆固定顯示幾週（一個月最多佔 6 週，再加前後各 1 週）
const WEEKS: i64 = 8;
/// 選取的那週離上下邊緣至少保留幾週，不夠就捲動（像 vim 的 scrolloff）
const SCROLLOFF: i64 = 1;
/// 框線和內容之間：左右各空幾格、上下各空幾行
const PADDING: Padding = Padding::symmetric(2, 1);
/// 右邊月份標記的寬度：「 11 月」= 空白 1 + 月份 2 + 空白 1 + 「月」2（中文字佔 2 格）
const MONTH_MARKER_WIDTH: u16 = 6;
/// 月曆框的寬度：左右框線 2 + 左右 padding + 左邊欄 4 + 7 天 × 每格 5 + 月份標記
const CALENDAR_WIDTH: u16 = 2 + PADDING.left + PADDING.right + 4 + 7 * 5 + MONTH_MARKER_WIDTH;
/// 月曆框的高度：上下框線 2 + 上下 padding + 星期列 1 + WEEKS 週
const CALENDAR_HEIGHT: u16 = 2 + PADDING.top + PADDING.bottom + 1 + WEEKS as u16;

fn main() -> Result<()> {
    // 出錯時印出好讀的錯誤訊息與 backtrace
    color_eyre::install()?;
    // 語言：--lang 參數 → 系統語言 → 英文
    let lang = match lang_arg()? {
        Some(lang) => lang,
        None => Lang::from_system(),
    };
    // 進入 TUI 模式（raw mode + alternate screen），closure 結束後自動還原 terminal
    ratatui::run(|terminal| App::new(lang.texts()).run(terminal))
}

/// 讀命令列參數 `--lang <語言>`；沒給就回傳 `None`，例：`roh-cal --lang en`
///
/// 在進入 TUI 之前讀，參數錯了才能正常印出錯誤訊息
fn lang_arg() -> Result<Option<Lang>> {
    // 第 0 個是程式自己的路徑，跳過
    let mut args = std::env::args().skip(1);
    let mut lang = None;
    while let Some(arg) = args.next() {
        if arg != "--lang" {
            bail!("unknown argument: {arg} (usage: roh-cal [--lang en|zh-TW])");
        }
        let Some(code) = args.next() else {
            bail!("--lang needs a value, e.g. --lang en");
        };
        let Some(parsed) = Lang::from_code(&code) else {
            bail!("unsupported language: {code} (supported: en, zh-TW)");
        };
        lang = Some(parsed);
    }
    Ok(lang)
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
    /// 畫面的顏色與樣式
    theme: Theme,
    /// 畫面上的字（語言）
    texts: Texts,
}

impl App {
    /// 建立 app，記下今天的日期；一開始選取今天
    fn new(texts: Texts) -> Self {
        let today = Zoned::now().date();
        let mut app = Self {
            should_quit: false,
            today,
            selected: today,
            top: month_top(today),
            theme: Theme::default(),
            texts,
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
        let month = self.selected.first_of_month();
        let title = format!(" {} ", (self.texts.title)(month.year(), month.month()));
        let hint = format!(" {} ", self.texts.hint);
        let block = Block::bordered()
            // 圓角框線 ╭─╮
            .border_type(BorderType::Rounded)
            .border_style(self.theme.border)
            .padding(PADDING)
            // 上邊線中間：選取日期的年月
            .title_top(Line::styled(title, self.theme.title).centered())
            // 下邊線中間：操作說明
            .title_bottom(Line::styled(hint, self.theme.hint).centered());

        // 框只畫到剛好包住月曆的大小（terminal 比較小時就用 terminal 的大小）
        let area = frame.area();
        let area = Rect {
            width: area.width.min(CALENDAR_WIDTH),
            height: area.height.min(CALENDAR_HEIGHT),
            ..area
        };
        let lines = calendar_lines(
            self.top,
            self.selected,
            self.today,
            &self.theme,
            &self.texts,
        );
        frame.render_widget(Paragraph::new(lines).block(block), area);
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

/// 組出月曆：星期列，接著從 `top` 那週開始一週一行，共 `WEEKS` 週
///
/// 每行最左邊像 vim 的 relativenumber：選取的那行顯示選取日期的週數，其他行顯示跟選取那行差幾行。
/// 每行最右邊：這週有某個月的 1 號，就標出那個月（例：「11 月」）。
/// 顏色都來自 `theme`：週末、其他月份、今天、選取各有自己的樣式（疊的順序見 `Theme`）
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
fn calendar_lines(
    top: Date,
    selected: Date,
    today: Date,
    theme: &Theme,
    texts: &Texts,
) -> Vec<Line<'static>> {
    let month = selected.first_of_month();
    // 畫面上每一行的星期一
    let mondays: Vec<Date> = top.series(1.week()).take(WEEKS as usize).collect();
    // 選取的那週在第幾行（0 = 最上面）；選取一定在畫面上，找不到只是保險
    let selected_row = mondays
        .iter()
        .position(|&monday| monday == monday_of(selected))
        .unwrap_or(0);

    // 星期列：前面空出左邊欄的 4 格，每個星期名稱佔 5 格（靠右對齊 4 格 + 1 個空白）
    let mut header = Line::from("    ");
    let weekdays = [
        Weekday::Monday,
        Weekday::Tuesday,
        Weekday::Wednesday,
        Weekday::Thursday,
        Weekday::Friday,
        Weekday::Saturday,
        Weekday::Sunday,
    ];
    for (name, weekday) in texts.weekdays.into_iter().zip(weekdays) {
        let style = theme.weekday_header.patch(weekend_style(weekday, theme));
        let padding = " ".repeat(4_usize.saturating_sub(display_width(name)));
        header.push_span(Span::styled(format!("{padding}{name} "), style));
    }
    let mut lines = vec![header];
    for (row, monday) in mondays.into_iter().enumerate() {
        let mut week = Line::default();
        // 左邊欄：選取那行顯示選取日期的週數（靠左），其他行顯示差幾行（靠右）
        if row == selected_row {
            // 選取那行整行墊底色；每個日期自己的顏色會疊在上面
            week.style = theme.cursor_line;
            let week_number = format!("{:<3}", week_of_year(selected));
            week.push_span(Span::styled(week_number, theme.week_number));
        } else {
            let distance = row.abs_diff(selected_row);
            week.push_span(Span::styled(format!("{distance:>3}"), theme.row_distance));
        }
        week.push_span(" ");

        // 這週如果有某個月的 1 號，記下是幾月，排完 7 天後標在最右邊
        let mut month_start = None;
        // 這一週的 7 天（只有 9999-12-31 這種日期上限才會不滿 7 天）
        for date in monday.series(1.day()).take(7) {
            if date.day() == 1 {
                month_start = Some(date.month());
            }
            // 依序疊上 style（順序見 `Theme` 的說明）
            let mut style = theme.day.patch(weekend_style(date.weekday(), theme));
            if date.first_of_month() != month {
                style = style.patch(theme.other_month);
            }
            if date == today {
                style = style.patch(theme.today);
            }
            if date == selected {
                style = style.patch(theme.selected);
            }
            // 一格 5 字元 = 1 個空白 + 「空白 + 2 位數日期 + 空白」
            // 後面那段套 style，選取的背景色才不會只蓋住數字
            week.push_span(" ");
            week.push_span(Span::styled(format!(" {:>2} ", date.day()), style));
        }
        let marker = match month_start {
            Some(month) => format!(" {}", (texts.month_marker)(month)),
            None => String::new(),
        };
        // 補空白到同樣寬度，每行才一樣寬（cursorline 的底色才會一樣長）
        let width = usize::from(MONTH_MARKER_WIDTH);
        let padding = " ".repeat(width.saturating_sub(display_width(&marker)));
        week.push_span(Span::styled(marker, theme.month_marker));
        week.push_span(padding);
        lines.push(week);
    }
    lines
}

/// 星期六、星期日的樣式；平日不另外上色（回傳空的 `Style`，疊上去不會改變任何東西）
fn weekend_style(weekday: Weekday, theme: &Theme) -> Style {
    match weekday {
        Weekday::Saturday => theme.saturday,
        Weekday::Sunday => theme.sunday,
        _ => Style::new(),
    }
}

/// 文字在 terminal 上佔幾格：英文字母 1 格、中文字 2 格，例：「11 月」佔 5 格
fn display_width(text: &str) -> usize {
    Span::raw(text).width()
}
