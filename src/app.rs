//! 程式的狀態與主迴圈：畫畫面 → 等輸入 → 改狀態，一直重複到離開

mod write;

pub use write::Notice;

use crate::{
    tasks::Item,
    texts::Lang,
    theme::Theme,
    ui::{self, Click},
};
use color_eyre::Result;
use jiff::{ToSpan, civil::Date};
use ratatui::{
    DefaultTerminal,
    crossterm::event::{
        self, KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
    },
    layout::Rect,
};
use std::path::PathBuf;

/// 鍵盤在哪個框
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Focus {
    /// 左邊的月曆：`hjkl` 移動選取的日期
    Calendar,
    /// 右邊的事項：`j` / `k` 一件一件選
    Agenda,
    /// 最左邊的 side menu（夠寬才有）：`j` / `k` 選日曆、`Space` 勾選
    Side,
}

/// 用哪個版型（數字鍵切換）
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum View {
    /// 版型 1（`1`）：左邊月曆、右邊每個月的事跟月份對齊
    Planner,
    /// 版型 3（`3`）：大月曆，一次 6 週，格子裡列那天的事
    Month,
}

/// side menu 上的一個日曆（= 任務檔的一個分組）
#[derive(Debug)]
pub struct Group {
    /// 分組名稱；`None` = 檔案開頭、還沒有標題的項目
    pub name: Option<String>,
    /// 有沒有勾選（剛打開時全部勾選）
    pub shown: bool,
}

impl Group {
    /// 任務檔裡的所有分組，照第一次出現的順序；名稱一樣的標題算同一個
    fn list(items: &[Item]) -> Vec<Self> {
        let mut groups: Vec<Self> = Vec::new();
        for item in items {
            let name = item.group.as_ref().map(|heading| heading.name.clone());
            if !groups.iter().any(|group| group.name == name) {
                groups.push(Self { name, shown: true });
            }
        }
        groups
    }

    /// `item` 是不是這個分組的
    pub fn includes(&self, item: &Item) -> bool {
        self.name.as_deref() == item.group.as_ref().map(|heading| heading.name.as_str())
    }
}

/// 右邊列出來的一行：哪件事、列在哪天（`App::agenda()` 排好的順序就是右邊由上到下的順序）
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Entry<'a> {
    /// 哪件事
    pub item: &'a Item,
    /// 列在哪天：一般是那件事的日子；跨多天又跨月的事，之後的月列在 1 號
    pub date: Date,
}

impl Entry<'_> {
    /// 這一行算不算 `day` 那天的事：那件事蓋到 `day`（跨多天的中間每一天都算），而且這一行列在 `day` 那個月
    /// （跨月的事每個月各一行，只算那個月的那一行）
    pub fn is_on(&self, day: Date) -> bool {
        self.item.covers(day) && self.date.first_of_month() == day.first_of_month()
    }
}

/// 整個程式的狀態
#[derive(Debug)]
pub struct App {
    /// 畫面的語言
    pub lang: Lang,
    /// 今天（打開程式那天；test 可以指定任何一天）
    pub today: Date,
    /// 選取的日期：月曆上加括號 `[17]` 的那天
    pub selected: Date,
    /// 任務檔裡的所有項目
    pub items: Vec<Item>,
    /// side menu 上的日曆，照任務檔裡出現的順序
    pub groups: Vec<Group>,
    /// 按 `s` 收起 side menu
    pub side_hidden: bool,
    /// side menu 選到第幾個日曆（`groups` 裡的位置）；鍵盤在 side menu 時才用
    pub group_index: usize,
    /// 鍵盤在哪個框
    pub focus: Focus,
    /// 用哪個版型
    pub view: View,
    /// 右邊選到第幾件事（`agenda()` 裡的位置）；鍵盤在右邊時才用
    pub agenda_index: usize,
    /// 月曆捲到哪：畫面最上面是第幾列（`None` = 下一次畫的時候把選取那天的那個月放在最上面）
    pub calendar_top: Option<i64>,
    /// 右邊捲到哪：(哪個月的 1 號, 那個月右邊最上面是排好的第幾列)；事太多、那個月放不下時才會捲
    pub agenda_window: Option<(Date, usize)>,
    /// 版型 3 捲到哪：畫面最上面那週的星期一（`None` = 下一次畫的時候把選取那個月 1 號的前一週放在最上面）
    pub month_top: Option<Date>,
    /// 畫面的顏色
    pub theme: Theme,
    /// terminal 的大小（每一輪畫之前更新），滑鼠點的位置要用它換算
    pub screen: Rect,
    /// 任務檔的位置（打勾、編輯要寫回去）；`None` = 沒有任務檔（test），不寫
    pub tasks_path: Option<PathBuf>,
    /// 最下面一列暫時換成的提示（寫檔失敗、重新載入）；下一個按鍵、點擊就消失
    pub notice: Option<Notice>,
    /// `true` = 這一輪畫完就結束主迴圈
    should_quit: bool,
}

impl App {
    /// 剛打開時的狀態：鍵盤在月曆、選取今天、theme 是 Catppuccin Mocha
    pub fn new(lang: Lang, today: Date, items: Vec<Item>) -> Self {
        Self {
            lang,
            today,
            selected: today,
            groups: Group::list(&items),
            side_hidden: false,
            group_index: 0,
            items,
            focus: Focus::Calendar,
            view: View::Planner,
            agenda_index: 0,
            calendar_top: None,
            agenda_window: None,
            month_top: None,
            theme: Theme::catppuccin_mocha(),
            screen: Rect::default(),
            tasks_path: None,
            notice: None,
            should_quit: false,
        }
    }

    /// 這件事要不要顯示：它的日曆（分組）有勾選才顯示
    pub fn is_shown(&self, item: &Item) -> bool {
        self.groups
            .iter()
            .find(|group| group.includes(item))
            .is_none_or(|group| group.shown)
    }

    /// `day` 是不是假日：假日日曆（標題寫了 `type:holiday`）裡有勾選的事蓋到那天
    pub fn is_holiday(&self, day: Date) -> bool {
        self.items.iter().any(|item| {
            item.group.as_ref().is_some_and(|group| group.holiday)
                && self.is_shown(item)
                && item.covers(day)
        })
    }

    /// 右邊列出來的事：有日期、日曆有勾選的，照列在哪天、時間排好（同一天沒有時間的在前；日期、時間一樣的照檔案裡的順序）
    ///
    /// 跨多天又跨月的事，碰到的每個月都列一次：第一個月列在開始那天，之後的月列在 1 號、排在那個月最上面。
    /// 沒有日期的事先不列（月曆上沒有位置放）
    pub fn agenda(&self) -> Vec<Entry<'_>> {
        let mut entries = Vec::new();
        for item in self.items.iter().filter(|item| self.is_shown(item)) {
            let (Some(first), Some(last)) = (item.date, item.last_day()) else {
                continue;
            };
            entries.push(Entry { item, date: first });
            // 之後碰到的每個月（1 號）
            let months = first
                .first_of_month()
                .series(1.month())
                .skip(1)
                .take_while(|&month| month <= last);
            entries.extend(months.map(|date| Entry { item, date }));
        }
        // sort_by_key 是 stable sort：key 一樣的保持原本的順序；同一天裡，從上個月接過來的排最前面，
        // 再來是沒有時間的（`None` 排在 `Some` 前面）
        entries.sort_by_key(|entry| {
            let continued = Some(entry.date) != entry.item.date;
            (entry.date, !continued, entry.item.time)
        });
        entries
    }

    /// `day` 那天的事（日曆有勾選的；跨多天的蓋到那天就算）：比較早開始的在前，同一天開始的照時間
    /// （沒有時間的在前；時間一樣的照檔案裡的順序）；版型 3 的格子用
    pub fn items_on(&self, day: Date) -> Vec<&Item> {
        let mut items: Vec<&Item> = self
            .items
            .iter()
            .filter(|item| item.covers(day) && self.is_shown(item))
            .collect();
        items.sort_by_key(|item| (item.date, item.time));
        items
    }

    /// 鍵盤在右邊時選到的那一行；鍵盤在月曆時是 `None`
    pub fn chosen(&self) -> Option<Entry<'_>> {
        match self.focus {
            Focus::Agenda => self.agenda().get(self.agenda_index).copied(),
            Focus::Calendar | Focus::Side => None,
        }
    }

    /// 這一次有沒有畫 side menu：terminal 夠寬、沒按 `s` 收起
    fn side_shown(&self) -> bool {
        ui::side_fits(self.screen) && !self.side_hidden
    }

    /// 記下 terminal 的大小；terminal 變窄、side menu 不見了，鍵盤在上面的話回到月曆
    fn resize(&mut self, screen: Rect) {
        self.screen = screen;
        if self.focus == Focus::Side && !self.side_shown() {
            self.focus = Focus::Calendar;
        }
    }

    /// 主迴圈：每一輪畫一次畫面、等一個輸入，直到要離開
    pub fn run(mut self, terminal: &mut DefaultTerminal) -> Result<()> {
        while !self.should_quit {
            // 先記下 terminal 的大小（視窗拉大縮小也會跟著變），畫面和滑鼠換算位置才一致
            let size = terminal.size()?;
            self.resize(Rect::new(0, 0, size.width, size.height));
            terminal.draw(|frame| ui::render(frame, &mut self))?;
            // event::read() 會 block 到有輸入為止；按鍵只處理按下（有的 terminal 放開也會送一次）
            let event = event::read()?;
            if let Some(key) = event.as_key_press_event() {
                self.handle_key(key);
            } else if let Some(mouse) = event.as_mouse_event() {
                self.handle_mouse(mouse);
            }
        }
        Ok(())
    }

    /// 按鍵：`Tab` 換框、`[` / `]` 換月、`t` 回今天、`1` / `3` 換版型、`s` 收起 / 打開 side menu、
    /// `q` / `Ctrl+C` 離開、`Esc` 回月曆（已經在月曆就離開）；其他鍵看鍵盤在哪個框
    fn handle_key(&mut self, key: KeyEvent) {
        // 上一次的提示看過了：按任何鍵就消失
        self.notice = None;
        match key.code {
            KeyCode::Char('q') => self.should_quit = true,
            // raw mode 下 Ctrl+C 不會送 SIGINT 讓程式結束，要自己處理
            KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.should_quit = true;
            }
            KeyCode::Esc if self.focus != Focus::Calendar => self.focus = Focus::Calendar,
            KeyCode::Esc => self.should_quit = true,
            KeyCode::Tab => self.next_focus(),
            // 回今天，並把今天那個月捲到最上面
            KeyCode::Char('t') => {
                self.selected = self.today;
                self.scroll_to_month();
                self.focus = Focus::Calendar;
            }
            KeyCode::Char('1') => self.set_view(View::Planner),
            KeyCode::Char('3') => self.set_view(View::Month),
            // 上個月 / 下個月的 1 號，並把那個月捲到最上面
            KeyCode::Char('[') => self.jump_month(-1),
            KeyCode::Char(']') => self.jump_month(1),
            // terminal 不夠寬時本來就沒有 side menu，按了不算；收起時鍵盤在上面的話回到月曆
            KeyCode::Char('s') if ui::side_fits(self.screen) => {
                self.side_hidden = !self.side_hidden;
                if self.focus == Focus::Side {
                    self.focus = Focus::Calendar;
                }
            }
            _ => match self.focus {
                Focus::Calendar => self.handle_calendar_key(key.code),
                Focus::Agenda => self.handle_agenda_key(key.code),
                Focus::Side => self.handle_side_key(key.code),
            },
        }
    }

    /// 鍵盤在月曆：左右 = 前一天 / 後一天，上下 = 上一週 / 下一週（像 vim）
    fn handle_calendar_key(&mut self, code: KeyCode) {
        match code {
            KeyCode::Char('h') | KeyCode::Left => self.move_by(-1),
            KeyCode::Char('l') | KeyCode::Right => self.move_by(1),
            KeyCode::Char('k') | KeyCode::Up => self.move_by(-7),
            KeyCode::Char('j') | KeyCode::Down => self.move_by(7),
            _ => {}
        }
    }

    /// 鍵盤在右邊：上下一件事，月曆選取的日期跟著那件事走；`Space` / `x` 打勾（寫回任務檔）
    fn handle_agenda_key(&mut self, code: KeyCode) {
        match code {
            KeyCode::Char('j') | KeyCode::Down => self.choose_next(1),
            KeyCode::Char('k') | KeyCode::Up => self.choose_next(-1),
            KeyCode::Char(' ' | 'x') => self.toggle_chosen(),
            _ => {}
        }
    }

    /// 鍵盤在 side menu：上下選日曆，`Space` / `Enter` 勾選或取消勾選（沒勾的日曆，它的事不顯示）
    fn handle_side_key(&mut self, code: KeyCode) {
        match code {
            KeyCode::Char('j') | KeyCode::Down => self.choose_group(1),
            KeyCode::Char('k') | KeyCode::Up => self.choose_group(-1),
            KeyCode::Char(' ') | KeyCode::Enter => self.toggle_group(),
            _ => {}
        }
    }

    /// side menu 選到的那個日曆：勾選 ⇄ 取消勾選
    fn toggle_group(&mut self) {
        if let Some(group) = self.groups.get_mut(self.group_index) {
            group.shown = !group.shown;
        }
    }

    /// 滑鼠：滾輪上下（月曆移一週、右邊移一件事、side menu 移一個日曆）；左鍵點月曆上的日期 = 選那天、點右邊的事 = 選那件、
    /// 點 side menu 的日曆 = 選那個（再點一次已經選到的 = 勾選 / 取消勾選）
    fn handle_mouse(&mut self, mouse: MouseEvent) {
        // 上一次的提示看過了：點擊、滾輪也算
        if !matches!(mouse.kind, MouseEventKind::Moved) {
            self.notice = None;
        }
        match mouse.kind {
            // 往下滾（看後面的）、往上滾
            MouseEventKind::ScrollDown => self.scroll(1),
            MouseEventKind::ScrollUp => self.scroll(-1),
            // 點在日期或事上才選；點在表頭、線、空格上不理
            MouseEventKind::Down(MouseButton::Left) => {
                match ui::click_at(self, mouse.column, mouse.row) {
                    Some(Click::Day(date)) => {
                        self.selected = date;
                        self.focus = Focus::Calendar;
                    }
                    // 再點一次已經選到的那件 = 打勾
                    Some(Click::Item(index))
                        if self.focus == Focus::Agenda && self.agenda_index == index =>
                    {
                        self.toggle_chosen();
                    }
                    Some(Click::Item(index)) => self.choose(index),
                    Some(Click::Group(index)) => self.click_group(index),
                    None => {}
                }
            }
            // 其他（右鍵、拖曳、移動）不理
            _ => {}
        }
    }

    /// 滾輪往下（`step` = 1）/ 往上（-1）：鍵盤在月曆就移一週，在右邊就移一件事，在 side menu 就移一個日曆
    fn scroll(&mut self, step: i8) {
        match self.focus {
            Focus::Calendar => self.move_by(7 * i64::from(step)),
            Focus::Agenda => self.choose_next(isize::from(step)),
            Focus::Side => self.choose_group(isize::from(step)),
        }
    }

    /// 換版型：選取那天的那個月捲到最上面；版型 3 沒有右邊的事項框，鍵盤在那裡的話回到月曆
    fn set_view(&mut self, view: View) {
        self.view = view;
        self.scroll_to_month();
        if view == View::Month && self.focus == Focus::Agenda {
            self.focus = Focus::Calendar;
        }
    }

    /// 下一次畫的時候，把選取那天的那個月捲到最上面（兩個版型都是）
    fn scroll_to_month(&mut self) {
        self.calendar_top = None;
        self.month_top = None;
    }

    /// `Tab`：月曆 → 右邊的事項（一件事都沒有、或是版型 3 就跳過）→ side menu（有畫才去）→ 月曆
    fn next_focus(&mut self) {
        match self.focus {
            Focus::Calendar => {
                if self.view == View::Planner {
                    self.enter_agenda();
                }
                if self.focus == Focus::Calendar && self.side_shown() {
                    self.focus = Focus::Side;
                }
            }
            Focus::Agenda if self.side_shown() => self.focus = Focus::Side,
            Focus::Agenda | Focus::Side => self.focus = Focus::Calendar,
        }
    }

    /// 點 side menu 上第 `index` 個日曆：點一下選它（鍵盤換到 side menu），再點一次已經選到的 = 勾選 / 取消勾選
    fn click_group(&mut self, index: usize) {
        if self.focus == Focus::Side && self.group_index == index {
            self.toggle_group();
        } else {
            self.focus = Focus::Side;
            self.group_index = index;
        }
    }

    /// side menu 選往後（負的 = 往前）第 `step` 個日曆；到第一個 / 最後一個就停住
    fn choose_group(&mut self, step: isize) {
        let last = self.groups.len().saturating_sub(1);
        self.group_index = self.group_index.saturating_add_signed(step).min(last);
    }

    /// 鍵盤換到右邊：選「選取那天的事」或「之後」的第一件（跨多天、比較早開始但蓋到選取那天的，排在前面所以先選它）；
    /// 之後都沒事就選最後一件；一件事都沒有就不換
    fn enter_agenda(&mut self) {
        let index = {
            let agenda = self.agenda();
            let Some(last) = agenda.len().checked_sub(1) else {
                return;
            };
            agenda
                .iter()
                .position(|entry| entry.date >= self.selected || entry.is_on(self.selected))
                .unwrap_or(last)
        };
        self.choose(index);
    }

    /// 選右邊第 `index` 件事：鍵盤換到右邊，月曆選取那件事列在的那天；
    /// 選取的日期本來就是那件事的日子（跨多天的中間某天）就不動
    fn choose(&mut self, index: usize) {
        let Some((date, on_selected)) = self
            .agenda()
            .get(index)
            .map(|entry| (entry.date, entry.is_on(self.selected)))
        else {
            return;
        };
        self.agenda_index = index;
        if !on_selected {
            self.selected = date;
        }
        self.focus = Focus::Agenda;
    }

    /// 選往後（負的 = 往前）第 `step` 件事；到第一件 / 最後一件就停住
    fn choose_next(&mut self, step: isize) {
        let last = self.agenda().len().saturating_sub(1);
        self.choose(self.agenda_index.saturating_add_signed(step).min(last));
    }

    /// 跳到往後（負的 = 往前）第 `months` 個月的 1 號，那個月捲到最上面；鍵盤回到月曆
    fn jump_month(&mut self, months: i64) {
        self.selected = self
            .selected
            .first_of_month()
            .saturating_add(months.months());
        self.scroll_to_month();
        self.focus = Focus::Calendar;
    }

    /// 選取的日期往後（負的 = 往前）移幾天
    fn move_by(&mut self, days: i64) {
        self.selected = self.selected.saturating_add(days.days());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use jiff::civil::date;

    /// 按一個鍵（沒有 Ctrl 之類的 modifier）
    fn press(app: &mut App, code: KeyCode) {
        app.handle_key(KeyEvent::from(code));
    }

    #[test]
    fn hjkl_and_arrows_move_by_a_day_or_a_week() {
        let mut app = App::new(Lang::En, date(2026, 10, 17), Vec::new());
        press(&mut app, KeyCode::Char('j'));
        assert_eq!(app.selected, date(2026, 10, 24));
        press(&mut app, KeyCode::Char('l'));
        assert_eq!(app.selected, date(2026, 10, 25));
        press(&mut app, KeyCode::Char('k'));
        assert_eq!(app.selected, date(2026, 10, 18));
        press(&mut app, KeyCode::Char('h'));
        assert_eq!(app.selected, date(2026, 10, 17));
        // 方向鍵一樣；跨月、跨年都照常移
        press(&mut app, KeyCode::Down);
        press(&mut app, KeyCode::Down);
        assert_eq!(app.selected, date(2026, 10, 31));
        press(&mut app, KeyCode::Right);
        assert_eq!(app.selected, date(2026, 11, 1));
        press(&mut app, KeyCode::Up);
        press(&mut app, KeyCode::Left);
        assert_eq!(app.selected, date(2026, 10, 24));
        app.selected = date(2026, 12, 29);
        press(&mut app, KeyCode::Char('j'));
        assert_eq!(app.selected, date(2027, 1, 5));
    }

    /// 滑鼠在 (`column`, `row`) 的一個動作
    fn mouse(kind: MouseEventKind, column: u16, row: u16) -> MouseEvent {
        MouseEvent {
            kind,
            column,
            row,
            modifiers: KeyModifiers::NONE,
        }
    }

    #[test]
    fn wheel_moves_a_week() {
        let mut app = App::new(Lang::En, date(2026, 10, 17), Vec::new());
        app.handle_mouse(mouse(MouseEventKind::ScrollDown, 0, 0));
        assert_eq!(app.selected, date(2026, 10, 24));
        app.handle_mouse(mouse(MouseEventKind::ScrollUp, 0, 0));
        app.handle_mouse(mouse(MouseEventKind::ScrollUp, 0, 0));
        assert_eq!(app.selected, date(2026, 10, 10));
    }

    #[test]
    fn left_click_on_a_date_selects_it() {
        // 40 × 26 的畫面、10 月在最上面：16 號在 (24～26, 5)，見 ui.rs 的畫面 test
        let mut app = App::new(Lang::En, date(2026, 10, 17), Vec::new());
        app.screen = Rect::new(0, 0, 40, 26);
        app.calendar_top = Some(0);
        app.handle_mouse(mouse(MouseEventKind::Down(MouseButton::Left), 25, 5));
        assert_eq!(app.selected, date(2026, 10, 16));
        // 右鍵、點在表頭上：不理
        app.handle_mouse(mouse(MouseEventKind::Down(MouseButton::Right), 28, 5));
        app.handle_mouse(mouse(MouseEventKind::Down(MouseButton::Left), 28, 1));
        assert_eq!(app.selected, date(2026, 10, 16));
    }

    #[test]
    fn brackets_jump_to_the_first_of_the_previous_or_next_month() {
        let mut app = App::new(Lang::En, date(2026, 10, 17), Vec::new());
        app.calendar_top = Some(3);
        press(&mut app, KeyCode::Char(']'));
        assert_eq!(app.selected, date(2026, 11, 1));
        // 跳完要把那個月捲到最上面（下一次畫的時候）
        assert_eq!(app.calendar_top, None);
        press(&mut app, KeyCode::Char('['));
        press(&mut app, KeyCode::Char('['));
        assert_eq!(app.selected, date(2026, 9, 1));
        // 跨年
        app.selected = date(2026, 12, 15);
        press(&mut app, KeyCode::Char(']'));
        assert_eq!(app.selected, date(2027, 1, 1));
        app.selected = date(2027, 1, 20);
        press(&mut app, KeyCode::Char('['));
        assert_eq!(app.selected, date(2026, 12, 1));
    }

    /// 今天 2026/10/17 的 app，任務檔是 `text`
    fn app_with(text: &str) -> App {
        App::new(
            Lang::En,
            date(2026, 10, 17),
            crate::tasks::parse(text).items,
        )
    }

    /// 右邊照順序是：10/10 a、10/17 b、10/17 c、10/20 d（沒日期的不算）
    const ITEMS: &str =
        "- d @2026-10-20\n- c @2026-10-17 09:00\n- 沒日期\n- b @2026-10-17\n- a @2026-10-10\n";

    #[test]
    fn tab_moves_into_the_agenda_and_back() {
        // 選取 10/17 → 換到右邊時選 10/17 當天的第一件（沒時間的 b 在前）
        let mut app = app_with(ITEMS);
        press(&mut app, KeyCode::Tab);
        assert_eq!(app.focus, Focus::Agenda);
        assert_eq!(
            app.chosen().map(|entry| entry.item.title.as_str()),
            Some("b")
        );
        press(&mut app, KeyCode::Tab);
        assert_eq!(app.focus, Focus::Calendar);
        assert_eq!(app.chosen(), None);
        // 選取那天之後都沒事：選最後一件，月曆跟著跳到那天
        app.selected = date(2026, 12, 1);
        press(&mut app, KeyCode::Tab);
        assert_eq!(
            app.chosen().map(|entry| entry.item.title.as_str()),
            Some("d")
        );
        assert_eq!(app.selected, date(2026, 10, 20));
    }

    #[test]
    fn tab_stays_on_the_calendar_without_items() {
        let mut app = app_with("- 沒日期\n");
        press(&mut app, KeyCode::Tab);
        assert_eq!(app.focus, Focus::Calendar);
    }

    #[test]
    fn jk_in_the_agenda_choose_items_and_the_calendar_follows() {
        let mut app = app_with(ITEMS);
        press(&mut app, KeyCode::Tab);
        press(&mut app, KeyCode::Char('j'));
        assert_eq!(
            app.chosen().map(|entry| entry.item.title.as_str()),
            Some("c")
        );
        press(&mut app, KeyCode::Down);
        assert_eq!(
            app.chosen().map(|entry| entry.item.title.as_str()),
            Some("d")
        );
        assert_eq!(app.selected, date(2026, 10, 20));
        // 最後一件再往下：停住
        press(&mut app, KeyCode::Char('j'));
        assert_eq!(
            app.chosen().map(|entry| entry.item.title.as_str()),
            Some("d")
        );
        // 往上到第一件、再往上：停住
        for _ in 0..5 {
            press(&mut app, KeyCode::Char('k'));
        }
        assert_eq!(
            app.chosen().map(|entry| entry.item.title.as_str()),
            Some("a")
        );
        assert_eq!(app.selected, date(2026, 10, 10));
        // h / l 在右邊不動
        press(&mut app, KeyCode::Char('l'));
        assert_eq!(app.selected, date(2026, 10, 10));
    }

    #[test]
    fn esc_in_the_agenda_goes_back_to_the_calendar() {
        let mut app = app_with(ITEMS);
        press(&mut app, KeyCode::Tab);
        press(&mut app, KeyCode::Esc);
        assert_eq!(app.focus, Focus::Calendar);
        assert!(!app.should_quit);
        // 已經在月曆：Esc 離開
        press(&mut app, KeyCode::Esc);
        assert!(app.should_quit);
    }

    #[test]
    fn t_and_brackets_go_back_to_the_calendar() {
        for code in [KeyCode::Char('t'), KeyCode::Char('['), KeyCode::Char(']')] {
            let mut app = app_with(ITEMS);
            press(&mut app, KeyCode::Tab);
            press(&mut app, code);
            assert_eq!(app.focus, Focus::Calendar, "{code:?}");
        }
    }

    #[test]
    fn wheel_in_the_agenda_moves_an_item() {
        let mut app = app_with(ITEMS);
        press(&mut app, KeyCode::Tab);
        app.handle_mouse(mouse(MouseEventKind::ScrollDown, 0, 0));
        assert_eq!(
            app.chosen().map(|entry| entry.item.title.as_str()),
            Some("c")
        );
        app.handle_mouse(mouse(MouseEventKind::ScrollUp, 0, 0));
        app.handle_mouse(mouse(MouseEventKind::ScrollUp, 0, 0));
        assert_eq!(
            app.chosen().map(|entry| entry.item.title.as_str()),
            Some("a")
        );
    }

    #[test]
    fn clicking_an_item_moves_the_keyboard_there() {
        // 70 × 23、10 月在最上面：右邊第一列是 10/10 a（右邊框裡面從 x 36、y 3 開始）
        let mut app = app_with(ITEMS);
        app.screen = Rect::new(0, 0, 70, 23);
        app.calendar_top = Some(0);
        app.handle_mouse(mouse(MouseEventKind::Down(MouseButton::Left), 40, 3));
        assert_eq!(app.focus, Focus::Agenda);
        assert_eq!(
            app.chosen().map(|entry| entry.item.title.as_str()),
            Some("a")
        );
        assert_eq!(app.selected, date(2026, 10, 10));
        // 再點月曆上的日期：鍵盤回到月曆
        app.handle_mouse(mouse(MouseEventKind::Down(MouseButton::Left), 25, 5));
        assert_eq!(app.focus, Focus::Calendar);
        assert_eq!(app.selected, date(2026, 10, 16));
    }

    #[test]
    fn t_goes_back_to_today() {
        let mut app = App::new(Lang::En, date(2026, 10, 17), Vec::new());
        app.selected = date(2027, 3, 1);
        app.calendar_top = Some(30);
        press(&mut app, KeyCode::Char('t'));
        assert_eq!(app.selected, date(2026, 10, 17));
        assert_eq!(app.calendar_top, None);
    }

    #[test]
    fn groups_keep_file_order() {
        // 檔案開頭沒有標題的是 `None`；同名的標題算同一個；`##` 也是一個分組
        let app = app_with("- a\n# 工作\n- b\n## 生活\n- c\n# 工作\n- d\n# 沒有項目\n");
        let names: Vec<Option<&str>> = app
            .groups
            .iter()
            .map(|group| group.name.as_deref())
            .collect();
        assert_eq!(names, [None, Some("工作"), Some("生活")]);
        assert!(app.groups.iter().all(|group| group.shown));
        // d 在第二個「# 工作」底下，也算「工作」
        assert!(app.groups[1].includes(&app.items[3]));
        assert!(!app.groups[0].includes(&app.items[3]));
    }

    #[test]
    fn unchecked_groups_leave_the_agenda() {
        // 「生活」沒勾：它的 b、c 不列，Tab 換到右邊時也跳過它們
        let mut app = app_with(
            "- a @2026-10-10\n# 生活\n- b @2026-10-17\n- c @2026-10-20\n# 工作\n- d @2026-10-25\n",
        );
        app.groups[1].shown = false;
        let titles: Vec<&str> = app
            .agenda()
            .iter()
            .map(|entry| entry.item.title.as_str())
            .collect();
        assert_eq!(titles, ["a", "d"]);
        assert!(!app.is_shown(&app.items[1]));
        press(&mut app, KeyCode::Tab);
        assert_eq!(
            app.chosen().map(|entry| entry.item.title.as_str()),
            Some("d")
        );
        // 勾回來：又列出來
        app.groups[1].shown = true;
        assert_eq!(app.agenda().len(), 4);
    }

    /// 120 格寬（有 side menu）、任務檔是 `text` 的 app
    fn wide_app_with(text: &str) -> App {
        let mut app = app_with(text);
        app.screen = Rect::new(0, 0, 120, 30);
        app
    }

    /// 三個分組：未分組（a）、生活（b）、工作（c）
    const GROUPS: &str = "- a @2026-10-10\n# 生活\n- b @2026-10-17\n# 工作\n- c @2026-10-20\n";

    #[test]
    fn tab_goes_through_the_side_menu_when_wide() {
        // 月曆 → 右邊 → side menu → 月曆
        let mut app = wide_app_with(GROUPS);
        for focus in [Focus::Agenda, Focus::Side, Focus::Calendar] {
            press(&mut app, KeyCode::Tab);
            assert_eq!(app.focus, focus);
        }
        // 右邊一件事都沒有：月曆直接到 side menu
        let mut app = wide_app_with("- 沒日期\n");
        press(&mut app, KeyCode::Tab);
        assert_eq!(app.focus, Focus::Side);
        // 收起來了：跟窄的時候一樣，月曆 ⇄ 右邊
        let mut app = wide_app_with(GROUPS);
        app.side_hidden = true;
        press(&mut app, KeyCode::Tab);
        press(&mut app, KeyCode::Tab);
        assert_eq!(app.focus, Focus::Calendar);
    }

    #[test]
    fn side_menu_keys_choose_and_check_calendars() {
        let mut app = wide_app_with(GROUPS);
        app.focus = Focus::Side;
        // j 到「生活」、Space 取消勾選：b 不列了
        press(&mut app, KeyCode::Char('j'));
        assert_eq!(app.group_index, 1);
        press(&mut app, KeyCode::Char(' '));
        assert!(!app.groups[1].shown);
        assert_eq!(app.agenda().len(), 2);
        // Enter 勾回來
        press(&mut app, KeyCode::Enter);
        assert!(app.groups[1].shown);
        // 到最後一個再往下、到第一個再往上：停住；方向鍵、滾輪一樣
        for _ in 0..3 {
            press(&mut app, KeyCode::Down);
        }
        assert_eq!(app.group_index, 2);
        app.handle_mouse(mouse(MouseEventKind::ScrollUp, 0, 0));
        assert_eq!(app.group_index, 1);
        for _ in 0..3 {
            press(&mut app, KeyCode::Char('k'));
        }
        assert_eq!(app.group_index, 0);
        // h / l 不動選取的日期；Esc 回月曆、不離開
        press(&mut app, KeyCode::Char('l'));
        assert_eq!(app.selected, date(2026, 10, 17));
        press(&mut app, KeyCode::Esc);
        assert_eq!(app.focus, Focus::Calendar);
        assert!(!app.should_quit);
    }

    #[test]
    fn the_keyboard_leaves_the_side_menu_when_it_goes_away() {
        // 按 `s` 收起
        let mut app = wide_app_with(GROUPS);
        app.focus = Focus::Side;
        press(&mut app, KeyCode::Char('s'));
        assert_eq!(app.focus, Focus::Calendar);
        // terminal 變窄
        let mut app = wide_app_with(GROUPS);
        app.focus = Focus::Side;
        app.resize(Rect::new(0, 0, 120, 20));
        assert_eq!(app.focus, Focus::Side);
        app.resize(Rect::new(0, 0, 119, 20));
        assert_eq!(app.focus, Focus::Calendar);
    }

    #[test]
    fn clicking_a_calendar_selects_it_then_toggles_it() {
        // 120 × 30：side menu 裡面從 y 3 開始一個日曆一列（畫面見 ui.rs 的 `side_menu_lists_the_calendars_when_wide`）
        let mut app = wide_app_with(GROUPS);
        let click = |app: &mut App, row| {
            app.handle_mouse(mouse(MouseEventKind::Down(MouseButton::Left), 5, row));
        };
        // 第一下：選「生活」、鍵盤換到 side menu，還沒勾
        click(&mut app, 4);
        assert_eq!((app.focus, app.group_index), (Focus::Side, 1));
        assert!(app.groups[1].shown);
        // 再點一次：取消勾選；再一次：勾回來
        click(&mut app, 4);
        assert!(!app.groups[1].shown);
        click(&mut app, 4);
        assert!(app.groups[1].shown);
        // 點別的日曆：只選、不勾
        click(&mut app, 5);
        assert_eq!(app.group_index, 2);
        assert!(app.groups[2].shown);
        // 鍵盤不在 side menu 時，點已經選到的那個也只是選（回到 side menu）
        app.focus = Focus::Calendar;
        click(&mut app, 5);
        assert_eq!(app.focus, Focus::Side);
        assert!(app.groups[2].shown);
    }

    #[test]
    fn s_hides_the_side_menu_only_when_it_fits() {
        let mut app = App::new(Lang::En, date(2026, 10, 17), Vec::new());
        // 不夠寬：按了不算
        app.screen = Rect::new(0, 0, 119, 30);
        press(&mut app, KeyCode::Char('s'));
        assert!(!app.side_hidden);
        // 夠寬：收起、再按一次打開
        app.screen = Rect::new(0, 0, 120, 30);
        press(&mut app, KeyCode::Char('s'));
        assert!(app.side_hidden);
        press(&mut app, KeyCode::Char('s'));
        assert!(!app.side_hidden);
    }

    #[test]
    fn a_multi_day_item_is_listed_in_each_month_and_chosen_from_any_of_its_days() {
        // 主管出差 10/28～11/3：10 月、11 月各列一行；11 月那行列在 11/1、排在 11/1 的準備月報前面
        let mut app = app_with(
            "- [ ] 準備月報 @2026-11-01\n- 主管出差 @2026-10-28..2026-11-03\n- [ ] 交週報 @2026-10-30\n",
        );
        let rows: Vec<(&str, Date)> = app
            .agenda()
            .iter()
            .map(|entry| (entry.item.title.as_str(), entry.date))
            .collect();
        assert_eq!(
            rows,
            [
                ("主管出差", date(2026, 10, 28)),
                ("交週報", date(2026, 10, 30)),
                ("主管出差", date(2026, 11, 1)),
                ("準備月報", date(2026, 11, 1)),
            ]
        );
        // 選 11/2（出差的中間）按 Tab：選到 11 月那一行，選取的日期不動
        app.selected = date(2026, 11, 2);
        press(&mut app, KeyCode::Tab);
        assert_eq!(app.agenda_index, 2);
        assert_eq!(app.selected, date(2026, 11, 2));
        // j：準備月報，月曆跳到 11/1；k：回到出差，11/1 也是它的日子，月曆不動
        press(&mut app, KeyCode::Char('j'));
        assert_eq!(app.selected, date(2026, 11, 1));
        press(&mut app, KeyCode::Char('k'));
        assert_eq!((app.agenda_index, app.selected), (2, date(2026, 11, 1)));
        // 再往上：交週報（10/30）→ 10 月那一行的出差（10/30 也是它的日子，月曆不動）
        press(&mut app, KeyCode::Char('k'));
        press(&mut app, KeyCode::Char('k'));
        assert_eq!((app.agenda_index, app.selected), (0, date(2026, 10, 30)));
        // 10/29（出差中間、10 月）按 Tab：選 10 月那一行，比 10/30 的交週報先
        let mut app = app_with("- [ ] 交週報 @2026-10-30\n- 主管出差 @2026-10-28..2026-11-03\n");
        app.selected = date(2026, 10, 29);
        press(&mut app, KeyCode::Tab);
        assert_eq!(
            app.chosen().map(|entry| entry.date),
            Some(date(2026, 10, 28))
        );
        assert_eq!(app.selected, date(2026, 10, 29));
    }

    #[test]
    fn number_keys_switch_layouts() {
        // 3 換到大月曆、1 換回來；換的時候選取那個月捲到最上面
        let mut app = app_with(ITEMS);
        app.calendar_top = Some(5);
        app.month_top = Some(date(2026, 1, 5));
        press(&mut app, KeyCode::Char('3'));
        assert_eq!(app.view, View::Month);
        assert_eq!((app.calendar_top, app.month_top), (None, None));
        press(&mut app, KeyCode::Char('1'));
        assert_eq!(app.view, View::Planner);
        // 鍵盤在右邊的事項框時換到大月曆（沒有事項框）：回到月曆；side menu 不受影響
        press(&mut app, KeyCode::Tab);
        assert_eq!(app.focus, Focus::Agenda);
        press(&mut app, KeyCode::Char('3'));
        assert_eq!(app.focus, Focus::Calendar);
        let mut app = wide_app_with(GROUPS);
        app.focus = Focus::Side;
        press(&mut app, KeyCode::Char('3'));
        assert_eq!(app.focus, Focus::Side);
    }

    #[test]
    fn tab_skips_the_agenda_in_the_month_view() {
        // 大月曆：月曆 → side menu → 月曆；窄的時候（沒有 side menu）按了不動
        let mut app = wide_app_with(GROUPS);
        app.view = View::Month;
        for focus in [Focus::Side, Focus::Calendar] {
            press(&mut app, KeyCode::Tab);
            assert_eq!(app.focus, focus);
        }
        let mut app = app_with(ITEMS);
        app.view = View::Month;
        press(&mut app, KeyCode::Tab);
        assert_eq!(app.focus, Focus::Calendar);
    }

    #[test]
    fn the_mouse_works_in_the_month_view() {
        // 80 × 30、最上面是 9/21（格子的位置見 ui.rs 的 `month_view_click_positions_map_to_the_cells`）
        let mut app = app_with(ITEMS);
        app.view = View::Month;
        app.screen = Rect::new(0, 0, 80, 30);
        app.month_top = Some(date(2026, 9, 21));
        // 點第 2 週星期二的格子（不一定要點在日期上）：選那天
        app.handle_mouse(mouse(MouseEventKind::Down(MouseButton::Left), 20, 9));
        assert_eq!(app.selected, date(2026, 9, 29));
        // 滾輪移一週
        app.handle_mouse(mouse(MouseEventKind::ScrollDown, 20, 9));
        assert_eq!(app.selected, date(2026, 10, 6));
    }

    #[test]
    fn t_and_brackets_put_the_month_on_top_in_the_month_view_too() {
        for code in [KeyCode::Char('t'), KeyCode::Char('['), KeyCode::Char(']')] {
            let mut app = app_with(ITEMS);
            app.view = View::Month;
            app.month_top = Some(date(2026, 1, 5));
            press(&mut app, code);
            assert_eq!(app.month_top, None, "{code:?}");
        }
    }

    #[test]
    fn quit_keys() {
        let keys = [
            KeyEvent::from(KeyCode::Char('q')),
            KeyEvent::from(KeyCode::Esc),
            KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL),
        ];
        for key in keys {
            let mut app = App::new(Lang::En, date(2026, 10, 17), Vec::new());
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
            let mut app = App::new(Lang::En, date(2026, 10, 17), Vec::new());
            app.handle_key(key);
            assert!(!app.should_quit, "{key:?} 不應該離開");
        }
    }
}
