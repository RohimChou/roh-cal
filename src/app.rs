//! App 的狀態、按鍵 / 滑鼠處理、捲動規則；當天清單在 `day_list`、表單在 `form`、寫任務檔在 `write`（畫面在 `ui`）

mod day_list;
mod form;
mod write;

pub use day_list::DayList;
pub use form::{Form, FormField, FormProblem};
pub use write::Notice;

use crate::calendar::{Scroll, monday_of, month_top, scrolled_top};
use crate::config;
use crate::i18n::Texts;
use crate::settings::{Panel, Settings};
use crate::tasks::Item;
use crate::theme::Theme;
use crate::ui::{self, compact, month};
use color_eyre::Result;
use jiff::{ToSpan, Zoned, civil::Date};
use ratatui::{
    DefaultTerminal,
    crossterm::event::{
        self, KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
    },
    layout::{Position, Rect},
};
use std::path::PathBuf;
use std::time::{Duration, Instant};
use write::UndoStep;

/// 現在顯示哪一種畫面；按 `v` 切換
///
/// 兩種畫面共用同一份選取 / 捲動狀態（`selected`、`top`），只是顯示的週數不同；
/// 切換時選取的日期不變，畫面只在選取的那週會掉出比較小的畫面時才捲動
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum View {
    /// 固定大小的小月曆（`ui::compact`）
    Compact,
    /// 填滿 terminal 的大月曆（`ui::month`）
    Month,
}

impl View {
    /// 這種畫面固定顯示幾週；捲動規則和畫面都用（`const fn`：畫面的常數也能拿它算）
    pub const fn weeks(self) -> i64 {
        match self {
            // 一個月最多佔 6 週，再加前後各 1 週當 context（一行很便宜）
            Self::Compact => 8,
            // 格子裡要放任務，每週會佔好幾行：只畫 6 週，每格才高一點
            Self::Month => 6,
        }
    }

    /// 下一種畫面（`v` 輪流切換）：小月曆 → 大月曆 → 小月曆
    fn next(self) -> Self {
        match self {
            Self::Compact => Self::Month,
            Self::Month => Self::Compact,
        }
    }
}

/// 疊在月曆上面的跳窗；一次只會開一個，開著時按鍵都給它
///
/// 表單裡有打到一半的字（`String`），所以不是 `Copy`；其他幾種的狀態都很小，比對時直接複製一份出來
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Overlay {
    /// 設定頁（`,`）
    Settings(Panel),
    /// 當天清單（`Enter` / `o`）
    DayList(DayList),
    /// 說明頁（`?`）；`back` = 關掉之後回到哪裡（從當天清單打開的，就回到那個清單）、
    /// `full` = 顯示完整版（打開時是精簡版，`Tab` 切換）、`scroll` = 往下捲了幾行（畫面放不下時才能捲）
    Help {
        back: Option<DayList>,
        full: bool,
        scroll: u16,
    },
    /// 新增 / 編輯表單（月曆或清單按 `a`、清單按 `Enter` / `o` / `e`）；表單比其他幾種大很多（字串、編輯中的項目），
    /// 放在 `Box` 裡（heap），`Overlay` 才不會每一種都佔表單那麼大的空間
    Form(Box<Form>),
}

/// 兩下之間最多隔多久才算「點兩下」（跟 Windows 預設一樣）
const DOUBLE_CLICK: Duration = Duration::from_millis(500);

/// 上一次在月曆上點到日期的時間和位置（判斷「點兩下」用）
#[derive(Debug, Clone, Copy)]
struct Click {
    at: Instant,
    column: u16,
    row: u16,
}

/// 整個 app 的狀態；畫面（`ui`）拿 `&App` 唯讀，所以畫面要用的欄位是 `pub`
#[derive(Debug)]
pub struct App {
    /// true 時離開主迴圈
    should_quit: bool,
    /// 今天的日期（啟動時取一次，用本機時區）
    pub today: Date,
    /// 目前選取的日期，用 h / j / k / l 或方向鍵移動
    pub selected: Date,
    /// 月曆最上面那一週的星期一；選取的日期太靠近上下邊緣時才會跟著捲動
    pub top: Date,
    /// 畫面的顏色與樣式
    pub theme: Theme,
    /// 畫面上的字（語言）
    pub texts: Texts,
    /// 任務檔裡的項目
    pub items: Vec<Item>,
    /// 任務檔的位置（打勾時寫回去）；`None` = 不寫檔（test 用 `on_date` 建的 app 都是 `None`）
    tasks_path: Option<PathBuf>,
    /// roh-cal 寫過的檔案，最新的在最後面；`u` 一次復原一步，最多記 `UNDO_LEVELS` 步
    undo: Vec<UndoStep>,
    /// 現在顯示哪一種畫面
    pub view: View,
    /// 執行時可以改的設定（格線開關…）
    pub settings: Settings,
    /// 設定檔裡現在的開關（啟動時讀到的，或上次存的）；跟 `settings` 不一樣 = 有改過還沒存
    saved_settings: Settings,
    /// 設定檔的位置；`None` = 不存檔（test 用 `on_date` 建的 app 都是 `None`，不會寫到真的設定檔）
    config_path: Option<PathBuf>,
    /// 現在開著的跳窗；`None` = 沒有跳窗，按鍵給月曆
    pub overlay: Option<Overlay>,
    /// 下框線暫時換成的提示（例：檔案在別處改過）；下一個按鍵就消失
    pub notice: Option<Notice>,
    /// 按了 `V`，正在等下一個鍵選 view（`c` 小月曆、`m` 大月曆，其他鍵取消）；下框線換成可以按的鍵
    pub view_prefix: bool,
    /// 上一次在月曆上點到日期（按了任何鍵就忘掉）；下一下點同一格、隔不到 `DOUBLE_CLICK` 就是點兩下
    last_click: Option<Click>,
    /// terminal 現在的大小（每次畫畫面前更新）；滑鼠點擊要換算成哪一格時用
    pub screen: Rect,
}

impl App {
    /// 建立 app，今天用系統時間（本機時區），開關用設定檔讀到的；
    /// 打勾寫回 `tasks_path`，設定頁改過的開關存回 `config_path`
    pub fn new(
        texts: Texts,
        items: Vec<Item>,
        tasks_path: Option<PathBuf>,
        settings: Settings,
        config_path: Option<PathBuf>,
    ) -> Self {
        let mut app = Self::on_date(Zoned::now().date(), texts, items);
        app.tasks_path = tasks_path;
        // 一開始是小月曆，捲動規則不看開關，所以換掉開關不用重新捲
        app.theme = Theme::with_colors(&settings.colors);
        app.saved_settings = settings.clone();
        app.settings = settings;
        app.config_path = config_path;
        app
    }

    /// 建立 app，指定「今天」是哪天；一開始選取今天（test 用這個指定日期）
    pub fn on_date(today: Date, texts: Texts, items: Vec<Item>) -> Self {
        let mut app = Self {
            should_quit: false,
            today,
            selected: today,
            top: month_top(today),
            theme: Theme::default(),
            texts,
            items,
            tasks_path: None,
            undo: Vec::new(),
            view: View::Compact,
            settings: Settings::default(),
            saved_settings: Settings::default(),
            config_path: None,
            overlay: None,
            notice: None,
            view_prefix: false,
            last_click: None,
            screen: Rect::default(),
        };
        // 小月曆上下各保留 1 週：本月一定在第 2～7 行，一開始不會捲；大月曆才會置中
        app.scroll_to_selected();
        app
    }

    /// 主迴圈：畫畫面 → 等按鍵 → 更新狀態，直到要離開
    pub fn run(mut self, terminal: &mut DefaultTerminal) -> Result<()> {
        while !self.should_quit {
            // 先記下 terminal 的大小（視窗拉大縮小也會跟著變），畫面和滑鼠換算才一致
            let size = terminal.size()?;
            self.screen = Rect::new(0, 0, size.width, size.height);
            terminal.draw(|frame| ui::render(frame, &self))?;
            self.handle_events()?;
            self.save_settings()?;
        }
        Ok(())
    }

    /// 設定頁關掉之後，開關跟設定檔裡的不一樣就存回去（設定頁開著時先不存，改來改去只寫一次）
    fn save_settings(&mut self) -> Result<()> {
        let settings_open = matches!(self.overlay, Some(Overlay::Settings(_)));
        if settings_open || self.settings == self.saved_settings {
            return Ok(());
        }
        if let Some(path) = &self.config_path {
            config::save(path, &self.settings)?;
        }
        self.saved_settings = self.settings.clone();
        Ok(())
    }

    /// 處理一個輸入事件（鍵盤或滑鼠）；event::read() 會 block 到有輸入為止
    fn handle_events(&mut self) -> Result<()> {
        let event = event::read()?;
        // 鍵盤只理會「按下」，忽略放開 / 長按重複
        if let Some(key) = event.as_key_press_event() {
            self.handle_key(key);
        } else if let Some(mouse) = event.as_mouse_event() {
            self.handle_mouse(mouse);
        }
        Ok(())
    }

    /// 處理一個按鍵：有跳窗就給跳窗，沒有就給月曆
    fn handle_key(&mut self, key: KeyEvent) {
        // 上一次的提示：按任何鍵就消失；中間按了鍵，前後兩下點擊就不算點兩下
        self.notice = None;
        self.last_click = None;
        // Ctrl+C：在哪個畫面都直接離開（raw mode 下 Ctrl+C 只是一個按鍵，不會自己結束程式）。
        // 跳窗先關掉：設定頁改過的設定照樣存（`run` 每個按鍵之後都會檢查）；表單還沒存的字丟掉，跟 Esc 一樣
        if key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL) {
            self.overlay = None;
            self.should_quit = true;
            return;
        }
        match self.overlay {
            Some(Overlay::Settings(panel)) => self.handle_settings_key(panel, key),
            Some(Overlay::DayList(list)) => self.handle_day_list_key(list, key),
            Some(Overlay::Help { back, full, scroll }) => {
                self.handle_help_key(back, full, scroll, key)
            }
            Some(Overlay::Form(_)) => self.handle_form_key(key),
            None => self.handle_calendar_key(key),
        }
    }

    /// 設定頁開著時的按鍵（j / k 選列、h / l 改值），月曆不動
    fn handle_settings_key(&mut self, mut panel: Panel, key: KeyEvent) {
        // 正在問「全部還原嗎？」：只有 y 會還原，其他鍵都只是取消（按錯不會有事）
        if panel.confirm_reset_all {
            if key.code == KeyCode::Char('y') {
                self.settings = Settings::default();
                self.theme = Theme::with_colors(&self.settings.colors);
            }
            panel.confirm_reset_all = false;
            self.overlay = Some(Overlay::Settings(panel));
            return;
        }
        match key.code {
            // q / Esc / `,`：關閉設定頁（q 在這裡不是離開 app）
            KeyCode::Char('q') | KeyCode::Esc | KeyCode::Char(',') => {
                self.overlay = None;
                return;
            }
            KeyCode::Char('j') | KeyCode::Down => panel.select(1),
            KeyCode::Char('k') | KeyCode::Up => panel.select(-1),
            KeyCode::Char('h') | KeyCode::Left => {
                panel.row().adjust(-1, &mut self.settings, &mut self.theme);
            }
            KeyCode::Char('l') | KeyCode::Right | KeyCode::Char(' ') | KeyCode::Enter => {
                panel.row().adjust(1, &mut self.settings, &mut self.theme);
            }
            // r：這一列還原成預設
            KeyCode::Char('r') => panel.row().reset(&mut self.settings, &mut self.theme),
            // R（Shift+r）：全部還原成預設，先問一次
            KeyCode::Char('R') => panel.confirm_reset_all = true,
            _ => {}
        }
        // panel 是複製一份出來改的（`Copy`），改完放回去
        self.overlay = Some(Overlay::Settings(panel));
    }

    /// 設定頁開著時點左鍵：點一列 = 選它，再點一次（點已經選到的那列）= 改值，跟按 `l` 一樣；
    /// 點在段落名、操作說明、框外不理。正在問「全部還原嗎？」時點一下 = 取消那個問題（跟按其他鍵一樣）
    fn click_settings(&mut self, mut panel: Panel, column: u16, row: u16) {
        if panel.confirm_reset_all {
            panel.confirm_reset_all = false;
        } else if let Some(index) = ui::settings::row_at(panel, self, column, row) {
            if index == panel.selected {
                panel.row().adjust(1, &mut self.settings, &mut self.theme);
            } else {
                panel.selected = index;
            }
        }
        self.overlay = Some(Overlay::Settings(panel));
    }

    /// 說明頁開著時的按鍵：`j` / `k`（`↓` / `↑`）畫面放不下時上下捲；`Tab` 換精簡版 / 完整版（回到最上面）；
    /// `?` / `q` / `Esc` 關掉，回到打開前的地方（月曆或當天清單）；其他鍵不理
    fn handle_help_key(&mut self, back: Option<DayList>, full: bool, scroll: u16, key: KeyEvent) {
        // 最多能捲幾行：放得下是 0，按了也不動（terminal 拉高之後，原本捲的可能超過，先夾回來）
        let max = ui::help::max_scroll(self, full);
        let scroll = scroll.min(max);
        match key.code {
            KeyCode::Char('j') | KeyCode::Down => {
                let scroll = scroll.saturating_add(1).min(max);
                self.overlay = Some(Overlay::Help { back, full, scroll });
            }
            KeyCode::Char('k') | KeyCode::Up => {
                let scroll = scroll.saturating_sub(1);
                self.overlay = Some(Overlay::Help { back, full, scroll });
            }
            KeyCode::Tab | KeyCode::BackTab => {
                self.overlay = Some(Overlay::Help {
                    back,
                    full: !full,
                    scroll: 0,
                });
            }
            KeyCode::Char('?') | KeyCode::Char('q') | KeyCode::Esc => {
                self.overlay = back.map(Overlay::DayList);
            }
            _ => {}
        }
    }

    /// 沒有跳窗時的按鍵：移動、換月、跳回今天、切換畫面、打開當天清單、新增、復原、說明、設定頁、離開
    fn handle_calendar_key(&mut self, key: KeyEvent) {
        // 按了 V 之後的下一個鍵：c 小月曆、m 大月曆（大寫也算：Shift 還沒放開）；其他鍵（含 Esc、q）只是取消，不做那個鍵原本的事
        if self.view_prefix {
            self.view_prefix = false;
            match key.code {
                KeyCode::Char('c' | 'C') => self.set_view(View::Compact),
                KeyCode::Char('m' | 'M') => self.set_view(View::Month),
                _ => {}
            }
            return;
        }
        match key.code {
            KeyCode::Char('q') | KeyCode::Esc => self.should_quit = true,
            // h / l（或 ← / →）：前一天 / 後一天
            KeyCode::Char('h') | KeyCode::Left => self.move_selected(-1),
            KeyCode::Char('l') | KeyCode::Right => self.move_selected(1),
            // k / j（或 ↑ / ↓）：上一週 / 下一週（同一欄往上 / 往下）
            KeyCode::Char('k') | KeyCode::Up => self.move_selected(-7),
            KeyCode::Char('j') | KeyCode::Down => self.move_selected(7),
            // [ / ]：上個月 / 下個月的同一天
            KeyCode::Char('[') => self.move_selected_months(-1),
            KeyCode::Char(']') => self.move_selected_months(1),
            // t：跳回今天
            KeyCode::Char('t') => self.jump_to_today(),
            // Enter / o：打開選取那天的清單
            KeyCode::Enter | KeyCode::Char('o') => {
                self.overlay = Some(Overlay::DayList(DayList::open(self.selected)));
            }
            // a：新增表單，日期是選取的那天；存好 / 取消之後回到月曆
            KeyCode::Char('a') => {
                self.overlay = Some(Overlay::Form(Box::new(Form::open(self.selected, None))));
            }
            // u：復原 roh-cal 最後一次寫檔
            KeyCode::Char('u') => self.notice = Some(self.undo()),
            // ?：說明頁
            KeyCode::Char('?') => {
                self.overlay = Some(Overlay::Help {
                    back: None,
                    full: false,
                    scroll: 0,
                });
            }
            // `,`：打開設定頁（跟 macOS 的 Cmd+, 一樣）；其他跳窗開著時不會走到這裡，表單裡的 `,` 是打字
            KeyCode::Char(',') => self.overlay = Some(Overlay::Settings(Panel::default())),
            // v：輪流切換 view（小月曆 → 大月曆 → 小月曆）
            KeyCode::Char('v') => self.set_view(self.view.next()),
            // V：等下一個鍵選 view（V c / V m），下框線列出可以按的鍵
            KeyCode::Char('V') => self.view_prefix = true,
            // 其他按鍵不理
            _ => {}
        }
    }

    /// 處理一個滑鼠事件：滾輪滾一格 = 按一次 j / k；左鍵點一下 = 選，有跳窗就交給跳窗
    fn handle_mouse(&mut self, mouse: MouseEvent) {
        // 清單、表單開著時滾輪不理：清單是那一天的，底下的月曆動了會對不起來
        let wheel_ignored = matches!(self.overlay, Some(Overlay::DayList(_) | Overlay::Form(_)));
        let help_open = matches!(self.overlay, Some(Overlay::Help { .. }));
        match mouse.kind {
            // 說明頁開著：滾輪捲說明頁，跟按 ↓ / ↑ 一樣（底下的月曆不動）
            MouseEventKind::ScrollDown if help_open => {
                self.handle_key(KeyEvent::from(KeyCode::Down))
            }
            MouseEventKind::ScrollUp if help_open => self.handle_key(KeyEvent::from(KeyCode::Up)),
            // 往下滾（看後面的日期）：下一週
            MouseEventKind::ScrollDown if !wheel_ignored => self.move_selected(7),
            // 往上滾：上一週
            MouseEventKind::ScrollUp if !wheel_ignored => self.move_selected(-7),
            // 點兩下月曆的同一格：打開第一下選到的那天的清單（第一下之後畫面可能捲動了，第二下底下已經是別天）；
            // 第一下就打開了清單（點的是已經選到的那天）就不再做事，免得第二下被當成點在清單上
            MouseEventKind::Down(MouseButton::Left)
                if self.is_double_click(mouse.column, mouse.row) =>
            {
                self.last_click = None;
                if self.overlay.is_none() {
                    self.overlay = Some(Overlay::DayList(DayList::open(self.selected)));
                }
            }
            // 左鍵按下：有跳窗時點在框外 = 按 Esc（關掉跳窗、回到打開它的地方；正在問「刪除嗎？」這類問題時只是取消問題），
            // 底下的格子不會被選到
            MouseEventKind::Down(MouseButton::Left)
                if ui::overlay_bounds(self).is_some_and(|bounds| {
                    !bounds.contains(Position::new(mouse.column, mouse.row))
                }) =>
            {
                self.handle_key(KeyEvent::from(KeyCode::Esc));
            }
            // 點在跳窗裡就交給那個跳窗；沒有跳窗就是點月曆
            MouseEventKind::Down(MouseButton::Left) => match self.overlay {
                Some(Overlay::Settings(panel)) => {
                    self.click_settings(panel, mouse.column, mouse.row)
                }
                Some(Overlay::DayList(list)) => self.click_day_list(list, mouse.column, mouse.row),
                Some(Overlay::Form(_)) => self.click_form(mouse.column, mouse.row),
                Some(Overlay::Help { .. }) => {}
                None => self.click_calendar(mouse.column, mouse.row),
            },
            // 其他（右鍵、拖曳、移動）不理
            _ => {}
        }
    }

    /// 沒有跳窗時點左鍵：點在日期格子上 = 選那天（選完照捲動規則對齊，置中時那一格會移到中間）；
    /// 點已經選到的那天 = 打開那天的清單，跟按 `Enter` 一樣（點兩下見 `handle_mouse`）
    fn click_calendar(&mut self, column: u16, row: u16) {
        let date = match self.view {
            View::Compact => compact::date_at(self, column, row),
            View::Month => month::date_at(self, column, row),
        };
        let Some(date) = date else {
            return;
        };
        if date == self.selected {
            self.overlay = Some(Overlay::DayList(DayList::open(date)));
        } else {
            self.selected = date;
            self.scroll_to_selected();
        }
        self.last_click = Some(Click {
            at: Instant::now(),
            column,
            row,
        });
    }

    /// 這一下是不是「點兩下」的第二下：跟上一次在月曆上點的是同一格、隔不到 `DOUBLE_CLICK`，
    /// 而且畫面上沒有別的跳窗（只會是沒有跳窗，或第一下打開的清單）
    fn is_double_click(&self, column: u16, row: u16) -> bool {
        let same_cell = self.last_click.is_some_and(|last| {
            last.column == column && last.row == row && last.at.elapsed() < DOUBLE_CLICK
        });
        same_cell && matches!(self.overlay, None | Some(Overlay::DayList(_)))
    }

    /// 換成 `view`；週數變少時選取的那週可能掉出畫面，所以要重新對齊
    fn set_view(&mut self, view: View) {
        self.view = view;
        self.scroll_to_selected();
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

    /// 選取的日期移到 `months` 個月後的同一天（負數 = 往前）；那個月沒有這天就停在月底（例：10/31 → 11/30）。
    /// 畫面跟按 `t` 一樣，換成那個月剛打開的樣子
    fn move_selected_months(&mut self, months: i64) {
        // jiff 加月份時，日子超過那個月的天數會自動夾到月底；超出 jiff 能表示的範圍就不動
        let Ok(date) = self.selected.checked_add(months.months()) else {
            return;
        };
        self.show_month_of(date);
    }

    /// 跳回今天，畫面也回到剛打開 app 的位置
    fn jump_to_today(&mut self) {
        self.show_month_of(self.today);
    }

    /// 選取 `date`，畫面換成那天剛打開 app 的樣子（那個月的前一週在最上面；大月曆置中時選取的那週在正中間）
    fn show_month_of(&mut self, date: Date) {
        self.selected = date;
        self.top = month_top(date);
        self.scroll_to_selected();
    }

    /// 現在的捲動規則（規則本身在 `calendar::scrolled_top`）：
    /// - 小月曆：上下各保留 1 週
    /// - 大月曆：`center_cursor` 開著（預設）選取的那週固定在正中間；關著就自由移動，掉出畫面才捲
    fn scroll_mode(&self) -> Scroll {
        match self.view {
            View::Compact => Scroll::Scrolloff(1),
            View::Month if self.settings.month.center_cursor => Scroll::Center,
            View::Month => Scroll::Scrolloff(0),
        }
    }

    /// 捲動月曆，讓選取的那週照 `scroll_mode` 的規則出現在畫面上
    fn scroll_to_selected(&mut self) {
        let week = monday_of(self.selected);
        self.top = scrolled_top(self.top, week, self.view.weeks(), self.scroll_mode());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::files;
    use jiff::civil::{Weekday, date};

    /// 設定頁開著就回傳它的狀態，沒開是 `None`（test 用）
    fn panel(app: &App) -> Option<Panel> {
        match app.overlay {
            Some(Overlay::Settings(panel)) => Some(panel),
            _ => None,
        }
    }

    /// 沒有修飾鍵的按鍵事件（test 用）
    pub(super) fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    /// 選取的那週在畫面第幾行（0 = 最上面）；不在畫面上回傳 `None`
    fn selected_row(app: &App) -> Option<i64> {
        let week = monday_of(app.selected);
        (0..app.view.weeks()).find(|&row| app.top.saturating_add(row.weeks()) == week)
    }

    /// 大月曆、置中關掉的 app（test 用；最上面那週跟小月曆一樣是 9/21）
    fn month_view_without_centering(today: Date) -> App {
        let mut app = App::on_date(today, Texts::en(), Vec::new());
        app.view = View::Month;
        app.settings.month.center_cursor = false;
        app.scroll_to_selected();
        app
    }

    #[test]
    fn opening_screen_shows_week_before_the_1st_on_top() {
        let app = App::on_date(date(2026, 10, 10), Texts::en(), Vec::new());
        // 10/1 是星期四 → 那週的星期一是 9/28 → 前一週是 9/21
        assert_eq!(app.top, date(2026, 9, 21));
        assert_eq!(selected_row(&app), Some(2));
    }

    #[test]
    fn scrolling_keeps_selected_on_screen_in_both_modes() {
        let starts = [
            date(2026, 10, 10),
            date(2026, 3, 1),
            date(2027, 2, 1),
            date(2028, 2, 29),
            date(2026, 12, 31),
        ];
        // 小月曆（上下保留 1 週）、大月曆置中、大月曆自由；h / l / k / j 各按 400 次（約 1 年 / 8 年）
        let modes = [
            (View::Compact, true),
            (View::Month, true),
            (View::Month, false),
        ];
        for (view, center) in modes {
            for start in starts {
                for days in [-1, 1, -7, 7] {
                    let mut app = App::on_date(start, Texts::en(), Vec::new());
                    app.view = view;
                    app.settings.month.center_cursor = center;
                    app.scroll_to_selected();
                    let mut previous = selected_row(&app).expect("selected on screen");
                    for _ in 0..400 {
                        app.move_selected(days);
                        let row = selected_row(&app).expect("selected on screen");
                        match app.scroll_mode() {
                            // 置中：永遠在中間那行
                            Scroll::Center => {
                                assert_eq!(row, (view.weeks() - 1) / 2, "{view:?} {start} {days}");
                            }
                            // 保留邊界：在允許的行裡，而且每按一次最多移 1 行，畫面不會跳
                            Scroll::Scrolloff(margin) => {
                                assert!(
                                    (margin..=view.weeks() - 1 - margin).contains(&row),
                                    "{view:?} {start} {days}: row {row}"
                                );
                                assert!(
                                    (row - previous).abs() <= 1,
                                    "{view:?} {start} {days}: jumped"
                                );
                            }
                        }
                        assert_eq!(app.top.weekday(), Weekday::Monday);
                        previous = row;
                    }
                }
            }
        }
    }

    #[test]
    fn jump_to_today_restores_the_opening_screen_in_both_views() {
        // 三種模式都回到 9/21 在最上面（大月曆置中：10/5 那週在 6 行的第 3 行，剛好也是 9/21）
        let cases = [
            (View::Compact, true, date(2026, 9, 21)),
            (View::Month, true, date(2026, 9, 21)),
            (View::Month, false, date(2026, 9, 21)),
        ];
        for (view, center, top) in cases {
            let mut app = App::on_date(date(2026, 10, 10), Texts::en(), Vec::new());
            app.view = view;
            app.settings.month.center_cursor = center;
            // 捲了幾週（今天還在畫面上）和捲到一年後，按 t 都回到剛打開的畫面
            for weeks in [3, 52] {
                for _ in 0..weeks {
                    app.move_selected(7);
                }
                app.jump_to_today();
                assert_eq!(
                    app.selected,
                    date(2026, 10, 10),
                    "{view:?} {center} {weeks}"
                );
                assert_eq!(app.top, top, "{view:?} {center} {weeks}");
            }
        }
    }

    #[test]
    fn brackets_move_a_month_and_stop_at_the_month_end() {
        // (開始, 按鍵, 選到哪天)
        let cases = [
            (date(2026, 10, 10), ']', date(2026, 11, 10)),
            (date(2026, 10, 10), '[', date(2026, 9, 10)),
            // 下個月沒有那天：停在月底
            (date(2026, 10, 31), ']', date(2026, 11, 30)),
            (date(2026, 3, 31), '[', date(2026, 2, 28)),
            // 閏年 2 月有 29 天
            (date(2028, 1, 31), ']', date(2028, 2, 29)),
            (date(2027, 1, 31), ']', date(2027, 2, 28)),
            // 跨年
            (date(2026, 12, 15), ']', date(2027, 1, 15)),
            (date(2027, 1, 15), '[', date(2026, 12, 15)),
            // 超出 jiff 能表示的範圍：不動
            (date(9999, 12, 15), ']', date(9999, 12, 15)),
        ];
        for (start, bracket, expected) in cases {
            let mut app = App::on_date(start, Texts::en(), Vec::new());
            app.handle_key(key(KeyCode::Char(bracket)));
            assert_eq!(app.selected, expected, "{start} {bracket}");
        }
        // 停在月底之後，日子就是那一天：10/31 按兩次 ] 是 12/30
        let mut app = App::on_date(date(2026, 10, 31), Texts::en(), Vec::new());
        app.handle_key(key(KeyCode::Char(']')));
        app.handle_key(key(KeyCode::Char(']')));
        assert_eq!(app.selected, date(2026, 12, 30));
    }

    #[test]
    fn brackets_show_the_month_like_opening_the_app_on_that_day() {
        // 10/10 按 ]：11/10（星期二，那週的星期一是 11/9）；11/1 是星期日 → 那週的星期一 10/26 → 前一週 10/19
        let cases = [
            // 小月曆：10/19 在最上面，11/9 那週在第 4 行
            (View::Compact, true, date(2026, 10, 19)),
            // 大月曆置中：11/9 那週在 6 行的第 3 行 → 最上面 10/26
            (View::Month, true, date(2026, 10, 26)),
            // 大月曆不置中：跟小月曆一樣 10/19，11/9 那週在畫面上，不用再捲
            (View::Month, false, date(2026, 10, 19)),
        ];
        for (view, center, top) in cases {
            let mut app = App::on_date(date(2026, 10, 10), Texts::en(), Vec::new());
            app.view = view;
            app.settings.month.center_cursor = center;
            app.scroll_to_selected();
            app.handle_key(key(KeyCode::Char(']')));
            assert_eq!(app.selected, date(2026, 11, 10), "{view:?} {center}");
            assert_eq!(app.top, top, "{view:?} {center}");
        }
        // 小月曆按 [：9/10；9/1 是星期二 → 那週的星期一 8/31 → 前一週 8/24
        let mut app = App::on_date(date(2026, 10, 10), Texts::en(), Vec::new());
        app.handle_key(key(KeyCode::Char('[')));
        assert_eq!(app.selected, date(2026, 9, 10));
        assert_eq!(app.top, date(2026, 8, 24));
    }

    #[test]
    fn arrow_keys_move_like_hjkl() {
        // 同一個方向，按字母鍵跟按方向鍵要走到同一天
        let pairs = [
            (KeyCode::Char('h'), KeyCode::Left),
            (KeyCode::Char('l'), KeyCode::Right),
            (KeyCode::Char('k'), KeyCode::Up),
            (KeyCode::Char('j'), KeyCode::Down),
        ];
        for (letter, arrow) in pairs {
            let mut by_letter = App::on_date(date(2026, 10, 10), Texts::en(), Vec::new());
            let mut by_arrow = App::on_date(date(2026, 10, 10), Texts::en(), Vec::new());
            by_letter.handle_key(key(letter));
            by_arrow.handle_key(key(arrow));
            assert_ne!(
                by_arrow.selected,
                date(2026, 10, 10),
                "{arrow:?} did nothing"
            );
            assert_eq!(
                by_arrow.selected, by_letter.selected,
                "{arrow:?} != {letter:?}"
            );
        }
    }

    #[test]
    fn v_cycles_the_view_and_keeps_the_selection() {
        let mut app = App::on_date(date(2026, 10, 10), Texts::en(), Vec::new());
        app.move_selected(7);
        assert_eq!(app.view, View::Compact);
        app.handle_key(key(KeyCode::Char('v')));
        assert_eq!(app.view, View::Month);
        // 選取不動，大月曆置中：10/12 那週在 6 行的第 3 行 → 最上面 9/28
        assert_eq!(app.selected, date(2026, 10, 17));
        assert_eq!(app.top, date(2026, 9, 28));
        // 再按一次回到小月曆：10/12 那週在第 3 行，沒碰到邊界，畫面不動
        app.handle_key(key(KeyCode::Char('v')));
        assert_eq!(app.view, View::Compact);
        assert_eq!(app.top, date(2026, 9, 28));
    }

    #[test]
    fn switching_view_scrolls_only_when_the_selected_week_would_fall_off_without_centering() {
        let mut app = App::on_date(date(2026, 10, 10), Texts::en(), Vec::new());
        app.settings.month.center_cursor = false;
        // compact：往下 4 週到 11/7，選取在第 7 行（上下各留 1 週的最後一行）
        for _ in 0..4 {
            app.move_selected(7);
        }
        assert_eq!(app.selected, date(2026, 11, 7));
        assert_eq!(selected_row(&app), Some(6));
        assert_eq!(app.top, date(2026, 9, 21));
        // V m 切到 6 週的大月曆（不置中）：第 7 行不存在，往下捲到選取在最後一行
        app.handle_key(key(KeyCode::Char('V')));
        app.handle_key(key(KeyCode::Char('m')));
        assert_eq!(app.selected, date(2026, 11, 7));
        assert_eq!(selected_row(&app), Some(5));
        assert_eq!(app.top, date(2026, 9, 28));
    }

    #[test]
    fn shift_v_then_c_or_m_picks_a_view_and_any_other_key_cancels() {
        let mut app = App::on_date(date(2026, 10, 10), Texts::en(), Vec::new());
        // V：還沒換，等下一個鍵；m：大月曆
        app.handle_key(key(KeyCode::Char('V')));
        assert!(app.view_prefix);
        assert_eq!(app.view, View::Compact);
        app.handle_key(key(KeyCode::Char('m')));
        assert_eq!(app.view, View::Month);
        assert!(!app.view_prefix);
        // 再 V m 還是大月曆；V c：小月曆；大寫（Shift 還沒放開）也一樣
        for (second, view) in [
            ('m', View::Month),
            ('c', View::Compact),
            ('M', View::Month),
            ('C', View::Compact),
        ] {
            app.handle_key(key(KeyCode::Char('V')));
            app.handle_key(key(KeyCode::Char(second)));
            assert_eq!(app.view, view, "V {second}");
            assert!(!app.view_prefix);
        }
        // V 之後按別的鍵：只是取消，不做那個鍵原本的事（不移動、不離開、不開表單、v 不輪流），也不換 view
        for other in [
            KeyCode::Char('j'),
            KeyCode::Char('q'),
            KeyCode::Esc,
            KeyCode::Char('a'),
            KeyCode::Char('v'),
            KeyCode::Char('V'),
        ] {
            app.handle_key(key(KeyCode::Char('V')));
            app.handle_key(key(other));
            assert!(!app.view_prefix, "{other:?}");
            assert_eq!(app.view, View::Compact, "{other:?}");
        }
        assert_eq!(app.selected, date(2026, 10, 10));
        assert!(!app.should_quit);
        assert_eq!(app.overlay, None);
    }

    #[test]
    fn comma_opens_the_settings_panel_and_hjkl_edits_it() {
        let mut app = App::on_date(date(2026, 10, 10), Texts::en(), Vec::new());
        // `,` 打開；第一列是 week_numbers
        app.handle_key(key(KeyCode::Char(',')));
        assert_eq!(panel(&app), Some(Panel::default()));
        // l：切換開關；h：再切回來
        app.handle_key(key(KeyCode::Char('l')));
        assert!(!app.settings.calendar.week_numbers);
        app.handle_key(key(KeyCode::Char('h')));
        assert!(app.settings.calendar.week_numbers);
        // j 四次：跳過 grid_lines、item_icons、center_cursor 到 item；l：顏色從 subtext1 換成下一個色 subtext0，馬上套到 theme
        for _ in 0..4 {
            app.handle_key(key(KeyCode::Char('j')));
        }
        assert_eq!(panel(&app).map(|panel| panel.selected), Some(4));
        app.handle_key(key(KeyCode::Char('l')));
        assert_eq!(app.theme.item.fg, Some(crate::theme::PALETTE[16].1));
        // 開著的時候 h / j / k / l 不會動選取的日期
        assert_eq!(app.selected, date(2026, 10, 10));
        // r：只有選到的那列回到預設（item 回 subtext1，上面改過的 week_numbers 不受影響）
        for _ in 0..4 {
            app.handle_key(key(KeyCode::Char('k')));
        }
        app.handle_key(key(KeyCode::Char('l')));
        assert!(!app.settings.calendar.week_numbers);
        for _ in 0..4 {
            app.handle_key(key(KeyCode::Char('j')));
        }
        app.handle_key(key(KeyCode::Char('r')));
        assert_eq!(app.theme.item, Theme::default().item);
        assert!(!app.settings.calendar.week_numbers);
        // q / Esc / `,` 都能關；關了之後 q 才是離開
        app.handle_key(key(KeyCode::Char('q')));
        assert_eq!(app.overlay, None);
        assert!(!app.should_quit);
        app.handle_key(key(KeyCode::Char(',')));
        app.handle_key(key(KeyCode::Esc));
        assert_eq!(app.overlay, None);
        app.handle_key(key(KeyCode::Char(',')));
        app.handle_key(key(KeyCode::Char(',')));
        assert_eq!(app.overlay, None);
        assert!(!app.should_quit);
    }

    #[test]
    fn ctrl_c_quits_from_anywhere() {
        let ctrl_c = KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL);
        // 月曆、當天清單、表單、說明頁、設定頁：都直接離開，跳窗也關掉
        let openers: [&[KeyCode]; 5] = [
            &[],
            &[KeyCode::Enter],
            &[KeyCode::Char('a')],
            &[KeyCode::Char('?')],
            &[KeyCode::Char(',')],
        ];
        for keys in openers {
            let mut app = App::on_date(date(2026, 10, 10), Texts::en(), Vec::new());
            for &code in keys {
                app.handle_key(key(code));
            }
            assert_eq!(app.overlay.is_some(), !keys.is_empty(), "{keys:?}");
            app.handle_key(ctrl_c);
            assert!(app.should_quit, "{keys:?}");
            assert_eq!(app.overlay, None, "{keys:?}");
        }
        // 沒按 Ctrl 的 c：月曆上不理，表單裡就是打字
        let mut app = App::on_date(date(2026, 10, 10), Texts::en(), Vec::new());
        app.handle_key(key(KeyCode::Char('c')));
        assert!(!app.should_quit);
        app.handle_key(key(KeyCode::Char('a')));
        app.handle_key(key(KeyCode::Char('c')));
        assert!(!app.should_quit);
        assert!(matches!(&app.overlay, Some(Overlay::Form(form)) if form.title.value() == "c"));
    }

    #[test]
    fn ctrl_c_in_the_settings_panel_still_saves_the_settings() {
        let dir = files::test_dir("app-ctrl-c");
        std::fs::create_dir_all(&dir).expect("dir");
        let path = dir.join("config.toml");
        let mut app = App::new(
            Texts::en(),
            Vec::new(),
            None,
            Settings::default(),
            Some(path.clone()),
        );
        // 設定頁：第一列（週數）關掉，直接 Ctrl+C
        app.handle_key(key(KeyCode::Char(',')));
        app.handle_key(key(KeyCode::Char('l')));
        app.handle_key(KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL));
        assert!(app.should_quit);
        // `run` 在每個按鍵之後存：設定頁已經關了，所以存得進去
        app.save_settings().expect("save");
        let saved = config::load(&path).expect("load");
        assert!(!saved.calendar.week_numbers);
        std::fs::remove_dir_all(&dir).expect("clean up");
    }

    #[test]
    fn colors_from_the_config_file_are_on_screen_and_reset_removes_them() {
        let dir = files::test_dir("app-colors");
        std::fs::create_dir_all(&dir).expect("dir");
        let path = dir.join("config.toml");
        std::fs::write(&path, "[colors]\nsunday = \"#f38ba8\"\n").expect("write");
        let settings = config::load(&path).expect("load");
        let mut app = App::new(Texts::en(), Vec::new(), None, settings, Some(path.clone()));
        // 一打開，星期日就是設定檔裡的紅色
        assert_eq!(
            app.theme.sunday.fg,
            Some(ratatui::style::Color::Rgb(243, 139, 168))
        );
        // 設定頁：k 從第一列往上繞到最後一列（sunday），r 還原，關掉 → 存
        app.handle_key(key(KeyCode::Char(',')));
        app.handle_key(key(KeyCode::Char('k')));
        app.handle_key(key(KeyCode::Char('r')));
        app.handle_key(key(KeyCode::Char('q')));
        app.save_settings().expect("save");
        assert_eq!(app.theme.sunday, Theme::default().sunday);
        // 顏色都回到預設 → 整個 [colors] 拿掉
        assert_eq!(std::fs::read_to_string(&path).expect("read"), "");
        std::fs::remove_dir_all(&dir).expect("clean up");
    }

    #[test]
    fn shift_r_resets_everything_after_y_and_saves_the_defaults() {
        let dir = files::test_dir("app-reset-all");
        std::fs::create_dir_all(&dir).expect("dir");
        let path = dir.join("config.toml");
        let text = "[month]\n# 我的開關\ngrid_lines = true\n\n[colors]\nsunday = \"#f38ba8\"\n";
        std::fs::write(&path, text).expect("write");
        let settings = config::load(&path).expect("load");
        let mut app = App::new(Texts::en(), Vec::new(), None, settings, Some(path.clone()));
        app.handle_key(key(KeyCode::Char(',')));
        // R：先問，還沒還原
        app.handle_key(key(KeyCode::Char('R')));
        assert!(panel(&app).is_some_and(|panel| panel.confirm_reset_all));
        assert!(app.settings.month.grid_lines);
        // y：全部回到預設，畫面的顏色也是
        app.handle_key(key(KeyCode::Char('y')));
        assert_eq!(app.settings, Settings::default());
        assert_eq!(app.theme, Theme::default());
        assert!(panel(&app).is_some_and(|panel| !panel.confirm_reset_all));
        // 關掉設定頁 → 存：開關寫回預設值（註解留著），[colors] 整段拿掉
        app.handle_key(key(KeyCode::Char('q')));
        app.save_settings().expect("save");
        assert_eq!(
            std::fs::read_to_string(&path).expect("read"),
            "[month]\n# 我的開關\ngrid_lines = false\n"
        );
        std::fs::remove_dir_all(&dir).expect("clean up");
    }

    #[test]
    fn shift_r_is_cancelled_by_any_other_key() {
        let mut app = App::on_date(date(2026, 10, 10), Texts::en(), Vec::new());
        app.handle_key(key(KeyCode::Char(',')));
        app.handle_key(key(KeyCode::Char('l')));
        assert!(!app.settings.calendar.week_numbers);
        for other in [
            KeyCode::Char('n'),
            KeyCode::Esc,
            KeyCode::Char('j'),
            KeyCode::Char('l'),
        ] {
            app.handle_key(key(KeyCode::Char('R')));
            // 其他鍵只會取消：不還原、不關設定頁、不移動、不改值
            app.handle_key(key(other));
            assert_eq!(panel(&app), Some(Panel::default()), "{other:?}");
            assert!(!app.settings.calendar.week_numbers, "{other:?}");
        }
    }

    #[test]
    fn closing_the_settings_panel_saves_changed_toggles() {
        let dir = files::test_dir("app-save");
        let path = dir.join("config.toml");
        let mut app = App::on_date(date(2026, 10, 10), Texts::en(), Vec::new());
        app.config_path = Some(path.clone());
        // 打開設定頁、關掉週數（第一列）：設定頁還開著 → 還不存
        app.handle_key(key(KeyCode::Char(',')));
        app.handle_key(key(KeyCode::Char('l')));
        app.save_settings().expect("save");
        assert!(!path.exists());
        // 關掉設定頁 → 存
        app.handle_key(key(KeyCode::Char('q')));
        app.save_settings().expect("save");
        assert_eq!(config::load(&path).expect("load"), app.settings);
        assert!(!config::load(&path).expect("load").calendar.week_numbers);
        // 打開又關掉、沒改東西 → 不寫（刪掉檔案來確認沒有被重新建立）
        std::fs::remove_file(&path).expect("remove");
        app.handle_key(key(KeyCode::Char(',')));
        app.handle_key(key(KeyCode::Esc));
        app.save_settings().expect("save");
        assert!(!path.exists());
        std::fs::remove_dir_all(&dir).expect("clean up");
    }

    #[test]
    fn mouse_wheel_moves_selected_by_one_week() {
        let mut app = App::on_date(date(2026, 10, 10), Texts::en(), Vec::new());
        let wheel = |kind| MouseEvent {
            kind,
            column: 0,
            row: 0,
            modifiers: KeyModifiers::NONE,
        };
        app.handle_mouse(wheel(MouseEventKind::ScrollDown));
        assert_eq!(app.selected, date(2026, 10, 17));
        app.handle_mouse(wheel(MouseEventKind::ScrollUp));
        app.handle_mouse(wheel(MouseEventKind::ScrollUp));
        assert_eq!(app.selected, date(2026, 10, 3));
    }

    #[test]
    fn left_click_selects_the_clicked_date() {
        let mut app = App::on_date(date(2026, 10, 10), Texts::en(), Vec::new());
        let click = |column, row| MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column,
            row,
            modifiers: KeyModifiers::NONE,
        };
        // 第 4 週星期二 = 10/13
        app.handle_mouse(click(12, 6));
        assert_eq!(app.selected, date(2026, 10, 13));
        // 點在左邊欄：不動
        app.handle_mouse(click(4, 6));
        assert_eq!(app.selected, date(2026, 10, 13));
        // 點最上面那行（9/22）：選取後照 scrolloff 往上捲一行，選取變成在第 2 行
        app.handle_mouse(click(12, 3));
        assert_eq!(app.selected, date(2026, 9, 22));
        assert_eq!(selected_row(&app), Some(1));
    }

    /// 現在的 view 上，`day` 那一格裡的一個位置（整個畫面掃一遍，用畫面的 `date_at` 找）
    fn cell_of(app: &App, day: Date) -> (u16, u16) {
        (0..app.screen.height)
            .flat_map(|row| (0..app.screen.width).map(move |column| (column, row)))
            .find(|&(column, row)| {
                let at = match app.view {
                    View::Compact => compact::date_at(app, column, row),
                    View::Month => month::date_at(app, column, row),
                };
                at == Some(day)
            })
            .expect("day on screen")
    }

    /// 在 (`column`, `row`) 按一下左鍵
    fn left_click((column, row): (u16, u16)) -> MouseEvent {
        MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column,
            row,
            modifiers: KeyModifiers::NONE,
        }
    }

    /// 開著的是哪天的清單（沒開清單是 `None`）
    fn list_date(app: &App) -> Option<Date> {
        match app.overlay {
            Some(Overlay::DayList(list)) => Some(list.date),
            _ => None,
        }
    }

    /// 讓上一下點擊變成 1 秒前的事（下一下就不算點兩下）
    fn forget_the_last_click(app: &mut App) {
        app.last_click = app.last_click.map(|last| Click {
            at: last
                .at
                .checked_sub(Duration::from_secs(1))
                .expect("1 s ago"),
            ..last
        });
    }

    #[test]
    fn clicking_the_selected_day_or_double_clicking_a_day_opens_its_list() {
        for view in [View::Compact, View::Month] {
            let mut app = App::on_date(date(2026, 10, 10), Texts::en(), Vec::new());
            app.screen = Rect::new(0, 0, 80, 24);
            app.set_view(view);
            let (oct_13, oct_20) = (date(2026, 10, 13), date(2026, 10, 20));
            // 點 10/13：選它，還沒打開
            app.handle_mouse(left_click(cell_of(&app, oct_13)));
            assert_eq!((app.selected, list_date(&app)), (oct_13, None), "{view:?}");
            // 隔一陣子（不算點兩下）再點已經選到的 10/13：打開那天的清單
            forget_the_last_click(&mut app);
            app.handle_mouse(left_click(cell_of(&app, oct_13)));
            assert_eq!(list_date(&app), Some(oct_13), "{view:?}");
            // 關掉；很快地在 10/20 上點兩下：第一下選、第二下打開
            app.handle_key(key(KeyCode::Char('q')));
            let cell = cell_of(&app, oct_20);
            app.handle_mouse(left_click(cell));
            app.handle_mouse(left_click(cell));
            assert_eq!(list_date(&app), Some(oct_20), "{view:?}");
            // 在已經選到的 10/20 上點兩下：第一下就打開，第二下吞掉（不會被當成點在清單上或點到框外關掉）
            app.handle_key(key(KeyCode::Char('q')));
            let cell = cell_of(&app, oct_20);
            app.handle_mouse(left_click(cell));
            app.handle_mouse(left_click(cell));
            assert_eq!(list_date(&app), Some(oct_20), "{view:?}");
        }
    }

    #[test]
    fn double_click_opens_the_first_day_even_after_the_view_scrolled() {
        // 大月曆置中（預設）：點最上面那週的星期四，那週捲到中間，同一格底下變成別天
        // 第二下之前：`slow` = 隔了 1 秒、`key_between` = 中間按了一個沒有作用的鍵（z）
        let open = |slow: bool, key_between: bool| {
            let mut app = App::on_date(date(2026, 10, 10), Texts::en(), Vec::new());
            app.screen = Rect::new(0, 0, 80, 24);
            app.set_view(View::Month);
            let first = app.top.checked_add(3.days()).expect("Thursday");
            let cell = cell_of(&app, first);
            app.handle_mouse(left_click(cell));
            assert_eq!(app.selected, first);
            let under = month::date_at(&app, cell.0, cell.1).expect("a day");
            assert_ne!(under, first);
            if slow {
                forget_the_last_click(&mut app);
            }
            if key_between {
                app.handle_key(key(KeyCode::Char('z')));
            }
            app.handle_mouse(left_click(cell));
            (first, under, app.selected, list_date(&app))
        };
        // 很快點第二下：打開第一下那天（不是現在底下那天）
        let (first, _, selected, list) = open(false, false);
        assert_eq!((selected, list), (first, Some(first)));
        // 慢慢點第二下、或中間按了鍵：只是點了現在底下那天，選它、不打開
        for (slow, key_between) in [(true, false), (false, true)] {
            let (_, under, selected, list) = open(slow, key_between);
            assert_eq!((selected, list), (under, None), "{slow} {key_between}");
        }
    }

    #[test]
    fn left_click_selects_a_cell_in_the_month_view() {
        let mut app = month_view_without_centering(date(2026, 10, 10));
        app.screen = Rect::new(0, 0, 80, 24);
        // 下面的座標是有格線、沒有左邊欄（週數）的排版；有左邊欄的另外測
        app.settings.month.grid_lines = true;
        app.settings.calendar.week_numbers = false;
        let click = |column, row| MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column,
            row,
            modifiers: KeyModifiers::NONE,
        };
        // 80 × 24 有格線：星期列 y=1、格線 y=2、第 1 週 y=3～4、格線 y=5、第 2 週 y=6～7…
        // 第 2 週星期四（x 從 35 開始）= 10/1
        app.handle_mouse(click(36, 7));
        assert_eq!(app.selected, date(2026, 10, 1));
        // 點在格線上：不動
        app.handle_mouse(click(11, 7));
        assert_eq!(app.selected, date(2026, 10, 1));
        app.handle_mouse(click(36, 5));
        assert_eq!(app.selected, date(2026, 10, 1));
        // 點最上面那週（9/21 那列）：不置中就留在最上面那行，畫面不捲
        app.handle_mouse(click(2, 3));
        assert_eq!(app.selected, date(2026, 9, 21));
        assert_eq!(selected_row(&app), Some(0));
        // 置中開著：點第 2 週星期四（10/1）之後，那一格移到中間那行（第 3 行）
        app.settings.month.center_cursor = true;
        app.handle_mouse(click(36, 7));
        assert_eq!(app.selected, date(2026, 10, 1));
        assert_eq!(selected_row(&app), Some(2));
        assert_eq!(app.top, date(2026, 9, 14));
        // 設定頁開著：點在設定頁外面（這裡剛好是它的左框線，底下是 9/24）不會選到底下的格子
        app.overlay = Some(Overlay::Settings(Panel::default()));
        app.handle_mouse(click(36, 7));
        assert_eq!(app.selected, date(2026, 10, 1));
    }

    #[test]
    fn clicking_the_settings_panel_selects_a_row_then_changes_it() {
        use ratatui::{Terminal, backend::TestBackend};
        let mut app = App::on_date(date(2026, 10, 10), Texts::en(), Vec::new());
        app.screen = Rect::new(0, 0, 80, 30);
        app.handle_key(key(KeyCode::Char(',')));
        // 從畫面上找位置：[month] 下一行是 grid_lines（開關），[colors] 下一行是 item（顏色）
        let mut terminal = Terminal::new(TestBackend::new(80, 30)).unwrap();
        terminal.draw(|frame| ui::render(frame, &app)).unwrap();
        let screen = ui::rows(terminal.backend().buffer());
        let line_of = |text: &str| screen.iter().position(|line| line.contains(text)).unwrap();
        let (month, colors) = (line_of("[month]"), line_of("[colors]"));
        // 框線字元佔 3 個 byte，所以數字元（這裡每個字都佔 1 格）
        let line = &screen[month + 1];
        let x = u16::try_from(line[..line.find("grid_lines").unwrap()].chars().count()).unwrap();
        let at = |line: usize| MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: x,
            row: u16::try_from(line).unwrap(),
            modifiers: KeyModifiers::NONE,
        };
        // 點 grid_lines：先選到它，值不變；再點一次才切換
        app.handle_mouse(at(month + 1));
        assert_eq!(panel(&app).map(|panel| panel.selected), Some(1));
        assert!(!app.settings.month.grid_lines);
        app.handle_mouse(at(month + 1));
        assert!(app.settings.month.grid_lines);
        assert_eq!(panel(&app).map(|panel| panel.selected), Some(1));
        // 點段落名：不理
        app.handle_mouse(at(colors));
        assert_eq!(panel(&app).map(|panel| panel.selected), Some(1));
        // 顏色（item）：點一下選、再點一下換成下一個顏色（跟按 l 一樣：subtext1 → subtext0）
        app.handle_mouse(at(colors + 1));
        assert_eq!(app.theme.item, Theme::default().item);
        app.handle_mouse(at(colors + 1));
        assert_eq!(app.theme.item.fg, Some(crate::theme::PALETTE[16].1));
        // 正在問「全部還原嗎？」：點一下只是取消問題，不選、不改
        app.handle_key(key(KeyCode::Char('R')));
        app.handle_mouse(at(month + 1));
        assert!(panel(&app).is_some_and(|panel| !panel.confirm_reset_all && panel.selected == 4));
        assert!(app.settings.month.grid_lines);
        // 點框外 = 按 Esc：還在問「全部還原嗎？」時只取消問題，再點一次才關掉設定頁；
        // 點的地方是底下月曆的一格，但不會被選到
        let outside = MouseEvent {
            column: 8,
            row: 3,
            ..at(0)
        };
        assert!(compact::date_at(&app, 8, 3).is_some_and(|day| day != app.selected));
        app.handle_key(key(KeyCode::Char('R')));
        app.handle_mouse(outside);
        assert!(panel(&app).is_some_and(|panel| !panel.confirm_reset_all));
        app.handle_mouse(outside);
        assert_eq!(app.overlay, None);
        assert_eq!(app.selected, date(2026, 10, 10));
    }

    #[test]
    fn clicking_outside_any_overlay_works_like_esc() {
        let text = "- [ ] 早上 @2026-10-10 09:00\n";
        let (mut app, path) = app_with_task_file("outside", text);
        app.screen = Rect::new(0, 0, 80, 24);
        // 右上角：在每一種跳窗的框外面（說明頁在左上角、清單和表單在月曆框正中間、設定頁在右下角）
        let outside = MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: 79,
            row: 0,
            modifiers: KeyModifiers::NONE,
        };
        // 清單 → 表單（打了字）→ 點外面：表單關掉、字丟掉，回到清單
        app.handle_key(key(KeyCode::Char('a')));
        for c in "沒存".chars() {
            app.handle_key(key(KeyCode::Char(c)));
        }
        app.handle_mouse(outside);
        assert!(matches!(app.overlay, Some(Overlay::DayList(_))));
        // 清單 → 說明頁 → 點外面：回到清單；再點外面：清單也關掉
        app.handle_key(key(KeyCode::Char('?')));
        app.handle_mouse(outside);
        assert!(matches!(app.overlay, Some(Overlay::DayList(_))));
        app.handle_mouse(outside);
        assert_eq!(app.overlay, None);
        // 月曆上打開的說明頁、設定頁、表單：點外面都關掉，回到月曆
        for open in [KeyCode::Char('?'), KeyCode::Char(','), KeyCode::Char('a')] {
            app.handle_key(key(open));
            assert!(app.overlay.is_some(), "{open:?}");
            app.handle_mouse(outside);
            assert_eq!(app.overlay, None, "{open:?}");
        }
        // 點外面不會離開 app、不會改到任務檔
        assert!(!app.should_quit);
        assert_eq!(std::fs::read_to_string(&path).expect("read"), text);
        std::fs::remove_dir_all(path.parent().expect("dir")).expect("clean up");
    }

    #[test]
    fn moving_past_the_last_date_stays_put() {
        let mut app = App::on_date(Date::MAX, Texts::en(), Vec::new());
        app.move_selected(1);
        assert_eq!(app.selected, Date::MAX);
    }

    /// 10/10 有兩個項目、10/11 沒有（當天清單的 test 用）
    pub(super) fn app_with_two_items() -> App {
        let text = "- [ ] 早上 @2026-10-10 09:00\n- [ ] 下午 @2026-10-10 14:00";
        App::on_date(
            date(2026, 10, 10),
            Texts::en(),
            crate::tasks::parse(text).items,
        )
    }

    /// 寫一個暫存的任務檔、讀進 app，打開 10/10 的清單（打勾的 test 用）
    pub(super) fn app_with_task_file(name: &str, text: &str) -> (App, PathBuf) {
        let dir = files::test_dir(name);
        std::fs::create_dir_all(&dir).expect("dir");
        let path = dir.join("tasks.md");
        std::fs::write(&path, text).expect("write");
        let items = crate::tasks::parse(text).items;
        let mut app = App::on_date(date(2026, 10, 10), Texts::en(), items);
        app.tasks_path = Some(path.clone());
        app.handle_key(key(KeyCode::Enter));
        (app, path)
    }

    #[test]
    fn question_mark_opens_help_and_closing_it_goes_back() {
        let mut app = app_with_two_items();
        // 從月曆打開：?、q、Esc 都能關，關掉回到月曆（q 不會離開 app）
        for close in ['?', 'q']
            .map(KeyCode::Char)
            .into_iter()
            .chain([KeyCode::Esc])
        {
            app.handle_key(key(KeyCode::Char('?')));
            assert_eq!(
                app.overlay,
                Some(Overlay::Help {
                    back: None,
                    full: false,
                    scroll: 0
                })
            );
            app.handle_key(key(close));
            assert_eq!(app.overlay, None, "{close:?}");
            assert!(!app.should_quit);
        }
        // 從清單打開（選了第二個）：說明頁開著時，月曆、清單的鍵（j 只捲說明頁）、`,`、滾輪都不會動到底下；
        // 關掉回到同一個清單、同一個選取
        app.handle_key(key(KeyCode::Enter));
        app.handle_key(key(KeyCode::Char('j')));
        app.handle_key(key(KeyCode::Char('?')));
        for code in ['j', 'h', 'd', ','].map(KeyCode::Char) {
            app.handle_key(key(code));
        }
        app.handle_mouse(MouseEvent {
            kind: MouseEventKind::ScrollDown,
            column: 0,
            row: 0,
            modifiers: KeyModifiers::NONE,
        });
        assert!(matches!(
            app.overlay,
            Some(Overlay::Help { back: Some(_), .. })
        ));
        assert_eq!(app.selected, date(2026, 10, 10));
        app.handle_key(key(KeyCode::Char('q')));
        assert!(matches!(
            app.overlay,
            Some(Overlay::DayList(DayList { selected: 1, .. }))
        ));
    }

    #[test]
    fn j_k_and_the_wheel_scroll_the_help_only_when_it_does_not_fit() {
        let mut app = App::on_date(date(2026, 10, 10), Texts::en(), Vec::new());
        let scroll = |app: &App| match app.overlay {
            Some(Overlay::Help { scroll, .. }) => Some(scroll),
            _ => None,
        };
        // 放得下（100 × 40）：j 不會捲
        app.screen = Rect::new(0, 0, 100, 40);
        app.handle_key(key(KeyCode::Char('?')));
        app.handle_key(key(KeyCode::Char('j')));
        assert_eq!(scroll(&app), Some(0));
        // 只有 12 行高：精簡版 10 行內容，框裡放得下 12 - 4 = 8 行 → 最多捲 2 行，再按也停在 2
        app.screen = Rect::new(0, 0, 100, 12);
        for expected in [1, 2, 2] {
            app.handle_key(key(KeyCode::Char('j')));
            assert_eq!(scroll(&app), Some(expected));
        }
        // k / ↑ 往回捲，到最上面就停
        for code in [KeyCode::Up, KeyCode::Char('k'), KeyCode::Char('k')] {
            app.handle_key(key(code));
        }
        assert_eq!(scroll(&app), Some(0));
        // 滾輪跟 ↓ / ↑ 一樣；底下的月曆不動
        let wheel = |kind| MouseEvent {
            kind,
            column: 5,
            row: 5,
            modifiers: KeyModifiers::NONE,
        };
        app.handle_mouse(wheel(MouseEventKind::ScrollDown));
        assert_eq!(scroll(&app), Some(1));
        app.handle_mouse(wheel(MouseEventKind::ScrollUp));
        assert_eq!(scroll(&app), Some(0));
        assert_eq!(app.selected, date(2026, 10, 10));
        // Tab 換版：回到最上面
        app.handle_key(key(KeyCode::Down));
        app.handle_key(key(KeyCode::Tab));
        assert_eq!(scroll(&app), Some(0));
        // 捲到底之後 terminal 拉高到放得下：按一次 k 就在最上面（不用先按掉多捲的行數）
        app.handle_key(key(KeyCode::Tab));
        for _ in 0..2 {
            app.handle_key(key(KeyCode::Char('j')));
        }
        app.screen = Rect::new(0, 0, 100, 40);
        app.handle_key(key(KeyCode::Char('k')));
        assert_eq!(scroll(&app), Some(0));
    }

    #[test]
    fn tab_switches_the_help_between_short_and_full() {
        let mut app = app_with_two_items();
        let full = |app: &App| match app.overlay {
            Some(Overlay::Help { full, .. }) => Some(full),
            _ => None,
        };
        // 打開是精簡版；Tab 換完整版、再 Tab 換回來；Shift+Tab 也一樣
        app.handle_key(key(KeyCode::Char('?')));
        assert_eq!(full(&app), Some(false));
        app.handle_key(key(KeyCode::Tab));
        assert_eq!(full(&app), Some(true));
        app.handle_key(key(KeyCode::Tab));
        assert_eq!(full(&app), Some(false));
        app.handle_key(KeyEvent::new(KeyCode::BackTab, KeyModifiers::SHIFT));
        assert_eq!(full(&app), Some(true));
        // 關掉再打開：又是精簡版；從清單打開的，換版之後關掉還是回到清單
        app.handle_key(key(KeyCode::Esc));
        app.handle_key(key(KeyCode::Char('?')));
        assert_eq!(full(&app), Some(false));
        app.handle_key(key(KeyCode::Esc));
        app.handle_key(key(KeyCode::Enter));
        app.handle_key(key(KeyCode::Char('?')));
        app.handle_key(key(KeyCode::Tab));
        assert!(matches!(
            app.overlay,
            Some(Overlay::Help {
                back: Some(_),
                full: true,
                ..
            })
        ));
        app.handle_key(key(KeyCode::Char('q')));
        assert!(matches!(app.overlay, Some(Overlay::DayList(_))));
    }
}
