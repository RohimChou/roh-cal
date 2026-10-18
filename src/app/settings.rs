//! 設定頁（`,`）：設定檔的開關列成一張清單，蓋在畫面正中間；改了畫面馬上跟著變
//!
//! `j` / `k` 選一列、`h` / `l`（`Space`、`Enter`）改那一列的值、`r` 還原那一列、`R` 全部還原（要再按 `y`）、
//! `q` / `Esc` / `,` 或點框外關掉

use super::App;
use crate::config::Settings;
use crate::ui;
use ratatui::{
    crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind},
    layout::Position,
};

/// 設定頁開著：選到第幾列、是不是正在問要不要全部還原
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct SettingsPage {
    /// 選到 `ROWS` 的第幾列
    pub row: usize,
    /// 按了 `R`、正在問要不要全部還原（下框線換成問句）
    pub confirm_reset: bool,
}

/// 設定頁的一列 = 設定檔的一個開關
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Row {
    /// `[calendar] relative_weeks`
    RelativeWeeks,
    /// `[month] grid_lines`
    GridLines,
    /// `[month] item_icons`
    ItemIcons,
    /// `[month] center_cursor`
    CenterCursor,
}

/// 設定頁的列，照顯示的順序（同一個表的排在一起）
pub const ROWS: [Row; 4] = [
    Row::RelativeWeeks,
    Row::GridLines,
    Row::ItemIcons,
    Row::CenterCursor,
];

impl Row {
    /// 這一列在設定檔的哪個表（`[calendar]`、`[month]`）；設定頁也照這個分段
    pub fn section(self) -> &'static str {
        match self {
            Self::RelativeWeeks => "calendar",
            Self::GridLines | Self::ItemIcons | Self::CenterCursor => "month",
        }
    }

    /// 顯示在左邊的名字 = 設定檔裡的名字（有 test 釘住）
    pub fn name(self) -> &'static str {
        match self {
            Self::RelativeWeeks => "relative_weeks",
            Self::GridLines => "grid_lines",
            Self::ItemIcons => "item_icons",
            Self::CenterCursor => "center_cursor",
        }
    }

    /// 這一列在 `settings` 裡的開關（`&mut`：改值、還原都用這個）
    fn switch(self, settings: &mut Settings) -> &mut bool {
        match self {
            Self::RelativeWeeks => &mut settings.calendar.relative_weeks,
            Self::GridLines => &mut settings.month.grid_lines,
            Self::ItemIcons => &mut settings.month.item_icons,
            Self::CenterCursor => &mut settings.month.center_cursor,
        }
    }

    /// 現在的值：開著 / 關著
    pub fn value(self, settings: &Settings) -> bool {
        // `switch` 要 `&mut`，複製一份來讀（`Settings` 是 `Copy`）
        *self.switch(&mut settings.clone())
    }

    /// 改這一列的值：開關 on ⇄ off
    fn change(self, settings: &mut Settings) {
        let switch = self.switch(settings);
        *switch = !*switch;
    }

    /// 這一列還原成預設值
    fn reset(self, settings: &mut Settings) {
        *self.switch(settings) = self.value(&Settings::default());
    }
}

impl App {
    /// `,`：打開設定頁，選第一列
    pub(super) fn open_settings(&mut self) {
        self.settings_page = Some(SettingsPage::default());
    }

    /// 設定頁開著時的按鍵：見這個 module 開頭的說明；其他鍵不理
    pub(super) fn handle_settings_key(&mut self, key: KeyEvent) {
        let Some(page) = self.settings_page else {
            return;
        };
        // 正在問要不要全部還原：`y` 還原，其他鍵只是取消
        if page.confirm_reset {
            if key.code == KeyCode::Char('y') {
                self.settings = Settings::default();
            }
            self.settings_page = Some(SettingsPage {
                confirm_reset: false,
                ..page
            });
            return;
        }
        let row = ROWS[page.row.min(ROWS.len() - 1)];
        match key.code {
            KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.should_quit = true;
            }
            KeyCode::Char('j') | KeyCode::Down => self.choose_setting(1),
            KeyCode::Char('k') | KeyCode::Up => self.choose_setting(-1),
            KeyCode::Char('h' | 'l' | ' ') | KeyCode::Left | KeyCode::Right | KeyCode::Enter => {
                row.change(&mut self.settings);
            }
            KeyCode::Char('r') => row.reset(&mut self.settings),
            KeyCode::Char('R') => {
                self.settings_page = Some(SettingsPage {
                    confirm_reset: true,
                    ..page
                });
            }
            KeyCode::Char('q' | ',') | KeyCode::Esc => self.close_settings(),
            _ => {}
        }
    }

    /// 設定頁開著時的滑鼠：滾輪上下選一列；點一列 = 選那列，再點一次選到的那列 = 改它的值；點框外關掉
    pub(super) fn handle_settings_mouse(&mut self, mouse: MouseEvent) {
        let Some(page) = self.settings_page else {
            return;
        };
        match mouse.kind {
            MouseEventKind::ScrollDown => self.choose_setting(1),
            MouseEventKind::ScrollUp => self.choose_setting(-1),
            MouseEventKind::Down(MouseButton::Left) => {
                let position = Position::new(mouse.column, mouse.row);
                if !ui::settings_area(self).contains(position) {
                    self.close_settings();
                    return;
                }
                match ui::settings_row_at(self, mouse.column, mouse.row) {
                    Some(row) if row == page.row => ROWS[row].change(&mut self.settings),
                    Some(row) => self.settings_page = Some(SettingsPage { row, ..page }),
                    None => {}
                }
            }
            _ => {}
        }
    }

    /// 選下一列（`step` = 1）/ 上一列（-1）；到頭尾就停
    fn choose_setting(&mut self, step: isize) {
        if let Some(page) = &mut self.settings_page {
            page.row = page.row.saturating_add_signed(step).min(ROWS.len() - 1);
        }
    }

    /// 關掉設定頁
    fn close_settings(&mut self) {
        self.settings_page = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{CalendarSettings, MonthSettings};
    use crate::texts::Lang;
    use jiff::civil::date;
    use ratatui::layout::Rect;

    /// 打開設定頁的 app（terminal 100 × 30）
    fn app_with_settings() -> App {
        let mut app = App::new(Lang::ZhTw, date(2026, 10, 17), Vec::new());
        app.screen = Rect::new(0, 0, 100, 30);
        press(&mut app, KeyCode::Char(','));
        app
    }

    fn press(app: &mut App, code: KeyCode) {
        app.handle_key(KeyEvent::from(code));
    }

    #[test]
    fn every_switch_has_a_row() {
        // 把 `Settings` 拆開到每個欄位：多加了開關、這裡沒跟著加就編譯不過，提醒要在 `ROWS` 加一列
        let Settings {
            calendar: CalendarSettings { relative_weeks: _ },
            month:
                MonthSettings {
                    grid_lines: _,
                    item_icons: _,
                    center_cursor: _,
                },
        } = Settings::default();
        assert_eq!(ROWS.len(), 4);
    }

    #[test]
    fn row_names_are_the_config_file_names() {
        // 每一列照「[表] 名字 = 跟預設相反的值」寫成設定檔，讀回來剛好是那一列改過的值
        for row in ROWS {
            let flipped = !row.value(&Settings::default());
            let text = format!("[{}]\n{} = {flipped}\n", row.section(), row.name());
            let settings: Settings = toml::from_str(&text).expect(&text);
            assert_eq!(row.value(&settings), flipped, "{text}");
        }
    }

    #[test]
    fn comma_opens_it_and_hl_change_the_chosen_row() {
        let mut app = app_with_settings();
        assert_eq!(app.settings_page, Some(SettingsPage::default()));
        // 第一列 relative_weeks：l 打開、h 再關掉
        press(&mut app, KeyCode::Char('l'));
        assert!(app.settings.calendar.relative_weeks);
        press(&mut app, KeyCode::Char('h'));
        assert!(!app.settings.calendar.relative_weeks);
        // j 到 grid_lines，Space 關掉；k 回到第一列；到頭不會再往上
        press(&mut app, KeyCode::Char('j'));
        press(&mut app, KeyCode::Char(' '));
        assert!(!app.settings.month.grid_lines);
        press(&mut app, KeyCode::Char('k'));
        press(&mut app, KeyCode::Char('k'));
        assert_eq!(app.settings_page.map(|page| page.row), Some(0));
        // 到尾也停在最後一列
        for _ in 0..10 {
            press(&mut app, KeyCode::Down);
        }
        assert_eq!(app.settings_page.map(|page| page.row), Some(ROWS.len() - 1));
    }

    #[test]
    fn r_resets_a_row_and_capital_r_then_y_resets_everything() {
        let mut app = app_with_settings();
        press(&mut app, KeyCode::Char('l'));
        press(&mut app, KeyCode::Char('j'));
        press(&mut app, KeyCode::Char('l'));
        // r：只還原 grid_lines，relative_weeks 還是開著
        press(&mut app, KeyCode::Char('r'));
        assert!(app.settings.month.grid_lines);
        assert!(app.settings.calendar.relative_weeks);
        // R 之後按別的鍵：只是取消，那個鍵也不做事（j 不會移動）
        press(&mut app, KeyCode::Char('R'));
        assert_eq!(app.settings_page.map(|page| page.confirm_reset), Some(true));
        press(&mut app, KeyCode::Char('j'));
        assert_eq!(
            app.settings_page.map(|page| page.confirm_reset),
            Some(false)
        );
        assert_eq!(app.settings_page.map(|page| page.row), Some(1));
        assert!(app.settings.calendar.relative_weeks);
        // R 再 y：全部還原
        press(&mut app, KeyCode::Char('R'));
        press(&mut app, KeyCode::Char('y'));
        assert_eq!(app.settings, Settings::default());
        assert!(app.settings_page.is_some());
    }

    #[test]
    fn clicking_chooses_a_row_then_changes_it_and_clicking_outside_closes() {
        // 100 × 30：框（46 × 13）在 (27, 9)，grid_lines 那列在框裡第 5 行：9 + 框線 1 + padding 1 + 4 = 15
        let mut app = app_with_settings();
        let click = |app: &mut App, column, row| {
            app.handle_mouse(MouseEvent {
                kind: MouseEventKind::Down(MouseButton::Left),
                column,
                row,
                modifiers: KeyModifiers::NONE,
            });
        };
        click(&mut app, 50, 15);
        assert_eq!(app.settings_page.map(|page| page.row), Some(1));
        assert!(app.settings.month.grid_lines);
        // 再點一次選到的那列：改值
        click(&mut app, 50, 15);
        assert!(!app.settings.month.grid_lines);
        // 滾輪：選下一列
        app.handle_mouse(MouseEvent {
            kind: MouseEventKind::ScrollDown,
            column: 50,
            row: 15,
            modifiers: KeyModifiers::NONE,
        });
        assert_eq!(app.settings_page.map(|page| page.row), Some(2));
        // 點框外：關掉，底下的月曆不會被點到
        click(&mut app, 0, 0);
        assert_eq!(app.settings_page, None);
        assert_eq!(app.selected, date(2026, 10, 17));
    }

    #[test]
    fn q_esc_and_comma_close_it_and_other_keys_do_nothing() {
        for code in [KeyCode::Char('q'), KeyCode::Esc, KeyCode::Char(',')] {
            let mut app = app_with_settings();
            press(&mut app, code);
            assert_eq!(app.settings_page, None, "{code:?}");
            assert!(!app.should_quit, "{code:?}");
        }
        // 移動日期、換版型、說明頁都不理
        let mut app = app_with_settings();
        for code in ['t', ']', '3', '?', 'a'].map(KeyCode::Char) {
            press(&mut app, code);
        }
        assert_eq!(app.selected, date(2026, 10, 17));
        assert_eq!(app.view, super::super::View::Planner);
        assert_eq!(app.help, None);
        assert_eq!(app.settings_page, Some(SettingsPage::default()));
    }
}
