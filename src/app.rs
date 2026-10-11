//! App 的狀態、按鍵 / 滑鼠處理、捲動規則（畫面在 `ui`）

use crate::calendar::{monday_of, month_top};
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
    layout::Rect,
};

/// 選取的那週離上下邊緣至少保留幾週，不夠就捲動（像 vim 的 scrolloff）
pub const SCROLLOFF: i64 = 1;

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

    /// 另一種畫面
    fn toggled(self) -> Self {
        match self {
            Self::Compact => Self::Month,
            Self::Month => Self::Compact,
        }
    }
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
    /// 現在顯示哪一種畫面
    pub view: View,
    /// 執行時可以改的設定（格線開關…）
    pub settings: Settings,
    /// 設定頁；`Some` 表示打開中（Option+q）
    pub panel: Option<Panel>,
    /// terminal 現在的大小（每次畫畫面前更新）；滑鼠點擊要換算成哪一格時用
    pub screen: Rect,
}

impl App {
    /// 建立 app，今天用系統時間（本機時區）
    pub fn new(texts: Texts, items: Vec<Item>) -> Self {
        Self::on_date(Zoned::now().date(), texts, items)
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
            view: View::Compact,
            settings: Settings::default(),
            panel: None,
            screen: Rect::default(),
        };
        // 今天如果太靠近上下邊緣，先捲到保留 SCROLLOFF 的位置
        // （compact 8 週、SCROLLOFF = 1 時本月一定在第 2～7 行，不會捲；改了這兩個值才可能捲）
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
        }
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

    /// 處理一個按鍵
    fn handle_key(&mut self, key: KeyEvent) {
        // Option+q：設定頁開 / 關。macOS 的 terminal 預設把 Option+q 送成字元 `œ`；
        // 有設「Option 當 Alt」（例：Ghostty 的 macos-option-as-alt）就是 `q` + ALT
        let option_q = key.code == KeyCode::Char('œ')
            || (key.code == KeyCode::Char('q') && key.modifiers.contains(KeyModifiers::ALT));
        if option_q {
            self.panel = match self.panel {
                Some(_) => None,
                None => Some(Panel::default()),
            };
            return;
        }
        // 設定頁開著：按鍵都給它（j / k 選列、h / l 改值），月曆不動
        if let Some(mut panel) = self.panel {
            match key.code {
                // q / Esc：關閉設定頁（q 在這裡不是離開 app）
                KeyCode::Char('q') | KeyCode::Esc => self.panel = None,
                KeyCode::Char('j') | KeyCode::Down => {
                    panel.select(1);
                    self.panel = Some(panel);
                }
                KeyCode::Char('k') | KeyCode::Up => {
                    panel.select(-1);
                    self.panel = Some(panel);
                }
                KeyCode::Char('h') | KeyCode::Left => {
                    panel.row().adjust(-1, &mut self.settings, &mut self.theme);
                }
                KeyCode::Char('l') | KeyCode::Right | KeyCode::Char(' ') | KeyCode::Enter => {
                    panel.row().adjust(1, &mut self.settings, &mut self.theme);
                }
                // r：這一列還原成預設
                KeyCode::Char('r') => {
                    panel.row().reset(&mut self.settings, &mut self.theme);
                }
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
            // t：跳回今天
            KeyCode::Char('t') => self.jump_to_today(),
            // v：小月曆 ↔ 大月曆；週數變少時選取的那週可能掉出畫面，所以要重新對齊
            KeyCode::Char('v') => {
                self.view = self.view.toggled();
                self.scroll_to_selected();
            }
            // 其他按鍵不理
            _ => {}
        }
    }

    /// 處理一個滑鼠事件：滾輪滾一格 = 按一次 j / k；左鍵點日期 = 選取那天
    fn handle_mouse(&mut self, mouse: MouseEvent) {
        match mouse.kind {
            // 往下滾（看後面的日期）：下一週
            MouseEventKind::ScrollDown => self.move_selected(7),
            // 往上滾：上一週
            MouseEventKind::ScrollUp => self.move_selected(-7),
            // 左鍵按下：點在日期格子上才選取；點到最上 / 最下那行時照 SCROLLOFF 捲一行
            // （設定頁開著時不理，免得點到設定頁底下的格子）
            MouseEventKind::Down(MouseButton::Left) if self.panel.is_none() => {
                let date = match self.view {
                    View::Compact => compact::date_at(self.top, mouse.column, mouse.row),
                    View::Month => month::date_at(self, mouse.column, mouse.row),
                };
                if let Some(date) = date {
                    self.selected = date;
                    self.scroll_to_selected();
                }
            }
            // 其他（右鍵、拖曳、移動）不理
            _ => {}
        }
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

    /// 跳回今天，畫面也回到剛打開 app 的位置（本月的前一週在最上面），小 / 大月曆都一樣
    fn jump_to_today(&mut self) {
        self.selected = self.today;
        self.top = month_top(self.today);
        // 今天在畫面最上 / 最下那行時，照 SCROLLOFF 再捲一行
        self.scroll_to_selected();
    }

    /// 捲動月曆，讓選取的那週離上下邊緣至少 `SCROLLOFF` 週（畫面有幾週看現在的 view）
    ///
    /// 例：compact 8 週、`SCROLLOFF = 1` → 選取的那週只能在第 2～7 行（共 8 行）
    fn scroll_to_selected(&mut self) {
        let week = monday_of(self.selected);
        // 選取的那週最上面 / 最下面可以在哪一行，換算成那行的星期一
        let last_row = self.view.weeks() - 1 - SCROLLOFF;
        let min_week = self.top.saturating_add(SCROLLOFF.weeks());
        let max_week = self.top.saturating_add(last_row.weeks());
        if week < min_week {
            // 太靠上：往上捲，讓選取的那週剛好在第 SCROLLOFF + 1 行
            self.top = week.saturating_sub(SCROLLOFF.weeks());
        } else if week > max_week {
            // 太靠下：往下捲，讓選取的那週剛好在倒數第 SCROLLOFF + 1 行
            self.top = week.saturating_sub(last_row.weeks());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use jiff::civil::{Weekday, date};

    /// 沒有修飾鍵的按鍵事件（test 用）
    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    /// 選取的那週在畫面第幾行（0 = 最上面）；不在畫面上回傳 `None`
    fn selected_row(app: &App) -> Option<i64> {
        let week = monday_of(app.selected);
        (0..app.view.weeks()).find(|&row| app.top.saturating_add(row.weeks()) == week)
    }

    #[test]
    fn opening_screen_shows_week_before_the_1st_on_top() {
        let app = App::on_date(date(2026, 10, 10), Texts::en(), Vec::new());
        // 10/1 是星期四 → 那週的星期一是 9/28 → 前一週是 9/21
        assert_eq!(app.top, date(2026, 9, 21));
        assert_eq!(selected_row(&app), Some(2));
    }

    #[test]
    fn scrolling_keeps_selected_row_inside_scrolloff() {
        let starts = [
            date(2026, 10, 10),
            date(2026, 3, 1),
            date(2027, 2, 1),
            date(2028, 2, 29),
            date(2026, 12, 31),
        ];
        // 兩種畫面（8 週、6 週）都要守規則；h / l / k / j 各按 400 次（約 1 年 / 8 年）
        for view in [View::Compact, View::Month] {
            let min_row = SCROLLOFF;
            let max_row = view.weeks() - 1 - SCROLLOFF;
            for start in starts {
                for days in [-1, 1, -7, 7] {
                    let mut app = App::on_date(start, Texts::en(), Vec::new());
                    app.view = view;
                    app.scroll_to_selected();
                    let mut previous = selected_row(&app).expect("selected on screen");
                    for _ in 0..400 {
                        app.move_selected(days);
                        let row = selected_row(&app).expect("selected on screen");
                        assert!(
                            (min_row..=max_row).contains(&row),
                            "{view:?} {start} {days}: row {row}"
                        );
                        // 每按一次最多移 1 行，畫面不會跳
                        assert!(
                            (row - previous).abs() <= 1,
                            "{view:?} {start} {days}: jumped"
                        );
                        assert_eq!(app.top.weekday(), Weekday::Monday);
                        previous = row;
                    }
                }
            }
        }
    }

    #[test]
    fn jump_to_today_restores_the_opening_screen_in_both_views() {
        for view in [View::Compact, View::Month] {
            let mut app = App::on_date(date(2026, 10, 10), Texts::en(), Vec::new());
            app.view = view;
            // 捲了幾週（今天還在畫面上）和捲到一年後，按 t 都回到剛打開的畫面
            for weeks in [3, 52] {
                for _ in 0..weeks {
                    app.move_selected(7);
                }
                app.jump_to_today();
                assert_eq!(app.selected, date(2026, 10, 10), "{view:?} {weeks}");
                assert_eq!(app.top, date(2026, 9, 21), "{view:?} {weeks}");
            }
        }
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
    fn v_toggles_the_view_and_keeps_the_selection() {
        let mut app = App::on_date(date(2026, 10, 10), Texts::en(), Vec::new());
        app.move_selected(7);
        assert_eq!(app.view, View::Compact);
        app.handle_key(key(KeyCode::Char('v')));
        assert_eq!(app.view, View::Month);
        // 選取在第 4 行，6 週的畫面也放得下：選取和捲動位置都不動
        assert_eq!(app.selected, date(2026, 10, 17));
        assert_eq!(app.top, date(2026, 9, 21));
        app.handle_key(key(KeyCode::Char('v')));
        assert_eq!(app.view, View::Compact);
    }

    #[test]
    fn v_scrolls_when_the_selected_week_would_fall_off_the_smaller_view() {
        let mut app = App::on_date(date(2026, 10, 10), Texts::en(), Vec::new());
        // compact：往下 4 週到 11/7，選取在第 7 行（8 週的最後一行再往上 SCROLLOFF）
        for _ in 0..4 {
            app.move_selected(7);
        }
        assert_eq!(app.selected, date(2026, 11, 7));
        assert_eq!(selected_row(&app), Some(6));
        // 切到 6 週的大月曆：第 7 行不存在，往下捲到選取在第 5 行（倒數第 2 行）
        app.handle_key(key(KeyCode::Char('v')));
        assert_eq!(app.selected, date(2026, 11, 7));
        assert_eq!(selected_row(&app), Some(4));
        assert_eq!(app.top, date(2026, 10, 5));
    }

    #[test]
    fn option_q_opens_the_settings_panel_and_hjkl_edits_it() {
        let mut app = App::on_date(date(2026, 10, 10), Texts::en(), Vec::new());
        // Option+q（送成 œ）打開；第一列是 grid lines
        app.handle_key(key(KeyCode::Char('œ')));
        assert_eq!(app.panel, Some(Panel { selected: 0 }));
        // l：切換開關；h：再切回來
        app.handle_key(key(KeyCode::Char('l')));
        assert!(!app.settings.grid_lines);
        app.handle_key(key(KeyCode::Char('h')));
        assert!(app.settings.grid_lines);
        // j 兩次：跳過 item icons 到 item；l：顏色從 subtext1 換成下一個色 subtext0，馬上套到 theme
        app.handle_key(key(KeyCode::Char('j')));
        app.handle_key(key(KeyCode::Char('j')));
        assert_eq!(app.panel, Some(Panel { selected: 2 }));
        app.handle_key(key(KeyCode::Char('l')));
        assert_eq!(app.theme.item.fg, Some(crate::theme::PALETTE[16].1));
        // 開著的時候 h / j / k / l 不會動選取的日期
        assert_eq!(app.selected, date(2026, 10, 10));
        // r：只有選到的那列回到預設（item 回 subtext1，上面改過的 grid lines 不受影響）
        app.handle_key(key(KeyCode::Char('k')));
        app.handle_key(key(KeyCode::Char('k')));
        app.handle_key(key(KeyCode::Char('l')));
        assert!(!app.settings.grid_lines);
        app.handle_key(key(KeyCode::Char('j')));
        app.handle_key(key(KeyCode::Char('j')));
        app.handle_key(key(KeyCode::Char('r')));
        assert_eq!(app.theme.item, Theme::default().item);
        assert!(!app.settings.grid_lines);
        // q / Esc / q + ALT 都能關；關了之後 q 才是離開
        app.handle_key(key(KeyCode::Char('q')));
        assert_eq!(app.panel, None);
        assert!(!app.should_quit);
        app.handle_key(key(KeyCode::Char('œ')));
        app.handle_key(key(KeyCode::Esc));
        assert_eq!(app.panel, None);
        app.handle_key(key(KeyCode::Char('œ')));
        app.handle_key(KeyEvent::new(KeyCode::Char('q'), KeyModifiers::ALT));
        assert_eq!(app.panel, None);
        assert!(!app.should_quit);
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
        assert_eq!(selected_row(&app), Some(SCROLLOFF));
    }

    #[test]
    fn left_click_selects_a_cell_in_the_month_view() {
        let mut app = App::on_date(date(2026, 10, 10), Texts::en(), Vec::new());
        app.view = View::Month;
        app.screen = Rect::new(0, 0, 80, 24);
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
        // 點最上面那週（9/21 那列）：選取後照 scrolloff 往上捲一行
        app.handle_mouse(click(2, 3));
        assert_eq!(app.selected, date(2026, 9, 21));
        assert_eq!(selected_row(&app), Some(SCROLLOFF));
        // 設定頁開著：點擊不理
        app.panel = Some(Panel::default());
        app.handle_mouse(click(36, 7));
        assert_eq!(app.selected, date(2026, 9, 21));
    }

    #[test]
    fn moving_past_the_last_date_stays_put() {
        let mut app = App::on_date(Date::MAX, Texts::en(), Vec::new());
        app.move_selected(1);
        assert_eq!(app.selected, Date::MAX);
    }
}
