//! App 的狀態、按鍵 / 滑鼠處理、捲動規則（畫面在 `ui`）

use crate::calendar::{Scroll, monday_of, month_top, scrolled_top};
use crate::config;
use crate::files;
use crate::i18n::Texts;
use crate::settings::{Panel, Settings};
use crate::tasks::{self, Item, edit};
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
use std::path::{Path, PathBuf};

/// undo 最多記幾步
const UNDO_LEVELS: usize = 50;

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

/// 疊在月曆上面的跳窗；一次只會開一個，開著時按鍵都給它
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Overlay {
    /// 設定頁（Option+q）
    Settings(Panel),
    /// 當天清單（`Enter` / `o`）
    DayList(DayList),
    /// 說明頁（`?`）；`back` = 關掉之後回到哪裡（從當天清單打開的，就回到那個清單）
    Help { back: Option<DayList> },
}

/// 當天清單：哪一天、選到第幾個項目
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DayList {
    /// 清單是哪一天的（打開時選取的那天）
    pub date: Date,
    /// 選到的項目：`ui::items_on(items, date)` 的 index
    pub selected: usize,
    /// 按了 `d`，正在等確認：會刪掉幾筆（含子任務）；下一個鍵是 `y` 才刪，其他鍵都是取消
    pub confirm_delete: Option<usize>,
}

/// 下框線暫時換成的提示（寫檔、undo 的結果）；下一個按鍵就消失
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Notice {
    /// 檔案在別處被改過（例：剛在 Obsidian 改了）：沒寫，已重新載入
    Reloaded,
    /// 讀 / 寫任務檔失敗（例：沒有權限、檔案被刪了）：沒寫
    Failed(std::io::ErrorKind),
    /// undo 成功，帶著被復原的項目標題，例：`週會`
    Undone(String),
    /// 沒有可以 undo 的（還沒寫過檔，或已經退到底）
    NothingToUndo,
}

/// 一次寫檔的紀錄（undo 用）：寫之前、寫之後的整個檔案，和改到的項目標題（提示用）
#[derive(Debug)]
struct UndoStep {
    before: String,
    after: String,
    title: String,
}

impl DayList {
    /// 打開某一天的清單，選第一個項目
    fn open(date: Date) -> Self {
        Self {
            date,
            selected: 0,
            confirm_delete: None,
        }
    }

    /// 往下（`step` = +1）/ 往上（−1）選一個，頭尾相接；`count` = 那天有幾個項目，0 個就不動
    fn select(&mut self, step: isize, count: usize) {
        if let Some(index) = (self.selected as isize + step).checked_rem_euclid(count as isize) {
            self.selected = index as usize;
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
        // 上一次的提示：按任何鍵就消失
        self.notice = None;
        // Option+q：設定頁開 / 關。macOS 的 terminal 預設把 Option+q 送成字元 `œ`；
        // 有設「Option 當 Alt」（例：Ghostty 的 macos-option-as-alt）就是 `q` + ALT
        let option_q = key.code == KeyCode::Char('œ')
            || (key.code == KeyCode::Char('q') && key.modifiers.contains(KeyModifiers::ALT));
        if option_q {
            self.overlay = match self.overlay {
                Some(Overlay::Settings(_)) => None,
                None => Some(Overlay::Settings(Panel::default())),
                // 當天清單、說明頁開著時不理：一次只開一個跳窗，要先關掉它
                Some(Overlay::DayList(_) | Overlay::Help { .. }) => return,
            };
            return;
        }
        match self.overlay {
            Some(Overlay::Settings(panel)) => self.handle_settings_key(panel, key),
            Some(Overlay::DayList(list)) => self.handle_day_list_key(list, key),
            Some(Overlay::Help { back }) => self.handle_help_key(back, key),
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
            // q / Esc：關閉設定頁（q 在這裡不是離開 app）
            KeyCode::Char('q') | KeyCode::Esc => {
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

    /// 當天清單開著時的按鍵：j / k 選項目、Space / x 打勾、d 刪除、u 復原、q / Esc 關閉，月曆不動
    fn handle_day_list_key(&mut self, mut list: DayList, key: KeyEvent) {
        if let Some(count) = list.confirm_delete.take() {
            // 正在問「刪除嗎？」：只有 y 會刪，其他鍵都只是取消（按錯不會有事）
            if key.code == KeyCode::Char('y') {
                self.notice = self.delete_selected(list, count);
            }
        } else {
            let count = ui::items_on(&self.items, list.date).len();
            match key.code {
                // q / Esc：關閉清單（q 在這裡不是離開 app）
                KeyCode::Char('q') | KeyCode::Esc => {
                    self.overlay = None;
                    return;
                }
                KeyCode::Char('j') | KeyCode::Down => list.select(1, count),
                KeyCode::Char('k') | KeyCode::Up => list.select(-1, count),
                // Space / x：打勾 / 取消，寫回任務檔
                KeyCode::Char(' ') | KeyCode::Char('x') => {
                    self.notice = self.toggle_selected(list);
                }
                // d：先問一次（顯示會刪掉幾筆），按 y 才刪
                KeyCode::Char('d') => match self.ask_delete(list) {
                    Ok(count) => list.confirm_delete = count,
                    Err(notice) => self.notice = Some(notice),
                },
                // u：復原 roh-cal 最後一次寫檔
                KeyCode::Char('u') => self.notice = Some(self.undo()),
                // ?：說明頁，關掉之後回到這個清單
                KeyCode::Char('?') => {
                    self.overlay = Some(Overlay::Help { back: Some(list) });
                    return;
                }
                _ => {}
            }
        }
        // 重新載入後項目可能變少：選的不能超過最後一個
        let count = ui::items_on(&self.items, list.date).len();
        list.selected = list.selected.min(count.saturating_sub(1));
        self.overlay = Some(Overlay::DayList(list));
    }

    /// 清單選到的項目、任務檔的位置和檔案現在的內容（寫之前一定先重讀）。
    /// 沒有任務檔（test）或那天沒有項目是 `Ok(None)`；讀不到檔案是 `Err(提示)`
    fn selected_and_file(&self, list: DayList) -> Result<Option<(Item, PathBuf, String)>, Notice> {
        let Some(path) = self.tasks_path.clone() else {
            return Ok(None);
        };
        let Some(item) = ui::items_on(&self.items, list.date)
            .get(list.selected)
            .copied()
            .cloned()
        else {
            return Ok(None);
        };
        match std::fs::read_to_string(&path) {
            Ok(text) => Ok(Some((item, path, text))),
            Err(error) => Err(Notice::Failed(error.kind())),
        }
    }

    /// 當天清單選到的項目打勾 / 取消，寫回任務檔；寫成功（或沒事可做）回傳 `None`，沒寫成回傳要顯示的提示
    ///
    /// 那一行在別處被改過（`Changed`）就不寫，改成重新載入
    fn toggle_selected(&mut self, list: DayList) -> Option<Notice> {
        let (item, path, text) = match self.selected_and_file(list) {
            Ok(found) => found?,
            Err(notice) => return Some(notice),
        };
        match edit::toggle(&text, &item) {
            Ok(new_text) => self.write_tasks(&path, text, new_text, item.title),
            Err(edit::Changed) => Some(self.reload(&text)),
        }
    }

    /// `d`：先算一次刪掉選到的項目會刪掉幾筆（含子任務），回傳要問的筆數（沒事可做是 `None`）；
    /// 算不出來（檔案在別處改過、讀不到）回傳提示
    fn ask_delete(&mut self, list: DayList) -> Result<Option<usize>, Notice> {
        let Some((item, _, text)) = self.selected_and_file(list)? else {
            return Ok(None);
        };
        match edit::delete(&text, &item) {
            Ok((_, count)) => Ok(Some(count)),
            Err(edit::Changed) => Err(self.reload(&text)),
        }
    }

    /// `y`：刪掉選到的項目（連子任務），寫回任務檔；回傳值跟 `toggle_selected` 一樣
    ///
    /// `count` = 剛剛問使用者的筆數；現在算出來不一樣（例：剛好在別處加了子任務）就不刪，改成重新載入
    fn delete_selected(&mut self, list: DayList, count: usize) -> Option<Notice> {
        let (item, path, text) = match self.selected_and_file(list) {
            Ok(found) => found?,
            Err(notice) => return Some(notice),
        };
        match edit::delete(&text, &item) {
            Ok((new_text, deleted)) if deleted == count => {
                self.write_tasks(&path, text, new_text, item.title)
            }
            _ => Some(self.reload(&text)),
        }
    }

    /// 把 `after` 寫進任務檔，畫面的項目跟著更新，記一步 undo（`before` = 寫之前的內容、`title` = 改到的項目）。
    /// 寫成功回傳 `None`，失敗回傳提示
    fn write_tasks(
        &mut self,
        path: &Path,
        before: String,
        after: String,
        title: String,
    ) -> Option<Notice> {
        if let Err(error) = files::write_atomically(path, &after) {
            return Some(Notice::Failed(error.kind()));
        }
        self.items = tasks::parse(&after).items;
        self.undo.push(UndoStep {
            before,
            after,
            title,
        });
        // 只留最近的 UNDO_LEVELS 步
        if self.undo.len() > UNDO_LEVELS {
            self.undo.remove(0);
        }
        None
    }

    /// 檔案在別處被改過：照檔案現在的內容 `text` 重新載入；之前記的 undo 都對不上了，清掉
    fn reload(&mut self, text: &str) -> Notice {
        self.items = tasks::parse(text).items;
        self.undo.clear();
        Notice::Reloaded
    }

    /// 復原 roh-cal 最後一次寫檔（`u`），回傳要顯示的提示
    ///
    /// 檔案要跟 roh-cal 最後寫的一模一樣才復原；在別處改過就不動檔案、改成重新載入，別人的改動不會被蓋掉
    fn undo(&mut self) -> Notice {
        let Some(path) = self.tasks_path.clone() else {
            return Notice::NothingToUndo;
        };
        let Some(step) = self.undo.pop() else {
            return Notice::NothingToUndo;
        };
        let current = match std::fs::read_to_string(&path) {
            Ok(text) => text,
            Err(error) => {
                // 沒復原成：這一步放回去，之後還能再試
                self.undo.push(step);
                return Notice::Failed(error.kind());
            }
        };
        if current != step.after {
            return self.reload(&current);
        }
        if let Err(error) = files::write_atomically(&path, &step.before) {
            self.undo.push(step);
            return Notice::Failed(error.kind());
        }
        self.items = tasks::parse(&step.before).items;
        Notice::Undone(step.title)
    }

    /// 說明頁開著時的按鍵：`?` / `q` / `Esc` 關掉，回到打開前的地方（月曆或當天清單）；其他鍵不理
    fn handle_help_key(&mut self, back: Option<DayList>, key: KeyEvent) {
        if matches!(
            key.code,
            KeyCode::Char('?') | KeyCode::Char('q') | KeyCode::Esc
        ) {
            self.overlay = back.map(Overlay::DayList);
        }
    }

    /// 沒有跳窗時的按鍵：移動、跳回今天、切換畫面、打開當天清單、復原、說明、離開
    fn handle_calendar_key(&mut self, key: KeyEvent) {
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
            // Enter / o：打開選取那天的清單
            KeyCode::Enter | KeyCode::Char('o') => {
                self.overlay = Some(Overlay::DayList(DayList::open(self.selected)));
            }
            // u：復原 roh-cal 最後一次寫檔
            KeyCode::Char('u') => self.notice = Some(self.undo()),
            // ?：說明頁
            KeyCode::Char('?') => self.overlay = Some(Overlay::Help { back: None }),
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
        // 當天清單、說明頁開著時不理：清單是那一天的，底下的月曆動了會對不起來
        if matches!(
            self.overlay,
            Some(Overlay::DayList(_) | Overlay::Help { .. })
        ) {
            return;
        }
        match mouse.kind {
            // 往下滾（看後面的日期）：下一週
            MouseEventKind::ScrollDown => self.move_selected(7),
            // 往上滾：上一週
            MouseEventKind::ScrollUp => self.move_selected(-7),
            // 左鍵按下：點在日期格子上才選取；選完照捲動規則對齊（置中時那一格會移到中間）
            // （跳窗開著時不理，免得點到跳窗底下的格子）
            MouseEventKind::Down(MouseButton::Left) if self.overlay.is_none() => {
                let date = match self.view {
                    View::Compact => compact::date_at(self, mouse.column, mouse.row),
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

    /// 跳回今天，畫面也回到剛打開 app 的位置（本月的前一週在最上面；大月曆置中時今天那週在正中間）
    fn jump_to_today(&mut self) {
        self.selected = self.today;
        self.top = month_top(self.today);
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
    use jiff::civil::{Weekday, date};

    /// 設定頁開著就回傳它的狀態，沒開是 `None`（test 用）
    fn panel(app: &App) -> Option<Panel> {
        match app.overlay {
            Some(Overlay::Settings(panel)) => Some(panel),
            _ => None,
        }
    }

    /// 沒有修飾鍵的按鍵事件（test 用）
    fn key(code: KeyCode) -> KeyEvent {
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
        // 選取不動，大月曆置中：10/12 那週在 6 行的第 3 行 → 最上面 9/28
        assert_eq!(app.selected, date(2026, 10, 17));
        assert_eq!(app.top, date(2026, 9, 28));
        // 切回小月曆：10/12 那週在第 3 行，沒碰到邊界，畫面不動
        app.handle_key(key(KeyCode::Char('v')));
        assert_eq!(app.view, View::Compact);
        assert_eq!(app.top, date(2026, 9, 28));
    }

    #[test]
    fn v_scrolls_only_when_the_selected_week_would_fall_off_without_centering() {
        let mut app = App::on_date(date(2026, 10, 10), Texts::en(), Vec::new());
        app.settings.month.center_cursor = false;
        // compact：往下 4 週到 11/7，選取在第 7 行（上下各留 1 週的最後一行）
        for _ in 0..4 {
            app.move_selected(7);
        }
        assert_eq!(app.selected, date(2026, 11, 7));
        assert_eq!(selected_row(&app), Some(6));
        assert_eq!(app.top, date(2026, 9, 21));
        // 切到 6 週的大月曆（不置中）：第 7 行不存在，往下捲到選取在最後一行
        app.handle_key(key(KeyCode::Char('v')));
        assert_eq!(app.selected, date(2026, 11, 7));
        assert_eq!(selected_row(&app), Some(5));
        assert_eq!(app.top, date(2026, 9, 28));
    }

    #[test]
    fn option_q_opens_the_settings_panel_and_hjkl_edits_it() {
        let mut app = App::on_date(date(2026, 10, 10), Texts::en(), Vec::new());
        // Option+q（送成 œ）打開；第一列是 week_numbers
        app.handle_key(key(KeyCode::Char('œ')));
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
        // q / Esc / q + ALT 都能關；關了之後 q 才是離開
        app.handle_key(key(KeyCode::Char('q')));
        assert_eq!(app.overlay, None);
        assert!(!app.should_quit);
        app.handle_key(key(KeyCode::Char('œ')));
        app.handle_key(key(KeyCode::Esc));
        assert_eq!(app.overlay, None);
        app.handle_key(key(KeyCode::Char('œ')));
        app.handle_key(KeyEvent::new(KeyCode::Char('q'), KeyModifiers::ALT));
        assert_eq!(app.overlay, None);
        assert!(!app.should_quit);
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
        app.handle_key(key(KeyCode::Char('œ')));
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
        app.handle_key(key(KeyCode::Char('œ')));
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
        app.handle_key(key(KeyCode::Char('œ')));
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
        app.handle_key(key(KeyCode::Char('œ')));
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
        app.handle_key(key(KeyCode::Char('œ')));
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
        // 設定頁開著：點擊不理（點的位置現在是 9/24，選取不會變）
        app.overlay = Some(Overlay::Settings(Panel::default()));
        app.handle_mouse(click(36, 7));
        assert_eq!(app.selected, date(2026, 10, 1));
    }

    #[test]
    fn moving_past_the_last_date_stays_put() {
        let mut app = App::on_date(Date::MAX, Texts::en(), Vec::new());
        app.move_selected(1);
        assert_eq!(app.selected, Date::MAX);
    }

    /// 10/10 有兩個項目、10/11 沒有（當天清單的 test 用）
    fn app_with_two_items() -> App {
        let text = "- [ ] 早上 @2026-10-10 09:00\n- [ ] 下午 @2026-10-10 14:00";
        App::on_date(
            date(2026, 10, 10),
            Texts::en(),
            crate::tasks::parse(text).items,
        )
    }

    #[test]
    fn enter_or_o_opens_the_day_list_and_q_or_esc_closes_it() {
        let mut app = app_with_two_items();
        for open in [KeyCode::Enter, KeyCode::Char('o')] {
            for close in [KeyCode::Char('q'), KeyCode::Esc] {
                app.handle_key(key(open));
                assert_eq!(
                    app.overlay,
                    Some(Overlay::DayList(DayList {
                        date: date(2026, 10, 10),
                        selected: 0,
                        confirm_delete: None
                    }))
                );
                // 清單開著時 q 只關清單，不離開 app
                app.handle_key(key(close));
                assert_eq!(app.overlay, None);
                assert!(!app.should_quit);
            }
        }
    }

    #[test]
    fn day_list_j_k_wrap_around_and_the_calendar_stays_put() {
        let mut app = app_with_two_items();
        app.handle_key(key(KeyCode::Enter));
        let selected = |app: &App| match app.overlay {
            Some(Overlay::DayList(list)) => Some(list.selected),
            _ => None,
        };
        app.handle_key(key(KeyCode::Char('j')));
        assert_eq!(selected(&app), Some(1));
        // 兩個項目：再往下回到第一個，往上又到最後一個
        app.handle_key(key(KeyCode::Down));
        assert_eq!(selected(&app), Some(0));
        app.handle_key(key(KeyCode::Char('k')));
        assert_eq!(selected(&app), Some(1));
        // 月曆的鍵、Option+q、滑鼠都不理：日期、畫面、跳窗都不變
        for code in ['h', 'l', 't', 'v', 'œ'].map(KeyCode::Char) {
            app.handle_key(key(code));
        }
        app.handle_mouse(MouseEvent {
            kind: MouseEventKind::ScrollDown,
            column: 0,
            row: 0,
            modifiers: KeyModifiers::NONE,
        });
        assert_eq!(app.selected, date(2026, 10, 10));
        assert_eq!(app.view, View::Compact);
        assert_eq!(selected(&app), Some(1));
    }

    #[test]
    fn day_list_of_an_empty_day_ignores_j_k() {
        let mut app = app_with_two_items();
        app.handle_key(key(KeyCode::Char('l')));
        app.handle_key(key(KeyCode::Char('o')));
        // 10/11 沒有項目：j / k 不會出錯，選的還是 0
        app.handle_key(key(KeyCode::Char('j')));
        app.handle_key(key(KeyCode::Char('k')));
        assert_eq!(
            app.overlay,
            Some(Overlay::DayList(DayList {
                date: date(2026, 10, 11),
                selected: 0,
                confirm_delete: None
            }))
        );
    }

    /// 寫一個暫存的任務檔、讀進 app，打開 10/10 的清單（打勾的 test 用）
    fn app_with_task_file(name: &str, text: &str) -> (App, PathBuf) {
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
    fn space_or_x_checks_the_selected_item_off_in_the_file() {
        let text = "# 例行\n- [ ] 早上 @2026-10-10 09:00\n- 下午 @2026-10-10 14:00\n";
        let (mut app, path) = app_with_task_file("toggle", text);
        let read = || std::fs::read_to_string(&path).expect("read");
        // 選第二個（下午，沒寫 checkbox）按 Space：補上打勾的 checkbox，只改那一行
        app.handle_key(key(KeyCode::Char('j')));
        app.handle_key(key(KeyCode::Char(' ')));
        assert_eq!(
            read(),
            "# 例行\n- [ ] 早上 @2026-10-10 09:00\n- [x] 下午 @2026-10-10 14:00\n"
        );
        // 畫面用的項目也跟著更新；清單還開著、還選著同一個、沒有提示
        assert!(app.items[1].done);
        assert_eq!(app.notice, None);
        // x 也可以：再按一次就取消
        app.handle_key(key(KeyCode::Char('x')));
        assert_eq!(
            read(),
            "# 例行\n- [ ] 早上 @2026-10-10 09:00\n- [ ] 下午 @2026-10-10 14:00\n"
        );
        assert!(!app.items[1].done);
        std::fs::remove_dir_all(path.parent().expect("dir")).expect("clean up");
    }

    #[test]
    fn toggle_reloads_instead_of_writing_when_the_file_changed_elsewhere() {
        let (mut app, path) = app_with_task_file("changed", "- [ ] 早上 @2026-10-10 09:00\n");
        // 在別處（例：Obsidian）上面多加了一行
        let edited = "- [ ] 新的 @2026-10-10 08:00\n- [ ] 早上 @2026-10-10 09:00\n";
        std::fs::write(&path, edited).expect("edit");
        app.handle_key(key(KeyCode::Char(' ')));
        // 檔案不動、重新載入（現在有兩個項目）、提示出現；下一個按鍵提示就消失
        assert_eq!(std::fs::read_to_string(&path).expect("read"), edited);
        assert_eq!(app.items.len(), 2);
        assert_eq!(app.notice, Some(Notice::Reloaded));
        app.handle_key(key(KeyCode::Char('j')));
        assert_eq!(app.notice, None);
        std::fs::remove_dir_all(path.parent().expect("dir")).expect("clean up");
    }

    #[test]
    fn toggle_says_why_when_the_file_is_gone() {
        let (mut app, path) = app_with_task_file("gone", "- [ ] 早上 @2026-10-10 09:00\n");
        std::fs::remove_file(&path).expect("remove");
        app.handle_key(key(KeyCode::Char(' ')));
        assert_eq!(
            app.notice,
            Some(Notice::Failed(std::io::ErrorKind::NotFound))
        );
        // 沒有偷偷建一個新檔案
        assert!(!path.exists());
        std::fs::remove_dir_all(path.parent().expect("dir")).expect("clean up");
    }

    #[test]
    fn toggle_without_a_task_file_does_nothing() {
        let mut app = app_with_two_items();
        app.handle_key(key(KeyCode::Enter));
        app.handle_key(key(KeyCode::Char(' ')));
        assert!(!app.items[0].done);
        assert_eq!(app.notice, None);
        app.handle_key(key(KeyCode::Char('u')));
        assert_eq!(app.notice, Some(Notice::NothingToUndo));
    }

    #[test]
    fn u_undoes_the_last_check_and_then_has_nothing_left() {
        let text = "- [ ] 早上 @2026-10-10 09:00\n- 下午 @2026-10-10 14:00\n";
        let (mut app, path) = app_with_task_file("undo", text);
        let read = || std::fs::read_to_string(&path).expect("read");
        app.handle_key(key(KeyCode::Char('j')));
        app.handle_key(key(KeyCode::Char(' ')));
        assert_ne!(read(), text);
        // u：檔案（連沒寫 checkbox 的寫法）和畫面都回到打勾之前，提示被復原的項目
        app.handle_key(key(KeyCode::Char('u')));
        assert_eq!(read(), text);
        assert!(!app.items[1].done);
        assert_eq!(app.notice, Some(Notice::Undone("下午".into())));
        app.handle_key(key(KeyCode::Char('u')));
        assert_eq!(app.notice, Some(Notice::NothingToUndo));
        std::fs::remove_dir_all(path.parent().expect("dir")).expect("clean up");
    }

    #[test]
    fn u_on_the_calendar_steps_back_one_write_at_a_time() {
        let text = "- [ ] 早上 @2026-10-10 09:00\n- [ ] 下午 @2026-10-10 14:00\n";
        let (mut app, path) = app_with_task_file("undo-steps", text);
        let read = || std::fs::read_to_string(&path).expect("read");
        // 兩個都打勾，關掉清單
        for code in [' ', 'j', ' ', 'q'].map(KeyCode::Char) {
            app.handle_key(key(code));
        }
        assert_eq!(
            read(),
            "- [x] 早上 @2026-10-10 09:00\n- [x] 下午 @2026-10-10 14:00\n"
        );
        // 在月曆上按 u：先復原最後一次（下午），再復原早上
        app.handle_key(key(KeyCode::Char('u')));
        assert_eq!(app.overlay, None);
        assert_eq!(
            read(),
            "- [x] 早上 @2026-10-10 09:00\n- [ ] 下午 @2026-10-10 14:00\n"
        );
        app.handle_key(key(KeyCode::Char('u')));
        assert_eq!(read(), text);
        assert_eq!(app.notice, Some(Notice::Undone("早上".into())));
        std::fs::remove_dir_all(path.parent().expect("dir")).expect("clean up");
    }

    #[test]
    fn u_leaves_the_file_alone_when_it_changed_elsewhere() {
        let (mut app, path) = app_with_task_file("undo-changed", "- [ ] 早上 @2026-10-10 09:00\n");
        // 打勾再取消（記了兩步），之後在別處又改了檔案
        app.handle_key(key(KeyCode::Char(' ')));
        app.handle_key(key(KeyCode::Char(' ')));
        let edited = "- [ ] 早上 @2026-10-10 09:00\n- [ ] 新的 @2026-10-10 10:00\n";
        std::fs::write(&path, edited).expect("edit");
        app.handle_key(key(KeyCode::Char('u')));
        // 別人的改動留著，只重新載入；之前記的兩步都對不上了，全部清掉
        assert_eq!(std::fs::read_to_string(&path).expect("read"), edited);
        assert_eq!(app.items.len(), 2);
        assert_eq!(app.notice, Some(Notice::Reloaded));
        app.handle_key(key(KeyCode::Char('u')));
        assert_eq!(app.notice, Some(Notice::NothingToUndo));
        std::fs::remove_dir_all(path.parent().expect("dir")).expect("clean up");
    }

    #[test]
    fn undo_keeps_only_the_last_50_steps() {
        let (mut app, path) = app_with_task_file("undo-limit", "- [ ] 早上 @2026-10-10 09:00\n");
        // 打勾 / 取消 51 次（單數次 → 最後是打勾的）
        for _ in 0..=UNDO_LEVELS {
            app.handle_key(key(KeyCode::Char('x')));
        }
        // 一直按 u：只復原得了 50 次（多按幾次也不會超過）
        let undone = (0..UNDO_LEVELS + 5)
            .filter(|_| {
                app.handle_key(key(KeyCode::Char('u')));
                matches!(app.notice, Some(Notice::Undone(_)))
            })
            .count();
        assert_eq!(undone, UNDO_LEVELS);
        // 最早的那一步掉了：檔案停在第一次打勾之後
        assert_eq!(
            std::fs::read_to_string(&path).expect("read"),
            "- [x] 早上 @2026-10-10 09:00\n"
        );
        std::fs::remove_dir_all(path.parent().expect("dir")).expect("clean up");
    }

    /// 10/10 早上有一個子任務（沒日期，不在清單上）、下午沒有（刪除的 test 用）
    const WITH_SUBTASK: &str =
        "- [ ] 早上 @2026-10-10 09:00\n  - [ ] 準備 slides\n- [ ] 下午 @2026-10-10 14:00\n";

    /// 清單正在問的刪除筆數（test 用）
    fn confirm_delete(app: &App) -> Option<usize> {
        match app.overlay {
            Some(Overlay::DayList(list)) => list.confirm_delete,
            _ => None,
        }
    }

    #[test]
    fn d_then_y_deletes_with_subtasks_and_u_brings_them_back() {
        let (mut app, path) = app_with_task_file("delete", WITH_SUBTASK);
        let read = || std::fs::read_to_string(&path).expect("read");
        // d：先問，檔案還沒動；早上 + 子任務 = 2 筆
        app.handle_key(key(KeyCode::Char('d')));
        assert_eq!(confirm_delete(&app), Some(2));
        assert_eq!(read(), WITH_SUBTASK);
        // y：刪掉早上和它的子任務，只剩下午
        app.handle_key(key(KeyCode::Char('y')));
        assert_eq!(read(), "- [ ] 下午 @2026-10-10 14:00\n");
        assert_eq!(app.items.len(), 1);
        assert_eq!(confirm_delete(&app), None);
        // u：整段救回來
        app.handle_key(key(KeyCode::Char('u')));
        assert_eq!(read(), WITH_SUBTASK);
        assert_eq!(app.notice, Some(Notice::Undone("早上".into())));
        std::fs::remove_dir_all(path.parent().expect("dir")).expect("clean up");
    }

    #[test]
    fn any_key_other_than_y_cancels_the_delete() {
        let (mut app, path) = app_with_task_file("delete-cancel", WITH_SUBTASK);
        for other in [KeyCode::Char('j'), KeyCode::Char('n'), KeyCode::Esc] {
            app.handle_key(key(KeyCode::Char('d')));
            // 取消用的那個鍵只拿來取消：清單還開著、選的還是第一個
            app.handle_key(key(other));
            assert_eq!(confirm_delete(&app), None, "{other:?}");
            assert!(matches!(
                app.overlay,
                Some(Overlay::DayList(DayList { selected: 0, .. }))
            ));
        }
        assert_eq!(std::fs::read_to_string(&path).expect("read"), WITH_SUBTASK);
        std::fs::remove_dir_all(path.parent().expect("dir")).expect("clean up");
    }

    #[test]
    fn y_does_not_delete_when_the_subtasks_changed_meanwhile() {
        let (mut app, path) = app_with_task_file("delete-changed", WITH_SUBTASK);
        app.handle_key(key(KeyCode::Char('d')));
        assert_eq!(confirm_delete(&app), Some(2));
        // 問的時候是 2 筆；按 y 之前，在別處多加了一個子任務 → 會刪 3 筆，跟問的不一樣
        let edited = "- [ ] 早上 @2026-10-10 09:00\n  - [ ] 準備 slides\n  - [ ] 印講義\n- [ ] 下午 @2026-10-10 14:00\n";
        std::fs::write(&path, edited).expect("edit");
        app.handle_key(key(KeyCode::Char('y')));
        assert_eq!(std::fs::read_to_string(&path).expect("read"), edited);
        assert_eq!(app.notice, Some(Notice::Reloaded));
        std::fs::remove_dir_all(path.parent().expect("dir")).expect("clean up");
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
            assert_eq!(app.overlay, Some(Overlay::Help { back: None }));
            app.handle_key(key(close));
            assert_eq!(app.overlay, None, "{close:?}");
            assert!(!app.should_quit);
        }
        // 從清單打開（選了第二個）：說明頁開著時其他鍵、Option+q 都不理；關掉回到同一個清單、同一個選取
        app.handle_key(key(KeyCode::Enter));
        app.handle_key(key(KeyCode::Char('j')));
        app.handle_key(key(KeyCode::Char('?')));
        for code in ['j', 'h', 'd', 'œ'].map(KeyCode::Char) {
            app.handle_key(key(code));
        }
        app.handle_mouse(MouseEvent {
            kind: MouseEventKind::ScrollDown,
            column: 0,
            row: 0,
            modifiers: KeyModifiers::NONE,
        });
        assert!(matches!(app.overlay, Some(Overlay::Help { back: Some(_) })));
        assert_eq!(app.selected, date(2026, 10, 10));
        app.handle_key(key(KeyCode::Char('q')));
        assert!(matches!(
            app.overlay,
            Some(Overlay::DayList(DayList { selected: 1, .. }))
        ));
    }
}
