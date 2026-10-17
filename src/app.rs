//! App 的狀態和按鍵：今天、選取的日期、捲到哪

use crate::{calendar, items::Item, planner, texts::Lang, theme::Theme};
use color_eyre::Result;
use jiff::{ToSpan, civil::Date};
use ratatui::{
    DefaultTerminal,
    crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers},
};

/// 鍵盤在操作哪一邊
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Focus {
    /// 左邊的月曆
    Calendar,
    /// 右邊的事項
    Agenda,
    /// 最左邊的 side menu（夠寬才有）
    Side,
}

/// 右邊那個框顯示什麼（版型）；數字鍵切換
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Variant {
    /// `1`：這個月的事，跟月曆的月份對齊
    MonthItems,
    /// `2`：從選取那天開始的事，一天一段
    Upcoming,
    /// `3`：大月曆，格子裡列當天的事（沒有右邊的事項框）
    Month,
}

/// side menu 上的一個日曆（= 任務檔的分組）
pub struct Group {
    /// 分組名稱；空字串 = 檔案開頭還沒有標題的項目
    pub name: String,
    /// 有沒有勾選：沒勾的日曆，它的事不顯示
    pub shown: bool,
}

/// 這件事要不要顯示：它的日曆有勾選才顯示
pub fn is_shown(groups: &[Group], item: &Item) -> bool {
    groups
        .iter()
        .any(|group| group.name == item.group && group.shown)
}

pub struct App {
    pub today: Date,
    pub selected: Date,
    /// 畫面上第一個月（1 號）：今天往前 12 個月
    pub first_month: Date,
    /// 畫面上最後一個月（1 號）：今天往後 24 個月
    pub last_month: Date,
    /// 捲動位置：畫面最上面是第幾列；畫的時候依選取的日期調整
    pub scroll: usize,
    /// 下次畫的時候，把選取的那個月捲到最上面（剛打開、按 `t` 的時候）
    pub jump_to_month: bool,
    pub items: Vec<Item>,
    pub theme: Theme,
    /// 畫面的語言，按 `L` 切換
    pub lang: Lang,
    /// 鍵盤在操作哪一邊，按 `Tab` 切換
    pub focus: Focus,
    /// 右邊選到第幾件事（有顯示的事裡的 index）；`focus` 是 `Agenda` 時才用
    pub selected_item: usize,
    /// 版型，按 `1` / `2` / `3` 切換
    pub variant: Variant,
    /// 版型 3（大月曆）最上面那週的星期一
    pub month_top: Date,
    /// side menu 上的日曆
    pub groups: Vec<Group>,
    /// side menu 選到第幾個日曆；`focus` 是 `Side` 時才用
    pub selected_group: usize,
    /// 按 `s` 收起 side menu
    pub side_hidden: bool,
    /// 這一次畫面有沒有畫 side menu（夠寬、沒收起）；畫的時候更新
    pub side_shown: bool,
    should_quit: bool,
}

impl App {
    pub fn new(today: Date, items: Vec<Item>, groups: Vec<String>) -> Self {
        let this_month = today.first_of_month();
        Self {
            today,
            selected: today,
            first_month: this_month.saturating_sub(12.months()),
            last_month: this_month.saturating_add(24.months()),
            scroll: 0,
            jump_to_month: true,
            items,
            theme: Theme::catppuccin_mocha(),
            lang: Lang::Zh,
            focus: Focus::Calendar,
            selected_item: 0,
            groups: groups
                .into_iter()
                .map(|name| Group { name, shown: true })
                .collect(),
            selected_group: 0,
            variant: Variant::MonthItems,
            month_top: calendar::monday_of(this_month),
            side_hidden: false,
            side_shown: false,
            should_quit: false,
        }
    }

    /// 主迴圈：畫畫面 → 等一個輸入 → 處理，直到按了離開
    pub fn run(mut self, terminal: &mut DefaultTerminal) -> Result<()> {
        while !self.should_quit {
            terminal.draw(|frame| planner::render(frame, &mut self))?;
            // event::read() 會 block 到有輸入為止
            if let Event::Key(key) = event::read()?
                && key.kind == KeyEventKind::Press
            {
                self.handle_key(key);
            }
        }
        Ok(())
    }

    fn handle_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Char('q') => self.should_quit = true,
            // Esc：不在月曆就回月曆，在月曆就離開
            KeyCode::Esc if self.focus != Focus::Calendar => self.focus = Focus::Calendar,
            KeyCode::Esc => self.should_quit = true,
            KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.should_quit = true;
            }
            KeyCode::Tab => self.next_focus(),
            KeyCode::Char('t') => {
                self.selected = self.today;
                self.focus = Focus::Calendar;
                self.jump_to_month = true;
            }
            KeyCode::Char('L') => self.lang = self.lang.toggled(),
            // 換版型：月份的高度會變，把選取的那個月捲到最上面
            KeyCode::Char('1') => {
                self.variant = Variant::MonthItems;
                self.jump_to_month = true;
            }
            KeyCode::Char('2') => {
                self.variant = Variant::Upcoming;
                self.jump_to_month = true;
            }
            KeyCode::Char('3') => {
                self.variant = Variant::Month;
                self.jump_to_month = true;
            }
            KeyCode::Char('s') => {
                self.side_hidden = !self.side_hidden;
                if self.focus == Focus::Side {
                    self.focus = Focus::Calendar;
                }
            }
            _ if self.focus == Focus::Agenda => self.handle_agenda_key(key.code),
            _ if self.focus == Focus::Side => self.handle_side_key(key.code),
            KeyCode::Char('h') | KeyCode::Left => self.move_by(-1),
            KeyCode::Char('l') | KeyCode::Right => self.move_by(1),
            KeyCode::Char('j') | KeyCode::Down => self.move_by(7),
            KeyCode::Char('k') | KeyCode::Up => self.move_by(-7),
            _ => {}
        }
    }

    /// 有顯示的事（它的日曆有勾選）
    pub fn visible_items(&self) -> Vec<&Item> {
        self.items
            .iter()
            .filter(|item| is_shown(&self.groups, item))
            .collect()
    }

    /// 右邊的事項：`j` / `k` 上下一件，月曆選取的日期跟著那件事走
    fn handle_agenda_key(&mut self, code: KeyCode) {
        let visible = self.visible_items();
        let last = visible.len().saturating_sub(1);
        let index = match code {
            KeyCode::Char('j') | KeyCode::Down => (self.selected_item + 1).min(last),
            KeyCode::Char('k') | KeyCode::Up => self.selected_item.saturating_sub(1),
            _ => return,
        };
        let Some(item) = visible.get(index) else {
            return;
        };
        let date = item.date;
        self.selected_item = index;
        self.selected = date;
    }

    /// side menu：`j` / `k` 上下選日曆，`Space` / `Enter` 勾選或取消
    fn handle_side_key(&mut self, code: KeyCode) {
        let last = self.groups.len().saturating_sub(1);
        match code {
            KeyCode::Char('j') | KeyCode::Down => {
                self.selected_group = (self.selected_group + 1).min(last);
            }
            KeyCode::Char('k') | KeyCode::Up => {
                self.selected_group = self.selected_group.saturating_sub(1);
            }
            KeyCode::Char(' ') | KeyCode::Enter => {
                if let Some(group) = self.groups.get_mut(self.selected_group) {
                    group.shown = !group.shown;
                }
            }
            _ => {}
        }
    }

    /// `Tab`：月曆 → 右邊的事項（版型 3 沒有）→ side menu（有畫才去）→ 月曆
    fn next_focus(&mut self) {
        match self.focus {
            Focus::Calendar => {
                if !self.focus_agenda() && self.side_shown {
                    self.focus = Focus::Side;
                }
            }
            Focus::Agenda if self.side_shown => self.focus = Focus::Side,
            Focus::Agenda | Focus::Side => self.focus = Focus::Calendar,
        }
    }

    /// 切到右邊的事項：選「選取那天或之後」的第一件事（都沒有就最後一件）；一件都沒顯示、或版型 3（沒有右邊的框）就不切，回傳 `false`
    fn focus_agenda(&mut self) -> bool {
        if self.variant == Variant::Month {
            return false;
        }
        let visible = self.visible_items();
        let Some(last) = visible.len().checked_sub(1) else {
            return false;
        };
        let index = visible
            .iter()
            .position(|item| item.date >= self.selected)
            .unwrap_or(last);
        let date = visible[index].date;
        self.selected_item = index;
        self.selected = date;
        self.focus = Focus::Agenda;
        true
    }

    /// 選取的日期往後（負的 = 往前）移幾天；超出畫面上的月份就停在頭 / 尾那天
    fn move_by(&mut self, days: i64) {
        let moved = self.selected.saturating_add(days.days());
        self.selected = moved.clamp(self.first_month, self.last_month.last_of_month());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use jiff::civil::date;

    fn press(app: &mut App, code: KeyCode) {
        app.handle_key(KeyEvent::from(code));
    }

    #[test]
    fn hjkl_move_by_a_day_or_a_week_and_t_jumps_back() {
        let mut app = App::new(date(2026, 10, 17), Vec::new(), Vec::new());
        press(&mut app, KeyCode::Char('j'));
        assert_eq!(app.selected, date(2026, 10, 24));
        press(&mut app, KeyCode::Char('l'));
        assert_eq!(app.selected, date(2026, 10, 25));
        press(&mut app, KeyCode::Char('k'));
        press(&mut app, KeyCode::Char('h'));
        assert_eq!(app.selected, date(2026, 10, 17));
        // 往後一個多月再按 t：回到今天，而且要把那個月捲到最上面
        for _ in 0..6 {
            press(&mut app, KeyCode::Down);
        }
        app.jump_to_month = false;
        press(&mut app, KeyCode::Char('t'));
        assert_eq!(app.selected, date(2026, 10, 17));
        assert!(app.jump_to_month);
        // L 切換語言，再按一次切回來
        press(&mut app, KeyCode::Char('L'));
        assert_eq!(app.lang, Lang::En);
        press(&mut app, KeyCode::Char('L'));
        assert_eq!(app.lang, Lang::Zh);
    }

    #[test]
    fn moving_stops_at_the_first_and_last_day_on_screen() {
        // 畫面上是 2025/10 ～ 2028/10
        let mut app = App::new(date(2026, 10, 17), Vec::new(), Vec::new());
        app.selected = date(2025, 10, 3);
        press(&mut app, KeyCode::Char('k'));
        assert_eq!(app.selected, date(2025, 10, 1));
        app.selected = date(2028, 10, 28);
        press(&mut app, KeyCode::Char('j'));
        assert_eq!(app.selected, date(2028, 10, 31));
    }
}
