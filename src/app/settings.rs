//! 設定頁（`,`）：設定檔的設定列成一張清單（theme、開關、每個角色的顏色），蓋在畫面正中間；
//! 改了畫面馬上跟著變，關掉時存回設定檔
//!
//! `j` / `k` 選一列、`h` / `l`（`Space`、`Enter`）改那一列的值、`r` 還原那一列、`R` 全部還原（要再按 `y`）、
//! `q` / `Esc` / `,` 或點框外關掉

use super::{App, Notice};
use crate::config::{self, Settings};
use crate::theme::{self, Theme};
use crate::ui;
use ratatui::{
    crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind},
    layout::Position,
};

/// 設定頁開著：選到第幾列、捲到哪、是不是正在問要不要全部還原
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct SettingsPage {
    /// 選到 `rows()` 的第幾列
    pub row: usize,
    /// 畫面放不下時，框裡最上面是第幾行（每次畫的時候捲到選到的那列看得到，見 `ui::settings`）
    pub top: usize,
    /// 按了 `R`、正在問要不要全部還原（下框線換成問句）
    pub confirm_reset: bool,
}

/// 設定頁的一列 = 設定檔的一個設定
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Row {
    /// 最上層的 `theme`：用哪一套內建的 theme
    Theme,
    /// `[calendar] relative_weeks`
    RelativeWeeks,
    /// `[month] grid_lines`
    GridLines,
    /// `[month] item_icons`
    ItemIcons,
    /// `[month] center_cursor`
    CenterCursor,
    /// `[colors]` 的一個角色（`theme::ROLES` 的名字）：蓋掉 theme 的那個顏色
    Color(&'static str),
}

/// 設定頁的列，照顯示的順序（跟設定檔一樣：最上層的在前，同一個表的排在一起；最後是每個角色的顏色）
pub fn rows() -> Vec<Row> {
    let mut rows = vec![
        Row::Theme,
        Row::RelativeWeeks,
        Row::GridLines,
        Row::ItemIcons,
        Row::CenterCursor,
    ];
    rows.extend(theme::ROLES.map(Row::Color));
    rows
}

impl Row {
    /// 這一列在設定檔的哪個表（`[calendar]`、`[month]`、`[colors]`）；最上層的（`theme`）是 `None`。設定頁也照這個分段
    pub fn section(self) -> Option<&'static str> {
        match self {
            Self::Theme => None,
            Self::RelativeWeeks => Some("calendar"),
            Self::GridLines | Self::ItemIcons | Self::CenterCursor => Some("month"),
            Self::Color(_) => Some("colors"),
        }
    }

    /// 顯示在左邊的名字 = 設定檔裡的名字（有 test 釘住）
    pub fn name(self) -> &'static str {
        match self {
            Self::Theme => "theme",
            Self::RelativeWeeks => "relative_weeks",
            Self::GridLines => "grid_lines",
            Self::ItemIcons => "item_icons",
            Self::CenterCursor => "center_cursor",
            Self::Color(role) => role,
        }
    }

    /// 開關那幾列在 `settings` 裡的那個 `bool`（`&mut`：改值、還原用）；`theme`、顏色不是開關，是 `None`
    fn switch(self, settings: &mut Settings) -> Option<&mut bool> {
        match self {
            Self::Theme | Self::Color(_) => None,
            Self::RelativeWeeks => Some(&mut settings.calendar.relative_weeks),
            Self::GridLines => Some(&mut settings.month.grid_lines),
            Self::ItemIcons => Some(&mut settings.month.item_icons),
            Self::CenterCursor => Some(&mut settings.month.center_cursor),
        }
    }

    /// 現在的值（畫在右邊的字）：開關是 `on` / `off`，`theme` 是那套 theme 的名字；
    /// 顏色是色票裡的名字（不在色票裡的寫顏色本身，例：`#010203`）
    pub fn value(self, settings: &Settings) -> String {
        let on_off = |on: bool| if on { "on" } else { "off" }.to_string();
        match self {
            Self::Theme => settings.theme.clone(),
            Self::Color(_) => {
                let Some(color) = self.color(settings) else {
                    return String::new();
                };
                theme::choices(&settings.theme)
                    .into_iter()
                    .find(|&(_, choice)| choice == color)
                    .map_or_else(|| theme::color_text(color), |(name, _)| name)
            }
            Self::RelativeWeeks => on_off(settings.calendar.relative_weeks),
            Self::GridLines => on_off(settings.month.grid_lines),
            Self::ItemIcons => on_off(settings.month.item_icons),
            Self::CenterCursor => on_off(settings.month.center_cursor),
        }
    }

    /// 顏色那幾列現在的顏色（theme + 蓋掉的）；其他列是 `None`
    pub fn color(self, settings: &Settings) -> Option<ratatui::style::Color> {
        let Self::Color(role) = self else {
            return None;
        };
        Theme::build(&settings.theme, &settings.colors)
            .ok()?
            .color(role)
    }

    /// 改這一列的值：開關 on ⇄ off；`theme` 照內建的順序換下一套（`step` = 1）/ 上一套（-1）；
    /// 顏色在 theme 的色票（沒有色票就是 terminal 的 16 色）裡換下一個 / 上一個。都是頭尾相接
    fn change(self, settings: &mut Settings, step: isize) {
        if let Some(switch) = self.switch(settings) {
            *switch = !*switch;
            return;
        }
        match self {
            Self::Color(role) => {
                let choices = theme::choices(&settings.theme);
                let current = self.color(settings);
                // 現在的顏色不在色票裡（例：手寫的色碼）就從頭（或尾）開始
                let next = match choices
                    .iter()
                    .position(|&(_, color)| Some(color) == current)
                {
                    Some(index) => wrap(index, step, choices.len()),
                    None if step > 0 => 0,
                    None => choices.len() - 1,
                };
                let color = choices[next].1;
                // 跟 theme 本來的顏色一樣就不記（設定檔只記改過的）
                let base = Theme::builtin(&settings.theme).and_then(|theme| theme.color(role));
                if Some(color) == base {
                    settings.colors.remove(role);
                } else {
                    settings
                        .colors
                        .insert(role.to_string(), theme::color_text(color));
                }
            }
            _ => {
                let names = theme::names();
                let index = names
                    .iter()
                    .position(|&name| name == settings.theme)
                    .unwrap_or(0);
                settings.set_theme(names[wrap(index, step, names.len())]);
            }
        }
    }

    /// 這一列還原成預設值（顏色 = 拿掉蓋掉的，回到 theme 的顏色）
    fn reset(self, settings: &mut Settings) {
        let defaults = Settings::default();
        if let Self::Color(role) = self {
            settings.colors.remove(role);
            return;
        }
        match self.switch(settings) {
            Some(switch) => *switch = self.value(&defaults) == "on",
            None => settings.set_theme(&defaults.theme),
        }
    }
}

/// 第 `index` 個往前（`step` = −1）/ 往後（+1）走，頭尾相接；`len` 個裡面的第幾個
fn wrap(index: usize, step: isize, len: usize) -> usize {
    index
        .cast_signed()
        .saturating_add(step)
        .rem_euclid(len.cast_signed())
        .cast_unsigned()
}

impl App {
    /// `,`：打開設定頁，選第一列
    pub(super) fn open_settings(&mut self) {
        self.settings_page = Some(SettingsPage::default());
        self.settings_opened = Some(self.settings.clone());
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
                self.apply_theme();
            }
            self.settings_page = Some(SettingsPage {
                confirm_reset: false,
                ..page
            });
            return;
        }
        let rows = rows();
        let row = rows[page.row.min(rows.len() - 1)];
        match key.code {
            // 離開前一樣存回設定檔
            KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.close_settings();
                self.should_quit = true;
            }
            KeyCode::Char('j') | KeyCode::Down => self.choose_setting(1),
            KeyCode::Char('k') | KeyCode::Up => self.choose_setting(-1),
            KeyCode::Char('h') | KeyCode::Left => self.change_setting(row, -1),
            KeyCode::Char('l' | ' ') | KeyCode::Right | KeyCode::Enter => {
                self.change_setting(row, 1);
            }
            KeyCode::Char('r') => {
                row.reset(&mut self.settings);
                self.apply_theme();
            }
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

    /// 改 `row` 的值（`step` 見 `Row::change`），換了 theme 畫面的顏色馬上跟著換
    fn change_setting(&mut self, row: Row, step: isize) {
        row.change(&mut self.settings, step);
        self.apply_theme();
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
                    Some(row) if row == page.row => self.change_setting(rows()[row], 1),
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
            page.row = page.row.saturating_add_signed(step).min(rows().len() - 1);
        }
    }

    /// 關掉設定頁，把打開之後改過的設定存回設定檔（只改那幾行；沒改東西就不寫）；存不了就在最下面一列說
    fn close_settings(&mut self) {
        self.settings_page = None;
        let before = self
            .settings_opened
            .take()
            .unwrap_or_else(|| self.settings.clone());
        if let Some(path) = &self.config_path
            && let Err(error) = config::save(path, &before, &self.settings)
        {
            self.notice = Some(Notice::SettingsFailed(error.kind()));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{CalendarSettings, MonthSettings};
    use crate::texts::Lang;
    use crate::theme::Theme;
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
    fn closing_saves_only_what_changed_and_says_when_it_cannot() {
        let dir = crate::files::test_dir("settings-save");
        let path = dir.join("config-v2.toml");
        // 打開看看、沒改就關掉：不建檔案
        let mut app = app_with_settings();
        app.config_path = Some(path.clone());
        press(&mut app, KeyCode::Esc);
        assert!(!path.exists());
        // 改了 relative_weeks 再關掉：只寫那一行
        press(&mut app, KeyCode::Char(','));
        press(&mut app, KeyCode::Char('j'));
        press(&mut app, KeyCode::Char('l'));
        press(&mut app, KeyCode::Char('q'));
        assert_eq!(
            std::fs::read_to_string(&path).expect("read"),
            "[calendar]\nrelative_weeks = true\n"
        );
        assert_eq!(app.notice, None);
        // Ctrl+C 離開前也存
        press(&mut app, KeyCode::Char(','));
        press(&mut app, KeyCode::Char('j'));
        press(&mut app, KeyCode::Char('j'));
        press(&mut app, KeyCode::Char('l'));
        app.handle_key(KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL));
        assert!(app.should_quit);
        assert!(
            std::fs::read_to_string(&path)
                .expect("read")
                .contains("grid_lines = false")
        );
        // 設定檔在別處改壞了：不存，最下面一列說存不了，檔案原封不動
        std::fs::write(&path, "[month]\ngrid_line = false\n").expect("write");
        let mut app = app_with_settings();
        app.config_path = Some(path.clone());
        press(&mut app, KeyCode::Char('l'));
        press(&mut app, KeyCode::Esc);
        assert_eq!(
            app.notice,
            Some(Notice::SettingsFailed(std::io::ErrorKind::InvalidData))
        );
        assert_eq!(
            std::fs::read_to_string(&path).expect("read"),
            "[month]\ngrid_line = false\n"
        );
        std::fs::remove_dir_all(&dir).expect("clean up");
    }

    #[test]
    fn color_rows_go_through_the_palette_and_keep_only_overrides() {
        let mut app = app_with_settings();
        // 第 6 列是 text 的顏色：l 換成色票的下一個（subtext0），記進 [colors]，畫面馬上跟著換
        for _ in 0..5 {
            press(&mut app, KeyCode::Char('j'));
        }
        press(&mut app, KeyCode::Char('l'));
        assert_eq!(
            app.settings.colors.get("text").map(String::as_str),
            Some("#a6adc8")
        );
        assert_eq!(app.theme.text, ratatui::style::Color::Rgb(0xa6, 0xad, 0xc8));
        assert_eq!(Row::Color("text").value(&app.settings), "subtext0");
        // h 回到 theme 本來的顏色：不記（設定檔只記改過的）
        press(&mut app, KeyCode::Char('h'));
        assert!(app.settings.colors.is_empty());
        // 再 h：頭尾相接到色票的最後一個（mauve）；r 還原
        press(&mut app, KeyCode::Char('h'));
        assert_eq!(
            app.settings.colors.get("text").map(String::as_str),
            Some("#cba6f7")
        );
        press(&mut app, KeyCode::Char('r'));
        assert!(app.settings.colors.is_empty());
        assert_eq!(app.theme, Theme::catppuccin_mocha());
    }

    #[test]
    fn switching_the_theme_keeps_the_overridden_colors() {
        // 設定檔手寫了 mocha 的色票名字；換到沒有色票的 terminal：先寫成顏色本身，換過去還是同一個顏色、組得出 theme
        let mut app = app_with_settings();
        app.settings
            .colors
            .insert("today".to_string(), "surface0".to_string());
        app.apply_theme();
        press(&mut app, KeyCode::Char('h'));
        assert_eq!(app.settings.theme, "terminal");
        assert_eq!(
            app.settings.colors.get("today").map(String::as_str),
            Some("#313244")
        );
        assert_eq!(
            app.theme.today,
            ratatui::style::Color::Rgb(0x31, 0x32, 0x44)
        );
        assert_eq!(app.theme.text, ratatui::style::Color::Reset);
        // terminal 沒有色票：顏色在 16 色裡換
        assert_eq!(Row::Color("today").value(&app.settings), "#313244");
    }

    #[test]
    fn r_on_the_theme_row_keeps_the_overridden_colors_too() {
        // 設定檔寫了 gruvbox 色票的名字：r 回到預設的 mocha，蓋掉的顏色先寫成顏色本身，組得出 theme（存回去下次也打得開）
        let mut app = app_with_settings();
        app.settings.theme = "gruvbox-dark".to_string();
        app.settings
            .colors
            .insert("today".to_string(), "light1".to_string());
        app.apply_theme();
        press(&mut app, KeyCode::Char('r'));
        assert_eq!(app.settings.theme, "catppuccin-mocha");
        assert_eq!(
            app.settings.colors.get("today").map(String::as_str),
            Some("#ebdbb2")
        );
        assert_eq!(
            app.theme.today,
            ratatui::style::Color::Rgb(0xeb, 0xdb, 0xb2)
        );
    }

    #[test]
    fn a_terminal_color_stays_itself_in_a_theme_with_the_same_palette_name() {
        // terminal 的 cyan 換到 tokyo-night（色票也有 cyan = #7dcfff）：還是 terminal 的 cyan，不會被色票的同名顏色換掉
        let mut app = app_with_settings();
        app.settings.theme = "terminal".to_string();
        app.settings
            .colors
            .insert("weekend".to_string(), "cyan".to_string());
        app.settings.set_theme("tokyo-night");
        app.apply_theme();
        assert_eq!(app.settings.theme, "tokyo-night");
        assert_eq!(app.theme.weekend, ratatui::style::Color::Cyan);
    }

    #[test]
    fn every_switch_has_a_row() {
        // 把 `Settings` 拆開到每個欄位：多加了設定、這裡沒跟著加就編譯不過，提醒要在 `rows()` 加一列
        let Settings {
            theme: _,
            colors: _,
            calendar: CalendarSettings { relative_weeks: _ },
            month:
                MonthSettings {
                    grid_lines: _,
                    item_icons: _,
                    center_cursor: _,
                },
        } = Settings::default();
        assert_eq!(rows().len(), 5 + theme::ROLES.len());
    }

    #[test]
    fn row_names_are_the_config_file_names() {
        // 每一列改一次值，照「[表] 名字 = 新的值」寫成設定檔，讀回來剛好是改過的設定
        for row in rows() {
            let mut changed = Settings::default();
            row.change(&mut changed, 1);
            let value = match row {
                Row::Theme => format!("\"{}\"", changed.theme),
                Row::Color(role) => format!("\"{}\"", changed.colors[role]),
                _ => (row.value(&changed) == "on").to_string(),
            };
            let table = row
                .section()
                .map(|section| format!("[{section}]\n"))
                .unwrap_or_default();
            let text = format!("{table}{} = {value}\n", row.name());
            let read: Settings = toml::from_str(&text).expect(&text);
            assert_eq!(read, changed, "{text}");
        }
    }

    #[test]
    fn comma_opens_it_and_hl_change_the_chosen_row() {
        let mut app = app_with_settings();
        assert_eq!(app.settings_page, Some(SettingsPage::default()));
        // 第一列 theme：l 換下一套（畫面的顏色馬上跟著換）、到最後一套再 l 回到第一套、h 往回
        press(&mut app, KeyCode::Char('l'));
        assert_eq!(app.settings.theme, "catppuccin-latte");
        assert_eq!(
            Some(&app.theme),
            Theme::builtin("catppuccin-latte").as_ref()
        );
        // 照 `theme::BUILTIN` 的順序：latte → gruvbox-dark → tokyo-night → terminal → 回到 mocha
        for expected in [
            "gruvbox-dark",
            "tokyo-night",
            "terminal",
            "catppuccin-mocha",
        ] {
            press(&mut app, KeyCode::Char('l'));
            assert_eq!(app.settings.theme, expected);
        }
        press(&mut app, KeyCode::Char('h'));
        assert_eq!(app.settings.theme, "terminal");
        assert_eq!(app.theme.text, ratatui::style::Color::Reset);
        // j 到 relative_weeks：l 打開、h 再關掉
        press(&mut app, KeyCode::Char('j'));
        press(&mut app, KeyCode::Char('l'));
        assert!(app.settings.calendar.relative_weeks);
        press(&mut app, KeyCode::Char('h'));
        assert!(!app.settings.calendar.relative_weeks);
        // j 到 grid_lines，Space 關掉；k 回到第一列；到頭不會再往上
        press(&mut app, KeyCode::Char('j'));
        press(&mut app, KeyCode::Char(' '));
        assert!(!app.settings.month.grid_lines);
        for _ in 0..3 {
            press(&mut app, KeyCode::Char('k'));
        }
        assert_eq!(app.settings_page.map(|page| page.row), Some(0));
        // 到尾也停在最後一列
        for _ in 0..40 {
            press(&mut app, KeyCode::Down);
        }
        assert_eq!(
            app.settings_page.map(|page| page.row),
            Some(rows().len() - 1)
        );
    }

    #[test]
    fn r_resets_a_row_and_capital_r_then_y_resets_everything() {
        let mut app = app_with_settings();
        // theme 換成 latte，relative_weeks 打開、grid_lines 關掉
        press(&mut app, KeyCode::Char('l'));
        press(&mut app, KeyCode::Char('j'));
        press(&mut app, KeyCode::Char('l'));
        press(&mut app, KeyCode::Char('j'));
        press(&mut app, KeyCode::Char('l'));
        // r：只還原 grid_lines，其他的不動
        press(&mut app, KeyCode::Char('r'));
        assert!(app.settings.month.grid_lines);
        assert!(app.settings.calendar.relative_weeks);
        assert_eq!(app.settings.theme, "catppuccin-latte");
        // R 之後按別的鍵：只是取消，那個鍵也不做事（j 不會移動）
        press(&mut app, KeyCode::Char('R'));
        assert_eq!(app.settings_page.map(|page| page.confirm_reset), Some(true));
        press(&mut app, KeyCode::Char('j'));
        assert_eq!(
            app.settings_page.map(|page| page.confirm_reset),
            Some(false)
        );
        assert_eq!(app.settings_page.map(|page| page.row), Some(2));
        assert!(app.settings.calendar.relative_weeks);
        // R 再 y：全部還原，畫面的顏色也回到預設的 theme
        press(&mut app, KeyCode::Char('R'));
        press(&mut app, KeyCode::Char('y'));
        assert_eq!(app.settings, Settings::default());
        assert_eq!(app.theme, Theme::catppuccin_mocha());
        assert!(app.settings_page.is_some());
        // theme 那列也能單獨 r
        for _ in 0..2 {
            press(&mut app, KeyCode::Char('k'));
        }
        press(&mut app, KeyCode::Char('h'));
        press(&mut app, KeyCode::Char('r'));
        assert_eq!(app.settings.theme, "catppuccin-mocha");
        assert_eq!(app.theme, Theme::catppuccin_mocha());
    }

    #[test]
    fn clicking_chooses_a_row_then_changes_it_and_clicking_outside_closes() {
        // 100 × 30：框（46 × 38）比畫面高、縮成 30 高，在 (27, 0)；grid_lines 是清單第 6 行：0 + 框線 1 + padding 1 + 6 = 8
        let mut app = app_with_settings();
        let click = |app: &mut App, column, row| {
            app.handle_mouse(MouseEvent {
                kind: MouseEventKind::Down(MouseButton::Left),
                column,
                row,
                modifiers: KeyModifiers::NONE,
            });
        };
        click(&mut app, 50, 8);
        assert_eq!(app.settings_page.map(|page| page.row), Some(2));
        assert!(app.settings.month.grid_lines);
        // 再點一次選到的那列：改值
        click(&mut app, 50, 8);
        assert!(!app.settings.month.grid_lines);
        // 滾輪：選下一列
        app.handle_mouse(MouseEvent {
            kind: MouseEventKind::ScrollDown,
            column: 50,
            row: 8,
            modifiers: KeyModifiers::NONE,
        });
        assert_eq!(app.settings_page.map(|page| page.row), Some(3));
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
