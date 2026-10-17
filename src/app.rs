//! 程式的狀態與主迴圈：畫畫面 → 等輸入 → 改狀態，一直重複到離開

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

/// 鍵盤在哪個框
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Focus {
    /// 左邊的月曆：`hjkl` 移動選取的日期
    Calendar,
    /// 右邊的事項：`j` / `k` 一件一件選
    Agenda,
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
    /// 鍵盤在哪個框
    pub focus: Focus,
    /// 右邊選到第幾件事（`agenda()` 裡的位置）；鍵盤在右邊時才用
    pub agenda_index: usize,
    /// 月曆捲到哪：畫面最上面是第幾列（`None` = 下一次畫的時候把選取那天的那個月放在最上面）
    pub calendar_top: Option<i64>,
    /// 右邊捲到哪：(哪個月的 1 號, 那個月右邊最上面是排好的第幾列)；事太多、那個月放不下時才會捲
    pub agenda_window: Option<(Date, usize)>,
    /// 畫面的顏色
    pub theme: Theme,
    /// terminal 的大小（每一輪畫之前更新），滑鼠點的位置要用它換算
    pub screen: Rect,
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
            items,
            focus: Focus::Calendar,
            agenda_index: 0,
            calendar_top: None,
            agenda_window: None,
            theme: Theme::catppuccin_mocha(),
            screen: Rect::default(),
            should_quit: false,
        }
    }

    /// 右邊列出來的事：有日期的，照日期、時間排好（同一天沒有時間的在前；日期、時間一樣的照檔案裡的順序）
    ///
    /// 沒有日期的事先不列（月曆上沒有位置放）
    pub fn agenda(&self) -> Vec<&Item> {
        let mut items: Vec<&Item> = self
            .items
            .iter()
            .filter(|item| item.date.is_some())
            .collect();
        // sort_by_key 是 stable sort：key 一樣的保持原本的順序；時間的 None 排在 Some 前面
        items.sort_by_key(|item| (item.date, item.time));
        items
    }

    /// 鍵盤在右邊時選到的那件事；鍵盤在月曆時是 `None`
    pub fn chosen(&self) -> Option<&Item> {
        match self.focus {
            Focus::Agenda => self.agenda().get(self.agenda_index).copied(),
            Focus::Calendar => None,
        }
    }

    /// 主迴圈：每一輪畫一次畫面、等一個輸入，直到要離開
    pub fn run(mut self, terminal: &mut DefaultTerminal) -> Result<()> {
        while !self.should_quit {
            // 先記下 terminal 的大小（視窗拉大縮小也會跟著變），畫面和滑鼠換算位置才一致
            let size = terminal.size()?;
            self.screen = Rect::new(0, 0, size.width, size.height);
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

    /// 按鍵：`Tab` 換框、`[` / `]` 換月、`t` 回今天、`s` 收起 / 打開 side menu、`q` / `Ctrl+C` 離開、
    /// `Esc` 回月曆（已經在月曆就離開）；其他鍵看鍵盤在哪個框
    fn handle_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Char('q') => self.should_quit = true,
            // raw mode 下 Ctrl+C 不會送 SIGINT 讓程式結束，要自己處理
            KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.should_quit = true;
            }
            KeyCode::Esc if self.focus == Focus::Agenda => self.focus = Focus::Calendar,
            KeyCode::Esc => self.should_quit = true,
            KeyCode::Tab => self.next_focus(),
            // 回今天，並把今天那個月捲到最上面
            KeyCode::Char('t') => {
                self.selected = self.today;
                self.calendar_top = None;
                self.focus = Focus::Calendar;
            }
            // 上個月 / 下個月的 1 號，並把那個月捲到最上面
            KeyCode::Char('[') => self.jump_month(-1),
            KeyCode::Char(']') => self.jump_month(1),
            // terminal 不夠寬時本來就沒有 side menu，按了不算
            KeyCode::Char('s') if ui::side_fits(self.screen) => {
                self.side_hidden = !self.side_hidden;
            }
            _ => match self.focus {
                Focus::Calendar => self.handle_calendar_key(key.code),
                Focus::Agenda => self.handle_agenda_key(key.code),
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

    /// 鍵盤在右邊：上下一件事，月曆選取的日期跟著那件事走
    fn handle_agenda_key(&mut self, code: KeyCode) {
        match code {
            KeyCode::Char('j') | KeyCode::Down => self.choose_next(1),
            KeyCode::Char('k') | KeyCode::Up => self.choose_next(-1),
            _ => {}
        }
    }

    /// 滑鼠：滾輪上下（月曆移一週、右邊移一件事）；左鍵點月曆上的日期 = 選那天、點右邊的事 = 選那件
    fn handle_mouse(&mut self, mouse: MouseEvent) {
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
                    Some(Click::Item(index)) => self.choose(index),
                    None => {}
                }
            }
            // 其他（右鍵、拖曳、移動）不理
            _ => {}
        }
    }

    /// 滾輪往下（`step` = 1）/ 往上（-1）：鍵盤在月曆就移一週，在右邊就移一件事
    fn scroll(&mut self, step: i8) {
        match self.focus {
            Focus::Calendar => self.move_by(7 * i64::from(step)),
            Focus::Agenda => self.choose_next(isize::from(step)),
        }
    }

    /// `Tab`：月曆 → 右邊的事項（一件事都沒有就留在月曆）→ 月曆
    fn next_focus(&mut self) {
        match self.focus {
            Focus::Calendar => self.enter_agenda(),
            Focus::Agenda => self.focus = Focus::Calendar,
        }
    }

    /// 鍵盤換到右邊：選「選取那天或之後」的第一件事；之後都沒事就選最後一件；一件事都沒有就不換
    fn enter_agenda(&mut self) {
        let index = {
            let agenda = self.agenda();
            let Some(last) = agenda.len().checked_sub(1) else {
                return;
            };
            agenda
                .iter()
                .position(|item| item.date >= Some(self.selected))
                .unwrap_or(last)
        };
        self.choose(index);
    }

    /// 選右邊第 `index` 件事：鍵盤換到右邊，月曆選取那件事那天
    fn choose(&mut self, index: usize) {
        if let Some(date) = self.agenda().get(index).and_then(|item| item.date) {
            self.agenda_index = index;
            self.selected = date;
            self.focus = Focus::Agenda;
        }
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
        self.calendar_top = None;
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
        assert_eq!(app.chosen().map(|item| item.title.as_str()), Some("b"));
        press(&mut app, KeyCode::Tab);
        assert_eq!(app.focus, Focus::Calendar);
        assert_eq!(app.chosen(), None);
        // 選取那天之後都沒事：選最後一件，月曆跟著跳到那天
        app.selected = date(2026, 12, 1);
        press(&mut app, KeyCode::Tab);
        assert_eq!(app.chosen().map(|item| item.title.as_str()), Some("d"));
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
        assert_eq!(app.chosen().map(|item| item.title.as_str()), Some("c"));
        press(&mut app, KeyCode::Down);
        assert_eq!(app.chosen().map(|item| item.title.as_str()), Some("d"));
        assert_eq!(app.selected, date(2026, 10, 20));
        // 最後一件再往下：停住
        press(&mut app, KeyCode::Char('j'));
        assert_eq!(app.chosen().map(|item| item.title.as_str()), Some("d"));
        // 往上到第一件、再往上：停住
        for _ in 0..5 {
            press(&mut app, KeyCode::Char('k'));
        }
        assert_eq!(app.chosen().map(|item| item.title.as_str()), Some("a"));
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
        assert_eq!(app.chosen().map(|item| item.title.as_str()), Some("c"));
        app.handle_mouse(mouse(MouseEventKind::ScrollUp, 0, 0));
        app.handle_mouse(mouse(MouseEventKind::ScrollUp, 0, 0));
        assert_eq!(app.chosen().map(|item| item.title.as_str()), Some("a"));
    }

    #[test]
    fn clicking_an_item_moves_the_keyboard_there() {
        // 70 × 23、10 月在最上面：右邊第一列是 10/10 a（右邊框裡面從 x 36、y 3 開始）
        let mut app = app_with(ITEMS);
        app.screen = Rect::new(0, 0, 70, 23);
        app.calendar_top = Some(0);
        app.handle_mouse(mouse(MouseEventKind::Down(MouseButton::Left), 40, 3));
        assert_eq!(app.focus, Focus::Agenda);
        assert_eq!(app.chosen().map(|item| item.title.as_str()), Some("a"));
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
